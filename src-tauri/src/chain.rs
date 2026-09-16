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
use hickeyfield_core::{ffmpeg, registry, use_case, MediaRef, MediaRole, Phase, UseCase};
use std::path::Path;
use std::sync::Arc;

/// Can this model continue from a start frame — i.e. does it have a route that
/// binds `MediaRole::Start` on an image-to-video endpoint? This is exactly the
/// per-model authority the picker uses for the Animate Image tab, so the chain
/// cannot attach a frame to a route that would ignore it (the S9 harm).
pub fn chainable_model(model: &hickeyfield_core::Model) -> bool {
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
}
