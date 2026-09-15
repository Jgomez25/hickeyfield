# S1-job-engine-reliability — ADVERSARIAL VERIFICATION (AMENDMENT F5 / AC9, full re-run)

This run SUPERSEDES the prior on-disk verdict. It verifies the F5 amendment (AC9a honest
7-day age backstop, AC9b Rerun advisory wording, AC9c no regression) and re-confirms the
still-in-scope prior criteria AC1–AC4 and AC6–AC8. I did not trust any pasted output; every
number below was re-captured on this machine.

Environment: `source "$HOME/.cargo/env"` → rustc 1.98.1 / cargo 1.98.1; `clippy 0.1.98`
already installed. The full-workspace run used the documented DUMMY dev env-key overrides
(`hickeyfield_<PROVIDER>_KEY=x`) to bypass the pre-existing macOS Keychain GUI hang in
`commands::tests` (`vault::is_configured` → keyring). The env was set per-subshell only and
does not persist; the tree was left exactly as found (no source edits, no temp probes).

Repo state: branch `forge/s1-job-engine-reliability`. HEAD is the S1 commit
`909cdea S1: job-engine reliability`. The working tree holds the F1.2/F3 AND F5 amendments
together, UNCOMMITTED — there is no commit boundary isolating F5.

## Implementer's claim (verbatim)

From the "## Amendment implement: F5 (honest age-cap + advisory)" section of `implement.md`:

> ### Files changed
> - **`src-tauri/src/runner.rs`**
>   - **AC9a** — Removed `fn age_cap(budget)` and its `budget*5`/clamp scheme. Added a fixed
>     `const STALL_AGE_BACKSTOP: Duration = Duration::from_secs(7 * 24 * 3600)` (7 days, matched to
>     the provider result-retention horizon). `mark_timed_out` now stalls when
>     `poll_cycles >= MAX_POLL_CYCLES` (unchanged active-polling bound) **OR**
>     `age > STALL_AGE_BACKSTOP.as_secs()`. `age`/`now` still computed from `created_at` as before.
> ...
>   - **AC9b** — Reworded the stalled advisory. Was:
>     `"{PREFIX} after N attempts — Hickeyfield stopped retrying automatically; reopen this job to try again"`.
>     Now: `"{PREFIX} — the provider hasn't returned a result after N attempts; it may still finish. Use Rerun to try again."`
>     Still starts with `TIMEOUT_ADVISORY_PREFIX` (`"no result yet"` ...), so F3's clear-on-`Completed` ... still removes it. No longer references the non-existent "reopen".

And the headline regression number:

> `cargo test --workspace` — 0 failed, **738 passed** (up from 736: +2 new tests)

---

## Commands (real output, captured on this machine)

### 0. Surface / deferral scope (`git`)

`git diff --stat` vs HEAD (=S1 commit) shows the CUMULATIVE amendment (F1.2/F3 + F5),
not F5 alone — the amendments are uncommitted with no boundary between them:

```
 crates/hickeyfield-core/src/engine.rs | 150 +++++++++++++++-
 crates/hickeyfield-core/src/recipe.rs |   4 +
 src-tauri/src/app.rs                  |   2 +
 src-tauri/src/commands.rs             |   3 +
 src-tauri/src/runner.rs               | 329 ++++++++++++++++++++++++++++++----
 src-tauri/src/store.rs                |  64 ++++++-
```

The engine/recipe/app/commands/store deltas are all attributable to the PRIOR F1.2/F3
amendment (poll_cycles/stalled fields, migration v6, `TIMEOUT_ADVISORY_PREFIX`), which
`implement.md` documents. The F5-specific code (STALL_AGE_BACKSTOP, reworded advisory,
removed `age_cap`) is confined to `src-tauri/src/runner.rs` — verified by grep (below);
`implement.md` states "No change to engine.rs was needed" for F5, consistent with the code.

Deferrals confirmed NOT built:
```
$ grep -rn "retry_job" src-tauri/ crates/ ui/     -> no matches (exit 1)   [F4 command not built]
$ git diff HEAD --name-only | grep -i "ui/"       -> no matches (exit 1)   [F4 UI untouched]
```
`resume_all` (runner.rs:91) is the S1 per-job `watch` loop verbatim; the diff touches it
only in comments/test strings, never the body → F1b (drain rewrite) NOT built.

### 1. AC9a — code read (age_cap gone, fixed 7-day backstop, OR semantics)

```
$ grep -rn "age_cap\|budget \* 5\|clamp" src-tauri/src/runner.rs
runner.rs:251:/// CYCLES` (clamped to the hosted cap and 7 days) maxed at 30h — the largest   [comment only]
runner.rs:1046:        // AC9a — the F5 defect regression. The old age_cap was clamp(budget*5,  [test comment only]
```
No `fn age_cap`, no live `clamp` — the old scheme is GONE (only prose remains).

`mark_timed_out` (runner.rs:276-300), verbatim core:
```rust
job.poll_cycles = job.poll_cycles.saturating_add(1);
let age = now.saturating_sub(job.created_at).max(0) as u64;
let aged_out = age > STALL_AGE_BACKSTOP.as_secs();
job.stalled = job.poll_cycles >= MAX_POLL_CYCLES || aged_out;   // OR, not AND
```
`const MAX_POLL_CYCLES: u32 = 5;` (runner.rs:242);
`const STALL_AGE_BACKSTOP: Duration = Duration::from_secs(7 * 24 * 3600);` (runner.rs:257 = 604800s).
Types (engine.rs): `created_at: i64`, `now_secs() -> i64`, `poll_cycles: u32`, `stalled: bool`.
`age` is still derived from `created_at`.

### 2. AC9b — advisory wording + F3 clear (code read)

Stalled branch (runner.rs:287-291): `"{TIMEOUT_ADVISORY_PREFIX} — the provider hasn't
returned a result after {N} attempts; it may still finish. Use Rerun to try again."`
— no "reopen", contains "Rerun", begins with the prefix constant.
`pub const TIMEOUT_ADVISORY_PREFIX: &str = "no result yet";` (engine.rs:285).
`apply_poll` (engine.rs:430-438) on `Phase::Completed` does
`job.advisories.retain(|a| !a.starts_with(TIMEOUT_ADVISORY_PREFIX));` — leaves `fail_reason`
and other advisories intact. Because the pushed string literally begins with the prefix
constant, `starts_with` is guaranteed true → completion always clears it.

### 3. Named tests (all `... ok`)

Runner (`cargo test -p hickeyfield-tauri --lib runner::`) — F5 + AC1–AC4 + AC6:
```
test runner::tests::a_six_hour_old_job_is_not_stalled_by_age ... ok                        (AC9a)
test runner::tests::age_past_seven_days_stalls_a_job_before_the_cycle_cap ... ok           (AC9a)
test runner::tests::the_fifth_timeout_stalls_the_job ... ok                                (AC9a/AC9b)
test runner::tests::the_stalled_advisory_points_at_rerun_and_still_clears_on_completion ... ok (AC9b)
test runner::tests::a_4k_job_gets_a_bigger_budget_than_a_short_one ... ok                   (AC1)
test runner::tests::an_unknown_provider_and_unsizable_settings_fall_back_to_base ... ok     (AC1)
test runner::tests::mark_timed_out_pauses_without_failing ... ok                            (AC2)
test runner::tests::a_timed_out_job_stays_in_unfinished ... ok                              (AC2)
test runner::tests::a_late_result_is_reattached_after_a_timeout ... ok                      (AC2+F3 e2e)
test runner::tests::provider_cap_bounds_simultaneous_poll_loops ... ok                      (AC3)
test runner::tests::local_runs_one_at_a_time ... ok                                         (AC3)
test runner::tests::a_completed_job_is_not_re_polled ... ok                                 (AC4)
test runner::tests::resuming_a_completed_job_keeps_its_success ... ok                       (AC4)
test runner::tests::a_timeout_within_the_cap_still_stays_resumable ... ok                   (AC6)
test runner::tests::a_stalled_job_is_dropped_from_the_resume_set ... ok                     (AC6)
test result: ok. 24 passed; 0 failed; 0 ignored; 0 measured; 79 filtered out; finished in 15.04s
```

Engine (AC6/AC7):
```
test engine::tests::a_stalled_job_leaves_the_auto_resume_set ... ok                         (AC6)
test engine::tests::completing_clears_the_timeout_advisory ... ok                           (AC7)
test engine::tests::completion_keeps_real_advisories ... ok                                 (AC7)
test engine::tests::a_failed_job_keeps_its_timeout_advisory ... ok                          (AC7)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 640 filtered out
```

Store (migration v6 / round-trip):
```
test store::tests::stall_fields_round_trip ... ok                                           (AC6 v6)
test store::tests::a_v1_database_upgrades_without_losing_rows ... ok                         (v6 migrate)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 101 filtered out
```

### 4. Regression gate

`cargo test --workspace` (dummy env overrides), **exit 0**:
```
hickeyfield_core       : test result: ok. 635 passed; 0 failed; 9 ignored
hickeyfield_tauri_lib  : test result: ok. 103 passed; 0 failed; 0 ignored
hickeyfield_tauri(main): test result: ok.   0 passed; 0 failed
Doc-tests (both)       : test result: ok.   0 passed; 0 failed
TOTAL: 738 passed, 0 failed, 9 ignored     (matches claim)
```
```
$ cargo fmt --all --check                                  -> FMT_EXIT=0 (no diff)
$ cargo clippy --workspace --all-targets -- -D warnings     -> CLIPPY_EXIT=0, "warning:" count = 0
$ ./scripts/lint-provenance.py                              -> PROV_EXIT=0
    CDN hostnames    PASS  no third-party media hosts referenced
    Copy provenance  PASS  no shipped string matches the reference corpus (80015 shingles indexed)
```
`cargo deny check advisories` FAILS on exactly the two DISCLOSED baseline advisories and
NO others: `RUSTSEC-2026-0258` (h2) and `RUSTSEC-2026-0285` (rustls/tokio-rustls). No
`Cargo.toml`/`Cargo.lock` change in the working tree (`git diff HEAD --name-only | grep Cargo` → none).
Pre-existing, orthogonal to F5 → noted, not blocking (per task and implement.md addendum).

---

## Break attempts

| # | Attack | Result |
|---|--------|--------|
| 1 | **Integer overflow in the 7d const** `Duration::from_secs(7 * 24 * 3600)` | 7*24*3600 = 604800, computed as u64 (from_secs arg). Fits trivially. Const compiled; `age_past_seven_days...` uses `STALL_AGE_BACKSTOP.as_secs() as i64 + 10` and stalls → value confirmed 604800. NO overflow. |
| 2 | **Underflow / clock skew in age** (`created_at` in the future, epoch-0 rows) | `now.saturating_sub(created_at).max(0) as u64`: saturating_sub avoids i64 overflow; `.max(0)` clamps a negative (future created_at) to 0 → age 0 → not aged out. Cast to u64 safe. NO underflow. |
| 3 | **Is stall AND, not OR?** (task's central edge probe) | Code: `poll_cycles >= 5 || aged_out`. Proven by TWO tests that would FAIL under AND: `the_fifth_timeout_stalls_the_job` stalls with age=0 (cycle branch alone); `age_past_seven_days...` stalls at poll_cycles=1 (age branch alone). Both pass → definitively OR. |
| 4 | **The exact pre-F5 wrongly-stalled case: ~6h job, <5 cycles** | `a_six_hour_old_job_is_not_stalled_by_age`: created_at=0, now=6*3600, one timeout → poll_cycles=1, `!stalled`, advisory has no "reopen". PASS — the F5 defect (old ~1–5h cap) is fixed. |
| 5 | **Fresh job (age≈0, 0 cycles) ever stalled?** | `job()` helper defaults `poll_cycles: 0, stalled: false`; a never-timed-out job never calls `mark_timed_out`. First timeout → poll_cycles=1, age~0 → stalled=false (`mark_timed_out_pauses_without_failing`, `a_timeout_within_the_cap...`). Never stalled. |
| 6 | **Prefix silently broken → F3 stops clearing** | Pushed advisory literally starts with the `TIMEOUT_ADVISORY_PREFIX` constant; `apply_poll` retains `!starts_with(PREFIX)`. `the_stalled_advisory_points_at_rerun_and_still_clears_on_completion` asserts the STALLED note (not just the pause note) is GONE after a real `apply_poll`→Completed. PASS — clear proven by removal, not by wording alone. |
| 7 | **Advisory wording regression** | `the_fifth_timeout_stalls_the_job` + AC9b test assert `!contains("reopen")`, `contains("Rerun")`, `starts_with(PREFIX)`. All pass. |
| 8 | **Genuine late result on an early cycle still reattaches (AC2 e2e)** | `a_late_result_is_reattached_after_a_timeout`: one timeout (poll_cycles=1, within both bounds, stays in `unfinished()`), then poll→Completed with outputs + actual_usd → final Completed, 1 result, `actual_usd=Some(0.75)`, timeout advisory cleared. PASS. |
| 9 | **F1b/F4 sneaked in?** | `resume_all` body byte-identical to S1; no `retry_job`; no `ui/` in diff. Deferrals respected. |
| 10 | **Idempotency / re-run of the whole suite** | Full workspace re-run → identical 738/0/9, exit 0. fmt/clippy/provenance re-run clean. |

### Finding (non-blocking ambiguity, disclosed)
The task's step 1 assumes `git diff --stat` can show "F5 changed ONLY runner.rs". It cannot:
F1.2/F3 and F5 are uncommitted together, so the stat shows all six files. I resolved this by
(a) grepping the F5-specific tokens (STALL_AGE_BACKSTOP/Rerun/age_cap) — all in runner.rs, and
(b) confirming the other five files' deltas are the documented F1.2/F3 work. This does not
contradict the F5 claim, and AC9c (tests + gate) holds regardless. Recorded for honesty.

---

## Per-AC verdict

| AC | Requirement | Evidence | Verdict |
|----|-------------|----------|---------|
| AC9a | Fixed 7-day backstop replaces budget*5/clamp; stall = cycles>=5 OR age>7d; age from created_at; 6h not stalled, >=5 cycles / >7d stalled; fresh never stalled; no over/underflow | age_cap gone; STALL_AGE_BACKSTOP=604800s; OR in code; attacks #1–5; 3 named tests ok | PASS |
| AC9b | Advisory drops "reopen", points at Rerun, keeps prefix so F3 clears on Completed | wording read + prefix const + apply_poll retain; attacks #6,#7; 2 named tests ok | PASS |
| AC9c | AC1–AC4, AC6–AC8 still pass; gate green | 24 runner + 4 engine + 2 store named ok; 738/0/9; fmt/clippy/prov exit 0 | PASS |
| AC1 | budget inequality (4K > short) | `a_4k_job_gets_a_bigger_budget...`, `...fall_back_to_base` ok | PASS |
| AC2 | late-result reattach after timeout, stays resumable | 3 named ok; attack #8 | PASS |
| AC3 | per-provider concurrency cap, none dropped | `provider_cap_bounds...`, `local_runs_one_at_a_time` ok | PASS |
| AC4 | completed job not re-polled; success survives resume | `a_completed_job_is_not_re_polled`, `resuming_a_completed_job_keeps_its_success` ok | PASS |
| AC6 | stalled excluded from unfinished; within-cap resumable | engine + runner tests ok; unfinished filter `!settled && !stalled` | PASS |
| AC7 | Completed clears only timeout advisory; fail_reason survives | 4 engine tests ok | PASS |
| AC8 | S1 preserved + gate green | see AC1–AC4 + regression | PASS |
| Deferrals | F1b + F4 NOT built | resume_all unchanged; no retry_job; no ui/ | RESPECTED |

Regression gate: `cargo test --workspace` 738 passed / 0 failed / 9 ignored (exit 0);
`cargo fmt --all --check` exit 0; `cargo clippy --workspace --all-targets -- -D warnings`
exit 0, 0 warnings; `./scripts/lint-provenance.py` exit 0. `cargo deny check` fails on the
two disclosed baseline advisories only, no manifest change — noted, not blocking.

AC9a, AC9b, AC9c all hold; prior ACs intact; deferrals respected; regression green. I could
not break the slice.

VERDICT: PASS
