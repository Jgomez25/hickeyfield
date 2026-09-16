//! Audit the registry against fal's published schemas.
//!
//! Every bug found on 2026-08-05 was the same shape: the vendored catalogue
//! describes *Higgsfield's* API, we send to fal, and the two disagree. Each was
//! found by a user hitting it. This finds the rest mechanically.
//!
//! Five checks, in descending order of how much they cost when wrong:
//!
//! 1. **Silently ignored media** — we let the user attach a file to an endpoint
//!    with no field for it. fal drops it and bills for a generation that did
//!    not do what was asked. This is the one that cost real money.
//! 2. **Missing required fields** — the endpoint demands something we never
//!    send, so the request 422s after a round trip.
//! 3. **Dropped settings** — a control the user changed that the endpoint does
//!    not declare, so it silently has no effect.
//! 4. **Wrong option sets** — our chip row offers values the endpoint rejects.
//! 5. **Pinned fields** — the endpoint fixes a value with `const` and we can
//!    still send another. Invisible to check 4, which needs both sides to
//!    enumerate: `fal-ai/veo3.1/extend-video` renders at `7s`/`720p` and
//!    nothing else, and the app cheerfully sent `5.0`/`1080p` for a slice.
//!
//! Run: `cargo run -p hickeyfield-core --example audit_fal`. No key needed; fal's
//! schema endpoint is unauthenticated.

use hickeyfield_core::capability::ModelCapability;
use hickeyfield_core::catalog::{ModelSpec, ValueSpec};
use hickeyfield_core::fal_schema::{self, EndpointSchema};
use hickeyfield_core::media::{self, InputMode};
use hickeyfield_core::registry;
use std::collections::BTreeSet;

fn main() {
    let reg = registry::registry();
    let mut ignored_media = Vec::new();
    let mut missing_required = Vec::new();
    let mut dropped_settings = Vec::new();
    let mut wrong_options = Vec::new();
    let mut pinned_fields = Vec::new();
    let mut unreachable = Vec::new();
    // Kept as rows, not a counter: "which slugs did you actually probe" is the
    // question an importer has to answer, and a number cannot answer it.
    let mut checked: Vec<String> = Vec::new();

    for m in reg.values() {
        let Some(route) = m
            .routes
            .iter()
            .find(|r| r.provider == hickeyfield_core::ProviderId::Fal)
        else {
            continue;
        };
        let produces_video = format!("{}", m.modality) == "video";

        // Audit the modes the model actually offers. A t2v-only endpoint and an
        // i2v endpoint are different contracts and both can be wrong.
        for mode in [InputMode::Text, InputMode::Image, InputMode::Video] {
            let Ok(endpoint) = media::resolve_endpoint(&route.slug, mode, produces_video) else {
                continue;
            };
            // Text and Image can resolve to the same endpoint for exact slugs;
            // do not audit it twice.
            if mode != InputMode::Text
                && media::resolve_endpoint(&route.slug, InputMode::Text, produces_video).as_deref()
                    == Ok(endpoint.as_str())
            {
                continue;
            }

            let Some(schema) = fal_schema::for_endpoint(&endpoint) else {
                unreachable.push(format!("{} -> {endpoint}", m.id));
                continue;
            };
            checked.push(format!("{:<28} {endpoint}", m.id));

            // 1. Does the catalogue promise media NO mode of this route can
            //    take? A text-to-video endpoint rejecting a start frame is by
            //    design — the user attaching one resolves to image-to-video
            //    instead. Only flag a route where every mode refuses media,
            //    because that is the case where the UI lets you attach a file
            //    that will be silently dropped.
            let any_mode_takes_media = [InputMode::Text, InputMode::Image, InputMode::Video]
                .into_iter()
                .filter_map(|md| media::resolve_endpoint(&route.slug, md, produces_video).ok())
                .filter_map(|ep| fal_schema::for_endpoint(&ep))
                .any(|sc| sc.takes_media());
            let catalogue_takes_media = m.spec.media_flags().next().is_some();
            if catalogue_takes_media && !any_mode_takes_media && mode == InputMode::Text {
                ignored_media.push(format!(
                    "{:<26} {endpoint}\n      catalogue offers [{}] but the endpoint takes none",
                    m.id,
                    m.spec
                        .media_flags()
                        .map(|f| f.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }

            // 2. Required fields we would never send.
            //
            // `prompt` is always supplied by the user, and a media key is
            // supplied when they attach something — so only flag the rest,
            // which is where a silent 422 comes from.
            let never_sent: Vec<&str> = schema
                .required
                .iter()
                .map(String::as_str)
                .filter(|k| *k != "prompt" && !schema.media_keys().contains(k))
                .collect();
            if !never_sent.is_empty() {
                missing_required.push(format!(
                    "{:<26} {endpoint}\n      requires [{}] which nothing in the app supplies",
                    m.id,
                    never_sent.join(", ")
                ));
            }

            // 3. Settings the UI can change that this endpoint ignores.
            for (ours, theirs) in [
                ("duration", "duration"),
                ("resolution", "resolution"),
                ("aspect", "aspect_ratio"),
                ("audio", "generate_audio"),
            ] {
                let we_offer = match ours {
                    "duration" => m.spec.capabilities().supports_duration,
                    "resolution" => m.spec.capabilities().supports_resolution,
                    "aspect" => m.spec.capabilities().supports_aspect,
                    _ => m.spec.capabilities().audio,
                };
                if we_offer && !schema.accepts(theirs) {
                    dropped_settings.push(format!(
                        "{:<26} {endpoint}\n      UI offers `{ours}` but the endpoint has no `{theirs}`",
                        m.id
                    ));
                }
            }

            // 4. Option sets the endpoint would reject. Only checkable where
            //    both sides enumerate.
            for flag in ["resolution", "aspect_ratio", "duration"] {
                let (Some(ours), Some(theirs)) = (
                    m.spec.flag(flag).and_then(|f| match &f.value {
                        ValueSpec::Enum(v) => Some(v.iter().cloned().collect::<BTreeSet<_>>()),
                        _ => None,
                    }),
                    schema
                        .enums
                        .get(flag)
                        .map(|v| v.iter().cloned().collect::<BTreeSet<_>>()),
                ) else {
                    continue;
                };
                let rejected: Vec<String> = ours.difference(&theirs).cloned().collect();
                if !rejected.is_empty() {
                    wrong_options.push(format!(
                        "{:<26} {endpoint}\n      offers {flag} [{}] which the endpoint rejects (it takes [{}])",
                        m.id,
                        rejected.join(", "),
                        theirs.iter().cloned().collect::<Vec<_>>().join(", ")
                    ));
                }
            }

            // 5. Values this endpoint pins with `const` while the app can still
            //    produce another. Check 4 cannot see these: a pin is not an
            //    `enum`, and before the parser read `const` neither side
            //    enumerated anything to compare.
            for (field, pin, could_send) in pin_conflicts(&m.spec, &endpoint, &schema) {
                pinned_fields.push(format!(
                    "{:<26} {endpoint}\n      pins {field} = {pin} but the app can send [{}]",
                    m.id,
                    could_send.join(", ")
                ));
            }
        }
    }

    section(
        "SILENTLY IGNORED MEDIA — bills for the wrong generation",
        &ignored_media,
    );
    section(
        "MISSING REQUIRED FIELDS — 422 after a round trip",
        &missing_required,
    );
    section(
        "DROPPED SETTINGS — the control has no effect",
        &dropped_settings,
    );
    section(
        "REJECTED OPTIONS — the chip row offers invalid values",
        &wrong_options,
    );
    section(
        "PINNED FIELDS — the endpoint fixes this value and we would send another",
        &pinned_fields,
    );
    // The loudest signal of all: we route to a slug fal does not serve. It was
    // collected and thrown away until 2026-09-15.
    section(
        "UNREACHABLE — fal serves no schema for this endpoint",
        &unreachable,
    );

    section("CHECKED — fal answered with a schema", &checked);

    println!(
        "\n{} endpoint(s) audited against fal's own schema. A slug listed under \
         CHECKED and under nothing else is one this registry can drive.",
        checked.len()
    );
}

/// Fields this endpoint pins where the app can still produce a different value.
///
/// Pure and network-free, so it is testable: the pin comes from the schema the
/// caller already fetched. Returns `(field, pinned_value, values_we_could_send)`.
///
/// Two paths can produce a value, and both are checked because the one that
/// actually bit `veo3_1_extend` was the second:
///
/// 1. **Online.** `ModelCapability::from_fal` reads the schema, so after the
///    parser learned `const` a mapped axis structurally cannot offer anything
///    but the pin. Asserting it anyway is the regression guard on `fal_axis`
///    staying schema-sourced.
/// 2. **Offline.** When fal's schema endpoint is unreachable, `capability::for_route`
///    falls back to the catalogue and the axis comes from the *spec*. A spec
///    flag that is free-form, or that enumerates a value the pin rejects, is a
///    422 waiting for the next outage.
///
/// `aspect_ratio` is checked even when the spec declares no such flag, because
/// the submit path derives that key from the **schema** and then always sends
/// something explicit.
fn pin_conflicts(
    spec: &ModelSpec,
    endpoint: &str,
    schema: &EndpointSchema,
) -> Vec<(String, String, Vec<String>)> {
    let cap = ModelCapability::from_fal(spec, endpoint, schema);
    let mut out = Vec::new();

    for field in &schema.pinned {
        let Some(pin) = schema.enums.get(field).and_then(|v| v.first()).cloned() else {
            continue;
        };
        let mut could_send: Vec<String> = Vec::new();
        let agrees = |v: &str| schema.coerce(field, v).as_deref() == Some(pin.as_str());

        // 1. The live capability axis.
        let axis = match field.as_str() {
            "duration" => Some(&cap.duration),
            "resolution" => Some(&cap.resolution),
            "aspect_ratio" => Some(&cap.aspect),
            _ => None,
        };
        if let Some(axis) = axis {
            could_send.extend(axis.values.iter().filter(|v| !agrees(v)).cloned());
        }

        // 2. The offline capability axis, which is the spec's flag.
        match spec.flag(field).map(|f| &f.value) {
            Some(ValueSpec::Enum(values)) => {
                could_send.extend(values.iter().filter(|v| !agrees(v)).cloned());
            }
            Some(ValueSpec::Number) | Some(ValueSpec::Integer) | Some(ValueSpec::Text) => {
                could_send.push("free-form, offline".to_string());
            }
            // 3. No flag at all still sends an aspect ratio, from the settings.
            None if field == "aspect_ratio" && axis.is_some_and(|a| a.values.is_empty()) => {
                could_send.push("free-form, offline".to_string());
            }
            _ => {}
        }

        could_send.sort();
        could_send.dedup();
        if !could_send.is_empty() {
            out.push((field.clone(), pin, could_send));
        }
    }
    out
}

fn section(title: &str, rows: &[String]) {
    println!("\n=== {title} ({}) ===", rows.len());
    for r in rows {
        println!("  {r}");
    }
    if rows.is_empty() {
        println!("  none");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hickeyfield_core::catalog::{Arity, FlagSpec, Modality};

    /// `fal-ai/veo3.1/extend-video`, as fal published it on 2026-09-16.
    const VEO_EXTEND: &str = r#"{"components":{"schemas":{"Veo31ExtendVideoInput":{"properties":{
        "prompt":{"type":"string"},
        "video_url":{"type":"string"},
        "duration":{"const":"7s","type":"string","default":"7s"},
        "resolution":{"const":"720p","type":"string","default":"720p"}},
        "required":["prompt","video_url"]}}}}"#;

    fn spec_with(flags: &[(&str, ValueSpec)]) -> ModelSpec {
        ModelSpec {
            id: "veo3_1_extend".into(),
            display_name: "Veo 3.1 Extend".into(),
            modality: Modality::Video,
            flags: flags
                .iter()
                .map(|(name, value)| FlagSpec {
                    name: (*name).to_string(),
                    alias: None,
                    required: false,
                    default: None,
                    value: value.clone(),
                    arity: Arity::One,
                })
                .collect(),
            constraints: Vec::new(),
        }
    }

    #[test]
    fn a_const_the_spec_contradicts_is_reported() {
        // The B2 failure of the 2026-09-16 import: the spec offered a free
        // duration and a 1080p resolution against an endpoint that renders 7s
        // at 720p and nothing else. Every submit was a 422.
        let schema = fal_schema::parse(&serde_json::from_str(VEO_EXTEND).unwrap()).unwrap();
        let spec = spec_with(&[
            ("duration", ValueSpec::Number),
            (
                "resolution",
                ValueSpec::Enum(vec!["720p".into(), "1080p".into()]),
            ),
        ]);

        let found = pin_conflicts(&spec, "fal-ai/veo3.1/extend-video", &schema);
        let fields: Vec<&str> = found.iter().map(|(f, _, _)| f.as_str()).collect();
        assert_eq!(fields, ["duration", "resolution"], "{found:?}");
        let (_, pin, could_send) = &found[1];
        assert_eq!(pin, "720p");
        assert_eq!(could_send, &["1080p".to_string()]);
    }

    #[test]
    fn a_spec_that_mirrors_the_pin_reports_nothing() {
        // The shape this slice lands: the pins are mirrored as one-member
        // enums, so the offline path agrees with the online one.
        let schema = fal_schema::parse(&serde_json::from_str(VEO_EXTEND).unwrap()).unwrap();
        let spec = spec_with(&[
            ("duration", ValueSpec::Enum(vec!["7s".into()])),
            ("resolution", ValueSpec::Enum(vec!["720p".into()])),
        ]);
        assert_eq!(
            pin_conflicts(&spec, "fal-ai/veo3.1/extend-video", &schema),
            vec![]
        );
    }

    #[test]
    fn an_endpoint_that_pins_nothing_reports_nothing() {
        let doc = r#"{"components":{"schemas":{"XInput":{"properties":{
            "prompt":{"type":"string"},
            "duration":{"enum":[6,8,10,"auto"]}}}}}}"#;
        let schema = fal_schema::parse(&serde_json::from_str(doc).unwrap()).unwrap();
        let spec = spec_with(&[("duration", ValueSpec::Number)]);
        assert!(pin_conflicts(&spec, "lightricks/x", &schema).is_empty());
    }
}
