//! AC6 — live reconciliation seam. **Human-run only. Spends no money.**
//!
//! The hermetic audit (`prices::audit`, `examples/price_check`) can only compare
//! the ~11 fal routes whose prose reduces to a single unconditional rate. The
//! other ~46 are conditional tables the parser refuses; this test verifies those
//! against fal's *own* live pricing API, which returns a structured
//! `{ unit_price, unit }` per endpoint.
//!
//! It is `#[ignore]` and skips (does not fail) when no key is present, so the
//! keyless `cargo test --workspace` gate never runs it. It reads the key from
//! `hickeyfield_FAL_KEY` or `FAL_KEY` only, and **never logs it** — the
//! `Authorization` header is never printed, only the endpoint and the figures.
//!
//! Run it yourself:
//!
//! ```sh
//! hickeyfield_FAL_KEY=… cargo test -p hickeyfield-core \
//!     --test live_fal_pricing -- --ignored --nocapture
//! ```
//!
//! The GET `models/pricing` endpoint is a price lookup — it starts no
//! generation and is not billed. `first_light` (which does spend money) is never
//! invoked here.

use hickeyfield_core::cost::CostModel;
use hickeyfield_core::media::{self, InputMode};
use hickeyfield_core::provider::ProviderId;
use hickeyfield_core::registry::registry;
use std::collections::BTreeSet;

const PRICING_URL: &str = "https://api.fal.ai/v1/models/pricing";

fn fal_key() -> Option<String> {
    std::env::var("hickeyfield_FAL_KEY")
        .or_else(|_| std::env::var("FAL_KEY"))
        .ok()
        .filter(|k| !k.trim().is_empty())
}

/// The single scalar a literal exposes for a fal unit, mirroring the hermetic
/// audit's `comparable_literal`.
fn literal_usd(cost: &CostModel) -> Option<f64> {
    match cost {
        CostModel::PerSecond { usd, .. } => Some(*usd),
        CostModel::PerImage { usd, .. } => Some(*usd),
        CostModel::PerMegapixel { usd, .. } => Some(*usd),
        CostModel::Flat { usd } => Some(*usd),
        _ => None,
    }
}

#[test]
#[ignore = "spends no money but needs the user's fal key; human-run"]
fn live_fal_pricing_matches_the_literals() {
    let Some(key) = fal_key() else {
        eprintln!("skipping: no hickeyfield_FAL_KEY / FAL_KEY set (this is not a failure)");
        return;
    };

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .expect("http client");

    let reg = registry();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut diffs = 0usize;
    let mut checked = 0usize;

    println!("endpoint                                             fal          literal    ratio");
    for m in reg.values() {
        let produces_video = format!("{}", m.modality) == "video";
        for route in m.routes.iter().filter(|r| r.provider == ProviderId::Fal) {
            let Some(cost) = m.cost_model(route) else {
                continue;
            };
            for mode in [InputMode::Text, InputMode::Image, InputMode::Video] {
                let Ok(ep) = media::resolve_endpoint(&route.slug, mode, produces_video) else {
                    continue;
                };
                if !seen.insert(ep.clone()) {
                    continue;
                }

                // GET the price lookup. The key travels in the Authorization
                // header and is NEVER printed.
                let url = format!("{PRICING_URL}?endpoint_id={ep}");
                let resp = client
                    .get(&url)
                    .header("Authorization", format!("Key {key}"))
                    .send();
                let Ok(resp) = resp else {
                    println!("{ep:<52} (request failed)");
                    continue;
                };
                if !resp.status().is_success() {
                    println!("{ep:<52} (HTTP {})", resp.status().as_u16());
                    continue;
                }
                let Ok(body) = resp.json::<serde_json::Value>() else {
                    println!("{ep:<52} (unreadable body)");
                    continue;
                };
                let fal_price = body.get("unit_price").and_then(|v| v.as_f64()).or_else(|| {
                    body.get("unit_price")
                        .and_then(|v| v.as_str()?.parse().ok())
                });
                let unit = body.get("unit").and_then(|v| v.as_str()).unwrap_or("?");

                match (fal_price, literal_usd(cost)) {
                    (Some(fal), Some(lit)) if fal > 0.0 && lit > 0.0 => {
                        checked += 1;
                        let ratio = (fal / lit).max(lit / fal);
                        let flag = if ratio > 1.2 { "  <-- DIFF" } else { "" };
                        if ratio > 1.2 {
                            diffs += 1;
                        }
                        println!("{ep:<52} ${fal:<10.4} ${lit:<9.4} {ratio:.2}x {unit}{flag}");
                    }
                    (Some(fal), None) => {
                        println!("{ep:<52} ${fal:<10.4} (literal Unknown/tiered) {unit}");
                    }
                    (None, _) => {
                        println!("{ep:<52} (fal published no unit_price)");
                    }
                    _ => {}
                }
            }
        }
    }

    println!("\nchecked {checked} endpoints; {diffs} differ from the literal by >1.2x");
    // Informational for a human; the failing gate is the hermetic audit. A
    // non-zero diff count here is a prompt to update a literal, not a red build.
}
