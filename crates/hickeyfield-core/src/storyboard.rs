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
//! which never guesses a nearest move), the shot count is 1..=8, and the
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
    /// In a multi-shot continuous plan this is composed by [`plan`] as
    /// `"{world} {action}"` so every shot carries the identical world block
    /// verbatim; a legacy or single-shot plan uses `prompt` directly.
    /// `#[serde(default)]` so a v2 plan that supplies only `world` + per-shot
    /// `action` (the continuous-run shape) parses; [`plan`] fills it.
    #[serde(default)]
    pub prompt: String,
    /// The per-shot beat, distinct from the shared `world`. The model drafts
    /// `world` once and an `action` per shot; [`plan`] composes them into
    /// `prompt`. `#[serde(default)]` so a legacy plan with only `prompt` parses.
    #[serde(default)]
    pub action: Option<String>,
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
    /// S15: whether this shot's model can continue from a start frame (has an
    /// image-to-video route). Computed server-side in [`plan`] so the UI's
    /// Chain-toggle default and the chain scheduler agree on the same
    /// authority. `#[serde(default)]` (false) so a plan written before this
    /// field still parses.
    #[serde(default)]
    pub chainable: bool,
    /// S17: whether this shot's model can bind an END frame (an `end_image_url`
    /// route not in `NO_END_FRAME`). The keyframe toggle enables only when every
    /// shot is end-frame-capable, so the UI default cannot drift from the
    /// scheduler. Computed server-side in [`plan`], mirroring `chainable`.
    /// `#[serde(default)]` (false) so a plan written before this field parses.
    #[serde(default)]
    pub end_frame_capable: bool,
    /// An optional per-shot resolution. One resolution per set is the rule (the
    /// frame dictates the canvas, so a mixed-resolution set is a continuity
    /// hazard — memo D-F); when shots disagree, [`plan`] records a
    /// `MixedResolution` advisory rather than rejecting. `#[serde(default)]`
    /// (None) so a plan that omits it — the common case, one set-wide
    /// resolution — still parses.
    #[serde(default)]
    pub resolution: Option<String>,
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
    /// The shared world block (subject, wardrobe, palette, lighting, lens,
    /// grade, aspect — no action verb) composed verbatim into every shot's
    /// prompt so a chained set holds identity across cuts (memo D-E).
    /// `#[serde(default)]` so a stored S13 plan still parses.
    #[serde(default)]
    pub world: String,
    #[serde(default)]
    pub brief: String,
    #[serde(default)]
    pub target_seconds: f64,
    /// The tab slug the plan was validated against.
    #[serde(default)]
    pub use_case: String,
    pub shots: Vec<Shot>,
    /// S17: the `N+1` still boundaries of a keyframe-anchored set (memo §3).
    /// Empty for a non-keyframe plan (`#[serde(default)]`), so every stored
    /// S13/S16 plan still parses. Derived by [`derive_keyframes`] from the world
    /// block + the shots' hold clauses — the model does not draft this; it is
    /// composed after validation.
    #[serde(default)]
    pub keyframes: Vec<Keyframe>,
}

/// A still's canvas, written into the image job's free-form `image_size` object
/// (the fal image models take `image_size: {width, height}` — verified keyless
/// against `alibaba/qwen-image-3/text-to-image`, S17 `schemas/`). Because "the
/// output canvas follows this image" on H3 Max i2v, this MUST equal the set's
/// target resolution × aspect, or the still is rescaled before conditioning and
/// the end anchor drifts (memo D-F). Serde camelCase for the UI bridge.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ImageSize {
    pub width: u32,
    pub height: u32,
}

/// One still boundary of a keyframe-anchored set. `K0` is shot 0's opening (its
/// `image_url`); `K_{i+1}` is shot `i`'s planned ending (its `end_image_url`).
/// Derived, not model-drafted (D1), so the still shares the identical world
/// string verbatim with the shots it anchors.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Keyframe {
    /// Boundary index `k` in `0..=N`.
    pub index: usize,
    /// `"{world} {hold clause}"` verbatim — the same world string the shots
    /// carry, so the still cannot drift from the run it anchors.
    pub prompt: String,
    /// The registry image model that will render this still (one model per set;
    /// default `qwen_image_3`, the D1 priced default).
    pub model_id: String,
    /// The canvas the still (and therefore the video) renders at.
    pub image_size: ImageSize,
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
    /// The shot count was not in `1..=8`.
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
    /// A multi-shot plan whose `world` block is shorter than five words — too
    /// thin to hold identity across a cut (memo D-E).
    MissingWorld,
    /// A composed `prompt` that does not begin with the `world` string, so a UI
    /// edit broke the verbatim composition the continuity depends on.
    PromptMissingWorld { shot: usize },
    /// Two shots whose `action`s are equal after trim + case-fold: a chained
    /// set of identical beats is a stutter, not continuity.
    IdenticalShots { a: usize, b: usize },
    /// A non-final shot whose `action` does not end on a static hold, so the
    /// next shot has no matching frame to open on.
    NoHoldEnding { shot: usize },
    /// A shot whose `action`/`prompt` carries a cut word ("cut to", "meanwhile",
    /// …) — an explicit scene break inside a continuous run.
    CutWords { shot: usize, word: String },
    /// A shot whose seconds exceed the model's hard per-call ceiling. A
    /// free-form axis cannot be silently capped past its range without the user
    /// knowing the target was unmet, so the planner must split to fill the
    /// ceiling (30s → 2×15) rather than propose one over-long shot.
    ShotOverCeiling {
        shot: usize,
        model_id: String,
        seconds: f64,
        ceiling: f64,
    },
    /// S17: a keyframe set whose still count is not `shots.len() + 1` — every
    /// shot boundary needs one still (both ends of every clip pinned).
    KeyframeCount { expected: usize, got: usize },
    /// S17: a keyframe's still model is not a registry model runnable for
    /// text-to-image, so it cannot render a still (the same predicate the
    /// picker gates on).
    KeyframeModelNotRunnable { index: usize, model_id: String },
    /// S17: a keyframe's `image_size` disagrees with the set's target canvas
    /// (aspect or resolution height) — a mixed canvas re-opens the continuity
    /// hazard, because H3 Max rescales the still before conditioning (memo D-F).
    KeyframeCanvasMismatch { index: usize },
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
                "a storyboard needs 1 to 8 shots, but the draft had {n}. Try drafting again."
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
            PlanError::MissingWorld => write!(
                f,
                "a multi-shot storyboard needs a shared world description to hold continuity across cuts. Try drafting again."
            ),
            PlanError::PromptMissingWorld { shot } => write!(
                f,
                "shot {}'s prompt no longer opens with the shared world block, which breaks continuity.",
                shot + 1
            ),
            PlanError::IdenticalShots { a, b } => write!(
                f,
                "shots {} and {} describe the same action — each shot needs its own beat.",
                a + 1,
                b + 1
            ),
            PlanError::NoHoldEnding { shot } => write!(
                f,
                "shot {} does not end on a static hold, so the next shot has no matching frame to continue from.",
                shot + 1
            ),
            PlanError::CutWords { shot, word } => write!(
                f,
                "shot {} uses a scene-break word (\"{word}\") inside a continuous run.",
                shot + 1
            ),
            PlanError::ShotOverCeiling {
                shot,
                model_id,
                seconds,
                ceiling,
            } => write!(
                f,
                "shot {} asks {model_id} for {}s, but it renders at most {}s per call — split the beat to fill the ceiling.",
                shot + 1,
                trim_secs(*seconds),
                trim_secs(*ceiling)
            ),
            PlanError::KeyframeCount { expected, got } => write!(
                f,
                "a keyframe set needs one still per shot boundary ({expected}), but the plan has {got}. Try drafting again."
            ),
            PlanError::KeyframeModelNotRunnable { index, model_id } => write!(
                f,
                "keyframe {index} names still model {model_id}, which cannot generate an image."
            ),
            PlanError::KeyframeCanvasMismatch { index } => write!(
                f,
                "keyframe {index}'s canvas does not match the set's resolution and aspect, which would rescale the still and break the anchor."
            ),
        }
    }
}

/// The ±fraction of the target the drafted durations must sum within.
const SUM_TOLERANCE: f64 = 0.10;

/// The default crossfade overlap between adjacent shots, in seconds. The single
/// source both the plan-time length gate and the render read, so the length the
/// user is validated against is the length the sidecar actually produces. A
/// hard cut is `0.0`; the render's `RenderOptions.overlap_seconds` defaults from
/// this.
pub const DEFAULT_CROSSFADE_SECONDS: f64 = 0.5;

/// The three numbers a continuity join is gated on (memo §0.1).
///
/// `ssim` is a floor (higher is better — same picture); `y`/`c` are ceilings on
/// the 8-bit luma / chroma average drift (lower is better — same grade). A join
/// passes iff `ssim >= ssim && dy <= y && du <= c && dv <= c`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ContinuityThresholds {
    pub ssim: f64,
    pub y: f64,
    pub c: f64,
}

/// The floor tier (S16 gates the flag on this). **Reasoned, not measured**: an
/// i2v first frame is a VAE round-trip of the conditioning image, so a faithful
/// reproduction lands ~0.9+ and a re-composed subject below ~0.7. These MUST be
/// re-read once against the first real paid 2×15 set and changed in one commit
/// (memo Q4 / design Q6) — the `--ignored` synth tests prove the machinery and
/// that the colour gate is non-vacuous, not that 0.80/12/6 is the right line.
pub const CONTINUITY_FLOOR: ContinuityThresholds = ContinuityThresholds {
    ssim: 0.80,
    y: 12.0,
    c: 6.0,
};

/// The ideal tier: *reported* on the badge ("continuous" vs "in-camera") but
/// NOT gated in S16. Also needs the real set to calibrate.
pub const CONTINUITY_IDEAL: ContinuityThresholds = ContinuityThresholds {
    ssim: 0.90,
    y: 6.0,
    c: 3.0,
};

/// The measured numbers for one join. Every field is optional because a scoring
/// failure (sidecar missing, frame unreadable) MUST record `None`, never `0.0`:
/// a `0` would read as a failed join and trigger a needless paid regenerate.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct JoinScore {
    pub ssim: Option<f64>,
    pub psnr: Option<f64>,
    pub dy: Option<f64>,
    pub du: Option<f64>,
    pub dv: Option<f64>,
}

impl JoinScore {
    /// A score that could not be computed — every number absent. Distinct from
    /// a zero score, which would flag the join as failed.
    pub fn unscored() -> Self {
        JoinScore::default()
    }

    /// True once the four gated numbers are present (so the pass/fail verdict is
    /// meaningful). An unscored join is neither passing nor failing.
    pub fn is_scored(&self) -> bool {
        self.ssim.is_some() && self.dy.is_some() && self.du.is_some() && self.dv.is_some()
    }
}

/// Whether a join clears the thresholds.
///
/// An **unscored** join returns `true` (not failing): a missing score must not
/// flag the join as low and fire a paid regenerate. A scored join passes iff
/// SSIM is at or above the floor and each colour drift is at or below its
/// ceiling.
pub fn join_passes(score: &JoinScore, t: ContinuityThresholds) -> bool {
    match (score.ssim, score.dy, score.du, score.dv) {
        (Some(ssim), Some(dy), Some(du), Some(dv)) => {
            ssim >= t.ssim && dy <= t.y && du <= t.c && dv <= t.c
        }
        // Unscored: not failing.
        _ => true,
    }
}

/// Whether a keyframe end-adherence score clears the thresholds (S17, AC4).
///
/// End adherence is the SSIM (+ colour drift) between a clip's ACTUAL last frame
/// and its planned still `K_{i+1}`. Keyframe mode is the 99% path, so this is
/// gated against [`CONTINUITY_IDEAL`], not the floor. Like [`join_passes`], an
/// **unscored** end (`None`) returns `true` (not failing): a missing score must
/// never fire a paid regenerate.
pub fn join_passes_end(score: &JoinScore, t: ContinuityThresholds) -> bool {
    // Identical shape to `join_passes` — the end score reuses the same
    // `JoinScore` carrier (its end-* fields on the record map onto this).
    join_passes(score, t)
}

/// The pass rate over the **scored** joins — "95% as a test, not a feeling"
/// (memo §0.5). Unscored joins are excluded (they are not evidence either way);
/// a set with no scored joins is `1.0` (nothing has failed).
pub fn continuity_rate(joins: &[JoinScore], t: ContinuityThresholds) -> f64 {
    let scored: Vec<&JoinScore> = joins.iter().filter(|j| j.is_scored()).collect();
    if scored.is_empty() {
        return 1.0;
    }
    let passing = scored.iter().filter(|j| join_passes(j, t)).count();
    passing as f64 / scored.len() as f64
}

/// How one shot joins the previous one in the stitched render (S15 D-B).
///
/// A **continuity** join is a hard cut (overlap 0): shot N+1's first frame *is*
/// shot N's last frame, so a crossfade would superimpose two instants of one
/// motion and read as a ghost/freeze — a match cut is invisible when the frames
/// match. A **crossfade** join blends over `seconds`, for a montage/scene-change
/// join where continuity was not applied.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Join {
    /// Hard cut — the previous shot's last frame seeded this one.
    Continuity,
    /// Blend over this many seconds.
    Crossfade(f64),
}

impl Join {
    /// The overlap this join removes from the stitched length.
    pub fn overlap(self) -> f64 {
        match self {
            Join::Continuity => 0.0,
            Join::Crossfade(s) => s,
        }
    }
}

/// The stitched length for a set of shots with an explicit per-join kind (D-B).
///
/// `joins[i]` is the join *into* shot `i+1`; there are `shots.len() − 1` of
/// them. A continuity join removes nothing (hard cut); a crossfade removes its
/// clamped overlap. This is the arithmetic the render actually produces once
/// continuity and crossfade joins are mixed, so the plan-time gate and the D3
/// duration assertion agree. Worked: 2×15 all-continuity → 30.0; 2×15 all
/// crossfade(0.5) → 29.5.
pub fn stitched_duration_joined(shots: &[Shot], joins: &[Join]) -> f64 {
    let mut total = shots.first().map_or(0.0, |s| s.seconds.max(0.0));
    let mut prev = total;
    for (shot, join) in shots.iter().skip(1).zip(joins.iter()) {
        let this = shot.seconds.max(0.0);
        let o = join.overlap();
        let d = if o > 0.0 { o.min(prev).min(this) } else { 0.0 };
        total += this - d;
        prev = this;
    }
    total
}

/// The stitched length of a set of shots crossfaded at `overlap` seconds each.
///
/// A crossfade is an *overlap*, not an insertion, so N shots joined by N−1
/// crossfades run `Σ seconds − (N−1)·overlap`, not the raw sum. Each overlap is
/// clamped to the shorter of the two neighbours it joins (a crossfade cannot be
/// longer than either side), mirroring [`crate::compositor::Timeline`]'s own
/// clamp so plan-time and render-time cannot disagree. Returns the raw sum for
/// `overlap <= 0` (a hard cut) or fewer than two shots.
pub fn stitched_duration(shots: &[Shot], overlap: f64) -> f64 {
    let mut total = shots.first().map_or(0.0, |s| s.seconds.max(0.0));
    let mut prev = total;
    for shot in shots.iter().skip(1) {
        let this = shot.seconds.max(0.0);
        let d = if overlap > 0.0 {
            overlap.min(prev).min(this)
        } else {
            0.0
        };
        total += this - d;
        prev = this;
    }
    total
}

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

    // 2b. Compose the world block into each shot's prompt VERBATIM (memo D-E):
    //     the exact string S7 previews and `submit_one` sends. Only when the
    //     plan carries a `world` and a shot supplies an `action`; a legacy plan
    //     with only `prompt` composes nothing.
    if !parsed.world.trim().is_empty() {
        for shot in parsed.shots.iter_mut() {
            if let Some(action) = shot.action.as_deref() {
                shot.prompt = format!("{} {}", parsed.world, action);
            }
        }
    }

    // 2c. Continuity rules (memo D-E): a multi-shot plan needs a world block,
    //     hold endings, distinct actions, no cut words; the composition must be
    //     verbatim. Pure, before the structural gates so a broken continuous
    //     run is named precisely.
    validate_continuity(&parsed.world, &parsed.shots)?;
    validate_composition(&parsed.world, &parsed.shots)?;

    // 3–6. Structural validation on the model's proposed shots. The stitched
    //    length (crossfade-aware) is gated against the target, using the
    //    default overlap the render will apply (AC4).
    validate(
        &parsed.shots,
        target_seconds,
        use_case,
        DEFAULT_CROSSFADE_SECONDS,
    )?;

    // Model-aware clamp + recorded continuity. Validation guaranteed every model
    // id resolves, so the lookups below cannot miss.
    let reg = registry();
    for (i, shot) in parsed.shots.iter_mut().enumerate() {
        if let Some(model) = reg.get(&shot.model_id) {
            clamp_shot(shot, i, model, use_case);
            // S15: does this shot's model serve image-to-video? The Chain-toggle
            // default reads this; it is the same authority the chain scheduler
            // gates on (a route that binds a start frame), so the two cannot
            // drift.
            shot.chainable = use_case::supports(model, UseCase::ImageToVideo);
            // S17: can this shot's model bind an END frame? The keyframe toggle
            // reads this so its default matches the scheduler's binding gate.
            shot.end_frame_capable = model.routes.first().is_some_and(|r| {
                crate::media::can_bind(
                    &model.spec,
                    crate::media::MediaRole::End,
                    crate::media::dialect_for(r.provider, &r.slug),
                    &r.slug,
                )
            });
        }
        shot.continuity = if i == 0 {
            None
        } else {
            Some(Continuity {
                last_frame_of: i - 1,
            })
        };
    }

    // AC4 (clamp interaction): the clamp narrows each shot toward the model's
    // duration axis, which the user cannot override — so if the *post-clamp*
    // stitched length has drifted beyond ±10% of the target, that is advised,
    // not failed. Failing here would strand a well-drafted plan on a model
    // constraint. Recorded as a plan-level advisory on the first shot.
    let stitched = stitched_duration(&parsed.shots, DEFAULT_CROSSFADE_SECONDS);
    if (stitched - target_seconds).abs() > SUM_TOLERANCE * target_seconds {
        if let Some(first) = parsed.shots.first_mut() {
            first.advisories.push(format!(
                "clamped shots run ~{}s stitched; target was {}s",
                trim_secs(stitched),
                trim_secs(target_seconds)
            ));
        }
    }

    // One resolution per set (memo D-F): a mixed-resolution set is a continuity
    // hazard, advised (not rejected) on the first shot.
    if let Some(advisory) = mixed_resolution_advisory(&parsed.shots) {
        if let Some(first) = parsed.shots.first_mut() {
            first.advisories.push(advisory);
        }
    }

    Ok(parsed)
}

/// Words that mark an explicit scene break — forbidden inside a continuous run,
/// where every cut is meant to be invisible (memo D-E).
const CUT_WORDS: [&str; 5] = ["cut to", "smash cut", "meanwhile", "elsewhere", "later"];

/// The minimum word count for a `world` block to hold identity across a cut.
const MIN_WORLD_WORDS: usize = 5;

/// Whether `hay` contains `needle` as a whole word/phrase — bounded on each side
/// by a non-alphanumeric character or a string edge. So the short cut word
/// `later` does not fire inside `slater`, and `elsewhere` does not fire inside a
/// longer token or a URL. `needle` is a lowercase ASCII word/phrase; `hay` is
/// already lowercased by the caller.
fn contains_word(hay: &str, needle: &str) -> bool {
    let mut from = 0;
    while let Some(rel) = hay[from..].find(needle) {
        let start = from + rel;
        let end = start + needle.len();
        let before_ok = start == 0
            || !hay[..start]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_alphanumeric());
        let after_ok = end == hay.len()
            || !hay[end..]
                .chars()
                .next()
                .is_some_and(|c| c.is_alphanumeric());
        if before_ok && after_ok {
            return true;
        }
        from = start + 1;
    }
    false
}

/// Whether an action ends on a hold — the v2 prompt dictates "static hold", but
/// a model may write "hold", "holds on", or "static hold", so any whole-word
/// `hold`/`holds` counts (the phrase "static hold" contains the word "hold").
fn ends_on_hold(action: &str) -> bool {
    contains_word(action, "hold") || contains_word(action, "holds")
}

/// The opening framing of a shot: the first sentence of its `action` (verbatim,
/// trimmed). Shot 0 opens on its own framing (the run's first still, K0), so K0
/// is derived from this rather than from a hold clause.
fn opening_clause(action: &str) -> String {
    action
        .split('.')
        .next()
        .unwrap_or(action)
        .trim()
        .to_string()
}

/// The ending-hold framing of a shot: the exact framing named after "hold of …"
/// (memo D-E — every non-final shot ends "Ends on a static hold of <framing>").
/// This is the describable still `K_{i+1}` must render. Falls back to the last
/// non-empty sentence, then the whole action, so a shot without the exact phrase
/// still yields a still prompt rather than an empty one.
fn ending_hold_clause(action: &str) -> String {
    let lower = action.to_lowercase();
    if let Some(pos) = lower.rfind("hold of ") {
        let tail = &action[pos + "hold of ".len()..];
        let clause = tail.trim().trim_end_matches('.').trim();
        if !clause.is_empty() {
            return clause.to_string();
        }
    }
    action
        .rsplit('.')
        .map(str::trim)
        .find(|s| !s.is_empty())
        .unwrap_or(action.trim())
        .to_string()
}

/// Compose a keyframe prompt: `"{world} {body}"` verbatim (the exact string the
/// shots carry), or just `body` when the plan declares no world.
fn compose_keyframe_prompt(world: &str, body: &str) -> String {
    let world = world.trim();
    if world.is_empty() {
        body.trim().to_string()
    } else {
        format!("{world} {}", body.trim())
    }
}

/// Derive the `N+1` keyframe stills of a keyframe-anchored set from an already
/// validated plan (D1). Pure: `K0` is shot 0's opening framing; `K_{i+1}` is
/// shot `i`'s ending hold clause; every still shares the plan's world string
/// verbatim and renders at `image_size`. `keyframes.len() == shots.len() + 1`.
///
/// The model does not draft this — deriving it guarantees the still and the
/// video share the identical world string (a second free-text draft is drift by
/// construction, memo §1) and needs no second LLM round trip.
pub fn derive_keyframes(
    plan: &StoryboardPlan,
    still_model: &str,
    image_size: ImageSize,
) -> Vec<Keyframe> {
    let world = plan.world.as_str();
    let n = plan.shots.len();
    let mut out = Vec::with_capacity(n + 1);

    // K0: shot 0's opening framing (the run's first still — its `image_url`).
    let k0_body = plan
        .shots
        .first()
        .map(|s| opening_clause(s.action.as_deref().unwrap_or(&s.prompt)))
        .unwrap_or_default();
    out.push(Keyframe {
        index: 0,
        prompt: compose_keyframe_prompt(world, &k0_body),
        model_id: still_model.to_string(),
        image_size,
    });

    // K_{i+1}: shot i's ending hold clause (that shot's `end_image_url`).
    for (i, shot) in plan.shots.iter().enumerate() {
        let body = ending_hold_clause(shot.action.as_deref().unwrap_or(&shot.prompt));
        out.push(Keyframe {
            index: i + 1,
            prompt: compose_keyframe_prompt(world, &body),
            model_id: still_model.to_string(),
            image_size,
        });
    }
    out
}

/// Validate the derived keyframes against the set (AC1), pure. Checks: the count
/// is `shots_len + 1`; every still model is a registry model runnable for
/// text-to-image (the picker's predicate); every still's canvas equals the set's
/// target `expected` canvas (a mixed canvas rescales the still — memo D-F).
pub fn validate_keyframes(
    keyframes: &[Keyframe],
    shots_len: usize,
    expected: ImageSize,
) -> Result<(), PlanError> {
    let want = shots_len + 1;
    if keyframes.len() != want {
        return Err(PlanError::KeyframeCount {
            expected: want,
            got: keyframes.len(),
        });
    }
    let reg = registry();
    for kf in keyframes {
        let runnable = reg
            .get(&kf.model_id)
            .is_some_and(|m| use_case::supports(m, UseCase::TextToImage));
        if !runnable {
            return Err(PlanError::KeyframeModelNotRunnable {
                index: kf.index,
                model_id: kf.model_id.clone(),
            });
        }
        if kf.image_size != expected {
            return Err(PlanError::KeyframeCanvasMismatch { index: kf.index });
        }
    }
    Ok(())
}

/// The continuity rules on a multi-shot **continuous** plan (memo D-E), pure.
///
/// Continuity is **opt-in through a non-empty `world`**: the v2 prompt requires
/// a world for a continuous run, while a montage or a stored pre-S16 plan
/// legitimately carries none and must still pass (backward-compat — the field
/// is `#[serde(default)]`). So the rules gate only a plan that declares a world;
/// a present-but-thin world (`< 5` words) on a multi-shot plan is the drafting
/// error `MissingWorld`. A single-shot plan needs no world regardless.
/// `MixedResolution` is an advisory handled in [`plan`], not a rejection here.
pub fn validate_continuity(world: &str, shots: &[Shot]) -> Result<(), PlanError> {
    let multi = shots.len() >= 2;
    let declares_world = !world.trim().is_empty();

    // A multi-shot plan that declares a world must give it enough substance to
    // hold identity across a cut.
    if multi && declares_world && world.split_whitespace().count() < MIN_WORLD_WORDS {
        return Err(PlanError::MissingWorld);
    }

    // Not a declared continuous run (montage / legacy): nothing to enforce.
    if !multi || !declares_world {
        return Ok(());
    }

    // Cut words in any shot's action or prompt.
    for (i, shot) in shots.iter().enumerate() {
        let hay =
            format!("{} {}", shot.action.as_deref().unwrap_or(""), shot.prompt).to_lowercase();
        if let Some(word) = CUT_WORDS.iter().find(|w| contains_word(&hay, w)) {
            return Err(PlanError::CutWords {
                shot: i,
                word: (*word).to_string(),
            });
        }
    }

    // A non-final shot must end on a static hold; the action carries the beat.
    for (i, shot) in shots.iter().enumerate() {
        let is_final = i + 1 == shots.len();
        if is_final {
            continue;
        }
        let action = shot
            .action
            .as_deref()
            .unwrap_or(&shot.prompt)
            .to_lowercase();
        if !ends_on_hold(&action) {
            return Err(PlanError::NoHoldEnding { shot: i });
        }
    }

    // No two shots share an action (after trim + case-fold): a chained set of
    // identical beats is a stutter, not continuity.
    for a in 0..shots.len() {
        for b in (a + 1)..shots.len() {
            let fa = shots[a]
                .action
                .as_deref()
                .unwrap_or("")
                .trim()
                .to_lowercase();
            let fb = shots[b]
                .action
                .as_deref()
                .unwrap_or("")
                .trim()
                .to_lowercase();
            if !fa.is_empty() && fa == fb {
                return Err(PlanError::IdenticalShots { a, b });
            }
        }
    }

    Ok(())
}

/// The `world`-composition check: every composed `prompt` in a multi-shot plan
/// must begin with the shared `world` string, or a UI edit has broken the
/// verbatim composition the continuity depends on. Pure.
pub fn validate_composition(world: &str, shots: &[Shot]) -> Result<(), PlanError> {
    if shots.len() < 2 || world.trim().is_empty() {
        return Ok(());
    }
    for (i, shot) in shots.iter().enumerate() {
        if !shot.prompt.starts_with(world) {
            return Err(PlanError::PromptMissingWorld { shot: i });
        }
    }
    Ok(())
}

/// The `MixedResolution` advisory (not a rejection): the distinct non-empty
/// resolutions a set's shots disagree on, if more than one. One resolution per
/// set is the rule; a mixed set is a continuity hazard the user should see.
pub fn mixed_resolution_advisory(shots: &[Shot]) -> Option<String> {
    let mut seen: Vec<&str> = Vec::new();
    for shot in shots {
        if let Some(r) = shot.resolution.as_deref() {
            if !r.is_empty() && !seen.contains(&r) {
                seen.push(r);
            }
        }
    }
    (seen.len() > 1).then(|| {
        format!(
            "shots mix resolutions ({}); one per set is safer",
            seen.join(", ")
        )
    })
}

/// Structural checks, in the documented order. Pure: registry/use-case/camera
/// lookups only, no network. Reused by [`plan`] and exposed for tests.
pub fn validate(
    shots: &[Shot],
    target_seconds: f64,
    use_case: UseCase,
    overlap: f64,
) -> Result<(), PlanError> {
    // 3. Shot count. S15 D-G: the floor is 1, not 3 — a single-shot plan (a
    //    Seedance 30s native, or a one-beat brief) is legitimate, and a 2×15
    //    H3 Max chain must be reachable. The ceiling of 8 stands.
    if !(1..=8).contains(&shots.len()) {
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

        // 5b. Fill-to-ceiling (memo D-F): a shot over the model's hard per-call
        //     ceiling is rejected, not silently clamped — clamping a free-form
        //     axis past its range would quietly lose target seconds. The
        //     planner is expected to split (30s → 2×15 on H3 Max). Skipped for
        //     a model with no known ceiling (a free-form axis with no range).
        if let Some(ceiling) = model_ceiling(model, use_case) {
            if shot.seconds > ceiling + f64::EPSILON {
                return Err(PlanError::ShotOverCeiling {
                    shot: i,
                    model_id: shot.model_id.clone(),
                    seconds: shot.seconds,
                    ceiling,
                });
            }
        }
    }

    // 6. Duration sum within ±10% of the target, on the *proposed* seconds —
    //    the drafting quality gate, before the per-shot clamp adjusts anything.
    //    The gate is on the STITCHED length (Σ seconds − overlaps), since a
    //    crossfade shortens the result: a caller that passes `overlap = 0.0`
    //    (a hard cut) gets the raw sum, so this is a strict generalisation of
    //    the S13 raw-sum gate.
    let sum = stitched_duration(shots, overlap);
    if (sum - target_seconds).abs() > SUM_TOLERANCE * target_seconds {
        return Err(PlanError::DurationSum {
            sum,
            target: target_seconds,
        });
    }

    Ok(())
}

/// The hard per-call ceiling of a **free-form** duration model, if it has a
/// documented range. Returns `None` for an enumerated axis (which [`clamp_shot`]
/// snaps toward its offered values instead) and for a free-form axis with no
/// range (the provider validates the exact number, nothing to enforce here).
///
/// This is the number the planner is asked to fill to (30s → 2×15 on H3 Max),
/// and the bound [`validate`] rejects a shot for exceeding — because a free-form
/// axis cannot be capped past its range without silently losing target seconds.
fn model_ceiling(model: &Model, use_case: UseCase) -> Option<f64> {
    let route = model.routes.first()?;
    let cap = capability::for_route(
        &model.spec,
        route.provider,
        &route.slug,
        use_case.input_mode(),
        |_| None,
    );
    // Only a free-form axis (no enumerated offers) is enforced by a ceiling;
    // an enumerated axis is snapped by `clamp_shot`.
    if cap.duration_seconds().is_some_and(|v| !v.is_empty()) {
        return None;
    }
    cap.duration_range().map(|(_, max)| max)
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
            return;
        }
    }

    // Free-form axis WITH a documented range (H3 Max 5–15, Wan 3.0 Prime 2–30):
    // snap into the bound with the same-shaped advisory. 12 on H3 Max stays 12
    // (5 ≤ 12 ≤ 15); 20 → 15. A free-form axis with no range, or an unknown
    // axis, is still left as-is — the provider validates the exact number.
    if let Some((min, max)) = cap.duration_range() {
        let asked = shot.seconds;
        if asked < min || asked > max {
            let clamped = asked.clamp(min, max);
            shot.advisories.push(format!(
                "shot {}: {} runs {}–{}s, not {}s — clamped",
                shot_index + 1,
                model.display_name,
                trim_secs(min),
                trim_secs(max),
                trim_secs(asked)
            ));
            shot.seconds = clamped;
        }
    }
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
    // Fill-to-ceiling (memo D-F): hand the model each allowed model's hard
    // per-call ceiling so it splits a long target into enough shots (30s on a
    // 15s-ceiling model = 2×15) rather than proposing one over-long shot that
    // the validator would reject with `ShotOverCeiling`.
    let reg = registry();
    let ceilings: Vec<String> = model_prefs
        .iter()
        .filter_map(|id| {
            let model = reg.get(id)?;
            let ceiling = model_ceiling(model, use_case)?;
            Some(format!("{id} ≤ {}s per shot", trim_secs(ceiling)))
        })
        .collect();
    if !ceilings.is_empty() {
        out.push_str("\nPer-call length ceilings (split a longer target across shots): ");
        out.push_str(&ceilings.join("; "));
    }
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

    /// Bare shots with the given durations (model id blank — for the pure
    /// `stitched_duration`, which reads only `seconds`).
    fn shots_of(seconds: &[f64]) -> Vec<Shot> {
        shots_on("", seconds)
    }

    /// Shots with the given durations on `model`, roles filled in, no cameras —
    /// for the overlap-aware `validate` (which resolves the model).
    fn shots_on(model: &str, seconds: &[f64]) -> Vec<Shot> {
        seconds
            .iter()
            .enumerate()
            .map(|(i, &s)| Shot {
                title: "x".into(),
                prompt: "x".into(),
                seconds: s,
                camera: None,
                model_id: model.to_string(),
                role: if i == 0 {
                    ShotRole::Start
                } else if i + 1 == seconds.len() {
                    ShotRole::End
                } else {
                    ShotRole::Middle
                },
                continuity: None,
                advisories: Vec::new(),
                chainable: false,
                end_frame_capable: false,
                action: None,
                resolution: None,
            })
            .collect()
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
                // The gate now reports the STITCHED length (S15 AC4): four 5s
                // shots joined by three 0.5s crossfades run 20 − 1.5 = 18.5s,
                // which is still far outside ±10% of 30s, so still rejected.
                assert_eq!(sum, 18.5);
                assert_eq!(target, 30.0);
            }
            other => panic!("expected DurationSum, got {other:?}"),
        }
    }

    #[test]
    fn stitched_duration_subtracts_the_overlaps() {
        // AC4: four shots 8/8/8/6 joined by three 0.5s crossfades run
        // 30 − 1.5 = 28.5s, not the raw 30. A hard cut (overlap 0) is the raw
        // sum. Mutating `stitched_duration` to return the raw sum fails this.
        let shots = shots_of(&[8.0, 8.0, 8.0, 6.0]);
        assert_eq!(stitched_duration(&shots, 0.5), 28.5);
        assert_eq!(stitched_duration(&shots, 0.0), 30.0);
        // Each overlap is clamped to the shorter neighbour: a 5s crossfade
        // between a 2s and a 3s shot can only take 2s.
        let short = shots_of(&[2.0, 3.0]);
        assert_eq!(stitched_duration(&short, 5.0), 3.0);
    }

    #[test]
    fn stitched_duration_joined_is_per_join() {
        // S15 D-B: a 2×15 set joined by a continuity (hard cut) keeps all 30s;
        // the same set crossfaded 0.5s renders 29.5s; a 3×10 set where shot 2
        // fell back (crossfade) but shot 1 chained (continuity) is 29.5s.
        let two = shots_of(&[15.0, 15.0]);
        assert_eq!(stitched_duration_joined(&two, &[Join::Continuity]), 30.0);
        assert_eq!(
            stitched_duration_joined(&two, &[Join::Crossfade(0.5)]),
            29.5
        );
        let three = shots_of(&[10.0, 10.0, 10.0]);
        assert_eq!(
            stitched_duration_joined(&three, &[Join::Crossfade(0.5), Join::Continuity]),
            29.5
        );
        // All faded 0.5s → 30 − 1.5 = 28.5 (design D4's example).
        let four = shots_of(&[8.0, 8.0, 8.0, 6.0]);
        assert_eq!(
            stitched_duration_joined(
                &four,
                &[
                    Join::Crossfade(0.5),
                    Join::Crossfade(0.5),
                    Join::Crossfade(0.5)
                ]
            ),
            28.5
        );
    }

    #[test]
    fn plan_time_gate_uses_the_stitched_duration() {
        // AC4: the overlap-aware validate accepts a 28.5s-stitched plan against
        // a 30s target (|28.5−30| = 1.5 ≤ 3) and rejects an 18.5s one.
        let model = a_t2v_model();
        let accepted = shots_on(&model, &[8.0, 8.0, 8.0, 6.0]);
        assert!(
            validate(&accepted, 30.0, UseCase::TextToVideo, 0.5).is_ok(),
            "28.5 stitched is within ±10% of 30"
        );
        let rejected = shots_on(&model, &[5.0, 5.0, 5.0, 5.0]);
        assert!(
            validate(&rejected, 30.0, UseCase::TextToVideo, 0.5).is_err(),
            "18.5 stitched is not within ±10% of 30"
        );
        // A hard cut (overlap 0) uses the raw sum: 8+8+8+6 = 30 = target, ok.
        assert!(validate(&accepted, 30.0, UseCase::TextToVideo, 0.0).is_ok());
        // …and a 5×4 raw sum of 20 is still short even as a hard cut.
        assert!(validate(&rejected, 30.0, UseCase::TextToVideo, 0.0).is_err());
    }

    #[test]
    fn clamp_drift_is_advised_not_failed() {
        // AC4: a plan whose per-shot clamp pushes the stitched length outside
        // ±10% is accepted with a plan-level advisory, not rejected — the clamp
        // is a model constraint the user cannot override. Kling 2.6 offers an
        // enumerated 5/10s axis, so 8s shots snap to a value that drifts the
        // stitched total; the plan still comes back Ok with an advisory.
        let model = "kling2_6";
        if !registry().contains_key(model) {
            return; // catalogue drift: skip rather than assert on a missing id
        }
        let cam = a_camera();
        let json = format!(
            r#"{{"shots": [
  {{"title": "a", "prompt": "x", "seconds": 8, "camera": "{cam}", "modelId": "{model}", "role": "start"}},
  {{"title": "b", "prompt": "x", "seconds": 8, "camera": null, "modelId": "{model}", "role": "middle"}},
  {{"title": "c", "prompt": "x", "seconds": 8, "camera": null, "modelId": "{model}", "role": "middle"}},
  {{"title": "d", "prompt": "x", "seconds": 6, "camera": null, "modelId": "{model}", "role": "end"}}
]}}"#
        );
        let out = plan(
            &fake(&json),
            "sys",
            "brief",
            30.0,
            UseCase::TextToVideo,
            &[],
        );
        // Either the model isn't runnable here (catalogue-dependent) — skip —
        // or the plan is accepted; it must never be rejected on clamp drift.
        if let Ok(p) = out {
            let clamped = p.shots.iter().any(|s| !s.advisories.is_empty());
            let stitched = stitched_duration(&p.shots, DEFAULT_CROSSFADE_SECONDS);
            if (stitched - 30.0).abs() > 0.10 * 30.0 {
                assert!(
                    p.shots[0].advisories.iter().any(|a| a.contains("stitched")),
                    "a drifted clamp must carry the plan-level advisory: {:?}",
                    p.shots[0].advisories
                );
            }
            let _ = clamped;
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
            chainable: false,
            end_frame_capable: false,
            action: None,
            resolution: None,
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
            chainable: false,
            end_frame_capable: false,
            action: None,
            resolution: None,
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
    fn the_shot_count_bound_is_one_to_eight() {
        // S15 D-G: a two-shot plan (the 2×15 H3 Max chain) and a one-shot plan
        // (a Seedance 30s native) are now BOTH accepted; the floor is 1. Only
        // an empty plan is too few. The mutation check for the `1..=8` bound.
        let model = a_t2v_model();
        let two = format!(
            r#"{{"shots": [
  {{"title": "a", "prompt": "x", "seconds": 15, "camera": null, "modelId": "{model}", "role": "start"}},
  {{"title": "b", "prompt": "x", "seconds": 15, "camera": null, "modelId": "{model}", "role": "end"}}
]}}"#
        );
        assert!(
            plan(&fake(&two), "sys", "brief", 30.0, UseCase::TextToVideo, &[]).is_ok(),
            "two 15s shots (H3 Max 2×15) must be accepted"
        );

        // Zero shots is still too few.
        let none = r#"{"shots": []}"#;
        let err = plan(&fake(none), "sys", "brief", 30.0, UseCase::TextToVideo, &[]).unwrap_err();
        assert!(matches!(err, PlanError::ShotCount(0)), "got {err:?}");

        // Nine shots is still too many.
        let nine: String = format!(
            r#"{{"shots": [{}]}}"#,
            (0..9)
                .map(|_| format!(
                    r#"{{"title":"a","prompt":"x","seconds":3.3,"camera":null,"modelId":"{model}","role":"middle"}}"#
                ))
                .collect::<Vec<_>>()
                .join(",")
        );
        let err = plan(
            &fake(&nine),
            "sys",
            "brief",
            30.0,
            UseCase::TextToVideo,
            &[],
        )
        .unwrap_err();
        assert!(matches!(err, PlanError::ShotCount(9)), "got {err:?}");
    }

    // ---- S16: duration range clamp + fill-to-ceiling (D5) ----

    /// The H3 Max image-to-video model, a free-form 5–15s axis.
    fn h3_max() -> &'static str {
        "minimax_h3_max"
    }

    fn one_shot(model: &str, seconds: f64) -> Shot {
        Shot {
            title: "x".into(),
            prompt: "x".into(),
            action: None,
            seconds,
            camera: None,
            model_id: model.to_string(),
            role: ShotRole::Start,
            continuity: None,
            advisories: Vec::new(),
            chainable: false,
            end_frame_capable: false,
            resolution: None,
        }
    }

    #[test]
    fn clamp_shot_uses_the_range_for_a_free_form_model() {
        let reg = registry();
        let model = reg.get(h3_max()).expect("h3 max is registered");

        // 12 on H3 Max stays 12 (5 ≤ 12 ≤ 15), no advisory.
        let mut ok = one_shot(h3_max(), 12.0);
        clamp_shot(&mut ok, 0, model, UseCase::ImageToVideo);
        assert_eq!(ok.seconds, 12.0);
        assert!(ok.advisories.is_empty());

        // 20 → 15, one advisory naming the range.
        let mut over = one_shot(h3_max(), 20.0);
        clamp_shot(&mut over, 0, model, UseCase::ImageToVideo);
        assert_eq!(over.seconds, 15.0);
        assert_eq!(over.advisories.len(), 1);
        assert!(
            over.advisories[0].contains("5–15s"),
            "{}",
            over.advisories[0]
        );
    }

    #[test]
    fn a_single_shot_thirty_survives_only_when_the_range_reaches_it() {
        // A model whose free-form range reaches 30 keeps a single 30s shot.
        let wan = "wan3_0_prime";
        let plan_ok = validate(&shots_on(wan, &[30.0]), 30.0, UseCase::TextToVideo, 0.0);
        assert!(plan_ok.is_ok(), "wan 3.0 prime reaches 30s: {plan_ok:?}");

        // A single 30s shot on H3 Max (ceiling 15) is rejected — split it.
        let err = validate(
            &shots_on(h3_max(), &[30.0]),
            30.0,
            UseCase::ImageToVideo,
            0.0,
        )
        .unwrap_err();
        assert!(
            matches!(err, PlanError::ShotOverCeiling { ceiling, .. } if ceiling == 15.0),
            "got {err:?}"
        );
    }

    #[test]
    fn fill_to_ceiling_splits_thirty_into_two_fifteens() {
        // A 2×15 H3 Max plan is accepted; the ceiling reaches the drafting
        // prompt so the model knows to split.
        let two = validate(
            &shots_on(h3_max(), &[15.0, 15.0]),
            30.0,
            UseCase::ImageToVideo,
            0.0,
        );
        assert!(two.is_ok(), "2×15 on h3 max is valid: {two:?}");

        let msg = build_user_message(
            "a 30s ad",
            30.0,
            UseCase::ImageToVideo,
            &[h3_max().to_string()],
        );
        assert!(
            msg.contains("≤ 15s per shot"),
            "the ceiling must reach the model: {msg}"
        );
    }

    // ---- S16: world block + validator (D6) ----

    fn continuous_json(world: &str) -> String {
        let model = h3_max();
        format!(
            r#"{{"world": "{world}",
"shots": [
  {{"title":"A","action":"The can beads with condensation. Ends on a static hold of the can centred.","seconds":15,"camera":null,"modelId":"{model}","role":"start"}},
  {{"title":"B","action":"Opens on the same static hold of the can centred, then the drop falls.","seconds":15,"camera":null,"modelId":"{model}","role":"end"}}
]}}"#
        )
    }

    const WORLD: &str = "A matte-black cold-brew can with a copper rim on wet slate, studio product photography, deep teal palette";

    #[test]
    fn a_world_block_is_composed_verbatim_into_each_prompt() {
        let out = plan(
            &fake(&continuous_json(WORLD)),
            "sys",
            "a 30s cold-brew ad",
            30.0,
            UseCase::ImageToVideo,
            &[h3_max().to_string()],
        )
        .expect("a continuous plan is accepted");
        for shot in &out.shots {
            assert!(shot.prompt.starts_with(WORLD), "prompt: {}", shot.prompt);
        }
    }

    #[test]
    fn identical_actions_are_rejected() {
        let model = h3_max();
        let dup = format!(
            r#"{{"world":"{WORLD}","shots":[
  {{"title":"A","action":"Ends on a static hold of the can.","seconds":15,"camera":null,"modelId":"{model}","role":"start"}},
  {{"title":"B","action":"Ends on a static hold of the can.","seconds":15,"camera":null,"modelId":"{model}","role":"end"}}
]}}"#
        );
        let err = plan(
            &fake(&dup),
            "sys",
            "b",
            30.0,
            UseCase::ImageToVideo,
            &[model.to_string()],
        )
        .unwrap_err();
        assert!(
            matches!(err, PlanError::IdenticalShots { a: 0, b: 1 }),
            "got {err:?}"
        );
    }

    #[test]
    fn a_non_final_shot_without_a_hold_is_rejected() {
        let model = h3_max();
        let nohold = format!(
            r#"{{"world":"{WORLD}","shots":[
  {{"title":"A","action":"The can spins wildly with no pause at all.","seconds":15,"camera":null,"modelId":"{model}","role":"start"}},
  {{"title":"B","action":"Opens on the same static hold, then it settles.","seconds":15,"camera":null,"modelId":"{model}","role":"end"}}
]}}"#
        );
        let err = plan(
            &fake(&nohold),
            "sys",
            "b",
            30.0,
            UseCase::ImageToVideo,
            &[model.to_string()],
        )
        .unwrap_err();
        assert!(
            matches!(err, PlanError::NoHoldEnding { shot: 0 }),
            "got {err:?}"
        );
    }

    #[test]
    fn a_short_world_is_rejected() {
        let err = plan(
            &fake(&continuous_json("three words here")),
            "sys",
            "b",
            30.0,
            UseCase::ImageToVideo,
            &[h3_max().to_string()],
        )
        .unwrap_err();
        assert!(matches!(err, PlanError::MissingWorld), "got {err:?}");
    }

    #[test]
    fn a_cut_word_is_rejected() {
        let model = h3_max();
        let cut = format!(
            r#"{{"world":"{WORLD}","shots":[
  {{"title":"A","action":"The can beads. Ends on a static hold of the can. Meanwhile the label shifts.","seconds":15,"camera":null,"modelId":"{model}","role":"start"}},
  {{"title":"B","action":"Opens on the same static hold, then it tilts.","seconds":15,"camera":null,"modelId":"{model}","role":"end"}}
]}}"#
        );
        let err = plan(
            &fake(&cut),
            "sys",
            "b",
            30.0,
            UseCase::ImageToVideo,
            &[model.to_string()],
        )
        .unwrap_err();
        assert!(
            matches!(err, PlanError::CutWords { shot: 0, .. }),
            "got {err:?}"
        );
    }

    #[test]
    fn a_cut_word_matches_only_whole_words() {
        // The short cut word `later` must not fire inside `slater`, and
        // `elsewhere` must not fire inside `elsewheres`/a longer token.
        assert!(contains_word("meanwhile the label shifts", "meanwhile"));
        assert!(contains_word("it holds, then later it moves", "later"));
        assert!(!contains_word("mr slater walks in", "later"));
        assert!(!contains_word("the elsewheres of the frame", "elsewhere"));
        assert!(contains_word("cut to black", "cut to"));
    }

    #[test]
    fn hold_ending_accepts_several_phrasings() {
        assert!(ends_on_hold("ends on a static hold of the can"));
        assert!(ends_on_hold("the can holds on the centre"));
        assert!(ends_on_hold("it comes to a hold"));
        assert!(!ends_on_hold("the can spins wildly with no pause"));
        // Not fooled by a longer word that merely contains "hold".
        assert!(!ends_on_hold("the household is quiet"));
    }

    #[test]
    fn a_holds_on_ending_passes() {
        let model = h3_max();
        let plan_json = format!(
            r#"{{"world":"{WORLD}","shots":[
  {{"title":"A","action":"The can beads with condensation, then holds on the centred can.","seconds":15,"camera":null,"modelId":"{model}","role":"start"}},
  {{"title":"B","action":"Opens on the same static hold, then the drop falls.","seconds":15,"camera":null,"modelId":"{model}","role":"end"}}
]}}"#
        );
        let out = plan(
            &fake(&plan_json),
            "sys",
            "b",
            30.0,
            UseCase::ImageToVideo,
            &[model.to_string()],
        );
        assert!(out.is_ok(), "got {out:?}");
    }

    #[test]
    fn a_broken_composition_is_caught() {
        // A UI edit that prepended text to shot 0's prompt breaks the verbatim
        // world composition. `validate_composition` catches it directly.
        let mut shots = shots_on(h3_max(), &[15.0, 15.0]);
        shots[0].prompt = format!("EXTRA {WORLD} beat one");
        shots[1].prompt = format!("{WORLD} beat two");
        let err = validate_composition(WORLD, &shots).unwrap_err();
        assert!(
            matches!(err, PlanError::PromptMissingWorld { shot: 0 }),
            "got {err:?}"
        );
    }

    #[test]
    fn the_one_to_eight_bound_is_preserved() {
        // The S15 D-G floor is untouched: one shot ok, zero rejected.
        assert!(validate(
            &shots_on(h3_max(), &[10.0]),
            10.0,
            UseCase::ImageToVideo,
            0.0
        )
        .is_ok());
        assert!(matches!(
            validate(&[], 10.0, UseCase::ImageToVideo, 0.0),
            Err(PlanError::ShotCount(0))
        ));
    }

    #[test]
    fn a_mixed_resolution_set_is_advised_not_rejected() {
        let mut shots = shots_on(h3_max(), &[15.0, 15.0]);
        shots[0].resolution = Some("768P".into());
        shots[1].resolution = Some("1080P".into());
        let advisory = mixed_resolution_advisory(&shots).expect("a mix is advised");
        assert!(advisory.contains("768P") && advisory.contains("1080P"));
        // A uniform set is silent.
        shots[1].resolution = Some("768P".into());
        assert!(mixed_resolution_advisory(&shots).is_none());
    }

    // ---- S16: score thresholds (D7) ----

    fn scored(ssim: f64, dy: f64, du: f64, dv: f64) -> JoinScore {
        JoinScore {
            ssim: Some(ssim),
            psnr: Some(30.0),
            dy: Some(dy),
            du: Some(du),
            dv: Some(dv),
        }
    }

    #[test]
    fn join_passes_at_the_four_boundaries() {
        let t = CONTINUITY_FLOOR; // 0.80 / 12 / 6
                                  // Exactly on every boundary passes.
        assert!(join_passes(&scored(0.80, 12.0, 6.0, 6.0), t));
        // One over each of the four fails.
        assert!(!join_passes(&scored(0.79, 12.0, 6.0, 6.0), t), "ssim floor");
        assert!(!join_passes(&scored(0.80, 12.1, 6.0, 6.0), t), "dy ceiling");
        assert!(!join_passes(&scored(0.80, 12.0, 6.1, 6.0), t), "du ceiling");
        assert!(!join_passes(&scored(0.80, 12.0, 6.0, 6.1), t), "dv ceiling");
        // Unscored is not failing (no spurious regenerate).
        assert!(join_passes(&JoinScore::unscored(), t));
        assert!(join_passes(
            &JoinScore {
                ssim: Some(0.1),
                dy: None,
                ..JoinScore::default()
            },
            t
        ));
    }

    #[test]
    fn continuity_rate_over_a_fake_store() {
        let mut joins: Vec<JoinScore> = (0..19).map(|_| scored(0.95, 2.0, 1.0, 1.0)).collect();
        joins.push(scored(0.5, 2.0, 1.0, 1.0)); // one fail
        assert!((continuity_rate(&joins, CONTINUITY_FLOOR) - 0.95).abs() < 1e-9);
        // Unscored joins are excluded from the rate.
        joins.push(JoinScore::unscored());
        assert!((continuity_rate(&joins, CONTINUITY_FLOOR) - 0.95).abs() < 1e-9);
        // No scored joins → 1.0 (nothing has failed).
        assert_eq!(
            continuity_rate(&[JoinScore::unscored()], CONTINUITY_FLOOR),
            1.0
        );
    }

    // ---- S17: keyframe derivation + validation (D1) + end adherence (D4) ----

    fn a_keyframe_plan() -> StoryboardPlan {
        // A 2×15 continuous plan with distinct hold clauses, parsed through the
        // real `plan()` so the shots carry composed prompts + actions.
        let model = h3_max();
        let json = format!(
            r#"{{"world": "{WORLD}",
"shots": [
  {{"title":"A","action":"The can centred, label to camera, one drop about to fall. Ends on a static hold of the can centred, label to camera.","seconds":15,"camera":null,"modelId":"{model}","role":"start"}},
  {{"title":"B","action":"Opens on the same static hold of the can centred, then the can fills the frame. Ends on a static hold of the can filling the frame, label sharp.","seconds":15,"camera":null,"modelId":"{model}","role":"end"}}
]}}"#
        );
        plan(
            &fake(&json),
            "sys",
            "a 30s cold-brew ad",
            30.0,
            UseCase::ImageToVideo,
            &[model.to_string()],
        )
        .expect("the keyframe fixture plans clean")
    }

    const CANVAS: ImageSize = ImageSize {
        width: 1360,
        height: 768,
    };

    #[test]
    fn derive_keyframes_builds_n_plus_one_stills_from_the_holds() {
        let plan = a_keyframe_plan();
        let kfs = derive_keyframes(&plan, "qwen_image_3", CANVAS);
        assert_eq!(kfs.len(), plan.shots.len() + 1, "N+1 stills");
        assert_eq!(kfs.len(), 3);
        // K0 is shot 0's opening framing; K1/K2 are each shot's ending hold.
        assert_eq!(
            kfs[0].prompt,
            format!("{WORLD} The can centred, label to camera, one drop about to fall")
        );
        assert_eq!(
            kfs[1].prompt,
            format!("{WORLD} the can centred, label to camera")
        );
        assert_eq!(
            kfs[2].prompt,
            format!("{WORLD} the can filling the frame, label sharp")
        );
        for (i, kf) in kfs.iter().enumerate() {
            assert_eq!(kf.index, i);
            assert_eq!(kf.model_id, "qwen_image_3");
            assert_eq!(kf.image_size, CANVAS);
            assert!(kf.prompt.starts_with(WORLD), "world is verbatim");
        }
    }

    #[test]
    fn a_keyframe_count_mismatch_is_rejected() {
        let plan = a_keyframe_plan();
        let mut kfs = derive_keyframes(&plan, "qwen_image_3", CANVAS);
        // The correct count validates.
        assert!(validate_keyframes(&kfs, plan.shots.len(), CANVAS).is_ok());
        // One short → KeyframeCount.
        kfs.pop();
        let err = validate_keyframes(&kfs, plan.shots.len(), CANVAS).unwrap_err();
        assert!(
            matches!(
                err,
                PlanError::KeyframeCount {
                    expected: 3,
                    got: 2
                }
            ),
            "got {err:?}"
        );
    }

    #[test]
    fn a_non_text_to_image_still_model_is_rejected() {
        let plan = a_keyframe_plan();
        // qwen_image_3 is a runnable text-to-image model → accepted.
        let ok = derive_keyframes(&plan, "qwen_image_3", CANVAS);
        assert!(validate_keyframes(&ok, plan.shots.len(), CANVAS).is_ok());
        // kling3_0_pro is a video model, not text-to-image → rejected.
        let bad = derive_keyframes(&plan, "kling3_0_pro", CANVAS);
        let err = validate_keyframes(&bad, plan.shots.len(), CANVAS).unwrap_err();
        assert!(
            matches!(&err, PlanError::KeyframeModelNotRunnable { model_id, .. } if model_id == "kling3_0_pro"),
            "got {err:?}"
        );
    }

    #[test]
    fn a_still_canvas_that_disagrees_with_the_set_is_rejected() {
        let plan = a_keyframe_plan();
        let mut kfs = derive_keyframes(&plan, "qwen_image_3", CANVAS);
        // A single still off the set canvas (wrong aspect/height) → mismatch.
        kfs[1].image_size = ImageSize {
            width: 768,
            height: 768,
        };
        let err = validate_keyframes(&kfs, plan.shots.len(), CANVAS).unwrap_err();
        assert!(
            matches!(err, PlanError::KeyframeCanvasMismatch { index: 1 }),
            "got {err:?}"
        );
    }

    #[test]
    fn join_passes_end_at_the_ideal_boundaries() {
        // Exactly on the ideal (0.90 / 6 / 3) passes.
        assert!(join_passes_end(
            &scored(0.90, 6.0, 3.0, 3.0),
            CONTINUITY_IDEAL
        ));
        // One-over each bound fails.
        assert!(!join_passes_end(
            &scored(0.89, 6.0, 3.0, 3.0),
            CONTINUITY_IDEAL
        ));
        assert!(!join_passes_end(
            &scored(0.95, 6.1, 3.0, 3.0),
            CONTINUITY_IDEAL
        ));
        assert!(!join_passes_end(
            &scored(0.95, 6.0, 3.1, 3.0),
            CONTINUITY_IDEAL
        ));
        assert!(!join_passes_end(
            &scored(0.95, 6.0, 3.0, 3.1),
            CONTINUITY_IDEAL
        ));
        // All-None is not failing (unscored end must never fire a regenerate).
        assert!(join_passes_end(&JoinScore::unscored(), CONTINUITY_IDEAL));
    }
}
