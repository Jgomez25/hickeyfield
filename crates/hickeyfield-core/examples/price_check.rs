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

use hickeyfield_core::prices::{audit, AuditRow, AUDIT_TOLERANCE};

fn section(title: &str, rows: &[String]) {
    println!("\n=== {title} ({}) ===", rows.len());
    for r in rows {
        println!("  {r}");
    }
    if rows.is_empty() {
        println!("  none");
    }
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
