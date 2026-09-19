//! The hermetic price audit: every fal route's literal `CostModel` against
//! fal's own published rate in the vendored index snapshot.
//!
//! Run: `cargo run -p hickeyfield-core --example price_check`
//!
//! Keyless and offline — it reads `vendor/fal-catalogue-snapshot.json` and the
//! registry, never the network. The question it answers is not "does the
//! fetcher compile" but "is any hand-typed price literal contradicting fal's
//! own number". A `MISMATCH` is a wrong literal and fails the AC1 unit test;
//! the `PROSE`/`UNPUBLISHED`/`UNMODELLABLE` sections are the routes fal states
//! as conditional prose the parser refuses, printed for a human and the live
//! seam (see `tests/live_fal_pricing.rs`).

use hickeyfield_core::prices::{audit, promo_recheck, AuditRow, AUDIT_TOLERANCE, PROMO_STALE_DAYS};
use std::time::{SystemTime, UNIX_EPOCH};

fn section(title: &str, rows: &[String]) {
    println!("\n=== {title} ({}) ===", rows.len());
    for r in rows {
        println!("  {r}");
    }
    if rows.is_empty() {
        println!("  none");
    }
}

/// Today's civil date from the wall clock — the example's ONE non-hermetic
/// input, so a human running the audit sees which promos are stale *now*. The
/// library `promo_recheck` keeps its injected `today` and never reads the clock;
/// only this printer does. Howard Hinnant's `civil_from_days` inverse, inlined
/// so the crate stays date-crate-free (R-RULES.4).
fn today_civil() -> (i32, u32, u32) {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let z = secs / 86_400 + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m as u32, d as u32)
}

/// Print any promotional rate older than `PROMO_STALE_DAYS` as of today, so the
/// re-check is visible to whoever runs the audit — the human-reachable caller
/// the `promo_recheck` API otherwise lacks. Keyless and offline: it reads only
/// the registry, exactly like `audit()`.
fn promo_recheck_section() {
    let (y, m, d) = today_civil();
    let stale = promo_recheck((y, m, d));
    let rows: Vec<String> = stale
        .iter()
        .map(|r| {
            format!(
                "{:<52} promo as-of {} — {} days old",
                r.route_id, r.as_of, r.age_days
            )
        })
        .collect();
    section(
        &format!("PROMO RE-CHECK (older than {PROMO_STALE_DAYS} days as of {y:04}-{m:02}-{d:02})"),
        &rows,
    );
}

fn main() {
    let rows = audit();

    let mut mismatch = Vec::new();
    let mut ok = Vec::new();
    let mut prose = Vec::new();
    let mut unpublished = Vec::new();
    let mut unmodellable = Vec::new();
    let mut no_snapshot = Vec::new();

    for row in &rows {
        match row {
            AuditRow::Mismatch {
                endpoint,
                unit,
                fal_usd,
                literal_usd,
                ratio,
            } => mismatch.push(format!(
                "{endpoint:<52} literal ${literal_usd:.4} vs fal ${fal_usd:.4} {} — {ratio:.2}x",
                unit.label()
            )),
            AuditRow::Ok {
                endpoint,
                unit,
                fal_usd,
                literal_usd,
            } => ok.push(format!(
                "{endpoint:<52} literal ${literal_usd:.4} ~ fal ${fal_usd:.4} {}",
                unit.label()
            )),
            AuditRow::Prose { endpoint, prose: p } => {
                prose.push(format!("{endpoint:<52} {}", p.replace('\n', " ")))
            }
            AuditRow::Unpublished { endpoint } => {
                unpublished.push(format!("{endpoint:<52} (no fal pricingInfoOverride)"))
            }
            AuditRow::UnmodellableUnit {
                endpoint,
                unit,
                fal_usd,
            } => unmodellable.push(format!(
                "{endpoint:<52} fal ${fal_usd:.4} {} — CostModel cannot express",
                unit.label()
            )),
            AuditRow::NoSnapshot { endpoint } => {
                no_snapshot.push(format!("{endpoint:<52} (not in snapshot)"))
            }
        }
    }

    println!("hermetic fal price audit (tolerance {AUDIT_TOLERANCE}x)");
    println!("endpoints audited: {}", rows.len());

    section("MISMATCH (build-failing)", &mismatch);
    section("OK (literal within tolerance of fal)", &ok);
    section(
        "UNMODELLABLE UNIT (fal rate, CostModel cannot express)",
        &unmodellable,
    );
    section(
        "PROSE (fal conditional/multi-figure — live-seam only)",
        &prose,
    );
    section("UNPUBLISHED (fal states no price)", &unpublished);
    section(
        "NO SNAPSHOT (endpoint absent from vendored index)",
        &no_snapshot,
    );
    promo_recheck_section();

    let with_prose = ok.len() + prose.len() + unmodellable.len() + mismatch.len();
    println!(
        "\nMISMATCH {} / OK {} / UNMODELLABLE {} / PROSE {} / UNPUBLISHED {} / NO-SNAPSHOT {}",
        mismatch.len(),
        ok.len(),
        unmodellable.len(),
        prose.len(),
        unpublished.len(),
        no_snapshot.len(),
    );
    println!(
        "coverage: {} endpoints checked / {} with fal prose / {} with null prose",
        rows.len(),
        with_prose,
        unpublished.len(),
    );
}
