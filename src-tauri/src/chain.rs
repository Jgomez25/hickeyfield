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
fn prepare_next_shot(
    model: &hickeyfield_core::Model,
    mut input: crate::commands::SubmitInput,
    frame: Option<&Path>,
    seed: u64,
    frame_error: Option<String>,
) -> (crate::commands::SubmitInput, bool, Option<String>) {
    match frame {
        Some(f) if chainable_model(model) => {
            input.media.push(MediaRef::local(
                MediaRole::Start,
                f.to_string_lossy().into_owned(),
            ));
            input.constant_overrides = Some(chained_overrides(seed));
            (input, true, None)
        }
        Some(_) => {
            let adv = format!(
                "continuity unavailable: {} has no image-to-video route — generated from the prompt alone",
                model.display_name
            );
            (input, false, Some(adv))
        }
        None => {
            let adv = frame_error.unwrap_or_else(|| {
                "continuity frame could not be extracted — this shot was generated from the prompt alone".into()
            });
            (input, false, Some(adv))
        }
    }
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

    let (input, _continuity_applied, fallback_advisory) =
        prepare_next_shot(&model, input, frame, storyboard_seed(&sid), frame_error);

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
            // Record the continuity fallback as an advisory on the new shot, so
            // the UI can say the join was generated from the prompt.
            if let Some(adv) = fallback_advisory {
                if let Ok(Some(mut job)) = store.get(&job_id) {
                    if !job.advisories.contains(&adv) {
                        job.advisories.push(adv);
                        let _ = store.upsert(&job);
                    }
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
pub fn score_join(
    store: &Arc<SqliteStore>,
    ffmpeg_dir: &Path,
    renders_root: &Path,
    settled: &hickeyfield_core::engine::JobSet,
) -> Option<crate::store::JoinRecord> {
    use hickeyfield_core::storyboard::{join_passes, JoinScore, CONTINUITY_FLOOR};

    let sid = settled.storyboard_id.clone()?;
    let mut sb = store.storyboard(&sid).ok()??;
    if !sb.chained {
        return None;
    }
    if settled.status.phase() != Phase::Completed {
        return None;
    }
    let i = settled.shot_index? as usize;
    if i == 0 {
        return None; // no join into the first shot
    }
    let clip = settled.results.iter().find_map(|o| o.local_path.clone())?;

    // Continuity was applied for this join iff the shot was submitted with a
    // start frame (the render + score authority, D8). Absent → a crossfade join.
    let applied = settled.media.iter().any(|m| m.role == MediaRole::Start);
    let kind = if applied { "continuity" } else { "crossfade" };

    let prev_frame = renders_root
        .join(&sid)
        .join("frames")
        .join(format!("shot{}.png", i - 1));

    // Keep the existing attempt count for this index (the regenerate path is
    // what increments it); default 0 for a first score.
    let mut joins = crate::store::parse_joins(sb.joins_json.as_deref());
    let attempt = joins
        .iter()
        .find(|j| j.index == i)
        .map(|j| j.attempt)
        .unwrap_or(0);

    // Score only a continuity join whose conditioning frame is on disk; a
    // crossfade join or a missing frame records the kind with an unscored score.
    let score = if applied && prev_frame.is_file() {
        match ffmpeg::locate(ffmpeg_dir).ok() {
            Some(bin) => {
                let first = renders_root
                    .join(&sid)
                    .join("frames")
                    .join(format!("first{i}.png"));
                let scored = ffmpeg::first_frame(&bin, Path::new(&clip), &first, None)
                    .and_then(|_| ffmpeg::continuity_score(&bin, &prev_frame, &first));
                // The `first{i}.png` frame is a scratch input to the score only
                // — unlike `shot{i}.png` (the continuity frame the chain and the
                // render still need), nothing reads it again, so remove it once
                // scored so attempts of a long set do not leave disk cruft.
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
                        tracing::warn!("chain {sid}: join {i} scoring failed: {e}");
                        JoinScore::unscored()
                    }
                }
            }
            None => JoinScore::unscored(),
        }
    } else {
        JoinScore::unscored()
    };

    let pass = score
        .is_scored()
        .then(|| join_passes(&score, CONTINUITY_FLOOR));
    let record = crate::store::JoinRecord {
        index: i,
        kind: kind.to_string(),
        applied,
        ssim: score.ssim,
        psnr: score.psnr,
        dy: score.dy,
        du: score.du,
        dv: score.dv,
        pass,
        attempt,
    };
    crate::store::upsert_join(&mut joins, record.clone());
    sb.joins_json = serde_json::to_string(&joins).ok();
    if let Err(e) = store.upsert_storyboard(&sb) {
        tracing::warn!("chain {sid}: could not persist join {i} score: {e}");
    }
    Some(record)
}

/// The bounded auto-retry ceiling (memo §0.4, house "two repairs then
/// escalate"): at most two regenerations of a below-floor join.
pub const AUTO_RETRY_CAP: u32 = 2;

/// Whether a scored join warrants an auto-retry: the toggle is on, the join
/// scored **below the floor** (`pass == false`), and it has not yet hit the cap.
/// An unscored join (`pass == None`) is never retried — a missing score must not
/// spend money. Pure, so the loop's bound is pinned by a test.
pub fn should_auto_retry(rec: &crate::store::JoinRecord, enabled: bool) -> bool {
    enabled && rec.pass == Some(false) && rec.attempt < AUTO_RETRY_CAP
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
    // the seed).
    let frame = renders_root
        .join(&sid)
        .join("frames")
        .join(format!("shot{}.png", i - 1));
    if !frame.is_file() {
        return;
    }
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

    // Same frame, seed offset by the new attempt (memo §0.3 item 6 — a retry
    // MUST change the seed or identical inputs reproduce the identical output).
    let seed = storyboard_seed(&sid) + new_attempt as u64;
    let (input, _applied, _adv) =
        prepare_next_shot(&model, base, Some(frame.as_path()), seed, None);
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
        // The highest shot that settled Completed with a local output.
        let advanceable = jobs
            .iter()
            .filter(|j| {
                j.status.phase() == Phase::Completed
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
        let (input, applied, adv) =
            prepare_next_shot(&model, input_for(&model.id), Some(&frame), 4242, None);
        assert!(applied, "continuity applied");
        assert!(adv.is_none(), "no fallback advisory");
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
        let (input, applied, adv) =
            prepare_next_shot(&model, input_for(&model.id), Some(&frame), 1, None);
        assert!(!applied);
        assert!(adv.unwrap().contains("no image-to-video route"));
        assert!(input.media.is_empty());
        assert!(input.constant_overrides.is_none());

        // And a missing frame (extraction failed) on an i2v model also falls
        // back, carrying the extraction error.
        let i2v = an_i2v_model();
        let (input2, applied2, adv2) =
            prepare_next_shot(&i2v, input_for(&i2v.id), None, 1, Some("boom".into()));
        assert!(!applied2);
        assert_eq!(adv2.as_deref(), Some("boom"));
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
}
