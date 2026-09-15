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

---
## Amendment implement: F1.2 + F3

Built the reopened amendment scope **exactly** — F1.2 (AC6, bound the resumable
timeout state) + F3 (AC7, clear the stale timeout advisory) — per the appended
`design.md` "# Amendment" section, honouring the two task deferrals (F1b
`resume_all` drain rewrite and F4 UI/`retry_job` are **not** built; see "Deferred"
below). AC8 (S1 AC1–AC4 unchanged, gate green) is proven at the bottom.

### Files changed (with the AC each serves)

| File | AC | What changed |
|---|---|---|
| `crates/hickeyfield-core/src/engine.rs` | **AC6** | `JobSet` gains two `#[serde(default)]` fields — `poll_cycles: u32`, `stalled: bool` (NOT a new `JobStatus` variant, per design). New `JobSet::is_stalled()`. The **single** `JobStore::unfinished()` default now filters `!is_settled() && !is_stalled()` (was `!is_settled()`) — the one behavioural lever; every store inherits it. Test helper `job()` and 1 engine test literal updated. |
| `crates/hickeyfield-core/src/engine.rs` | **AC7** | New `pub const TIMEOUT_ADVISORY_PREFIX: &str = "no result yet"` (shortened from S1's `"no result yet after"` so it matches both the running and stalled notes). In `apply_poll`, guarded by `poll.status.phase() == Phase::Completed`, `retain`-drops **only** advisories with that prefix — `fail_reason` and all other advisories untouched. |
| `src-tauri/src/runner.rs` | **AC6** | `mark_timed_out` now takes `now: i64`, always increments `poll_cycles` (removed the old dedup early-return), and sets `stalled = poll_cycles >= MAX_POLL_CYCLES (5) OR age > age_cap(budget)`. New `const MAX_POLL_CYCLES: u32 = 5` and `fn age_cap(budget) = (budget × 5).clamp(HOSTED.cap = 1h, 7d)`. Advisory de-duplicated via retain-then-push, using the shared prefix so F3 can clear it; swaps to the "reopen this job" wording once stalled. Always returns `true` (poll_cycles always changed) so the caller's upsert always persists the count. Module doc updated to describe the bound. |
| `src-tauri/src/runner.rs` | **AC6/AC7** | Timeout branch in `poll_until_terminal` passes `now_secs()` and unconditionally upserts. `use` adds `TIMEOUT_ADVISORY_PREFIX`. |
| `src-tauri/src/store.rs` | **AC6** | Migration **v6** adds `poll_cycles INTEGER NOT NULL DEFAULT 0` + `stalled INTEGER NOT NULL DEFAULT 0`. `row_to_job` reads both (`.max(0) as u32` / `!= 0`, `.unwrap_or` for pre-v6 rows). `upsert` writes both in the INSERT (`?21,?22`) and `ON CONFLICT` clauses (`j.stalled as i64`). Test helper `job()` updated. |
| `src-tauri/src/commands.rs` | **AC6 (ripple)** | `submit_job`'s `JobSet` literal adds `poll_cycles: 0, stalled: false` (no `Default` impl to lean on). **No DTO or command added** — `list_jobs` returns the raw `JobSet`, so the fields cross the bridge via serde automatically; that surfacing is the deferred F4, untouched here. |
| `crates/hickeyfield-core/src/recipe.rs` | **AC6 (ripple)** | Two test `JobSet` literals add the two fields (compile fix). |
| `src-tauri/src/app.rs` | **AC6 (ripple)** | One test `JobSet` literal adds the two fields (compile fix). |

`resume_all` (`runner.rs`) is left **exactly as S1 shipped** (per-job `watch`
loop) — F1b deferred. No `ui/` file touched, no command added — F4 deferred.

### Tests added / changed

**Added (9):**
- engine `a_stalled_job_leaves_the_auto_resume_set` (AC6) — a `stalled=true`
  `InProgress` row is absent from `unfinished()` while a non-stalled one is
  present, and the stalled row is neither terminal nor settled.
- engine `completing_clears_the_timeout_advisory` (AC7) — the timeout advisory is
  gone after an `apply_poll` to `Completed`.
- engine `completion_keeps_real_advisories` (AC7) — only the timeout advisory is
  dropped; a genuine "audio not supported" note and `fail_reason` survive.
- engine `a_failed_job_keeps_its_timeout_advisory` (AC7) — the `Completed`-only
  guard: a `Failed` transition keeps the advisory.
- runner `the_fifth_timeout_stalls_the_job` (AC6) — 5 `mark_timed_out` calls
  (age held at 0); `stalled` flips on the fifth only, status stays `InProgress`,
  `fail_reason` `None`, exactly one advisory whose text says "reopen".
- runner `age_stalls_a_job_before_the_cycle_cap` (AC6) — one call with
  `now > created_at + age_cap` stalls at `poll_cycles == 1`.
- runner `a_timeout_within_the_cap_still_stays_resumable` (AC6/AC2) — one timeout
  leaves `stalled=false` and the row in `unfinished()`.
- runner `a_stalled_job_is_dropped_from_the_resume_set` (AC6) — at the cap the row
  leaves `unfinished()` but `status`/`request_id` survive for a manual retry.
- store `stall_fields_round_trip` (AC6) — `poll_cycles=3, stalled=true` round-trip,
  full-struct equality, and an update persists a new count.

**Changed:**
- runner `mark_timed_out_pauses_without_failing` (AC2) — updated to the new
  `now` arg and the always-`true`/always-increment contract; still asserts status
  unchanged, `fail_reason` `None`, non-terminal, advisory not stacked.
- runner `a_late_result_is_reattached_after_a_timeout` (AC2 + **AC7 end-to-end**) —
  passes `now`; now also asserts the timeout advisory is present before the late
  completion and **gone after**, proving F3 through the real poll loop.
- runner `a_timed_out_job_stays_in_unfinished` (AC2) — sets `j.created_at =
  now_secs()`. **Why:** the real timeout path stamps `mark_timed_out(..,
  now_secs())`; a helper job left at `created_at = 0` (epoch 1970) is genuinely
  older than any `age_cap` and now (correctly) stalls immediately, so this AC2
  "within-cap stays resumable" test needed a realistic creation time. (This was
  caught by a first failing run — see below — and is a test-fixture fix, not a
  production behaviour change.)
- store `a_v1_database_upgrades_without_losing_rows` (AC6) — DROP list extended
  with `poll_cycles` + `stalled`, and it asserts a pre-v6 row loads as
  `poll_cycles=0, stalled=false`.

### AC2 preservation — the critical invariant

A late provider result arriving **before** the cap still auto-downloads/reattaches:
within the cap `mark_timed_out` sets `poll_cycles=1, stalled=false`, the job stays
non-terminal + `!is_settled()` + `!is_stalled()`, so it remains in `unfinished()`
and `resume_all` re-attaches it; the fresh poll to `Completed` runs `apply_poll` +
`download_outputs` unchanged. Proven by `a_late_result_is_reattached_after_a_timeout`
and `a_timeout_within_the_cap_still_stays_resumable`. Only never-settling jobs
past the cap go `stalled`.

### Commands run and observed output

Environment: `source "$HOME/.cargo/env"`; `rustup component add clippy` (already
present). Node/pnpm not used (Rust core+shell only). `tauri build` not run (per task).

**Keychain note (honest, load-bearing for how the workspace run was done).** In
this sandbox the pre-existing `commands::tests` hang indefinitely: `configured_providers`
→ `vault::is_configured` → `keyring get_password` blocks on a macOS Keychain GUI
"allow access" prompt that cannot be answered headlessly (the unsigned test binary
is not in the ACL of items the installed `/Applications/Hickeyfield.app` created).
This is **unrelated to F1.2/F3** — I changed no vault/commands provider code. To
get the full-workspace number I used the project's **own** documented dev escape
hatch (`vault.rs:24-36`, `hickeyfield_<PROVIDER>_KEY` env overrides, checked *before*
the keychain in `get()`), which short-circuits every keychain read. My slice's own
suites (core, `runner::`, `store::`) touch no keychain and passed **without** any
override.

`cargo test --workspace` (with the env overrides above to bypass the keychain hang):
```
     Running unittests src/lib.rs (…/hickeyfield_core-8df486f4919bb2f9)
test result: ok. 635 passed; 0 failed; 9 ignored; 0 measured; 0 filtered out; finished in 0.19s
     Running unittests src/lib.rs (…/hickeyfield_tauri_lib-37ca18e46b11d4ae)
test result: ok. 101 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 15.09s
     Running unittests src/main.rs (…/hickeyfield_tauri-78b13688ca92de02)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
   Doc-tests hickeyfield_core
test result: ok. 0 passed; 0 failed; 0 ignored; …
   Doc-tests hickeyfield_tauri_lib
test result: ok. 0 passed; 0 failed; 0 ignored; …
```
**Total: 635 + 101 = 736 passed, 0 failed, 9 ignored.** Baseline was 727; +9 =
4 engine + 4 runner + 1 store new tests. **736 >= 727.** ✅ (The 9 ignored are the
same pre-existing core tests, untouched.)

Independently, without any env override (so, no keychain path at all):
- `cargo test -p hickeyfield-core` → `635 passed; 0 failed; 9 ignored`.
- `cargo test -p hickeyfield-tauri --lib "runner::"` → `22 passed; 0 failed` (see AC list below).
- `cargo test -p hickeyfield-tauri --lib "store::"` → `12 passed; 0 failed`.

`cargo fmt --all --check` → **exit 0** (first pass reformatted two of my new blocks;
re-check clean).

`cargo clippy --workspace --all-targets -- -D warnings`:
```
    Checking hickeyfield-core v0.1.0 (…/crates/hickeyfield-core)
    Checking hickeyfield-tauri v0.1.0 (…/src-tauri)
    Finished `dev` profile … in 4.16s
```
**exit 0**, zero warnings.

`./scripts/lint-provenance.py`:
```
CDN hostnames        PASS  no third-party media hosts referenced
Copy provenance      PASS  no shipped string matches the reference corpus (80015 shingles indexed)
```
**exit 0.**

### AC8 — S1 AC1–AC4 still pass unchanged (named)

All original S1 runner tests are present and **ok** in the `runner::` run above:
- **AC1** — `a_4k_job_gets_a_bigger_budget_than_a_short_one`,
  `an_unknown_provider_and_unsizable_settings_fall_back_to_base`.
- **AC2** — `mark_timed_out_pauses_without_failing`,
  `a_timed_out_job_stays_in_unfinished`, `a_late_result_is_reattached_after_a_timeout`.
- **AC3** — `provider_cap_bounds_simultaneous_poll_loops`, `local_runs_one_at_a_time`.
- **AC4** — `a_completed_job_is_not_re_polled`, `resuming_a_completed_job_keeps_its_success`.

The AC3 Condvar limiter and its wiring are byte-for-byte unchanged; F1b (the drain
rewrite that would have touched it) is deferred.

### Deferred (logged as follow-ups, NOT built — per task)

- **F1b** — `resume_all` bounded per-provider drain-worker rewrite. `resume_all` is
  left as S1 shipped. The F1.2 cap is the load-bearing fix (it shrinks `unfinished()`
  so the never-settling set cannot grow without bound); the drain rewrite is a
  separate future slice.
- **F4** — surfacing `stalled`/`poll_cycles` in the UI DTO and a `retry_job` command
  to manually recover a stalled job. `ui/`, `commands.rs` DTOs, and command
  registration were **not** touched. Until F4, a stalled job stops auto-retry (its
  output is provider-expired by the age cap anyway) and remains recoverable via the
  existing Rerun (`delete_job` + re-`submit_job`) path.

### Known limitations / honesty

1. **Full `cargo test --workspace` required provider env overrides** to bypass the
   pre-existing macOS keychain GUI-prompt hang in `commands::tests` (detailed above).
   Not a regression from this amendment; the slice's own core/runner/store suites
   pass with no override. A verifier on an interactive machine (or one where the
   login keychain grants the test binary access, as the S1 baseline run had) will
   get the same 736 with a plain `cargo test --workspace`.
2. **No wall-clock test of the production age cap.** As in S1, there is no injectable
   clock; `age_cap` behaviour is proven by passing `now` explicitly to the pure
   `mark_timed_out` (`age_stalls_a_job_before_the_cycle_cap`) and by the fixed
   `a_timed_out_job_stays_in_unfinished` (real loop, fresh `created_at`). The exact
   multi-day wall-clock coupling is not slept through.
3. **`cargo deny check` not run** (cargo-deny not installed; this amendment adds no
   crate dependency, so the dependency graph is unchanged — same standing as the S1
   claim above).
4. **The `stalled` field crosses the bridge but is not yet rendered** — that is the
   deferred F4. The value is correct and persisted; nothing in the UI reads it yet.

## Amendment implement: F5 (honest age-cap + advisory)

### What & why
F1.2 shipped an age backstop of `age_cap(budget) = (budget * MAX_POLL_CYCLES).clamp(HOSTED.cap /*1h*/, 7d)`.
Because the largest per-session budget is the local 6h cap, `budget × 5` tops out at 30h, so
the 7-day ceiling **never bound** and the effective age horizon was only ~1–5h. Age is measured
from `created_at` (which includes app-closed time), so a slow-but-alive **hosted** job could be
marked `stalled` — and dropped from `unfinished()` — while its paid output was still retrievable
inside the provider's retention window. The stalled advisory also told the user to "reopen this
job", an affordance that does not exist. AC9 fixes both, minimal surface, no new deps.

### Files changed
- **`src-tauri/src/runner.rs`**
  - **AC9a** — Removed `fn age_cap(budget)` and its `budget*5`/clamp scheme. Added a fixed
    `const STALL_AGE_BACKSTOP: Duration = Duration::from_secs(7 * 24 * 3600)` (7 days, matched to
    the provider result-retention horizon). `mark_timed_out` now stalls when
    `poll_cycles >= MAX_POLL_CYCLES` (unchanged active-polling bound) **OR**
    `age > STALL_AGE_BACKSTOP.as_secs()`. `age`/`now` still computed from `created_at` as before.
    Updated the module doc comment, the `mark_timed_out` doc comment, and added a doc comment on
    the constant explaining why it is deliberately *not* scaled by budget (the exact defect).
  - **AC9b** — Reworded the stalled advisory. Was:
    `"{PREFIX} after N attempts — Hickeyfield stopped retrying automatically; reopen this job to try again"`.
    Now: `"{PREFIX} — the provider hasn't returned a result after N attempts; it may still finish. Use Rerun to try again."`
    Still starts with `TIMEOUT_ADVISORY_PREFIX` (`"no result yet"`, verified in
    `crates/hickeyfield-core/src/engine.rs`), so F3's clear-on-`Completed` `retain(!starts_with(PREFIX))`
    in `apply_poll` still removes it. No longer references the non-existent "reopen".
  - **Tests**: adjusted `age_stalls_a_job_before_the_cycle_cap` → `age_past_seven_days_stalls_a_job_before_the_cycle_cap`
    (uses `STALL_AGE_BACKSTOP` instead of the removed `age_cap`); adjusted `the_fifth_timeout_stalls_the_job`
    to assert the advisory does **not** contain "reopen", **does** contain "Rerun", and still starts
    with the prefix. Added `a_six_hour_old_job_is_not_stalled_by_age` (the F5 regression: a 6h-old
    job that would have stalled under the old ~1–5h cap now stays resumable) and
    `the_stalled_advisory_points_at_rerun_and_still_clears_on_completion` (AC9b end-to-end: stalled
    wording + F3 clears it on a late `Completed`).

No change to `engine.rs` was needed: `TIMEOUT_ADVISORY_PREFIX` and the clear-on-`Completed`
`retain` in `apply_poll` already do exactly what AC9b relies on; the new advisory keeps the prefix.

### AC → evidence
- **AC9a** — 7-day backstop replaces `budget*5`; active bound `poll_cycles >= 5` intact.
  - `a_six_hour_old_job_is_not_stalled_by_age` — `... ok` (a 6h job is NOT stalled; would have been under the old cap).
  - `age_past_seven_days_stalls_a_job_before_the_cycle_cap` — `... ok` (age > 7d IS stalled at poll_cycles=1).
  - `the_fifth_timeout_stalls_the_job` — `... ok` (poll_cycles >= 5 IS stalled).
- **AC9b** — advisory drops "reopen", points at Rerun, keeps the prefix, F3 still clears it.
  - `the_stalled_advisory_points_at_rerun_and_still_clears_on_completion` — `... ok`.
  - `the_fifth_timeout_stalls_the_job` (extended asserts) — `... ok`.
  - Core F3 tests `completing_clears_the_timeout_advisory`, `completion_keeps_real_advisories`,
    `a_failed_job_keeps_its_timeout_advisory` — still `... ok`.
- **AC9c** — AC1–AC4, AC6–AC8 and the full gate still green (see below).

### Verify (observed output)
Env-key overrides exported first to avoid the macOS Keychain GUI-prompt hang in `commands::tests`
(pre-existing; unrelated to this amendment):
```
export hickeyfield_FAL_KEY=x hickeyfield_HIGGSFIELD_KEY=x hickeyfield_GOOGLE_KEY=x \
       hickeyfield_OPENAI_KEY=x hickeyfield_XAI_KEY=x hickeyfield_BFL_KEY=x \
       hickeyfield_RECRAFT_KEY=x hickeyfield_VAIG_KEY=x
```

`cargo test --workspace` — 0 failed, **738 passed** (up from 736: +2 new tests):
```
test result: ok. 635 passed; 0 failed; 9 ignored; 0 measured; 0 filtered out; finished in 0.19s
test result: ok. 103 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 15.07s
test result: ok. 0 passed; 0 failed; ... (main.rs)
test result: ok. 0 passed; 0 failed; ... (Doc-tests hickeyfield_core)
test result: ok. 0 passed; 0 failed; ... (Doc-tests hickeyfield_tauri_lib)
TOTAL passed=738 failed=0
```
New/adjusted runner tests (from `cargo test -p hickeyfield-tauri --lib runner::`):
```
test runner::tests::a_six_hour_old_job_is_not_stalled_by_age ... ok
test runner::tests::age_past_seven_days_stalls_a_job_before_the_cycle_cap ... ok
test runner::tests::the_fifth_timeout_stalls_the_job ... ok
test runner::tests::the_stalled_advisory_points_at_rerun_and_still_clears_on_completion ... ok
test result: ok. 24 passed; 0 failed; 0 ignored; 0 measured; 79 filtered out; finished in 15.04s
```
Gate commands (exit codes captured):
```
cargo fmt --all --check         -> fmt_exit=0
cargo clippy --workspace --all-targets -- -D warnings -> clippy_exit=0  (Finished, no warnings)
./scripts/lint-provenance.py    -> provenance_exit=0  (CDN hostnames PASS, Copy provenance PASS)
```

### Known limitations / gaps (not fixed here — by scope)
1. **`commands::tests` still needs the dummy env-key overrides** on this machine (the pre-existing
   macOS Keychain GUI-prompt hang). Not a regression from this amendment.
2. **No injectable wall-clock.** As in S1, `STALL_AGE_BACKSTOP` behaviour is proven by passing
   `now` explicitly into the pure `mark_timed_out` (the 6h and 7d tests), not by sleeping through
   multi-day wall-clock time.
3. **`cargo deny check` not run** (cargo-deny not installed; this amendment adds no crate
   dependency, so the graph is unchanged).
4. **F4 and F1b remain deferred** (per task): the `stalled` field still crosses the bridge but no
   UI renders it, and there is no `retry_job` command yet. Recovery today is via the existing
   Rerun (which the reworded advisory now correctly points at). F4, when built, must reset
   `created_at` as well as `poll_cycles`, or the 7-day age branch could re-stall a retried job.
