//! One switch that decides whether this process may spend money.
//!
//! The 2026-09-16 incident (a unit test that reached the keychain and submitted
//! three real generations) showed that a single credential guard is not enough:
//! once a key exists, *any* code path that reaches a billable POST can spend.
//! This module is the layer below the vault — it decides, per process, whether
//! a billable request may be sent at all.
//!
//! The default is OFF. Spending is possible only when:
//!
//! - the packaged app's Tauri entrypoint (`src-tauri/src/lib.rs::run`) called
//!   [`enable`] at startup, or
//! - the process was started with `HICKEYFIELD_ALLOW_SPEND=1` (exact match,
//!   trimmed) — the documented CLI/example override.
//!
//! Tests and examples inherit neither, so their default is OFF *by
//! construction*: the failure mode of a forgotten wiring is "blocked", never
//! "silently spends". `scripts/lint-spend.py` enforces that no test enables it
//! and that `enable()` is called from nowhere but the entrypoint.

use std::sync::OnceLock;

/// Set exactly once, by the packaged app's Tauri entrypoint. Unset in every
/// other process (tests, examples, integration crates), so the default is OFF.
static ALLOWED: OnceLock<bool> = OnceLock::new();

/// Turn spending on for this process. Called once from
/// `src-tauri/src/lib.rs::run` — and nowhere else (enforced by
/// `scripts/lint-spend.py`).
///
/// Idempotent: a second call (e.g. mobile + desktop entry attributes) is a
/// no-op because `OnceLock::set` returns `Err` once set, which we ignore.
pub fn enable() {
    let _ = ALLOWED.set(true);
}

/// The pure decision, split out so the truth table is testable WITHOUT setting
/// the real global or the real env (both of which are banned in tests).
///
/// A money switch has exactly one "on" spelling — `"1"` — deliberately stricter
/// than the vault's truthy parse, so there is no ambiguity about `"true"`,
/// `"yes"`, or `"0"`.
pub(crate) fn decide(process_flag: Option<bool>, env_value: Option<&str>) -> bool {
    if let Some(v) = process_flag {
        return v;
    }
    matches!(env_value.map(str::trim), Some("1"))
}

/// True when this process may make a billable request.
pub fn allowed() -> bool {
    decide(
        ALLOWED.get().copied(),
        std::env::var("HICKEYFIELD_ALLOW_SPEND").ok().as_deref(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decide_is_off_by_default() {
        // A plain `cargo test` process: no entrypoint call, no env var.
        assert!(!decide(None, None));
    }

    #[test]
    fn only_exactly_1_env_allows() {
        assert!(decide(None, Some("1")));
        assert!(decide(None, Some(" 1 ")), "trimmed");
        assert!(!decide(None, Some("0")));
        assert!(!decide(None, Some("true")));
        assert!(!decide(None, Some("yes")));
        assert!(!decide(None, Some("")));
    }

    #[test]
    fn the_process_flag_allows() {
        // The state `enable()` produces in the packaged app — proven without
        // touching the real global (which would poison sibling tests).
        assert!(decide(Some(true), None));
        // A flag set to false would win over any env, but nothing sets it.
        assert!(!decide(Some(false), Some("1")));
    }
}
