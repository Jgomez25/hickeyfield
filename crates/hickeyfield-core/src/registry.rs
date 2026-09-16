//! The model registry: catalogue specs joined to routes and per-route prices.
//!
//! [`crate::catalog`] answers "what does this model accept". This module answers
//! the two questions that make a model usable: **who serves it** and **what will
//! it cost me**. Those are separate axes — the same logical model is a different
//! price on fal than on Vercel AI Gateway, sometimes by 2x — so cost is stored
//! per route, never per model.
//!
//! Three sources feed it, in this order:
//!
//! 1. the vendored `MODELS.md` spec (flags, defaults, constraints),
//! 2. the route table from the research plan §3.2 plus the per-model price rows
//!    in §2.1/§2.2 and `gap-1.md`,
//! 3. hand-authored specs for the models in Higgsfield's *live picker* that
//!    their own CLI spec omits.
//!
//! **Every USD figure here is transcribed from those documents.** Where a
//! provider publishes no price — several fal `pricingInfoOverride` fields are
//! literally blank — the entry is [`CostModel::Unknown`] and the resolver sorts
//! it behind anything priced. Guessing would be worse than admitting ignorance,
//! because the number goes straight onto the Generate button.
//!
//! **Route slugs are family roots.** A single Higgsfield model maps to one
//! endpoint per input mode on every provider (`/text-to-video`,
//! `/image-to-video`, `/reference-to-video`, …), and the input mode is not known
//! until the user has attached their media. The adapter appends the suffix; the
//! route names the family. Where the corpus never recorded a slug at all, the
//! route carries a note beginning `unverified slug` so the uncertainty reaches
//! the UI instead of dying in a comment.

use std::collections::BTreeMap;

use crate::catalog::{self, Arity, FlagSpec, Modality, ModelSpec, ValueSpec};
use crate::cost::{Billable, CostModel, Estimate};
use crate::enhance::JobType;
use crate::provider::ProviderId;
use crate::route::Route;

/// A routable, priceable model.
#[derive(Debug, Clone, PartialEq)]
pub struct Model {
    /// Higgsfield's `job_set_type`, reused verbatim so their preset and credit
    /// data line up with ours without a translation table.
    pub id: String,
    pub display_name: String,
    pub modality: Modality,
    /// Which prompt-enhance dialect this model's prompt compiles under —
    /// the join that lets [`crate::enhance::decide`] run for a real generation.
    /// Assigned from [`JOB_TYPES`]; that table also carries the reasoning for
    /// every model where the assignment was a judgement rather than an identity.
    pub job_type: JobType,
    /// What the model accepts. Parsed from the vendored spec, or hand-authored
    /// for the picker-only models.
    pub spec: ModelSpec,
    /// Ordered by preference — roughly quality-first. [`crate::route::resolve`]
    /// reorders by price when the policy asks it to.
    pub routes: Vec<Route>,
    /// Keyed by [`Route::id`], because two routes can share a provider.
    costs: BTreeMap<String, CostModel>,
    /// In the plan's M1 launch set.
    pub launch: bool,
}

impl Model {
    pub fn cost_model(&self, route: &Route) -> Option<&CostModel> {
        self.costs.get(&route.id())
    }

    /// `None` when the price is unknown *or* the route is not one this model
    /// offers. Both cases mean "we cannot quote you", which is the only thing
    /// the caller can act on.
    pub fn estimate(&self, route: &Route, b: &Billable) -> Option<Estimate> {
        self.cost_model(route)?.estimate(b)
    }

    pub fn route(&self, id: &str) -> Option<&Route> {
        self.routes.iter().find(|r| r.id() == id)
    }

    /// The `cost_of` closure [`crate::route::resolve`] wants.
    pub fn pricer<'a>(&'a self, b: &'a Billable) -> impl Fn(&Route) -> Option<f64> + 'a {
        move |r| self.estimate(r, b).map(|e| e.usd)
    }
}

/// The full model set: the vendored catalogue minus [`EXCLUSIONS`], plus the
/// picker-only models the vendored spec never listed.
pub fn registry() -> BTreeMap<String, Model> {
    let catalogue = catalog::catalogue();
    let mut out: BTreeMap<String, Model> = BTreeMap::new();

    for (id, spec) in &catalogue {
        if EXCLUSIONS.iter().any(|(x, _)| id == x) {
            continue;
        }
        let m = assemble(spec.clone());
        out.insert(m.id.clone(), m);
    }

    for spec in picker_only_specs(&catalogue) {
        let m = assemble(spec);
        out.insert(m.id.clone(), m);
    }

    out
}

/// The M1 launch set, expanded from [`LAUNCH_FAMILIES`].
pub fn launch_models() -> Vec<Model> {
    let reg = registry();
    LAUNCH_FAMILIES
        .iter()
        .flat_map(|(_, ids)| ids.iter())
        .filter_map(|id| reg.get(*id).cloned())
        .collect()
}

fn assemble(mut spec: ModelSpec) -> Model {
    for (id, label) in DISPLAY_OVERRIDES {
        if spec.id == id {
            spec.display_name = label.to_string();
        }
    }

    let priced = priced_routes(&spec.id);
    debug_assert!(!priced.is_empty(), "{} has no route", spec.id);

    let routes: Vec<Route> = priced.iter().map(|(r, _)| r.clone()).collect();
    let costs = priced.into_iter().map(|(r, c)| (r.id(), c)).collect();
    let launch = LAUNCH_FAMILIES
        .iter()
        .any(|(_, ids)| ids.contains(&spec.id.as_str()));

    Model {
        id: spec.id.clone(),
        display_name: spec.display_name.clone(),
        modality: spec.modality,
        job_type: job_type_for(&spec.id, spec.modality),
        routes,
        costs,
        launch,
        spec,
    }
}

// ---------------------------------------------------------------------------
// Naming
// ---------------------------------------------------------------------------

/// Higgsfield's CLI spec and Higgsfield's own product disagree on four labels.
/// The CLI is an internal artifact; the picker is what a user has seen before
/// they ever open Hickeyfield, so the picker wins. Kept as an explicit table rather
/// than a string transform, because these are four editorial decisions someone
/// made — not a pattern — and a fifth will not follow from a rule.
///
/// (CLI `MODELS.md` heading, label their app shows)
pub const DISPLAY_OVERRIDES: [(&str, &str); 4] = [
    ("kling3_0", "Kling 3.0"),              // "Kling v3.0"
    ("kling2_6", "Kling 2.6"),              // "Kling 2.6 Video"
    ("wan2_6", "Wan 2.6"),                  // "Wan 2.6 Video"
    ("grok_video_v15", "Grok Imagine 1.5"), // "Grok Video 1.5"
];

// ---------------------------------------------------------------------------
// Exclusions
// ---------------------------------------------------------------------------

/// Catalogue entries we deliberately refuse to route, with the reason. Public so
/// the docs page and the "why isn't X here" answer come from one place.
///
/// **Sora 2 is absent from this list because it is absent from the vendored
/// spec** — the CLI never shipped it. It must stay absent: OpenAI removes the
/// Videos API on 2026-09-24 with an empty replacement field, fal has already
/// delisted it, and the plan's ruling is to omit the surface entirely rather
/// than ship a button that dies on a known date. Higgsfield will keep theirs and
/// look worse for it. If a future vendor refresh adds `sora2_video` or
/// `open_sora_video` to `MODELS.md`, add it here — do not add it as a model.
pub const EXCLUSIONS: [(&str, &str); 2] = [
    (
        "brain_activity",
        "Virality Predictor: claims to simulate the cortical response of 720 \
         modeled brains. Shipping it would cost us more credibility than the \
         feature is worth.",
    ),
    (
        "veo3",
        "Google shut down veo-3.0-generate-001 and veo-2.0-generate-001 on \
         2026-06-30. Higgsfield's veo-3-preview entry is stale or silently \
         remapped; the research verdict is do not implement it.",
    ),
];

// ---------------------------------------------------------------------------
// Launch set
// ---------------------------------------------------------------------------

/// The twelve models M1 ships, as families — several are one picker entry with
/// two or three job types behind it, so the id count is higher than twelve and
/// that is not a bug.
///
/// Veo 3.1 Fast is in the family because the plan's whole Veo price story rests
/// on it ($0.10/s against $0.40/s for the full model), even though Higgsfield's
/// picker exposes only Veo 3.1 and Veo 3.1 Lite.
pub const LAUNCH_FAMILIES: [(&str, &[&str]); 12] = [
    ("Kling 3.0", &["kling3_0"]),
    ("Kling 2.6", &["kling2_6"]),
    (
        "Seedance 2.0",
        &["seedance_2_0", "seedance_2_0_fast", "seedance_2_0_mini"],
    ),
    ("Wan 2.7", &["wan2_7"]),
    ("Wan 2.6", &["wan2_6"]),
    ("Veo 3.1", &["veo3_1", "veo3_1_fast", "veo3_1_lite"]),
    ("Gemini Omni", &["gemini_omni"]),
    // The two Nano Banana ids read backwards and it is not a typo: Higgsfield's
    // `nano_banana_2` is the model they sell as "Nano Banana Pro", and their
    // `nano_banana_flash` is the one they sell as "Nano Banana 2".
    ("Nano Banana Pro", &["nano_banana_2"]),
    ("Nano Banana 2", &["nano_banana_flash"]),
    ("Seedream 5.0 Pro", &["seedream_v5_pro"]),
    ("FLUX.2 Pro", &["flux_2"]),
    ("GPT Image 2", &["gpt_image_2"]),
];

// ---------------------------------------------------------------------------
// Job types
// ---------------------------------------------------------------------------

/// Which [`JobType`] each model's prompt compiles under, grouped by job type.
///
/// [`crate::enhance`] ports Higgsfield's per-job-type prompt-enhance defaults
/// verbatim, but their table is keyed by *job type* while our roster is keyed by
/// *model id*, and nothing joined the two — so before this table existed there
/// was no way to obtain a [`JobType`] from a model the user had actually picked,
/// and the entire enhance subsystem was unreachable from a real generation.
///
/// Shape deliberately mirrors [`LAUNCH_FAMILIES`]: one row per job type listing
/// every model that resolves to it. Grouping this way rather than one row per
/// model is what makes the editorial calls legible — a model sitting in a family
/// it does not belong to is visible at a glance, where a flat 68-row list would
/// hide it.
///
/// It cannot be a bijection: Higgsfield's table has 26 job types and we route 68
/// models. Where the join is an identity the row is silent; where a model had no
/// job type of its own, the row says which one it borrows and why. **The choice
/// only ever changes a default** — [`crate::enhance::decide`] rule 3 hands the
/// prompt back to the user the moment they touch the toggle — so the cost of a
/// debatable row is one wrong initial toggle position, not a wrong generation.
///
/// Eleven job types match no model. That is the roster being smaller than their
/// product, not an omission here:
///
/// - `lipsync`, `photodump` and `fashion-factory` name surfaces `PARITY.md` §2a
///   lists as built-zero, as do `avatar` and `complex-avatar` under
///   Characters/identity. `animate` is image-to-animation; the nearest unbuilt
///   surface is Recast (Wan 2.2 Animate, §2a), though we have not confirmed the
///   two are the same thing. `scene` we cannot place at all — nothing in the
///   corpus we hold says which of their surfaces it belongs to, so it is left
///   unused rather than assigned on a guess.
/// - `image-wan2.2` is a Wan *image* model; the vendored spec lists no Wan model
///   under Image at all.
/// - `image-flux-2-flex` and `image-flux-2-max` are `--variant flex|max` on the
///   one `flux_2` entry, not separate ids.
/// - `image-gpt-image-2-mini` names a GPT Image 2 tier that appears neither in
///   the vendored spec nor in the picker capture, so we have no model for it.
pub const JOB_TYPES: [(&str, &[&str]); 16] = [
    // Editing an existing clip is instruction-following, not cinematic
    // prose-writing, so these belong with `animate` (enhance **off**) rather
    // than with `video` (enhance on). Rewriting "remove the person walking"
    // into a lush camera description would actively destroy the instruction —
    // which is the same reason an end frame disables the enhancer.
    (
        "animate",
        &[
            "ray2_modify",
            "wan_vace",
            "grok_edit_video",
            "grok_extend_video",
        ],
    ),
    // Every text-to-video and image-to-video path, which is exactly what
    // [`JobType::Video`] is defined as. That settles `image2video` — Higgsfield
    // DoP — against the nearby-looking `animate`: DoP is an image-to-video
    // model, and `Video`'s own definition claims image-to-video, so `animate`
    // is some other surface and not the one to reach for here.
    (
        "video",
        &[
            "gemini_omni",
            "grok_video",
            "grok_video_v15",
            "happy_horse_video",
            "image2video",
            "kling-omni-flf",
            "kling-v2-5-turbo",
            "kling2_6",
            "kling3_0",
            "kling3_0_pro",
            "kling3_0_turbo",
            "kling_o3_flf",
            "minimax_h3",
            "minimax_hailuo",
            "seedance1_5",
            "seedance_2_0",
            "seedance_2_0_fast",
            "seedance_2_0_mini",
            "seedance_2_5",
            "seedance_pro",
            "veo3_1",
            "veo3_1_fast",
            "veo3_1_lite",
            // The Wan 2.2 *video* model. Not `image-wan2.2`, which is their
            // separate Wan image surface and is not in this registry.
            "wan2_2_video",
            "wan2_5_video",
            "wan2_6",
            "wan2_7",
            "wan3_0_prime",
        ],
    ),
    // The studio compilers. Three of these earn `builder` outright — the spec
    // gives `cinematic_studio_3_0`, `cinematic_studio_video_3_5` and
    // `cinematic_studio_video_v2` a `multi_prompt`/`multi_shots`/
    // `multi_shot_mode` trio, which is the multi-step shape `builder` names. The
    // rest are grouped by product surface rather than by flag set:
    // `cinematic_studio_2_5` and `cinematic_studio_video` are the single-shot
    // members of the same studio, `marketing_studio_video` composes from
    // `storyboard_id`/`hook_id`/`setting_id` with `marketing_studio_image` as
    // its image half, and `video_explainer` is the member of the explainer pair
    // that plans the narrative (the spec says its sibling `explainer_video`
    // explicitly does not).
    //
    // `marketing_studio_*` could defensibly have gone to `product` instead. The
    // deciding argument is that its prompt is a creative brief to be expanded,
    // where `product`'s is an object description to be obeyed.
    (
        "builder",
        &[
            "cinematic_studio_2_5",
            "cinematic_studio_3_0",
            "cinematic_studio_video",
            "cinematic_studio_video_3_5",
            "cinematic_studio_video_v2",
            "marketing_studio_image",
            "marketing_studio_video",
            "video_explainer",
        ],
    ),
    // `image-styled` is the Characters/Styles image path, so all four Soul
    // surfaces belong here as one family. `soul_location` is the one that could
    // have gone elsewhere — it generates places, and `scene` exists — but
    // `PARITY.md` §6 records Soul's look as coming partly from a proprietary
    // prompt enhancer, and `scene` defaults off, so splitting it out would ship
    // Soul's name with none of its aesthetic.
    //
    // `image_auto`, `grok_image` and `recraft_v4_1` have no job type of their
    // own and land here as the generic prose-image bucket, `image-styled` being
    // the only image type that is not named for one specific checkpoint.
    // `recraft_v4_1` is the weakest fit: its `vector` mode takes literal
    // brief-style prompts that a rewrite would bloat. It still starts on,
    // because its default `standard` raster mode is an ordinary text-to-image.
    (
        "image-styled",
        &[
            "grok_image",
            "image_auto",
            // Qwen Image 3's text-to-image tier: an ordinary prose image model
            // with no surface of its own in the job-type table.
            "qwen_image_3",
            "recraft_v4_1",
            "soul_cast",
            "soul_cinematic",
            "soul_location",
            "text2image_soul_v2",
        ],
    ),
    // `image-flux` is named for FLUX.1 Kontext in the job-type table, so it
    // takes Kontext and not FLUX.2.
    ("image-flux", &["flux_kontext"]),
    // Our catalogue models pro/flex/max as one `--variant` flag on `flux_2`, so
    // the base job type covers all three tiers.
    ("image-flux-2", &["flux_2"]),
    // `image-gpt` is the legacy GPT image surface and `openai_hazel` is the one
    // OpenAI image model in the roster that is not a GPT Image 2 tier. Folding
    // it into `image-gpt-image-2` instead would put a different model with a
    // different parameter surface on the instruction-follower default.
    ("image-gpt", &["openai_hazel"]),
    // GPT Image 2.5's two tiers and their edit paths are the same OpenAI
    // instruction-following surface as GPT Image 2, at a different checkpoint.
    (
        "image-gpt-image-2",
        &[
            "gpt_image_2",
            "gpt_image_2_5_flare",
            "gpt_image_2_5_flare_edit",
            "gpt_image_2_5_sunburst",
            "gpt_image_2_5_sunburst_edit",
        ],
    ),
    ("image-kling-omni", &["kling_omni_image"]),
    ("image-nano-banana", &["nano_banana"]),
    // Keyed by Higgsfield's internal id, not by the name they sell: their
    // `nano_banana_2` is the model marketed as "Nano Banana Pro" (see
    // `LAUNCH_FAMILIES`). The other two have no job type of their own and are
    // sold under the Nano Banana 2 name anyway — `nano_banana_flash` as "Nano
    // Banana 2", `nano_banana_2_lite` as "Nano Banana 2 Lite". Nothing rides on
    // getting that split exactly right today: `image-nano-banana` and
    // `image-nano-banana-2` both default off, so the two are interchangeable
    // until one of them gains a behaviour the other lacks.
    (
        "image-nano-banana-2",
        &[
            "nano_banana_2",
            "nano_banana_2_edit",
            "nano_banana_2_lite",
            "nano_banana_flash",
            "nano_banana_pro_edit",
        ],
    ),
    (
        "image-seedream",
        &[
            "seedream_v4_5",
            "seedream_v5_lite",
            "seedream_v5_pro",
            "seedream_v5_pro_edit",
        ],
    ),
    ("z-image", &["z_image"]),
    // Jobs that take no `--prompt` at all: background removal, outpaint, and the
    // explainer assembler, which is driven entirely by references to
    // already-generated jobs. There is nothing to rewrite, so the only correct
    // default is off, and `reference` is the off-by-default job type that names
    // reference-driven work.
    (
        "reference",
        &[
            "explainer_video",
            "image_background_remover",
            "outpaint",
            "video_background_remover",
        ],
    ),
    // The 3D family has no job type of its own. All five go to `product`, the
    // off-by-default type that names object work. Only `tripo_3d` and
    // `sam_3_3d` take a `--prompt` at all, and it names an object to be built
    // to spec — the atmosphere, lens and lighting language a rewrite adds is
    // meaningless to a mesh. The other three take none, so off is the only
    // defensible default for them regardless.
    (
        "product",
        &[
            "3d_rigging",
            "image_to_3d",
            "multi_image_to_3d",
            "sam_3_3d",
            "tripo_3d",
        ],
    ),
    // `speech` is the only job type whose output is audio. Sound effects
    // (`mirelo_text_to_audio`) and music (`sonilo_music`) are not speech, but
    // their prompt is a literal description of a sound to produce, and sharing
    // the audio type keeps them off the cinematic-prose default that every
    // remaining alternative — all image or video types — would hand them.
    (
        "speech",
        &[
            "inworld_text_to_speech",
            "mirelo_text_to_audio",
            "seed_audio",
            "sonilo_music",
            "text2speech_v2",
        ],
    ),
];

/// The job type a model's prompt compiles under.
///
/// A model the table has never heard of falls back to its modality's group. That
/// path exists for exactly one case — a vendor refresh adding a model to
/// `MODELS.md` — and `debug_assert` names it, because the failure it guards is
/// silent: an unlisted image model would quietly inherit a prose default and
/// start rewriting edit instructions with no visible symptom until the outputs
/// stopped matching the prompt.
fn job_type_for(id: &str, modality: Modality) -> JobType {
    let slug = JOB_TYPES
        .iter()
        .find(|(_, ids)| ids.contains(&id))
        .map(|(slug, _)| *slug);

    debug_assert!(
        slug.is_some(),
        "{id} is not in JOB_TYPES; add it rather than letting the modality \
         fallback choose its enhance default"
    );

    // A test pins every slug in the table to a real variant, so `from_slug`
    // cannot fail here — but resolving through the fallback rather than
    // unwrapping means a typo in the table stays a wrong default instead of
    // becoming a panic in the shell.
    slug.and_then(JobType::from_slug).unwrap_or(match modality {
        Modality::Video => JobType::Video,
        Modality::Image => JobType::ImageStyled,
        Modality::Audio => JobType::Speech,
        Modality::ThreeD => JobType::Product,
        // `Modality::Other` is the vendored spec's "Video explainer jobs"
        // section, which is the builder pipeline.
        Modality::Other => JobType::Builder,
    })
}

// ---------------------------------------------------------------------------
// Routes and prices
// ---------------------------------------------------------------------------

fn per_second(usd: f64) -> CostModel {
    CostModel::PerSecond {
        usd,
        audio_multiplier: 1.0,
    }
}

fn per_second_audio(usd: f64, audio_multiplier: f64) -> CostModel {
    CostModel::PerSecond {
        usd,
        audio_multiplier,
    }
}

/// Video billed by output tokens. `tokens = (h * w * fps * seconds) / 1024`, so
/// the same clip at 1080p is 2.25x its 720p price — the trap that makes
/// per-second arithmetic wrong for the whole Seedance family.
fn per_token(usd_per_million: f64) -> CostModel {
    CostModel::PerToken {
        usd_per_million,
        fps: 24,
    }
}

fn per_image(usd: f64) -> CostModel {
    CostModel::PerImage {
        usd,
        usd_per_extra_input: 0.0,
    }
}

/// A model whose only path is the user's own Higgsfield key.
///
/// Their platform API takes `POST /{model_id}` where `model_id` is a fal-shaped
/// slug, and the only paths the corpus ever saw documented are `soul/*` and
/// `dop/*`. So for the studio compilers, the Soul compositions, the 3D family
/// and the explainer pipelines we know the surface exists in their product and
/// we do not know that an API key reaches it — hence the note. Price is always
/// unknown: those surfaces bill in Higgsfield credits, and a credit is anywhere
/// from $0.030 (Ultra) to $0.075 (Starter) depending on the plan the user is on.
fn higgsfield_only(slug: &str, note: &str) -> Vec<(Route, CostModel)> {
    vec![(
        Route::noted(ProviderId::Higgsfield, slug, note),
        CostModel::Unknown,
    )]
}

/// A *second* route on the user's own Higgsfield key, for a model a third party
/// also serves.
///
/// Distinct from [`higgsfield_only`], which says "your key is the only way in".
/// This one says "your key is another way in", and it exists because their
/// key-pair platform carries ~20 third-party families that the fal-anchored
/// table above already prices — verified against their live `openapi.json` on
/// 2026-08-28. A user paying for a Higgsfield plan was previously forced to
/// spend fal cash on models their subscription already covers.
///
/// Always [`CostModel::Unknown`]: these bill in Higgsfield credits, and a
/// credit is $0.030-$0.075 depending on plan. The resolver therefore sorts
/// these behind every priced route, so adding one never silently moves a
/// generation off the cheapest known path — the user has to pin it.
fn higgsfield_also(slug: &str, note: &str) -> (Route, CostModel) {
    (
        Route::noted(ProviderId::Higgsfield, slug, note),
        CostModel::Unknown,
    )
}

const NO_THIRD_PARTY: &str = "unverified route — no third-party API serves this surface; \
     your own Higgsfield key is the only path and their platform API documents only the \
     soul/ and dop/ model paths";

/// Compositions over Soul, not models in their own right. No standalone API
/// exists for any of them; the substitute (FLUX.2 plus our ported preset
/// prompts) is the preset layer's job, not a route.
const SOUL_ROUTE: &str =
    "the only literal-Soul path; the FLUX.2 substitute is a preset, not a route";

#[allow(clippy::too_many_lines)]
fn priced_routes(id: &str) -> Vec<(Route, CostModel)> {
    match id {
        // -- Kling ----------------------------------------------------------
        "kling3_0" => vec![
            (
                Route::noted(
                    ProviderId::Fal,
                    "fal-ai/kling-video/v3/standard",
                    "standard tier: $0.084/s audio-off, $0.126/s audio-on; pro is $0.112/$0.168 \
                     and 4K a flat $0.42/s",
                ),
                per_second_audio(0.084, 1.5),
            ),
            (
                Route::noted(
                    ProviderId::Vaig,
                    "klingai/kling-v3.0",
                    "exactly 2x fal on every Kling 3.0 tier",
                ),
                per_second_audio(0.168, 1.5),
            ),
        ],
        // The pro tier of the same generation, imported 2026-09-16 from fal's
        // refreshed index. Text-to-video only: fal's `/v3/pro/image-to-video`
        // names its start frame `start_image_url`, which `media::fal_keys` does
        // not speak, so an animate request would be refused at submit. Offering
        // a mode we cannot bind is the dead tile this registry keeps out.
        "kling3_0_pro" => vec![(
            Route::noted(
                ProviderId::Fal,
                "fal-ai/kling-video/v3/pro",
                "pro tier: $0.112/s audio-off, $0.168/s audio-on, $0.196/s with voice control",
            ),
            per_second_audio(0.112, 1.5),
        )],
        "kling3_0_turbo" => vec![(
            Route::new(ProviderId::Fal, "fal-ai/kling-video/v3/turbo"),
            per_second(0.14),
        )],
        "kling2_6" => vec![
            (
                Route::noted(
                    ProviderId::Fal,
                    "fal-ai/kling-video/v2.6/pro",
                    "pro tier: $0.07/s audio-off, $0.14/s audio-on, $0.168/s with voice control",
                ),
                per_second_audio(0.07, 2.0),
            ),
            (
                Route::noted(
                    ProviderId::Vaig,
                    "klingai/kling-v2.6-standard",
                    "standard tier, the cheapest Kling 2.6 anywhere; VAIG publishes no audio-on \
                     rate for it, so audio here is an under-estimate",
                ),
                per_second(0.042),
            ),
        ],

        // -- Seedance -------------------------------------------------------
        //
        // fal publishes both a per-second figure and a token rate for Seedance;
        // the token rate is the real billing shape and reproduces their
        // per-second numbers exactly ($14.00/M x 21,600 tok/s at 720p =
        // $0.3024/s against their quoted $0.3034), so we encode the token rate
        // and let resolution drive the price.
        "seedance_2_0" => vec![
            (
                Route::noted(
                    ProviderId::Fal,
                    "bytedance/seedance-2.0",
                    "$0.3034/s at 720p, $0.682/s at 1080p — the same clip costs 2.25x at 1080p",
                ),
                per_token(14.00),
            ),
            (
                Route::noted(
                    ProviderId::Vaig,
                    "bytedance/seedance-2.0",
                    "half fal's token rate, and drops again to $4.30/M when the job has a video \
                     input",
                ),
                per_token(7.00),
            ),
        ],
        "seedance_2_0_fast" => vec![
            (
                Route::noted(
                    ProviderId::Fal,
                    "bytedance/seedance-2.0/fast",
                    "$0.2419/s at 720p",
                ),
                per_token(11.20),
            ),
            (
                Route::noted(
                    ProviderId::Vaig,
                    "bytedance/seedance-2.0-fast",
                    "$3.30/M with a video input",
                ),
                per_token(5.60),
            ),
        ],
        // Mini is the one Seedance whose two published rates do not sit on a
        // single token line: 720p implies $7.16/M and 480p implies $7.70/M. We
        // quote fal's published 720p per-second rate rather than fit a curve
        // through two points, and carry the 480p figure in the note.
        "seedance_2_0_mini" => vec![(
            Route::noted(
                ProviderId::Fal,
                "bytedance/seedance-2.0/mini",
                "720p rate; fal bills $0.0721/s at 480p",
            ),
            per_second(0.1547),
        )],
        "seedance_pro" => vec![
            (
                Route::noted(
                    ProviderId::Fal,
                    "fal-ai/bytedance/seedance/v1/pro",
                    "$2.50/M output tokens, about $0.62 for a 5s 1080p clip",
                ),
                per_token(2.50),
            ),
            (
                Route::noted(
                    ProviderId::Vaig,
                    "bytedance/seedance-v1.0-pro",
                    "carried, but VAIG publishes a price only for the v1.5 line",
                ),
                CostModel::Unknown,
            ),
            // Tier mismatch, stated rather than hidden: their platform carries
            // only `v1/pro/fast`, so this route is the faster, lower-fidelity
            // sibling of fal's `v1/pro`. Silently substituting it would be the
            // same class of harm as quietly downgrading 4k to 720p.
            higgsfield_also(
                "bytedance/seedance/v1/pro/fast",
                "your own Higgsfield key — note this is their pro/FAST tier, not the pro tier \
                 fal serves; billed in Higgsfield credits, so no USD estimate",
            ),
        ],
        // The largest arbitrage in the roster, so the default route is inverted:
        // VAIG first, fal second.
        "seedance1_5" => vec![
            (
                Route::noted(
                    ProviderId::Vaig,
                    "bytedance/seedance-v1.5-pro",
                    "~10x cheaper than fal: 720p $0.0259/s silent, $0.0518/s with audio; \
                     1080p $0.0583/$0.1166",
                ),
                per_second_audio(0.0259, 2.0),
            ),
            (
                Route::noted(
                    ProviderId::Fal,
                    "fal-ai/bytedance/seedance/v1.5/pro",
                    "$1.20/M output tokens silent; fal doubles to $2.40/M with audio, which this \
                     token model cannot express",
                ),
                per_token(1.20),
            ),
        ],
        // Shipped. fal served `bytedance/seedance-2.5/{text,image}-to-video`
        // on 2026-09-16 and publishes the token rate the family is really
        // billed on, so the placeholder's `Unknown` and its "unverified slug"
        // note are both retired.
        "seedance_2_5" => vec![(
            Route::noted(
                ProviderId::Fal,
                "bytedance/seedance-2.5",
                "$0.0214 per 1,000 output tokens at 480p and 720p, ~$0.0234 at 1080p — the \
                 higher tier is not expressible here, so 1080p is a ~9% under-estimate",
            ),
            per_token(21.40),
        )],

        // -- Kling O-series --------------------------------------------------
        "kling_o3_flf" => vec![(
            Route::noted(
                ProviderId::Fal,
                "fal-ai/kling-video/o3/pro",
                "$0.112/s audio-off, $0.14/s audio-on",
            ),
            per_second_audio(0.112, 0.14 / 0.112),
        )],
        // fal's `pricingInfoOverride` for the whole o1 family is blank. A
        // third-party reseller quotes $0.1111/s; it is unverified and this is a
        // per-second video model, so a wrong number here compounds with
        // duration. Unknown.
        "kling-omni-flf" => vec![(
            Route::noted(
                ProviderId::Fal,
                "fal-ai/kling-video/o1",
                "fal publishes no price for the o1 family",
            ),
            CostModel::Unknown,
        )],
        "kling-v2-5-turbo" => vec![
            (
                Route::new(ProviderId::Fal, "fal-ai/kling-video/v2.5-turbo/pro"),
                per_second(0.07),
            ),
            (
                Route::noted(
                    ProviderId::Vaig,
                    "klingai/kling-v2.5-turbo-standard",
                    "standard tier; VAIG's pro tier matches fal at $0.07/s",
                ),
                per_second(0.042),
            ),
            higgsfield_also(
                "kling-video/v2.5-turbo/pro",
                "your own Higgsfield key, same pro tier fal serves; billed in Higgsfield \
                 credits, so no USD estimate",
            ),
        ],

        // -- Wan -------------------------------------------------------------
        //
        // Wan is the one family where fal and VAIG match to the cent, so the
        // route choice is about concurrency and key ownership, not price.
        "wan2_7" => vec![
            (
                Route::noted(
                    ProviderId::Fal,
                    "fal-ai/wan/v2.7",
                    "720p rate; $0.15/s at 1080p",
                ),
                per_second(0.10),
            ),
            (
                Route::noted(
                    ProviderId::Vaig,
                    "alibaba/wan-v2.7",
                    "identical to fal to the cent",
                ),
                per_second(0.10),
            ),
        ],
        // Alibaba's 2026-09 flagship, imported from fal's refreshed index.
        // Text-to-video only for the same reason as Kling v3 pro: its
        // image-to-video endpoint names the start frame `start_image_url`.
        "wan3_0_prime" => vec![(
            Route::noted(
                ProviderId::Fal,
                "alibaba/wan-3.0-prime",
                "$0.068/s at 480p, $0.14/s at 720p, $0.28/s at 1080p",
            ),
            CostModel::PerSecondTiered {
                tiers: vec![(480, 0.068), (720, 0.14), (1080, 0.28)],
            },
        )],
        "wan2_6" => vec![
            (
                Route::noted(
                    ProviderId::Fal,
                    "fal-ai/wan/v2.6",
                    "720p rate; $0.15/s at 1080p. The /flash endpoint halves both to $0.05/$0.075",
                ),
                per_second(0.10),
            ),
            (
                Route::noted(
                    ProviderId::Vaig,
                    "alibaba/wan-v2.6",
                    "identical to fal to the cent",
                ),
                per_second(0.10),
            ),
        ],
        "wan2_5_video" => vec![
            (
                Route::noted(
                    ProviderId::Fal,
                    "fal-ai/wan-25-preview",
                    "720p rate; $0.05/s at 480p, $0.15/s at 1080p",
                ),
                per_second(0.10),
            ),
            (
                Route::noted(
                    ProviderId::Vaig,
                    "alibaba/wan-v2.5-t2v-preview",
                    "identical to fal to the cent",
                ),
                per_second(0.10),
            ),
            higgsfield_also(
                "wan-25-preview",
                "your own Higgsfield key, the same preview build; billed in Higgsfield \
                 credits, so no USD estimate",
            ),
        ],
        "wan2_2_video" => vec![(
            Route::noted(
                ProviderId::Fal,
                "fal-ai/wan/v2.2-a14b",
                "family root verified from the text-to-image endpoint; the video-to-video \
                 variant is the only route in the roster that edits an existing clip",
            ),
            CostModel::PerSecondTiered {
                // fal publishes this in prose on the model page, verified
                // 2026-08-05: $0.08/s at 720p, $0.06/s at 580p, $0.04/s at 480p.
                tiers: vec![(480, 0.04), (580, 0.06), (720, 0.08)],
            },
        )],

        // -- Video editing ----------------------------------------------------
        //
        // The roster had nothing that edits an existing clip, which is how a
        // user attaching a video to Gemini Omni got an unrelated generation.
        // Both endpoints verified live 2026-08-05 against fal's schema.
        //
        // Prices are Unknown rather than guessed. fal publishes none for either
        // line, and the resolver sorts an unknown price last instead of
        // treating it as free — the estimate reads "price unavailable", which
        // is the honest thing to put next to a Generate button.
        // $0.05/s of output plus a small per-input charge, from fal's prose.
        // Modelled as the per-second part only: the surcharge is a fraction of
        // a cent and inventing a second term would be a guess.
        "grok_edit_video" => vec![(
            Route::noted(
                ProviderId::Fal,
                "xai/grok-imagine-video/edit-video",
                "whole-frame prompt editing; $0.05/s of output at 480p",
            ),
            per_second(0.05),
        )],
        "grok_extend_video" => vec![(
            Route::noted(
                ProviderId::Fal,
                "xai/grok-imagine-video/extend-video",
                "continues a clip; $0.05/s of output at 480p",
            ),
            per_second(0.05),
        )],
        "ray2_modify" => vec![(
            Route::noted(
                ProviderId::Fal,
                "fal-ai/luma-dream-machine/ray-2/modify",
                "the only endpoint on fal built for prompt-driven editing of an existing clip",
            ),
            CostModel::Unknown,
        )],
        "wan_vace" => vec![(
            Route::noted(
                ProviderId::Fal,
                "fal-ai/wan-vace-14b",
                "masked inpainting over video — the nearest thing to Higgsfield's Recast",
            ),
            CostModel::PerSecondTiered {
                // fal publishes this in prose on the model page, verified
                // 2026-08-05: $0.08/s at 720p, $0.06/s at 580p, $0.04/s at 480p.
                tiers: vec![(480, 0.04), (580, 0.06), (720, 0.08)],
            },
        )],

        // -- Veo -------------------------------------------------------------
        //
        // Vertex's `-001` ids are the stable line; the Gemini `-preview` ids
        // churn. Veo bills wall-clock output seconds, so a clip you throw away
        // still costs its full length — there is nothing to model, only to say.
        "veo3_1" => vec![
            (
                Route::noted(
                    ProviderId::Google,
                    "veo-3.1-generate-001",
                    "$0.40/s at both 720p and 1080p, $0.60/s at 4K. personGeneration is \
                     allowlist-gated per Google Cloud project and we cannot grant it",
                ),
                per_second(0.40),
            ),
            (
                Route::noted(
                    ProviderId::Fal,
                    "fal-ai/veo3.1",
                    "$0.20/s silent, $0.40/s with audio — half Google direct for a silent clip",
                ),
                per_second_audio(0.20, 2.0),
            ),
            (
                Route::new(ProviderId::Vaig, "google/veo-3.1-generate-001"),
                per_second_audio(0.20, 2.0),
            ),
            higgsfield_also(
                "veo3.1",
                "your own Higgsfield key; billed in Higgsfield credits, so no USD estimate",
            ),
        ],
        "veo3_1_fast" => vec![
            (
                Route::noted(
                    ProviderId::Google,
                    "veo-3.1-fast-generate-001",
                    "$0.10/s at 720p, $0.12/s at 1080p, $0.30/s at 4K",
                ),
                per_second(0.10),
            ),
            (
                Route::noted(
                    ProviderId::Fal,
                    "fal-ai/veo3.1/fast",
                    "carried, but fal publishes a price only for the lite tier",
                ),
                CostModel::Unknown,
            ),
            higgsfield_also(
                "veo3.1/fast",
                "your own Higgsfield key; billed in Higgsfield credits, so no USD estimate",
            ),
        ],
        "veo3_1_lite" => vec![
            (
                Route::noted(
                    ProviderId::Google,
                    "veo-3.1-lite-generate-preview",
                    "the cheapest name-brand video in the roster: $0.05/s at 720p, $0.08/s at 1080p",
                ),
                per_second(0.05),
            ),
            (
                Route::noted(
                    ProviderId::Fal,
                    "fal-ai/veo3.1/lite",
                    "$0.03/s silent at 720p, $0.05/s with audio",
                ),
                per_second_audio(0.03, 0.05 / 0.03),
            ),
        ],

        // -- Everything else, video -----------------------------------------
        "gemini_omni" => vec![
            (
                Route::noted(
                    ProviderId::Google,
                    "gemini-omni-flash",
                    "$17.50/M video-output tokens, quoted as $0.10/s",
                ),
                per_second(0.10),
            ),
            (
                Route::noted(
                    ProviderId::Fal,
                    "google/gemini-omni-flash",
                    "$21.875/M output tokens, about $0.125/s at 720p",
                ),
                per_second(0.125),
            ),
        ],
        // The xAI-direct route is gone, and with it the plan's "3x cheaper
        // direct from xAI" claim. **xAI publishes no generation API at all** —
        // probed 2026-08-05, `/v1/image/generations` and `/v1/video/generations`
        // both 404 and the reference documents only chat and responses. The
        // route and its confident $0.080/s were transcribed from a corpus
        // claim that was never true, and an unreachable route quoting the
        // cheapest price is worse than no route: the resolver preferred it.
        "grok_video_v15" => vec![
            (
                Route::noted(
                    ProviderId::Fal,
                    "xai/grok-imagine-video/v1.5",
                    "fal publishes 480p $0.08/s, 720p $0.14/s — verified from the catalogue",
                ),
                CostModel::PerSecondTiered {
                    tiers: vec![(480, 0.08), (720, 0.14)],
                },
            ),
            (
                Route::noted(
                    ProviderId::Vaig,
                    "xai/grok-imagine-video-1.5",
                    "identical to fal",
                ),
                per_second(0.14),
            ),
        ],
        "minimax_h3" => vec![
            (
                Route::noted(
                    ProviderId::Vaig,
                    "minimax/minimax-h3",
                    "$0.13/s at 2K — exactly half fal",
                ),
                per_second(0.13),
            ),
            (Route::new(ProviderId::Fal, "minimax/h3"), per_second(0.26)),
        ],
        // The one model here whose fal route does not exist: `fal-ai/minimax/
        // hailuo-2.3` is in `media::FAL_MISSING_ROUTES`, measured absent
        // 2026-08-05. Until now that left the model with no reachable route at
        // all — the Higgsfield path is the first one that can actually run it.
        "minimax_hailuo" => vec![
            (
                Route::noted(
                    ProviderId::Fal,
                    "fal-ai/minimax/hailuo-2.3",
                    "pro tier bills a flat rate per video, not per second",
                ),
                CostModel::Flat { usd: 0.49 },
            ),
            higgsfield_also(
                "minimax/hailuo-2.3/pro",
                "your own Higgsfield key — the only route that reaches this model, since fal \
                 is measured not to serve it; billed in Higgsfield credits, so no USD estimate",
            ),
        ],
        "happy_horse_video" => higgsfield_only(
            "happy_horse_video",
            "unverified route — the corpus identifies the vendor as Alibaba but never \
             found the slug on fal or VAIG, and no price at all",
        ),

        // -- Higgsfield's own models -----------------------------------------
        //
        // DoP is in-house with no third-party API, so it is deliberately absent
        // from the fal-anchored table above. It still belongs in the registry:
        // a user who brings their own Higgsfield key can reach it, and pretending
        // it does not exist would leave a hole where their picker has a model.
        // Their credit rate spans $0.030-$0.075 depending on plan, so no USD.
        "image2video" => vec![
            (
                Route::noted(
                    ProviderId::Higgsfield,
                    "higgsfield-ai/dop/standard",
                    "in-house model; your own Higgsfield key is the only route",
                ),
                CostModel::Unknown,
            ),
            (
                Route::noted(
                    ProviderId::Higgsfield,
                    // `dop/preview` was a 404: their spec publishes lite,
                    // standard and turbo, and has no preview tier. Corrected
                    // against the live openapi.json 2026-08-28.
                    "higgsfield-ai/dop/lite",
                    "the cheaper DoP tier, billed in Higgsfield credits",
                ),
                CostModel::Unknown,
            ),
        ],
        "text2image_soul_v2" | "soul_cast" | "soul_cinematic" | "soul_location" => {
            higgsfield_only("higgsfield-ai/soul/standard", SOUL_ROUTE)
        }

        // -- Image ------------------------------------------------------------
        "flux_2" => vec![
            (
                Route::noted(
                    ProviderId::Bfl,
                    "flux-2-pro",
                    "$0.03 for the first megapixel, $0.015 for each after; [max] is $0.07+$0.03 \
                     and [flex] a flat $0.05/MP",
                ),
                CostModel::PerMegapixel {
                    usd: 0.015,
                    first_usd: Some(0.03),
                },
            ),
            (
                Route::noted(ProviderId::Fal, "fal-ai/flux-2-pro", "matches BFL exactly"),
                CostModel::PerMegapixel {
                    usd: 0.015,
                    first_usd: Some(0.03),
                },
            ),
            (
                Route::noted(ProviderId::Vaig, "bfl/flux-2-pro", "VAIG exposes no price"),
                CostModel::Unknown,
            ),
        ],
        "flux_kontext" => vec![
            (
                Route::noted(ProviderId::Bfl, "flux-kontext-pro", "BFL exposes no price"),
                CostModel::Unknown,
            ),
            (
                Route::noted(
                    ProviderId::Fal,
                    "fal-ai/flux-pro/kontext",
                    "fal's pricing field is blank for the whole Kontext family",
                ),
                CostModel::Unknown,
            ),
            (
                Route::noted(
                    ProviderId::Vaig,
                    "bfl/flux-kontext-pro",
                    "the only Kontext route with a published price; [max] is $0.08",
                ),
                per_image(0.04),
            ),
        ],
        // OpenAI, fal and VAIG charge the same to the cent, so there is nothing
        // to arbitrage — but there is also no per-image number to quote. It is
        // billed on output tokens ($30/M) and the `quality` flag swings the
        // result from $0.006 to $0.211. A "cost" of either end would be a lie,
        // and Billable has no token count to compute from.
        "gpt_image_2" => vec![
            (
                Route::noted(
                    ProviderId::OpenAi,
                    "gpt-image-2",
                    "$30/M output tokens; $0.006-$0.211 per image depending on quality and size",
                ),
                CostModel::Unknown,
            ),
            (
                Route::noted(ProviderId::Fal, "openai/gpt-image-2", "identical to OpenAI direct"),
                CostModel::Unknown,
            ),
            (
                Route::noted(ProviderId::Vaig, "openai/gpt-image-2", "identical to OpenAI direct"),
                CostModel::Unknown,
            ),
        ],
        // GPT Image 2.5's two tiers. fal bills them on tokens exactly as it
        // bills GPT Image 2 — $30/M image-output tokens, swung by `quality` —
        // and publishes no per-image figure, so the same refusal applies: a
        // number here would be invented, and Billable has no token count.
        "gpt_image_2_5_flare" => vec![(
            Route::noted(
                ProviderId::Fal,
                "openai/gpt-image-2.5/flare/text-to-image",
                "$30/M image-output tokens; the `quality` setting moves the per-image cost by \
                 more than an order of magnitude, so no single figure is honest",
            ),
            CostModel::Unknown,
        )],
        "gpt_image_2_5_flare_edit" => vec![(
            Route::noted(
                ProviderId::Fal,
                "openai/gpt-image-2.5/flare/edit",
                "same token billing as the text-to-image tier, plus the input image's tokens",
            ),
            CostModel::Unknown,
        )],
        "gpt_image_2_5_sunburst" => vec![(
            Route::noted(
                ProviderId::Fal,
                "openai/gpt-image-2.5/sunburst/text-to-image",
                "$30/M image-output tokens, priced identically to the flare tier",
            ),
            CostModel::Unknown,
        )],
        "gpt_image_2_5_sunburst_edit" => vec![(
            Route::noted(
                ProviderId::Fal,
                "openai/gpt-image-2.5/sunburst/edit",
                "same token billing as the text-to-image tier, plus the input image's tokens",
            ),
            CostModel::Unknown,
        )],
        "qwen_image_3" => vec![(
            Route::noted(
                ProviderId::Fal,
                "alibaba/qwen-image-3/text-to-image",
                "$0.04 per image at 1K, $0.075 at 2K",
            ),
            per_image(0.04),
        )],
        // Same correction as the video line: xAI serves no image generation
        // API either. fal does, and publishes the price.
        "grok_image" => vec![(
            Route::noted(
                ProviderId::Fal,
                "xai/grok-imagine-image",
                "xAI serves no public image API; fal is the only path",
            ),
            CostModel::Unknown,
        )],
        "nano_banana" => vec![(
            Route::noted(
                ProviderId::Google,
                "gemini-2.5-flash-image",
                "the original Nano Banana, $0.039 per 1K image",
            ),
            per_image(0.039),
        )],
        "nano_banana_2" => vec![
            (
                Route::noted(
                    ProviderId::Google,
                    "gemini-3-pro-image",
                    "direct is the recommended route but Google publishes no per-image price for \
                     it; the gateways do",
                ),
                CostModel::Unknown,
            ),
            (
                Route::noted(
                    ProviderId::Vaig,
                    "google/gemini-3-pro-image",
                    "$0.1344 at 1K and 2K, $0.24 at 4K",
                ),
                per_image(0.1344),
            ),
            (
                Route::noted(
                    ProviderId::Fal,
                    "fal-ai/nano-banana-pro",
                    "4K doubles, and web-search grounding adds $0.015 per image which we do not \
                     model",
                ),
                per_image(0.15),
            ),
        ],
        "nano_banana_flash" => vec![
            (
                Route::noted(
                    ProviderId::Google,
                    "gemini-3.1-flash-image",
                    "$0.067 per 1K image",
                ),
                per_image(0.067),
            ),
            (
                Route::noted(
                    ProviderId::Vaig,
                    "google/gemini-3.1-flash-image",
                    "$0.045 at 512px, $0.067 at 1K, $0.101 at 2K, $0.151 at 4K",
                ),
                per_image(0.067),
            ),
            (
                Route::noted(
                    ProviderId::Fal,
                    "fal-ai/nano-banana-2",
                    "2K is 1.5x, 4K is 2x, 0.5K is 0.75x",
                ),
                per_image(0.08),
            ),
        ],
        // The edit paths. Separate ids on purpose: `fal-ai/nano-banana-pro`
        // and `fal-ai/nano-banana-2` take no attachments at all
        // (`media::FAL_NO_MEDIA`), and `…/edit` is a different endpoint that
        // does. Conflating them is how a reference image gets billed for and
        // ignored.
        "nano_banana_pro_edit" => vec![(
            Route::noted(
                ProviderId::Fal,
                "fal-ai/nano-banana-pro/edit",
                "$0.15 per image; 4K doubles, and web-search grounding adds $0.015 which we do \
                 not model",
            ),
            per_image(0.15),
        )],
        "nano_banana_2_edit" => vec![(
            Route::noted(
                ProviderId::Fal,
                "fal-ai/nano-banana-2/edit",
                "$0.08 per image at 1K; 2K is 1.5x, 4K is 2x and 0.5K is 0.75x",
            ),
            per_image(0.08),
        )],
        "nano_banana_2_lite" => vec![
            (
                Route::noted(
                    ProviderId::Google,
                    "gemini-3.1-flash-lite-image",
                    "$0.0336 per 1K image",
                ),
                per_image(0.0336),
            ),
            (
                Route::new(ProviderId::Vaig, "google/gemini-3.1-flash-lite-image"),
                per_image(0.034),
            ),
            (
                Route::noted(
                    ProviderId::Fal,
                    "google/nano-banana-2-lite",
                    "billed at $37.50/M image-output tokens, which we cannot convert without a \
                     token count",
                ),
                CostModel::Unknown,
            ),
        ],
        "seedream_v5_pro" => vec![
            (
                Route::noted(
                    ProviderId::Vaig,
                    "bytedance/seedream-5.0-pro",
                    "$0.035 flat — 1.9x cheaper than fal",
                ),
                per_image(0.035),
            ),
            (
                Route::noted(
                    ProviderId::Fal,
                    "bytedance/seedream/v5/pro",
                    "$0.0675 up to 1536x1536 and $0.135 above it; edits add $0.0045 per input \
                     image after the first",
                ),
                CostModel::PerImage {
                    usd: 0.0675,
                    usd_per_extra_input: 0.0045,
                },
            ),
        ],
        // Seedream 5 Pro's editing endpoint. `bytedance/seedream/v5/pro` is in
        // `media::FAL_NO_MEDIA`; this is the path that takes the images.
        "seedream_v5_pro_edit" => vec![(
            Route::noted(
                ProviderId::Fal,
                "bytedance/seedream/v5/pro/edit",
                "$0.0675 per output image up to 1536x1536 and $0.135 above it; every input image \
                 after the first adds $0.0045",
            ),
            CostModel::PerImage {
                usd: 0.0675,
                usd_per_extra_input: 0.0045,
            },
        )],
        "seedream_v5_lite" => vec![
            (
                Route::noted(ProviderId::Vaig, "bytedance/seedream-5.0-lite", "$0.035 flat"),
                per_image(0.035),
            ),
            (
                Route::noted(
                    ProviderId::Fal,
                    "bytedance/seedream/v5/lite",
                    "fal's pricing field is blank for the lite line",
                ),
                CostModel::Unknown,
            ),
        ],
        "recraft_v4_1" => vec![
            (
                Route::noted(
                    ProviderId::Recraft,
                    "recraft-v4.1",
                    "$0.035 raster, $0.21 pro raster, $0.08 vector, $0.30 pro vector. Recraft \
                     sells prepaid unit packs that are non-refundable and non-cancellable — say \
                     so before the user buys",
                ),
                per_image(0.035),
            ),
            (
                Route::noted(ProviderId::Vaig, "recraft/recraft-v4.1", "matches Recraft exactly"),
                per_image(0.035),
            ),
            (
                Route::noted(
                    ProviderId::Fal,
                    "fal-ai/recraft/v4.1/text-to-image",
                    "fal's pricing field is blank for Recraft",
                ),
                CostModel::Unknown,
            ),
        ],
        "image_background_remover" => vec![(
            Route::noted(
                ProviderId::Recraft,
                "recraft-v4.1-utility",
                "Recraft prices background removal at $0.01",
            ),
            per_image(0.01),
        )],
        "outpaint" => vec![(
            Route::noted(
                ProviderId::Bfl,
                "flux-2-pro/outpaint",
                "unverified slug — BFL prices Outpaint [high] at $0.10/MP and [fast] at \
                 $0.045 + $0.006/MP",
            ),
            CostModel::PerMegapixel {
                usd: 0.10,
                first_usd: None,
            },
        )],
        // Open weights, so a detected local ComfyUI serves it for nothing. That
        // is the one genuinely free tier in the roster and only a native app can
        // offer it.
        "z_image" => vec![(
            Route::noted(
                ProviderId::Local,
                "z-image",
                "open weights: free on a detected local endpoint",
            ),
            CostModel::Flat { usd: 0.0 },
        )],

        // -- Audio -------------------------------------------------------------
        "seed_audio" => vec![(
            Route::new(ProviderId::Fal, "bytedance/seed-audio-1.0"),
            per_second(0.003125),
        )],
        "mirelo_text_to_audio" => vec![(
            Route::noted(
                ProviderId::Fal,
                "mirelo-ai/sfx1.6/text-to-audio",
                "approximate: the corpus records ~$0.002/s and no exact rate",
            ),
            per_second(0.002),
        )],
        "sonilo_music" => vec![(
            Route::new(ProviderId::Fal, "sonilo/v1.1/text-to-music"),
            per_second(0.0025),
        )],
        "inworld_text_to_speech" => vec![(
            Route::noted(
                ProviderId::Fal,
                "fal-ai/inworld-tts",
                "billed at $0.01 per 1,000 characters, and Billable has no character count",
            ),
            CostModel::Unknown,
        )],

        // -- No third-party equivalent ----------------------------------------
        other => higgsfield_only(other, NO_THIRD_PARTY),
    }
}

// ---------------------------------------------------------------------------
// Hand-authored specs
// ---------------------------------------------------------------------------

/// The models no vendored document describes, in two groups.
///
/// **From Higgsfield's live picker**, verified against the create-page DOM
/// capture of 2026-08-02 — twelve models their own `MODELS.md` never listed:
/// Seedance 2.5, Seedance 2.0 Fast, Seedance Pro, Kling 3.0 Omni, Kling 2.5,
/// Kling O1, HappyHorse, MiniMax H3, Wan 2.5, Wan 2.2, Seedream 5.0 Pro and
/// Higgsfield DoP. Sora 2 is the thirteenth entry in that picker and is
/// deliberately not here — see [`EXCLUSIONS`].
///
/// Flags come from the verified `toProvider` wire-param tables, not from
/// guesswork. Where the corpus records a param but not its domain, the flag is
/// free-form rather than a fabricated enum: a wrong enum silently rejects valid
/// input, which is worse than no enum at all. `--prompt` is added to every
/// prompt-driven model even where the wire table omits it, because the picker's
/// prompt box is model-independent.
///
/// `base` supplies flags for the variants that are a *mode* of a model the
/// vendored spec already documents, so they cannot drift apart.
///
/// **From fal's own index**, imported 2026-09-16 by `examples/fal_diff` and
/// probed by `examples/audit_fal` before landing: Kling 3.0 Pro, Wan 3.0 Prime,
/// LTX 2.5 Pro and its animate half, Veo 3.1 Extend, GPT Image 2.5 in both
/// tiers with their edit paths, Qwen Image 3, the Nano Banana and Seedream 5
/// edit endpoints. Their flags are transcribed from fal's published schema per
/// endpoint — the only document that describes the API we actually call — and
/// every enumerated option set is copied from it verbatim.
fn picker_only_specs(base: &BTreeMap<String, ModelSpec>) -> Vec<ModelSpec> {
    let mut out = Vec::new();

    // Transcribed from fal's own schema for `bytedance/seedance-2.5/*` on
    // 2026-09-16, replacing the placeholder that carried nothing but a prompt.
    // The duration list is fal's, copied value for value: it is the only
    // document that knows which lengths the endpoint takes, and 30 is on it —
    // this is the model that makes a native half-minute clip possible.
    out.push(ModelSpec {
        constraints: vec![
            "fal serves 4-30s; 30s is the native maximum. `auto` lets the model choose."
                .to_string(),
        ],
        ..spec(
            "seedance_2_5",
            "Seedance 2.5",
            Modality::Video,
            &[
                ("prompt", true, ValueSpec::Text),
                ("start_image", false, ValueSpec::Media),
                ("end_image", false, ValueSpec::Media),
                (
                    "duration",
                    false,
                    en(&[
                        "auto", "4", "5", "6", "7", "8", "9", "10", "11", "12", "13", "14", "15",
                        "16", "17", "18", "19", "20", "21", "22", "23", "24", "25", "26", "27",
                        "28", "29", "30",
                    ]),
                ),
                ("resolution", false, en(&["480p", "720p", "1080p"])),
                (
                    "aspect_ratio",
                    false,
                    en(&["auto", "21:9", "16:9", "4:3", "1:1", "3:4", "9:16"]),
                ),
                ("generate_audio", false, ValueSpec::Boolean),
                ("bitrate_mode", false, ValueSpec::Text),
            ],
        )
    });

    if let Some(b) = base.get("seedance_2_0") {
        out.push(derived(
            b,
            "seedance_2_0_fast",
            "Seedance 2.0 Fast",
            "Higgsfield drives this as mode=fast on their seedance_2_0 adapter, so it takes that \
             model's parameters exactly.",
        ));
    }

    out.push(spec(
        "seedance_pro",
        "Seedance Pro",
        Modality::Video,
        &[
            ("prompt", true, ValueSpec::Text),
            ("model", false, ValueSpec::Text),
            ("resolution", false, ValueSpec::Text),
            ("duration", false, ValueSpec::Number),
            ("camera_fixed", false, ValueSpec::Boolean),
            ("input_image", false, ValueSpec::Media),
            ("enhance_prompt", false, ValueSpec::Boolean),
            ("seed", false, ValueSpec::Integer),
            ("width", false, ValueSpec::Integer),
            ("height", false, ValueSpec::Integer),
        ],
    ));

    out.push(spec(
        "kling_o3_flf",
        "Kling 3.0 Omni",
        Modality::Video,
        &[
            ("prompt", true, ValueSpec::Text),
            ("aspect_ratio", false, ValueSpec::Text),
            ("duration", false, ValueSpec::Number),
            ("mode", false, ValueSpec::Text),
            ("sound", false, ValueSpec::Boolean),
            ("model", false, ValueSpec::Text),
            ("width", false, ValueSpec::Integer),
            ("height", false, ValueSpec::Integer),
        ],
    ));

    out.push(ModelSpec {
        constraints: vec![
            "Higgsfield routes Kling 2.5 through their legacy `kling` adapter, so its parameters \
             are that adapter's, not Kling 3.0's."
                .to_string(),
        ],
        ..spec(
            "kling-v2-5-turbo",
            "Kling 2.5",
            Modality::Video,
            &[
                ("prompt", true, ValueSpec::Text),
                ("input_image", false, ValueSpec::Media),
                ("input_image_end", false, ValueSpec::Media),
                ("model", false, ValueSpec::Text),
                ("resolution", false, ValueSpec::Text),
                ("camera_control", false, ValueSpec::Text),
                ("duration", false, ValueSpec::Number),
                ("aspect_ratio", false, ValueSpec::Text),
                ("mode", false, ValueSpec::Text),
                ("enhance_prompt", false, ValueSpec::Boolean),
            ],
        )
    });

    out.push(ModelSpec {
        constraints: vec![
            "The `flf` suffix is first-last-frame: this is Higgsfield's start-and-end-frame \
             reference model."
                .to_string(),
        ],
        ..spec(
            "kling-omni-flf",
            "Kling O1",
            Modality::Video,
            &[
                ("prompt", true, ValueSpec::Text),
                ("duration", false, ValueSpec::Number),
                ("aspect_ratio", false, ValueSpec::Text),
                ("mode", false, ValueSpec::Text),
                ("model", false, ValueSpec::Text),
                ("input_image", false, ValueSpec::Media),
                ("input_image_end", false, ValueSpec::Media),
            ],
        )
    });

    out.push(spec(
        "happy_horse_video",
        "HappyHorse",
        Modality::Video,
        &[
            ("prompt", true, ValueSpec::Text),
            ("resolution", false, ValueSpec::Text),
            ("aspect_ratio", false, ValueSpec::Text),
            ("duration", false, ValueSpec::Number),
            ("batch_size", false, ValueSpec::Integer),
            ("seed", false, ValueSpec::Integer),
            ("width", false, ValueSpec::Integer),
            ("height", false, ValueSpec::Integer),
        ],
    ));

    out.push(spec(
        "minimax_h3",
        "MiniMax H3",
        Modality::Video,
        &[
            ("prompt", true, ValueSpec::Text),
            ("duration", false, ValueSpec::Number),
            ("aspect_ratio", false, ValueSpec::Text),
            ("resolution", false, ValueSpec::Text),
            ("width", false, ValueSpec::Integer),
            ("height", false, ValueSpec::Integer),
        ],
    ));

    out.push(spec(
        "wan2_5_video",
        "Wan 2.5",
        Modality::Video,
        &[
            ("prompt", true, ValueSpec::Text),
            ("input_image", false, ValueSpec::Media),
            ("draw_input_image", false, ValueSpec::Media),
            ("is_draw", false, ValueSpec::Boolean),
            ("mode", false, ValueSpec::Text),
            ("resolution", false, ValueSpec::Text),
            ("duration", false, ValueSpec::Number),
            ("seed", false, ValueSpec::Integer),
            ("motion_id", false, ValueSpec::Text),
            ("enhance_prompt", false, ValueSpec::Boolean),
            ("aspect_ratio", false, ValueSpec::Text),
            ("width", false, ValueSpec::Integer),
            ("height", false, ValueSpec::Integer),
        ],
    ));

    out.push(spec(
        "wan2_2_video",
        "Wan 2.2",
        Modality::Video,
        &[
            ("prompt", true, ValueSpec::Text),
            ("input_image", false, ValueSpec::Media),
            ("model", false, ValueSpec::Text),
            ("frames", false, ValueSpec::Integer),
            ("steps", false, ValueSpec::Integer),
            ("seed", false, ValueSpec::Integer),
            ("motion_id", false, ValueSpec::Text),
            ("enhance_prompt", false, ValueSpec::Boolean),
            ("width", false, ValueSpec::Integer),
            ("height", false, ValueSpec::Integer),
        ],
    ));

    // Their live cost table prices `seedream_v5_pro` per image and their mobile
    // routes link straight to it, so it is unambiguously shipped — the CLI spec
    // documents only the lite and 4.5 tiers.
    if let Some(b) = base.get("seedream_v5_lite") {
        out.push(derived(
            b,
            "seedream_v5_pro",
            "Seedream 5.0 Pro",
            "Parameters taken from the v5 lite tier, the nearest documented sibling.",
        ));
    }

    out.push(ModelSpec {
        constraints: vec![
            "Higgsfield's own image-animation model. `image2video` is genuinely their job type \
             for it, confusing as that reads next to the generic input mode of the same name."
                .to_string(),
        ],
        ..spec(
            "image2video",
            "Higgsfield DoP",
            Modality::Video,
            &[
                ("prompt", true, ValueSpec::Text),
                ("input_image", false, ValueSpec::Media),
                ("input_image_end", false, ValueSpec::Media),
                ("input_audio", false, ValueSpec::Media),
                ("input_video", false, ValueSpec::Media),
                ("model", false, ValueSpec::Text),
                ("frames", false, ValueSpec::Integer),
                ("guide_scale", false, ValueSpec::Number),
                ("sample_shift", false, ValueSpec::Number),
                ("strength", false, ValueSpec::Number),
                ("steps", false, ValueSpec::Integer),
                ("motion_id", false, ValueSpec::Text),
                ("enhance_prompt", false, ValueSpec::Boolean),
                ("seed", false, ValueSpec::Integer),
                ("width", false, ValueSpec::Integer),
                ("height", false, ValueSpec::Integer),
            ],
        )
    });

    // Not in their picker: Higgsfield exposes only Veo 3.1 and Veo 3.1 Lite as
    // job types. We carry Fast anyway because it is the tier that wins the Veo
    // price argument outright, and M1 scopes it by name.
    if let Some(b) = base.get("veo3_1") {
        out.push(derived(
            b,
            "veo3_1_fast",
            "Veo 3.1 Fast",
            "Not a Higgsfield job type: a Google tier they do not surface, selected on their \
             veo3_1 adapter by the `model` field.",
        ));
    }

    // ── Video editing ──────────────────────────────────────────────────────
    //
    // Added 2026-08-05 after a user attached a clip, asked for it to be edited,
    // and got an unrelated generation. The roster had **no model that edits an
    // existing video** — the catalogue is Higgsfield's list, and their editing
    // surfaces run on their own backend, so nothing in it filled the gap.
    //
    // Every flag below is transcribed from fal's published schema for the
    // endpoint, not from the catalogue. That is the point: these are fal
    // models, so fal is the only authority on them.

    // Grok Imagine's two video editors, from fal's schema. Added 2026-08-05
    // after finding that the roster routed Grok through a Higgsfield-only path
    // and an xAI endpoint that does not exist, while fal serves the whole
    // suite — including these, which nothing in the roster could do.
    out.push(ModelSpec {
        constraints: vec![
            "Edits the clip in place from a prompt. Whole-frame, not masked: it \
             restyles what is there rather than removing part of it."
                .to_string(),
        ],
        ..spec(
            "grok_edit_video",
            "Grok Imagine Edit",
            Modality::Video,
            &[
                ("prompt", true, ValueSpec::Text),
                ("video", true, ValueSpec::Media),
                (
                    "resolution",
                    false,
                    ValueSpec::Enum(
                        ["auto", "480p", "720p"]
                            .iter()
                            .map(|s| s.to_string())
                            .collect(),
                    ),
                ),
            ],
        )
    });

    out.push(ModelSpec {
        constraints: vec![
            "Continues an existing clip rather than changing it. `duration` is \
             the length to add, not the total."
                .to_string(),
        ],
        ..spec(
            "grok_extend_video",
            "Grok Imagine Extend",
            Modality::Video,
            &[
                ("prompt", true, ValueSpec::Text),
                ("video", true, ValueSpec::Media),
                ("duration", false, ValueSpec::Integer),
            ],
        )
    });

    out.push(ModelSpec {
        constraints: vec![
            "`mode` trades fidelity to the source against freedom to reinterpret: \
             adhere_1 stays closest to the original, reimagine_3 departs furthest."
                .to_string(),
        ],
        ..spec(
            "ray2_modify",
            "Ray 2 Modify",
            Modality::Video,
            &[
                ("video", true, ValueSpec::Media),
                ("prompt", false, ValueSpec::Text),
                ("image", false, ValueSpec::Media),
                (
                    "mode",
                    false,
                    ValueSpec::Enum(
                        [
                            "adhere_1",
                            "adhere_2",
                            "adhere_3",
                            "flex_1",
                            "flex_2",
                            "flex_3",
                            "reimagine_1",
                            "reimagine_2",
                            "reimagine_3",
                        ]
                        .iter()
                        .map(|s| s.to_string())
                        .collect(),
                    ),
                ),
            ],
        )
    });

    out.push(ModelSpec {
        constraints: vec![
            "`task` selects the operation. `inpainting` with a mask is the one that \
             removes or replaces part of a clip rather than restyling all of it — the \
             closest thing in the roster to Higgsfield's Recast."
                .to_string(),
            "Masked tasks need mask_video_url (or mask_image_url for a still mask); \
             without one, inpainting has nothing to act on."
                .to_string(),
        ],
        ..spec(
            "wan_vace",
            "Wan VACE 14B",
            Modality::Video,
            &[
                ("prompt", true, ValueSpec::Text),
                ("video", false, ValueSpec::Media),
                ("video_references", false, ValueSpec::Media),
                ("image_references", false, ValueSpec::Media),
                ("start_image", false, ValueSpec::Media),
                ("end_image", false, ValueSpec::Media),
                (
                    "task",
                    false,
                    ValueSpec::Enum(
                        ["depth", "pose", "inpainting", "outpainting", "reframe"]
                            .iter()
                            .map(|s| s.to_string())
                            .collect(),
                    ),
                ),
                (
                    "resolution",
                    false,
                    ValueSpec::Enum(
                        ["auto", "240p", "360p", "480p", "580p", "720p"]
                            .iter()
                            .map(|s| s.to_string())
                            .collect(),
                    ),
                ),
                (
                    "aspect_ratio",
                    false,
                    ValueSpec::Enum(
                        ["auto", "16:9", "1:1", "9:16"]
                            .iter()
                            .map(|s| s.to_string())
                            .collect(),
                    ),
                ),
                ("num_frames", false, ValueSpec::Integer),
                ("guidance_scale", false, ValueSpec::Number),
                ("num_inference_steps", false, ValueSpec::Integer),
            ],
        )
    });

    // ── fal's 2026-09 frontier ─────────────────────────────────────────────
    //
    // Imported from the refreshed index (`examples/fal_diff`) on 2026-09-16 and
    // transcribed flag by flag from each endpoint's own OpenAPI, not from the
    // vendored catalogue, which describes a different provider's API and has
    // never heard of any of them. Every option set below is fal's own; every
    // slug below was probed by `examples/audit_fal` before it landed.
    //
    // Two families are deliberately narrower than fal's index. Kling v3 pro and
    // Wan 3.0 prime serve image-to-video, but under a `start_image_url` key
    // that `media::fal_keys` does not speak, so the attachment would be refused
    // at submit; until the binder learns that spelling, these two are
    // text-to-video models here and `media::FAL_ROUTE_MODES` says so.

    out.push(ModelSpec {
        constraints: vec![
            "Voice control is a third audio tier fal bills at $0.196/s; the app does not \
             drive it, so an audio-on estimate is the $0.168/s figure."
                .to_string(),
        ],
        ..spec(
            "kling3_0_pro",
            "Kling 3.0 Pro",
            Modality::Video,
            &[
                ("prompt", true, ValueSpec::Text),
                (
                    "duration",
                    false,
                    en(&[
                        "3", "4", "5", "6", "7", "8", "9", "10", "11", "12", "13", "14", "15",
                    ]),
                ),
                ("aspect_ratio", false, en(&["16:9", "9:16", "1:1"])),
                ("generate_audio", false, ValueSpec::Boolean),
                ("negative_prompt", false, ValueSpec::Text),
                ("cfg_scale", false, ValueSpec::Number),
                ("shot_type", false, ValueSpec::Text),
            ],
        )
    });

    out.push(ModelSpec {
        constraints: vec![
            "fal declares `duration` as a plain integer with no published ceiling, so the app \
             offers it free-form and lets the endpoint judge."
                .to_string(),
        ],
        ..spec(
            "wan3_0_prime",
            "Wan 3.0 Prime",
            Modality::Video,
            &[
                ("prompt", true, ValueSpec::Text),
                ("duration", false, ValueSpec::Number),
                ("resolution", false, en(&["480p", "720p", "1080p"])),
                (
                    "aspect_ratio",
                    false,
                    en(&["adaptive", "16:9", "4:3", "1:1", "3:4", "9:16"]),
                ),
                ("enable_prompt_expansion", false, ValueSpec::Boolean),
                ("seed", false, ValueSpec::Integer),
            ],
        )
    });

    // GPT Image 2.5's two tiers. `image_size` is OpenAI's shape control here —
    // there is no `aspect_ratio` and no `resolution` on any of the four
    // endpoints, so neither is declared and the UI offers neither.
    for (id, display, slug_tier) in [
        ("gpt_image_2_5_flare", "GPT Image 2.5 Flare", "flare"),
        (
            "gpt_image_2_5_sunburst",
            "GPT Image 2.5 Sunburst",
            "sunburst",
        ),
    ] {
        out.push(ModelSpec {
            constraints: vec![format!(
                "fal's `{slug_tier}` tier. `quality` moves the per-image cost by more than an \
                 order of magnitude; fal defaults it to high."
            )],
            ..spec(
                id,
                display,
                Modality::Image,
                &[
                    ("prompt", true, ValueSpec::Text),
                    ("quality", false, ValueSpec::Text),
                    ("background", false, ValueSpec::Text),
                    ("image_size", false, ValueSpec::Text),
                    ("num_images", false, ValueSpec::Integer),
                    ("output_format", false, ValueSpec::Text),
                ],
            )
        });
        let edit_id = format!("{id}_edit");
        out.push(ModelSpec {
            constraints: vec![
                "The edit path takes its sources as a list of images; fal names the field \
                 `image_urls`, so they bind as references rather than as a start frame."
                    .to_string(),
            ],
            ..spec(
                &edit_id,
                &format!("{display} Edit"),
                Modality::Image,
                &[
                    ("prompt", true, ValueSpec::Text),
                    ("image_references", true, ValueSpec::Media),
                    ("quality", false, ValueSpec::Text),
                    ("background", false, ValueSpec::Text),
                    ("image_size", false, ValueSpec::Text),
                    ("num_images", false, ValueSpec::Integer),
                    ("output_format", false, ValueSpec::Text),
                ],
            )
        });
    }

    out.push(spec(
        "qwen_image_3",
        "Qwen Image 3",
        Modality::Image,
        &[
            ("prompt", true, ValueSpec::Text),
            ("negative_prompt", false, ValueSpec::Text),
            ("image_size", false, ValueSpec::Text),
            ("num_images", false, ValueSpec::Integer),
            ("output_format", false, ValueSpec::Text),
            ("enable_prompt_expansion", false, ValueSpec::Boolean),
            ("seed", false, ValueSpec::Integer),
        ],
    ));

    // The three editing endpoints whose generating siblings take no media at
    // all. Keeping them as separate ids is what stops a reference image being
    // attached to an endpoint with nowhere to put it.
    out.push(ModelSpec {
        constraints: vec![
            "4K output is billed at double; web-search grounding adds $0.015 per image, which \
             the estimate does not model."
                .to_string(),
        ],
        ..spec(
            "nano_banana_pro_edit",
            "Nano Banana Pro Edit",
            Modality::Image,
            &[
                ("prompt", true, ValueSpec::Text),
                ("image_references", true, ValueSpec::Media),
                ("resolution", false, en(&["1K", "2K", "4K"])),
                ("aspect_ratio", false, ValueSpec::Text),
                ("num_images", false, ValueSpec::Integer),
                ("output_format", false, ValueSpec::Text),
            ],
        )
    });

    out.push(ModelSpec {
        constraints: vec![
            "Resolution moves the price: 0.5K is 0.75x, 2K is 1.5x and 4K is 2x the 1K rate."
                .to_string(),
        ],
        ..spec(
            "nano_banana_2_edit",
            "Nano Banana 2 Edit",
            Modality::Image,
            &[
                ("prompt", true, ValueSpec::Text),
                ("image_references", false, ValueSpec::Media),
                ("resolution", false, en(&["0.5K", "1K", "2K", "4K"])),
                ("aspect_ratio", false, ValueSpec::Text),
                ("num_images", false, ValueSpec::Integer),
                ("output_format", false, ValueSpec::Text),
            ],
        )
    });

    out.push(ModelSpec {
        constraints: vec![
            "The first input image is free; each one after it adds $0.0045 to the bill."
                .to_string(),
        ],
        ..spec(
            "seedream_v5_pro_edit",
            "Seedream 5.0 Pro Edit",
            Modality::Image,
            &[
                ("prompt", true, ValueSpec::Text),
                ("image_references", true, ValueSpec::Media),
                ("image_size", false, ValueSpec::Text),
                ("num_images", false, ValueSpec::Integer),
                ("output_format", false, ValueSpec::Text),
            ],
        )
    });

    out
}

/// An enumerated flag, written the way fal's schema writes it.
///
/// Always copied from a probed endpoint, never composed here: an option set we
/// invent is a chip row that 422s, which is the failure `audit_fal`'s fourth
/// check exists to find.
fn en(values: &[&str]) -> ValueSpec {
    ValueSpec::Enum(values.iter().map(|s| (*s).to_string()).collect())
}

fn spec(
    id: &str,
    display: &str,
    modality: Modality,
    flags: &[(&str, bool, ValueSpec)],
) -> ModelSpec {
    ModelSpec {
        id: id.to_string(),
        display_name: display.to_string(),
        modality,
        flags: flags
            .iter()
            .map(|(name, required, value)| FlagSpec {
                name: (*name).to_string(),
                alias: None,
                required: *required,
                default: None,
                value: value.clone(),
                arity: Arity::One,
            })
            .collect(),
        constraints: Vec::new(),
    }
}

fn derived(base: &ModelSpec, id: &str, display: &str, why: &str) -> ModelSpec {
    ModelSpec {
        id: id.to_string(),
        display_name: display.to_string(),
        modality: base.modality,
        flags: base.flags.clone(),
        constraints: vec![why.to_string()],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::route::{resolve, RoutePolicy};

    fn all() -> Vec<ProviderId> {
        ProviderId::ALL.to_vec()
    }

    #[test]
    fn registry_covers_the_catalogue_minus_its_exclusions() {
        let cat = catalog::catalogue();
        let reg = registry();

        for id in cat.keys() {
            let excluded = EXCLUSIONS.iter().any(|(x, _)| id == x);
            assert_eq!(
                reg.contains_key(id),
                !excluded,
                "{id} excluded={excluded} but present={}",
                reg.contains_key(id)
            );
        }
        // 55 catalogue models minus 2 exclusions, plus 13 hand-authored from
        // Higgsfield's picker, plus 2 fal-native video editors the catalogue
        // never had because Higgsfield runs its editing surfaces in-house,
        // plus the 13 fal frontier models imported on 2026-09-16 from fal's
        // own index — none of which Higgsfield sells, so none of which any
        // vendored document describes.
        assert_eq!(
            reg.len(),
            cat.len() - EXCLUSIONS.len() + 30,
            "registry size"
        );
    }

    #[test]
    fn the_picker_only_models_are_all_present() {
        let reg = registry();
        for id in [
            "seedance_2_5",
            "seedance_2_0_fast",
            "seedance_pro",
            "kling_o3_flf",
            "kling-v2-5-turbo",
            "kling-omni-flf",
            "happy_horse_video",
            "minimax_h3",
            "wan2_5_video",
            "wan2_2_video",
            "seedream_v5_pro",
            "image2video",
            // Imported from fal's own index on 2026-09-16. Higgsfield sells
            // none of them, so `MODELS.md` describes none of them either.
            "kling3_0_pro",
            "wan3_0_prime",
            "gpt_image_2_5_flare",
            "gpt_image_2_5_flare_edit",
            "gpt_image_2_5_sunburst",
            "gpt_image_2_5_sunburst_edit",
            "qwen_image_3",
            "nano_banana_pro_edit",
            "nano_banana_2_edit",
            "seedream_v5_pro_edit",
        ] {
            assert!(reg.contains_key(id), "{id} missing");
            assert!(
                !catalog::catalogue().contains_key(id),
                "{id} is in MODELS.md after all — drop the hand-authored spec"
            );
        }
    }

    #[test]
    fn sora_2_is_not_a_model_and_never_becomes_one() {
        let reg = registry();
        for id in ["sora2_video", "open_sora_video", "sora_2", "sora-2"] {
            assert!(!reg.contains_key(id), "{id} must not be routable");
        }
        // Nor may it sneak in as an alternate route on something else.
        for m in reg.values() {
            for r in &m.routes {
                assert!(
                    !r.slug.to_ascii_lowercase().contains("sora"),
                    "{} routes to {}",
                    m.id,
                    r.slug
                );
            }
        }
    }

    #[test]
    fn dop_is_reachable_only_with_the_users_own_higgsfield_key() {
        let reg = registry();
        let dop = reg.get("image2video").expect("DoP present");
        assert_eq!(dop.display_name, "Higgsfield DoP");
        assert!(
            dop.routes
                .iter()
                .all(|r| r.provider == ProviderId::Higgsfield),
            "DoP has a third-party route: {:?}",
            dop.routes
        );
        // In-house means no USD, not free.
        let b = Billable::video(5.0, 1280, 720);
        assert!(dop.estimate(&dop.routes[0], &b).is_none());
    }

    #[test]
    fn excluded_models_are_absent_and_say_why() {
        let reg = registry();
        for (id, why) in EXCLUSIONS {
            assert!(!reg.contains_key(id), "{id} should be excluded");
            assert!(why.len() > 40, "{id} needs a real reason, not a shrug");
        }
    }

    #[test]
    fn no_model_has_an_empty_route_list() {
        for (id, m) in registry() {
            assert!(!m.routes.is_empty(), "{id} has no route");
        }
    }

    #[test]
    fn every_route_has_a_cost_entry_even_if_unknown() {
        // A route with no entry at all would make estimate() indistinguishable
        // from "wrong route", and the UI would silently show nothing.
        for (id, m) in registry() {
            for r in &m.routes {
                assert!(
                    m.cost_model(r).is_some(),
                    "{id} route {} has no cost model",
                    r.id()
                );
            }
        }
    }

    #[test]
    fn route_ids_are_unique_within_a_model() {
        for (id, m) in registry() {
            let mut seen = std::collections::HashSet::new();
            for r in &m.routes {
                assert!(seen.insert(r.id()), "{id} has duplicate route {}", r.id());
            }
        }
    }

    #[test]
    fn an_estimate_for_a_foreign_route_is_none_not_a_panic() {
        let reg = registry();
        let kling = reg.get("kling3_0").unwrap();
        let flux = reg.get("flux_2").unwrap();
        assert!(kling
            .estimate(&flux.routes[0], &Billable::video(5.0, 1280, 720))
            .is_none());
    }

    #[test]
    fn display_overrides_replace_the_cli_names() {
        let cat = catalog::catalogue();
        let reg = registry();
        // Guard both halves: that the CLI really still says the old thing, and
        // that we really replaced it. If a vendor refresh fixes their label,
        // the first assert fires and the override can be retired.
        for (id, want) in DISPLAY_OVERRIDES {
            let cli = &cat.get(id).expect(id).display_name;
            assert_ne!(cli, want, "{id}: MODELS.md now matches the picker");
            assert_eq!(reg.get(id).unwrap().display_name, want, "{id}");
        }
        assert_eq!(reg["kling3_0"].display_name, "Kling 3.0");
        assert_eq!(reg["grok_video_v15"].display_name, "Grok Imagine 1.5");
    }

    #[test]
    fn untouched_names_keep_the_catalogue_spelling() {
        let reg = registry();
        assert_eq!(reg["veo3_1"].display_name, "Google Veo 3.1");
        // The id/name inversion is real and must survive.
        assert_eq!(reg["nano_banana_2"].display_name, "Nano Banana Pro");
        assert_eq!(reg["nano_banana_flash"].display_name, "Nano Banana 2");
    }

    #[test]
    fn there_are_twelve_launch_families() {
        assert_eq!(LAUNCH_FAMILIES.len(), 12);
        let reg = registry();
        for (family, ids) in LAUNCH_FAMILIES {
            assert!(!ids.is_empty(), "{family} has no models");
            for id in ids {
                assert!(reg.contains_key(*id), "{family}: {id} not in the registry");
                assert!(reg[*id].launch, "{family}: {id} not marked launch");
            }
        }
    }

    #[test]
    fn launch_models_all_resolve_to_a_route() {
        let b = Billable::video(8.0, 1280, 720);
        for m in launch_models() {
            let chosen = resolve(&m.routes, &all(), RoutePolicy::Cheapest, None, m.pricer(&b))
                .unwrap_or_else(|e| panic!("{} did not resolve: {e}", m.id));
            assert!(m.cost_model(chosen).is_some(), "{}", m.id);
        }
    }

    #[test]
    fn launch_models_are_priced_except_the_one_openai_will_not_quote() {
        // Every launch model must show real USD before submit. GPT Image 2 is
        // the single exception and it is deliberate: OpenAI bills it on output
        // tokens and publishes only a $0.006-$0.211 range.
        let video = Billable::video(8.0, 1280, 720);
        // A 1 MP image, because the FLUX family bills per megapixel and a bare
        // count tells it nothing.
        let image = Billable {
            megapixels: Some(1.0),
            ..Billable::image(1)
        };
        let mut unpriced = Vec::new();
        for m in launch_models() {
            let b = if m.modality == Modality::Video {
                video.clone()
            } else {
                image.clone()
            };
            if m.routes.iter().all(|r| m.estimate(r, &b).is_none()) {
                unpriced.push(m.id);
            }
        }
        assert_eq!(unpriced, vec!["gpt_image_2".to_string()]);
    }

    #[test]
    fn only_launch_models_carry_the_marker() {
        let flagged: Vec<String> = registry()
            .into_values()
            .filter(|m| m.launch)
            .map(|m| m.id)
            .collect();
        let expected: usize = LAUNCH_FAMILIES.iter().map(|(_, ids)| ids.len()).sum();
        assert_eq!(flagged.len(), expected);
        assert_eq!(launch_models().len(), expected);
    }

    // ---- prices ---------------------------------------------------------
    //
    // Each of these reproduces a headline figure from the research plan's cost
    // table. If a vendor moves, the number that moved is named in the failure.

    fn usd(model: &str, route: &str, b: &Billable) -> f64 {
        let reg = registry();
        let m = reg.get(model).unwrap_or_else(|| panic!("{model} missing"));
        let r = m
            .route(route)
            .unwrap_or_else(|| panic!("{model} has no route {route}"));
        m.estimate(r, b)
            .unwrap_or_else(|| panic!("{model}/{route} unpriced"))
            .usd
    }

    fn near(got: f64, want: f64, what: &str) {
        assert!((got - want).abs() < 0.01, "{what}: got {got}, want {want}");
    }

    #[test]
    fn kling_3_matches_the_published_eight_second_price() {
        let b = Billable::video(8.0, 1280, 720);
        near(
            usd("kling3_0", "fal:fal-ai/kling-video/v3/standard", &b),
            0.67,
            "Kling 3.0 8s 720p on fal",
        );
    }

    #[test]
    fn kling_audio_multiplier_is_not_hidden() {
        // Higgsfield does not tell you audio costs 1.5x. We do.
        let reg = registry();
        let m = &reg["kling3_0"];
        let r = m.route("fal:fal-ai/kling-video/v3/standard").unwrap();
        let mut b = Billable::video(8.0, 1280, 720);
        b.audio = true;
        let e = m.estimate(r, &b).unwrap();
        near(e.usd, 1.008, "Kling 3.0 with audio");
        assert!(e.basis.contains("audio x1.5"), "basis was {}", e.basis);
    }

    #[test]
    fn seedance_2_0_costs_more_on_fal_than_on_the_gateway() {
        let b = Billable::video(8.0, 1280, 720);
        let fal = usd("seedance_2_0", "fal:bytedance/seedance-2.0", &b);
        let vaig = usd("seedance_2_0", "vaig:bytedance/seedance-2.0", &b);
        // $2.4192 from the token rate against fal's own rounded $2.43 quote —
        // a 0.4% gap that is theirs, not ours. Tolerance is widened for exactly
        // this comparison rather than the token rate being bent to match.
        assert!(
            (fal - 2.43).abs() < 0.02,
            "Seedance 2.0 8s 720p on fal: {fal}"
        );
        near(vaig, fal / 2.0, "VAIG is half fal");
        assert!(vaig < fal);
    }

    #[test]
    fn seedance_scales_with_resolution_not_only_duration() {
        // The whole reason Seedance is token-priced rather than per-second.
        let sd = usd(
            "seedance_2_0",
            "fal:bytedance/seedance-2.0",
            &Billable::video(8.0, 1280, 720),
        );
        let hd = usd(
            "seedance_2_0",
            "fal:bytedance/seedance-2.0",
            &Billable::video(8.0, 1920, 1080),
        );
        assert!(
            (hd / sd - 2.25).abs() < 0.01,
            "1080p should be 2.25x 720p, got {}",
            hd / sd
        );
    }

    #[test]
    fn seedance_fast_matches_the_published_price() {
        near(
            usd(
                "seedance_2_0_fast",
                "fal:bytedance/seedance-2.0/fast",
                &Billable::video(8.0, 1280, 720),
            ),
            1.94,
            "Seedance 2.0 Fast 8s 720p on fal",
        );
    }

    #[test]
    fn the_veo_tiers_are_an_order_of_magnitude_apart() {
        let b = Billable::video(8.0, 1280, 720);
        near(
            usd("veo3_1", "google:veo-3.1-generate-001", &b),
            3.20,
            "Veo 3.1",
        );
        near(
            usd("veo3_1_fast", "google:veo-3.1-fast-generate-001", &b),
            0.80,
            "Veo 3.1 Fast",
        );
        near(
            usd("veo3_1_lite", "google:veo-3.1-lite-generate-preview", &b),
            0.40,
            "Veo 3.1 Lite",
        );
    }

    #[test]
    fn direct_vendor_routes_win_where_the_plan_says_they_do() {
        let vid = Billable::video(8.0, 1280, 720);
        // The Grok row is deliberately gone. The plan claimed xAI-direct was
        // 3x cheaper than fal at 1080p, and it is not cheaper — it does not
        // exist. Probed 2026-08-05: xAI's API serves chat and responses only;
        // `/v1/image/generations` and `/v1/video/generations` both 404, and the
        // published reference lists no generation endpoint. The route, the
        // price and the arbitrage claim were all transcription from a corpus
        // assertion nobody had checked.
        //
        // Gemini Omni direct beats fal.
        near(
            usd("gemini_omni", "google:gemini-omni-flash", &vid),
            0.80,
            "Gemini Omni direct",
        );
        assert!(
            usd("gemini_omni", "google:gemini-omni-flash", &vid)
                < usd("gemini_omni", "fal:google/gemini-omni-flash", &vid)
        );
    }

    #[test]
    fn gateway_routes_win_where_the_plan_says_they_do() {
        let img = Billable::image(1);
        near(
            usd("seedream_v5_pro", "vaig:bytedance/seedream-5.0-pro", &img),
            0.035,
            "Seedream VAIG",
        );
        assert!(
            usd("seedream_v5_pro", "vaig:bytedance/seedream-5.0-pro", &img)
                < usd("seedream_v5_pro", "fal:bytedance/seedream/v5/pro", &img)
        );

        let vid = Billable::video(5.0, 2560, 1440);
        near(
            usd("minimax_h3", "vaig:minimax/minimax-h3", &vid),
            0.65,
            "MiniMax H3 2K 5s",
        );
        assert!(
            (usd("minimax_h3", "fal:minimax/h3", &vid)
                / usd("minimax_h3", "vaig:minimax/minimax-h3", &vid)
                - 2.0)
                .abs()
                < 1e-9,
            "fal should be exactly 2x VAIG on H3"
        );
    }

    /// The arbitrage *knowledge* — which provider is genuinely cheapest.
    ///
    /// Asserted on price directly rather than through `resolve`, because the
    /// resolver deliberately will not pick a route it cannot execute (see
    /// `ProviderId::has_adapter`). These are the numbers the pricing story in
    /// the plan rests on, so losing them to an adapter gap would be silent and
    /// expensive. When the Vaig and xAI adapters land, this table and
    /// `cheapest_policy_picks_the_cheapest_executable_route` converge.
    #[test]
    fn the_cheapest_provider_by_price_is_the_arbitrage_one() {
        let reg = registry();
        let b = Billable::video(8.0, 1280, 720);
        for (id, want) in [
            ("seedance_2_0", ProviderId::Vaig),
            ("seedance1_5", ProviderId::Vaig),
            ("minimax_h3", ProviderId::Vaig),
            ("kling3_0", ProviderId::Fal),
        ] {
            let m = &reg[id];
            let cheapest = m
                .routes
                .iter()
                .filter_map(|r| m.estimate(r, &b).map(|e| (r.provider, e.usd)))
                .min_by(|a, c| a.1.total_cmp(&c.1))
                .map(|(p, _)| p);
            assert_eq!(cheapest, Some(want), "{id} priced cheapest on {cheapest:?}");
        }
    }

    #[test]
    fn cheapest_policy_picks_the_cheapest_executable_route() {
        // Whatever the resolver returns must be something the shell can
        // actually run — the A1 fix. Picking a cheaper unreachable route told
        // the user to add a key they had already added.
        let reg = registry();
        let b = Billable::video(8.0, 1280, 720);
        for id in ["seedance_2_0", "minimax_h3", "grok_video_v15", "kling3_0"] {
            let m = &reg[id];
            let got =
                resolve(&m.routes, &all(), RoutePolicy::Cheapest, None, m.pricer(&b)).unwrap();
            assert!(
                got.provider.has_adapter(),
                "{id} resolved to {}, which has no client",
                got.provider
            );
        }
    }

    #[test]
    fn every_launch_model_can_actually_be_run() {
        // The roster is allowed to contain models we cannot route yet, but the
        // *launch* set is what onboarding puts in front of a new user, and a
        // launch model with no executable route is a dead end on first contact.
        let unroutable: Vec<String> = launch_models()
            .into_iter()
            .filter(|m| !m.routes.iter().any(|r| r.provider.has_adapter()))
            .map(|m| m.id)
            .collect();
        assert!(
            unroutable.is_empty(),
            "launch models with no executable route: {unroutable:?}"
        );
    }

    #[test]
    fn the_unroutable_models_are_a_known_short_list() {
        // Pins the blast radius of the adapter gap. If this grows, a route was
        // added for a provider with no client — or a client was removed.
        let reg = registry();
        let mut unroutable: Vec<&str> = reg
            .values()
            .filter(|m| !m.routes.iter().any(|r| r.provider.has_adapter()))
            .map(|m| m.id.as_str())
            .collect();
        unroutable.sort_unstable();
        assert_eq!(
            unroutable,
            [
                "image_background_remover",
                "nano_banana",
                "outpaint",
                // z_image's only route is `local:z-image`, and Local has no
                // client yet (S2-honest-free-tier): its keyless "free tier" was
                // a lie, so it joins the unroutable set until a local adapter
                // ships. Dropping back out of this list is the re-enable signal.
                "z_image",
            ],
            "the set of unroutable models changed"
        );
    }

    #[test]
    fn an_unpriced_route_never_wins_on_price() {
        // Kontext: only the VAIG route has a published price, and BFL is first
        // in the authored order. Neither has a client yet, so the resolver
        // cannot pick either — but the *ordering rule* being tested here is
        // that an unknown price sorts behind a known one, which is asserted
        // directly so it survives the adapter gap.
        let reg = registry();
        let m = &reg["flux_kontext"];
        assert_eq!(m.routes[0].provider, ProviderId::Bfl);
        let b = Billable::image(1);

        let vaig = m
            .routes
            .iter()
            .find(|r| r.provider == ProviderId::Vaig)
            .expect("Kontext has a VAIG route");
        near(m.estimate(vaig, &b).unwrap().usd, 0.04, "Kontext pro");
        assert!(
            m.estimate(&m.routes[0], &b).is_none(),
            "BFL's Kontext price is unpublished; if that changes, revisit this"
        );

        // And the resolver still returns something runnable.
        let got = resolve(&m.routes, &all(), RoutePolicy::Cheapest, None, m.pricer(&b)).unwrap();
        assert!(got.provider.has_adapter(), "resolved to {}", got.provider);
    }

    #[test]
    fn flux_2_prices_the_first_megapixel_differently() {
        let reg = registry();
        let m = &reg["flux_2"];
        let r = m.route("bfl:flux-2-pro").unwrap();
        let mut b = Billable::image(1);
        b.megapixels = Some(1.0);
        near(m.estimate(r, &b).unwrap().usd, 0.03, "FLUX.2 pro 1MP");
        b.megapixels = Some(3.0);
        near(m.estimate(r, &b).unwrap().usd, 0.06, "FLUX.2 pro 3MP");
    }

    #[test]
    fn seedream_edit_charges_for_extra_reference_images() {
        let reg = registry();
        let m = &reg["seedream_v5_pro"];
        let r = m.route("fal:bytedance/seedream/v5/pro").unwrap();
        let mut b = Billable::image(1);
        b.extra_inputs = 3;
        near(
            m.estimate(r, &b).unwrap().usd,
            0.0810,
            "Seedream edit with 3 extra inputs",
        );
    }

    #[test]
    fn a_local_route_is_free_and_needs_no_key() {
        let reg = registry();
        let m = &reg["z_image"];
        assert_eq!(m.routes[0].provider, ProviderId::Local);
        assert!(!ProviderId::Local.needs_key());
        assert_eq!(
            m.estimate(&m.routes[0], &Billable::image(1)).unwrap().usd,
            0.0
        );
    }

    #[test]
    fn models_the_corpus_could_not_price_stay_unknown() {
        // Never a zero, never a plausible-looking guess.
        let reg = registry();
        let b = Billable::video(8.0, 1280, 720);
        for id in [
            "kling-omni-flf",    // fal's pricing field is blank for the o1 family
            "happy_horse_video", // vendor known, slug and price never found
            "image2video",       // in-house, credit-priced
        ] {
            let m = &reg[id];
            assert!(
                m.routes.iter().all(|r| m.estimate(r, &b).is_none()),
                "{id} produced a price we cannot source"
            );
        }
        assert!(reg["gpt_image_2"].routes.iter().all(|r| reg["gpt_image_2"]
            .estimate(r, &Billable::image(1))
            .is_none()));
        // The same refusal, extended to GPT Image 2.5: fal publishes a token
        // rate and a `quality` flag that swings the per-image cost by an order
        // of magnitude, and picking either end would be a fabricated number.
        for id in [
            "gpt_image_2_5_flare",
            "gpt_image_2_5_flare_edit",
            "gpt_image_2_5_sunburst",
            "gpt_image_2_5_sunburst_edit",
        ] {
            let m = &reg[id];
            assert!(
                m.routes
                    .iter()
                    .all(|r| m.estimate(r, &Billable::image(1)).is_none()),
                "{id} produced a price we cannot source"
            );
        }
    }

    // ---- the 2026-09-16 fal import ---------------------------------------

    /// Every id added by the fal catalogue sync, in one place, so the three
    /// tests below cannot drift apart from each other.
    const IMPORTED_2026_09: [&str; 11] = [
        "gpt_image_2_5_flare",
        "gpt_image_2_5_flare_edit",
        "gpt_image_2_5_sunburst",
        "gpt_image_2_5_sunburst_edit",
        "kling3_0_pro",
        "nano_banana_2_edit",
        "nano_banana_pro_edit",
        "qwen_image_3",
        // Promoted from placeholder to a real, probed, priced model rather
        // than added, which is why it is here and not in the count above.
        "seedance_2_5",
        "seedream_v5_pro_edit",
        "wan3_0_prime",
    ];

    #[test]
    fn every_imported_model_resolves_to_an_exact_fal_endpoint() {
        // The failure this closes is the one `docs/FIRST-LIGHT.md` records: a
        // family root posted unsuffixed answers 404 after a full round trip.
        // An imported model that resolves to nothing is a tile that cannot run.
        let reg = registry();
        for id in IMPORTED_2026_09 {
            let m = &reg[id];
            let route = m
                .routes
                .iter()
                .find(|r| r.provider == ProviderId::Fal)
                .unwrap_or_else(|| panic!("{id} has no fal route"));
            let produces_video = m.modality == Modality::Video;
            let resolved: Vec<String> = [
                crate::media::InputMode::Text,
                crate::media::InputMode::Image,
                crate::media::InputMode::Video,
            ]
            .into_iter()
            .filter_map(|mode| {
                crate::media::resolve_endpoint(&route.slug, mode, produces_video).ok()
            })
            .collect();
            assert!(
                !resolved.is_empty(),
                "{id}: no input mode resolves {} to an endpoint",
                route.slug
            );
            // And the endpoint it resolves to is one fal's own index lists.
            // This is the check a slug diff cannot do: `bytedance/seedance-2.5`
            // is not an endpoint, `bytedance/seedance-2.5/text-to-video` is,
            // and only the resolver knows which one goes on the wire.
            let index = crate::fal_catalogue::Catalogue::bundled();
            for ep in &resolved {
                assert!(
                    index.get(ep).is_some(),
                    "{id}: {ep} is not in fal's index — the route would 404"
                );
            }
        }
    }

    #[test]
    fn every_imported_model_has_a_fal_route_with_a_real_adapter() {
        // Guards against importing a frontier model that only reaches a
        // provider we have no client for — a picker tile that cannot generate.
        let reg = registry();
        for id in IMPORTED_2026_09 {
            assert!(
                reg[id]
                    .routes
                    .iter()
                    .any(|r| r.provider == ProviderId::Fal && r.provider.has_adapter()),
                "{id} has no executable fal route"
            );
        }
    }

    #[test]
    fn seedance_2_5_offers_a_thirty_second_clip() {
        // The reason this slice imported it: no other model in the roster can
        // make a native half-minute clip. Written to accept either shape —
        // fal's own enum, or a free-form axis the UI renders as a number box —
        // so it survives fal changing how it declares the field.
        let caps = registry()["seedance_2_5"].spec.capabilities();
        assert!(
            caps.supports_duration,
            "seedance_2_5 lost its duration axis"
        );
        assert!(
            caps.durations.contains(&30.0) || caps.durations.is_empty(),
            "seedance_2_5 offers {:?}, which cannot reach 30s",
            caps.durations
        );
    }

    #[test]
    fn xai_has_no_direct_routes_because_it_serves_no_generation_api() {
        // Probed 2026-08-05: api.x.ai serves chat and responses only, and both
        // /v1/image/generations and /v1/video/generations answer 404. Every
        // xAI-direct route in the roster was transcribed from a corpus claim
        // that was never checked — and because the fabricated price was the
        // cheapest, the resolver preferred it.
        let reg = registry();
        let direct: Vec<&str> = reg
            .values()
            .flat_map(|m| m.routes.iter().map(move |r| (m, r)))
            .filter(|(_, r)| r.provider == ProviderId::XAi)
            .map(|(m, _)| m.id.as_str())
            .collect();
        assert!(
            direct.is_empty(),
            "xAI serves no generation API, so these routes cannot work: {direct:?}"
        );
    }

    #[test]
    fn the_grok_suite_routes_through_fal() {
        // fal serves the whole Grok Imagine line, including two video editors
        // nothing else in the roster could do.
        let reg = registry();
        for id in [
            "grok_video_v15",
            "grok_image",
            "grok_edit_video",
            "grok_extend_video",
        ] {
            let m = reg.get(id).unwrap_or_else(|| panic!("{id} missing"));
            assert!(
                m.routes.iter().any(|r| r.provider == ProviderId::Fal),
                "{id} has no fal route"
            );
        }
    }

    #[test]
    fn the_grok_editors_can_edit_a_clip() {
        // They exist to fill the gap that started this: a user attaching a clip
        // and finding nothing in the roster that could take one.
        use crate::use_case::{models_for, UseCase};
        let editors = models_for(UseCase::EditVideo);
        for id in ["grok_edit_video", "grok_extend_video"] {
            assert!(
                editors.iter().any(|m| m.id == id),
                "{id} is not offered for editing a video"
            );
        }
    }

    #[test]
    fn wan_video_is_priced_from_fals_published_prose() {
        // Previously listed as unpriceable, on the strength of fal's structured
        // `pricingInfoOverride` field being blank. It is not blank — fal states
        // the price in prose on the model page, and the same tiering covers
        // text-to-video, image-to-video, video-to-video and VACE. Verified from
        // the catalogue on 2026-08-05.
        let reg = registry();
        for id in ["wan2_2_video", "wan_vace"] {
            let m = &reg[id];
            let r = &m.routes[0];
            let at_720 = m.estimate(r, &Billable::video(5.0, 1280, 720)).unwrap();
            let at_480 = m.estimate(r, &Billable::video(5.0, 854, 480)).unwrap();
            assert!(
                (at_720.usd - 0.40).abs() < 1e-9,
                "{id} 5s at 720p should be $0.40, got {}",
                at_720.usd
            );
            assert!(
                (at_480.usd - 0.20).abs() < 1e-9,
                "{id} 5s at 480p should be $0.20, got {}",
                at_480.usd
            );
            // The estimate has to explain itself; "$0.40" alone is unauditable.
            assert!(at_720.basis.contains("720"), "{}", at_720.basis);
        }
    }

    #[test]
    fn a_tiered_price_without_a_resolution_is_unknown_not_a_guess() {
        // Picking the cheapest tier would under-quote a 720p job by 2x on the
        // Generate button; picking the dearest would over-charge. Neither is
        // acceptable, so say nothing.
        let reg = registry();
        let m = &reg["wan_vace"];
        let mut b = Billable::video(5.0, 1280, 720);
        b.height = None;
        assert!(m.estimate(&m.routes[0], &b).is_none());
    }

    #[test]
    fn unverified_slugs_are_flagged_in_the_note_the_ui_shows() {
        let reg = registry();
        // `seedance_2_5` left this list on 2026-09-16: fal now serves the slug
        // and publishes its token rate, both verified by `examples/audit_fal`.
        // `outpaint` stays, so the test keeps its teeth.
        let outpaint = &reg["outpaint"];
        assert!(
            outpaint
                .routes
                .iter()
                .any(|r| r.note.as_deref().is_some_and(|n| n.contains("unverified"))),
            "outpaint hides its uncertainty"
        );
        // And the model that left must not have taken the word with it: a
        // sourced price and an "unverified slug" note cannot both be true.
        assert!(
            reg["seedance_2_5"].routes.iter().all(|r| !r
                .note
                .as_deref()
                .unwrap_or_default()
                .contains("unverified")),
            "seedance_2_5 is priced and probed but still calls itself unverified"
        );
    }

    #[test]
    fn surfaces_with_no_third_party_api_route_to_higgsfield_only() {
        let reg = registry();
        for id in [
            "text2image_soul_v2",
            "soul_cast",
            "soul_cinematic",
            "cinematic_studio_video_3_5",
            "tripo_3d",
            "video_explainer",
        ] {
            let m = &reg[id];
            assert!(
                m.routes
                    .iter()
                    .all(|r| r.provider == ProviderId::Higgsfield),
                "{id} claims a third-party route: {:?}",
                m.routes
            );
            assert!(
                m.routes.iter().all(|r| r.note.is_some()),
                "{id} needs a note"
            );
        }
        assert_eq!(
            reg["text2image_soul_v2"].routes[0].slug,
            "higgsfield-ai/soul/standard"
        );
    }

    #[test]
    fn a_model_with_no_configured_provider_says_which_key_would_help() {
        let reg = registry();
        let m = &reg["seedance_2_0"];
        let b = Billable::video(8.0, 1280, 720);
        let err = resolve(&m.routes, &[], RoutePolicy::Cheapest, None, m.pricer(&b)).unwrap_err();
        assert!(err.to_string().contains("fal.ai"), "{err}");
    }

    #[test]
    fn hand_authored_specs_carry_their_parameters() {
        let reg = registry();
        assert!(reg["minimax_h3"].spec.takes_prompt());
        assert!(reg["image2video"].spec.flag("input_image").is_some());
        assert!(reg["kling_o3_flf"].spec.flag("sound").is_some());
        // A derived spec must track its base rather than drift.
        assert_eq!(
            reg["seedance_2_0_fast"].spec.flags,
            catalog::catalogue()["seedance_2_0"].flags
        );
        assert!(!reg["seedance_2_5"].spec.constraints.is_empty());
    }

    // ---- job types --------------------------------------------------------

    #[test]
    fn every_model_has_a_job_type_from_the_table_not_from_the_fallback() {
        // The bug this closes: nothing mapped a model id to a JobType, so
        // `enhance::decide` could never be called for a model the user picked.
        // A model missing from JOB_TYPES still gets *a* job type from the
        // modality fallback, which is why this asserts table membership rather
        // than just that the field is populated.
        let reg = registry();
        for (id, m) in &reg {
            let (slug, _) = JOB_TYPES
                .iter()
                .find(|(_, ids)| ids.contains(&id.as_str()))
                .unwrap_or_else(|| {
                    panic!("{id} is not in JOB_TYPES; its enhance default was inferred, not chosen")
                });
            assert_eq!(m.job_type.slug(), *slug, "{id}");
        }
    }

    #[test]
    fn the_job_type_table_names_only_real_job_types_and_real_models_exactly_once() {
        let reg = registry();
        let mut seen = std::collections::HashSet::new();
        for (slug, ids) in JOB_TYPES {
            assert!(
                JobType::from_slug(slug).is_some(),
                "{slug} is not a JobType wire name"
            );
            assert!(!ids.is_empty(), "{slug} lists no models");
            for id in ids {
                assert!(reg.contains_key(*id), "JOB_TYPES lists {id}, not a model");
                assert!(seen.insert(*id), "{id} appears in two job-type groups");
            }
        }
        assert_eq!(
            seen.len(),
            reg.len(),
            "JOB_TYPES and the registry disagree on the model set"
        );
    }

    #[test]
    fn representative_models_map_to_the_job_type_their_prompt_needs() {
        let reg = registry();
        for (id, want) in [
            ("seedance_2_0", JobType::Video),
            ("veo3_1_lite", JobType::Video),
            // DoP is image-to-video, and `Video` is defined as every
            // text-to-video *and* image-to-video path — so not `Animate`.
            ("image2video", JobType::Video),
            ("nano_banana_2", JobType::ImageNanoBanana2),
            ("nano_banana", JobType::ImageNanoBanana),
            ("flux_kontext", JobType::ImageFlux),
            ("flux_2", JobType::ImageFlux2),
            ("gpt_image_2", JobType::ImageGptImage2),
            ("openai_hazel", JobType::ImageGpt),
            ("kling_omni_image", JobType::ImageKlingOmni),
            ("z_image", JobType::ZImage),
            ("seedream_v5_pro", JobType::ImageSeedream),
            ("text2image_soul_v2", JobType::ImageStyled),
            ("cinematic_studio_video_3_5", JobType::Builder),
            ("outpaint", JobType::Reference),
            ("tripo_3d", JobType::Product),
            ("text2speech_v2", JobType::Speech),
        ] {
            assert_eq!(reg[id].job_type, want, "{id}");
        }
    }

    #[test]
    fn nano_banana_job_types_follow_the_internal_id_not_the_name_they_sell() {
        // The trap: `nano_banana_2` is sold as "Nano Banana Pro" and
        // `nano_banana_flash` is sold as "Nano Banana 2". Higgsfield's enhance
        // table is keyed by their internal job type, so `image-nano-banana-2`
        // belongs to `nano_banana_2`. Reading the sold names would swap them.
        let reg = registry();
        assert_eq!(reg["nano_banana_2"].display_name, "Nano Banana Pro");
        assert_eq!(reg["nano_banana_2"].job_type, JobType::ImageNanoBanana2);
        assert_eq!(reg["nano_banana_flash"].display_name, "Nano Banana 2");
        assert_eq!(reg["nano_banana_flash"].job_type, JobType::ImageNanoBanana2);
        assert_eq!(reg["nano_banana"].job_type, JobType::ImageNanoBanana);
    }

    #[test]
    fn a_model_with_no_prompt_never_defaults_to_enhancing_one() {
        // Enhancement rewrites the prompt. Background removal, outpaint, the
        // explainer assembler and the image-driven 3D jobs take no `--prompt`
        // flag at all, so an "on" default would send an LLM an empty string to
        // embellish and spend a call on a result the provider never sees.
        for (id, m) in registry() {
            if !m.spec.takes_prompt() {
                assert!(
                    !m.job_type.default_enhance(),
                    "{id} takes no prompt but {:?} enhances by default",
                    m.job_type
                );
            }
        }
    }

    #[test]
    fn a_model_id_now_reaches_the_enhance_decision() {
        // The whole point of the join: `decide` takes a JobType and the picker
        // produces a model id. Before JOB_TYPES there was nothing in between.
        use crate::enhance::{decide, EnhanceInputs, EnhanceReason};
        let reg = registry();

        let video = decide(EnhanceInputs::new(reg["seedance_2_0"].job_type));
        assert!(video.enhance, "a video prompt defaults to enhanced prose");
        assert_eq!(video.reason, EnhanceReason::JobDefault);

        let edit = decide(EnhanceInputs::new(reg["nano_banana_2"].job_type));
        assert!(!edit.enhance, "an instruction-follower is not rewritten");
        assert_eq!(edit.reason, EnhanceReason::JobDefault);
    }

    #[test]
    fn every_model_keeps_its_id_as_the_map_key() {
        for (k, m) in registry() {
            assert_eq!(k, m.id);
            assert_eq!(m.spec.id, m.id);
            assert!(!m.display_name.is_empty(), "{k} has no label");
        }
    }

    // -- The user's own Higgsfield key as a second route ---------------------

    /// Every model that gained a Higgsfield route alongside a third-party one.
    const DUAL_ROUTED: [(&str, &str); 6] = [
        ("seedance_pro", "bytedance/seedance/v1/pro/fast"),
        ("kling-v2-5-turbo", "kling-video/v2.5-turbo/pro"),
        ("wan2_5_video", "wan-25-preview"),
        ("minimax_hailuo", "minimax/hailuo-2.3/pro"),
        ("veo3_1", "veo3.1"),
        ("veo3_1_fast", "veo3.1/fast"),
    ];

    #[test]
    fn a_higgsfield_key_is_a_second_way_into_models_a_third_party_also_serves() {
        let reg = registry();
        for (id, slug) in DUAL_ROUTED {
            let m = reg.get(id).unwrap_or_else(|| panic!("{id} missing"));
            let hf = m
                .routes
                .iter()
                .find(|r| r.provider == ProviderId::Higgsfield)
                .unwrap_or_else(|| panic!("{id} has no Higgsfield route"));
            assert_eq!(hf.slug, slug, "{id} points at the wrong path");
            assert!(hf.note.is_some(), "{id}'s Higgsfield route needs a note");
            assert!(
                m.routes
                    .iter()
                    .any(|r| r.provider != ProviderId::Higgsfield),
                "{id} is dual-routed, so it must keep a third-party route"
            );
        }
    }

    #[test]
    fn every_higgsfield_route_is_one_their_spec_actually_publishes() {
        // The guard for the whole class. `higgsfield-ai/dop/preview` shipped
        // as a route to a tier that has never existed, and a wrong slug is
        // indistinguishable from an unreachable surface at the call site: both
        // answer `model_not_found`, and the client blames their web-app gate.
        //
        // Only the measured slugs are checked. The Studio compilers and the 3D
        // family are deliberately unmeasured — we know their *product* has
        // them and do not know any path — so absence from the table is not an
        // error there. What is an error is a slug shaped like a platform path
        // that the platform does not serve.
        let reg = registry();
        for m in reg.values() {
            for r in &m.routes {
                if r.provider != ProviderId::Higgsfield || !r.slug.contains('/') {
                    continue;
                }
                assert!(
                    crate::media::higgsfield_is_measured(&r.slug),
                    "{}'s route {} looks like a platform path but is not in their spec",
                    m.id,
                    r.slug
                );
            }
        }
    }

    #[test]
    fn a_higgsfield_route_never_silently_takes_a_generation() {
        // These bill in credits, so they are Unknown, and the resolver sorts
        // Unknown last. Adding one must never move a user off a priced route
        // they were already getting — they have to pin it deliberately.
        let reg = registry();
        let b = Billable::video(5.0, 1280, 720);
        for (id, _) in DUAL_ROUTED {
            let m = &reg[id];
            let hf = m
                .routes
                .iter()
                .find(|r| r.provider == ProviderId::Higgsfield)
                .unwrap();
            assert!(
                m.estimate(hf, &b).is_none(),
                "{id}: a credit-billed route must not quote USD"
            );
        }
    }

    #[test]
    fn hailuo_23_finally_has_a_route_that_exists() {
        // Its only route was `fal-ai/minimax/hailuo-2.3`, measured absent from
        // fal — the model was unreachable by any path until the Higgsfield one.
        let reg = registry();
        let m = &reg["minimax_hailuo"];
        let fal = m
            .routes
            .iter()
            .find(|r| r.provider == ProviderId::Fal)
            .unwrap();
        assert!(
            crate::media::route_is_missing(&fal.slug),
            "if fal now serves hailuo-2.3 this test's premise is stale"
        );
        assert!(
            m.routes
                .iter()
                .any(|r| r.provider == ProviderId::Higgsfield),
            "hailuo-2.3 would have no reachable route at all"
        );
    }
}
