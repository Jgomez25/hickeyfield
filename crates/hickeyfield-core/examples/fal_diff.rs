//! What fal serves that the registry cannot reach.
//!
//! [`hickeyfield_core::fal_catalogue`] reads fal's own index — 1,400-odd live
//! endpoints, unauthenticated, one request a page. [`hickeyfield_core::registry`]
//! holds the few dozen we route to. Nothing joined the two, so "is our
//! catalogue current?" was a question only a human reading fal's website could
//! answer, and the answer drifted.
//!
//! This is that join, in the direction that matters: **fal minus registry**,
//! bucketed by the five use cases the picker offers, so the output is a
//! shortlist of models a person could import rather than a wall of 1,418 rows.
//!
//! The comparison is by **exact endpoint id**, never by prefix — `fal-ai/flux`
//! and `fal-ai/flux/dev` are different endpoints at different prices. Registry
//! slugs are usually family *roots*, so each is first expanded through
//! [`hickeyfield_core::media::resolve_endpoint`] — the same resolver the submit
//! path uses — into the endpoints it can actually reach. Without that,
//! `fal-ai/kling-video/v3/standard` would be reported as missing from fal while
//! `…/v3/pro` hid in the noise.
//!
//! ```sh
//! cargo run -p hickeyfield-core --example fal_diff             # diff, live
//! cargo run -p hickeyfield-core --example fal_diff -- --offline # diff, bundled snapshot
//! cargo run -p hickeyfield-core --example fal_diff -- --dump    # snapshot JSON on stdout
//! ```
//!
//! No key is read and none is needed. `--dump` writes to stdout only: a tool
//! that overwrites the vendored snapshot in place is a tool that can destroy
//! the offline fallback on a partial fetch.

use std::collections::{BTreeSet, HashMap};

use hickeyfield_core::fal_catalogue::{self, Captured, Catalogue, Category, Model, Pricing};
use hickeyfield_core::media::{self, InputMode};
use hickeyfield_core::registry;
use hickeyfield_core::use_case::UseCase;
use hickeyfield_core::ProviderId;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dump = args.iter().any(|a| a == "--dump");
    let offline = args.iter().any(|a| a == "--offline");
    if let Some(unknown) = args
        .iter()
        .find(|a| !matches!(a.as_str(), "--dump" | "--offline"))
    {
        eprintln!("unknown argument {unknown}; expected --dump and/or --offline");
        std::process::exit(2);
    }

    let catalogue = if offline {
        Catalogue::bundled()
    } else {
        match fal_catalogue::fetch() {
            Ok(c) => c,
            Err(e) => {
                // `CatalogueError`'s own Display already separates a Vercel
                // challenge from a 500, and the two mean opposite things.
                eprintln!("fal's index did not answer: {e}");
                std::process::exit(1);
            }
        }
    };

    if dump {
        dump_snapshot(&catalogue);
        return;
    }
    report(&catalogue);
}

/// Print the snapshot file's exact shape on stdout.
///
/// `Catalogue::all` is post-filter: retired, id-less and duplicate rows are
/// dropped at construction. That makes the dump lossless only while fal carries
/// no retired rows, so this refuses to write rather than silently baking fal's
/// retirements out of the vendored file.
fn dump_snapshot(catalogue: &Catalogue) {
    if catalogue.retired() > 0 {
        eprintln!(
            "refusing to dump: fal now publishes {} retired row(s), which this \
             catalogue drops at load. Decide deliberately what the snapshot \
             should carry before refreshing it.",
            catalogue.retired()
        );
        std::process::exit(1);
    }
    let doc = serde_json::json!({
        "snapshot_date": today_utc(),
        "items": catalogue.all(),
    });
    match serde_json::to_string_pretty(&doc) {
        Ok(s) => println!("{s}"),
        Err(e) => {
            eprintln!("could not serialise the snapshot: {e}");
            std::process::exit(1);
        }
    }
}

/// `YYYY-MM-DD`, UTC, without pulling in a date crate.
fn today_utc() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = (secs / 86_400) as i64;
    // Howard Hinnant's civil-from-days, the standard integer algorithm.
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

/// Every fal endpoint some registry route can reach.
///
/// Identical construction to `examples/audit_fal.rs`, deliberately: the two
/// tools must not be able to disagree about what the registry covers.
fn covered_endpoints() -> BTreeSet<String> {
    let mut covered = BTreeSet::new();
    for m in registry::registry().values() {
        let produces_video = format!("{}", m.modality) == "video";
        for route in m.routes.iter().filter(|r| r.provider == ProviderId::Fal) {
            for mode in [InputMode::Text, InputMode::Image, InputMode::Video] {
                if let Ok(ep) = media::resolve_endpoint(&route.slug, mode, produces_video) {
                    covered.insert(ep);
                }
            }
        }
    }
    covered
}

/// The fal rows no registry route reaches, bucketed by use case.
///
/// `covered` is the endpoint set expanded from the registry. Rows whose fal
/// category maps to none of the five use cases are not returned at all — see
/// [`use_case_of`] — and the caller counts them separately rather than letting
/// them vanish.
fn fal_minus_registry<'a>(
    catalogue: &'a Catalogue,
    covered: &BTreeSet<String>,
) -> HashMap<UseCase, Vec<&'a Model>> {
    let mut out: HashMap<UseCase, Vec<&Model>> = HashMap::new();
    for m in catalogue.all() {
        if covered.contains(&m.id) {
            continue;
        }
        let Some(uc) = use_case_of(&m.category) else {
            continue;
        };
        out.entry(uc).or_default().push(m);
    }
    for rows in out.values_mut() {
        rows.sort_by(|a, b| a.id.cmp(&b.id));
    }
    out
}

/// fal's category, as one of the app's five jobs — or `None`.
///
/// `None` is the honest answer for `Training`, `Llm`, `TextToAudio`,
/// `Category::Other` and `Category::Unknown` alike. Coercing any of them into a
/// use case would put a speech model on the video tab of a report someone then
/// imports from.
fn use_case_of(category: &Category) -> Option<UseCase> {
    match category {
        Category::TextToVideo => Some(UseCase::TextToVideo),
        Category::ImageToVideo => Some(UseCase::ImageToVideo),
        Category::VideoToVideo => Some(UseCase::EditVideo),
        Category::TextToImage => Some(UseCase::TextToImage),
        Category::ImageToImage => Some(UseCase::EditImage),
        _ => None,
    }
}

fn report(catalogue: &Catalogue) {
    let covered = covered_endpoints();
    let missing = fal_minus_registry(catalogue, &covered);

    let captured = match catalogue.captured() {
        Captured::Snapshot { date } => format!("bundled snapshot of {date}"),
        Captured::Live { unix_seconds } => format!("live fetch (unix {unix_seconds})"),
    };
    println!(
        "fal index: {} model(s), {captured}. Registry reaches {} fal endpoint(s).",
        catalogue.len(),
        covered.len()
    );

    let in_scope = catalogue
        .all()
        .iter()
        .filter(|m| use_case_of(&m.category).is_some())
        .count();
    let mut shown = 0usize;
    for uc in UseCase::ALL {
        let rows = missing.get(&uc).map(Vec::as_slice).unwrap_or(&[]);
        println!(
            "\n=== {} — not in the registry ({}) ===",
            uc.slug(),
            rows.len()
        );
        for m in rows {
            println!(
                "  {:<52} | {:<38} | {:<22} | {}",
                m.id,
                truncate(&m.title, 38),
                truncate(m.model_family.as_deref().unwrap_or("-"), 22),
                price_of(m)
            );
        }
        if rows.is_empty() {
            println!("  none");
        }
        shown += rows.len();
    }

    println!(
        "\n{shown} of {in_scope} generation model(s) are outside the registry; \
         {} model(s) in other categories (training, audio, vision, …) are not shown.",
        catalogue.len() - in_scope
    );
}

/// What fal said about the price, never dressed up as more than that.
fn price_of(m: &Model) -> String {
    match m.pricing() {
        Pricing::Rate { usd, unit, .. } => format!("${usd} {}", unit.label()),
        Pricing::Unparsed { prose } => format!("(prose) {}", truncate(&prose, 60)),
        Pricing::Unpublished => "(none)".to_string(),
    }
}

fn truncate(s: &str, n: usize) -> String {
    let flat = s.replace(['\n', '\r'], " ");
    if flat.chars().count() <= n {
        return flat;
    }
    flat.chars().take(n.saturating_sub(1)).collect::<String>() + "…"
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: &str, category: Category) -> Model {
        Model {
            id: id.to_string(),
            title: id.to_string(),
            category,
            short_description: String::new(),
            pricing_info_override: None,
            deprecated: false,
            removed: false,
            model_family: None,
            thumbnail_url: None,
        }
    }

    #[test]
    fn fal_minus_registry_reports_only_endpoints_no_route_reaches() {
        let mut retired = row("vendor/retired/text-to-video", Category::TextToVideo);
        retired.deprecated = true;

        let catalogue = Catalogue::new(
            vec![
                // Reached by a family-root route once it is expanded.
                row(
                    "fal-ai/kling-video/v3/standard/text-to-video",
                    Category::TextToVideo,
                ),
                row(
                    "fal-ai/kling-video/v3/standard/image-to-video",
                    Category::ImageToVideo,
                ),
                // Vendor-namespaced and reached by nothing.
                row("minimax/h3-max/text-to-video", Category::TextToVideo),
                row("openai/some-editor", Category::ImageToImage),
                // Out of scope for the five use cases.
                row("fal-ai/some-trainer", Category::Training),
                retired,
            ],
            Captured::Snapshot {
                date: "2026-09-15".into(),
            },
        );
        assert_eq!(catalogue.retired(), 1, "the retired row is dropped at load");
        assert!(
            catalogue.get("vendor/retired/text-to-video").is_none(),
            "a retired row must never reach the report"
        );

        let covered: BTreeSet<String> = [
            "fal-ai/kling-video/v3/standard/text-to-video",
            "fal-ai/kling-video/v3/standard/image-to-video",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();

        let missing = fal_minus_registry(&catalogue, &covered);

        let ids = |uc: UseCase| -> Vec<&str> {
            missing
                .get(&uc)
                .map(|rows| rows.iter().map(|m| m.id.as_str()).collect())
                .unwrap_or_default()
        };
        assert_eq!(
            ids(UseCase::TextToVideo),
            ["minimax/h3-max/text-to-video"],
            "only the endpoint no route reaches is reported"
        );
        assert_eq!(
            ids(UseCase::EditImage),
            ["openai/some-editor"],
            "image-to-image maps onto the edit-image tab"
        );
        assert!(
            !missing.contains_key(&UseCase::ImageToVideo),
            "the covered family-root expansion is suppressed"
        );
        assert!(
            missing
                .values()
                .flatten()
                .all(|m| m.id != "fal-ai/some-trainer"),
            "an out-of-scope category is never bucketed"
        );
    }

    #[test]
    fn the_category_map_only_claims_the_five_use_cases() {
        // Of every category fal published, exactly the five generation ones map.
        let mapped: Vec<UseCase> = Category::named().iter().filter_map(use_case_of).collect();
        assert_eq!(mapped.len(), 5, "mapped: {mapped:?}");
        for uc in UseCase::ALL {
            assert!(
                mapped.contains(&uc),
                "{} is unreachable from fal",
                uc.slug()
            );
        }
        assert_eq!(use_case_of(&Category::Unknown), None);
        assert_eq!(use_case_of(&Category::Other("agent".into())), None);
        assert_eq!(
            use_case_of(&Category::VideoToVideo),
            Some(UseCase::EditVideo),
            "fal's video-to-video is the app's edit-video job"
        );
    }
}
