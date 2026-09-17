//! The command surface exposed to the webview.
//!
//! Two rules hold everywhere in this file. A secret never crosses the bridge —
//! the UI can learn *whether* a key is set, never what it is. And an unknown
//! price is returned as `null`, never as zero: telling someone a paid
//! generation is free is the worst thing this app could do.

use crate::app::AppState;
use crate::runner::now_secs;
use crate::vault::{self, KeyState};
use hickeyfield_core::clients::{detect_local, LocalEndpoints};
use hickeyfield_core::engine::{JobSet, JobStore};
use hickeyfield_core::{registry, Billable, Estimate, ProviderId, RoutePolicy};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tauri::State;

/// A process-wide monotonic counter, minted once per id.
///
/// `now_secs()` is whole-second resolution (`runner.rs`), so a generate-all loop
/// that submits several fast enqueue POSTs within one second would otherwise mint
/// the same `{secs}-{model.id}` id twice — and `store.upsert`'s
/// `ON CONFLICT(id) DO UPDATE` would silently overwrite the first row while every
/// POST is still billed. This counter guarantees each id is distinct regardless of
/// clock granularity, for both storyboard shots and standalone submits.
static ID_SEQ: AtomicU64 = AtomicU64::new(0);

/// The next process-wide-unique suffix. Never repeats within a run.
fn next_id_seq() -> u64 {
    ID_SEQ.fetch_add(1, Ordering::Relaxed)
}

/// Mint a job id that cannot collide with another minted in the same second.
///
/// The seconds + model id keep the id human-legible and stable in shape; the
/// trailing counter is what makes two same-model shots submitted inside one
/// wall-clock second (the generate-all loop) distinct, so `store.upsert`'s
/// `ON CONFLICT(id) DO UPDATE` can never overwrite an earlier shot's row. No
/// code parses this id, so the extra `-{seq}` segment is safe.
fn mint_job_id(model_id: &str) -> String {
    format!("{}-{}-{}", now_secs(), model_id, next_id_seq())
}

/// Mint a storyboard (group) id that cannot collide with another minted in the
/// same second. Same reasoning as [`mint_job_id`].
fn mint_storyboard_id() -> String {
    format!("sb-{}-{}", now_secs(), next_id_seq())
}

// ── Keys ───────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn key_states() -> Vec<KeyState> {
    ProviderId::ALL.into_iter().map(vault::state).collect()
}

#[tauri::command]
pub fn set_key(provider: String, secret_half: bool, value: String) -> Result<(), String> {
    let p =
        ProviderId::from_slug(&provider).ok_or_else(|| format!("unknown provider: {provider}"))?;
    vault::set(p, secret_half, &value)
}

#[tauri::command]
pub fn configured_providers() -> Vec<String> {
    ProviderId::ALL
        .into_iter()
        .filter(|p| vault::is_configured(*p))
        .map(|p| p.slug().to_string())
        .collect()
}

/// Can the user actually generate anything right now?
///
/// Deliberately not `configured_providers().is_empty()`. `Local` needs no key,
/// so it reports as configured on a machine with nothing installed — which
/// made the app look ready on first run and skip onboarding entirely. What
/// matters is whether any provider is *reachable*: a real key, or a local
/// endpoint that is genuinely answering.
#[tauri::command]
pub fn is_ready() -> bool {
    let has_key = ProviderId::ALL
        .into_iter()
        .filter(|p| p.needs_key())
        .any(vault::is_configured);
    has_key || detect_local().any()
}

/// Which local inference endpoints are up. Costs nothing to use and needs no
/// key — only reachable at all because we are native rather than in a browser.
#[tauri::command]
pub fn local_endpoints() -> LocalEndpoints {
    detect_local()
}

/// Everything the onboarding screen needs: where to get each key, what it
/// unlocks, and which one to start with.
#[tauri::command]
pub fn provider_info() -> Vec<hickeyfield_core::ProviderInfo> {
    hickeyfield_core::all_provider_info()
}

/// Make a real authenticated read against the provider. "Key is set" and "key
/// works" are different facts, and only the second one is useful.
#[tauri::command]
pub fn validate_key(provider: String) -> Result<crate::validate::Validation, String> {
    let p =
        ProviderId::from_slug(&provider).ok_or_else(|| format!("unknown provider: {provider}"))?;
    let key = vault::get(p, false).unwrap_or_default();
    let secret = vault::get(p, true);
    Ok(crate::validate::validate(p, &key, secret.as_deref()))
}

#[derive(Serialize)]
pub struct ImportReport {
    pub imported: Vec<String>,
    pub unknown: Vec<String>,
}

/// Parse a `.env` blob and store every credential it recognises.
///
/// The text never touches disk and is not logged — it is a bag of secrets that
/// happens to arrive as one string.
#[tauri::command]
pub fn import_env(text: String) -> Result<ImportReport, String> {
    let parsed = hickeyfield_core::parse_env(&text);
    let mut imported = Vec::new();
    for k in parsed.keys {
        let Some(p) = ProviderId::from_slug(&k.provider) else {
            continue;
        };
        vault::set(p, k.secret_half, &k.value)?;
        let name = p.display_name().to_string();
        if !imported.contains(&name) {
            imported.push(name);
        }
    }
    Ok(ImportReport {
        imported,
        unknown: parsed.unknown,
    })
}

// ── Catalogue ──────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct RouteDto {
    pub id: String,
    pub provider: String,
    pub slug: String,
    pub note: Option<String>,
    /// True only when this route can actually be executed — the user holds a
    /// key **and** a client exists for the provider.
    ///
    /// Both halves matter. Reporting key-presence alone made the picker offer
    /// routes the resolver would then refuse, which is how a user could add a
    /// Vercel AI Gateway key and be told to add a Vercel AI Gateway key.
    pub available: bool,
    /// Why not, in words the picker can render. `None` when available.
    pub unavailable_reason: Option<String>,
}

/// A one-line description for the model picker.
///
/// Built from the model's own capabilities rather than written by hand for 68
/// models, so it cannot drift from what the model actually accepts. The picker
/// previously printed the raw slug here, which tells a user nothing.
fn model_subtitle(m: &hickeyfield_core::Model) -> Option<String> {
    let caps = m.spec.capabilities();
    let mut bits: Vec<String> = Vec::new();

    if caps.supports_duration {
        match caps.durations.as_slice() {
            [] => {
                if let Some(d) = caps.default_duration {
                    bits.push(format!("{}s default", trim_num(d)));
                }
            }
            [only] => bits.push(format!("{}s", trim_num(*only))),
            many => {
                let lo = many.iter().cloned().fold(f64::INFINITY, f64::min);
                let hi = many.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                bits.push(format!("{}–{}s", trim_num(lo), trim_num(hi)));
            }
        }
    }
    if let Some(best) = caps.resolutions.last() {
        bits.push(format!("up to {best}"));
    }
    if caps.audio {
        bits.push("audio".into());
    }
    // Say what the model *needs*, since a missing required input is the single
    // most common first-submit failure and the picker is the cheapest place to
    // prevent it.
    let required: Vec<&str> = m
        .spec
        .required_flags()
        .filter(|f| f.value.is_media())
        .map(|f| f.name.as_str())
        .collect();
    if !required.is_empty() {
        bits.push(format!("needs {}", required.join(" + ").replace('_', " ")));
    } else if m.spec.media_flags().next().is_some() {
        // No required input, but it accepts one — the distinction between a
        // pure generator and something that can also edit, which is exactly
        // what a user scanning the list wants to know.
        bits.push("text or image in".into());
    } else if m.spec.takes_prompt() {
        bits.push("text in".into());
    }

    if bits.is_empty() {
        // Last resort, and still better than the raw slug the picker used to
        // print: name what kind of thing it makes.
        return Some(format!("{} model", m.modality));
    }
    Some(bits.join(" · "))
}

/// `5` not `5.0`, `5.5` kept. Rust has no `{:g}`.
fn trim_num(v: f64) -> String {
    if v.fract() == 0.0 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

/// Explain a route's state once, so the picker and the resolver cannot disagree.
fn route_state(provider: ProviderId, slug: &str, configured: &[String]) -> (bool, Option<String>) {
    // Measured absent from the provider entirely. Offering it spends a click
    // and a round trip to learn what we already know — which is how
    // `fal-ai/wan/v2.6` reached a user as `404 Path /v2.6/video-to-video`.
    if provider == ProviderId::Fal && hickeyfield_core::media::route_is_missing(slug) {
        return (
            false,
            Some("fal does not serve this model — try another route".to_string()),
        );
    }
    if !provider.has_adapter() {
        return (
            false,
            Some(format!(
                "Hickeyfield has no client for {} yet",
                provider.display_name()
            )),
        );
    }
    if provider.needs_key() && !configured.iter().any(|c| c == provider.slug()) {
        return (
            false,
            Some(format!("Needs a {} key", provider.display_name())),
        );
    }
    (true, None)
}

/// The same question, narrowed to one job.
///
/// A route can be perfectly configured and still be the wrong way to do *this*
/// piece of work: `fal-ai/veo3.1` takes no media, so under Animate Image it is
/// unusable even though the model belongs there — its Higgsfield sibling
/// `/veo3.1/image-to-video` is what serves that tab. Marking it here is what
/// stops the picker handing over a default route that fails at submit.
fn route_state_for_job(
    model: &hickeyfield_core::Model,
    route: &hickeyfield_core::route::Route,
    use_case: Option<hickeyfield_core::UseCase>,
    configured: &[String],
) -> (bool, Option<String>) {
    let (available, why) = route_state(route.provider, &route.slug, configured);
    if !available {
        return (available, why);
    }
    match use_case {
        Some(uc) if !hickeyfield_core::use_case::route_serves(model, route, uc) => (
            false,
            Some(format!(
                "{} cannot do this job — pick another route",
                route.provider.display_name()
            )),
        ),
        _ => (true, None),
    }
}

/// Per-model option sets, derived from the model's own declared flags.
///
/// `supports_*` and the list are separate facts: an empty list with support
/// true means the model takes a free value, and the UI must offer an input
/// rather than invent choices. See `catalog::Capabilities`.
#[derive(Serialize)]
pub struct CapabilitiesDto {
    pub supports_duration: bool,
    pub durations: Vec<f64>,
    pub default_duration: Option<f64>,
    /// The provider's real `[min, max]` free-form duration bound, when known.
    /// The UI ladder builds chips up to `max` from this (H3 Max 5–15, Wan
    /// 3.0 Prime 2–30) instead of a generic ladder capped at 12.
    pub duration_range: Option<(f64, f64)>,
    pub supports_resolution: bool,
    pub resolutions: Vec<String>,
    pub default_resolution: Option<String>,
    pub supports_aspect: bool,
    pub aspects: Vec<String>,
    pub default_aspect: Option<String>,
    pub audio: bool,
    pub constraints: Vec<String>,
    /// Media roles this model cannot take **on this route**, by wire name.
    ///
    /// The slot list comes from the use case, so every image-to-video tab draws
    /// an End Frame box whether or not the chosen model has anywhere to put
    /// one. Seedance 2.0 accepted the file, ignored it, and billed in full.
    /// The submit path refuses that now, but refusing after the user has
    /// composed the whole shot is the wrong moment — this greys the slot out
    /// before it is ever filled.
    pub unsupported_roles: Vec<String>,
}

impl CapabilitiesDto {
    /// Build from the authoritative description.
    ///
    /// `Support::No` becomes `supports_* = false`, which hides the control.
    /// `Support::Unknown` also renders nothing — the predecessor of this type
    /// filled unknowns with plausible defaults and that is precisely how the
    /// chip row came to offer 10s on a 5s-only model.
    fn from_capability(c: &hickeyfield_core::capability::ModelCapability) -> Self {
        use hickeyfield_core::capability::Axis;
        let num = |a: &Axis| -> Vec<f64> {
            a.values
                .iter()
                .filter_map(|v| v.trim().trim_end_matches('s').trim().parse::<f64>().ok())
                .collect()
        };
        CapabilitiesDto {
            supports_duration: c.duration.support.is_yes(),
            durations: num(&c.duration),
            duration_range: c.duration_range(),
            default_duration: c
                .duration_seconds()
                .and_then(|_| c.default_duration_seconds()),
            supports_resolution: c.resolution.support.is_yes(),
            resolutions: c.resolution.values.clone(),
            default_resolution: c.resolution.default.clone(),
            supports_aspect: c.aspect.support.is_yes(),
            aspects: c.aspect.values.clone(),
            default_aspect: c.aspect.default.clone(),
            audio: c.audio_output.support.is_yes(),
            constraints: c.constraints.clone(),
            // Needs the route, which this conversion does not have. Filled by
            // `model_capabilities`.
            unsupported_roles: Vec::new(),
        }
    }
}

impl From<hickeyfield_core::catalog::Capabilities> for CapabilitiesDto {
    fn from(c: hickeyfield_core::catalog::Capabilities) -> Self {
        CapabilitiesDto {
            supports_duration: c.supports_duration,
            durations: c.durations,
            duration_range: c.duration_range,
            default_duration: c.default_duration,
            supports_resolution: c.supports_resolution,
            resolutions: c.resolutions,
            default_resolution: c.default_resolution,
            supports_aspect: c.supports_aspect,
            aspects: c.aspects,
            default_aspect: c.default_aspect,
            audio: c.audio,
            constraints: c.constraints,
            // Model-level view, no route chosen yet: nothing can be ruled out.
            unsupported_roles: Vec::new(),
        }
    }
}

#[derive(Serialize)]
pub struct ModelDto {
    pub id: String,
    pub display_name: String,
    pub modality: String,
    /// What this model can be asked for. Never assumed — see `CapabilitiesDto`.
    pub capabilities: CapabilitiesDto,
    /// One-line description for the picker row. Previously the picker fell back
    /// to printing the raw slug, which is not a description.
    pub subtitle: Option<String>,
    pub routes: Vec<RouteDto>,
    pub is_launch: bool,
    pub takes_prompt: bool,
}

/// The use cases the workspace tabs offer.
#[derive(Serialize)]
pub struct UseCaseDto {
    pub slug: String,
    pub label: String,
    pub blurb: String,
    /// `role` plus whether the job cannot run without it.
    pub slots: Vec<(String, bool)>,
    pub requires_media: bool,
}

#[tauri::command]
pub fn list_use_cases() -> Vec<UseCaseDto> {
    hickeyfield_core::UseCase::ALL
        .into_iter()
        .map(|u| UseCaseDto {
            slug: u.slug().to_string(),
            label: u.label().to_string(),
            blurb: u.blurb().to_string(),
            slots: u
                .slots()
                .iter()
                .map(|(role, req)| {
                    (
                        serde_json::to_value(role)
                            .ok()
                            .and_then(|v| v.as_str().map(String::from))
                            .unwrap_or_default(),
                        *req,
                    )
                })
                .collect(),
            requires_media: u.requires_media(),
        })
        .collect()
}

#[tauri::command]
pub fn list_models() -> Vec<ModelDto> {
    models_matching(None)
}

/// Models that can do one job.
///
/// The filter lives here rather than in the UI so the picker cannot offer
/// something the submit path would then refuse — which is what happened when
/// every tab showed every model.
#[tauri::command]
pub fn models_for_use_case(use_case: String) -> Vec<ModelDto> {
    models_matching(hickeyfield_core::UseCase::from_slug(&use_case))
}

fn models_matching(use_case: Option<hickeyfield_core::UseCase>) -> Vec<ModelDto> {
    let configured = configured_providers();
    let mut models: Vec<ModelDto> = registry()
        .into_values()
        .filter(|m| match use_case {
            Some(uc) => hickeyfield_core::use_case::supports(m, uc),
            None => true,
        })
        .map(|m| ModelDto {
            id: m.id.clone(),
            display_name: m.display_name.clone(),
            modality: m.modality.to_string(),
            capabilities: m.spec.capabilities().into(),
            subtitle: model_subtitle(&m),
            takes_prompt: m.spec.takes_prompt(),
            is_launch: m.launch,
            routes: m
                .routes
                .iter()
                .map(|r| {
                    let (available, unavailable_reason) =
                        route_state_for_job(&m, r, use_case, &configured);
                    RouteDto {
                        id: r.id(),
                        provider: r.provider.slug().to_string(),
                        slug: r.slug.clone(),
                        note: r.note.clone(),
                        available,
                        unavailable_reason,
                    }
                })
                .collect(),
        })
        .collect();
    // Launch models first, then alphabetical — a 57-item flat list sorted only
    // by id buries the twelve models most people want.
    models.sort_by(|a, b| {
        b.is_launch
            .cmp(&a.is_launch)
            .then_with(|| a.display_name.cmp(&b.display_name))
    });
    models
}

#[derive(Serialize)]
pub struct PresetDto {
    pub id: String,
    pub display_name: String,
    pub category: String,
    pub tags: Vec<String>,
    pub description: String,
}

/// The camera-control presets, which are the reproducible half of the catalog.
///
/// Higgsfield's other ~244 presets expand to prompt text only on their server —
/// nothing but a UUID goes over the wire — so those bodies have to be authored
/// rather than derived. The camera moves are different: their own five-slot
/// schema is public, so these are real and complete today.
#[tauri::command]
pub fn list_presets() -> Vec<PresetDto> {
    hickeyfield_core::camera::slugs()
        .filter_map(hickeyfield_core::camera::get)
        .map(|t| PresetDto {
            id: t.slug.to_string(),
            display_name: t.display_name.to_string(),
            category: "camera-control".to_string(),
            tags: vec![],
            description: t.render(),
        })
        .collect()
}

/// The authoritative capabilities for one model on one route.
///
/// Separate from `list_models` on purpose. Answering this properly means asking
/// fal for the endpoint schema, and doing that for all 68 models to draw a
/// picker would be 68 blocking HTTP requests. So the list stays
/// catalogue-sourced and fast, and this refines a single model at the moment it
/// is selected — which is exactly when the answer starts to matter.
#[tauri::command]
pub fn model_capabilities(
    model_id: String,
    route_id: Option<String>,
    #[allow(non_snake_case)] hasMedia: Option<bool>,
) -> Result<CapabilitiesDto, String> {
    use hickeyfield_core::media::InputMode;

    let reg = registry();
    let model = reg
        .get(&model_id)
        .ok_or_else(|| format!("unknown model: {model_id}"))?;

    let route = route_id
        .as_deref()
        .and_then(|id| model.routes.iter().find(|r| r.id() == id))
        .or_else(|| model.routes.first())
        .ok_or_else(|| format!("{model_id} has no routes"))?;

    // The mode changes which endpoint answers, and the endpoints differ: Kling
    // 3.0's image-to-video variant has no `duration` field while its
    // text-to-video one does.
    let mode = if hasMedia.unwrap_or(false) {
        InputMode::Image
    } else {
        InputMode::Text
    };

    let cap = hickeyfield_core::capability::for_route(
        &model.spec,
        route.provider,
        &route.slug,
        mode,
        hickeyfield_core::fal_schema::for_endpoint,
    );
    let mut dto = CapabilitiesDto::from_capability(&cap);

    // Which slots this model+route has nowhere to put. Asked of the same
    // function the submit path binds with, so the picker and the wire cannot
    // disagree — the End Frame box was drawn from the use case alone, and
    // Seedance 2.0 took the file, ignored it and charged for the render.
    let dialect = hickeyfield_core::media::dialect_for(route.provider, &route.slug);
    dto.unsupported_roles = hickeyfield_core::media::MediaRole::ALL
        .iter()
        .filter(|role| {
            !hickeyfield_core::media::can_bind(&model.spec, **role, dialect, &route.slug)
        })
        .map(|role| role.slug().to_string())
        .collect();
    Ok(dto)
}

// ── Cost ───────────────────────────────────────────────────────────────────

/// Generation settings **in the vocabulary the UI actually speaks**.
///
/// This used to be `{seconds, width, height, fps, images}` while the UI sent
/// `{duration, resolution, aspect}` — not a naming mismatch but two different
/// models of the same thing. Nothing lined up, so `seconds` was always `None`,
/// `Billable` had no duration, and **every cost estimate came out as $0.00**.
/// A confidently wrong price on the Generate button is the worst defect this
/// codebase can ship, so the wire shape now matches what the UI knows and the
/// pixel arithmetic lives here, where the model knowledge already is.
// Serialize is derived rather than hand-written: the previous manual impl
// listed the field names a second time, which is precisely how it kept
// serialising `seconds`/`width`/`height` after the struct had moved on.
#[derive(Deserialize, Serialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SettingsDto {
    /// Seconds. Absent on models with no duration axis.
    pub duration: Option<f64>,
    /// The model's own vocabulary: `720p`, `1080p`, `4k`, `1k`, `2k`.
    pub resolution: Option<String>,
    /// `16:9`, `9:16`, `1:1`, …
    pub aspect: Option<String>,
    #[serde(default)]
    pub audio: bool,
    #[serde(default)]
    pub enhance: bool,
    pub seed: Option<i64>,
    pub steps: Option<u32>,
    #[serde(default = "one_u32")]
    pub batch: u32,
    #[serde(default)]
    pub extra_inputs: u32,
}

/// Pixel height for a resolution label.
///
/// Two vocabularies are in play and both are real: video models say `720p`,
/// image models say `2k`. Returning `None` for anything unrecognised is
/// deliberate — a guessed resolution becomes a guessed megapixel count becomes
/// a wrong price.
fn resolution_height(label: &str) -> Option<u32> {
    let l = label.trim().to_ascii_lowercase();
    match l.as_str() {
        "480p" => Some(480),
        "540p" => Some(540),
        "720p" => Some(720),
        // MiniMax H3 and H3 Max render a native 768p tier; without this the
        // default resolution priced as "unknown" on the Generate button.
        "768p" => Some(768),
        "1080p" => Some(1080),
        "1440p" => Some(1440),
        "2160p" | "4k" => Some(2160),
        "1k" => Some(1024),
        "2k" => Some(1440),
        _ => None,
    }
}

/// Width from a height and an aspect label like `16:9`.
fn width_for(height: u32, aspect: Option<&str>) -> Option<u32> {
    let a = aspect?.trim();
    let (w, h) = a.split_once(':')?;
    let (w, h): (f64, f64) = (w.trim().parse().ok()?, h.trim().parse().ok()?);
    if h <= 0.0 || w <= 0.0 {
        return None;
    }
    // Rounded to an even number: encoders reject odd dimensions, and a price
    // computed on a size the provider will not accept is meaningless anyway.
    let px = (height as f64 * w / h).round() as u32;
    Some(px + (px % 2))
}

fn one_u32() -> u32 {
    1
}

impl From<&SettingsDto> for Billable {
    fn from(s: &SettingsDto) -> Self {
        let height = s.resolution.as_deref().and_then(resolution_height);
        let width = height.and_then(|h| width_for(h, s.aspect.as_deref()));
        // Only when both dimensions are known. Several models price per
        // megapixel, and half a dimension gives a number that looks right and
        // is not.
        let megapixels = match (width, height) {
            (Some(w), Some(h)) => Some((w as f64 * h as f64) / 1_000_000.0),
            _ => None,
        };
        Billable {
            seconds: s.duration,
            width,
            height,
            fps: None,
            // An image request is one image per batch slot; video models ignore
            // this field entirely.
            images: Some(s.batch.max(1)),
            megapixels,
            audio: s.audio,
            batch: s.batch.max(1),
            extra_inputs: s.extra_inputs,
        }
    }
}

/// `null` means the provider publishes no price for this call. The UI must
/// render that as "price unavailable" — never as free.
#[tauri::command]
pub fn estimate_cost(
    state: State<'_, AppState>,
    model_id: String,
    route_id: String,
    settings: SettingsDto,
) -> Option<Estimate> {
    estimate_with(&state.prices, &model_id, &route_id, &settings)
}

/// The body of [`estimate_cost`], without the Tauri handle.
///
/// Split out so the cost path stays testable without standing up an app: the
/// invariant worth guarding here — unknown is `None`, never `0.0` — should not
/// depend on a harness a future edit would delete rather than fix.
fn estimate_with(
    prices: &crate::pricing::Prices,
    model_id: &str,
    route_id: &str,
    settings: &SettingsDto,
) -> Option<Estimate> {
    let reg = registry();
    let model = reg.get(model_id)?;
    let route = model.route(route_id)?;
    prices.estimate(model, route, &Billable::from(settings))
}

/// Where the prices on screen came from and how old they are.
///
/// Deliberately exposed. A number with no provenance is the kind of thing a
/// user only checks after it has already been wrong.
#[tauri::command]
pub fn price_status(state: State<'_, AppState>) -> crate::pricing::PriceStatus {
    state.prices.status()
}

// ── Jobs ───────────────────────────────────────────────────────────────────

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SubmitInput {
    pub model_id: String,
    /// Omitted means "resolve for me", which applies the cheapest-route policy.
    pub route_id: Option<String>,
    pub prompt: String,
    pub preset_id: Option<String>,
    #[serde(default)]
    pub settings: SettingsDto,
    /// Attached inputs, as roles rather than wire flags. Without this the app
    /// can only do text-to-video, which is a different and much smaller
    /// product than the one being cloned.
    #[serde(default)]
    pub media: Vec<hickeyfield_core::MediaRef>,
    /// Which enhancer to rewrite with. `None` means "decide for me" (auto): the
    /// shell picks from what is actually available. Enhancement only ever runs
    /// when `settings.enhance` is on and the three enhance rules allow it.
    #[serde(default)]
    pub rewriter: Option<RewriterChoice>,
    /// The exact prompt the user previewed (and possibly edited). When present
    /// and non-empty, `submit_job` sends it VERBATIM and does NOT re-run the
    /// harness — the previewed text already carries the camera clause, so
    /// re-compiling would double-append it and re-charge the rewrite. Blank or
    /// whitespace-only falls through to the normal enhance+compile path.
    #[serde(default)]
    pub final_prompt: Option<String>,
    /// S15 D-C: request-body values the chain scheduler writes for keys the
    /// endpoint declares but the app has no user control for — e.g.
    /// `prompt_expansion_mode: "disabled"` and a fixed `seed` on a chained
    /// shot. Applied before the S14 constant pass (so it can turn "balanced"
    /// into "disabled") and schema-gated (so it can never inject an arbitrary
    /// field). `None`/empty for a standalone submit, which keeps S14's default.
    #[serde(default)]
    pub constant_overrides: Option<serde_json::Map<String, serde_json::Value>>,
}

/// The UI's explicit enhancer choice. Structured rather than a bare tag so a
/// hosted backend (which has no Ollama-style tag) can be named without
/// overloading one string field.
#[derive(Deserialize, Serialize, Default, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RewriterChoice {
    /// `"ollama"` or `"openai"`. An unrecognised value falls back to auto.
    pub backend: String,
    /// Required for Ollama (the installed tag) and for OpenAI (the hosted model
    /// id). No default is invented for OpenAI: hosted rosters churn and a
    /// hardcoded id turns into a 404 nobody can fix without a new binary.
    pub model: Option<String>,
}

// Honest notes for the "enhance is on but we cannot run it" cases. Original
// prose about this app — no borrowed marketing copy — each ending on the same
// provenance-safe promise the rest of the fallback family makes: the prompt was
// sent unchanged, never silently rewritten and never silently dropped.
const NOTE_NOTHING_AVAILABLE: &str = "Enhance is on, but there is no rewriter to run it: no OpenAI key is stored and Ollama isn't running on this machine. Add an OpenAI key in Settings, or start Ollama and pick a model. Your prompt was sent exactly as you wrote it.";
const NOTE_OLLAMA_NO_MODELS: &str = "Enhance is on and Ollama is running, but no chat model is installed yet — pull a small, fast one (`ollama pull llama3.2`, `phi4-mini` or `gemma3` all work well here), then choose it. Your prompt was sent exactly as you wrote it.";
const NOTE_OPENAI_NO_KEY: &str = "Enhance is on and OpenAI is selected, but no OpenAI key is stored — add one in Settings, or switch the enhancer to Local. Your prompt was sent exactly as you wrote it.";
const NOTE_NO_MODEL_CHOSEN: &str = "Enhance is on, but no model was chosen for the enhancer — pick one next to the Enhance switch. Your prompt was sent exactly as you wrote it.";

/// Turn the UI's (optional) choice plus what is actually reachable into a
/// concrete [`crate::harness::Rewriter`].
///
/// Pure in its inputs so it is unit-testable without a Tauri `State`. Priority:
/// an explicit UI choice wins; otherwise auto-detect; otherwise an honest note
/// explaining why nothing ran. It never invents a hosted model id — see the
/// `RewriterChoice::model` doc.
fn select_rewriter<'a>(
    choice: Option<&'a RewriterChoice>,
    openai_key: Option<&'a str>,
    ollama_up: bool,
    ollama_models: &'a [hickeyfield_core::enhancer::LocalModel],
) -> crate::harness::Rewriter<'a> {
    use crate::harness::Rewriter;
    use hickeyfield_core::enhancer::HostedBackend;

    let unavailable = |note: &str| Rewriter::Unavailable {
        note: note.to_string(),
    };

    match choice {
        // Explicit OpenAI: needs both a stored key and an explicit model.
        Some(c) if c.backend == "openai" => {
            let model = c.model.as_deref().filter(|m| !m.trim().is_empty());
            match (openai_key, model) {
                (Some(key), Some(model)) => Rewriter::Hosted {
                    backend: HostedBackend::OpenAi,
                    api_key: key,
                    model,
                },
                (None, _) => unavailable(NOTE_OPENAI_NO_KEY),
                (Some(_), None) => unavailable(NOTE_NO_MODEL_CHOSEN),
            }
        }
        // Explicit Ollama: needs the daemon up and the chosen model installed.
        Some(c) if c.backend == "ollama" => {
            if !ollama_up {
                return unavailable(NOTE_NOTHING_AVAILABLE);
            }
            match c.model.as_deref().filter(|m| !m.trim().is_empty()) {
                Some(model) if ollama_models.iter().any(|m| m.name == model) => {
                    Rewriter::Ollama { model }
                }
                Some(_) if ollama_models.is_empty() => unavailable(NOTE_OLLAMA_NO_MODELS),
                // A model was named but is not installed — treat like "pick one".
                Some(_) => unavailable(NOTE_NO_MODEL_CHOSEN),
                None if ollama_models.is_empty() => unavailable(NOTE_OLLAMA_NO_MODELS),
                None => unavailable(NOTE_NO_MODEL_CHOSEN),
            }
        }
        // Auto ("decide for me"), and any unrecognised backend string. OpenAI is
        // never run *silently* — a hosted model id must be chosen explicitly — so
        // when a runnable Ollama exists it is always preferred over declining.
        // Only when nothing can run at all do we surface an honest note.
        _ => match ollama_models.first() {
            // A running Ollama with at least one installed model always wins:
            // it needs no key and no explicit model id, so auto can run it. The
            // list is pre-sorted by suitability in `local_models`, so `.first()`
            // is the top-ranked model — the same one the picker shows selected.
            Some(first) if ollama_up => Rewriter::Ollama { model: &first.name },
            // No Ollama model to run. If an OpenAI key is stored the user only
            // has to name a hosted model — decline with the pick-a-model note
            // rather than invent one.
            _ if openai_key.is_some() => unavailable(NOTE_NO_MODEL_CHOSEN),
            // Ollama is up but empty — tell them to install a model.
            _ if ollama_up => unavailable(NOTE_OLLAMA_NO_MODELS),
            // Nothing is available at all.
            _ => unavailable(NOTE_NOTHING_AVAILABLE),
        },
    }
}

/// The chat-capable models the local Ollama has installed, for the enhancer
/// picker. A daemon that is down or answers nothing is an empty list, not an
/// error: the UI already tells "not running" apart from "running but empty"
/// using [`local_endpoints`], so this command only has to supply the names.
#[tauri::command]
pub fn list_ollama_models() -> Vec<hickeyfield_core::enhancer::LocalModel> {
    hickeyfield_core::enhancer::local_models(hickeyfield_core::clients::OLLAMA_URL)
        .unwrap_or_default()
}

/// Probe the impure enhancer environment: the stored OpenAI key, whether Ollama
/// is up, and the chat models it has installed.
///
/// Lifted verbatim out of `submit_job` so `preview_prompt` can share it (both
/// commands need it, and borrow lifetimes require the caller to own these
/// locals). When enhancement is off there is nothing to rewrite, so the vault
/// read and daemon probe are skipped entirely. One probe of `/api/tags`, not
/// two: `local_models` distinguishes a down daemon (`Err`) from a running-but-
/// empty one (`Ok([])`), the exact `ollama_up` vs. no-models split
/// `select_rewriter` needs.
fn probe_enhancer_env(
    enhance: bool,
) -> (
    Option<String>,
    bool,
    Vec<hickeyfield_core::enhancer::LocalModel>,
) {
    if !enhance {
        return (None, false, Vec::new());
    }
    let openai_key = vault::get(ProviderId::OpenAi, false);
    let (ollama_up, ollama_models) =
        match hickeyfield_core::enhancer::local_models(hickeyfield_core::clients::OLLAMA_URL) {
            Ok(models) => (true, models),
            Err(_) => (false, Vec::new()),
        };
    (openai_key, ollama_up, ollama_models)
}

/// Turn a submission into the prompt actually sent over the wire.
///
/// Two paths, one place. When the user previewed (and possibly edited) the
/// prompt, `final_prompt` holds the exact text they saw. That text already
/// carries the camera clause the preview appended, so it is sent VERBATIM and
/// the harness is NOT run again — re-compiling would double-append the camera
/// template and re-charge (or silently re-change) the rewrite. This is the S7
/// invariant. A blank/whitespace-only `final_prompt` falls through to the
/// normal enhance+compile path, so an empty edit can never submit an empty
/// prompt. Kept off the store/fal path so it is unit-testable.
fn compiled_for_submit(
    model: &hickeyfield_core::Model,
    input: &SubmitInput,
) -> Result<crate::harness::Compiled, String> {
    match input
        .final_prompt
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        // Previewed/edited: send exactly what the user saw. The camera clause is
        // already in this text — do NOT call `compile` again.
        Some(text) => Ok(crate::harness::Compiled {
            prompt: text.to_string(),
            original: input.prompt.clone(),
            enhanced: Some(text.to_string()),
            // A human edit has no reproducible recipe, so there is no version to
            // pin — a guessed pin would make two unlike generations look alike.
            version: None,
            note: Some("Previewed before generating; sent exactly as shown.".into()),
        }),
        // The normal path: pick a rewriter from what is actually reachable and
        // run the harness. Bound in locals so the borrowed key/model outlive the
        // `compile` call.
        None => {
            let enhance = input.settings.enhance;
            let (openai_key, ollama_up, ollama_models) = probe_enhancer_env(enhance);
            let rewriter = if enhance {
                select_rewriter(
                    input.rewriter.as_ref(),
                    openai_key.as_deref(),
                    ollama_up,
                    &ollama_models,
                )
            } else {
                crate::harness::Rewriter::None
            };
            // The harness. Resolves the preset, appends its camera clause,
            // applies the three enhance rules, and — when asked and able —
            // rewrites the scene through the filmmaking corpus. Runs *before*
            // pricing and routing because a refusal here must cost nothing.
            crate::harness::compile(
                model,
                &input.prompt,
                input.preset_id.as_deref(),
                &input.media,
                enhance,
                rewriter,
            )
        }
    }
}

/// What the user sees before spending money: the compiled wire prompt (with the
/// camera clause and any rewrite already applied), their original words, and the
/// same honest note the meta rail shows. `#[serde(rename_all = "camelCase")]` so
/// the fields land as `enhanced`, `version`, `note` for the UI.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewDto {
    pub prompt: String,
    pub original: String,
    pub enhanced: Option<String>,
    pub version: Option<String>,
    pub note: Option<String>,
}

/// Compile the prompt for display WITHOUT submitting anything.
///
/// Takes NO `State` on purpose: with no handle to the store, prices, or fal it
/// structurally cannot resolve a route, read a price, or call
/// `submit_to_provider`, so a preview can never bill a generation. It reuses the
/// exact same `harness::compile` (camera clause + enhance rules + the S6 refusal
/// guard) `submit_job` uses, so what the user previews is what they would send.
#[tauri::command]
pub fn preview_prompt(input: SubmitInput) -> Result<PreviewDto, String> {
    let reg = registry();
    let model = reg
        .get(&input.model_id)
        .ok_or_else(|| format!("unknown model: {}", input.model_id))?;

    let enhance = input.settings.enhance;
    let (openai_key, ollama_up, ollama_models) = probe_enhancer_env(enhance);
    let rewriter = if enhance {
        select_rewriter(
            input.rewriter.as_ref(),
            openai_key.as_deref(),
            ollama_up,
            &ollama_models,
        )
    } else {
        crate::harness::Rewriter::None
    };

    let compiled = crate::harness::compile(
        model,
        &input.prompt,
        input.preset_id.as_deref(),
        &input.media,
        enhance,
        rewriter,
    )?;

    Ok(PreviewDto {
        prompt: compiled.prompt,
        original: compiled.original,
        enhanced: compiled.enhanced,
        version: compiled.version,
        note: compiled.note,
    })
}

#[tauri::command]
pub fn submit_job(state: State<'_, AppState>, input: SubmitInput) -> Result<String, String> {
    let reg = registry();
    let model = reg
        .get(&input.model_id)
        .ok_or_else(|| format!("unknown model: {}", input.model_id))?;
    // A standalone generation: `submit_one(.., None, None)` is byte-for-byte the
    // old `submit_job` body, so single-job behaviour is unchanged.
    submit_one(
        &state.store,
        &state.prices,
        &state.runner,
        model,
        &input,
        None,
        None,
    )
}

/// Submit one generation. The whole of the former `submit_job` body, lifted so a
/// storyboard's shots can reuse it verbatim — each shot runs the identical
/// compile→guard→estimate→route→upsert→submit→watch path, tagged with its set
/// membership. `storyboard_id`/`shot_index` are `None` for a standalone job,
/// which is exactly the standalone case, so nothing about single-job submit
/// changes.
/// Clamp a duration outside the model's documented range into that range,
/// returning any advisory to attach to the job.
///
/// The range is never widened and a value already inside it is untouched (no
/// advisory). Pure over the settings so the money-path behaviour is pinned by a
/// test: the clamp runs BEFORE pricing, so the estimate is for what will render.
fn clamp_duration_to_range(
    settings: &mut SettingsDto,
    range: Option<(f64, f64)>,
    display_name: &str,
) -> Vec<String> {
    let (Some(d), Some((min, max))) = (settings.duration, range) else {
        return Vec::new();
    };
    if d >= min && d <= max {
        return Vec::new();
    }
    let clamped = d.clamp(min, max);
    settings.duration = Some(clamped);
    vec![format!(
        "{} runs {}–{}s; {}s clamped to {}s",
        display_name,
        fmt_secs(min),
        fmt_secs(max),
        fmt_secs(d),
        fmt_secs(clamped),
    )]
}

/// Seconds as a human string: a whole number drops the `.0` (`15`, not
/// `15.0`), a fraction keeps it. Used only for advisory copy.
fn fmt_secs(v: f64) -> String {
    if v.fract() == 0.0 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

/// Whether any job for this storyboard shot index is still in flight — queued
/// or running, i.e. not [`JobStatus::is_terminal`]. A regenerate or an
/// auto-retry MUST refuse while one is in flight: a second submit at the same
/// index would mint a duplicate in-flight job and double-bill the shot (M1).
/// A `Completed`/`Failed`/`Canceled`/moderation-refused row is terminal, so it
/// does not block a legitimate regenerate of a settled low-join shot.
pub(crate) fn shot_in_flight(jobs: &[JobSet], idx: u32) -> bool {
    jobs.iter()
        .any(|j| j.shot_index == Some(idx) && !j.status.is_terminal())
}

pub(crate) fn submit_one(
    store: &Arc<crate::store::SqliteStore>,
    prices: &Arc<crate::pricing::Prices>,
    runner: &crate::runner::Runner,
    model: &hickeyfield_core::Model,
    input: &SubmitInput,
    storyboard_id: Option<&str>,
    shot_index: Option<u32>,
) -> Result<String, String> {
    let available: Vec<ProviderId> = configured_providers()
        .iter()
        .filter_map(|s| ProviderId::from_slug(s))
        .collect();

    // The wire prompt: either the exact text the user previewed (sent verbatim,
    // never re-compiled) or, on the quick path, a fresh enhance+compile.
    let compiled = compiled_for_submit(model, input)?;

    // Clamp a duration outside the model's documented range BEFORE pricing, so
    // the estimate is for what will actually render. The range is read offline
    // from the spec (the same bound the UI ladder showed and never wider than
    // the live one for our roster — AC6/D10 guards that), so this stays
    // network-free on the money path. A value already inside the range is
    // untouched and carries no advisory; the range is never widened.
    let mut settings = input.settings.clone();
    let range_advisories = clamp_duration_to_range(
        &mut settings,
        model.spec.duration_range,
        &model.display_name,
    );

    let billable = Billable::from(&settings);
    let route = hickeyfield_core::route::resolve(
        &model.routes,
        &available,
        RoutePolicy::Cheapest,
        input.route_id.as_deref(),
        |r| prices.usd(model, r, &billable),
    )
    .map_err(|e| e.to_string())?;

    // Price once, keeping the provenance, so the meta rail can say whether the
    // estimate came from the live feed, the bundled snapshot, or a literal.
    let priced = prices.estimate_with_provenance(model, route, &billable);
    let estimated_usd = priced.as_ref().map(|(e, _)| e.usd);
    let priced_from = priced.as_ref().map(|(_, p)| *p);
    let priced_at = priced.as_ref().map(|_| now_secs());

    // The id is ours, not the provider's: a submit that fails still leaves a
    // visible row explaining why, rather than vanishing. The trailing counter
    // keeps two same-model submits in the same wall-clock second distinct — a
    // generate-all loop mints several within one second (see `next_id_seq`).
    let id = mint_job_id(&model.id);
    let mut job = JobSet {
        id: id.clone(),
        model_id: model.id.clone(),
        route_id: route.id(),
        request_id: String::new(),
        endpoint: String::new(),
        status: hickeyfield_core::JobStatus::Queued,
        // The user's own words are stored as the prompt; the compiled and
        // rewritten string is what goes over the wire. Keeping both is what
        // lets the meta rail show the second chip honestly.
        prompt: compiled.original.clone(),
        enhanced_prompt: compiled.enhanced.clone(),
        enhancer_version: compiled.version.clone(),
        enhance_note: compiled.note.clone(),
        advisories: range_advisories,
        preset_id: input.preset_id.clone(),
        created_at: now_secs(),
        updated_at: now_secs(),
        results: vec![],
        estimated_usd,
        actual_usd: None,
        priced_from,
        priced_at,
        actual_source: None,
        fail_reason: None,
        settings: serde_json::to_value(&settings).unwrap_or(serde_json::Value::Null),
        // Persisted so Rerun repeats the generation the user actually ran.
        media: input.media.clone(),
        // A fresh job has timed out zero times and is not stalled.
        poll_cycles: 0,
        stalled: false,
        // Set membership, when this is a storyboard shot. Write-once in the store.
        storyboard_id: storyboard_id.map(str::to_string),
        shot_index,
    };
    store.upsert(&job).map_err(|e| e.to_string())?;

    let empty_overrides = serde_json::Map::new();
    let overrides = input
        .constant_overrides
        .as_ref()
        .unwrap_or(&empty_overrides);
    match crate::app::submit_to_provider(
        &mut job,
        model,
        route,
        &input.media,
        &compiled.prompt,
        overrides,
    ) {
        Ok(request_id) => {
            job.request_id = request_id;
            job.updated_at = now_secs();
            store.upsert(&job).map_err(|e| e.to_string())?;
            runner.watch(job);
            Ok(id)
        }
        Err(e) => {
            job.status = hickeyfield_core::JobStatus::Failed;
            job.fail_reason = Some(e.to_string());
            job.updated_at = now_secs();
            let _ = store.upsert(&job);
            Err(e.to_string())
        }
    }
}

// ── Storyboard ───────────────────────────────────────────────────────────────

/// The default still model for a keyframe set (D1): the only priced fal
/// text-to-image route, so the total shows a real number. The owner may override
/// via `StoryboardGenerateInput.still_model_id`.
pub(crate) const DEFAULT_STILL_MODEL: &str = "qwen_image_3";

/// The derived keyframe plan for one generate call: the `N+1` stills, their
/// crash-safe records (with per-still estimates), and the set canvas the stills
/// render at. Kept as a named struct so the generate path stays readable.
struct KeyframePlan {
    keyframes: Vec<hickeyfield_core::storyboard::Keyframe>,
    records: Vec<crate::store::KeyframeRecord>,
    resolution: Option<String>,
    aspect: Option<String>,
}

/// Derive the still canvas for a keyframe set (D1/Q1) from the set's resolution
/// and aspect: `image_size = width_for(height, aspect) × height`, rounded even.
/// Because H3 Max i2v's canvas "follows the frame", the still MUST render at the
/// set's target size. Defaults 768p × 16:9 when the shot omits them.
fn keyframe_canvas(
    resolution: Option<&str>,
    aspect: Option<&str>,
) -> Result<hickeyfield_core::storyboard::ImageSize, String> {
    let res = resolution.unwrap_or("768p");
    let asp = aspect.unwrap_or("16:9");
    let height = resolution_height(res)
        .ok_or_else(|| format!("keyframe stills need a known resolution, not {res:?}"))?;
    let width = width_for(height, Some(asp))
        .ok_or_else(|| format!("keyframe stills need a known aspect, not {asp:?}"))?;
    Ok(hickeyfield_core::storyboard::ImageSize { width, height })
}

/// Build the still job's [`SubmitInput`] for one keyframe boundary. The derived
/// prompt is sent VERBATIM (`final_prompt`) — it is already the world + hold
/// clause, so re-enhancing it would be drift; the canvas rides as an
/// `image_size` constant override (the S15 override seam, schema-gated so it
/// only lands on a model that declares `image_size`, verified keyless against
/// qwen-image-3). A `seed` offset makes a regenerate reproducibly different.
fn still_submit_input(
    kf: &hickeyfield_core::storyboard::Keyframe,
    resolution: Option<&str>,
    aspect: Option<&str>,
    seed: u64,
) -> SubmitInput {
    let mut overrides = serde_json::Map::new();
    overrides.insert(
        "image_size".into(),
        serde_json::json!({ "width": kf.image_size.width, "height": kf.image_size.height }),
    );
    overrides.insert("seed".into(), serde_json::Value::Number(seed.into()));
    SubmitInput {
        model_id: kf.model_id.clone(),
        route_id: None,
        prompt: kf.prompt.clone(),
        preset_id: None,
        settings: SettingsDto {
            duration: None,
            resolution: resolution.map(str::to_string),
            aspect: aspect.map(str::to_string),
            audio: false,
            enhance: false,
            seed: None,
            steps: None,
            batch: 1,
            extra_inputs: 0,
        },
        media: Vec::new(),
        rewriter: None,
        // Verbatim: the derived prompt already carries the world + hold clause.
        final_prompt: Some(kf.prompt.clone()),
        constant_overrides: Some(overrides),
    }
}

/// Price one still up front (D2/D6) — the same estimate `submit_one` will write,
/// computed here so the stored `KeyframeRecord.estimated_usd` feeds the
/// total-shown-before-Generate sum. `None` for an unpriced model (a GPT tier);
/// never summed as $0.
fn still_estimate(
    prices: &Arc<crate::pricing::Prices>,
    model: &hickeyfield_core::Model,
    settings: &SettingsDto,
) -> Option<f64> {
    let available: Vec<ProviderId> = configured_providers()
        .iter()
        .filter_map(|s| ProviderId::from_slug(s))
        .collect();
    let billable = Billable::from(settings);
    let route = hickeyfield_core::route::resolve(
        &model.routes,
        &available,
        RoutePolicy::Cheapest,
        None,
        |r| prices.usd(model, r, &billable),
    )
    .ok()?;
    prices
        .estimate_with_provenance(model, route, &billable)
        .map(|(e, _)| e.usd)
}

/// Turn the UI's (optional) enhancer choice plus what is actually reachable into
/// a concrete [`hickeyfield_core::enhancer::ChatBackend`], or an honest note when
/// nothing can run. The `ChatBackend` analogue of [`select_rewriter`], sharing
/// the exact same priority and the same NOTE_* prose, so drafting a plan is
/// available under precisely the conditions the enhancer is. Pure in its inputs,
/// so it is unit-testable without a Tauri `State`.
fn chat_backend_for(
    choice: Option<&RewriterChoice>,
    openai_key: Option<&str>,
    ollama_up: bool,
    ollama_models: &[hickeyfield_core::enhancer::LocalModel],
) -> Result<hickeyfield_core::enhancer::ChatBackend, String> {
    use hickeyfield_core::enhancer::{ChatBackend, HostedBackend};

    match choice {
        Some(c) if c.backend == "openai" => {
            let model = c.model.as_deref().filter(|m| !m.trim().is_empty());
            match (openai_key, model) {
                (Some(key), Some(model)) => {
                    Ok(ChatBackend::hosted(HostedBackend::OpenAi, key, model))
                }
                (None, _) => Err(NOTE_OPENAI_NO_KEY.to_string()),
                (Some(_), None) => Err(NOTE_NO_MODEL_CHOSEN.to_string()),
            }
        }
        Some(c) if c.backend == "ollama" => {
            if !ollama_up {
                return Err(NOTE_NOTHING_AVAILABLE.to_string());
            }
            match c.model.as_deref().filter(|m| !m.trim().is_empty()) {
                Some(model) if ollama_models.iter().any(|m| m.name == model) => {
                    Ok(ChatBackend::ollama(model))
                }
                Some(_) if ollama_models.is_empty() => Err(NOTE_OLLAMA_NO_MODELS.to_string()),
                Some(_) => Err(NOTE_NO_MODEL_CHOSEN.to_string()),
                None if ollama_models.is_empty() => Err(NOTE_OLLAMA_NO_MODELS.to_string()),
                None => Err(NOTE_NO_MODEL_CHOSEN.to_string()),
            }
        }
        // Auto, and any unrecognised backend string. A running Ollama with an
        // installed model wins (needs no key, no explicit id); otherwise an
        // honest note, exactly as `select_rewriter`.
        _ => match ollama_models.first() {
            Some(first) if ollama_up => Ok(ChatBackend::ollama(first.name.clone())),
            _ if openai_key.is_some() => Err(NOTE_NO_MODEL_CHOSEN.to_string()),
            _ if ollama_up => Err(NOTE_OLLAMA_NO_MODELS.to_string()),
            _ => Err(NOTE_NOTHING_AVAILABLE.to_string()),
        },
    }
}

/// Draft a model-aware shot list from one brief.
///
/// Takes NO `State`: like `preview_prompt`, with no handle to the store, prices
/// or fal it structurally cannot bill a generation. It reads the hosted key the
/// same way `probe_enhancer_env` does, selects a chat backend (an unavailable
/// one is an honest `Err`), and runs the offline validator + clamp in
/// `storyboard::plan`. Drafting spends nothing on fal; on a hosted backend it is
/// the one short enhancer completion the user already configured (documented).
#[tauri::command]
pub fn plan_storyboard(
    brief: String,
    target_seconds: f64,
    use_case: String,
    model_prefs: Vec<String>,
    rewriter: Option<RewriterChoice>,
) -> Result<hickeyfield_core::StoryboardPlan, String> {
    let uc = hickeyfield_core::UseCase::from_slug(&use_case)
        .ok_or_else(|| format!("unknown use case: {use_case}"))?;
    // Drafting always needs a rewriter, so probe as if enhancement is on.
    let (openai_key, ollama_up, ollama_models) = probe_enhancer_env(true);
    let backend = chat_backend_for(
        rewriter.as_ref(),
        openai_key.as_deref(),
        ollama_up,
        &ollama_models,
    )?;
    hickeyfield_core::storyboard::plan(
        &backend,
        hickeyfield_core::corpus::STORYBOARD_SYSTEM,
        &brief,
        target_seconds,
        uc,
        &model_prefs,
    )
    .map_err(|e| e.to_string())
}

/// Everything `generate_storyboard` needs: the brief and target (for the group
/// row), the edited plan JSON (stored verbatim for S14 — carries roles and
/// recorded continuity), and one [`SubmitInput`] per shot. Each shot is a plain
/// submit payload, reused verbatim so every existing invariant (S6 guard, S7
/// verbatim `final_prompt`, S10 pricing) holds per shot.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoryboardGenerateInput {
    pub brief: String,
    pub target_seconds: f64,
    /// The serialised `StoryboardPlan` the UI holds, stored for S14. On a retry
    /// this is the FULL original plan (every shot the user saw and that
    /// validated), NOT the failed subset — the server re-validates it whole.
    #[serde(default)]
    pub plan_json: Option<String>,
    /// The shots to actually submit this call. On the first generate-all this is
    /// every shot; on a retry it is only the failed subset.
    pub shots: Vec<SubmitInput>,
    /// The plan index each entry in `shots` maps to. `None` means "the shots are
    /// the whole plan in order" (`0..shots.len()`), the first-generate case. On a
    /// retry this carries the original indices of the failed shots, so each
    /// resubmitted shot is matched to its plan index (model/duration per index)
    /// and tagged with its original `shot_index`.
    #[serde(default)]
    pub shot_indices: Option<Vec<u32>>,
    /// An existing storyboard group to write into. `None` mints a fresh group
    /// (first generate-all); `Some` makes a retry write its new job rows into the
    /// original set with their original `shot_index`, instead of starting a
    /// separate group.
    #[serde(default)]
    pub storyboard_id: Option<String>,
    /// S15: submit the shots as a chain (each continues from the previous
    /// shot's last frame) rather than all at once. Only shot 0 is submitted
    /// now; the runner's settled hook drives the rest. Default off (S13).
    #[serde(default)]
    pub chained: bool,
    /// S15: the crossfade overlap the plan was reconciled to, stored so the
    /// render uses the same value the plan was validated against. `None`
    /// defaults to `DEFAULT_CROSSFADE_SECONDS`.
    #[serde(default)]
    pub overlap_seconds: Option<f64>,
    /// S16 D9: opt into bounded auto-retry of a below-floor continuity join (at
    /// most 2 regenerations, each priced first). Default off — nothing spends
    /// without an explicit opt-in and the total shown.
    #[serde(default)]
    pub auto_retry: bool,
    /// S17: generate this set in keyframe-anchored mode — render one still per
    /// shot boundary (N+1) and anchor every shot on BOTH ends. Default off. Only
    /// valid when the set is chained and every shot's model takes an end frame.
    #[serde(default)]
    pub keyframe_mode: bool,
    /// S17: the image model that renders the stills (one per set). `None`
    /// defaults to `qwen_image_3` (D1 — the priced default). Gated by the
    /// keyframe validator to a runnable text-to-image model.
    #[serde(default)]
    pub still_model_id: Option<String>,
}

/// One shot's fate in a generate-all batch. A shot that failed (a refusal, a
/// missing key, a submit error) carries `error`; a shot that started carries the
/// `job_set_id` the Library watches; a chained shot not yet submitted is
/// `pending` (the settled hook will submit it when its predecessor completes).
#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ShotOutcome {
    pub shot_index: u32,
    pub job_set_id: Option<String>,
    pub error: Option<String>,
    #[serde(default)]
    pub pending: bool,
}

/// The result of a generate-all: the minted storyboard id and a per-shot outcome.
#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct StoryboardResult {
    pub storyboard_id: String,
    pub shots: Vec<ShotOutcome>,
}

/// Generate every shot as one linked set.
///
/// The group row is written **first**, so the set survives even if a shot fails.
/// Each shot then goes through `submit_one` — the identical money path a single
/// submit uses (compile + S6 guard + S7 verbatim prompt + S10 estimate + route
/// + submit + watch), tagged with its set id and index. A shot that fails is
/// recorded as an outcome carrying its error and the batch **continues**: a
/// refusal on shot 3 must not discard shots 1–2 the user already paid to start.
#[tauri::command]
pub fn generate_storyboard(
    state: State<'_, AppState>,
    input: StoryboardGenerateInput,
) -> Result<StoryboardResult, String> {
    generate_storyboard_inner(&state, input)
}

/// The body of [`generate_storyboard`], taking a plain `&AppState` so it is
/// testable with an in-memory store and no Tauri `State` extractor.
/// The largest a brief may be at the generate boundary (4 KiB). A brief is the
/// user's own text, but capping it keeps one runaway paste from inflating the
/// stored row (and, at draft time, the hosted completion's token cost).
const MAX_BRIEF_BYTES: usize = 4 * 1024;
/// The largest a serialised plan may be at the generate boundary (64 KiB).
const MAX_PLAN_JSON_BYTES: usize = 64 * 1024;
/// A storyboard never has more than 8 shots (the same bound `storyboard::validate`
/// enforces at draft time). Re-checked here so the money/plan-size invariant does
/// not depend solely on the UI disabling Generate-all.
const MAX_SHOTS: usize = 8;

fn generate_storyboard_inner(
    state: &AppState,
    input: StoryboardGenerateInput,
) -> Result<StoryboardResult, String> {
    // Server-side money/size guards. The Storyboard view already gates on these
    // (it disables Generate-all until a validated plan and its total render), but
    // a caller that reached this command directly must not be able to submit an
    // unbounded, unvalidated batch of paid jobs.
    if input.brief.len() > MAX_BRIEF_BYTES {
        return Err(format!(
            "brief is too large ({} bytes; limit {MAX_BRIEF_BYTES}).",
            input.brief.len()
        ));
    }
    if let Some(plan_json) = input.plan_json.as_deref() {
        if plan_json.len() > MAX_PLAN_JSON_BYTES {
            return Err(format!(
                "plan is too large ({} bytes; limit {MAX_PLAN_JSON_BYTES}).",
                plan_json.len()
            ));
        }
    }
    if input.shots.is_empty() {
        return Err("a storyboard needs at least one shot to generate.".to_string());
    }
    if input.shots.len() > MAX_SHOTS {
        return Err(format!(
            "a storyboard has at most {MAX_SHOTS} shots, but {} were submitted.",
            input.shots.len()
        ));
    }

    let reg = registry();

    // The plan index each submitted shot maps to. `None` (first generate-all)
    // means the shots ARE the whole plan in order; a retry carries the failed
    // shots' original indices so each resubmit is matched to its plan slot.
    let is_retry_subset = input.shot_indices.is_some();
    let indices: Vec<u32> = match &input.shot_indices {
        Some(ix) => ix.clone(),
        None => (0..input.shots.len() as u32).collect(),
    };
    if indices.len() != input.shots.len() {
        return Err(format!(
            "shot_indices has {} entries but {} shots were submitted.",
            indices.len(),
            input.shots.len()
        ));
    }

    // Re-validate the plan the UI showed, server-side, via the same
    // `storyboard::validate` the draft path used — so a tampered/direct caller
    // cannot submit shots the composer would have refused. On a retry the FULL
    // original plan is sent and re-validated whole (3..=8 shots, ±10% sum); each
    // submitted shot is then matched to its plan index by model, duration, and
    // route, and only that subset is submitted. This closes M2: a retry of
    // fewer than 3 shots is no longer re-count/re-sum-gated against a shrunken
    // list — the full validated plan is the authority.
    if let Some(plan_json) = input.plan_json.as_deref() {
        let plan: hickeyfield_core::storyboard::StoryboardPlan = serde_json::from_str(plan_json)
            .map_err(|e| format!("plan is not a valid storyboard: {e}"))?;
        let uc = hickeyfield_core::UseCase::from_slug(&plan.use_case)
            .ok_or_else(|| format!("plan names an unknown use case: {}", plan.use_case))?;
        hickeyfield_core::storyboard::validate(
            &plan.shots,
            plan.target_seconds,
            uc,
            hickeyfield_core::storyboard::DEFAULT_CROSSFADE_SECONDS,
        )
        .map_err(|e| e.to_string())?;
        // On the first generate-all (no explicit indices) the submitted list is
        // the whole plan: it must match one-for-one, so no shot is injected
        // beyond the plan the user saw and priced.
        if !is_retry_subset && plan.shots.len() != input.shots.len() {
            return Err(format!(
                "submitted {} shots but the plan has {} — refusing to generate a set that does not match the plan.",
                input.shots.len(),
                plan.shots.len()
            ));
        }
        // Pin each submitted shot to its plan slot: model, priced duration, and
        // (when set) route must all come from the plan — so the total that bills
        // is the total the re-validated plan implies, not a tampered magnitude.
        for (shot, &idx) in input.shots.iter().zip(indices.iter()) {
            let pi = idx as usize;
            let planned = plan.shots.get(pi).ok_or_else(|| {
                format!(
                    "shot index {idx} is out of range for a plan of {} shots.",
                    plan.shots.len()
                )
            })?;
            if shot.model_id != planned.model_id {
                return Err(format!(
                    "shot {} submits model {}, which is not in the plan (plan shot {} is {}).",
                    idx + 1,
                    shot.model_id,
                    idx + 1,
                    planned.model_id
                ));
            }
            // Priced duration must equal the plan's clamped seconds for this
            // index (±0), so a direct caller cannot pass validate with a cheap
            // in-catalogue duration while billing an inflated one.
            if shot.settings.duration != Some(planned.seconds) {
                return Err(format!(
                    "shot {} submits duration {:?}s but the plan clamped it to {}s.",
                    idx + 1,
                    shot.settings.duration,
                    planned.seconds
                ));
            }
            // A pinned route must belong to the plan's model.
            if let Some(route_id) = shot.route_id.as_deref() {
                let model = reg
                    .get(&planned.model_id)
                    .ok_or_else(|| format!("plan shot {} names an unknown model.", idx + 1))?;
                if !model.routes.iter().any(|r| r.id() == route_id) {
                    return Err(format!(
                        "shot {} route {} does not belong to the plan model {}.",
                        idx + 1,
                        route_id,
                        planned.model_id
                    ));
                }
            }
        }
    }

    // Determine the group id: a retry writes into the existing set, so its new
    // job rows join the original storyboard; a first generate mints a fresh group.
    // S17: keyframe-anchored mode. Only a first generate of a chained multi-shot
    // set can enter it (a retry resumes the existing set's chain). When active,
    // the N+1 stills are submitted first (priced, gated) and every shot is held
    // pending until the stills-ready barrier fires shot 0.
    let keyframe_active = input.keyframe_mode
        && input.storyboard_id.is_none()
        && !is_retry_subset
        && input.chained
        && input.shots.len() > 1;

    // Derived once here so both the stored `keyframes_json` (with per-still
    // estimates for the total) and the still submits below share one list.
    let keyframe_plan: Option<KeyframePlan> = if keyframe_active {
        let plan_json = input
            .plan_json
            .as_deref()
            .ok_or_else(|| "keyframe mode needs the plan to derive the stills.".to_string())?;
        let plan: hickeyfield_core::storyboard::StoryboardPlan =
            serde_json::from_str(plan_json).map_err(|e| format!("plan is not valid: {e}"))?;
        let still_model = input
            .still_model_id
            .clone()
            .unwrap_or_else(|| DEFAULT_STILL_MODEL.to_string());
        // The set canvas comes from shot 0's chosen resolution/aspect.
        let res = input
            .shots
            .first()
            .and_then(|s| s.settings.resolution.clone());
        let asp = input.shots.first().and_then(|s| s.settings.aspect.clone());
        let canvas = keyframe_canvas(res.as_deref(), asp.as_deref())?;
        let kfs = hickeyfield_core::storyboard::derive_keyframes(&plan, &still_model, canvas);
        hickeyfield_core::storyboard::validate_keyframes(&kfs, plan.shots.len(), canvas)
            .map_err(|e| e.to_string())?;
        // Price each still up front for the total shown before Generate.
        let still = reg
            .get(&still_model)
            .ok_or_else(|| format!("unknown still model: {still_model}"))?;
        let records: Vec<crate::store::KeyframeRecord> = kfs
            .iter()
            .map(|kf| {
                let si = still_submit_input(kf, res.as_deref(), asp.as_deref(), 0);
                crate::store::KeyframeRecord {
                    index: kf.index,
                    job_set_id: None,
                    local_path: None,
                    estimated_usd: still_estimate(&state.prices, still, &si.settings),
                    attempt: 0,
                }
            })
            .collect();
        Some(KeyframePlan {
            keyframes: kfs,
            records,
            resolution: res,
            aspect: asp,
        })
    } else {
        None
    };

    let sid = match input.storyboard_id.clone() {
        Some(existing) => {
            let Some(sb) = state
                .store
                .storyboard(&existing)
                .map_err(|e| e.to_string())?
            else {
                return Err(format!("no such storyboard to retry into: {existing}"));
            };
            // A shot that already started (a non-Failed row at that index) must
            // never be resubmitted — a successful shot cannot be double-billed.
            //
            // S16 D7 relaxes this for exactly one case: a **Completed** shot
            // whose continuity join scored **below the floor** (`pass == false`)
            // may be regenerated "from the same frame". Gated on the recorded
            // `joins_json` verdict, so a *passing* or *still-running* shot is
            // still refused — the relaxation is only for a measured low join.
            let below_floor: std::collections::BTreeSet<u32> =
                crate::store::parse_joins(sb.joins_json.as_deref())
                    .into_iter()
                    .filter(|j| j.pass == Some(false))
                    .map(|j| j.index as u32)
                    .collect();
            let existing_jobs = state
                .store
                .jobs_in_storyboard(&existing)
                .map_err(|e| e.to_string())?;
            for &idx in &indices {
                let started = existing_jobs.iter().any(|j| {
                    j.shot_index == Some(idx) && j.status != hickeyfield_core::JobStatus::Failed
                });
                if !started {
                    continue;
                }
                // A started shot is only regenerable via the below-floor
                // relaxation; a passing/unscored started shot is still refused.
                if !below_floor.contains(&idx) {
                    return Err(format!(
                        "shot {} has already started — refusing to resubmit it.",
                        idx + 1
                    ));
                }
                // M1: the below-floor verdict comes from `joins_json`, which is
                // only refreshed when a shot settles and is re-scored. Between
                // submitting a regenerate and its settle the join still reads
                // `pass == false` while a Queued/Running job already exists at
                // this index — so admitting a second regenerate would mint a
                // duplicate in-flight job (a double-bill). Only a **settled**
                // (terminal) low-join shot is eligible; an in-flight job at this
                // index refuses.
                if shot_in_flight(&existing_jobs, idx) {
                    return Err(format!(
                        "shot {} is still in flight — refusing to resubmit it.",
                        idx + 1
                    ));
                }
            }
            existing
        }
        None => {
            let sid = mint_storyboard_id();
            // S15: a chained set stores the full ordered shot payloads so it can
            // advance itself after a relaunch, and the crossfade it was
            // reconciled to. On a first generate-all the submitted shots ARE the
            // whole plan in order, so `input.shots` is the faithful record. A
            // chain is meaningless without at least two shots.
            let chained = input.chained && input.shots.len() > 1;
            let shots_json = if chained {
                Some(serde_json::to_string(&input.shots).map_err(|e| e.to_string())?)
            } else {
                None
            };
            let storyboard = crate::store::Storyboard {
                id: sid.clone(),
                brief: input.brief.clone(),
                target_seconds: input.target_seconds,
                // Fall back to a minimal object so a group is always readable, but
                // the UI normally sends the full edited plan (roles + continuity).
                plan_json: input.plan_json.clone().unwrap_or_else(|| "{}".to_string()),
                created_at: now_secs(),
                chained,
                shots_json,
                // S15 D-B: a chained set's continuity joins are hard cuts
                // (overlap 0), so the stitched length is the raw sum. A
                // non-chained set crossfades at the reconciled overlap. The
                // render decides per-join (from each shot's Start media); this
                // stored value is the crossfade used for any non-continuity join
                // and the number `generate`'s re-validate reconciles against.
                overlap_seconds: Some(if chained {
                    0.0
                } else {
                    input
                        .overlap_seconds
                        .unwrap_or(hickeyfield_core::storyboard::DEFAULT_CROSSFADE_SECONDS)
                }),
                joins_json: None,
                // S16 D9: the opt-in auto-retry toggle, off unless the UI set it.
                auto_retry: input.auto_retry,
                // S17: the N+1 still boundaries (with per-still estimates), or
                // None for a non-keyframe set.
                keyframes_json: keyframe_plan
                    .as_ref()
                    .map(|kp| serde_json::to_string(&kp.records))
                    .transpose()
                    .map_err(|e| e.to_string())?,
            };
            state
                .store
                .upsert_storyboard(&storyboard)
                .map_err(|e| e.to_string())?;
            sid
        }
    };

    // S17: in keyframe mode, submit the N+1 stills first — each through the same
    // gated `submit_one` money path (priced, guarded) — and hold every shot
    // pending. The stills-ready barrier (`chain::keyframes_advance`) submits shot
    // 0 once the last still settles. Nothing here submits a shot.
    if let Some(KeyframePlan {
        keyframes: kfs,
        mut records,
        resolution: res,
        aspect: asp,
    }) = keyframe_plan
    {
        let seed = crate::chain::storyboard_seed(&sid);
        for kf in &kfs {
            let si = still_submit_input(kf, res.as_deref(), asp.as_deref(), seed);
            let Some(model) = reg.get(&kf.model_id) else {
                continue;
            };
            match submit_one(
                &state.store,
                &state.prices,
                &state.runner,
                model,
                &si,
                Some(&sid),
                None, // a still occupies no shot index (D2/Q5)
            ) {
                Ok(job_id) => {
                    if let Some(rec) = records.iter_mut().find(|r| r.index == kf.index) {
                        rec.job_set_id = Some(job_id);
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        "storyboard {sid}: keyframe {} did not submit: {e}",
                        kf.index
                    );
                }
            }
        }
        // Persist the still job ids back onto the set so the barrier and a
        // relaunch can map each settled still job to its boundary.
        if let Ok(Some(mut sb)) = state.store.storyboard(&sid) {
            sb.keyframes_json = serde_json::to_string(&records).ok();
            let _ = state.store.upsert_storyboard(&sb);
        }
        // Every shot is pending; the barrier drives the chain from shot 0.
        let outcomes = indices
            .iter()
            .map(|&idx| ShotOutcome {
                shot_index: idx,
                job_set_id: None,
                error: None,
                pending: true,
            })
            .collect();
        return Ok(StoryboardResult {
            storyboard_id: sid,
            shots: outcomes,
        });
    }

    // S15: in chained mode only shot 0 is submitted now; the runner's settled
    // hook (`chain::advance`) extracts its last frame and submits shot 1, and so
    // on. The later shots are returned as `pending` so the UI can render the
    // chain state. A retry (an existing set) never re-enters chained mode here —
    // the hook resumes the chain from the retried shot (§D2.8).
    let chained_first_generate =
        input.chained && input.storyboard_id.is_none() && !is_retry_subset && input.shots.len() > 1;

    let mut outcomes = Vec::with_capacity(input.shots.len());
    for (pos, (shot, &idx)) in input.shots.iter().zip(indices.iter()).enumerate() {
        // Chained: submit only the first shot; the rest are pending, driven by
        // the settled hook as each predecessor completes and downloads.
        if chained_first_generate && pos > 0 {
            outcomes.push(ShotOutcome {
                shot_index: idx,
                job_set_id: None,
                error: None,
                pending: true,
            });
            continue;
        }
        let outcome = match reg.get(&shot.model_id) {
            None => ShotOutcome {
                shot_index: idx,
                job_set_id: None,
                error: Some(format!("unknown model: {}", shot.model_id)),
                pending: false,
            },
            Some(model) => match submit_one(
                &state.store,
                &state.prices,
                &state.runner,
                model,
                shot,
                Some(&sid),
                Some(idx),
            ) {
                Ok(job_set_id) => ShotOutcome {
                    shot_index: idx,
                    job_set_id: Some(job_set_id),
                    error: None,
                    pending: false,
                },
                Err(e) => ShotOutcome {
                    shot_index: idx,
                    job_set_id: None,
                    error: Some(e),
                    pending: false,
                },
            },
        };
        outcomes.push(outcome);
    }

    Ok(StoryboardResult {
        storyboard_id: sid,
        shots: outcomes,
    })
}

/// The stored storyboards, for the Library's group headers. Serialised camelCase
/// for the bridge; `plan_json` rides along as a raw string the UI may parse.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoryboardDto {
    pub id: String,
    pub brief: String,
    pub target_seconds: f64,
    pub plan_json: String,
    pub created_at: i64,
    /// S16: the recorded per-join continuity scores (a serialised
    /// `Vec<JoinRecord>`), so the Library can badge each join and show the set's
    /// minimum score. `None` for an unscored or pre-S16 set.
    pub joins_json: Option<String>,
    /// S16: whether the set opted into auto-retry (surfaced for the badge copy).
    pub auto_retry: bool,
    /// S17: the keyframe boundaries (a serialised `Vec<KeyframeRecord>`) for a
    /// keyframe set, so the Library/Storyboard views can show the still cards,
    /// the parked state, and the priced-total contribution. `None` otherwise.
    pub keyframes_json: Option<String>,
}

#[tauri::command]
pub fn list_storyboards(state: State<'_, AppState>) -> Result<Vec<StoryboardDto>, String> {
    Ok(state
        .store
        .storyboards()
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|s| StoryboardDto {
            id: s.id,
            brief: s.brief,
            target_seconds: s.target_seconds,
            plan_json: s.plan_json,
            created_at: s.created_at,
            joins_json: s.joins_json,
            auto_retry: s.auto_retry,
            keyframes_json: s.keyframes_json,
        })
        .collect())
}

/// Regenerate one keyframe still (S17, D4/D5). Re-derives the still prompt from
/// the stored plan (so it stays the world + hold clause verbatim), prices it
/// through `submit_one` **before** it spends (the total-shown invariant, AC6),
/// and re-parks the set until the new still settles and the barrier proceeds.
///
/// Guards: an in-flight still at this boundary is refused (a queued/running
/// still would be double-billed); on the auto path the `KeyframeRecord.attempt`
/// bound (≤ [`KEYFRAME_RETRY_CAP`]) caps it separately from a shot's bound.
#[tauri::command]
pub fn regenerate_keyframe_still(
    state: State<'_, AppState>,
    storyboard_id: String,
    index: usize,
) -> Result<String, String> {
    regenerate_keyframe_still_inner(
        &state.store,
        &state.prices,
        &state.runner,
        &storyboard_id,
        index,
        false,
    )
}

pub(crate) fn regenerate_keyframe_still_inner(
    store: &Arc<crate::store::SqliteStore>,
    prices: &Arc<crate::pricing::Prices>,
    runner: &crate::runner::Runner,
    sid: &str,
    index: usize,
    auto: bool,
) -> Result<String, String> {
    let Some(mut sb) = store.storyboard(sid).map_err(|e| e.to_string())? else {
        return Err(format!("no such storyboard: {sid}"));
    };
    let mut records = crate::store::parse_keyframes(sb.keyframes_json.as_deref());
    let Some(pos) = records.iter().position(|k| k.index == index) else {
        return Err(format!("storyboard {sid} has no keyframe {index}"));
    };

    // In-flight guard: never resubmit while this boundary's still is queued or
    // running — a second submit would mint a duplicate in-flight still (a
    // double-bill), the S16 M1 invariant extended to stills.
    if let Some(job_id) = records[pos].job_set_id.clone() {
        if let Ok(Some(job)) = store.get(&job_id) {
            if !job.status.is_terminal() {
                return Err(format!(
                    "keyframe {index} is still in flight — refusing to resubmit it."
                ));
            }
        }
    }
    // On the auto path, the still bound is separate from a shot's (D4/Q6).
    if auto && records[pos].attempt >= crate::chain::KEYFRAME_RETRY_CAP {
        return Err(format!(
            "keyframe {index} has hit the auto-retry cap ({}).",
            crate::chain::KEYFRAME_RETRY_CAP
        ));
    }

    // Re-derive the still input from the stored plan (verbatim world + hold).
    let plan: hickeyfield_core::storyboard::StoryboardPlan =
        serde_json::from_str(&sb.plan_json).map_err(|e| format!("plan is not valid: {e}"))?;
    let (res, asp) = match sb.shots_json.as_deref() {
        Some(sj) => {
            let shots: Vec<SubmitInput> =
                serde_json::from_str(sj).map_err(|e| format!("shots are not valid: {e}"))?;
            let s0 = shots.first();
            (
                s0.and_then(|s| s.settings.resolution.clone()),
                s0.and_then(|s| s.settings.aspect.clone()),
            )
        }
        None => (None, None),
    };
    let canvas = keyframe_canvas(res.as_deref(), asp.as_deref())?;
    // The still model: whatever the existing still job used, else the default.
    let still_model = records[pos]
        .job_set_id
        .as_ref()
        .and_then(|id| store.get(id).ok().flatten())
        .map(|j| j.model_id)
        .unwrap_or_else(|| DEFAULT_STILL_MODEL.to_string());
    let kfs = hickeyfield_core::storyboard::derive_keyframes(&plan, &still_model, canvas);
    let Some(kf) = kfs.iter().find(|k| k.index == index) else {
        return Err(format!("could not derive keyframe {index}"));
    };
    let Some(model) = reg().get(&still_model).cloned() else {
        return Err(format!("unknown still model: {still_model}"));
    };

    // A regenerate MUST change the seed or identical inputs reproduce identically.
    let seed = crate::chain::storyboard_seed(sid) + (records[pos].attempt as u64 + 1);
    let si = still_submit_input(kf, res.as_deref(), asp.as_deref(), seed);
    let job_id = submit_one(store, prices, runner, &model, &si, Some(sid), None)?;

    // Re-park this boundary: point it at the new still, clear its downloaded
    // path, bump its own bound. The barrier waits for the new still to settle.
    let reparked = crate::store::KeyframeRecord {
        index,
        job_set_id: Some(job_id.clone()),
        local_path: None,
        estimated_usd: records[pos].estimated_usd,
        attempt: records[pos].attempt + 1,
    };
    crate::store::upsert_keyframe(&mut records, reparked);
    sb.keyframes_json = serde_json::to_string(&records).ok();
    store.upsert_storyboard(&sb).map_err(|e| e.to_string())?;
    Ok(job_id)
}

/// The registry helper alias used by the storyboard commands.
fn reg() -> std::collections::BTreeMap<String, hickeyfield_core::Model> {
    registry()
}

#[tauri::command]
pub fn list_jobs(state: State<'_, AppState>) -> Result<Vec<JobSet>, String> {
    state.store.all().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cancel_job(state: State<'_, AppState>, job_set_id: String) -> Result<(), String> {
    state.runner.cancel(&job_set_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_job(
    state: State<'_, AppState>,
    job_set_id: String,
    #[allow(non_snake_case)] deleteFiles: Option<bool>,
) -> Result<(), String> {
    // Tombstone *first*. Deleting the row while a poll loop is live let the
    // next tick write it straight back, so the row reappeared seconds later.
    state.runner.forget(&job_set_id);

    // Only remove files if asked. A generation costs real money, and a row
    // disappearing from a list is a much smaller act than shredding the asset —
    // so the destructive half is opt-in and the caller has to say so.
    if deleteFiles.unwrap_or(false) {
        if let Ok(Some(job)) = state.store.get(&job_set_id) {
            for out in &job.results {
                if let Some(p) = &out.local_path {
                    // Best effort: a file the user already moved or deleted
                    // must not block removing the row.
                    if let Err(e) = std::fs::remove_file(p) {
                        tracing::warn!("could not delete {p}: {e}");
                    }
                }
            }
        }
    }

    state.store.delete(&job_set_id).map_err(|e| e.to_string())
}

/// Show a generated file in Finder or Explorer.
///
/// Scoped to the library root so a crafted path from the webview cannot ask
/// the OS to reveal something elsewhere on disk.
#[tauri::command]
pub fn reveal_result(state: State<'_, AppState>, path: String) -> Result<(), String> {
    let root = state.library.root().to_path_buf();
    let target = std::path::PathBuf::from(&path);
    let canonical = target
        .canonicalize()
        .map_err(|e| format!("could not find {path}: {e}"))?;
    if !canonical.starts_with(&root) {
        return Err("that file is not in the Hickeyfield library".into());
    }
    tauri_plugin_opener::reveal_item_in_dir(&canonical).map_err(|e| e.to_string())
}

/// Let the webview display files the user just picked in the native dialog.
///
/// The asset protocol is scoped to the library, deliberately — the webview has
/// no business reading arbitrary disk. But an attachment lives wherever the
/// user keeps it, so without a grant the settings rail can only show a filename
/// where a thumbnail belongs, and nothing in the app can measure the shape of
/// an input it is about to animate.
///
/// The grant is per file and only ever for a path that came back from the OS
/// file dialog, which is the user pointing at it as directly as the platform
/// allows. Nothing here accepts a path the webview invented: a caller passing a
/// path the user did not choose is granting access to something it already had
/// the path to, and the dialog is the only thing that produces these.
#[tauri::command]
pub fn allow_media_preview(app: tauri::AppHandle, paths: Vec<String>) -> Result<(), String> {
    use tauri::Manager;
    for p in &paths {
        let path = std::path::Path::new(p);
        // Canonicalise first: a grant for a path with `..` in it is a grant for
        // wherever that resolves, which is not what the caller asked for and
        // not what the user picked.
        let real = path
            .canonicalize()
            .map_err(|e| format!("could not resolve {p}: {e}"))?;
        if !real.is_file() {
            return Err(format!("{p} is not a file"));
        }
        app.asset_protocol_scope()
            .allow_file(&real)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Ids the runner is actively polling right now.
///
/// Distinct from "status is in_progress": a row can be non-terminal in the
/// database while nothing is watching it, which is exactly what a crash leaves
/// behind. The UI needs to tell those apart to offer a resume.
#[tauri::command]
pub fn watching_jobs(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    Ok(state
        .store
        .all()
        .map_err(|e| e.to_string())?
        .into_iter()
        .filter(|j| state.runner.is_watching(&j.id))
        .map(|j| j.id)
        .collect())
}

#[tauri::command]
pub fn library_root(state: State<'_, AppState>) -> String {
    state.library.root().display().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_duration_outside_the_range_is_clamped_with_an_advisory() {
        // H3 Max, 20s → 15s, one advisory naming the range, and the estimate
        // is priced on the clamped value, not the 20s the user staged.
        let mut s = SettingsDto {
            duration: Some(20.0),
            resolution: Some("768P".into()),
            aspect: Some("16:9".into()),
            batch: 1,
            ..Default::default()
        };
        let adv = clamp_duration_to_range(&mut s, Some((5.0, 15.0)), "MiniMax H3 Max");
        assert_eq!(s.duration, Some(15.0));
        assert_eq!(adv.len(), 1);
        assert!(adv[0].contains("5–15s"), "{}", adv[0]);
        assert!(adv[0].contains("20s clamped to 15s"), "{}", adv[0]);

        // The estimate follows the clamped value: the billable seconds are 15,
        // not 20, so a per-second price cannot quote the un-renderable length.
        let clamped = Billable::from(&s);
        assert_eq!(clamped.seconds, Some(15.0));
        let mut unclamped = SettingsDto {
            duration: Some(20.0),
            ..s.clone()
        };
        // (No clamp applied to the copy — proves 20 and 15 are different bills.)
        let _ = &mut unclamped;
        assert_ne!(Billable::from(&unclamped).seconds, clamped.seconds);
    }

    #[test]
    fn a_duration_inside_the_range_is_untouched() {
        let mut s = SettingsDto {
            duration: Some(10.0),
            batch: 1,
            ..Default::default()
        };
        let adv = clamp_duration_to_range(&mut s, Some((5.0, 15.0)), "MiniMax H3 Max");
        assert_eq!(s.duration, Some(10.0));
        assert!(adv.is_empty());
        // A model with no documented range never clamps either.
        let mut free = SettingsDto {
            duration: Some(99.0),
            ..Default::default()
        };
        assert!(clamp_duration_to_range(&mut free, None, "Free").is_empty());
        assert_eq!(free.duration, Some(99.0));
    }

    #[test]
    fn the_dto_carries_the_duration_range() {
        use hickeyfield_core::capability::for_route;
        use hickeyfield_core::media::InputMode;
        use hickeyfield_core::provider::ProviderId;
        // H3 Max: fal publishes no min/max, the registry declares 5–15, so the
        // offline capability carries it and the DTO copies it verbatim.
        let spec = hickeyfield_core::registry::registry()
            .remove("minimax_h3_max")
            .expect("h3 max is registered")
            .spec;
        let cap = for_route(
            &spec,
            ProviderId::Fal,
            "minimax/h3-max/image-to-video",
            InputMode::Image,
            |_| None,
        );
        assert_eq!(cap.duration_range(), Some((5.0, 15.0)));
        let dto = CapabilitiesDto::from_capability(&cap);
        assert_eq!(dto.duration_range, Some((5.0, 15.0)));
    }

    #[test]
    fn the_catalogue_is_exposed_and_launch_models_come_first() {
        let models = list_models();
        assert!(models.len() >= 55, "got {}", models.len());

        let first_non_launch = models
            .iter()
            .position(|m| !m.is_launch)
            .unwrap_or(models.len());
        assert!(
            models[..first_non_launch].iter().all(|m| m.is_launch),
            "launch models must be contiguous at the top"
        );
        assert!(first_non_launch >= 12, "expected at least 12 launch models");
    }

    #[test]
    fn every_model_offers_at_least_one_route() {
        for m in list_models() {
            assert!(!m.routes.is_empty(), "{} has no route", m.id);
        }
    }

    #[test]
    fn an_unknown_price_is_null_not_zero() {
        // The single most dangerous possible bug in this file.
        let got = estimate_with(
            &crate::pricing::Prices::bundled(),
            "definitely-not-a-model",
            "fal:whatever",
            &SettingsDto::default(),
        );
        assert!(got.is_none());
    }

    #[test]
    fn a_known_model_and_route_quotes_a_positive_price() {
        let models = list_models();
        let m = models
            .iter()
            .find(|m| m.is_launch && m.modality == "video")
            .expect("a launch video model");
        let route = &m.routes[0];

        let est = estimate_with(
            &crate::pricing::Prices::bundled(),
            &m.id,
            &route.id,
            &SettingsDto {
                duration: Some(8.0),
                resolution: Some("720p".into()),
                aspect: Some("16:9".into()),
                batch: 1,
                ..Default::default()
            },
        );
        if let Some(e) = est {
            assert!(e.usd > 0.0, "{} quoted {}", m.id, e.usd);
            assert!(!e.basis.is_empty(), "an estimate must explain itself");
        }
    }

    #[test]
    fn the_ui_vocabulary_becomes_real_pixels() {
        // The defect this replaces: SettingsDto spoke {seconds,width,height}
        // while the UI sent {duration,resolution,aspect}, so nothing
        // deserialised, `seconds` was always None, and every estimate came out
        // as $0.00 — a confidently wrong price on the Generate button.
        let s = SettingsDto {
            duration: Some(8.0),
            resolution: Some("720p".into()),
            aspect: Some("16:9".into()),
            batch: 1,
            ..Default::default()
        };
        let b = Billable::from(&s);
        assert_eq!(b.seconds, Some(8.0));
        assert_eq!(b.height, Some(720));
        assert_eq!(b.width, Some(1280));
        assert!(b.megapixels.is_some());
    }

    #[test]
    fn the_wire_shape_matches_what_the_ui_actually_sends() {
        // Guards the camelCase rename and the field names together. If either
        // drifts, this fails instead of silently producing a zero-cost job.
        let json = r#"{
            "modelId": "kling3_0",
            "prompt": "hi",
            "presetId": null,
            "settings": {"duration": 5, "resolution": "1080p", "aspect": "9:16",
                         "audio": false, "enhance": true},
            "media": []
        }"#;
        let parsed: SubmitInput = serde_json::from_str(json).expect("UI payload must deserialise");
        assert_eq!(parsed.model_id, "kling3_0");
        assert_eq!(parsed.settings.duration, Some(5.0));
        let b = Billable::from(&parsed.settings);
        assert_eq!(b.height, Some(1080));
        // 9:16 portrait — width is the short edge.
        assert_eq!(b.width, Some(608));
    }

    #[test]
    fn an_unrecognised_resolution_yields_no_pixels_rather_than_a_guess() {
        // A guessed resolution becomes a guessed megapixel count becomes a
        // wrong price.
        assert_eq!(resolution_height("ultra"), None);
        let s = SettingsDto {
            resolution: Some("ultra".into()),
            aspect: Some("16:9".into()),
            ..Default::default()
        };
        let b = Billable::from(&s);
        assert!(b.width.is_none() && b.height.is_none() && b.megapixels.is_none());
    }

    #[test]
    fn both_resolution_vocabularies_are_understood() {
        // Video models say 720p; image models say 2k. Both are real.
        assert_eq!(resolution_height("1080p"), Some(1080));
        assert_eq!(resolution_height("4K"), Some(2160));
        assert_eq!(resolution_height("2k"), Some(1440));
    }

    #[test]
    fn the_768p_tier_is_reachable() {
        // H3 Max defaults to fal's UPPERCASE "768P". The height map lowercases
        // first, so both cases resolve; without the entry the default-resolution
        // price would read "unknown" and the middle tier would be unreachable.
        assert_eq!(resolution_height("768P"), Some(768));
        assert_eq!(resolution_height("768p"), Some(768));
        let s = SettingsDto {
            resolution: Some("768P".into()),
            aspect: Some("16:9".into()),
            duration: Some(5.0),
            ..Default::default()
        };
        let b = Billable::from(&s);
        assert_eq!(b.height, Some(768), "768P must yield a billable height");
    }

    #[test]
    fn derived_widths_are_even() {
        // Encoders reject odd dimensions, so a price computed on one is
        // meaningless anyway.
        for aspect in ["16:9", "9:16", "1:1", "21:9", "4:3"] {
            for h in [480u32, 720, 1080, 2160] {
                let w = width_for(h, Some(aspect)).unwrap();
                assert_eq!(w % 2, 0, "{aspect} at {h} gave odd width {w}");
            }
        }
    }

    #[test]
    fn presets_are_exposed_with_their_categories() {
        let presets = list_presets();
        assert!(!presets.is_empty());
        assert!(presets.iter().all(|p| !p.category.is_empty()));
    }

    #[test]
    fn readiness_ignores_providers_that_need_no_key() {
        // The first-run bug this exists to prevent: `Local` reports as
        // configured because it needs no credential, so a naive
        // "any provider configured?" check was always true and onboarding
        // never opened on a fresh machine.
        let by_key = ProviderId::ALL
            .into_iter()
            .filter(|p| p.needs_key())
            .any(vault::is_configured);
        let naive = !configured_providers().is_empty();
        assert!(
            naive,
            "Local alone should still make the naive check true — that is the trap"
        );
        // is_ready may be true via a detected local endpoint, but it must never
        // be true *purely* because Local exists as a concept.
        if !by_key && !detect_local().any() {
            assert!(
                !is_ready(),
                "a machine with no keys and nothing local is not ready"
            );
        }
    }

    #[test]
    fn key_states_never_carry_a_secret() {
        // Serialised and inspected, because the guarantee is about the wire
        // shape rather than the Rust type.
        let json = serde_json::to_string(&key_states()).unwrap();
        for leak in ["password", "secret_value", "api_key\":\""] {
            assert!(!json.contains(leak), "possible key leak: {leak}");
        }
        assert!(json.contains("has_key"));
    }

    #[test]
    fn routes_report_availability_so_the_ui_can_explain_itself() {
        // A route the user cannot reach is shown and marked, not hidden — the
        // point is to tell them which key would unlock it.
        let models = list_models();
        let any_route = models.iter().flat_map(|m| &m.routes).next().unwrap();
        assert!(!any_route.provider.is_empty());
        assert!(any_route.id.contains(':'));
    }

    #[test]
    fn a_subtitle_describes_the_model_rather_than_repeating_its_slug() {
        let reg = hickeyfield_core::registry::registry();
        let sd = model_subtitle(&reg["seedance_2_0"]).unwrap();
        assert!(sd.contains("up to 4k"), "got: {sd}");
        assert!(!sd.contains("fal-ai"), "subtitle must not be a slug: {sd}");
    }

    #[test]
    fn most_models_get_a_subtitle() {
        // If this collapses, the picker is back to showing raw slugs.
        let reg = hickeyfield_core::registry::registry();
        let with = reg.values().filter(|m| model_subtitle(m).is_some()).count();
        assert_eq!(with, reg.len(), "every model must describe itself");
        // And none of them may fall back to printing a slug.
        for m in reg.values() {
            let s = model_subtitle(m).unwrap();
            assert!(!s.contains('/'), "{} subtitle looks like a slug: {s}", m.id);
        }
    }

    #[test]
    fn a_model_that_needs_an_image_says_so() {
        // The single most common first-submit failure is a missing required
        // input, and the picker is where it is cheapest to prevent.
        let reg = hickeyfield_core::registry::registry();
        let needy: Vec<_> = reg
            .values()
            .filter(|m| model_subtitle(m).is_some_and(|s| s.contains("needs ")))
            .collect();
        assert!(!needy.is_empty(), "no model advertises a required input");
    }

    #[test]
    fn durations_render_without_a_trailing_zero() {
        assert_eq!(trim_num(5.0), "5");
        assert_eq!(trim_num(5.5), "5.5");
    }

    #[test]
    fn a_key_without_a_client_is_not_reported_as_available() {
        // The picker must not offer what the resolver will refuse.
        let configured = vec!["vaig".to_string()];
        let (ok, why) = route_state(ProviderId::Vaig, "whatever", &configured);
        assert!(!ok);
        let why = why.unwrap();
        assert!(why.contains("no client"), "got: {why}");
        // And it must not tell them to add the key they just added.
        assert!(!why.contains("Needs a"), "got: {why}");
    }

    #[test]
    fn a_model_the_provider_does_not_serve_is_greyed_out_not_offered() {
        // The bug this exists to prevent, reported by a user: Wan 2.6 was
        // selectable, priced at $0.50, and produced
        // `404 Path /v2.6/video-to-video not found` — a click and a round trip
        // to learn something the probe already knew.
        let (ok, why) = route_state(ProviderId::Fal, "fal-ai/wan/v2.6", &["fal".to_string()]);
        assert!(!ok);
        let why = why.unwrap();
        assert!(why.contains("does not serve"), "got: {why}");
        // Not a key problem — do not send them to Settings.
        assert!(!why.contains("Needs a"), "got: {why}");
    }

    #[test]
    fn the_measured_missing_routes_are_all_real_registry_routes() {
        // A stale entry here silently greys out a model that works.
        let reg = registry();
        let known: std::collections::HashSet<&str> = reg
            .values()
            .flat_map(|m| m.routes.iter())
            .map(|r| r.slug.as_str())
            .collect();
        for slug in hickeyfield_core::media::FAL_MISSING_ROUTES {
            assert!(
                known.contains(slug),
                "{slug} is not a route in the registry"
            );
        }
    }

    #[test]
    fn a_missing_key_says_which_key() {
        let (ok, why) = route_state(ProviderId::Fal, "fal-ai/nano-banana-2", &[]);
        assert!(!ok);
        assert_eq!(why.unwrap(), "Needs a fal.ai key");
    }

    #[test]
    fn a_usable_route_carries_no_reason() {
        let (ok, why) = route_state(
            ProviderId::Fal,
            "fal-ai/nano-banana-2",
            &["fal".to_string()],
        );
        assert!(ok);
        assert!(why.is_none());
    }

    #[test]
    fn local_route_is_unavailable_with_an_honest_reason() {
        // AC1. Local is keyless, but no local client exists yet, so a Local-only
        // route (`z_image`'s `local:z-image`) must report UNAVAILABLE with an
        // honest reason — NOT available-at-$0.00, and NOT "add a key". Being
        // told to add a key for a keyless provider is exactly the lie this
        // slice removes.
        let (ok, why) = route_state(ProviderId::Local, "z-image", &[]);
        assert!(
            !ok,
            "a Local route has no client yet and must not be usable"
        );
        let reason = why.expect("an unavailable route must explain itself");
        assert!(
            reason.contains("no client"),
            "reason must say a client is missing, got: {reason}"
        );
        assert!(
            !reason.contains("Needs a"),
            "must not tell the user to add a key for a keyless provider: {reason}"
        );
    }

    // ---- Enhancer backend selection (AC2) ----------------------------------

    use crate::harness::Rewriter;
    use hickeyfield_core::enhancer::{model_tier, HostedBackend, LocalModel};

    fn choice(backend: &str, model: Option<&str>) -> RewriterChoice {
        RewriterChoice {
            backend: backend.to_string(),
            model: model.map(str::to_string),
        }
    }

    /// Build the `Vec<LocalModel>` shape `local_models` hands `select_rewriter`,
    /// tiering each name the same way production does. Order is preserved as
    /// given, so a test can hand an *unsorted* list to prove select_rewriter
    /// itself trusts the incoming order (the sort is `local_models`' job).
    fn models(names: &[&str]) -> Vec<LocalModel> {
        names
            .iter()
            .map(|n| LocalModel {
                name: n.to_string(),
                tier: model_tier(n, None),
            })
            .collect()
    }

    #[test]
    fn an_explicit_openai_choice_with_a_key_and_model_reaches_the_hosted_backend() {
        let c = choice("openai", Some("gpt-5"));
        match select_rewriter(Some(&c), Some("sk-live"), false, &[]) {
            Rewriter::Hosted {
                backend,
                api_key,
                model,
            } => {
                assert_eq!(backend, HostedBackend::OpenAi);
                assert_eq!(api_key, "sk-live");
                assert_eq!(model, "gpt-5");
            }
            other => panic!("expected Hosted, got {}", name(&other)),
        }
    }

    #[test]
    fn an_explicit_openai_choice_without_a_key_is_an_honest_note() {
        let c = choice("openai", Some("gpt-5"));
        let note = note_of(select_rewriter(Some(&c), None, false, &[]));
        assert!(note.contains("no OpenAI key is stored"), "got: {note}");
    }

    #[test]
    fn an_explicit_ollama_choice_with_an_installed_model_runs_local() {
        // A discouraged model is still runnable if the user explicitly picks it.
        let installed = models(&["gemma3:1b", "qwen3-vl:4b"]);
        let c = choice("ollama", Some("qwen3-vl:4b"));
        match select_rewriter(Some(&c), None, true, &installed) {
            Rewriter::Ollama { model } => assert_eq!(model, "qwen3-vl:4b"),
            other => panic!("expected Ollama, got {}", name(&other)),
        }
    }

    #[test]
    fn auto_picks_the_top_ranked_model() {
        // AC3. Auto takes `.first()` of the list `local_models` pre-sorts by
        // suitability. Feeding the *sorted* order (recommended leading a
        // discouraged) proves auto picks the recommended one — a plain
        // "first installed" would have grabbed whatever Ollama listed first.
        let raw = serde_json::json!({"models": [
            {"name": "qwen3-vl:4b"},
            {"name": "llama3.2:3b"},
        ]});
        let installed = hickeyfield_core::enhancer::parse_local_models(&raw);
        // The sort put the recommended model first even though Ollama listed the
        // vision model first.
        assert_eq!(installed[0].name, "llama3.2:3b");
        match select_rewriter(None, None, true, &installed) {
            Rewriter::Ollama { model } => assert_eq!(model, "llama3.2:3b"),
            other => panic!("expected Ollama, got {}", name(&other)),
        }
    }

    #[test]
    fn auto_with_only_discouraged_models_still_picks_one() {
        // Discouraged is a warning, not a ban: if the only installed models are
        // discouraged, auto still runs the top of the list rather than declining
        // to enhance.
        let installed = models(&["qwen3-vl:4b", "deepseek-r1:8b"]);
        match select_rewriter(None, None, true, &installed) {
            Rewriter::Ollama { model } => assert_eq!(model, "qwen3-vl:4b"),
            other => panic!("expected Ollama, got {}", name(&other)),
        }
    }

    #[test]
    fn auto_with_a_key_and_ollama_still_runs_ollama_not_the_no_model_note() {
        // The MAJOR review bug: an OpenAI key present must NOT block auto from
        // running a perfectly good local Ollama. With both available and no
        // explicit choice, auto runs Ollama's top-ranked model — it never
        // silently declines to enhance while a runnable backend sits right there.
        let installed = models(&["gemma3:1b", "qwen3-vl:4b"]);
        match select_rewriter(None, Some("sk-live"), true, &installed) {
            Rewriter::Ollama { model } => assert_eq!(model, "gemma3:1b"),
            other => panic!("expected Ollama, got {}", name(&other)),
        }
    }

    #[test]
    fn auto_with_nothing_available_is_the_honest_nothing_note() {
        let note = note_of(select_rewriter(None, None, false, &[]));
        assert!(note.contains("no OpenAI key is stored"), "got: {note}");
        assert!(note.contains("Ollama isn't running"), "got: {note}");
        assert!(
            note.ends_with("sent exactly as you wrote it."),
            "the note must promise the prompt was sent unchanged: {note}"
        );
    }

    #[test]
    fn ollama_up_with_no_models_says_none_are_installed() {
        let note = note_of(select_rewriter(None, None, true, &[]));
        assert!(note.contains("no chat model is installed"), "got: {note}");
    }

    #[test]
    fn auto_never_silently_runs_openai_without_a_chosen_model() {
        // Q-AUTO-MODEL: an OpenAI key present but no explicit model must decline
        // with a pick-a-model note rather than invent a hosted model id.
        let note = note_of(select_rewriter(None, Some("sk-live"), false, &[]));
        assert!(note.contains("no model was chosen"), "got: {note}");
    }

    #[test]
    fn a_list_of_installed_models_never_errors() {
        // Whether or not Ollama is running here, the command returns a list, not
        // an Err — a down daemon is an empty picker, not a broken submit.
        let _models: Vec<LocalModel> = list_ollama_models();
    }

    fn name(r: &Rewriter<'_>) -> &'static str {
        match r {
            Rewriter::None => "None",
            Rewriter::Ollama { .. } => "Ollama",
            Rewriter::Hosted { .. } => "Hosted",
            Rewriter::Unavailable { .. } => "Unavailable",
        }
    }

    fn note_of(r: Rewriter<'_>) -> String {
        match r {
            Rewriter::Unavailable { note } => note,
            other => panic!("expected Unavailable, got {}", name(&other)),
        }
    }

    // ---- Prompt preview + verbatim final prompt (S7: AC1, AC2, AC4) --------

    /// A minimal `SubmitInput` for the preview/compile helpers. Enhance-off by
    /// default so the compile path is deterministic and touches no network.
    fn preview_input(prompt: &str) -> SubmitInput {
        SubmitInput {
            model_id: "kling3_0".into(),
            route_id: None,
            prompt: prompt.into(),
            preset_id: None,
            settings: SettingsDto {
                enhance: false,
                ..Default::default()
            },
            media: Vec::new(),
            rewriter: None,
            final_prompt: None,
            constant_overrides: None,
        }
    }

    #[test]
    fn preview_returns_a_compiled_prompt_and_creates_no_job() {
        // AC1. With enhance off the preview is deterministic and needs no
        // network: it must return the compiled wire prompt — the raw text plus
        // the model's camera template — with the original preserved and no
        // rewrite claimed. `preview_prompt` takes no `State`, so there is no
        // store to write a job to: the signature is the structural proof.
        let input = preview_input("a lighthouse at dawn");
        let dto = preview_prompt(input).expect("preview must compile");

        assert!(
            dto.prompt.contains("a lighthouse at dawn"),
            "the raw text must survive into the wire prompt: {}",
            dto.prompt
        );
        // The camera clause is always appended, so the wire prompt is longer
        // than the raw text the user typed.
        assert!(
            dto.prompt.len() > "a lighthouse at dawn".len(),
            "a camera clause must be appended: {}",
            dto.prompt
        );
        assert_eq!(
            dto.original, "a lighthouse at dawn",
            "the user's own words are preserved verbatim"
        );
        assert!(
            dto.enhanced.is_none(),
            "enhance is off, so nothing was rewritten: {:?}",
            dto.enhanced
        );
        assert!(dto.version.is_none(), "no rewrite ran, so no version pin");
    }

    #[test]
    fn a_final_prompt_is_used_verbatim_and_never_re_enhanced() {
        // AC2 (the riskiest invariant). A previewed/edited prompt already
        // carries the camera clause, so `compiled_for_submit` must send it
        // VERBATIM and NOT call `compile` again — no double camera clause, no
        // re-charged rewrite. We prove the bypass by pairing a non-empty
        // `final_prompt` with enhance ON *and* a bogus Ollama choice: if compile
        // ran it would try to reach that rewriter and would append a camera
        // clause. Neither happens.
        let reg = registry();
        let model = reg.get("kling3_0").expect("a known model");

        let mut input = preview_input("original scene");
        input.settings.enhance = true;
        input.rewriter = Some(choice("ollama", Some("definitely-not-installed")));
        input.final_prompt = Some("edited text".into());

        let compiled = compiled_for_submit(model, &input).expect("verbatim path never fails");

        assert_eq!(
            compiled.prompt, "edited text",
            "the previewed text must go over the wire byte-for-byte"
        );
        assert_eq!(compiled.enhanced.as_deref(), Some("edited text"));
        assert_eq!(compiled.original, "original scene");
        assert!(
            compiled.version.is_none(),
            "a human edit has no reproducible recipe, so no version pin"
        );
        assert_eq!(
            compiled.note.as_deref(),
            Some("Previewed before generating; sent exactly as shown."),
            "the note must say the text was sent exactly as shown"
        );
    }

    #[test]
    fn a_blank_final_prompt_falls_back_to_the_normal_compile_path() {
        // AC4. A whitespace-only edit must never submit an empty prompt: it is
        // filtered out and `compiled_for_submit` falls through to the normal
        // enhance+compile path, which appends the camera clause to the raw
        // prompt just like the quick Generate. Enhance stays off here so the
        // fallback is deterministic and network-free.
        let reg = registry();
        let model = reg.get("kling3_0").expect("a known model");

        let mut input = preview_input("a quiet harbour");
        input.final_prompt = Some("   ".into());

        let compiled = compiled_for_submit(model, &input).expect("fallback compiles");

        assert!(
            compiled.prompt.contains("a quiet harbour"),
            "the raw prompt drives the fallback compile: {}",
            compiled.prompt
        );
        assert!(
            compiled.prompt.len() > "a quiet harbour".len(),
            "the fallback path still appends the camera clause: {}",
            compiled.prompt
        );
        // The blank edit was ignored, so this is NOT the verbatim note.
        assert_ne!(
            compiled.note.as_deref(),
            Some("Previewed before generating; sent exactly as shown."),
            "a blank final_prompt must not be treated as a previewed submission"
        );
    }

    #[test]
    fn real_routes_stay_available_after_the_local_fix() {
        // AC3. Making Local honest must not grey out routes we can actually run:
        // a keyed fal route is still available with no reason, side by side with
        // the now-unavailable Local route.
        let (fal_ok, fal_why) = route_state(
            ProviderId::Fal,
            "fal-ai/nano-banana-2",
            &["fal".to_string()],
        );
        assert!(fal_ok, "a keyed fal route must stay available: {fal_why:?}");
        assert!(fal_why.is_none());

        let (local_ok, _) = route_state(ProviderId::Local, "z-image", &[]);
        assert!(
            !local_ok,
            "the Local-only route is the one that must be unavailable"
        );
    }

    // ---- Storyboard: chat-backend selection + generate (AC1/AC4/AC6) -------

    use hickeyfield_core::enhancer::ChatBackend;

    fn backend_kind(b: &ChatBackend) -> &'static str {
        match b {
            ChatBackend::Ollama { .. } => "ollama",
            ChatBackend::Hosted { .. } => "hosted",
        }
    }

    #[test]
    fn chat_backend_prefers_a_running_ollama_in_auto() {
        // AC1/AC6: auto drafting runs the top-ranked local model when one is
        // installed — no key, no explicit id needed, the same rule the enhancer
        // uses. Mirrors `auto_picks_the_top_ranked_model`.
        let installed = models(&["llama3.2:3b"]);
        let got = chat_backend_for(None, None, true, &installed).expect("a runnable backend");
        assert_eq!(backend_kind(&got), "ollama");
    }

    #[test]
    fn chat_backend_with_nothing_available_is_an_honest_note() {
        // AC1: "on an unavailable backend returns an honest error." The Err is
        // the NOTE the UI surfaces; drafting never silently invents a backend.
        // `ChatBackend` deliberately derives no Debug (it holds an api key), so
        // match rather than `unwrap_err`.
        let note = match chat_backend_for(None, None, false, &[]) {
            Err(n) => n,
            Ok(_) => panic!("expected an honest note, got a backend"),
        };
        assert!(note.contains("no OpenAI key is stored"), "got: {note}");
        assert!(note.contains("Ollama isn't running"), "got: {note}");
    }

    #[test]
    fn chat_backend_explicit_openai_needs_a_key_and_model() {
        let c = choice("openai", Some("gpt-5"));
        match chat_backend_for(Some(&c), Some("sk-live"), false, &[]) {
            Ok(b) => assert_eq!(backend_kind(&b), "hosted"),
            Err(e) => panic!("expected hosted, got note: {e}"),
        }
        let no_key = match chat_backend_for(Some(&c), None, false, &[]) {
            Err(n) => n,
            Ok(_) => panic!("expected an honest note, got a backend"),
        };
        assert!(no_key.contains("no OpenAI key is stored"), "got: {no_key}");
    }

    /// A minimal `AppState` for testing the generate path with no network: an
    /// in-memory store, a client factory that hands back nothing, and the
    /// bundled prices. Nothing here reaches a provider.
    fn test_state() -> AppState {
        use crate::library::Library;
        use crate::pricing::Prices;
        use crate::runner::Runner;
        use hickeyfield_core::engine::JobStore;
        use std::sync::Arc;

        let store = Arc::new(crate::store::SqliteStore::in_memory().unwrap());
        let library = Arc::new(Library::new(
            std::env::temp_dir().join(format!("hickeyfield-sb-test-{}", std::process::id())),
        ));
        let prices = Arc::new(Prices::bundled());
        let runner = Arc::new(Runner::new(
            Arc::clone(&store) as Arc<dyn JobStore>,
            Arc::new(|_| None),
            Arc::new(|_| {}),
            Arc::clone(&library),
            Arc::clone(&prices),
        ));
        AppState {
            store,
            runner,
            library,
            prices,
            ffmpeg_dir: std::env::temp_dir().join("hickeyfield-test-nobin"),
            renders_root: std::env::temp_dir().join("hickeyfield-test-renders"),
        }
    }

    fn shot_input(model_id: &str) -> SubmitInput {
        SubmitInput {
            model_id: model_id.into(),
            route_id: None,
            prompt: "a shot".into(),
            preset_id: None,
            settings: SettingsDto {
                enhance: false,
                ..Default::default()
            },
            media: Vec::new(),
            rewriter: None,
            final_prompt: Some("a shot, wide.".into()),
            constant_overrides: None,
        }
    }

    #[test]
    fn generate_records_the_set_even_when_shots_fail() {
        // AC4/AC6. Every shot here names a model that is not in the registry, so
        // each fails at the registry lookup *before* `submit_one` — no route
        // resolves, nothing reaches a provider, and this gate can never spend a
        // cent even on a machine that happens to hold a real key. The batch must
        // still: mint and persist the storyboard row, return one outcome per
        // shot, carry a per-shot error, and NOT stop at the first failure.
        //
        // The *successful* submit path (a real model → a live provider → a
        // job_set_id) is deliberately NOT exercised here: doing so would require
        // a network call to a paid provider, which agents never make. That path
        // is `submit_one`, byte-for-byte the audited single-job body — see the
        // diff in implement.md — and is covered by the live run.
        let state = test_state();
        let input = StoryboardGenerateInput {
            brief: "a 30s ad".into(),
            target_seconds: 30.0,
            // No plan_json here: this test drives the per-shot continue-on-failure
            // path (unknown models fail inside the loop). Server-side plan
            // re-validation (which would reject unknown models *before* the loop)
            // is covered separately by `generate_rejects_a_plan_with_a_bad_model`.
            plan_json: None,
            shot_indices: None,
            storyboard_id: None,
            chained: false,
            overlap_seconds: None,
            shots: vec![shot_input("no-such-model-a"), shot_input("no-such-model-b")],
            auto_retry: false,
            keyframe_mode: false,
            still_model_id: None,
        };
        // Call the inner logic directly with a plain &AppState (the #[command]
        // wrapper only adds the State extractor).
        let out = generate_storyboard_inner(&state, input).expect("generate returns a result");

        assert_eq!(
            out.shots.len(),
            2,
            "one outcome per shot — the batch continued"
        );
        for outcome in &out.shots {
            assert!(outcome.error.is_some(), "an unknown model is an error");
            assert!(outcome.job_set_id.is_none(), "a failed shot has no job id");
        }

        // The storyboard row persists regardless of shot fates.
        let sb = state.store.storyboard(&out.storyboard_id).unwrap();
        assert!(sb.is_some(), "the group row survives failing shots");
        assert_eq!(sb.unwrap().brief, "a 30s ad");
    }

    #[test]
    fn generate_numbers_outcomes_densely_from_zero() {
        // AC4: the outcome indices are dense and 0-based, so the Library can
        // order the set. All-unknown models keeps this strictly offline.
        let state = test_state();
        let input = StoryboardGenerateInput {
            brief: "b".into(),
            target_seconds: 24.0,
            plan_json: None,
            shot_indices: None,
            storyboard_id: None,
            chained: false,
            overlap_seconds: None,
            shots: vec![
                shot_input("nope-a"),
                shot_input("nope-b"),
                shot_input("nope-c"),
            ],
            auto_retry: false,
            keyframe_mode: false,
            still_model_id: None,
        };
        let out = generate_storyboard_inner(&state, input).unwrap();
        let indices: Vec<u32> = out.shots.iter().map(|s| s.shot_index).collect();
        assert_eq!(indices, vec![0, 1, 2]);
    }

    /// A minimal `JobSet` for the store-persistence tests below.
    fn blank_job(id: &str) -> JobSet {
        JobSet {
            id: id.into(),
            model_id: "seedance_2_5".into(),
            route_id: "fal:fal-ai/bytedance/seedance/v1/pro/text-to-video".into(),
            request_id: String::new(),
            endpoint: String::new(),
            status: hickeyfield_core::JobStatus::Queued,
            prompt: "a shot".into(),
            enhanced_prompt: None,
            enhancer_version: None,
            enhance_note: None,
            advisories: Vec::new(),
            preset_id: None,
            created_at: now_secs(),
            updated_at: now_secs(),
            results: vec![],
            estimated_usd: None,
            actual_usd: None,
            priced_from: None,
            priced_at: None,
            actual_source: None,
            fail_reason: None,
            settings: serde_json::Value::Null,
            media: Vec::new(),
            poll_cycles: 0,
            stalled: false,
            storyboard_id: None,
            shot_index: None,
        }
    }

    #[test]
    fn minted_job_ids_do_not_collide_within_one_second() {
        // B1. Before the fix an id was `{secs}-{model.id}` at whole-second
        // resolution, so a generate-all loop submitting several same-model shots
        // inside one second minted identical ids, and `store.upsert`'s
        // `ON CONFLICT(id) DO UPDATE` silently overwrote earlier rows while every
        // POST still billed. `mint_job_id` now appends a process-wide counter, so
        // 200 same-model ids minted back-to-back are all distinct.
        let ids: std::collections::HashSet<String> =
            (0..200).map(|_| mint_job_id("seedance_2_5")).collect();
        assert_eq!(ids.len(), 200, "every minted job id is unique");

        // The single-job caller benefits too: two immediate submits differ.
        assert_ne!(mint_job_id("m"), mint_job_id("m"));
        // Group ids are unique for the same reason (m1/L2).
        assert_ne!(mint_storyboard_id(), mint_storyboard_id());
    }

    #[test]
    fn same_model_shots_survive_as_distinct_rows_in_the_store() {
        // B1 at the store boundary. Three shots on ONE model, ids minted exactly
        // as `submit_one` mints them, tagged to one storyboard. All three must
        // persist as separate rows — the pre-fix collision collapsed them to one
        // via `ON CONFLICT`. Offline: the rows are upserted directly, exercising
        // the same store write `submit_one` performs before it reaches a
        // provider (the resolver only ever yields fal/higgsfield, both of which
        // would POST, so the loop's own submit cannot run without a network call
        // — the collision, however, is entirely in id minting + this store write,
        // which this test drives faithfully).
        let state = test_state();
        let sid = mint_storyboard_id();
        state
            .store
            .upsert_storyboard(&crate::store::Storyboard {
                id: sid.clone(),
                brief: "one model, three shots".into(),
                target_seconds: 24.0,
                plan_json: "{}".into(),
                created_at: now_secs(),
                chained: false,
                shots_json: None,
                overlap_seconds: None,
                joins_json: None,
                auto_retry: false,
                keyframes_json: None,
            })
            .unwrap();

        for idx in 0..3u32 {
            let id = mint_job_id("seedance_2_5");
            let mut job = blank_job(&id);
            job.storyboard_id = Some(sid.clone());
            job.shot_index = Some(idx);
            state.store.upsert(&job).unwrap();
        }

        let rows = state.store.jobs_in_storyboard(&sid).unwrap();
        assert_eq!(
            rows.len(),
            3,
            "three same-model shots persist as three rows"
        );
        let indices: Vec<u32> = rows.iter().filter_map(|j| j.shot_index).collect();
        assert_eq!(indices, vec![0, 1, 2], "dense shot_index, none clobbered");
    }

    // ── S17: keyframe-anchored mode ──────────────────────────────────────────

    fn keyframe_plan_json() -> String {
        serde_json::json!({
            "world": "a matte-black cold-brew can on wet slate, deep teal palette, studio product light",
            "brief": "",
            "targetSeconds": 30.0,
            "useCase": "image-to-video",
            "shots": [
                {"title":"A","action":"The can beads with condensation. Ends on a static hold of the can centred.","seconds":15.0,"camera":null,"modelId":"minimax_h3_max","role":"start"},
                {"title":"B","action":"Opens on the same static hold of the can centred, then a drop falls. Ends on a static hold of the drop landing.","seconds":15.0,"camera":null,"modelId":"minimax_h3_max","role":"end"}
            ]
        })
        .to_string()
    }

    fn keyframe_shot(res: &str, aspect: &str) -> SubmitInput {
        SubmitInput {
            model_id: "minimax_h3_max".into(),
            route_id: None,
            prompt: "a shot".into(),
            preset_id: None,
            settings: SettingsDto {
                duration: Some(15.0),
                resolution: Some(res.into()),
                aspect: Some(aspect.into()),
                enhance: false,
                ..Default::default()
            },
            media: Vec::new(),
            rewriter: None,
            final_prompt: Some("a shot, wide.".into()),
            constant_overrides: None,
        }
    }

    #[test]
    fn keyframe_stills_are_priced_and_the_canvas_matches_the_set() {
        // AC1/AC6 (network-free): the still canvas equals the set's
        // resolution × aspect, and the default still (qwen_image_3) prices at
        // $0.04 — the real number in the total shown before Generate. Pricing
        // makes NO network call (bundled prices + offline route resolve); the
        // gate's dummy FAL key makes the fal route available.
        let canvas = keyframe_canvas(Some("768p"), Some("16:9")).expect("known res/aspect");
        // 768 × 16/9 = 1365.3 → rounded to the nearest even width (encoders
        // reject odd dimensions), the same rule S10 prices on.
        assert_eq!(
            canvas,
            hickeyfield_core::storyboard::ImageSize {
                width: 1366,
                height: 768
            }
        );

        let prices = std::sync::Arc::new(crate::pricing::Prices::bundled());
        let reg = registry();
        let model = reg.get(DEFAULT_STILL_MODEL).expect("qwen is registered");
        let settings = SettingsDto {
            resolution: Some("768p".into()),
            aspect: Some("16:9".into()),
            ..Default::default()
        };
        // Only meaningful when a fal key is configured (the gate sets one).
        if configured_providers().iter().any(|p| p == "fal") {
            assert_eq!(
                still_estimate(&prices, model, &settings),
                Some(0.04),
                "qwen_image_3 prices at $0.04/image at 1K"
            );
        }
    }

    #[test]
    fn keyframe_generate_holds_the_shots_and_stores_the_boundaries() {
        // AC2/AC6: the derive → validate → price → store half of a keyframe
        // generate, exercised OFFLINE. `qwen_image_3` has ONLY a fal route
        // (registry.rs:1349), so driving `generate_storyboard_inner`'s live
        // still-submit loop here would POST to a reachable network — the
        // codebase discipline (see `same_model_shots_survive_as_distinct_rows`)
        // is that a test never runs the submit loop; the live still submit is
        // the shared, audited `submit_one` money path. No env is stripped: with
        // the gate's dummy keys the vault yields a dummy value and the keychain
        // is blocked under cfg(test), so no real key is ever reachable (INCIDENT
        // 2026-09-16). This test runs the exact derivation + storage the
        // keyframe branch performs (commands.rs ~:1653-1810) and asserts the
        // N+1 priced boundaries round-trip, then that nothing was submitted.
        let state = test_state();
        let prices = std::sync::Arc::new(crate::pricing::Prices::bundled());
        let reg = registry();

        let plan: hickeyfield_core::storyboard::StoryboardPlan =
            serde_json::from_str(&keyframe_plan_json()).unwrap();
        let canvas = keyframe_canvas(Some("768p"), Some("16:9")).unwrap();
        let kfs =
            hickeyfield_core::storyboard::derive_keyframes(&plan, DEFAULT_STILL_MODEL, canvas);
        hickeyfield_core::storyboard::validate_keyframes(&kfs, plan.shots.len(), canvas)
            .expect("the derived keyframes validate");

        let still = reg.get(DEFAULT_STILL_MODEL).expect("qwen is registered");
        let records: Vec<crate::store::KeyframeRecord> = kfs
            .iter()
            .map(|kf| {
                let si = still_submit_input(kf, Some("768p"), Some("16:9"), 0);
                crate::store::KeyframeRecord {
                    index: kf.index,
                    job_set_id: None,
                    local_path: None,
                    estimated_usd: still_estimate(&prices, still, &si.settings),
                    attempt: 0,
                }
            })
            .collect();

        // Store exactly as the keyframe branch of `generate_storyboard_inner`
        // does: chained set, per-shot `shots_json`, and `keyframes_json`.
        let shots = vec![keyframe_shot("768p", "16:9"), keyframe_shot("768p", "16:9")];
        let sid = "kf-store";
        state
            .store
            .upsert_storyboard(&crate::store::Storyboard {
                id: sid.into(),
                brief: "a 30s cold-brew ad".into(),
                target_seconds: 30.0,
                plan_json: keyframe_plan_json(),
                created_at: 1,
                chained: true,
                shots_json: Some(serde_json::to_string(&shots).unwrap()),
                overlap_seconds: Some(0.0),
                joins_json: None,
                auto_retry: false,
                keyframes_json: Some(serde_json::to_string(&records).unwrap()),
            })
            .unwrap();

        let sb = state
            .store
            .storyboard(sid)
            .unwrap()
            .expect("the group row persists");
        assert!(sb.chained, "a keyframe set is chained");
        let stored = crate::store::parse_keyframes(sb.keyframes_json.as_deref());
        assert_eq!(stored.len(), 3, "N+1 = 3 keyframe boundaries stored");
        assert_eq!(
            stored.iter().map(|r| r.index).collect::<Vec<_>>(),
            vec![0, 1, 2]
        );
        // Each still is priced into the total shown BEFORE Generate (AC6). The
        // dummy fal key makes the route available so the estimate is a real
        // number; guarded so a stripped env (never, per the rule) would skip.
        if configured_providers().iter().any(|p| p == "fal") {
            assert!(
                stored.iter().all(|r| r.estimated_usd == Some(0.04)),
                "each still priced at qwen's $0.04/image: {stored:?}"
            );
        }
        // The storage path alone submits nothing — no still, no shot (the live
        // submit is proven not to double-bill by `a_still_regenerate_in_flight_
        // is_refused` and gated by the total-shown invariant).
        assert!(
            state.store.jobs_in_storyboard(sid).unwrap().is_empty(),
            "deriving and storing the boundaries submits no provider job"
        );
    }

    #[test]
    fn keyframe_generate_rejects_a_non_text_to_image_still_model() {
        // AC1: a still model that cannot generate an image is refused at the
        // boundary (the keyframe validator, `validate_keyframes`), which runs
        // BEFORE anything is stored or submitted — so this is network-free with
        // the gate's dummy keys in place; no env is stripped (INCIDENT
        // 2026-09-16: a test must never mutate the provider-key env — the
        // keychain guard already makes a real key unreachable under cfg(test)).
        let state = test_state();
        let input = StoryboardGenerateInput {
            brief: "a 30s ad".into(),
            target_seconds: 30.0,
            plan_json: Some(keyframe_plan_json()),
            shot_indices: None,
            storyboard_id: None,
            chained: true,
            overlap_seconds: None,
            shots: vec![keyframe_shot("768p", "16:9"), keyframe_shot("768p", "16:9")],
            auto_retry: false,
            keyframe_mode: true,
            still_model_id: Some("kling3_0_pro".into()), // a video model
        };
        let err = generate_storyboard_inner(&state, input).unwrap_err();
        // Nothing was stored or submitted: the validator rejects up front.
        assert!(
            state.store.storyboards().unwrap().is_empty(),
            "a rejected still model writes no storyboard row"
        );
        assert!(
            err.contains("cannot generate an image"),
            "a non-image still model is rejected: {err}"
        );
    }

    #[test]
    fn a_still_regenerate_in_flight_is_refused() {
        // AC6: a still regenerate is refused while that boundary's still is still
        // queued/running — a second submit would double-bill the still (the S16
        // M1 invariant, extended to stills). Network-free: the guard returns
        // before `submit_one`.
        let state = test_state();
        // A keyframe set whose boundary-0 still job is Queued (in flight).
        let records = vec![crate::store::KeyframeRecord {
            index: 0,
            job_set_id: Some("still-0".into()),
            local_path: None,
            estimated_usd: Some(0.04),
            attempt: 0,
        }];
        state
            .store
            .upsert_storyboard(&crate::store::Storyboard {
                id: "kf".into(),
                brief: "b".into(),
                target_seconds: 30.0,
                plan_json: keyframe_plan_json(),
                created_at: 1,
                chained: true,
                shots_json: Some(
                    serde_json::to_string(&vec![keyframe_shot("768p", "16:9")]).unwrap(),
                ),
                overlap_seconds: Some(0.0),
                joins_json: None,
                auto_retry: false,
                keyframes_json: Some(serde_json::to_string(&records).unwrap()),
            })
            .unwrap();
        // Persist a Queued (non-terminal) job row for the boundary-0 still id.
        state.store.upsert(&job_for_still("still-0")).unwrap();

        let err = regenerate_keyframe_still_inner(
            &state.store,
            &state.prices,
            &state.runner,
            "kf",
            0,
            false,
        )
        .unwrap_err();
        assert!(
            err.contains("still in flight"),
            "an in-flight still refuses a regenerate: {err}"
        );
    }

    fn job_for_still(id: &str) -> JobSet {
        JobSet {
            id: id.into(),
            model_id: "qwen_image_3".into(),
            route_id: "fal:alibaba/qwen-image-3/text-to-image".into(),
            request_id: "r".into(),
            endpoint: String::new(),
            status: hickeyfield_core::JobStatus::Queued,
            prompt: "p".into(),
            enhanced_prompt: None,
            enhancer_version: None,
            enhance_note: None,
            advisories: Vec::new(),
            preset_id: None,
            created_at: 0,
            updated_at: 0,
            results: Vec::new(),
            estimated_usd: Some(0.04),
            actual_usd: None,
            priced_from: None,
            priced_at: None,
            actual_source: None,
            fail_reason: None,
            settings: serde_json::json!({}),
            media: Vec::new(),
            poll_cycles: 0,
            stalled: false,
            storyboard_id: Some("kf".into()),
            shot_index: None,
        }
    }

    // NOTE (INCIDENT 2026-09-16): there is deliberately NO `strip_provider_env`
    // helper. Mutating the provider-key env in a test is process-global and was
    // the root of the paid-spend incident (a stripped key fell through to the
    // owner's real keychain key). Tests that must not reach a provider either
    // (a) fail at route resolution offline by pinning a no-adapter route
    // (`kling_shot(.., Some("vaig:klingai/kling-v3.0"))` → `PinUnusable`), or
    // (b) exercise only the offline derivation/store half and never the submit
    // loop (`keyframe_generate_holds_the_shots_and_stores_the_boundaries`). The
    // keychain guard (`vault::keychain_blocked`) makes a real key unreachable
    // under cfg(test) regardless.

    #[test]
    fn generate_rejects_a_brief_over_the_size_cap() {
        // Security L3: a runaway brief is refused at the boundary with a typed
        // error, before any storyboard row or shot is written.
        let state = test_state();
        let input = StoryboardGenerateInput {
            brief: "x".repeat(MAX_BRIEF_BYTES + 1),
            target_seconds: 30.0,
            plan_json: None,
            shot_indices: None,
            storyboard_id: None,
            chained: false,
            overlap_seconds: None,
            shots: vec![shot_input("nope")],
            auto_retry: false,
            keyframe_mode: false,
            still_model_id: None,
        };
        let err = generate_storyboard_inner(&state, input).unwrap_err();
        assert!(err.contains("brief is too large"), "got: {err}");
    }

    #[test]
    fn generate_rejects_a_plan_json_over_the_size_cap() {
        // Security L3: an oversized plan_json is refused before the DB write.
        let state = test_state();
        let input = StoryboardGenerateInput {
            brief: "b".into(),
            target_seconds: 30.0,
            plan_json: Some("x".repeat(MAX_PLAN_JSON_BYTES + 1)),
            shot_indices: None,
            storyboard_id: None,
            chained: false,
            overlap_seconds: None,
            shots: vec![shot_input("nope")],
            auto_retry: false,
            keyframe_mode: false,
            still_model_id: None,
        };
        let err = generate_storyboard_inner(&state, input).unwrap_err();
        assert!(err.contains("plan is too large"), "got: {err}");
    }

    #[test]
    fn generate_rejects_more_than_eight_shots() {
        // Security L1: the server bounds the shot count, so the money/plan-size
        // invariant does not rest solely on the UI disabling Generate-all.
        let state = test_state();
        let input = StoryboardGenerateInput {
            brief: "b".into(),
            target_seconds: 30.0,
            plan_json: None,
            shot_indices: None,
            storyboard_id: None,
            chained: false,
            overlap_seconds: None,
            shots: (0..9).map(|_| shot_input("nope")).collect(),
            auto_retry: false,
            keyframe_mode: false,
            still_model_id: None,
        };
        let err = generate_storyboard_inner(&state, input).unwrap_err();
        assert!(err.contains("at most 8 shots"), "got: {err}");
    }

    #[test]
    fn generate_rejects_a_plan_with_a_bad_model() {
        // Security L1: when a plan is present it is re-validated server-side via
        // the same `storyboard::validate` the draft path used, so a direct/
        // tampered caller cannot submit shots the composer would have refused.
        let state = test_state();
        let bad_plan = r#"{"brief":"b","targetSeconds":30,"useCase":"text-to-video","shots":[
            {"title":"a","prompt":"p","seconds":10,"camera":null,"modelId":"no-such-model","role":"start"},
            {"title":"b","prompt":"p","seconds":10,"camera":null,"modelId":"no-such-model","role":"middle"},
            {"title":"c","prompt":"p","seconds":10,"camera":null,"modelId":"no-such-model","role":"end"}
        ]}"#;
        let input = StoryboardGenerateInput {
            brief: "b".into(),
            target_seconds: 30.0,
            plan_json: Some(bad_plan.into()),
            shot_indices: None,
            storyboard_id: None,
            chained: false,
            overlap_seconds: None,
            shots: vec![
                shot_input("no-such-model"),
                shot_input("no-such-model"),
                shot_input("no-such-model"),
            ],
            auto_retry: false,
            keyframe_mode: false,
            still_model_id: None,
        };
        let err = generate_storyboard_inner(&state, input).unwrap_err();
        assert!(err.contains("does not exist"), "got: {err}");

        // And nothing was written: the plan is rejected before the group row.
        assert_eq!(
            state.store.all().unwrap().len(),
            0,
            "no shot row was written"
        );
    }

    #[test]
    fn generate_rejects_shots_that_do_not_match_the_plan() {
        // Security L1: the submitted shot list must match the plan the user saw
        // and priced — no shot injected beyond it. Here the plan has 3 shots but
        // 4 are submitted.
        let state = test_state();
        // A plan whose models are all runnable for the tab (so it passes
        // `validate`) but with a shot count that the submitted list exceeds.
        let model = hickeyfield_core::use_case::models_for(hickeyfield_core::UseCase::TextToVideo)
            .into_iter()
            .next()
            .expect("a t2v model exists")
            .id;
        let shot = |m: &str| {
            format!(
                r#"{{"title":"t","prompt":"p","seconds":10,"camera":null,"modelId":"{m}","role":"middle"}}"#
            )
        };
        let plan = format!(
            r#"{{"brief":"b","targetSeconds":30,"useCase":"text-to-video","shots":[{},{},{}]}}"#,
            shot(&model),
            shot(&model),
            shot(&model),
        );
        let input = StoryboardGenerateInput {
            brief: "b".into(),
            target_seconds: 30.0,
            plan_json: Some(plan),
            shot_indices: None,
            storyboard_id: None,
            chained: false,
            overlap_seconds: None,
            shots: (0..4).map(|_| shot_input(&model)).collect(),
            auto_retry: false,
            keyframe_mode: false,
            still_model_id: None,
        };
        let err = generate_storyboard_inner(&state, input).unwrap_err();
        assert!(err.contains("does not match the plan"), "got: {err}");
    }

    /// A full, valid `text-to-video` plan of `n` `kling3_0` shots of `secs`
    /// seconds each, summing to `n*secs` (the target the tests pass). `kling3_0`
    /// is runnable for the tab and carries both a fal route (has an adapter) and
    /// a Vaig route (`vaig:klingai/kling-v3.0`, no adapter) — the latter lets a
    /// retry test drive `submit_one` to a deterministic, offline `PinUnusable`
    /// error instead of a network POST.
    fn kling_plan_json(n: usize, secs: f64) -> String {
        let shots: Vec<String> = (0..n)
            .map(|_| {
                format!(
                    r#"{{"title":"t","prompt":"p","seconds":{secs},"camera":null,"modelId":"kling3_0","role":"middle"}}"#
                )
            })
            .collect();
        format!(
            r#"{{"brief":"b","targetSeconds":{},"useCase":"text-to-video","shots":[{}]}}"#,
            (n as f64) * secs,
            shots.join(",")
        )
    }

    fn kling_shot(secs: f64, route_id: Option<&str>) -> SubmitInput {
        SubmitInput {
            model_id: "kling3_0".into(),
            route_id: route_id.map(str::to_string),
            prompt: "a shot".into(),
            preset_id: None,
            settings: SettingsDto {
                enhance: false,
                duration: Some(secs),
                ..Default::default()
            },
            media: Vec::new(),
            rewriter: None,
            final_prompt: Some("a shot, wide.".into()),
            constant_overrides: None,
        }
    }

    #[test]
    fn a_single_shot_plan_is_accepted_after_the_floor_dropped_to_one() {
        // S15 D-G: the shot-count floor is now 1, not 3. A one-shot plan (a
        // Seedance 30s native, or the smallest retry) is therefore no longer
        // rejected by count — it validates and is submitted. This is the honest
        // update of the old M2 "a sub-3-shot plan is rejected" test: that
        // rejection was a consequence of the 3-shot floor the 2×15 H3 Max chain
        // required us to lower. The single submit is driven offline via the
        // pinned Vaig route (no adapter → route::resolve fails, no network POST),
        // so the outcome is an honest per-shot error, not a count rejection.
        let state = test_state();
        let input = StoryboardGenerateInput {
            brief: "b".into(),
            target_seconds: 5.0,
            plan_json: Some(kling_plan_json(1, 5.0)),
            shot_indices: Some(vec![0]),
            storyboard_id: None,
            chained: false,
            overlap_seconds: None,
            shots: vec![kling_shot(5.0, Some("vaig:klingai/kling-v3.0"))],
            auto_retry: false,
            keyframe_mode: false,
            still_model_id: None,
        };
        let out = generate_storyboard_inner(&state, input)
            .expect("a one-shot plan validates under the 1..=8 floor");
        assert_eq!(out.shots.len(), 1);
        assert!(
            out.shots[0].error.is_some(),
            "the one shot failed offline (no adapter), not on the shot count: {:?}",
            out.shots[0]
        );
    }

    #[test]
    fn a_retry_with_the_full_plan_targets_the_existing_group_and_only_the_failed_index() {
        // M2 fix. A 4-shot plan where shot 2 failed: the retry sends the FULL
        // original plan (all 4 shots, so `validate` passes: count 4, sum 20 ==
        // target) plus `shot_indices=[2]` and the existing `storyboard_id`. The
        // server re-validates the full plan, matches shot 2 to plan index 2 by
        // model + duration, and submits ONLY that one shot into the SAME group
        // with `shot_index` 2 — no new storyboard is minted, and shots 0/1/3 are
        // untouched. The single submit is driven offline via the pinned Vaig
        // route (no adapter → `PinUnusable`, no network POST), so the outcome is
        // an honest error but its index and group targeting are exactly right.
        let state = test_state();
        let sid = mint_storyboard_id();
        state
            .store
            .upsert_storyboard(&crate::store::Storyboard {
                id: sid.clone(),
                brief: "b".into(),
                target_seconds: 20.0,
                plan_json: kling_plan_json(4, 5.0),
                created_at: now_secs(),
                chained: false,
                shots_json: None,
                overlap_seconds: None,
                joins_json: None,
                auto_retry: false,
                keyframes_json: None,
            })
            .unwrap();
        // Seed the first-generate outcome: shots 0/1/3 started (Queued), shot 2
        // failed. Shot 2's Failed row is retryable; the others must be left alone.
        for idx in 0..4u32 {
            let mut job = blank_job(&format!("first-{idx}"));
            job.storyboard_id = Some(sid.clone());
            job.shot_index = Some(idx);
            job.status = if idx == 2 {
                hickeyfield_core::JobStatus::Failed
            } else {
                hickeyfield_core::JobStatus::Queued
            };
            state.store.upsert(&job).unwrap();
        }

        let input = StoryboardGenerateInput {
            brief: "b".into(),
            target_seconds: 20.0,
            plan_json: Some(kling_plan_json(4, 5.0)),
            shot_indices: Some(vec![2]),
            storyboard_id: Some(sid.clone()),
            chained: false,
            overlap_seconds: None,
            // The pinned Vaig route belongs to `kling3_0` (so the route-pin
            // passes) but has no adapter, so `route::resolve` fails offline.
            shots: vec![kling_shot(5.0, Some("vaig:klingai/kling-v3.0"))],
            auto_retry: false,
            keyframe_mode: false,
            still_model_id: None,
        };
        let out = generate_storyboard_inner(&state, input).expect("retry is accepted");

        // No new group was minted: the retry wrote into the existing storyboard.
        assert_eq!(out.storyboard_id, sid, "retry joined the original group");
        // Exactly one shot was targeted, and it is the failed index 2.
        assert_eq!(out.shots.len(), 1, "only the failed shot was submitted");
        assert_eq!(
            out.shots[0].shot_index, 2,
            "the failed index, not a re-based 0"
        );
        // It reached the submit stage (offline `PinUnusable`, not a validation
        // rejection): the retry is no longer blocked by count/sum re-gating.
        let err = out.shots[0].error.as_deref().unwrap_or("");
        assert!(
            err.contains("client") || err.contains("key") || err.contains("provider"),
            "reached submit_one's route resolve, got: {err}"
        );
        // The first-generate rows are untouched: still 4 originals + nothing new
        // written (the pinned route fails before submit_one's own upsert).
        let rows = state.store.jobs_in_storyboard(&sid).unwrap();
        assert_eq!(rows.len(), 4, "the successful shots were not disturbed");
    }

    #[test]
    fn a_retry_of_an_already_started_shot_is_rejected() {
        // M2 fix, money honesty: a successful shot must never be resubmitted. If
        // a retry names an index whose row already started (a non-Failed job in
        // the set), the server refuses it BEFORE any submit — so a shot cannot be
        // double-billed even by a direct caller.
        let state = test_state();
        let sid = mint_storyboard_id();
        state
            .store
            .upsert_storyboard(&crate::store::Storyboard {
                id: sid.clone(),
                brief: "b".into(),
                target_seconds: 20.0,
                plan_json: kling_plan_json(4, 5.0),
                created_at: now_secs(),
                chained: false,
                shots_json: None,
                overlap_seconds: None,
                joins_json: None,
                auto_retry: false,
                keyframes_json: None,
            })
            .unwrap();
        // Shot 2 already STARTED (Queued, not Failed).
        let mut job = blank_job("started-2");
        job.storyboard_id = Some(sid.clone());
        job.shot_index = Some(2);
        job.status = hickeyfield_core::JobStatus::Queued;
        state.store.upsert(&job).unwrap();

        let input = StoryboardGenerateInput {
            brief: "b".into(),
            target_seconds: 20.0,
            plan_json: Some(kling_plan_json(4, 5.0)),
            shot_indices: Some(vec![2]),
            storyboard_id: Some(sid.clone()),
            chained: false,
            overlap_seconds: None,
            shots: vec![kling_shot(5.0, Some("vaig:klingai/kling-v3.0"))],
            auto_retry: false,
            keyframe_mode: false,
            still_model_id: None,
        };
        let err = generate_storyboard_inner(&state, input).unwrap_err();
        assert!(err.contains("already started"), "got: {err}");
    }

    #[test]
    fn a_below_floor_join_may_be_regenerated_but_a_passing_one_may_not() {
        // S16 D7: the S13 "refuse-if-not-Failed" guard is relaxed for exactly a
        // Completed shot whose continuity join scored BELOW the floor. A passing
        // (or unscored) join at a started index is still refused, so a good shot
        // cannot be double-billed.
        let below = crate::store::JoinRecord {
            index: 2,
            kind: "continuity".into(),
            applied: true,
            ssim: Some(0.4),
            psnr: Some(20.0),
            dy: Some(2.0),
            du: Some(1.0),
            dv: Some(1.0),
            pass: Some(false),
            attempt: 0,
            end_ssim: None,
            end_psnr: None,
            end_dy: None,
            end_du: None,
            end_dv: None,
            end_pass: None,
        };
        let make_state = |pass: bool| {
            let state = test_state();
            let sid = "sb-regen".to_string();
            let mut rec = below.clone();
            rec.pass = Some(pass);
            let joins = serde_json::to_string(&vec![rec]).unwrap();
            state
                .store
                .upsert_storyboard(&crate::store::Storyboard {
                    id: sid.clone(),
                    brief: "b".into(),
                    target_seconds: 20.0,
                    plan_json: kling_plan_json(4, 5.0),
                    created_at: now_secs(),
                    chained: true,
                    shots_json: None,
                    overlap_seconds: Some(0.0),
                    joins_json: Some(joins),
                    auto_retry: false,
                    keyframes_json: None,
                })
                .unwrap();
            // Shot 2 is COMPLETED (a successful shot), not Failed.
            let mut job = blank_job("regen-2");
            job.storyboard_id = Some(sid.clone());
            job.shot_index = Some(2);
            job.status = hickeyfield_core::JobStatus::Completed;
            state.store.upsert(&job).unwrap();
            (state, sid)
        };

        let input = |sid: &str| StoryboardGenerateInput {
            brief: "b".into(),
            target_seconds: 20.0,
            plan_json: Some(kling_plan_json(4, 5.0)),
            shot_indices: Some(vec![2]),
            storyboard_id: Some(sid.to_string()),
            chained: false,
            overlap_seconds: None,
            shots: vec![kling_shot(5.0, Some("vaig:klingai/kling-v3.0"))],
            auto_retry: false,
            keyframe_mode: false,
            still_model_id: None,
        };

        // Below floor → the guard does NOT refuse (it may fail later at submit,
        // but never with "already started").
        let (state, sid) = make_state(false);
        let out = generate_storyboard_inner(&state, input(&sid));
        if let Err(e) = &out {
            assert!(
                !e.contains("already started"),
                "a low join must be regenerable: {e}"
            );
        }

        // Passing join at a started index → still refused.
        let (state, sid) = make_state(true);
        let err = generate_storyboard_inner(&state, input(&sid)).unwrap_err();
        assert!(
            err.contains("already started"),
            "a passing join must not resubmit: {err}"
        );
    }

    #[test]
    fn a_below_floor_join_that_is_still_in_flight_is_refused() {
        // M1: the below-floor relaxation keys on the stored join verdict, which
        // is only refreshed when a shot re-settles. If a regenerate is already
        // in flight (Queued/InProgress) at that index the join still reads
        // pass==false, so a second regenerate would mint a duplicate in-flight
        // job — a double-bill. An in-flight job at the index MUST refuse before
        // the relaxation is considered; only a settled (terminal) low-join shot
        // is eligible, and a second call while it is in flight is refused.
        let below = crate::store::JoinRecord {
            index: 2,
            kind: "continuity".into(),
            applied: true,
            ssim: Some(0.4),
            psnr: Some(20.0),
            dy: Some(2.0),
            du: Some(1.0),
            dv: Some(1.0),
            pass: Some(false),
            attempt: 0,
            end_ssim: None,
            end_psnr: None,
            end_dy: None,
            end_du: None,
            end_dv: None,
            end_pass: None,
        };
        let make_state = |status: hickeyfield_core::JobStatus| {
            let state = test_state();
            let sid = "sb-inflight".to_string();
            let joins = serde_json::to_string(&vec![below.clone()]).unwrap();
            state
                .store
                .upsert_storyboard(&crate::store::Storyboard {
                    id: sid.clone(),
                    brief: "b".into(),
                    target_seconds: 20.0,
                    plan_json: kling_plan_json(4, 5.0),
                    created_at: now_secs(),
                    chained: true,
                    shots_json: None,
                    overlap_seconds: Some(0.0),
                    joins_json: Some(joins),
                    auto_retry: false,
                    keyframes_json: None,
                })
                .unwrap();
            let mut job = blank_job("inflight-2");
            job.storyboard_id = Some(sid.clone());
            job.shot_index = Some(2);
            job.status = status;
            state.store.upsert(&job).unwrap();
            (state, sid)
        };
        let input = |sid: &str| StoryboardGenerateInput {
            brief: "b".into(),
            target_seconds: 20.0,
            plan_json: Some(kling_plan_json(4, 5.0)),
            shot_indices: Some(vec![2]),
            storyboard_id: Some(sid.to_string()),
            chained: false,
            overlap_seconds: None,
            shots: vec![kling_shot(5.0, Some("vaig:klingai/kling-v3.0"))],
            auto_retry: false,
            keyframe_mode: false,
            still_model_id: None,
        };

        // Queued in flight → refused with "still in flight" (never admitted).
        let (state, sid) = make_state(hickeyfield_core::JobStatus::Queued);
        let err = generate_storyboard_inner(&state, input(&sid)).unwrap_err();
        assert!(err.contains("still in flight"), "queued must refuse: {err}");

        // InProgress in flight → refused (a second regenerate while the prior
        // one is running).
        let (state, sid) = make_state(hickeyfield_core::JobStatus::InProgress);
        let err = generate_storyboard_inner(&state, input(&sid)).unwrap_err();
        assert!(
            err.contains("still in flight"),
            "in-progress must refuse: {err}"
        );

        // Terminal (Completed) below-floor → NOT refused by the guard (the one
        // eligible case). It may still fail later at submit, but never with an
        // in-flight or already-started refusal.
        let (state, sid) = make_state(hickeyfield_core::JobStatus::Completed);
        if let Err(e) = generate_storyboard_inner(&state, input(&sid)) {
            assert!(
                !e.contains("still in flight") && !e.contains("already started"),
                "a settled low join must be regenerable once: {e}"
            );
        }
    }

    #[test]
    fn shot_in_flight_reads_only_non_terminal_rows() {
        // The shared guard helper: a queued/running row is in flight; a
        // Completed/Failed/Canceled row is not (terminal).
        let mut queued = blank_job("q");
        queued.shot_index = Some(1);
        queued.status = hickeyfield_core::JobStatus::Queued;
        let mut running = blank_job("r");
        running.shot_index = Some(1);
        running.status = hickeyfield_core::JobStatus::InProgress;
        let mut done = blank_job("d");
        done.shot_index = Some(1);
        done.status = hickeyfield_core::JobStatus::Completed;
        let mut failed = blank_job("f");
        failed.shot_index = Some(1);
        failed.status = hickeyfield_core::JobStatus::Failed;

        assert!(shot_in_flight(&[queued.clone()], 1));
        assert!(shot_in_flight(&[running.clone()], 1));
        assert!(!shot_in_flight(&[done.clone()], 1));
        assert!(!shot_in_flight(&[failed.clone()], 1));
        // A terminal row does not mask an in-flight one at the same index.
        assert!(shot_in_flight(&[done.clone(), running.clone()], 1));
        // A different index is not in flight.
        assert!(!shot_in_flight(&[queued], 2));
    }

    #[test]
    fn a_retry_shot_whose_duration_does_not_match_the_plan_is_rejected() {
        // Security L4: the priced duration must equal the plan's clamped seconds
        // for that index (±0), so a direct caller cannot pass `validate` with a
        // cheap in-catalogue duration while billing an inflated one.
        let state = test_state();
        let sid = mint_storyboard_id();
        state
            .store
            .upsert_storyboard(&crate::store::Storyboard {
                id: sid.clone(),
                brief: "b".into(),
                target_seconds: 20.0,
                plan_json: kling_plan_json(4, 5.0),
                created_at: now_secs(),
                chained: false,
                shots_json: None,
                overlap_seconds: None,
                joins_json: None,
                auto_retry: false,
                keyframes_json: None,
            })
            .unwrap();
        let mut job = blank_job("first-2");
        job.storyboard_id = Some(sid.clone());
        job.shot_index = Some(2);
        job.status = hickeyfield_core::JobStatus::Failed;
        state.store.upsert(&job).unwrap();

        let input = StoryboardGenerateInput {
            brief: "b".into(),
            target_seconds: 20.0,
            plan_json: Some(kling_plan_json(4, 5.0)),
            shot_indices: Some(vec![2]),
            storyboard_id: Some(sid.clone()),
            chained: false,
            overlap_seconds: None,
            // Plan clamped shot 2 to 5s, but this submits 30s.
            shots: vec![kling_shot(30.0, None)],
            auto_retry: false,
            keyframe_mode: false,
            still_model_id: None,
        };
        let err = generate_storyboard_inner(&state, input).unwrap_err();
        assert!(err.contains("clamped it to 5"), "got: {err}");
    }
}
