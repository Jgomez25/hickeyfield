# S1-job-engine-reliability — REVIEW (evidence)

Read-only production-standards review of the sole diff in this slice:
`src-tauri/src/runner.rs` (`git diff --stat` confirms 1 file, +529/-24; no other
tracked file, no `Cargo.toml`/`Cargo.lock`, changed). Reviewed against slice.md
AC1–AC5, design.md, implement.md. Cross-checked every wired API against
`crates/hickeyfield-core/src/engine.rs`, `provider.rs`, and
`src-tauri/src/commands.rs`.

## Verification run (not modifying code)
- `cargo test -p hickeyfield-tauri --lib runner::` → 18 passed, 0 failed (8 pre-existing + 10 new).
- `cargo clippy -p hickeyfield-tauri --lib --all-targets -- -D warnings` → exit 0, no warnings.

## What I checked and found clean

- **AC1 `timeout_budget` (runner.rs:217-230)** — Correct. `route_id.split(':').next()`
  → `ProviderId::from_slug` → `features().timeout`, falling back to
  `TimeoutPolicy::HOSTED` for an unroutable prefix. Settings deserialized to the
  same camelCase `SettingsDto` (`commands.rs:553-555`) that `submit_job` persists
  (`commands.rs:763`), then `Billable::from`. `Value::Null` (the value written on
  a serialize failure) and legacy/unsizable rows fail `from_value` → `None` →
  `TimeoutPolicy::timeout(None)` = `base` (600s), never zero — core clamps
  non-finite/absurd `work_units` (`engine.rs:358-372`), so a corrupt `settings`
  blob cannot panic `mul_f64` on the runner thread. Budget is sized once off the
  loop (runner.rs:116) before `job` is moved into the task.
- **AC2 `mark_timed_out` (runner.rs:239-251)** — Correct non-terminal semantics.
  Never sets `status`/`fail_reason`; only pushes a de-duplicated advisory (stable
  `"no result yet after"` prefix) and bumps `updated_at`. Status stays non-terminal
  ⇒ `is_settled()` false (`engine.rs:105-109`) ⇒ `unfinished()` still returns the
  row (`engine.rs:167-173`) ⇒ `resume_all` re-attaches it. The timeout branch
  (runner.rs:437-445) returns the job without a terminal status. Verified real
  provider/network/store failures are still surfaced as `Failed`: missing client
  (400-410), exhausted retries "lost contact" (467-476), permanent error (479-486)
  are unchanged — the timeout-as-pause change does not swallow any of them.
- **AC3 `ConcurrencyLimiter`/`Slot`/`Permit` (runner.rs:263-329)** — Correct.
  `acquire` holds the `avail` mutex and loops `while *avail == 0 { wait }`, so a
  spurious wakeup or a missed notify cannot let it through with zero permits, and
  it decrements only a value verified positive under the lock ⇒ no underflow.
  `Permit::Drop` (281-292) increments then `notify_one` under the lock ⇒ no
  lost-wakeup; releases on every exit path — normal return, timeout, missing
  client, and panic unwind — because it is bound to a real name `_permit`
  (runner.rs:359, not a bare `_` that would drop immediately) scoped to the poll
  block, so it is held for the whole poll and released before the uncapped
  `download_outputs`. Both `acquire` and `Drop` recover a poisoned lock
  (`unwrap_or_else(|e| e.into_inner())`), so a slot cannot leak and stall the
  queue. Slots are built for all 9 `ProviderId::ALL` variants (provider.rs:34-44),
  each `permits() >= 1` (provider.rs:200-213), so `slots[&provider]` indexing
  cannot panic and no slot starts empty (no deadlock-on-zero).
- **No deadlock / lock ordering** — `acquire` blocks on the Condvar holding only
  its own `avail` mutex; no store/`watching`/`abandoned` lock is held across the
  wait. `run_watched` releases the `abandoned` lock (352-354) before acquiring a
  permit, and `store.upsert` locks are transient inside the poll loop, never held
  across `acquire`. `resume_all` (86-93) calls `watch` per job; each task takes its
  own permit, extras park, all complete (proven by the AC3 tests). No starvation
  for a finite job set.
- **AC4 `provider_poll_needed` (runner.rs:259-261) + gate (356-376)** — Correct and
  strictly safe: `!is_terminal()` means any terminal row (only Completed-but-
  undownloaded reaches here via `unfinished()`) skips the poll entirely and goes
  straight to `download_outputs`, which never flips status back (147-150, 190-204).
  This closes the "expired status URL 404 overwrites a paid success with Failed"
  path. `download_outputs` re-fetches only outputs missing `local_path`.
- **Tests are real regression tests, not tautologies.** AC1 asserts strict
  inequalities computed through the real core (`big > small` and `big > 600s`;
  fallback `== base` and `> 0`). AC2 drives the actual `elapsed() > budget` branch
  with a 1ms budget and asserts `unfinished()` still returns the row, plus a late
  `Completed` re-poll preserving `actual_usd`. AC3 asserts peak concurrency `<= 2`
  (fal) / `== 1` (local) and `done == N` (nothing dropped) under real thread
  contention with a 3–5ms in-slot sleep to force overlap. AC4's
  `resuming_a_completed_job_keeps_its_success` is non-vacuous: the client is
  scripted `Err(Permanent("HTTP 404"))`, so an ungated re-poll would flip the row
  to Failed; the test asserts `calls == 0` (poll skipped) and Completed + URL +
  `actual_usd` survive, with `local_path` pre-set so the download stays offline.
- **Dead code / AI-code tells** — `DEFAULT_TIMEOUT` import removed; no runtime
  reference remains in runner.rs (only a test-section comment header). No invented
  APIs — every call (`features().timeout`, `max_concurrent.permits()`,
  `TimeoutPolicy::timeout/HOSTED/base`, `is_terminal`/`is_settled`/`unfinished`,
  `Billable::from`, `ProviderId::ALL/from_slug`) exists and is public. Module doc
  (1-19) updated to match the new limiter + per-job budget behavior; no comment
  contradicts the code on the correctness-critical paths.

## Findings (all minor; none on the paid-output-loss critical path)

- **MINOR — stale timeout advisory survives a late completion.**
  `runner.rs:239-251` records the advisory "no result yet after N min … will keep
  trying when the app next launches"; `apply_poll` (`engine.rs:385-409`) never
  touches `job.advisories`. So when a timed-out job later re-polls to `Completed`
  and its bytes are saved, the row still carries a user-facing advisory that
  contradicts its own state (result is present, nothing is "still running"). Not
  data loss and not on the critical path — status is correctly `Completed` — but
  it is a misleading, self-contradicting user-facing string.
  Fix (follow-up, not applied): clear advisories matching the `NOTE` prefix when a
  job reaches terminal `Completed` (e.g. in `run_watched`/download path), or filter
  them out in the UI for settled jobs. Recommend a backlog item.

- **MINOR — comment overstates the abandoned-while-parked guarantee (runner.rs:350-351).**
  The comment says a job "deleted while it was parked in the queue never consumes a
  slot another job is waiting for." The `abandoned` check is *before* `acquire`, so
  it protects a job that has not yet reached `acquire`; a job already parked inside
  `limiter.acquire()` has passed the check and, if deleted while parked, will still
  take a permit when woken — then return immediately via the `is_abandoned()` check
  at the top of `poll_until_terminal` (434-436). Behavior is safe (RAII release, no
  leak/deadlock, permit held for microseconds), but the comment is imprecise about
  which window it covers. Fix: reword the comment. No code change needed.

- **MINOR (accepted design tradeoff) — parked tasks occupy blocking-pool threads.**
  Each queued job is a `spawn_blocking` task that parks on the Condvar while waiting
  for a permit (runner.rs:118-132, 319-328). A very large queue (hundreds) would
  pressure tokio's default ~512-thread blocking pool. Explicitly acknowledged and
  defaulted in design.md (Open question #2) as acceptable for a desktop app; no
  action required for this slice, noted for awareness.

- **MINOR (test coverage) — the permit-around-poll wiring in `run_watched` is
  verified by inspection, not by a concurrent end-to-end test.** The limiter
  primitive is well-tested in isolation (AC3) and the wiring is a trivial
  `.map(|p| limiter.acquire(p))` scoping the poll (359-372); no test spawns two
  live `run_watched` calls for the same provider and asserts their polls serialize.
  Acceptable gap given the difficulty of timing `spawn_blocking`; noted, not
  blocking.

## Cosmetic (no action)
- `budget.as_secs() / 60` (runner.rs:247) reads "0 min" for sub-minute budgets,
  which only occurs in the 1ms-budget unit test; the production floor is `base`
  (600s = 10 min), so the advisory always reads a sensible minute count in the app.

## Conclusion
The slice delivers AC1–AC4 with correct, defensive logic and non-vacuous tests;
error propagation is intact and the timeout-as-pause change is narrowly scoped.
The Condvar limiter is free of the missed-notify, underflow, permit-leak, and
poison failure modes flagged as the weakest part. The only findings are minor and
off the critical path — chiefly a stale advisory after late completion, worth a
follow-up backlog item but not a merge blocker. No blockers, no majors.

VERDICT: PASS
