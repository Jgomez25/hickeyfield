//! Storyboard planning: one brief → an editable, model-aware shot list.
//!
//! This module owns the *pure* half of the storyboard feature — the plan DTO,
//! a strict text→JSON validator, and a model-aware duration clamp. The impure
//! half (choosing a backend, reading a hosted key, submitting the shots) lives
//! in the shell's command layer; this crate stays network-free and database-
//! free so it is unit-testable with a fake chat backend and no key.
//!
//! ## Why a validator and not a JSON mode
//!
//! The enhancer backends have no structured-output mode (see
//! [`crate::enhancer`]): all three send a plain system+user chat. Rather than
//! grow tool-call plumbing on every backend, the storyboard system prompt asks
//! the model to emit **one JSON object**, and this module parses it through the
//! same [`crate::enhancer::clean_reply`] the rewriter already uses (it unwraps a
//! ```` ```json ```` fence and a quote pair) and then a strict serde parse. A
//! reply that does not parse is a typed [`PlanError`], never a half-plan sent on.
//!
//! ## What the validator guarantees
//!
//! A plan that survives [`validate`] can be handed to the composer without it
//! disagreeing: every `model_id` exists in the registry **and** is runnable for
//! the tab (the same [`crate::use_case::supports`] predicate the model picker
//! and the submit path use), every camera slug exists ([`crate::camera::get`],
//! which never guesses a nearest move), the shot count is 3..=8, and the
//! proposed durations sum to within ±10% of the target. Only after that does
//! [`clamp_shot`] narrow each shot's seconds toward a value the chosen model
//! actually offers — always advising, never silently substituting.

use crate::capability;
use crate::enhancer::{clean_reply, refusal_reason, Chat};
use crate::{camera, registry, use_case, Model, UseCase};
use serde::{Deserialize, Serialize};

/// A shot's place in the sequence. `start`/`end` bracket the story; everything
/// between is `middle`. Recorded so S14 (render/stitch) knows the ends without
/// re-deriving them.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum ShotRole {
    Start,
    Middle,
    End,
}

/// A recorded continuity intent, **not** an applied operation.
///
/// `last_frame_of` names the shot this one chains from. S14 reads it to extract
/// that shot's last frame and seed this one; S13 only records it. See the
/// module note on why applying it is out of scope.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Continuity {
    /// Index of the previous shot whose last frame leads into this one.
    pub last_frame_of: usize,
}

/// One shot in a storyboard.
///
/// Serde camelCase for the UI bridge. `continuity` and `advisories` carry
/// `#[serde(default)]` because the model never emits them — the planner fills
/// them after validation — and so a stored plan written before they existed
/// still deserialises.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Shot {
    /// A few words naming the beat, for the editor to scan.
    pub title: String,
    /// What to generate for this shot. Sent through the submit path per shot.
    pub prompt: String,
    /// The shot's length. Clamped toward the model's duration axis by
    /// [`clamp_shot`]; the sum across shots is gated to the target by [`validate`].
    pub seconds: f64,
    /// A [`crate::camera`] slug, or `None` for a locked-off frame.
    pub camera: Option<String>,
    /// A registry model id. Must exist and be runnable for the tab; enforced by
    /// [`validate`] with the same predicate the picker uses.
    pub model_id: String,
    /// Where this shot sits in the sequence.
    pub role: ShotRole,
    /// Recorded, not applied — see [`Continuity`]. Set by the planner for every
    /// shot after the first.
    #[serde(default)]
    pub continuity: Option<Continuity>,
    /// Clamp notes: a shot whose seconds were narrowed toward the model's axis
    /// carries the honest reason here rather than being silently changed.
    #[serde(default)]
    pub advisories: Vec<String>,
}

/// A whole storyboard plan.
///
/// The `brief`, `target_seconds` and `use_case` are authoritative from the
/// command inputs — the model only drafts the `shots` — so [`plan`] overwrites
/// them after parsing rather than trusting the model to echo them. They carry
/// `#[serde(default)]` so a reply that is just `{"shots":[…]}` still parses.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct StoryboardPlan {
    #[serde(default)]
    pub brief: String,
    #[serde(default)]
    pub target_seconds: f64,
    /// The tab slug the plan was validated against.
    #[serde(default)]
    pub use_case: String,
    pub shots: Vec<Shot>,
}

/// Why a plan was rejected. Each variant carries what the UI (or a test) needs
/// to say *which* rule fired and *where*, so a rejection is actionable rather
/// than a bare "invalid plan".
#[derive(Debug, Clone, PartialEq)]
pub enum PlanError {
    /// The backend was unavailable, or the provider returned an error.
    Backend(String),
    /// The reply was a refusal/apology, caught by the S6 detector before parse.
    Refused(&'static str),
    /// `clean_reply` or the serde parse failed. Carries a short cause.
    NotJson(String),
    /// The shot count was not in `3..=8`.
    ShotCount(usize),
    /// The proposed durations did not sum to within ±10% of the target.
    DurationSum { sum: f64, target: f64 },
    /// A shot named a model id that is not in the registry.
    UnknownModel { shot: usize, model_id: String },
    /// A shot named a model that cannot run for this tab.
    ModelNotRunnable {
        shot: usize,
        model_id: String,
        use_case: String,
    },
    /// A shot named a camera slug that does not exist.
    UnknownCamera { shot: usize, slug: String },
}

impl std::fmt::Display for PlanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlanError::Backend(why) => {
                write!(f, "could not draft a storyboard: {why}")
            }
            PlanError::Refused(why) => {
                write!(f, "the model declined to draft this storyboard ({why})")
            }
            PlanError::NotJson(why) => write!(
                f,
                "the model did not return a usable storyboard ({why}). Try drafting again."
            ),
            PlanError::ShotCount(n) => write!(
                f,
                "a storyboard needs 3 to 8 shots, but the draft had {n}. Try drafting again."
            ),
            PlanError::DurationSum { sum, target } => write!(
                f,
                "the drafted shots run {}s in total, which is too far from the {}s you asked for. Try drafting again.",
                trim_secs(*sum),
                trim_secs(*target)
            ),
            PlanError::UnknownModel { shot, model_id } => write!(
                f,
                "shot {} named a model that does not exist ({model_id}).",
                shot + 1
            ),
            PlanError::ModelNotRunnable {
                shot,
                model_id,
                use_case,
            } => write!(
                f,
                "shot {} named {model_id}, which cannot run under {use_case}.",
                shot + 1
            ),
            PlanError::UnknownCamera { shot, slug } => write!(
                f,
                "shot {} named a camera move that does not exist ({slug}).",
                shot + 1
            ),
        }
    }
}

/// The ±fraction of the target the drafted durations must sum within.
const SUM_TOLERANCE: f64 = 0.10;

/// Draft a storyboard for `brief` through `chat`, then validate and clamp it.
///
/// `chat` is any [`Chat`] backend — a real one in the command layer, a
/// [`Chat`]-implementing fake in tests, so this runs offline. `system` is the
/// storyboard corpus ([`crate::corpus::STORYBOARD_SYSTEM`]); the per-draft
/// context (target length, allowed model ids, allowed camera slugs) is built
/// into the user message here so the corpus stays short and the allowed lists
/// cannot go stale.
pub fn plan(
    chat: &dyn Chat,
    system: &str,
    brief: &str,
    target_seconds: f64,
    use_case: UseCase,
    model_prefs: &[String],
) -> Result<StoryboardPlan, PlanError> {
    let user = build_user_message(brief, target_seconds, use_case, model_prefs);
    let raw = chat
        .complete(system, &user)
        .map_err(|e| PlanError::Backend(e.to_string()))?;

    // 1. Refusal guard (S6), before parsing: a decline is not garbage JSON, and
    //    the detector recognises it model-agnostically.
    if let Some(why) = refusal_reason(&raw) {
        return Err(PlanError::Refused(why));
    }

    // 2. Parse. `clean_reply` unwraps a fence/quote pair; then a strict serde
    //    parse. The `brief`/`target`/`use_case` fields default here and are
    //    overwritten below with the authoritative command inputs.
    let cleaned = clean_reply(&raw).map_err(PlanError::NotJson)?;
    let mut parsed: StoryboardPlan =
        serde_json::from_str(&cleaned.prompt).map_err(|e| PlanError::NotJson(e.to_string()))?;
    parsed.brief = brief.to_string();
    parsed.target_seconds = target_seconds;
    parsed.use_case = use_case.slug().to_string();

    // 3–6. Structural validation on the model's proposed shots.
    validate(&parsed.shots, target_seconds, use_case)?;

    // Model-aware clamp + recorded continuity. Validation guaranteed every model
    // id resolves, so the lookups below cannot miss.
    let reg = registry();
    for (i, shot) in parsed.shots.iter_mut().enumerate() {
        if let Some(model) = reg.get(&shot.model_id) {
            clamp_shot(shot, i, model, use_case);
        }
        shot.continuity = if i == 0 {
            None
        } else {
            Some(Continuity {
                last_frame_of: i - 1,
            })
        };
    }

    Ok(parsed)
}

/// Structural checks, in the documented order. Pure: registry/use-case/camera
/// lookups only, no network. Reused by [`plan`] and exposed for tests.
pub fn validate(shots: &[Shot], target_seconds: f64, use_case: UseCase) -> Result<(), PlanError> {
    // 3. Shot count.
    if !(3..=8).contains(&shots.len()) {
        return Err(PlanError::ShotCount(shots.len()));
    }

    let reg = registry();
    for (i, shot) in shots.iter().enumerate() {
        // 4. Model exists and is runnable for the tab — the same predicate the
        //    picker and the submit path gate on, so a plan can never name a
        //    model the composer would refuse.
        let Some(model) = reg.get(&shot.model_id) else {
            return Err(PlanError::UnknownModel {
                shot: i,
                model_id: shot.model_id.clone(),
            });
        };
        if !use_case::supports(model, use_case) {
            return Err(PlanError::ModelNotRunnable {
                shot: i,
                model_id: shot.model_id.clone(),
                use_case: use_case.slug().to_string(),
            });
        }

        // 5. Camera slug exists, or is None. `camera::get` never guesses a
        //    nearest move — a wrong camera is worse than none.
        if let Some(slug) = shot.camera.as_deref() {
            if camera::get(slug).is_none() {
                return Err(PlanError::UnknownCamera {
                    shot: i,
                    slug: slug.to_string(),
                });
            }
        }
    }

    // 6. Duration sum within ±10% of the target, on the *proposed* seconds —
    //    the drafting quality gate, before the per-shot clamp adjusts anything.
    let sum: f64 = shots.iter().map(|s| s.seconds).sum();
    if (sum - target_seconds).abs() > SUM_TOLERANCE * target_seconds {
        return Err(PlanError::DurationSum {
            sum,
            target: target_seconds,
        });
    }

    Ok(())
}

/// Narrow one shot's seconds toward the chosen model's duration axis, reusing
/// the S9 capability data offline (catalogue-sourced — `|_| None` is the same
/// fallback `list_models` uses to avoid 68 HTTP requests).
///
/// - **Enumerated axis**: snap to the nearest offered value; if it changed, add
///   an advisory naming both. This only ever narrows *toward* an offered value.
/// - **Free-form axis** (e.g. Seedance 2.5 native 4–30s): leave as-is — the
///   provider validates the exact number. This is what lets a single-shot 30s
///   plan keep its 30s.
/// - **No duration control** (a still model): leave untouched, no advisory.
/// - **Unknown axis**: leave as-is — inventing a clamp from an unknown axis is
///   the S9 harm.
pub fn clamp_shot(shot: &mut Shot, shot_index: usize, model: &Model, use_case: UseCase) {
    let Some(route) = model.routes.first() else {
        return;
    };
    let cap = capability::for_route(
        &model.spec,
        route.provider,
        &route.slug,
        use_case.input_mode(),
        |_| None,
    );

    // A model with no duration control at all (a still) carries no seconds that
    // bill — leave it and say nothing.
    if cap.duration.support.is_no() {
        return;
    }

    // Enumerated axis: snap to the nearest offered value.
    if let Some(values) = cap.duration_seconds() {
        if !values.is_empty() {
            let asked = shot.seconds;
            let nearest = values
                .iter()
                .copied()
                .min_by(|a, b| {
                    (a - asked)
                        .abs()
                        .partial_cmp(&(b - asked).abs())
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .unwrap_or(asked);
            if (nearest - asked).abs() > f64::EPSILON {
                shot.advisories.push(format!(
                    "shot {}: {} runs {}s, not {}s — clamped",
                    shot_index + 1,
                    model.display_name,
                    trim_secs(nearest),
                    trim_secs(asked)
                ));
                shot.seconds = nearest;
            }
        }
    }
    // Free-form (`values` empty) or unknown: leave as-is.
}

/// The per-draft context, assembled into the user message so the corpus stays
/// short and the allowed lists are always current.
fn build_user_message(
    brief: &str,
    target_seconds: f64,
    use_case: UseCase,
    model_prefs: &[String],
) -> String {
    let cameras: Vec<&str> = camera::slugs().collect();
    let mut out = String::with_capacity(brief.len() + 512);
    out.push_str("Brief: ");
    out.push_str(brief.trim());
    out.push_str(&format!(
        "\nTarget length: {}s (the shot lengths should add up to about this)",
        trim_secs(target_seconds)
    ));
    out.push_str(&format!("\nOutput: {}", use_case.output()));
    out.push_str("\nAllowed model ids (choose modelId only from these): ");
    out.push_str(&model_prefs.join(", "));
    out.push_str("\nAllowed camera slugs (use one of these, or null): ");
    out.push_str(&cameras.join(", "));
    out.push_str("\n\nDraft the storyboard now. Reply with only the JSON object.");
    out
}

/// `5` not `5.0`, `5.5` kept — the same rendering the picker uses for durations.
fn trim_secs(v: f64) -> String {
    if v.fract() == 0.0 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::JobError;

    /// A [`Chat`] that returns one canned reply (or error), so plan tests run
    /// with no network. The system/user arguments are ignored — the point is to
    /// exercise the validator against a known reply.
    struct FakeChat(Result<String, JobError>);

    impl Chat for FakeChat {
        fn complete(&self, _system: &str, _user: &str) -> Result<String, JobError> {
            self.0.clone()
        }
    }

    fn fake(reply: &str) -> FakeChat {
        FakeChat(Ok(reply.to_string()))
    }

    /// A runnable text-to-video model id from the registry, and a real camera
    /// slug, so the fixtures cannot drift from the catalogue.
    fn a_t2v_model() -> String {
        use_case::models_for(UseCase::TextToVideo)
            .into_iter()
            .next()
            .expect("at least one text-to-video model")
            .id
    }

    fn a_camera() -> String {
        camera::slugs()
            .next()
            .expect("at least one camera")
            .to_string()
    }

    /// Four shots summing to 30s, all on `model`, cameras real.
    fn good_json(model: &str, cam: &str) -> String {
        format!(
            r#"```json
{{"shots": [
  {{"title": "Open", "prompt": "a cold-brew can on ice", "seconds": 8, "camera": "{cam}", "modelId": "{model}", "role": "start"}},
  {{"title": "Pour", "prompt": "dark coffee pours over ice", "seconds": 8, "camera": null, "modelId": "{model}", "role": "middle"}},
  {{"title": "Lift", "prompt": "a hand lifts the can", "seconds": 8, "camera": "{cam}", "modelId": "{model}", "role": "middle"}},
  {{"title": "Logo", "prompt": "the brand logo resolves", "seconds": 6, "camera": null, "modelId": "{model}", "role": "end"}}
]}}
```"#
        )
    }

    #[test]
    fn a_valid_plan_is_accepted() {
        let model = a_t2v_model();
        let cam = a_camera();
        let out = plan(
            &fake(&good_json(&model, &cam)),
            "sys",
            "a 30-second ad for a cold-brew can",
            30.0,
            UseCase::TextToVideo,
            std::slice::from_ref(&model),
        )
        .expect("a good plan is accepted");
        assert_eq!(out.shots.len(), 4);
        assert_eq!(out.brief, "a 30-second ad for a cold-brew can");
        assert_eq!(out.target_seconds, 30.0);
        assert_eq!(out.use_case, "text-to-video");
        // Continuity is recorded for every shot after the first.
        assert_eq!(out.shots[0].continuity, None);
        assert_eq!(
            out.shots[1].continuity,
            Some(Continuity { last_frame_of: 0 })
        );
        assert_eq!(
            out.shots[3].continuity,
            Some(Continuity { last_frame_of: 2 })
        );
    }

    #[test]
    fn a_bad_model_id_is_rejected_with_a_reason() {
        let cam = a_camera();
        // A real model exists, so parse succeeds, but this id is not runnable
        // for the tab — in fact it is not a model at all.
        let json = good_json("definitely-not-a-model", &cam);
        let err = plan(
            &fake(&json),
            "sys",
            "brief",
            30.0,
            UseCase::TextToVideo,
            &[],
        )
        .unwrap_err();
        match err {
            PlanError::UnknownModel { model_id, .. } => {
                assert_eq!(model_id, "definitely-not-a-model")
            }
            other => panic!("expected UnknownModel, got {other:?}"),
        }
    }

    #[test]
    fn a_model_that_cannot_run_the_tab_is_rejected() {
        // An image model named under a video tab: it exists in the registry but
        // `use_case::supports` says No, exactly as the picker would.
        let img = use_case::models_for(UseCase::TextToImage)
            .into_iter()
            .find(|m| !use_case::supports(m, UseCase::TextToVideo))
            .expect("an image model that cannot make video");
        let cam = a_camera();
        let json = good_json(&img.id, &cam);
        let err = plan(
            &fake(&json),
            "sys",
            "brief",
            30.0,
            UseCase::TextToVideo,
            &[],
        )
        .unwrap_err();
        match err {
            PlanError::ModelNotRunnable {
                model_id, use_case, ..
            } => {
                assert_eq!(model_id, img.id);
                assert_eq!(use_case, "text-to-video");
            }
            other => panic!("expected ModelNotRunnable, got {other:?}"),
        }
    }

    #[test]
    fn a_sum_outside_ten_percent_is_rejected() {
        let model = a_t2v_model();
        // Four 5s shots = 20s for a 30s target — 33% short.
        let json = format!(
            r#"{{"shots": [
  {{"title": "a", "prompt": "x", "seconds": 5, "camera": null, "modelId": "{model}", "role": "start"}},
  {{"title": "b", "prompt": "x", "seconds": 5, "camera": null, "modelId": "{model}", "role": "middle"}},
  {{"title": "c", "prompt": "x", "seconds": 5, "camera": null, "modelId": "{model}", "role": "middle"}},
  {{"title": "d", "prompt": "x", "seconds": 5, "camera": null, "modelId": "{model}", "role": "end"}}
]}}"#
        );
        let err = plan(
            &fake(&json),
            "sys",
            "brief",
            30.0,
            UseCase::TextToVideo,
            &[],
        )
        .unwrap_err();
        match err {
            PlanError::DurationSum { sum, target } => {
                assert_eq!(sum, 20.0);
                assert_eq!(target, 30.0);
            }
            other => panic!("expected DurationSum, got {other:?}"),
        }
    }

    #[test]
    fn a_refusal_is_a_typed_error_not_garbage() {
        // The S6 detector fires on the reply's shape before any parse. Dropping
        // the pre-check would make this a `NotJson` instead — that is the
        // non-vacuity mutation the design names.
        let err = plan(
            &fake("I can't help with that request."),
            "sys",
            "brief",
            30.0,
            UseCase::TextToVideo,
            &[],
        )
        .unwrap_err();
        assert!(matches!(err, PlanError::Refused(_)), "got {err:?}");
    }

    #[test]
    fn garbage_that_is_not_json_is_rejected() {
        // Prose that is not a refusal and not JSON: long enough to pass the S6
        // short-reply guard, so it must land in the parse step as `NotJson`.
        let err = plan(
            &fake("Here is a lovely paragraph about storyboards, but no JSON at all."),
            "sys",
            "brief",
            30.0,
            UseCase::TextToVideo,
            &[],
        )
        .unwrap_err();
        match err {
            PlanError::NotJson(cause) => assert!(!cause.is_empty(), "NotJson must carry a cause"),
            other => panic!("expected NotJson, got {other:?}"),
        }
    }

    #[test]
    fn an_unknown_camera_is_rejected() {
        let model = a_t2v_model();
        let json = good_json(&model, "zoomy-mc-zoomface");
        let err = plan(
            &fake(&json),
            "sys",
            "brief",
            30.0,
            UseCase::TextToVideo,
            &[],
        )
        .unwrap_err();
        match err {
            PlanError::UnknownCamera { slug, .. } => assert_eq!(slug, "zoomy-mc-zoomface"),
            other => panic!("expected UnknownCamera, got {other:?}"),
        }
    }

    #[test]
    fn a_backend_error_is_an_honest_plan_error() {
        let err = plan(
            &FakeChat(Err(JobError::Permanent("connection refused".into()))),
            "sys",
            "brief",
            30.0,
            UseCase::TextToVideo,
            &[],
        )
        .unwrap_err();
        match err {
            PlanError::Backend(why) => assert!(why.contains("connection refused"), "got {why}"),
            other => panic!("expected Backend, got {other:?}"),
        }
    }

    #[test]
    fn a_twelve_second_ltx_shot_clamps_to_ten_with_an_advisory() {
        // LTX 2.5 enumerates 6/8/10s; the catalogue-sourced axis (|_|None) says
        // so offline. A 12s ask snaps to the nearest offered value, 10s, and
        // records the reason rather than silently changing.
        let reg = registry();
        let model = reg.get("ltx_2_5").expect("ltx_2_5 in the registry");
        let mut shot = Shot {
            title: "hero".into(),
            prompt: "a shot".into(),
            seconds: 12.0,
            camera: None,
            model_id: "ltx_2_5".into(),
            role: ShotRole::Middle,
            continuity: None,
            advisories: Vec::new(),
        };
        clamp_shot(&mut shot, 2, model, UseCase::TextToVideo);
        assert_eq!(
            shot.seconds, 10.0,
            "12s must snap to the nearest offered 10s"
        );
        assert_eq!(shot.advisories.len(), 1);
        let note = &shot.advisories[0];
        assert!(note.contains("10s") && note.contains("12s"), "got: {note}");
    }

    #[test]
    fn a_single_shot_seedance_thirty_survives() {
        // Seedance 2.5 can make a native 30s clip; whether its axis is free-form
        // or enumerates 30, a 30s ask is kept and carries no clamp advisory.
        let reg = registry();
        let model = reg
            .get("seedance_2_5")
            .expect("seedance_2_5 in the registry");
        let mut shot = Shot {
            title: "one-take".into(),
            prompt: "a thirty second spot".into(),
            seconds: 30.0,
            camera: None,
            model_id: "seedance_2_5".into(),
            role: ShotRole::Start,
            continuity: None,
            advisories: Vec::new(),
        };
        clamp_shot(&mut shot, 0, model, UseCase::TextToVideo);
        assert_eq!(shot.seconds, 30.0);
        assert!(shot.advisories.is_empty(), "no clamp, so no advisory");
    }

    #[test]
    fn a_plan_with_continuity_and_roles_round_trips_through_serde() {
        let model = a_t2v_model();
        let cam = a_camera();
        let out = plan(
            &fake(&good_json(&model, &cam)),
            "sys",
            "brief",
            30.0,
            UseCase::TextToVideo,
            &[model],
        )
        .unwrap();
        let json = serde_json::to_string(&out).unwrap();
        let back: StoryboardPlan = serde_json::from_str(&json).unwrap();
        assert_eq!(out, back);
        assert_eq!(back.shots[0].role, ShotRole::Start);
        assert_eq!(back.shots[3].role, ShotRole::End);
        assert_eq!(
            back.shots[1].continuity,
            Some(Continuity { last_frame_of: 0 })
        );
    }

    #[test]
    fn the_shot_count_bound_is_three_to_eight() {
        let model = a_t2v_model();
        // Two shots is too few; the mutation check for the 3..=8 bound.
        let json = format!(
            r#"{{"shots": [
  {{"title": "a", "prompt": "x", "seconds": 15, "camera": null, "modelId": "{model}", "role": "start"}},
  {{"title": "b", "prompt": "x", "seconds": 15, "camera": null, "modelId": "{model}", "role": "end"}}
]}}"#
        );
        let err = plan(
            &fake(&json),
            "sys",
            "brief",
            30.0,
            UseCase::TextToVideo,
            &[],
        )
        .unwrap_err();
        assert!(matches!(err, PlanError::ShotCount(2)), "got {err:?}");
    }
}
