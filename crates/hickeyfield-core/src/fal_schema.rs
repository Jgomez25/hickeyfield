//! What a fal endpoint actually accepts, from fal.
//!
//! Three bugs in one day came from the same mistake: trusting the vendored
//! catalogue about a *different provider's* API. The fourth, found six weeks
//! later, came from reading fal's own document too narrowly.
//!
//! 1. Parameter **names** differ — the catalogue says `image`, fal wants
//!    `image_url`. Patched with a dialect table.
//! 2. Input **modes** differ — the catalogue has one model, fal splits it into
//!    `/text-to-video` and `/image-to-video`. Patched with a mode table.
//! 3. Accepted **inputs** differ, and this one cost real money: the catalogue
//!    says Gemini Omni takes `image_references` and `video_references`; fal's
//!    endpoint takes only `prompt`, `duration` and `aspect_ratio`. A user
//!    attached a clip, asked for it to be edited, and got an unrelated
//!    text-to-video generation — because fal ignored the field it did not
//!    recognise and billed for the result.
//! 4. A field the endpoint **pins**, which is none of the above: `const` is
//!    not a wrong name, not a wrong mode and not an unaccepted key, so every
//!    guard above waves it through. fal's Veo 3.1 Extend renders at `"7s"` and
//!    `"720p"` and will take nothing else; LTX 2.5 writes its lengths as the
//!    integers `{6, 8, 10}` beside the string `"auto"`. The enum reader kept
//!    only strings and never looked at `const`, so both read as "no
//!    constraint" (or, worse, as `["auto"]` alone) and both 422 on submit.
//! 5. A field the endpoint **requires but the app has no setting for**: fal
//!    lists a *default* yet still 422s when the key is absent. The MiniMax H3
//!    Max family requires `prompt_expansion_mode`; it is neither the prompt nor
//!    a media key, so nothing in the submit path ever added it and every
//!    request was refused. [`apply_constants`] writes such a field from a table
//!    keyed by slug ([`crate::media::fal_constants`]), respecting a value the
//!    user already set and the endpoint's own schema.
//!
//! The catalogue describes Higgsfield's API. It is not wrong; it is about
//! something else. fal publishes its own per-endpoint schema, unauthenticated,
//! so the durable fix is to stop guessing and read it.
//!
//! **Silently dropping a user's input is the failure mode to design against.**
//! A rejected request costs a round trip; a request that quietly ignores half
//! of what was asked costs money and trust.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

/// The input contract for one endpoint.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct EndpointSchema {
    /// Every field the endpoint declares.
    pub accepted: BTreeSet<String>,
    pub required: BTreeSet<String>,
    /// Enumerated values, where the endpoint publishes them.
    ///
    /// Needed because the catalogue and fal spell the *same* value
    /// differently: we offer `1k`, fal wants `1K`; we offer `4`, Veo wants
    /// `4s`. Both are 422s that look like our bug to the user.
    ///
    /// Members are recorded as text whatever JSON type fal declares them with,
    /// because that is the only spelling both sides can compare: LTX 2.5's
    /// `duration` is the mixed list `{6, 8, 10, "auto"}` and reading only its
    /// string member left the app believing `auto` was the sole legal length.
    pub enums: BTreeMap<String, Vec<String>>,
    /// Fields the endpoint pins to a single value with `const`.
    ///
    /// Also present in `enums` as a one-member list, because that is what
    /// every consumer already reads; this set is what distinguishes "fal
    /// offers exactly one value" from "fal pins this and will 422 anything
    /// else", which the submit path and the audit each say differently.
    ///
    /// Defaulted on deserialize so an older serialised snapshot still loads.
    #[serde(default)]
    pub pinned: BTreeSet<String>,
    /// The `[min, max]` bound of a numeric field, where the endpoint declares
    /// one.
    ///
    /// Recorded because a range is NOT an enum: turning `minimum:5 /
    /// maximum:15` into `[5,6,…,15]` would invent members fal never published
    /// (the LTX-class bug the enum reader already guards). fal states a
    /// duration bound two ways — directly (`{type:"integer", minimum, maximum}`)
    /// and, for a nullable field, inside `anyOf[{integer, minimum, maximum},
    /// {null}]` (Wan 3.0 Prime) — and both must be read or the range the UI
    /// shows is wrong for half the roster.
    ///
    /// Defaulted on deserialize so an older serialised snapshot still loads.
    #[serde(default)]
    pub ranges: BTreeMap<String, (f64, f64)>,
}

impl EndpointSchema {
    pub fn accepts(&self, key: &str) -> bool {
        self.accepted.contains(key)
    }

    /// Any field that looks like it carries media.
    ///
    /// Matched on the name because fal is consistent about it — `image_url`,
    /// `image_urls`, `video_url`, `audio_url`, `*_frame_url` — and because the
    /// OpenAPI type is just `string` either way.
    pub fn media_keys(&self) -> Vec<&str> {
        self.accepted
            .iter()
            .map(String::as_str)
            .filter(|k| {
                k.ends_with("_url")
                    || k.ends_with("_urls")
                    || k.ends_with("_image")
                    || k.ends_with("_video")
            })
            .collect()
    }

    /// Whether this endpoint can take attached media at all.
    pub fn takes_media(&self) -> bool {
        !self.media_keys().is_empty()
    }

    /// Rewrite a value into the endpoint's own spelling, when it is obviously
    /// the same value written differently.
    ///
    /// Only unambiguous rewrites: case (`1k` -> `1K`) and a unit suffix
    /// (`4` -> `4s`). Anything else returns `None`, because a value the
    /// endpoint genuinely does not offer must surface as a refusal rather than
    /// be silently swapped for a neighbour the user did not pick — that would
    /// be the same silent-substitution failure in a new place.
    pub fn coerce(&self, field: &str, value: &str) -> Option<String> {
        let Some(allowed) = self.enums.get(field) else {
            return Some(value.to_string());
        };
        if allowed.iter().any(|a| a == value) {
            return Some(value.to_string());
        }
        if let Some(hit) = allowed.iter().find(|a| a.eq_ignore_ascii_case(value)) {
            return Some(hit.clone());
        }
        // `4` against `4s`, and the reverse.
        let bare = value.trim_end_matches(|c: char| c.is_ascii_alphabetic());
        if let Some(hit) = allowed.iter().find(|a| {
            a.trim_end_matches(|c: char| c.is_ascii_alphabetic())
                .eq_ignore_ascii_case(bare)
                && !bare.is_empty()
        }) {
            return Some(hit.clone());
        }
        None
    }
}

/// A value the endpoint will not accept, named in words a user can act on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    pub field: String,
    pub offered: Vec<String>,
    pub sent: String,
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "this endpoint takes {} of {}, not {}",
            self.field.replace('_', " "),
            self.offered.join(", "),
            self.sent
        )
    }
}

/// A field the endpoint pins with `const`, rewritten to the value it pins.
///
/// Reported rather than swallowed: the render the user gets is not the one
/// their settings asked for, and a job that does not say so is the silent kind
/// of wrong this module exists to stop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pinned {
    pub field: String,
    pub from: String,
    pub to: String,
}

/// Rewrite a request body into the spelling *and the JSON type* the endpoint
/// publishes, before anything is sent.
///
/// The type half is not pedantry. fal's Seedance endpoint declares `duration`
/// as an enumeration of **strings** — `'auto' | '4' | … | '15'` — and we were
/// sending the number `4.0` straight out of the settings object, because the
/// binder inserts each setting verbatim. The result is a 422 that names our own
/// field back at us and looks, to the user, exactly like the app being broken:
///
///   `Input should be 'auto', '4', … or '15'` … `"input": 4.0`
///
/// Only fields the endpoint actually enumerates are touched, which keeps this
/// from becoming a general-purpose coercion layer that guesses. A value with no
/// published enum goes over the wire exactly as built.
///
/// A value that is genuinely not on offer is returned as a [`Refusal`] rather
/// than quietly swapped for a neighbour. Substituting 6s for a requested 8s
/// would produce a generation the user did not ask for and charge them for it —
/// the same silent-substitution failure this module was written to stop.
///
/// A field the endpoint **pins** (`const`) is the one exception, and it is not
/// a substitution: there is no neighbour to pick instead. Veo 3.1 Extend renders
/// at 7 seconds and nothing else, so refusing a stale `duration: 5` would cost
/// the user a generation fal would happily have run. The pinned value is written
/// and returned in the [`Pinned`] list so the caller can say so on the job.
/// `prompt` and media keys are never rewritten this way — overwriting what a
/// person typed or uploaded with a schema constant would be this module's
/// founding failure committed in a new place — and are refused instead.
pub fn reconcile(
    schema: &EndpointSchema,
    body: &mut serde_json::Map<String, serde_json::Value>,
) -> Result<Vec<Pinned>, Refusal> {
    // Pins first, then the enum pass below retypes what was written: the JSON
    // type is half the contract and both halves must hold.
    let mut rewritten: Vec<Pinned> = Vec::new();
    for field in &schema.pinned {
        let Some(pin) = schema.enums.get(field).and_then(|v| v.first()).cloned() else {
            continue;
        };
        // A key the body does not carry stays fal's business.
        let Some(current) = body.get(field) else {
            continue;
        };
        let Some(as_text) = scalar_text(current) else {
            continue;
        };
        // Already the pinned value, or the same value spelled differently
        // (`7` against `7s`) — nothing was overridden, so nothing to report.
        if schema.coerce(field, &as_text).as_deref() == Some(pin.as_str()) {
            continue;
        }
        if field == "prompt" || schema.media_keys().contains(&field.as_str()) {
            return Err(Refusal {
                field: field.clone(),
                offered: vec![pin],
                sent: as_text,
            });
        }
        body.insert(field.clone(), serde_json::Value::String(pin.clone()));
        rewritten.push(Pinned {
            field: field.clone(),
            from: as_text,
            to: pin,
        });
    }

    for (field, allowed) in &schema.enums {
        let Some(current) = body.get(field) else {
            continue;
        };
        // Only scalars have a spelling. An array or object under an enumerated
        // key means our model of this endpoint is wrong in a way a string
        // rewrite would paper over.
        let Some(as_text) = scalar_text(current) else {
            continue;
        };
        match schema.coerce(field, &as_text) {
            // Always write the enum's own member back, so `4` becomes the
            // string `"4"` even when the text already matched: the type is
            // half the contract.
            Some(fixed) => {
                body.insert(field.clone(), serde_json::Value::String(fixed));
            }
            None => {
                return Err(Refusal {
                    field: field.clone(),
                    offered: allowed.clone(),
                    sent: as_text,
                })
            }
        }
    }
    Ok(rewritten)
}

/// Write each `(key, value)` constant into `body` for a field the endpoint
/// fixes, before [`reconcile`].
///
/// A constant is NOT a user setting: it exists only because fal requires the
/// key while giving the app nothing to surface (see
/// [`crate::media::fal_constants`]). The rules are the whole point of a
/// *mechanism* rather than a hardcode:
///
/// - **Never override a field already present.** A value the user set, or that
///   the binder wrote, always wins; a constant only fills a gap. Today no H3 Max
///   constant collides with a real control, but the next required-with-default
///   field a provider adds might.
/// - **With a schema, write only a key the endpoint `accepts`.** A constant an
///   endpoint does not declare would be swept away by the unknown-key pass
///   anyway, and sending it to a strict endpoint 422s.
/// - **With NO schema (offline / uncached), write it regardless.** fal requires
///   the key, so omitting it is a guaranteed 422; sending a field that every
///   endpoint we ship this for is known to take is the lesser risk. Correct for
///   the four H3 Max endpoints, which all declare `prompt_expansion_mode`.
pub fn apply_constants(
    constants: &[(&str, &str)],
    schema: Option<&EndpointSchema>,
    body: &mut serde_json::Map<String, serde_json::Value>,
) {
    for (key, value) in constants {
        if body.contains_key(*key) {
            continue;
        }
        let accepted = schema.is_none_or(|s| s.accepts(key));
        if accepted {
            body.insert(
                (*key).to_string(),
                serde_json::Value::String((*value).to_string()),
            );
        }
    }
}

/// Write caller-supplied overrides (S15 D-C) into `body`, for keys the endpoint
/// declares.
///
/// Like [`apply_constants`] but the values are typed JSON — a `seed` is a
/// number, `prompt_expansion_mode` a string — and the intent is to *override* a
/// constant, so this runs **before** [`apply_constants`]: a key written here
/// makes the constant pass skip it (`"balanced"` → `"disabled"` on a chained
/// shot). It is still gap-only against the user/binder (never clobbers a key
/// already in the body) and still schema-gated (with a cached schema it writes
/// only a key the endpoint `accepts`, so the chain cannot inject an arbitrary
/// field). Offline it writes regardless, the same lesser-risk rule constants use.
pub fn apply_overrides(
    overrides: &serde_json::Map<String, serde_json::Value>,
    schema: Option<&EndpointSchema>,
    body: &mut serde_json::Map<String, serde_json::Value>,
) {
    for (key, value) in overrides {
        if body.contains_key(key) {
            continue;
        }
        if schema.is_none_or(|s| s.accepts(key)) {
            body.insert(key.clone(), value.clone());
        }
    }
}

/// A scalar as the text an enum would spell it with.
///
/// `4.0` is the number a JSON settings object carries for a whole-second
/// duration, and `"4.0"` matches no enum anywhere — so a float that is exactly
/// an integer is written as one.
fn scalar_text(v: &serde_json::Value) -> Option<String> {
    match v {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Bool(b) => Some(b.to_string()),
        serde_json::Value::Number(n) => Some(match n.as_f64() {
            Some(f) if f.fract() == 0.0 && f.is_finite() => format!("{}", f as i64),
            _ => n.to_string(),
        }),
        _ => None,
    }
}

fn cache() -> &'static Mutex<BTreeMap<String, Option<EndpointSchema>>> {
    static CACHE: OnceLock<Mutex<BTreeMap<String, Option<EndpointSchema>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(BTreeMap::new()))
}

/// Parse fal's OpenAPI document into the input contract.
///
/// Separated from the fetch so the parser is testable against a captured
/// document, which is the only part likely to break when fal restructures.
pub fn parse(doc: &serde_json::Value) -> Option<EndpointSchema> {
    let schemas = doc.get("components")?.get("schemas")?.as_object()?;
    // fal names it `<Model>Input`. Take the first such schema: an endpoint
    // document describes exactly one input.
    let (_, input) = schemas.iter().find(|(name, _)| name.ends_with("Input"))?;

    let accepted = input
        .get("properties")?
        .as_object()?
        .keys()
        .cloned()
        .collect();
    let mut enums = BTreeMap::new();
    let mut pinned = BTreeSet::new();
    let mut ranges = BTreeMap::new();
    if let Some(props) = input.get("properties").and_then(|p| p.as_object()) {
        for (name, prop) in props {
            // A numeric bound, read alongside the enum/const read (a `const`-
            // pinned or fully-enumerated field carries no range; a nullable
            // range field is not an enum, so the two never collide). This is
            // the ONE place `anyOf`/`oneOf` is descended — for a range, never
            // for an enum.
            if let Some((min, max)) = numeric_range(prop) {
                ranges.insert(name.clone(), (min, max));
            }
            // `const` first, and it beats a sibling `enum`: fal generates these
            // from a pydantic `Literal[...]`, so the enum in that case is the
            // wider declared type rather than a second offer. A `const` that is
            // an object, an array or `null` is ignored outright — we do not
            // model those, and pretending to would be worse than silence.
            if let Some(text) = prop.get("const").and_then(scalar_text) {
                enums.insert(name.clone(), vec![text]);
                pinned.insert(name.clone());
                continue;
            }
            if let Some(vals) = prop.get("enum").and_then(|e| e.as_array()) {
                // All-or-nothing. A partially recorded enum is exactly the LTX
                // 2.5 failure: it looks authoritative and then refuses every
                // value the endpoint actually takes. A `null` member is skipped
                // without voiding the entry — in a fal enum it means "may be
                // omitted", not a value the app can send.
                let mut vs: Vec<String> = Vec::new();
                let mut readable = true;
                for v in vals {
                    if v.is_null() {
                        continue;
                    }
                    match scalar_text(v) {
                        Some(t) => vs.push(t),
                        None => {
                            readable = false;
                            break;
                        }
                    }
                }
                if readable && !vs.is_empty() {
                    enums.insert(name.clone(), vs);
                }
            }
            // `anyOf` / `oneOf` / `allOf` are deliberately not descended into.
            // Wan 3.0 Prime declares `duration` as
            // `anyOf[{integer 2..30}, {null}]`, which is a range and not an
            // offer: enumerating it would invent members fal never published.
        }
    }

    let required = input
        .get("required")
        .and_then(|r| r.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    Some(EndpointSchema {
        accepted,
        required,
        enums,
        pinned,
        ranges,
    })
}

/// The `[min, max]` numeric bound of a property, read directly or from the
/// first `anyOf`/`oneOf` member that carries both `minimum` and `maximum`.
///
/// Records `(min, max)` only when both are present and `min <= max`; a field
/// with neither returns `None`. A `const` field has no range (a pin, not a
/// bound); a fully enumerated field has no range (the offers are the bound).
fn numeric_range(prop: &serde_json::Value) -> Option<(f64, f64)> {
    if prop.get("const").is_some() || prop.get("enum").is_some() {
        return None;
    }
    if let Some(r) = range_of(prop) {
        return Some(r);
    }
    for key in ["anyOf", "oneOf"] {
        if let Some(members) = prop.get(key).and_then(|m| m.as_array()) {
            for member in members {
                if let Some(r) = range_of(member) {
                    return Some(r);
                }
            }
        }
    }
    None
}

/// `(minimum, maximum)` of a schema object when both are present numbers and
/// ordered; otherwise `None`.
fn range_of(obj: &serde_json::Value) -> Option<(f64, f64)> {
    let min = obj.get("minimum").and_then(|v| v.as_f64())?;
    let max = obj.get("maximum").and_then(|v| v.as_f64())?;
    (min <= max).then_some((min, max))
}

/// Fetch and cache the schema for one endpoint.
///
/// `None` means we could not find out — no network, or fal restructured the
/// document. Callers must treat that as *unknown*, never as *nothing accepted*:
/// refusing a valid request because we are offline would be a worse failure
/// than the one this module exists to prevent.
///
/// Negative results are cached too, so a model that genuinely has no published
/// schema does not cost a request per generation.
pub fn for_endpoint(endpoint: &str) -> Option<EndpointSchema> {
    if let Some(hit) = cache().lock().ok()?.get(endpoint) {
        return hit.clone();
    }

    let fetched = fetch(endpoint);
    if let Ok(mut c) = cache().lock() {
        c.insert(endpoint.to_string(), fetched.clone());
    }
    fetched
}

fn fetch(endpoint: &str) -> Option<EndpointSchema> {
    // Short: this runs on the submit path, and a slow answer must not delay a
    // generation. Missing the schema costs correctness we already lacked.
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(8))
        .user_agent(concat!("hickeyfield/", env!("CARGO_PKG_VERSION")))
        .build()
        .ok()?;

    let resp = client
        .get(format!(
            "https://fal.ai/api/openapi/queue/openapi.json?endpoint_id={endpoint}"
        ))
        .send()
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    parse(&resp.json::<serde_json::Value>().ok()?)
}

/// Seed the cache, for tests and for a bundled snapshot.
pub fn preload(endpoint: &str, schema: Option<EndpointSchema>) {
    if let Ok(mut c) = cache().lock() {
        c.insert(endpoint.to_string(), schema);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Captured from
    /// `fal.ai/api/openapi/queue/openapi.json?endpoint_id=google/gemini-omni-flash`
    /// on 2026-08-05 — the endpoint that silently ignored a user's video.
    const GEMINI_OMNI: &str = r#"{
      "components": { "schemas": {
        "GeminiOmniFlashOutput": { "properties": { "video": {"type": "object"} } },
        "GeminiOmniFlashInput": {
          "properties": {
            "duration": {"type": "integer"},
            "prompt": {"type": "string"},
            "aspect_ratio": {"type": "string"}
          },
          "required": ["prompt"]
        }
      } }
    }"#;

    const KLING_I2V: &str = r#"{
      "components": { "schemas": {
        "KlingInput": {
          "properties": {
            "prompt": {"type": "string"},
            "image_url": {"type": "string"},
            "tail_image_url": {"type": "string"},
            "duration": {"type": "string"}
          },
          "required": ["prompt", "image_url"]
        }
      } }
    }"#;

    #[test]
    fn published_enums_are_captured() {
        let doc = r#"{"components":{"schemas":{"XInput":{"properties":{
            "resolution":{"type":"string","enum":["1K","2K"]},
            "prompt":{"type":"string"}}}}}}"#;
        let s = parsed(doc);
        assert_eq!(s.enums.get("resolution").unwrap(), &["1K", "2K"]);
        assert!(!s.enums.contains_key("prompt"));
    }

    fn parsed(src: &str) -> EndpointSchema {
        parse(&serde_json::from_str(src).unwrap()).unwrap()
    }

    #[test]
    fn a_text_only_endpoint_reports_that_it_takes_no_media() {
        // The bug this exists to prevent: our catalogue says Gemini Omni takes
        // image_references and video_references. fal's endpoint takes neither,
        // so a user's attached clip was dropped on the floor and billed for.
        let s = parsed(GEMINI_OMNI);
        assert!(!s.takes_media(), "media keys: {:?}", s.media_keys());
        assert!(s.accepts("prompt"));
        assert!(!s.accepts("video_url"));
        assert!(!s.accepts("image_references"));
    }

    #[test]
    fn an_image_to_video_endpoint_reports_its_media_keys() {
        let s = parsed(KLING_I2V);
        assert!(s.takes_media());
        let mut keys = s.media_keys();
        keys.sort_unstable();
        assert_eq!(keys, ["image_url", "tail_image_url"]);
    }

    #[test]
    fn required_fields_are_captured() {
        assert_eq!(
            parsed(KLING_I2V)
                .required
                .iter()
                .cloned()
                .collect::<Vec<_>>(),
            ["image_url", "prompt"]
        );
    }

    #[test]
    fn the_output_schema_is_not_mistaken_for_the_input() {
        // Both live in the same document and only the suffix distinguishes
        // them; reading the output would report a video field on a text model.
        let s = parsed(GEMINI_OMNI);
        assert!(!s.accepts("video"));
    }

    #[test]
    fn a_document_we_cannot_understand_is_unknown_rather_than_empty() {
        // "Unknown" must never collapse into "accepts nothing", or being
        // offline would block every generation.
        assert!(parse(&serde_json::json!({})).is_none());
        assert!(parse(&serde_json::json!({"components": {}})).is_none());
    }

    #[test]
    fn case_only_differences_are_rewritten_not_rejected() {
        // Measured: we offer `1k`, fal's Nano Banana wants `1K`. Identical
        // value, different spelling, guaranteed 422 that reads as our bug.
        let s = EndpointSchema {
            accepted: ["resolution".to_string()].into_iter().collect(),
            required: Default::default(),
            enums: [(
                "resolution".to_string(),
                vec!["1K".into(), "2K".into(), "4K".into()],
            )]
            .into_iter()
            .collect(),
            pinned: Default::default(),
            ranges: Default::default(),
        };
        assert_eq!(s.coerce("resolution", "1k").as_deref(), Some("1K"));
        assert_eq!(s.coerce("resolution", "4K").as_deref(), Some("4K"));
    }

    #[test]
    fn a_unit_suffix_is_reconciled_in_both_directions() {
        // Measured: we offer duration `4`, Veo 3.1 wants `4s`.
        let s = EndpointSchema {
            accepted: ["duration".to_string()].into_iter().collect(),
            required: Default::default(),
            enums: [(
                "duration".to_string(),
                vec!["4s".into(), "6s".into(), "8s".into()],
            )]
            .into_iter()
            .collect(),
            pinned: Default::default(),
            ranges: Default::default(),
        };
        assert_eq!(s.coerce("duration", "4").as_deref(), Some("4s"));
        assert_eq!(s.coerce("duration", "8s").as_deref(), Some("8s"));
    }

    #[test]
    fn a_value_the_endpoint_does_not_offer_is_refused_not_substituted() {
        // Seedance Fast tops out at 720p while our catalogue offers 4k.
        // Quietly downgrading to 720p would charge for something the user did
        // not pick — the same silent-substitution failure in a new place.
        let s = EndpointSchema {
            accepted: ["resolution".to_string()].into_iter().collect(),
            required: Default::default(),
            enums: [("resolution".to_string(), vec!["480p".into(), "720p".into()])]
                .into_iter()
                .collect(),
            pinned: Default::default(),
            ranges: Default::default(),
        };
        assert_eq!(s.coerce("resolution", "4k"), None);
        assert_eq!(s.coerce("resolution", "1080p"), None);
    }

    #[test]
    fn a_field_with_no_published_enum_passes_through_untouched() {
        let s = EndpointSchema {
            accepted: ["prompt".to_string()].into_iter().collect(),
            required: Default::default(),
            enums: Default::default(),
            pinned: Default::default(),
            ranges: Default::default(),
        };
        assert_eq!(s.coerce("prompt", "anything").as_deref(), Some("anything"));
    }

    #[test]
    fn the_cache_returns_what_was_preloaded() {
        let s = parsed(KLING_I2V);
        preload("test/only", Some(s.clone()));
        assert_eq!(for_endpoint("test/only"), Some(s));
    }

    #[test]
    fn a_cached_negative_is_still_a_cache_hit() {
        // Otherwise a model with no published schema costs a network round trip
        // on every single generation.
        preload("test/negative", None);
        assert_eq!(for_endpoint("test/negative"), None);
    }

    /// The H3 Max text-to-video input surface, as fal published it 2026-09-16.
    const H3_MAX_T2V: &str = r#"{"components":{"schemas":{"H3MaxTextToVideoInput":{"properties":{
        "prompt":{"type":"string"},
        "prompt_expansion_mode":{"type":"string","default":"balanced","examples":["disabled","balanced","quality"]},
        "resolution":{"type":"string","enum":["480P","768P","1080P"],"default":"768P"},
        "aspect_ratio":{"type":"string","enum":["21:9","16:9","4:3","1:1","3:4","9:16"],"default":"16:9"},
        "duration":{"type":"integer","minimum":5,"maximum":15,"default":5}},
        "required":["prompt","prompt_expansion_mode"]}}}}"#;

    #[test]
    fn a_constant_is_written_when_the_schema_accepts_it() {
        let schema = parsed(H3_MAX_T2V);
        let mut body = serde_json::Map::new();
        body.insert("prompt".into(), serde_json::json!("a red door"));
        apply_constants(
            &[("prompt_expansion_mode", "balanced")],
            Some(&schema),
            &mut body,
        );
        assert_eq!(
            body.get("prompt_expansion_mode"),
            Some(&serde_json::json!("balanced"))
        );
    }

    #[test]
    fn a_constant_never_overrides_a_user_value() {
        // A value already in the body — set by the user or the binder — always
        // wins; a constant only fills a gap.
        let schema = parsed(H3_MAX_T2V);
        let mut body = serde_json::Map::new();
        body.insert("prompt_expansion_mode".into(), serde_json::json!("quality"));
        apply_constants(
            &[("prompt_expansion_mode", "balanced")],
            Some(&schema),
            &mut body,
        );
        assert_eq!(
            body.get("prompt_expansion_mode"),
            Some(&serde_json::json!("quality"))
        );
    }

    #[test]
    fn a_constant_is_dropped_when_the_schema_rejects_the_key() {
        // An endpoint that does not declare the key would sweep it away anyway,
        // and a strict endpoint 422s on it.
        let schema = parsed(GEMINI_OMNI);
        assert!(!schema.accepts("prompt_expansion_mode"));
        let mut body = serde_json::Map::new();
        apply_constants(
            &[("prompt_expansion_mode", "balanced")],
            Some(&schema),
            &mut body,
        );
        assert!(!body.contains_key("prompt_expansion_mode"));
    }

    #[test]
    fn a_chained_override_disables_expansion_and_carries_the_seed() {
        // S15 D-C: a chained shot writes `prompt_expansion_mode: "disabled"`
        // (overriding S14's "balanced" constant, because `apply_overrides` runs
        // first and `apply_constants` then skips the present key) and a typed
        // `seed`. Both keys are declared by the H3 Max schema, so both land;
        // `audit_fal` stays clean because neither is invented.
        // A schema that declares both keys, as the real H3 Max i2v endpoint
        // does (`schemas/minimax_h3-max_image-to-video.json` carries `seed`).
        let with_seed = r#"{"components":{"schemas":{"H3MaxInput":{"properties":{
            "prompt":{"type":"string"},
            "prompt_expansion_mode":{"type":"string","default":"balanced","examples":["disabled","balanced","quality"]},
            "seed":{"type":"integer"},
            "duration":{"type":"integer","minimum":5,"maximum":15,"default":5}},
            "required":["prompt","prompt_expansion_mode"]}}}}"#;
        let schema = parsed(with_seed);
        let mut overrides = serde_json::Map::new();
        overrides.insert(
            "prompt_expansion_mode".into(),
            serde_json::json!("disabled"),
        );
        overrides.insert("seed".into(), serde_json::json!(4242));
        let mut body = serde_json::Map::new();
        body.insert("prompt".into(), serde_json::json!("a red door"));
        apply_overrides(&overrides, Some(&schema), &mut body);
        // The constant pass now cannot revert the mode.
        apply_constants(
            &[("prompt_expansion_mode", "balanced")],
            Some(&schema),
            &mut body,
        );
        assert_eq!(
            body.get("prompt_expansion_mode"),
            Some(&serde_json::json!("disabled")),
            "chained shot disables fal's second rewrite"
        );
        assert_eq!(body.get("seed"), Some(&serde_json::json!(4242)));
    }

    #[test]
    fn a_standalone_submit_keeps_balanced_and_has_no_seed() {
        // The mirror: with no overrides the constant stands and no seed appears.
        let schema = parsed(H3_MAX_T2V);
        let mut body = serde_json::Map::new();
        body.insert("prompt".into(), serde_json::json!("a red door"));
        apply_overrides(&serde_json::Map::new(), Some(&schema), &mut body);
        apply_constants(
            &[("prompt_expansion_mode", "balanced")],
            Some(&schema),
            &mut body,
        );
        assert_eq!(
            body.get("prompt_expansion_mode"),
            Some(&serde_json::json!("balanced"))
        );
        assert!(!body.contains_key("seed"), "standalone has no seed");
    }

    #[test]
    fn an_override_cannot_inject_a_field_the_endpoint_rejects() {
        // The chain cannot smuggle an arbitrary key: a schema-gated override for
        // a field the endpoint does not declare is dropped.
        let schema = parsed(GEMINI_OMNI);
        assert!(!schema.accepts("prompt_expansion_mode"));
        let mut overrides = serde_json::Map::new();
        overrides.insert(
            "prompt_expansion_mode".into(),
            serde_json::json!("disabled"),
        );
        let mut body = serde_json::Map::new();
        apply_overrides(&overrides, Some(&schema), &mut body);
        assert!(!body.contains_key("prompt_expansion_mode"));
    }

    /// The Qwen Image 3 text-to-image input surface (the S17 default keyframe
    /// still model), as fal published it 2026-09-16
    /// (`.forge/slices/S17-keyframe-anchored-mode/schemas/qwen_image-3_text-to-image.json`):
    /// `image_size` is an `anyOf` of the `{width,height}` object and a
    /// named-preset enum, defaulting to `square_hd`.
    const QWEN_IMAGE_3_T2I: &str = r##"{"components":{"schemas":{"QwenImage3TextToImageInput":{"properties":{
        "prompt":{"type":"string"},
        "image_size":{"anyOf":[{"$ref":"#/components/schemas/ImageSize"},{"enum":["square_hd","square","portrait_4_3","portrait_16_9","landscape_4_3","landscape_16_9"],"type":"string"}],"default":"square_hd"},
        "seed":{"type":"integer"}},
        "required":["prompt"]}}}}"##;

    #[test]
    fn the_keyframe_still_image_size_object_lands_in_the_qwen_body() {
        // S17 §10.3 (the design's flagged weak point — "the one thing this
        // design could not run"): the derived still canvas is written as a
        // free-form `image_size: {width, height}` object through the override
        // seam. Qwen Image 3 declares `image_size` (an anyOf of that object plus
        // named presets), so the schema gate keeps it and the object reaches the
        // wire body verbatim — no rescale/crop of the anchor. Keyless.
        let schema = parsed(QWEN_IMAGE_3_T2I);
        assert!(schema.accepts("image_size"), "qwen declares image_size");
        let mut overrides = serde_json::Map::new();
        overrides.insert(
            "image_size".into(),
            serde_json::json!({ "width": 1366, "height": 768 }),
        );
        overrides.insert("seed".into(), serde_json::json!(7u32));
        let mut body = serde_json::Map::new();
        body.insert("prompt".into(), serde_json::json!("a can, label to camera"));
        apply_overrides(&overrides, Some(&schema), &mut body);
        assert_eq!(
            body.get("image_size"),
            Some(&serde_json::json!({ "width": 1366, "height": 768 })),
            "the {{width,height}} still canvas lands verbatim in the body"
        );
        assert_eq!(body.get("seed"), Some(&serde_json::json!(7u32)));
    }

    #[test]
    fn a_still_image_size_is_dropped_when_the_endpoint_does_not_declare_it() {
        // Defense (the S9 harm): the same canvas override on an endpoint that
        // does NOT declare `image_size` (e.g. the H3 Max video surface) is swept
        // away, so a still canvas can never 422 a model that has no such key.
        let schema = parsed(H3_MAX_T2V);
        assert!(!schema.accepts("image_size"));
        let mut overrides = serde_json::Map::new();
        overrides.insert(
            "image_size".into(),
            serde_json::json!({ "width": 1366, "height": 768 }),
        );
        let mut body = serde_json::Map::new();
        apply_overrides(&overrides, Some(&schema), &mut body);
        assert!(
            !body.contains_key("image_size"),
            "an endpoint without image_size drops the override"
        );
    }

    #[test]
    fn a_constant_is_sent_when_no_schema_is_cached() {
        // Offline: fal requires the key, so omitting it is a guaranteed 422.
        let mut body = serde_json::Map::new();
        apply_constants(&[("prompt_expansion_mode", "balanced")], None, &mut body);
        assert_eq!(
            body.get("prompt_expansion_mode"),
            Some(&serde_json::json!("balanced"))
        );
    }

    #[test]
    fn resolution_case_matches_the_uppercase_members() {
        // AC4: the app must send fal's exact UPPERCASE spelling. `coerce` is
        // case-insensitive, and `reconcile` writes the enum's own member back.
        let schema = parsed(H3_MAX_T2V);
        assert_eq!(
            schema.coerce("resolution", "1080p").as_deref(),
            Some("1080P")
        );
        assert_eq!(schema.coerce("resolution", "768p").as_deref(), Some("768P"));
        // A body already carrying "768P" reconciles to "768P" unchanged.
        let mut body = serde_json::Map::new();
        body.insert("resolution".into(), serde_json::json!("768P"));
        reconcile(&schema, &mut body).unwrap();
        assert_eq!(body.get("resolution"), Some(&serde_json::json!("768P")));
    }

    /// Build a schema that enumerates one field, the way fal publishes it.
    fn enumerated(field: &str, values: &[&str]) -> EndpointSchema {
        EndpointSchema {
            accepted: [field.to_string(), "prompt".to_string()]
                .into_iter()
                .collect(),
            required: Default::default(),
            enums: [(
                field.to_string(),
                values.iter().map(|s| s.to_string()).collect(),
            )]
            .into_iter()
            .collect(),
            pinned: Default::default(),
            ranges: Default::default(),
        }
    }

    /// Build a schema that pins one field to one value.
    ///
    /// Deliberately built by running [`parse`] over the document shape fal
    /// publishes rather than by filling the struct in by hand: a hand-built
    /// `pinned` set would keep these tests green even if the parser stopped
    /// recording the pin, which is the one thing they exist to catch.
    fn pinned_to(field: &str, value: &str) -> EndpointSchema {
        let mut props = serde_json::json!({
            "prompt": {"type": "string"},
            "image_url": {"type": "string"},
        });
        props[field] = serde_json::json!({"const": value, "type": "string"});
        let doc = serde_json::json!({
            "components": {"schemas": {"XInput": {"properties": props}}}
        });
        parse(&doc).expect("a document with one Input schema")
    }

    #[test]
    fn a_numeric_duration_becomes_the_string_the_endpoint_enumerates() {
        // The live 422, exactly: Seedance 2.0 Mini declares duration as a
        // string enum, the settings object carries the number 4.0, and fal
        // answers `Input should be 'auto', '4', … or '15'` with `"input": 4.0`.
        let schema = enumerated("duration", &["auto", "4", "5", "6", "8", "10"]);
        let mut body = serde_json::Map::new();
        body.insert("duration".into(), serde_json::json!(4.0));

        reconcile(&schema, &mut body).expect("4 seconds is on offer");
        assert_eq!(body["duration"], serde_json::json!("4"));
    }

    #[test]
    fn an_integer_duration_is_also_retyped() {
        let schema = enumerated("duration", &["4", "8"]);
        let mut body = serde_json::Map::new();
        body.insert("duration".into(), serde_json::json!(8));
        reconcile(&schema, &mut body).unwrap();
        assert_eq!(body["duration"], serde_json::json!("8"));
    }

    #[test]
    fn the_unit_suffix_is_still_reconciled_through_the_body() {
        // Veo spells the same duration `8s`. Coercion already knew this; what
        // was missing was anything calling it on the outgoing request.
        let schema = enumerated("duration", &["4s", "6s", "8s"]);
        let mut body = serde_json::Map::new();
        body.insert("duration".into(), serde_json::json!(8));
        reconcile(&schema, &mut body).unwrap();
        assert_eq!(body["duration"], serde_json::json!("8s"));
    }

    #[test]
    fn a_body_value_the_endpoint_does_not_offer_is_refused_not_substituted() {
        // Quietly rounding 12s down to the nearest offered value would bill the
        // user for a generation they did not ask for.
        let schema = enumerated("duration", &["4", "6", "8"]);
        let mut body = serde_json::Map::new();
        body.insert("duration".into(), serde_json::json!(12));

        let err = reconcile(&schema, &mut body).expect_err("12 is not on offer");
        assert_eq!(err.sent, "12");
        let msg = err.to_string();
        assert!(msg.contains("duration"), "{msg}");
        assert!(msg.contains('4') && msg.contains('8'), "{msg}");
    }

    #[test]
    fn a_field_with_no_published_enum_is_left_exactly_as_built() {
        // This must not become a general coercion layer: an unenumerated field
        // is one we know nothing about, and rewriting it would be a guess.
        let schema = enumerated("duration", &["4"]);
        let mut body = serde_json::Map::new();
        body.insert("duration".into(), serde_json::json!(4));
        body.insert("seed".into(), serde_json::json!(1234));
        body.insert("guidance".into(), serde_json::json!(6.5));

        reconcile(&schema, &mut body).unwrap();
        assert_eq!(body["seed"], serde_json::json!(1234));
        assert_eq!(body["guidance"], serde_json::json!(6.5));
    }

    #[test]
    fn an_absent_field_is_not_invented() {
        let schema = enumerated("duration", &["4"]);
        let mut body = serde_json::Map::new();
        body.insert("prompt".into(), serde_json::json!("a red door"));
        reconcile(&schema, &mut body).unwrap();
        assert!(!body.contains_key("duration"));
    }

    #[test]
    fn resolution_case_is_reconciled_the_same_way() {
        let schema = enumerated("resolution", &["720p", "1K", "4K"]);
        let mut body = serde_json::Map::new();
        body.insert("resolution".into(), serde_json::json!("1k"));
        reconcile(&schema, &mut body).unwrap();
        assert_eq!(body["resolution"], serde_json::json!("1K"));
    }

    // ---- `const` and non-string enum members -----------------------------
    //
    // Every fixture below is copied from the endpoint document fal served on
    // 2026-09-16, not composed here: the bug this closes was a parser that read
    // fal's own document more narrowly than fal wrote it.

    #[test]
    fn a_const_is_recorded_as_a_one_member_enum() {
        // `fal-ai/veo3.1/extend-video`, verbatim. Neither key is an `enum`, so
        // the old parser recorded nothing and the app sent 5.0 / "1080p".
        let doc = r#"{"components":{"schemas":{"Veo31ExtendVideoInput":{"properties":{
            "duration":{"const":"7s","type":"string","default":"7s"},
            "resolution":{"const":"720p","type":"string","default":"720p"},
            "prompt":{"type":"string"}}}}}}"#;
        let s = parsed(doc);
        assert_eq!(s.enums.get("duration").unwrap(), &["7s"]);
        assert_eq!(s.enums.get("resolution").unwrap(), &["720p"]);
        assert!(s.pinned.contains("duration"));
        assert!(s.pinned.contains("resolution"));
        assert!(!s.pinned.contains("prompt"));
    }

    #[test]
    fn an_integer_enum_is_stringified_not_dropped() {
        // `lightricks/ltx-2.5/text-to-video/pro`, verbatim: a mixed list with
        // no `type` key. The old parser kept `["auto"]` alone, so `reconcile`
        // refused 6, 8 and 10 — every length the UI could produce.
        let doc = r#"{"components":{"schemas":{"Ltx25Input":{"properties":{
            "duration":{"default":"auto","enum":[6,8,10,"auto"]}}}}}}"#;
        let s = parsed(doc);
        assert_eq!(s.enums.get("duration").unwrap(), &["6", "8", "10", "auto"]);
        assert!(s.pinned.is_empty());
    }

    #[test]
    fn a_pure_integer_enum_is_kept() {
        // `fps` on both LTX 2.5 endpoints.
        let doc = r#"{"components":{"schemas":{"XInput":{"properties":{
            "fps":{"type":"integer","default":25,"enum":[24,25,50]}}}}}}"#;
        assert_eq!(parsed(doc).enums.get("fps").unwrap(), &["24", "25", "50"]);
    }

    #[test]
    fn a_const_beside_an_enum_wins() {
        // fal generates a `const` from a pydantic `Literal`, so a sibling
        // `enum` is the wider declared type rather than a second offer.
        let doc = r#"{"components":{"schemas":{"XInput":{"properties":{
            "aspect_ratio":{"const":"auto","enum":["auto","16:9","9:16"]}}}}}}"#;
        let s = parsed(doc);
        assert_eq!(s.enums.get("aspect_ratio").unwrap(), &["auto"]);
        assert!(s.pinned.contains("aspect_ratio"));
    }

    #[test]
    fn a_nullable_integer_is_not_enumerated() {
        // `alibaba/wan-3.0-prime/*`, verbatim. A range is not an offer: turning
        // it into an enum would invent members fal never published, and the
        // free-form axis it produces today is correct.
        let doc = r#"{"components":{"schemas":{"XInput":{"properties":{
            "duration":{"default":5,"anyOf":[{"type":"integer","minimum":2,"maximum":30},{"type":"null"}]}}}}}}"#;
        let s = parsed(doc);
        assert!(!s.enums.contains_key("duration"), "{:?}", s.enums);
        assert!(s.pinned.is_empty());
    }

    #[test]
    fn a_nullable_integer_range_is_recorded_from_anyof() {
        // `alibaba/wan-3.0-prime/*`, verbatim: the range lives inside
        // `anyOf[{integer 2..30}, {null}]`, which the enum reader ignores.
        let doc = r#"{"components":{"schemas":{"XInput":{"properties":{
            "duration":{"default":5,"anyOf":[{"type":"integer","minimum":2,"maximum":30},{"type":"null"}]}}}}}}"#;
        let s = parsed(doc);
        assert_eq!(s.ranges.get("duration").copied(), Some((2.0, 30.0)));
        assert!(s.enums.is_empty(), "{:?}", s.enums);
        assert!(s.pinned.is_empty());
    }

    #[test]
    fn a_direct_integer_min_max_is_recorded() {
        // The H3 Max shape, were fal to publish min/max directly.
        let doc = r#"{"components":{"schemas":{"XInput":{"properties":{
            "duration":{"type":"integer","minimum":5,"maximum":15,"default":5}}}}}}"#;
        let s = parsed(doc);
        assert_eq!(s.ranges.get("duration").copied(), Some((5.0, 15.0)));
    }

    #[test]
    fn a_field_with_no_bound_has_no_range() {
        // A plain string, a `const`, and a full enum each carry no range.
        let doc = r#"{"components":{"schemas":{"XInput":{"properties":{
            "prompt":{"type":"string"},
            "mode":{"const":"balanced","type":"string"},
            "resolution":{"type":"string","enum":["480P","768P"]},
            "duration":{"type":"integer","minimum":5,"maximum":15}}}}}}"#;
        let s = parsed(doc);
        assert_eq!(s.ranges.get("prompt"), None);
        assert_eq!(s.ranges.get("mode"), None);
        assert_eq!(s.ranges.get("resolution"), None);
        assert_eq!(s.ranges.get("duration").copied(), Some((5.0, 15.0)));
    }

    #[test]
    fn a_null_member_is_skipped_without_voiding_the_enum() {
        let doc = r#"{"components":{"schemas":{"XInput":{"properties":{
            "duration":{"enum":[4,null,8]}}}}}}"#;
        assert_eq!(parsed(doc).enums.get("duration").unwrap(), &["4", "8"]);
    }

    #[test]
    fn an_enum_we_cannot_fully_read_is_recorded_as_nothing() {
        // All-or-nothing. A partially recorded enum is the LTX 2.5 failure: it
        // looks authoritative and refuses values the endpoint actually takes.
        let doc = r#"{"components":{"schemas":{"XInput":{"properties":{
            "size":{"enum":[{"width":1024},"auto"]}}}}}}"#;
        let s = parsed(doc);
        assert!(!s.enums.contains_key("size"), "{:?}", s.enums);
        assert!(s.pinned.is_empty());
    }

    #[test]
    fn a_const_that_is_not_a_scalar_is_ignored_entirely() {
        let doc = r#"{"components":{"schemas":{"XInput":{"properties":{
            "size":{"const":{"width":1024}},"other":{"const":null}}}}}}"#;
        let s = parsed(doc);
        assert!(s.enums.is_empty(), "{:?}", s.enums);
        assert!(s.pinned.is_empty());
    }

    #[test]
    fn a_pinned_field_is_written_to_the_value_the_endpoint_fixes() {
        // Seedance 2.5's image-to-video endpoint: `aspect_ratio` is `const
        // "auto"`, so a stale "16:9" in the settings is a guaranteed 422. There
        // is no neighbouring ratio to have picked instead — the endpoint
        // renders one shape — so the value is written and reported, not refused.
        let schema = pinned_to("aspect_ratio", "auto");
        let mut body = serde_json::Map::new();
        body.insert("aspect_ratio".into(), serde_json::json!("16:9"));

        let pinned = reconcile(&schema, &mut body).expect("a pin is not a refusal");
        assert_eq!(body["aspect_ratio"], serde_json::json!("auto"));
        assert_eq!(pinned.len(), 1);
        assert_eq!(pinned[0].field, "aspect_ratio");
        assert_eq!(pinned[0].from, "16:9");
        assert_eq!(pinned[0].to, "auto");
    }

    #[test]
    fn a_pinned_duration_accepts_the_bare_number() {
        // `7` against the pin `"7s"` is the same duration spelled differently:
        // nothing was overridden, so nothing is reported.
        let schema = pinned_to("duration", "7s");
        let mut body = serde_json::Map::new();
        body.insert("duration".into(), serde_json::json!(7.0));

        let pinned = reconcile(&schema, &mut body).unwrap();
        assert_eq!(body["duration"], serde_json::json!("7s"));
        assert!(pinned.is_empty(), "{pinned:?}");
    }

    #[test]
    fn a_pinned_prompt_or_media_key_is_refused_not_rewritten() {
        // No live endpoint pins either today. The guard exists because reading
        // `const` makes it possible for the first time, and overwriting what a
        // person typed or uploaded with a schema constant would be this
        // module's founding failure committed in a new place.
        for field in ["prompt", "image_url"] {
            let schema = pinned_to(field, "fixed");
            let mut body = serde_json::Map::new();
            body.insert(field.into(), serde_json::json!("what the user gave us"));

            let err = reconcile(&schema, &mut body).expect_err("must refuse, not rewrite");
            assert_eq!(err.field, field);
            assert_eq!(body[field], serde_json::json!("what the user gave us"));
        }
    }

    #[test]
    fn a_pinned_field_the_body_does_not_carry_is_left_to_fal() {
        let schema = pinned_to("duration", "7s");
        let mut body = serde_json::Map::new();
        body.insert("prompt".into(), serde_json::json!("a red door"));

        assert!(reconcile(&schema, &mut body).unwrap().is_empty());
        assert!(!body.contains_key("duration"));
    }

    #[test]
    fn a_numeric_duration_matches_an_integer_enum_member() {
        // The LTX 2.5 submit that failed on every attempt before this slice.
        let schema = enumerated("duration", &["6", "8", "10", "auto"]);
        let mut body = serde_json::Map::new();
        body.insert("duration".into(), serde_json::json!(6.0));

        assert!(reconcile(&schema, &mut body).unwrap().is_empty());
        assert_eq!(body["duration"], serde_json::json!("6"));
    }

    #[test]
    fn the_two_ltx_endpoints_do_not_agree_on_aspect_ratio() {
        // Copied from the two documents fal served on 2026-09-16. They are
        // separate documents and they differ: the image-to-video half adds
        // `auto` (it reads the shape off the attached still). One shared enum
        // across both would offer a value one of them rejects, which is the
        // failure this module exists to stop — hence two specs.
        let t2v = parsed(
            r#"{"components":{"schemas":{"Ltx25TextToVideoProInput":{"properties":{
            "aspect_ratio":{"type":"string","default":"16:9","enum":["16:9","9:16"]},
            "duration":{"default":"auto","enum":[6,8,10,"auto"]},
            "resolution":{"type":"string","default":"1080p","enum":["720p","1080p"]}}}}}}"#,
        );
        let i2v = parsed(
            r#"{"components":{"schemas":{"Ltx25ImageToVideoProInput":{"properties":{
            "aspect_ratio":{"type":"string","default":"auto","enum":["auto","16:9","9:16"]},
            "duration":{"default":"auto","enum":[6,8,10,"auto"]},
            "camera_motion":{"anyOf":[{"type":"string","enum":["static"]},{"type":"null"}]},
            "resolution":{"type":"string","default":"1080p","enum":["720p","1080p"]}}}}}}"#,
        );
        assert_eq!(t2v.enums.get("aspect_ratio").unwrap(), &["16:9", "9:16"]);
        assert_eq!(
            i2v.enums.get("aspect_ratio").unwrap(),
            &["auto", "16:9", "9:16"]
        );
        // Both agree on the duration and resolution the specs enumerate.
        assert_eq!(t2v.enums.get("duration"), i2v.enums.get("duration"));
        assert_eq!(t2v.enums.get("resolution"), i2v.enums.get("resolution"));
        // An enum nested inside `anyOf` is not read, so `camera_motion` stays
        // an undeclared field rather than a half-understood one.
        assert!(!i2v.enums.contains_key("camera_motion"));
    }
}
