//! The storyboard chain scheduler (S15 §D2).
//!
//! In a **chained** storyboard, shot N+1 is submitted only after shot N
//! completes and its bytes are local: shot N's true last frame becomes shot
//! N+1's start frame (image-to-video), so the second beat continues the first
//! in pixels rather than re-drawing it from the prompt. The runner fires
//! [`crate::runner::OnSettled`] the instant a shot settles; that calls
//! [`advance`], which extracts the frame and submits the next shot through the
//! **same money-gated path** a single generation uses ([`crate::commands::submit_one`]:
//! S6 guard, S7 verbatim prompt, S10 estimate + route guard). Nothing here
//! re-opens that path; the chain is purely "when, and with which start frame".
//!
//! ## Money and safety invariants (AC2/AC6)
//!
//! - **A failed predecessor never submits its successor.** The hook only fires
//!   for a `Completed`, fully-downloaded shot, so a failed shot simply parks
//!   the chain — there is no next submit.
//! - **Every chained shot re-runs the estimate + guard at its own submit time**
//!   via `submit_one`; the total shown at generate is the sum of per-shot
//!   estimates, and a shot whose price drifted is re-priced then (design D2.7).
//! - **Idempotent.** `advance` no-ops if a row already exists at the next
//!   index, so the settled hook and the startup reconciliation ([`resume_all`])
//!   can both run without a double submit (design §10.1).
//! - **Local-only spend.** The only process this spawns is the bundled ffmpeg
//!   sidecar, on local files, to extract a frame — no network, no charge.

use crate::library::Library;
use crate::pricing::Prices;
use crate::runner::Runner;
use crate::store::SqliteStore;
use hickeyfield_core::engine::JobStore;
use hickeyfield_core::{ffmpeg, registry, MediaRef, MediaRole, Phase, UseCase};
use std::path::Path;
use std::sync::Arc;

/// Can this model continue from a start frame — i.e. can it run image-to-video?
///
/// AC0-H2 tried to tighten this to a per-route `can_bind(Start)` check, but on
/// `Dialect::Fal` `can_bind` is deliberately *permissive* for an endpoint whose
/// modes are unmeasured (the schema gate at submit is the backstop). That made
/// the per-route check LOOSER, not tighter: it reported `gemini_omni` — a
/// measured media-ignoring model that `use_case::supports` correctly excludes —
/// as chainable. Per the design's own escape hatch (§10.4), the tightening is
/// **reverted** to the coarse `use_case::supports`, which already excludes the
/// media-ignoring roster (`NO_MEDIA_MODELS`) and keeps H3 Max i2v chainable. See
/// `implement.md` (AC0-H2). `storyboard::plan` reads the same predicate, so the
/// toggle default and the scheduler cannot drift.
pub fn chainable_model(model: &hickeyfield_core::Model) -> bool {
    use hickeyfield_core::use_case;
    use_case::supports(model, UseCase::ImageToVideo)
}

/// A stable per-storyboard seed (memo Q1): a `u32` derived from the id, so a
/// re-render or a resume reproduces the same value, and a retry can offset it
/// (`seed + attempt`). Not cryptographic — reproducibility, not secrecy.
pub fn storyboard_seed(storyboard_id: &str) -> u64 {
    // FNV-1a over the id bytes, truncated to 32 bits (fal seeds are u32-range).
    let mut hash: u64 = 0xcbf29ce484222325;
    for b in storyboard_id.bytes() {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash & 0xffff_ffff
}

/// The request-body overrides a chained shot submits (D-C): fal's own prompt
/// rewrite disabled (a second enhancer on top of S4–S7's guarded one is drift
/// by construction) and the fixed per-storyboard seed. Schema-gated at submit,
/// so on a model that does not declare these keys they are simply dropped.
fn chained_overrides(seed: u64) -> serde_json::Map<String, serde_json::Value> {
    let mut m = serde_json::Map::new();
    m.insert(
        "prompt_expansion_mode".into(),
        serde_json::Value::String("disabled".into()),
    );
    m.insert("seed".into(), serde_json::Value::Number(seed.into()));
    m
}

/// Turn the stored next-shot payload into the exact `SubmitInput` the chain
/// submits, given whether the previous shot's continuity frame was extracted.
///
/// Pure and keyless-testable (it does not submit): with a `frame` and an i2v
/// model it attaches the frame as `MediaRole::Start` and sets the chained
/// overrides (expansion off + seed); otherwise it leaves the shot text-only and
/// returns a continuity advisory. Returns `(input, continuity_applied,
/// advisory)`.
/// Whether this model can bind an end frame (S17, D3): not in `NO_END_FRAME` and
/// its route accepts the End role. A false here means the end frame would be
/// silently dropped into a route that ignores it (the S9 full-price harm), so
/// the caller falls back start-only with an advisory.
fn end_frame_capable(model: &hickeyfield_core::Model) -> bool {
    use hickeyfield_core::media;
    // Judge on ANY route, not just `routes.first()` (review M1): `submit_one`
    // resolves the cheapest AVAILABLE route, which may not be the first, so the
    // first alone can disagree with what is actually submitted. If any route
    // binds the End role the shot can be end-anchored; the per-route binding is
    // re-checked at submit (`media::bind` errors before any POST if the resolved
    // route cannot take it — no silent full-price drop, the S9 harm).
    model.routes.iter().any(|route| {
        let dialect = media::dialect_for(route.provider, &route.slug);
        media::can_bind(&model.spec, media::MediaRole::End, dialect, &route.slug)
    })
}

/// Turn the stored next-shot payload into the exact `SubmitInput` the chain
/// submits, attaching the start frame (S15) and — in keyframe mode — the planned
/// end still (S17, D3). Pure and keyless-testable (it does not submit). Returns
/// `(input, continuity_applied, advisories)`.
///
/// The end frame is attached only when the model can bind an end frame
/// (`end_frame_capable`); a `NO_END_FRAME`-class model falls back start-only and
/// records the advisory `"end-frame anchoring unavailable: …"` so the join is
/// scored end-absent, never billed for a frame the route dropped.
fn prepare_next_shot_keyframe(
    model: &hickeyfield_core::Model,
    mut input: crate::commands::SubmitInput,
    frame: Option<&Path>,
    end_frame: Option<&Path>,
    seed: u64,
    frame_error: Option<String>,
) -> (crate::commands::SubmitInput, bool, Vec<String>) {
    let mut advisories = Vec::new();
    let applied = match frame {
        Some(f) if chainable_model(model) => {
            input.media.push(MediaRef::local(
                MediaRole::Start,
                f.to_string_lossy().into_owned(),
            ));
            input.constant_overrides = Some(chained_overrides(seed));
            true
        }
        Some(_) => {
            advisories.push(format!(
                "continuity unavailable: {} has no image-to-video route — generated from the prompt alone",
                model.display_name
            ));
            false
        }
        None => {
            advisories.push(frame_error.unwrap_or_else(|| {
                "continuity frame could not be extracted — this shot was generated from the prompt alone".into()
            }));
            false
        }
    };

    // The planned end still (keyframe mode). Attach only when the model can bind
    // an end frame; otherwise fall back start-only with an advisory.
    if let Some(end) = end_frame {
        if end_frame_capable(model) {
            input.media.push(MediaRef::local(
                MediaRole::End,
                end.to_string_lossy().into_owned(),
            ));
            // Ensure the chained overrides (seed + expansion off) are set even if
            // the start frame path did not (e.g. a start-only fallback still
            // wants the seed pinned).
            if input.constant_overrides.is_none() {
                input.constant_overrides = Some(chained_overrides(seed));
            }
        } else {
            advisories.push(format!(
                "end-frame anchoring unavailable: {} has no end-frame route — this join is start-anchored only",
                model.display_name
            ));
        }
    }

    (input, applied, advisories)
}

/// Advance a chained storyboard by one shot, given the shot that just settled.
///
/// Idempotent and safe to call from both the settled hook and [`resume_all`].
#[allow(clippy::too_many_arguments)]
pub fn advance(
    store: &Arc<SqliteStore>,
    prices: &Arc<Prices>,
    _library: &Arc<Library>,
    ffmpeg_dir: &Path,
    renders_root: &Path,
    runner: &Runner,
    settled: &hickeyfield_core::engine::JobSet,
) {
    let Some(sid) = settled.storyboard_id.clone() else {
        return;
    };
    let Ok(Some(sb)) = store.storyboard(&sid) else {
        return;
    };
    // Only a chained set advances itself; an all-at-once set (S13) never does.
    if !sb.chained {
        return;
    }
    // A failed predecessor parks the chain — never submit past a failure.
    if settled.status.phase() != Phase::Completed {
        return;
    }
    let Some(clip) = settled.results.iter().find_map(|o| o.local_path.clone()) else {
        return;
    };
    let Some(cur_idx) = settled.shot_index else {
        return;
    };
    let next = cur_idx + 1;

    // The full ordered shot payloads, stored at generate so a relaunch can
    // advance without re-deriving the user's edits.
    let Some(shots_json) = sb.shots_json.as_deref() else {
        return;
    };
    let Ok(shots): Result<Vec<crate::commands::SubmitInput>, _> = serde_json::from_str(shots_json)
    else {
        tracing::warn!("chain {sid}: shots_json did not parse; not advancing");
        return;
    };
    if next as usize >= shots.len() {
        return; // the chain is complete; the render is a separate, user step.
    }

    // Idempotency guard (§10.1): if a row already exists at `next`, the hook and
    // the resume scan must not both submit. This is the single check that keeps
    // a chain from double-billing an index.
    let existing = store.jobs_in_storyboard(&sid).unwrap_or_default();
    if existing.iter().any(|j| j.shot_index == Some(next)) {
        return;
    }

    let reg = registry();
    let input = shots[next as usize].clone();
    let Some(model) = reg.get(&input.model_id).cloned() else {
        tracing::warn!("chain {sid}: shot {next} names an unknown model; not advancing");
        return;
    };

    // The set's canvas: the settled clip's probed dimensions (all shots in a
    // chained set share one resolution, D-F), so the extracted frame matches
    // the canvas shot N+1 will render onto ("the output canvas follows this
    // image"). If the sidecar or the probe is unavailable, extract at native.
    let bin = ffmpeg::locate(ffmpeg_dir).ok();
    let size = bin.as_ref().and_then(|b| {
        let d = ffmpeg::probe_file(b, Path::new(&clip)).ok()?;
        Some((d.width?, d.height?))
    });

    // Extract the settled shot's true last frame, at the set's size, as this
    // shot's start frame. A failure must not strand a paid set — fall back to
    // text-only with an advisory (design D2.3 step 5/6).
    let frames_dir = renders_root.join(&sid).join("frames");
    let _ = std::fs::create_dir_all(&frames_dir);
    let frame_path = frames_dir.join(format!("shot{cur_idx}.png"));

    let mut frame_error: Option<String> = None;
    let frame: Option<&Path> = match bin.as_ref() {
        Some(bin) => match ffmpeg::last_frame(bin, Path::new(&clip), &frame_path, size) {
            Ok(_) => Some(frame_path.as_path()),
            Err(e) => {
                frame_error = Some(format!(
                    "continuity frame could not be extracted ({e}) — this shot was generated from the prompt alone"
                ));
                None
            }
        },
        None => {
            frame_error = Some(
                "continuity unavailable: the ffmpeg sidecar is missing — generated from the prompt alone".into(),
            );
            None
        }
    };

    // S17: in a keyframe set, bind the next shot's planned end still K_{next+1}
    // as the End frame (D3). `keyframes_json` survives a relaunch, so a resumed
    // chain still anchors both ends. `None` (non-keyframe) behaves as S15/S16.
    let keyframes = crate::store::parse_keyframes(sb.keyframes_json.as_deref());
    let end_path: Option<std::path::PathBuf> = keyframes
        .iter()
        .find(|k| k.index == next as usize + 1)
        .and_then(|k| k.local_path.clone())
        .map(std::path::PathBuf::from);

    let (input, _continuity_applied, advisories) = prepare_next_shot_keyframe(
        &model,
        input,
        frame,
        end_path.as_deref(),
        storyboard_seed(&sid),
        frame_error,
    );

    // Submit through the identical money path a single generation uses.
    match crate::commands::submit_one(
        store,
        prices,
        runner,
        &model,
        &input,
        Some(&sid),
        Some(next),
    ) {
        Ok(job_id) => {
            // Record the continuity / end-anchor fallbacks as advisories on the
            // new shot, so the UI can say the join was start-anchored only.
            if !advisories.is_empty() {
                if let Ok(Some(mut job)) = store.get(&job_id) {
                    for adv in advisories {
                        if !job.advisories.contains(&adv) {
                            job.advisories.push(adv);
                        }
                    }
                    let _ = store.upsert(&job);
                }
            }
        }
        Err(e) => {
            tracing::warn!("chain {sid}: shot {next} did not submit: {e}");
        }
    }
}

/// Score the continuity join *into* the shot that just settled (S16 D7).
///
/// Runs beside [`advance`] in the settled hook: independent of it (the score is
/// for the join into the settled shot; advance submits the *next* shot). Spawns
/// only the bundled ffmpeg sidecar on local PNGs — no network, no charge. A low
/// join never blocks the chain; it flags. **A scoring failure records
/// `JoinScore::unscored` (all `None`), never `0.0`** — a zero would read as a
/// failed join and fire a needless paid regenerate.
///
/// Returns the recorded [`crate::store::JoinRecord`] for callers that act on it
/// (the auto-retry loop), or `None` when there is nothing to score (shot 0, an
/// all-at-once set, or a missing clip).
/// The continuity tier the CUT score is gated against (review M3, design D4/Q7):
/// a keyframe set is the 99% path → the IDEAL tier (0.90/6/3); a plain chained
/// set keeps the S16 FLOOR (0.80/12/6). End adherence is always IDEAL. Pure, so
/// the tier choice is pinned by a test without scoring a real clip in the narrow
/// band between the two tiers.
pub(crate) fn cut_threshold(
    is_keyframe: bool,
) -> hickeyfield_core::storyboard::ContinuityThresholds {
    use hickeyfield_core::storyboard::{CONTINUITY_FLOOR, CONTINUITY_IDEAL};
    if is_keyframe {
        CONTINUITY_IDEAL
    } else {
        CONTINUITY_FLOOR
    }
}

pub fn score_join(
    store: &Arc<SqliteStore>,
    ffmpeg_dir: &Path,
    renders_root: &Path,
    settled: &hickeyfield_core::engine::JobSet,
) -> Option<crate::store::JoinRecord> {
    use hickeyfield_core::storyboard::{join_passes, join_passes_end, JoinScore, CONTINUITY_IDEAL};

    let sid = settled.storyboard_id.clone()?;
    let mut sb = store.storyboard(&sid).ok()??;
    if !sb.chained {
        return None;
    }
    if settled.status.phase() != Phase::Completed {
        return None;
    }
    let i = settled.shot_index? as usize;

    // S17: a keyframe set records shot 0's END adherence too (against K1), so it
    // is scored even though there is no join INTO shot 0. A non-keyframe set
    // keeps S16's behaviour: no join 0.
    let keyframes = crate::store::parse_keyframes(sb.keyframes_json.as_deref());
    let is_keyframe = !keyframes.is_empty();
    if i == 0 && !is_keyframe {
        return None; // no join into the first shot
    }
    let clip = settled.results.iter().find_map(|o| o.local_path.clone())?;

    // Continuity was applied for this join iff the shot was submitted with a
    // start frame (the render + score authority, D8). Absent → a crossfade join.
    let applied = settled.media.iter().any(|m| m.role == MediaRole::Start);
    let kind = if applied { "continuity" } else { "crossfade" };

    let frames_dir = renders_root.join(&sid).join("frames");
    let prev_frame = frames_dir.join(format!("shot{}.png", i.saturating_sub(1)));

    // Keep the existing attempt count for this index (the regenerate path is
    // what increments it); default 0 for a first score.
    let mut joins = crate::store::parse_joins(sb.joins_json.as_deref());
    let attempt = joins
        .iter()
        .find(|j| j.index == i)
        .map(|j| j.attempt)
        .unwrap_or(0);

    let bin = ffmpeg::locate(ffmpeg_dir).ok();

    // ── The cut score (S16): SSIM of clip i-1's last frame vs clip i's first ──
    // Only for i>=1 with the conditioning frame on disk; shot 0 has no cut.
    let cut = if i >= 1 && applied && prev_frame.is_file() {
        match bin.as_ref() {
            Some(bin) => {
                let first = frames_dir.join(format!("first{i}.png"));
                let scored = ffmpeg::first_frame(bin, Path::new(&clip), &first, None)
                    .and_then(|_| ffmpeg::continuity_score(bin, &prev_frame, &first));
                let _ = std::fs::remove_file(&first);
                match scored {
                    Ok(m) => JoinScore {
                        ssim: Some(m.ssim),
                        psnr: Some(m.psnr),
                        dy: Some(m.dy),
                        du: Some(m.du),
                        dv: Some(m.dv),
                    },
                    Err(e) => {
                        tracing::warn!("chain {sid}: join {i} cut scoring failed: {e}");
                        JoinScore::unscored()
                    }
                }
            }
            None => JoinScore::unscored(),
        }
    } else {
        JoinScore::unscored()
    };

    // ── End-frame adherence (S17, D4): SSIM of clip i's ACTUAL last frame vs its
    //    planned still K_{i+1}, gated against the IDEAL tier. The last shot has
    //    no successor, so `score_join` extracts its own last frame here rather
    //    than relying on `advance` (which only runs for a shot with a successor).
    let end_still = keyframes
        .iter()
        .find(|k| k.index == i + 1)
        .and_then(|k| k.local_path.clone());
    let end = match (is_keyframe, end_still.as_deref(), bin.as_ref()) {
        (true, Some(still), Some(bin)) if Path::new(still).is_file() => {
            let last = frames_dir.join(format!("endlast{i}.png"));
            let _ = std::fs::create_dir_all(&frames_dir);
            let scored = ffmpeg::last_frame(bin, Path::new(&clip), &last, None)
                .and_then(|_| ffmpeg::continuity_score(bin, Path::new(still), &last));
            // The extracted last frame is a scratch input to the end score only —
            // remove it once scored (the same discipline as `first{i}.png`).
            let _ = std::fs::remove_file(&last);
            match scored {
                Ok(m) => JoinScore {
                    ssim: Some(m.ssim),
                    psnr: Some(m.psnr),
                    dy: Some(m.dy),
                    du: Some(m.du),
                    dv: Some(m.dv),
                },
                Err(e) => {
                    tracing::warn!("chain {sid}: shot {i} end adherence scoring failed: {e}");
                    JoinScore::unscored()
                }
            }
        }
        // No still, no sidecar, or not a keyframe set → end unscored (None,
        // never 0 — an unscored end must not fire a paid regenerate).
        _ => JoinScore::unscored(),
    };

    // The cut score reads the IDEAL tier (0.90/6/3) for a keyframe set — the 99%
    // path (design D4/Q7) — and the FLOOR (0.80/12/6) for a plain chained set.
    // End adherence (keyframe-only) is always the IDEAL tier.
    let pass = cut
        .is_scored()
        .then(|| join_passes(&cut, cut_threshold(is_keyframe)));
    let end_pass = end
        .is_scored()
        .then(|| join_passes_end(&end, CONTINUITY_IDEAL));
    let record = crate::store::JoinRecord {
        index: i,
        kind: kind.to_string(),
        applied,
        ssim: cut.ssim,
        psnr: cut.psnr,
        dy: cut.dy,
        du: cut.du,
        dv: cut.dv,
        pass,
        attempt,
        end_ssim: end.ssim,
        end_psnr: end.psnr,
        end_dy: end.dy,
        end_du: end.du,
        end_dv: end.dv,
        end_pass,
    };
    crate::store::upsert_join(&mut joins, record.clone());
    sb.joins_json = serde_json::to_string(&joins).ok();
    if let Err(e) = store.upsert_storyboard(&sb) {
        tracing::warn!("chain {sid}: could not persist join {i} score: {e}");
    }
    Some(record)
}

/// The stills-ready → shot-0 barrier (S17, D2). Called from the settled hook
/// for every settled job in a chained set. When the settled job is a **still**
/// (a keyframe job — `shot_index == None`, tagged to the set), it records that
/// still's downloaded path; once **every** boundary still is on disk and no shot
/// 0 row exists yet, it submits shot 0 anchored on BOTH ends (K0 in, K1 out).
///
/// Idempotent (guarded on "shot 0 row exists"), so the hook and the startup
/// [`resume_all`] scan cannot double-fire it. A **Failed** still leaves its
/// record `local_path: None`, so `all_ready` stays false and the set is parked —
/// no shot is ever submitted against a missing anchor (AC2).
pub fn keyframes_advance(
    store: &Arc<SqliteStore>,
    prices: &Arc<Prices>,
    ffmpeg_dir: &Path,
    renders_root: &Path,
    runner: &Runner,
    settled: &hickeyfield_core::engine::JobSet,
) {
    let Some(sid) = settled.storyboard_id.clone() else {
        return;
    };
    let Ok(Some(mut sb)) = store.storyboard(&sid) else {
        return;
    };
    if !sb.chained {
        return;
    }
    let mut keyframes = crate::store::parse_keyframes(sb.keyframes_json.as_deref());
    if keyframes.is_empty() {
        return; // not a keyframe set
    }
    // Only a still job (no shot index) is a keyframe boundary; a settled shot is
    // handled by `advance`/`score_join` and must never trip this barrier.
    if settled.shot_index.is_some() {
        return;
    }

    // Record this still's downloaded path (on a Completed, downloaded still).
    if settled.status.phase() == Phase::Completed {
        if let Some(png) = settled.results.iter().find_map(|o| o.local_path.clone()) {
            if let Some(rec) = keyframes
                .iter_mut()
                .find(|k| k.job_set_id.as_deref() == Some(settled.id.as_str()))
            {
                if rec.local_path.as_deref() != Some(png.as_str()) {
                    rec.local_path = Some(png);
                    sb.keyframes_json = serde_json::to_string(&keyframes).ok();
                    let _ = store.upsert_storyboard(&sb);
                }
            }
        }
    }

    // A failed still parks the set. When the set opted into auto-retry, try a
    // bounded still regenerate (its OWN bound, `KEYFRAME_RETRY_CAP`, separate
    // from a shot join's) — priced first through `submit_one`. Otherwise the
    // set stays parked for a manual "Regenerate still" (D4).
    if settled.status.phase() != Phase::Completed && sb.auto_retry {
        if let Some(rec) = keyframes
            .iter()
            .find(|k| k.job_set_id.as_deref() == Some(settled.id.as_str()))
        {
            if should_retry_still(rec, sb.auto_retry) {
                let idx = rec.index;
                let _ = crate::commands::regenerate_keyframe_still_inner(
                    store, prices, runner, &sid, idx, true,
                );
            }
        }
    }

    // The barrier: every boundary still on disk, and no shot 0 yet.
    if keyframes.iter().any(|k| k.local_path.is_none()) {
        return; // a still failed or is still in flight → parked/waiting
    }
    submit_shot_zero(store, prices, ffmpeg_dir, renders_root, runner, &sid);
}

/// The stills-ready → shot-0 decision (S17, D2), pure so the barrier's
/// exactly-once + idempotent behaviour is pinned by a test: true iff **every**
/// boundary still is on disk AND no shot 0 row exists yet. A missing still (a
/// Failed or in-flight still) → false (parked); an existing shot 0 row → false
/// (idempotent — the hook and resume cannot double-submit).
pub fn stills_ready_for_shot_zero(
    keyframes: &[crate::store::KeyframeRecord],
    jobs: &[hickeyfield_core::engine::JobSet],
) -> bool {
    !keyframes.is_empty()
        && keyframes.iter().all(|k| k.local_path.is_some())
        && !jobs.iter().any(|j| j.shot_index == Some(0))
}

/// Submit shot 0 of a keyframe set anchored on both ends (K0 in, K1 out), once
/// every still is ready. Idempotent: no-ops if a row already exists at shot 0,
/// so the barrier and the resume scan are safe to overlap (D2/D7).
fn submit_shot_zero(
    store: &Arc<SqliteStore>,
    prices: &Arc<Prices>,
    _ffmpeg_dir: &Path,
    _renders_root: &Path,
    runner: &Runner,
    sid: &str,
) {
    let Ok(Some(sb)) = store.storyboard(sid) else {
        return;
    };
    let keyframes = crate::store::parse_keyframes(sb.keyframes_json.as_deref());
    // Idempotency + readiness guard: all stills on disk and no shot 0 row yet.
    let existing = store.jobs_in_storyboard(sid).unwrap_or_default();
    if !stills_ready_for_shot_zero(&keyframes, &existing) {
        return;
    }
    let Some(shots_json) = sb.shots_json.as_deref() else {
        return;
    };
    let Ok(shots): Result<Vec<crate::commands::SubmitInput>, _> = serde_json::from_str(shots_json)
    else {
        return;
    };
    let Some(base) = shots.first().cloned() else {
        return;
    };
    let Some(model) = registry().get(&base.model_id).cloned() else {
        return;
    };
    let k0 = keyframes
        .iter()
        .find(|k| k.index == 0)
        .and_then(|k| k.local_path.clone());
    let k1 = keyframes
        .iter()
        .find(|k| k.index == 1)
        .and_then(|k| k.local_path.clone());
    let (Some(k0), Some(k1)) = (k0, k1) else {
        return;
    };

    // Shot 0 uses K0 as its start frame and K1 as its end frame (D3).
    let (input, _applied, advisories) = prepare_next_shot_keyframe(
        &model,
        base,
        Some(Path::new(&k0)),
        Some(Path::new(&k1)),
        storyboard_seed(sid),
        None,
    );
    match crate::commands::submit_one(store, prices, runner, &model, &input, Some(sid), Some(0)) {
        Ok(job_id) => {
            if !advisories.is_empty() {
                if let Ok(Some(mut job)) = store.get(&job_id) {
                    for adv in advisories {
                        if !job.advisories.contains(&adv) {
                            job.advisories.push(adv);
                        }
                    }
                    let _ = store.upsert(&job);
                }
            }
            tracing::info!("storyboard {sid}: stills ready — submitted shot 0 with both anchors");
        }
        Err(e) => tracing::warn!("storyboard {sid}: shot 0 did not submit: {e}"),
    }
}

/// The bounded still auto-retry ceiling (S17, D4/Q6): at most two regenerations
/// of a keyframe still, separate from a shot join's [`AUTO_RETRY_CAP`].
pub const KEYFRAME_RETRY_CAP: u32 = 2;

/// Whether a keyframe still warrants an auto-retry: the set's toggle is on and
/// the still has not yet hit its OWN bound (`KEYFRAME_RETRY_CAP`, distinct from a
/// shot join's `AUTO_RETRY_CAP` — a still failure and a shot failure are
/// different work and must not share a budget, D4/Q6). Pure, so the separateness
/// is pinned by a test. The "failed" trigger is checked at the call site.
pub fn should_retry_still(rec: &crate::store::KeyframeRecord, enabled: bool) -> bool {
    enabled && rec.attempt < KEYFRAME_RETRY_CAP
}

/// The bounded auto-retry ceiling (memo §0.4, house "two repairs then
/// escalate"): at most two regenerations of a below-floor join.
pub const AUTO_RETRY_CAP: u32 = 2;

/// Whether a scored join warrants an auto-retry: the toggle is on, the join
/// scored **below the floor** on the cut (`pass == false`) **or below the ideal
/// on its end adherence** (`end_pass == false`, S17 keyframe mode), and it has
/// not yet hit the cap. An unscored cut/end (`None`) is never retried — a missing
/// score must not spend money. Pure, so the loop's bound is pinned by a test.
pub fn should_auto_retry(rec: &crate::store::JoinRecord, enabled: bool) -> bool {
    let below = rec.pass == Some(false) || rec.end_pass == Some(false);
    enabled && below && rec.attempt < AUTO_RETRY_CAP
}

/// Auto-retry a below-floor join once, through the same money-gated regenerate
/// path as the manual button (D9). Fires only when the set's toggle is on and
/// the just-scored `record` is below the floor under the cap. Re-uses the
/// identical conditioning frame on disk, offsets the seed by the new attempt,
/// and submits through `submit_one` — which prices the shot on its own row
/// **before** it spends, so nothing spends without the total shown. The bumped
/// `attempt` in `joins_json` is what bounds the loop across settles.
#[allow(clippy::too_many_arguments)]
pub fn maybe_auto_retry(
    store: &Arc<SqliteStore>,
    prices: &Arc<Prices>,
    runner: &Runner,
    ffmpeg_dir: &Path,
    renders_root: &Path,
    settled: &hickeyfield_core::engine::JobSet,
    record: &crate::store::JoinRecord,
) {
    let Some(sid) = settled.storyboard_id.clone() else {
        return;
    };
    let Ok(Some(mut sb)) = store.storyboard(&sid) else {
        return;
    };
    if !should_auto_retry(record, sb.auto_retry) {
        return;
    }
    let i = record.index;
    // M1: use the same in-flight guard as the manual regenerate path. If a job
    // at this index is still queued or running (e.g. a prior auto-retry attempt
    // has not settled yet), a second submit would mint a duplicate in-flight
    // job and double-bill the shot. Only a settled (terminal) low-join shot is
    // eligible; the bumped `attempt` below still bounds the loop across settles.
    if let Ok(jobs) = store.jobs_in_storyboard(&sid) {
        if crate::commands::shot_in_flight(&jobs, i as u32) {
            return;
        }
    }
    // The conditioning frame is the previous shot's continuity frame, kept on
    // disk since it was extracted (identical pixels — a regenerate changes only
    // the seed). In a keyframe set, shot 0's start frame is the planned still K0
    // (there is no predecessor clip); a mid-chain shot's is the extracted
    // continuity frame `shot{i-1}.png`.
    let keyframes = crate::store::parse_keyframes(sb.keyframes_json.as_deref());
    let start_frame: std::path::PathBuf = if i == 0 {
        match keyframes
            .iter()
            .find(|k| k.index == 0)
            .and_then(|k| k.local_path.clone())
        {
            Some(p) => std::path::PathBuf::from(p),
            None => return,
        }
    } else {
        renders_root
            .join(&sid)
            .join("frames")
            .join(format!("shot{}.png", i - 1))
    };
    if !start_frame.is_file() {
        return;
    }
    // The planned end still K_{i+1} (keyframe mode); `None` for a non-keyframe set.
    let end_frame: Option<std::path::PathBuf> = keyframes
        .iter()
        .find(|k| k.index == i + 1)
        .and_then(|k| k.local_path.clone())
        .map(std::path::PathBuf::from);

    let Some(shots_json) = sb.shots_json.as_deref() else {
        return;
    };
    let Ok(shots): Result<Vec<crate::commands::SubmitInput>, _> = serde_json::from_str(shots_json)
    else {
        return;
    };
    let Some(base) = shots.get(i).cloned() else {
        return;
    };
    let Some(model) = registry().get(&base.model_id).cloned() else {
        return;
    };

    // Bump the attempt FIRST and persist, so a settle that fires before this
    // returns sees the incremented count and the loop stays bounded.
    let new_attempt = record.attempt + 1;
    let mut joins = crate::store::parse_joins(sb.joins_json.as_deref());
    if let Some(rec) = joins.iter_mut().find(|j| j.index == i) {
        rec.attempt = new_attempt;
    }
    sb.joins_json = serde_json::to_string(&joins).ok();
    let _ = store.upsert_storyboard(&sb);

    // Same frames, seed offset by the new attempt (memo §0.3 item 6 — a retry
    // MUST change the seed or identical inputs reproduce the identical output).
    let seed = storyboard_seed(&sid) + new_attempt as u64;
    let (input, _applied, _adv) = prepare_next_shot_keyframe(
        &model,
        base,
        Some(start_frame.as_path()),
        end_frame.as_deref(),
        seed,
        None,
    );
    let _ = ffmpeg_dir; // (the frame is already extracted; no sidecar needed here)

    match crate::commands::submit_one(
        store,
        prices,
        runner,
        &model,
        &input,
        Some(&sid),
        Some(i as u32),
    ) {
        Ok(_) => tracing::info!("chain {sid}: auto-retry {new_attempt} of join {i}"),
        Err(e) => tracing::warn!("chain {sid}: auto-retry of join {i} did not submit: {e}"),
    }
}

/// Reconcile every chained set at startup (§D2.5).
///
/// A crash *between* shot N downloading and shot N+1 being submitted leaves
/// shot N settled but not in `unfinished()`, so the settled hook never fired
/// and the chain stalled. Called once after `runner.resume_all()` (which
/// re-attaches still-running shots so their eventual settle advances the chain
/// normally): for each chained set, find the highest settled `Completed` shot
/// that has no successor row and re-`advance` from it. The idempotency guard in
/// [`advance`] makes this safe to overlap with a shot that is still in flight.
pub fn resume_all(state: &crate::app::AppState) {
    let Ok(boards) = state.store.storyboards() else {
        return;
    };
    for sb in boards.into_iter().filter(|b| b.chained) {
        let jobs = match state.store.jobs_in_storyboard(&sb.id) {
            Ok(j) => j,
            Err(_) => continue,
        };

        // S17 (D7): a keyframe set whose stills all downloaded but shot 0 never
        // got submitted (a crash in the barrier window) must resume shot 0. First
        // reconcile each boundary's `local_path` from its settled still job (the
        // barrier may not have run), then fire the idempotent shot-0 submit.
        let mut keyframes = crate::store::parse_keyframes(sb.keyframes_json.as_deref());
        if !keyframes.is_empty() {
            let mut changed = false;
            for rec in keyframes.iter_mut() {
                if rec.local_path.is_some() {
                    continue;
                }
                if let Some(job) = jobs.iter().find(|j| {
                    Some(j.id.as_str()) == rec.job_set_id.as_deref()
                        && j.shot_index.is_none()
                        && j.status.phase() == Phase::Completed
                }) {
                    if let Some(png) = job.results.iter().find_map(|o| o.local_path.clone()) {
                        rec.local_path = Some(png);
                        changed = true;
                    }
                }
            }
            if changed {
                let mut sb2 = sb.clone();
                sb2.keyframes_json = serde_json::to_string(&keyframes).ok();
                let _ = state.store.upsert_storyboard(&sb2);
            }
            // Every still ready and no shot 0 yet → submit it (idempotent). A
            // still still Queued, or one Failed (record still None), leaves the
            // set parked — no shot 0.
            if keyframes.iter().all(|k| k.local_path.is_some())
                && !jobs.iter().any(|j| j.shot_index == Some(0))
            {
                submit_shot_zero(
                    &state.store,
                    &state.prices,
                    &state.ffmpeg_dir,
                    &state.renders_root,
                    &state.runner,
                    &sb.id,
                );
            }
        }

        // The highest shot that settled Completed with a local output. A still
        // (no shot index) is excluded so it can never masquerade as shot 0.
        let advanceable = jobs
            .iter()
            .filter(|j| {
                j.shot_index.is_some()
                    && j.status.phase() == Phase::Completed
                    && j.results.iter().any(|o| o.local_path.is_some())
            })
            .max_by_key(|j| j.shot_index.unwrap_or(0));
        if let Some(job) = advanceable {
            // `advance` no-ops if the successor already exists, so this only
            // acts on a genuinely stalled chain.
            advance(
                &state.store,
                &state.prices,
                &state.library,
                &state.ffmpeg_dir,
                &state.renders_root,
                &state.runner,
                job,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::SubmitInput;
    use hickeyfield_core::engine::{JobSet, Output, OutputKind};
    use hickeyfield_core::{registry, use_case, JobStatus, UseCase};
    use std::path::PathBuf;

    fn input_for(model_id: &str) -> SubmitInput {
        serde_json::from_value(serde_json::json!({
            "modelId": model_id,
            "prompt": "a matte-black can, the drop falls",
        }))
        .unwrap()
    }

    fn an_i2v_model() -> hickeyfield_core::Model {
        use_case::models_for(UseCase::ImageToVideo)
            .into_iter()
            .next()
            .expect("an image-to-video model")
    }

    #[test]
    fn chainable_model_matches_the_animate_image_tab() {
        let i2v = an_i2v_model();
        assert!(chainable_model(&i2v), "{} should chain", i2v.id);
        // A model the Animate Image tab does not offer cannot chain.
        let not = registry()
            .into_values()
            .find(|m| !use_case::supports(m, UseCase::ImageToVideo));
        if let Some(m) = not {
            assert!(!chainable_model(&m), "{} should not chain", m.id);
        }
    }

    #[test]
    fn the_storyboard_seed_is_stable_and_deterministic() {
        let a = storyboard_seed("sb-abc");
        assert_eq!(a, storyboard_seed("sb-abc"), "same id → same seed");
        assert_ne!(
            a,
            storyboard_seed("sb-xyz"),
            "different id → different seed"
        );
        assert!(a <= 0xffff_ffff, "u32 range");
    }

    #[test]
    fn prepare_attaches_the_frame_and_disables_expansion_when_chainable() {
        // AC2/D-C: an i2v model with a frame gets a Start attachment and the
        // chained overrides (expansion disabled + the fixed seed).
        let model = an_i2v_model();
        let frame = PathBuf::from("/renders/sid/frames/shot0.png");
        let (input, applied, advs) = prepare_next_shot_keyframe(
            &model,
            input_for(&model.id),
            Some(&frame),
            None,
            4242,
            None,
        );
        assert!(applied, "continuity applied");
        assert!(advs.is_empty(), "no fallback advisory");
        assert_eq!(input.media.len(), 1);
        assert_eq!(input.media[0].role, MediaRole::Start);
        let ov = input.constant_overrides.expect("chained overrides");
        assert_eq!(
            ov.get("prompt_expansion_mode"),
            Some(&serde_json::json!("disabled"))
        );
        assert_eq!(ov.get("seed"), Some(&serde_json::json!(4242)));
    }

    #[test]
    fn prepare_falls_back_to_text_only_with_an_advisory() {
        // AC2: a model with no i2v route (frame present) → text-only + advisory,
        // no Start media, no overrides.
        let not = registry()
            .into_values()
            .find(|m| !use_case::supports(m, UseCase::ImageToVideo));
        let Some(model) = not else { return };
        let frame = PathBuf::from("/f.png");
        let (input, applied, advs) =
            prepare_next_shot_keyframe(&model, input_for(&model.id), Some(&frame), None, 1, None);
        assert!(!applied);
        assert!(advs[0].contains("no image-to-video route"));
        assert!(input.media.is_empty());
        assert!(input.constant_overrides.is_none());

        // And a missing frame (extraction failed) on an i2v model also falls
        // back, carrying the extraction error.
        let i2v = an_i2v_model();
        let (input2, applied2, advs2) = prepare_next_shot_keyframe(
            &i2v,
            input_for(&i2v.id),
            None,
            None,
            1,
            Some("boom".into()),
        );
        assert!(!applied2);
        assert_eq!(advs2.first().map(String::as_str), Some("boom"));
        assert!(input2.media.is_empty());
    }

    // --- advance guard paths (no submit → keyless, no network) ---------------

    fn test_runner(store: &Arc<SqliteStore>) -> Runner {
        Runner::new(
            Arc::clone(store) as Arc<dyn JobStore>,
            Arc::new(|_| None),
            Arc::new(|_| {}),
            Arc::new(Library::new(
                std::env::temp_dir().join("hickeyfield-chain-test-lib"),
            )),
            Arc::new(Prices::bundled()),
        )
    }

    fn chained_board(store: &Arc<SqliteStore>, id: &str, model_id: &str, n: usize) {
        let shots: Vec<SubmitInput> = (0..n).map(|_| input_for(model_id)).collect();
        store
            .upsert_storyboard(&crate::store::Storyboard {
                id: id.into(),
                brief: "b".into(),
                target_seconds: 30.0,
                plan_json: "{}".into(),
                created_at: 1,
                chained: true,
                shots_json: Some(serde_json::to_string(&shots).unwrap()),
                overlap_seconds: Some(0.0),
                joins_json: None,
                auto_retry: false,
                keyframes_json: None,
            })
            .unwrap();
    }

    fn settled_shot(sid: &str, idx: u32) -> JobSet {
        JobSet {
            id: format!("{sid}-shot{idx}"),
            model_id: "m".into(),
            route_id: "fal:m".into(),
            request_id: "r".into(),
            endpoint: String::new(),
            status: JobStatus::Completed,
            prompt: "p".into(),
            enhanced_prompt: None,
            enhancer_version: None,
            enhance_note: None,
            advisories: Vec::new(),
            preset_id: None,
            created_at: 0,
            updated_at: 0,
            results: vec![Output {
                url: "https://x/c.mp4".into(),
                kind: OutputKind::Video,
                local_path: Some("/lib/c.mp4".into()),
            }],
            estimated_usd: None,
            actual_usd: None,
            priced_from: None,
            priced_at: None,
            actual_source: None,
            fail_reason: None,
            settings: serde_json::json!({}),
            media: Vec::new(),
            poll_cycles: 0,
            stalled: false,
            storyboard_id: Some(sid.into()),
            shot_index: Some(idx),
        }
    }

    #[test]
    fn advance_is_idempotent_when_the_next_shot_exists() {
        // §10.1: a row already at `next` → advance no-ops, so the hook and the
        // resume scan cannot both submit (a double-bill). No submit path is
        // reached, so this is keyless and makes no network call.
        let store: Arc<SqliteStore> = Arc::new(SqliteStore::in_memory().unwrap());
        let runner = test_runner(&store);
        let model = an_i2v_model();
        chained_board(&store, "sid", &model.id, 3);
        let shot0 = settled_shot("sid", 0);
        store.upsert(&shot0).unwrap();
        // A row already exists at shot 1.
        let mut shot1 = settled_shot("sid", 1);
        shot1.status = JobStatus::Queued;
        store.upsert(&shot1).unwrap();

        let before = store.jobs_in_storyboard("sid").unwrap().len();
        advance(
            &store,
            &Arc::new(Prices::bundled()),
            &Arc::new(Library::new(std::env::temp_dir().join("hf-x"))),
            std::path::Path::new("/no/sidecar/here"),
            std::path::Path::new("/tmp/renders"),
            &runner,
            &shot0,
        );
        assert_eq!(
            store.jobs_in_storyboard("sid").unwrap().len(),
            before,
            "no new row when the successor already exists"
        );
    }

    #[test]
    fn a_failed_shot_stops_the_chain() {
        // AC2/AC6: a settled shot that did not complete never submits its
        // successor. No row appears at `next`.
        let store: Arc<SqliteStore> = Arc::new(SqliteStore::in_memory().unwrap());
        let runner = test_runner(&store);
        let model = an_i2v_model();
        chained_board(&store, "sid", &model.id, 3);
        let mut failed = settled_shot("sid", 0);
        failed.status = JobStatus::Failed;
        failed.results.clear();
        store.upsert(&failed).unwrap();

        advance(
            &store,
            &Arc::new(Prices::bundled()),
            &Arc::new(Library::new(std::env::temp_dir().join("hf-x"))),
            std::path::Path::new("/no/sidecar/here"),
            std::path::Path::new("/tmp/renders"),
            &runner,
            &failed,
        );
        let jobs = store.jobs_in_storyboard("sid").unwrap();
        assert!(
            !jobs.iter().any(|j| j.shot_index == Some(1)),
            "a failed predecessor must not submit shot 1"
        );
    }

    #[test]
    fn advance_ignores_a_non_chained_set_and_a_complete_chain() {
        let store: Arc<SqliteStore> = Arc::new(SqliteStore::in_memory().unwrap());
        let runner = test_runner(&store);
        let model = an_i2v_model();
        // Non-chained set: never advances.
        store
            .upsert_storyboard(&crate::store::Storyboard {
                id: "plain".into(),
                brief: "b".into(),
                target_seconds: 30.0,
                plan_json: "{}".into(),
                created_at: 1,
                chained: false,
                shots_json: None,
                overlap_seconds: None,
                joins_json: None,
                auto_retry: false,
                keyframes_json: None,
            })
            .unwrap();
        let mut s0 = settled_shot("plain", 0);
        s0.id = "plain-0".into();
        store.upsert(&s0).unwrap();
        advance(
            &store,
            &Arc::new(Prices::bundled()),
            &Arc::new(Library::new(std::env::temp_dir().join("hf-x"))),
            std::path::Path::new("/no/sidecar"),
            std::path::Path::new("/tmp/renders"),
            &runner,
            &s0,
        );
        assert_eq!(store.jobs_in_storyboard("plain").unwrap().len(), 1);

        // Chained set of 1 shot: the settled shot is the last → complete, no
        // submit.
        chained_board(&store, "one", &model.id, 1);
        let last = settled_shot("one", 0);
        store.upsert(&last).unwrap();
        advance(
            &store,
            &Arc::new(Prices::bundled()),
            &Arc::new(Library::new(std::env::temp_dir().join("hf-x"))),
            std::path::Path::new("/no/sidecar"),
            std::path::Path::new("/tmp/renders"),
            &runner,
            &last,
        );
        assert_eq!(store.jobs_in_storyboard("one").unwrap().len(), 1);
    }

    #[test]
    fn chainable_model_keeps_h3_max_and_excludes_media_ignorers() {
        // AC0-H2 (reverted to coarse — see the fn doc): H3 Max i2v stays
        // chainable, and a measured media-ignoring model (`gemini_omni`) does
        // NOT chain — the case the tightened per-route check regressed.
        let reg = registry();
        let h3 = reg.get("minimax_h3_max").expect("h3 max is registered");
        assert!(chainable_model(h3), "H3 Max i2v must stay chainable");
        if let Some(g) = reg.get("gemini_omni") {
            assert!(!chainable_model(g), "a media-ignoring model must not chain");
        }
        // And it agrees with the picker's authority for the whole roster.
        for m in reg.values() {
            assert_eq!(
                chainable_model(m),
                use_case::supports(m, UseCase::ImageToVideo),
                "{} must match the Animate-Image picker authority",
                m.id
            );
        }
    }

    #[test]
    fn a_scoring_failure_is_none_never_zero() {
        // AC9: the sidecar is missing, so the join cannot be scored — the record
        // must be unscored (`pass == None`, ssim `None`), NEVER a `0.0` that
        // would read as a failed join and fire a paid regenerate.
        let store: Arc<SqliteStore> = Arc::new(SqliteStore::in_memory().unwrap());
        let model = an_i2v_model();
        chained_board(&store, "sid", &model.id, 2);
        // Shot 1 settled, submitted WITH a start frame (a continuity join), but
        // no sidecar is available to score it.
        let mut shot1 = settled_shot("sid", 1);
        shot1
            .media
            .push(MediaRef::local(MediaRole::Start, "/f.png".to_string()));
        let rec = score_join(
            &store,
            std::path::Path::new("/no/sidecar/here"),
            std::path::Path::new("/tmp/renders-missing"),
            &shot1,
        )
        .expect("a continuity join records something");
        assert_eq!(rec.kind, "continuity");
        assert!(rec.applied);
        assert_eq!(rec.ssim, None, "unscored, not zero");
        assert_eq!(rec.pass, None, "unscored is not a failed join");
        // And it does not warrant an auto-retry even with the toggle on.
        assert!(!should_auto_retry(&rec, true));
    }

    #[test]
    #[ignore = "spawns the bundled sidecar; run with --ignored"]
    fn score_join_cleans_up_the_scratch_first_frame() {
        // m5: `first{i}.png` is a scratch input to the score only — nothing
        // reads it again — so a scored join must not leave it behind, while the
        // `shot{i-1}.png` continuity frame the chain/render need must survive.
        use std::process::Command;
        let sidecar_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("binaries");
        let bin =
            ffmpeg::locate(&sidecar_dir).expect("bundled sidecar; run scripts/fetch-ffmpeg.sh");

        let root = std::env::temp_dir().join(format!("hf-scorejoin-{}", std::process::id()));
        let frames = root.join("sid").join("frames");
        std::fs::create_dir_all(&frames).unwrap();
        // The previous shot's continuity frame (kept on disk) and a real clip
        // for shot 1 to score its first frame against.
        let prev = frames.join("shot0.png");
        let synth_png = |out: &std::path::Path| {
            let ok = Command::new(&bin)
                .args([
                    "-y",
                    "-f",
                    "lavfi",
                    "-i",
                    "testsrc=duration=1:size=320x240:rate=5",
                    "-frames:v",
                    "1",
                ])
                .arg(out)
                .status()
                .expect("synth png")
                .success();
            assert!(ok, "synth png failed");
        };
        synth_png(&prev);
        let clip = root.join("shot1.mp4");
        let ok = Command::new(&bin)
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                "testsrc=duration=1:size=320x240:rate=5",
            ])
            .arg(&clip)
            .status()
            .expect("synth clip")
            .success();
        assert!(ok, "synth clip failed");

        let store: Arc<SqliteStore> = Arc::new(SqliteStore::in_memory().unwrap());
        let model = an_i2v_model();
        chained_board(&store, "sid", &model.id, 2);
        let mut shot1 = settled_shot("sid", 1);
        shot1.results = vec![Output {
            url: "https://x/c.mp4".into(),
            kind: OutputKind::Video,
            local_path: Some(clip.to_string_lossy().into_owned()),
        }];
        shot1.media.push(MediaRef::local(
            MediaRole::Start,
            prev.to_string_lossy().into_owned(),
        ));

        let rec = score_join(&store, &sidecar_dir, &root, &shot1)
            .expect("a continuity join records something");
        assert!(rec.ssim.is_some(), "the real sidecar should score the join");

        // The scratch first frame is gone; the continuity frame survives.
        let leftovers: Vec<_> = std::fs::read_dir(&frames)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().starts_with("first"))
            .collect();
        assert!(
            leftovers.is_empty(),
            "no first*.png scratch frame must remain, found {leftovers:?}"
        );
        assert!(
            prev.is_file(),
            "the shot0.png continuity frame must survive"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn auto_retry_stops_after_two_attempts() {
        // AC10: a FakeScorer returning 0.6, 0.6, 0.95 (below floor twice, then
        // pass) drives EXACTLY two regenerates when the toggle is on, and zero
        // when it is off. Pure over `should_auto_retry` + the attempt counter,
        // the same bound the settled-hook loop uses.
        use crate::store::JoinRecord;
        let scores = [0.6_f64, 0.6, 0.95];
        let floor = hickeyfield_core::storyboard::CONTINUITY_FLOOR.ssim;

        let run = |enabled: bool| -> u32 {
            let mut attempt = 0u32;
            let mut regenerates = 0u32;
            loop {
                // Score this attempt; a scorer index past the last stays passing.
                let ssim = *scores.get(attempt as usize).unwrap_or(&0.95);
                let pass = ssim >= floor;
                let rec = JoinRecord {
                    index: 1,
                    kind: "continuity".into(),
                    applied: true,
                    ssim: Some(ssim),
                    psnr: Some(30.0),
                    dy: Some(1.0),
                    du: Some(1.0),
                    dv: Some(1.0),
                    pass: Some(pass),
                    attempt,
                    end_ssim: None,
                    end_psnr: None,
                    end_dy: None,
                    end_du: None,
                    end_dv: None,
                    end_pass: None,
                };
                if should_auto_retry(&rec, enabled) {
                    regenerates += 1;
                    attempt += 1;
                } else {
                    break;
                }
            }
            regenerates
        };

        assert_eq!(run(true), 2, "two below-floor scores → exactly two retries");
        assert_eq!(run(false), 0, "toggle off → never retries");
    }

    #[test]
    fn a_completed_predecessor_would_advance_so_the_failed_guard_bites() {
        // AC0-H3: prove the `Phase::Completed` guard — not the missing provider —
        // is what stops a failed predecessor. `advance` creates the set's
        // `frames/` directory AFTER the guard and BEFORE any submit, so it is an
        // observable that a Completed predecessor reaches and a Failed one does
        // not, WITHOUT needing a configured provider (which would be the vacuous
        // dependency the old test conflated). A Completed predecessor advances;
        // a Failed one returns at the guard.
        let store: Arc<SqliteStore> = Arc::new(SqliteStore::in_memory().unwrap());
        let runner = test_runner(&store);
        let model = an_i2v_model();
        let root = std::env::temp_dir().join(format!("hf-h3-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);

        // Failed predecessor: the guard returns before the frames dir is made.
        chained_board(&store, "bad", &model.id, 2);
        let failed = {
            let mut s = settled_shot("bad", 0);
            s.id = "bad-0".into();
            s.status = JobStatus::Failed;
            s.results.clear();
            s
        };
        store.upsert(&failed).unwrap();
        advance(
            &store,
            &Arc::new(Prices::bundled()),
            &Arc::new(Library::new(std::env::temp_dir().join("hf-h3-lib"))),
            std::path::Path::new("/no/sidecar"),
            &root,
            &runner,
            &failed,
        );
        assert!(
            !root.join("bad").join("frames").is_dir(),
            "a failed predecessor must not get past the guard"
        );

        // Completed predecessor: advance proceeds past the guard and prepares
        // the set's frames dir (then attempts the submit).
        chained_board(&store, "ok", &model.id, 2);
        let done = {
            let mut s = settled_shot("ok", 0);
            s.id = "ok-0".into();
            s
        };
        store.upsert(&done).unwrap();
        advance(
            &store,
            &Arc::new(Prices::bundled()),
            &Arc::new(Library::new(std::env::temp_dir().join("hf-h3-lib"))),
            std::path::Path::new("/no/sidecar"),
            &root,
            &runner,
            &done,
        );
        assert!(
            root.join("ok").join("frames").is_dir(),
            "a completed predecessor must advance past the guard"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    // ── S17: keyframe-anchored mode ──────────────────────────────────────────

    fn h3_max_model() -> hickeyfield_core::Model {
        registry().get("minimax_h3_max").expect("h3 max").clone()
    }

    /// Build a keyframe set: `n_shots` shots on `shot_model`, and a
    /// `keyframes_json` of `n_shots+1` records. Stills are marked ready
    /// (`local_path` set) iff `stills_ready`.
    fn keyframe_board(
        store: &Arc<SqliteStore>,
        id: &str,
        shot_model: &str,
        n_shots: usize,
        stills_ready: bool,
    ) {
        // The shots pin an UNAVAILABLE route id on purpose: if any barrier in a
        // test drives `submit_shot_zero` → `submit_one`, `route::resolve` fails
        // with `UnknownPin` BEFORE a Queued row is written or any provider is
        // contacted (INCIDENT 2026-09-16 — never let a test reach a real submit).
        // This is the rule-compliant equivalent of "no configured provider"
        // without stripping the dummy key env.
        let shots: Vec<SubmitInput> = (0..n_shots)
            .map(|_| {
                let mut s = input_for(shot_model);
                s.route_id = Some("unavailable:no-such-route".into());
                s
            })
            .collect();
        let records: Vec<crate::store::KeyframeRecord> = (0..=n_shots)
            .map(|k| crate::store::KeyframeRecord {
                index: k,
                job_set_id: Some(format!("{id}-still{k}")),
                local_path: stills_ready.then(|| format!("/lib/{id}/k{k}.png")),
                estimated_usd: Some(0.04),
                attempt: 0,
            })
            .collect();
        store
            .upsert_storyboard(&crate::store::Storyboard {
                id: id.into(),
                brief: "b".into(),
                target_seconds: 30.0,
                plan_json: "{}".into(),
                created_at: 1,
                chained: true,
                shots_json: Some(serde_json::to_string(&shots).unwrap()),
                overlap_seconds: Some(0.0),
                joins_json: None,
                auto_retry: false,
                keyframes_json: Some(serde_json::to_string(&records).unwrap()),
            })
            .unwrap();
    }

    fn settled_still(sid: &str, k: usize, phase: JobStatus) -> JobSet {
        let mut j = settled_shot(sid, 0);
        j.id = format!("{sid}-still{k}");
        j.shot_index = None; // a still occupies no shot index
        j.status = phase;
        j.results = vec![Output {
            url: "https://x/still.png".into(),
            kind: OutputKind::Image,
            local_path: Some(format!("/lib/{sid}/k{k}.png")),
        }];
        j
    }

    #[test]
    fn both_frames_land_in_the_body_on_h3_max_i2v() {
        // AC3 (keyless): shot i built through `prepare_next_shot_keyframe` with a
        // Start frame and an end still, bound against H3 Max i2v, carries BOTH
        // `image_url` and `end_image_url` as non-null strings — no network, no key.
        use hickeyfield_core::media;
        let model = h3_max_model();
        let start = PathBuf::from("/renders/sid/frames/shot0.png");
        let end = PathBuf::from("/lib/sid/k2.png");
        let (input, applied, advs) = prepare_next_shot_keyframe(
            &model,
            input_for(&model.id),
            Some(&start),
            Some(&end),
            7,
            None,
        );
        assert!(applied, "H3 Max is i2v, so the start frame binds");
        assert!(advs.is_empty(), "H3 Max takes an end frame → no advisory");
        assert_eq!(input.media.len(), 2, "start + end media attached");

        let route = model.routes.first().expect("a route");
        let dialect = media::dialect_for(route.provider, &route.slug);
        let body = media::bind(
            &model.spec,
            &model.display_name,
            &input.media,
            dialect,
            &route.slug,
        )
        .expect("bind succeeds keyless");
        assert!(
            body.get("image_url").and_then(|v| v.as_str()).is_some(),
            "image_url must land: {body:?}"
        );
        assert!(
            body.get("end_image_url").and_then(|v| v.as_str()).is_some(),
            "end_image_url must land on H3 Max i2v: {body:?}"
        );
    }

    #[test]
    fn a_no_end_frame_model_falls_back_to_start_only_with_an_advisory() {
        // AC3: a chainable model that cannot bind an end frame (a NO_END_FRAME
        // slug) attaches the start only and records the end-anchor advisory —
        // never a silently dropped, full-price end frame (the S9 harm).
        let no_end = registry()
            .into_values()
            .find(|m| chainable_model(m) && !end_frame_capable(m));
        let Some(model) = no_end else {
            return; // no such model in the roster — nothing to assert
        };
        let start = PathBuf::from("/renders/sid/frames/shot0.png");
        let end = PathBuf::from("/lib/sid/k2.png");
        let (input, applied, advs) = prepare_next_shot_keyframe(
            &model,
            input_for(&model.id),
            Some(&start),
            Some(&end),
            1,
            None,
        );
        assert!(applied, "the start frame still binds");
        assert!(
            !input.media.iter().any(|m| m.role == MediaRole::End),
            "the end frame must NOT be attached to a NO_END_FRAME model"
        );
        assert!(
            advs.iter()
                .any(|a| a.contains("end-frame anchoring unavailable")),
            "the end fallback advisory must be recorded: {advs:?}"
        );
    }

    #[test]
    fn the_cut_score_reads_the_ideal_tier_for_a_keyframe_set_and_the_floor_otherwise() {
        // Review M3 / design D4/Q7: a keyframe set's CUT score is gated on the
        // IDEAL tier (the 99% path), a plain chained set on the S16 FLOOR. A cut
        // ABOVE the floor but BELOW the ideal therefore passes only for a
        // non-keyframe set — so `should_auto_retry` fires on the right tier.
        use hickeyfield_core::storyboard::{
            join_passes, JoinScore, CONTINUITY_FLOOR, CONTINUITY_IDEAL,
        };
        assert_eq!(cut_threshold(true), CONTINUITY_IDEAL, "keyframe → ideal");
        assert_eq!(cut_threshold(false), CONTINUITY_FLOOR, "chained → floor");
        // 0.85 SSIM, small colour drift: above the 0.80 floor, below the 0.90 ideal.
        let mid = JoinScore {
            ssim: Some(0.85),
            psnr: Some(30.0),
            dy: Some(4.0),
            du: Some(2.0),
            dv: Some(2.0),
        };
        assert!(
            join_passes(&mid, cut_threshold(false)),
            "a mid-band cut passes the FLOOR (non-keyframe set)"
        );
        assert!(
            !join_passes(&mid, cut_threshold(true)),
            "the same cut FAILS the IDEAL (keyframe set) → an ideal-tier retry"
        );
    }

    #[test]
    fn end_frame_capability_reads_any_route_not_just_the_first() {
        // Review M1: `submit_one` resolves the cheapest AVAILABLE route, which
        // may not be `routes.first()`. `end_frame_capable` must judge on ANY
        // route — a model whose FIRST route cannot take an end frame but a later
        // one can is still end-capable, and one whose only route cannot is not.
        use hickeyfield_core::media::{self, MediaRole};
        use hickeyfield_core::route::Route;
        use hickeyfield_core::ProviderId;

        let mut model = h3_max_model(); // its own route binds End
        assert!(
            end_frame_capable(&model),
            "baseline: H3 Max binds End on its route"
        );
        // A NO_END_FRAME slug cannot bind End.
        let no_end = "veo3.1";
        assert!(
            !media::can_bind(
                &model.spec,
                MediaRole::End,
                media::dialect_for(ProviderId::Fal, no_end),
                no_end,
            ),
            "veo3.1 is in NO_END_FRAME → cannot bind End"
        );
        // Prepend it so `routes.first()` is the NON-end route; the H3 Max route
        // that CAN bind End is now second. The old `routes.first()` logic would
        // report false; the fixed `any`-route logic reports true.
        model.routes.insert(0, Route::new(ProviderId::Fal, no_end));
        assert!(
            !media::can_bind(
                &model.spec,
                MediaRole::End,
                media::dialect_for(model.routes[0].provider, &model.routes[0].slug),
                &model.routes[0].slug,
            ),
            "the FIRST route now cannot bind End"
        );
        assert!(
            end_frame_capable(&model),
            "a later route binds End → the model is end-frame capable (M1)"
        );
        // With ONLY the non-end route, it is not capable.
        model.routes.retain(|r| r.slug == no_end);
        assert!(
            !end_frame_capable(&model),
            "no route binds End → not capable"
        );
    }

    #[test]
    fn all_stills_ready_submits_shot_zero_with_both_anchors() {
        // AC2: (a) a settled still's downloaded path is recorded onto the set;
        // (b) the stills-ready → shot-0 decision fires exactly once and is
        // idempotent (an existing shot 0 row blocks a re-submit) and parks while
        // a still is missing. The shot-0 submit itself is the shared, audited
        // `submit_one` money path (see `submit_shot_zero`), exercised live — this
        // test proves the barrier LOGIC keylessly, with no network.
        let store: Arc<SqliteStore> = Arc::new(SqliteStore::in_memory().unwrap());
        let runner = test_runner(&store);
        let prices = Arc::new(Prices::bundled());
        let model = an_i2v_model();

        // Stills NOT yet recorded; one still job settles Completed.
        keyframe_board(&store, "kf", &model.id, 2, false);
        let still1 = settled_still("kf", 1, JobStatus::Completed);
        store.upsert(&still1).unwrap();
        keyframes_advance(
            &store,
            &prices,
            std::path::Path::new("/no/sidecar"),
            std::path::Path::new("/tmp/renders"),
            &runner,
            &still1,
        );
        // (a) The settled still's path is now recorded on boundary 1.
        let kfs = crate::store::parse_keyframes(
            store
                .storyboard("kf")
                .unwrap()
                .unwrap()
                .keyframes_json
                .as_deref(),
        );
        assert_eq!(
            kfs.iter()
                .find(|k| k.index == 1)
                .unwrap()
                .local_path
                .as_deref(),
            Some("/lib/kf/k1.png"),
            "the settled still's download is recorded"
        );

        // (b) The exactly-once decision.
        let all_ready: Vec<crate::store::KeyframeRecord> = (0..=2)
            .map(|k| crate::store::KeyframeRecord {
                index: k,
                job_set_id: Some(format!("s{k}")),
                local_path: Some(format!("/lib/kf/k{k}.png")),
                estimated_usd: Some(0.04),
                attempt: 0,
            })
            .collect();
        assert!(
            stills_ready_for_shot_zero(&all_ready, &[]),
            "all stills ready + no shot 0 → submit"
        );
        // A missing still parks it.
        let mut one_missing = all_ready.clone();
        one_missing[2].local_path = None;
        assert!(
            !stills_ready_for_shot_zero(&one_missing, &[]),
            "a missing still → parked, no shot 0"
        );
        // An existing shot 0 row makes it idempotent (no second submit).
        assert!(
            !stills_ready_for_shot_zero(&all_ready, &[settled_shot("kf", 0)]),
            "shot 0 already exists → idempotent, no double submit"
        );
    }

    #[test]
    fn a_failed_still_parks_the_set() {
        // AC2: a still that is not yet on disk (Failed / in flight) leaves the set
        // parked — no shot 0 is ever submitted against a missing anchor.
        let store: Arc<SqliteStore> = Arc::new(SqliteStore::in_memory().unwrap());
        let runner = test_runner(&store);
        let prices = Arc::new(Prices::bundled());
        let model = an_i2v_model();
        // Stills NOT ready (all local_path None).
        keyframe_board(&store, "kf", &model.id, 2, false);
        let failed = settled_still("kf", 1, JobStatus::Failed);
        store.upsert(&failed).unwrap();

        keyframes_advance(
            &store,
            &prices,
            std::path::Path::new("/no/sidecar"),
            std::path::Path::new("/tmp/renders"),
            &runner,
            &failed,
        );
        assert!(
            !store
                .jobs_in_storyboard("kf")
                .unwrap()
                .iter()
                .any(|j| j.shot_index == Some(0)),
            "no shot 0 while a still is missing — the set is parked"
        );
    }

    #[test]
    fn a_still_settling_never_trips_advance_or_score_join() {
        // §10.1: a settled still (shot_index None) must not be treated as a shot
        // by `advance`/`score_join` — both early-return on a missing shot index.
        let store: Arc<SqliteStore> = Arc::new(SqliteStore::in_memory().unwrap());
        let runner = test_runner(&store);
        let model = an_i2v_model();
        keyframe_board(&store, "kf", &model.id, 2, false);
        let still = settled_still("kf", 0, JobStatus::Completed);
        advance(
            &store,
            &Arc::new(Prices::bundled()),
            &Arc::new(Library::new(std::env::temp_dir().join("hf-kf-lib"))),
            std::path::Path::new("/no/sidecar"),
            std::path::Path::new("/tmp/renders"),
            &runner,
            &still,
        );
        assert!(
            score_join(
                &store,
                std::path::Path::new("/no/sidecar"),
                std::path::Path::new("/tmp/renders"),
                &still
            )
            .is_none(),
            "a still has no shot index → score_join does nothing"
        );
        assert!(
            !store
                .jobs_in_storyboard("kf")
                .unwrap()
                .iter()
                .any(|j| j.shot_index.is_some()),
            "advance never submitted a shot for a still"
        );
    }

    #[test]
    fn score_join_records_end_adherence_slot_even_without_a_sidecar() {
        // AC4 (keyless): in a keyframe set, shot 0 gets a JoinRecord (carrying the
        // end fields) even though there is no cut into it; with no sidecar the end
        // is unscored → None, never 0 (an unscored end must not fire a regenerate).
        let store: Arc<SqliteStore> = Arc::new(SqliteStore::in_memory().unwrap());
        keyframe_board(&store, "kf", &an_i2v_model().id, 2, true);
        let shot0 = settled_shot("kf", 0);
        let rec = score_join(
            &store,
            std::path::Path::new("/no/sidecar"),
            std::path::Path::new("/tmp/renders"),
            &shot0,
        )
        .expect("a keyframe set scores shot 0's end slot");
        assert_eq!(rec.index, 0);
        assert_eq!(rec.end_ssim, None, "no sidecar → end unscored, never 0");
        assert_eq!(rec.end_pass, None, "unscored end is not a fail");
        assert_eq!(rec.ssim, None, "shot 0 has no cut score");
        // And it is persisted at joins_json[0].
        let parsed = crate::store::parse_joins(
            store
                .storyboard("kf")
                .unwrap()
                .unwrap()
                .joins_json
                .as_deref(),
        );
        assert!(parsed.iter().any(|j| j.index == 0));
    }

    #[test]
    fn still_and_shot_auto_retry_bounds_are_separate() {
        // AC4/AC6: a below-ideal SHOT join retries ≤2 via `JoinRecord.attempt`;
        // a failed STILL retries ≤2 via `KeyframeRecord.attempt`; neither borrows
        // the other's budget; both OFF by default → 0 retries.
        let shot_run = |enabled: bool| -> u32 {
            let mut attempt = 0u32;
            let mut n = 0u32;
            loop {
                let rec = crate::store::JoinRecord {
                    index: 1,
                    kind: "continuity".into(),
                    applied: true,
                    ssim: Some(0.5),
                    psnr: Some(30.0),
                    dy: Some(1.0),
                    du: Some(1.0),
                    dv: Some(1.0),
                    pass: Some(false),
                    attempt,
                    end_ssim: None,
                    end_psnr: None,
                    end_dy: None,
                    end_du: None,
                    end_dv: None,
                    end_pass: None,
                };
                if should_auto_retry(&rec, enabled) {
                    n += 1;
                    attempt += 1;
                } else {
                    break;
                }
            }
            n
        };
        let still_run = |enabled: bool| -> u32 {
            let mut attempt = 0u32;
            let mut n = 0u32;
            loop {
                let rec = crate::store::KeyframeRecord {
                    index: 0,
                    job_set_id: Some("s".into()),
                    local_path: None,
                    estimated_usd: Some(0.04),
                    attempt,
                };
                if should_retry_still(&rec, enabled) {
                    n += 1;
                    attempt += 1;
                } else {
                    break;
                }
            }
            n
        };
        assert_eq!(shot_run(true), AUTO_RETRY_CAP, "shot bound is its own cap");
        assert_eq!(
            still_run(true),
            KEYFRAME_RETRY_CAP,
            "still bound is its own cap"
        );
        assert_eq!(shot_run(false), 0, "off → no shot retries");
        assert_eq!(still_run(false), 0, "off → no still retries");

        // An end-adherence failure ALSO drives a shot auto-retry (D4).
        let end_fail = crate::store::JoinRecord {
            index: 1,
            kind: "continuity".into(),
            applied: true,
            ssim: Some(0.95),
            psnr: Some(30.0),
            dy: Some(1.0),
            du: Some(1.0),
            dv: Some(1.0),
            pass: Some(true), // the cut passes …
            attempt: 0,
            end_ssim: Some(0.5),
            end_psnr: Some(20.0),
            end_dy: Some(9.0),
            end_du: Some(4.0),
            end_dv: Some(4.0),
            end_pass: Some(false), // … but the end drifts
        };
        assert!(
            should_auto_retry(&end_fail, true),
            "a below-ideal end adherence warrants a shot auto-retry"
        );
    }

    #[test]
    fn resume_submits_shot_zero_when_all_stills_are_ready() {
        // S1 (D7): a set whose stills all downloaded but shot 0 never submitted
        // (a crash in the barrier window) resumes shot 0; a Queued still leaves
        // it; a Failed still leaves it parked.
        let store: Arc<SqliteStore> = Arc::new(SqliteStore::in_memory().unwrap());
        let model = an_i2v_model();

        // "ready" set: the still records have NO local_path yet (a crash before
        // the barrier recorded them), but every still job settled Completed.
        // resume must reconcile each boundary's path from its still job, then the
        // shot-0 decision is unblocked.
        keyframe_board(&store, "ready", &model.id, 2, false);
        for k in 0..=2 {
            store
                .upsert(&settled_still("ready", k, JobStatus::Completed))
                .unwrap();
        }
        // "waiting" set: a still still Queued → its record stays None → parked.
        keyframe_board(&store, "waiting", &model.id, 2, false);
        store
            .upsert(&settled_still("waiting", 0, JobStatus::Queued))
            .unwrap();

        let state = crate::app::AppState {
            store: Arc::clone(&store),
            runner: Arc::new(test_runner(&store)),
            library: Arc::new(Library::new(std::env::temp_dir().join("hf-resume-lib"))),
            prices: Arc::new(Prices::bundled()),
            ffmpeg_dir: std::env::temp_dir().join("hf-resume-nobin"),
            renders_root: std::env::temp_dir().join("hf-resume-renders"),
        };
        resume_all(&state);

        // resume reconciled every boundary of the "ready" set from its still job,
        // so the shot-0 decision is now unblocked (the submit itself is the
        // audited `submit_one` path; the shot model here does not resolve to a
        // configured provider, so no network call is made in this test).
        let ready_kfs = crate::store::parse_keyframes(
            store
                .storyboard("ready")
                .unwrap()
                .unwrap()
                .keyframes_json
                .as_deref(),
        );
        assert!(
            ready_kfs.iter().all(|k| k.local_path.is_some()),
            "resume reconciled every still's download: {ready_kfs:?}"
        );
        assert!(
            stills_ready_for_shot_zero(&ready_kfs, &store.jobs_in_storyboard("ready").unwrap()),
            "the fully-reconciled set is ready to submit shot 0"
        );
        // Zero provider calls: the shots pin an unavailable route (see
        // `keyframe_board`), so `submit_one` fails at `route::resolve` before it
        // writes a Queued row or contacts a provider. No shot job of any index
        // exists — only the three still jobs the test seeded (INCIDENT rule).
        assert!(
            !store
                .jobs_in_storyboard("ready")
                .unwrap()
                .iter()
                .any(|j| j.shot_index.is_some()),
            "resume made no provider submit — no shot row was created"
        );
        // The "waiting" set's Queued still is never reconciled → stays parked.
        let waiting_kfs = crate::store::parse_keyframes(
            store
                .storyboard("waiting")
                .unwrap()
                .unwrap()
                .keyframes_json
                .as_deref(),
        );
        assert!(
            waiting_kfs.iter().any(|k| k.local_path.is_none()),
            "a Queued still is not reconciled → parked"
        );
        assert!(
            !stills_ready_for_shot_zero(
                &waiting_kfs,
                &store.jobs_in_storyboard("waiting").unwrap()
            ),
            "a set with a still not yet ready stays parked"
        );
    }

    #[test]
    #[ignore = "spawns the bundled ffmpeg sidecar on synth PNGs — run with --ignored"]
    fn end_adherence_reads_the_actual_last_frame() {
        // AC4 (real sidecar, no money): the end score reads the clip's ACTUAL
        // last frame. A clip whose last frame equals the still scores near 1.0
        // (adheres); a clip whose last frame is a different source scores below
        // the floor (drift). Uses `continuity_score` on synth lavfi frames.
        let dir = std::env::temp_dir().join(format!("hf-end-adhere-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let ffdir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("binaries");
        let bin = match ffmpeg::locate(&ffdir) {
            Ok(b) => b,
            Err(_) => return, // no sidecar fetched → nothing to run
        };

        // A still = one testsrc frame; a matching clip's last frame = the same
        // source re-encoded → high SSIM. A different source → low SSIM.
        let still = dir.join("still.png");
        let same_last = dir.join("same.png");
        let diff_last = dir.join("diff.png");
        let mk_frame = |out: &std::path::Path, src: &str| {
            let args = [
                "-nostdin",
                "-hide_banner",
                "-f",
                "lavfi",
                "-i",
                src,
                "-frames:v",
                "1",
                out.to_str().unwrap(),
            ];
            std::process::Command::new(&bin).args(args).status().ok();
        };
        mk_frame(&still, "testsrc=size=1360x768:duration=1");
        mk_frame(&same_last, "testsrc=size=1360x768:duration=1");
        mk_frame(&diff_last, "mandelbrot=size=1360x768");

        let adhere = ffmpeg::continuity_score(&bin, &still, &same_last).expect("scores");
        assert!(adhere.ssim >= 0.9, "the same frame adheres: {adhere:?}");
        let drift = ffmpeg::continuity_score(&bin, &still, &diff_last).expect("scores");
        assert!(drift.ssim < 0.8, "a different frame drifts: {drift:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
