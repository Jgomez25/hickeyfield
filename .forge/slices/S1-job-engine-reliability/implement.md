# S1-job-engine-reliability — IMPLEMENT claim

Built exactly per `design.md`. **All production and test changes are confined to
one file: `src-tauri/src/runner.rs`** (529 insertions, 24 deletions). No core
API changed; only already-public APIs were wired in (`ProviderId::features()`,
`Concurrency::permits()`, `TimeoutPolicy::timeout`, `JobSet::is_settled` /
`JobStore::unfinished`, `JobSet::is_terminal`).

## Files changed

### `src-tauri/src/runner.rs` — the only file touched

| Change | Serves | What it does |
|---|---|---|
| Module doc (top of file) corrected | AC1/AC3 | Replaces the stale "one task per job … no limiter / flat timeout" narrative with the per-provider limiter + per-job budget it now implements. |
| Imports: dropped `DEFAULT_TIMEOUT`, added `TimeoutPolicy`, `Billable`, `ProviderId`, `HashMap`, `Condvar`, `Duration` | AC1/AC3 | The new machinery's dependencies. `DEFAULT_TIMEOUT` is no longer referenced by the runner. |
| New pure fn `timeout_budget(route_id, settings) -> Duration` | **AC1** | Maps the persisted `settings` blob through the route's provider `TimeoutPolicy`. Unknown route prefix → `TimeoutPolicy::HOSTED`; a settings blob that will not deserialize into `SettingsDto` → `work = None` → `policy.base` (never zero). |
| New pure fn `mark_timed_out(job, budget) -> bool` | **AC2** | On timeout, pushes a de-duplicated advisory and bumps `updated_at`; **does not** touch `job.status` or set `fail_reason`, so the job stays non-terminal / unsettled / resumable. |
| New pure fn `provider_poll_needed(job) -> bool` = `!job.is_terminal()` | **AC4** | Predicate gating the poll: a terminal (Completed-but-undownloaded) row skips polling. |
| New `Slot` / `Permit` (RAII, `Drop` releases) / `ConcurrencyLimiter` (Condvar-based counting semaphore keyed by `ProviderId`, one slot per `ProviderId::ALL`, sized from `features().max_concurrent.permits()`) | **AC3** | Blocking per-provider cap. `acquire` waits under the lock while `avail == 0` (no underflow, no missed-notify); `Permit::drop` increments + `notify_one` on **every** exit path (return / timeout / missing-client / panic unwind), recovering from a poisoned lock so a slot can never leak. |
| New fn `run_watched(job, budget, store, clients, on_update, abandoned, library, limiter)` | AC3/AC4 | Extracted body of the `watch` closure, off the tokio runtime so it is unit-testable. Checks `abandoned` **before** acquiring a permit; if `provider_poll_needed`, acquires one per-provider permit scoped to a block around `poll_until_terminal` (released before the un-capped `download_outputs`); else skips the poll and goes straight to `download_outputs`. |
| `Runner` gains `limiter: Arc<ConcurrencyLimiter>`, built in `Runner::new` | AC3 | One limiter shared across all watched jobs. |
| `watch` now computes `budget` once and delegates to `run_watched` inside `spawn_blocking` | AC1/AC3/AC4 | The `watching` insert still happens synchronously before spawn, so parked jobs are remembered and never dropped. |
| `poll_until_terminal` gains a `budget: Duration` param; its timeout branch now calls `mark_timed_out` and returns a **non-terminal** job | AC1/AC2 | Replaces `started.elapsed() > DEFAULT_TIMEOUT { status = Failed; … }` with `started.elapsed() > budget { if mark_timed_out(...) { upsert; on_update } return job }`. |
| Tests: `run()` helper + the 2 direct `poll_until_terminal` calls updated to pass a generous `TimeoutPolicy::HOSTED.timeout(None)` budget | AC5 | Existing tests unaffected (scripted terminal results settle in ms). |
| **10 new tests added** (AC1×2, AC2×3, AC3×2, AC4×2 … see below) | **AC5** | Reuse existing doubles `MemStore`, `ScriptedClient` (its `calls` counter proves "never polled"), and helpers `job()`, `ok()`, `run()`. |

### New tests (all in the existing `#[cfg(test)] mod tests` in runner.rs)
- **AC1** `a_4k_job_gets_a_bigger_budget_than_a_short_one` — 4K 10s clip budget (3300s) > 720p 10s clip (900s) and > old flat 600s.
- **AC1** `an_unknown_provider_and_unsizable_settings_fall_back_to_base` — `timeout_budget("nope:m", Null)` == `HOSTED.timeout(None)` == `HOSTED.base`, and `> 0`; a known provider with an unsizable row degrades identically.
- **AC2** `mark_timed_out_pauses_without_failing` — pure: status stays `InProgress`, `fail_reason` still `None`, `!is_terminal()`, one advisory, returns `true` then `false` (de-dup).
- **AC2** `a_timed_out_job_stays_in_unfinished` — real `elapsed() > budget` branch via a 1 ms budget + always-`InProgress` client; returned job is non-terminal, `!is_settled()`, and `store.unfinished()` still contains it.
- **AC2** `a_late_result_is_reattached_after_a_timeout` — a job left `InProgress` **with** the timeout advisory, re-polled with a generous budget against a client scripted `Completed`+outputs+`actual_usd`; final status `Completed`, one result, `actual_usd = Some(0.75)`.
- **AC3** `provider_cap_bounds_simultaneous_poll_loops` — 6 threads, `ProviderId::Fal`; peak concurrent `<= 2`, `done == 6` (none dropped).
- **AC3** `local_runs_one_at_a_time` — 4 threads, `ProviderId::Local`; peak concurrent `== 1`, `done == 4`.
- **AC4** `a_completed_job_is_not_re_polled` — pure: `!provider_poll_needed(Completed)` and `provider_poll_needed(InProgress)`.
- **AC4** `resuming_a_completed_job_keeps_its_success` — `run_watched` on a `Completed` job whose output has a `local_path` set (download no-ops offline) with a client scripted `Err(Permanent("HTTP 404 …"))`; asserts the client `calls == 0` (never polled) **and** the stored row is still `Completed` with URL + `actual_usd` intact.

## Deviation from the design (noted, not hidden)

The design's AC1 fallback test text suggested `timeout_budget("nope:m", &json!({}))
== HOSTED.timeout(None)`. That literal assertion is **false**: an empty `{}`
deserializes cleanly into a default `SettingsDto` (every field is `Option` or has
a serde default), yielding a default `Billable` with `work_units == 1.0` →
`600 + 30 = 630s`, not the `base` `600s`. The `timeout_budget` **function body** is
implemented exactly as the design's code block specifies (deserialize;
`.ok()` → `None` on failure → `policy.base`). The only change is the **test
input**: I assert the fallback with `serde_json::Value::Null`, which genuinely
fails to deserialize into a struct → `work = None` → exactly `policy.base`. This
is strictly more faithful to weak-point #3 ("legacy/unsizable rows degrade to
base, never zero"), because `Value::Null` is the real value `submit_job` writes
when settings serialization fails (`commands.rs:763`,
`.unwrap_or(serde_json::Value::Null)`). "Never zero" is asserted directly
(`fallback > Duration::ZERO`).

## Weak-points addressed (per design "verifier, start here")

1. **Condvar limiter correctness** — `acquire` holds the lock and loops
   `while *avail == 0 { wait }` (no spurious-wakeup or missed-notify escape, no
   underflow since it only decrements a verified-positive count). `Permit::drop`
   releases on every path incl. panic unwind, and recovers from a poisoned lock
   (`unwrap_or_else(|e| e.into_inner())`) so a slot is never lost. Proven by
   `provider_cap_bounds_simultaneous_poll_loops` (cap held under contention, all
   6 complete) and `local_runs_one_at_a_time` (exactly 1).
2. **AC4 fixture actually proves the guard** —
   `resuming_a_completed_job_keeps_its_success` asserts the `ScriptedClient.calls`
   counter is **0** (the poll was skipped, not merely benign) **and** the stored
   status/URL/`actual_usd` survive; the pre-set `local_path` keeps
   `download_outputs` offline so nothing else could mask a re-poll.
3. **SettingsDto round-trip** — confirmed `submit_job` writes the camelCase
   `SettingsDto` shape (`commands.rs:763`) and that a non-map `Value` degrades to
   `work = None → policy.base` (tested with `Value::Null`).

## Commands run and observed output

Environment: `source "$HOME/.cargo/env"` (rustc/cargo 1.98); installed
`rustfmt` + `clippy` via `rustup component add rustfmt clippy` (both were missing).

### Runner unit tests — `cargo test -p hickeyfield-tauri --lib runner::`
(The package is `hickeyfield-tauri`; the task's `-p hickeyfield-tauri-lib` is the
*lib name* `hickeyfield_tauri_lib`, not a package selector — corrected to `-p
hickeyfield-tauri --lib`.)

```
running 18 tests
test runner::tests::a_completed_job_is_not_re_polled ... ok
test runner::tests::a_late_result_is_reattached_after_a_timeout ... ok
test runner::tests::a_missing_credential_is_explained_not_swallowed ... ok
test runner::tests::a_live_job_is_still_written ... ok
test runner::tests::an_abandoned_job_is_never_written_back ... ok
test runner::tests::a_permanent_error_fails_immediately_with_the_reason ... ok
test runner::tests::an_unknown_provider_and_unsizable_settings_fall_back_to_base ... ok
test runner::tests::a_4k_job_gets_a_bigger_budget_than_a_short_one ... ok
test runner::tests::mark_timed_out_pauses_without_failing ... ok
test runner::tests::nsfw_is_terminal_and_keeps_its_own_status ... ok
test runner::tests::a_timed_out_job_stays_in_unfinished ... ok
test runner::tests::runs_to_completion_and_persists_each_transition ... ok
test runner::tests::intermediate_stages_do_not_terminate_the_loop ... ok
test runner::tests::local_runs_one_at_a_time ... ok
test runner::tests::provider_cap_bounds_simultaneous_poll_loops ... ok
test runner::tests::resuming_a_completed_job_keeps_its_success ... ok
test runner::tests::a_transient_error_is_retried_rather_than_failing_the_job ... ok
test runner::tests::exhausted_retries_say_contact_was_lost_not_that_it_failed ... ok

test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 78 filtered out; finished in 15.02s
```
(8 pre-existing runner tests + 10 new = 18, all passing.)

### Full workspace — `cargo test --workspace`

```
     Running unittests src/lib.rs (…/hickeyfield_core-8df486f4919bb2f9)
test result: ok. 631 passed; 0 failed; 9 ignored; 0 measured; 0 filtered out; finished in 0.23s
     Running unittests src/lib.rs (…/hickeyfield_tauri_lib-37ca18e46b11d4ae)
test result: ok. 96 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 15.08s
     Running unittests src/main.rs (…/hickeyfield_tauri-78b13688ca92de02)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
   Doc-tests hickeyfield_core
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
   Doc-tests hickeyfield_tauri_lib
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

**Total: 631 + 96 = 727 passed, 0 failed, 9 ignored** (the 9 ignored are
pre-existing core tests, untouched by this slice). **727 >= 718.** ✅

### Formatting — `cargo fmt --all --check`
Exit 0, no diff. (First run reformatted a few of the new assertions; re-check is
clean.)

### Lint — `cargo clippy --workspace --all-targets -- -D warnings`

```
    Checking hickeyfield-core v0.1.0 (…/crates/hickeyfield-core)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2m 57s
```
Exit 0, zero warnings.

## Known gaps / limitations (honest)

1. **`cargo deny check` — not run here.** `cargo-deny` is **not installed** on
   this shell and installing new global tooling is out of scope for this slice's
   file surface. This slice adds **no new crate dependency** (the limiter is
   `std::sync::Condvar`), so the dependency graph is unchanged and the gate
   should be unaffected. The verifier should run it in the TEST phase.
2. **`./scripts/lint-provenance.py` — pre-existing failure, unrelated to my
   change.** It fails only on `.forge/backlog.md:67` (a FORGE planning doc:
   "3 shingles match reference copy"), a file this slice never touched. The
   "CDN hostnames" check PASSes. No production or test source I authored trips
   it. This is a planning-doc prose issue for whoever owns the backlog, not a
   code defect in S1.
3. **App build+launch (`tauri build`) — not run.** Per the task note this is a
   Rust-only slice and the app-launch regression is verified in a later phase;
   `tauri build` was deliberately skipped (slow).
4. **No wall-clock test of a real 600s+ budget → branch coupling.** As the design
   flags under "Risk": there is no injectable clock. AC1 tests the budget *value*
   purely (`timeout_budget`) and AC2 tests the timeout *behaviour* with a 1 ms
   budget through the real `started.elapsed() > budget` branch. The exact
   production coupling for a multi-minute deadline is not wall-clock-exercised —
   judged acceptable per the design; a time-injection seam remains a Non-goal.
5. **Same-session re-poll after timeout is not implemented** (design Non-goal /
   Open-question #1 default): a timed-out job resumes on the next `resume_all`,
   not immediately, to avoid a hot loop against a deliberately slow provider.

---
## ORCHESTRATOR ADDENDUM (post-implement, out-of-band gate housekeeping)

The IMPLEMENT claim above is accurate: the only **slice code** change is
`src-tauri/src/runner.rs` (confirmed by `git status --short`: `M src-tauri/src/runner.rs`
is the sole tracked modification; `.forge/` is untracked scaffolding).

Two gate items were handled by the orchestrator, NOT by editing slice code:

1. **Provenance lint (§A gate) — resolved as toolkit housekeeping, separate from slice code.**
   The lint scans all `.md`, including FORGE evidence docs, which legitimately quote the
   strings under discussion — so evidence files perpetually false-positive on the reference
   corpus. Since `.forge/` is committed scaffolding that never ships in the `.app`, `.forge`
   was added to the lint's `SKIP_DIRS` (same class as the existing `provenance`/`vendor`
   skips). This is a FORGE-toolkit change, NOT part of S1's production code, and was
   **explicitly approved by the repo owner** (see `.forge/PROVENANCE-SCOPE.md`). Shipped-source
   scanning is unchanged. `./scripts/lint-provenance.py` exits 0. Supersedes "Known gaps #2".
   So the working tree carries two changes: `src-tauri/src/runner.rs` (this slice's production
   + test code) and `scripts/lint-provenance.py` (toolkit housekeeping, owner-approved).

2. **cargo-deny advisories (§A gate).** `cargo deny check` exits nonzero on baseline advisories
   (RUSTSEC-2026-0258 h2 0.4.15, RUSTSEC-2026-0285 rustls/tokio-rustls). S1 changes **no**
   `Cargo.toml`/`Cargo.lock` (git confirms), so these are pre-existing and orthogonal to S1 —
   NOT an S1 regression. Tracked as a new backlog item (dependency-advisory bump); flagged for
   the SECURITY phase and the RELEASE human gate.
