# S1-job-engine-reliability — DESIGN

Upstream read: `.forge/objective.md` (success criteria §B), this slice's
`slice.md` (AC1–AC5). No prior slice exists (S1 is first), and the repo keeps no
`CLAUDE.md`, glossary, or ADR set — so this design reuses the vocabulary already
established in the code (`Billable`, `TimeoutPolicy`, `Features`, `Concurrency`,
`is_settled`/`unfinished`, `Phase`, "reattach") rather than coining new terms.

## Scope

All production changes are localized to **`src-tauri/src/runner.rs`**. The
`TimeoutPolicy`/`Concurrency`/`Features` machinery already exists and is
unit-tested in the core; this slice is the runtime wiring the core comment at
`crates/hickeyfield-core/src/provider.rs:111-122` says is missing.

- **`src-tauri/src/runner.rs`** — new pure helpers, a per-provider concurrency
  limiter, and edits to `watch`/`poll_until_terminal`. Plus the module doc
  (`runner.rs:1-9`) which currently claims "One task per running job … with no
  limiter" and must be corrected.
- **`crates/hickeyfield-core/src/engine.rs`** — **N/A (no code change).**
  Everything needed is already public: `TimeoutPolicy::timeout` (`engine.rs:358`),
  `TimeoutPolicy::HOSTED` (`engine.rs:290`), `JobSet::is_settled` (`engine.rs:105`),
  `JobStore::unfinished` default (`engine.rs:167`), `apply_poll` (`engine.rs:385`).
- **`crates/hickeyfield-core/src/provider.rs`** — **N/A (no code change).**
  `ProviderId::features()` (`provider.rs:123`), `Features.timeout`/
  `Features.max_concurrent` (`provider.rs:168-176`), and `Concurrency::permits()`
  (`provider.rs:203`) are already `pub`. The unknown-provider fallback is handled
  in the runner (see AC1), so no `Features::default` needs adding. The stale
  "no limiter" narrative in the `features()` doc SHOULD be corrected in the
  DOCUMENT step, but that is prose, not an API change.
- **`src-tauri/src/commands.rs`** — reused read-only: `SettingsDto`
  (`commands.rs:553`, already `pub`) and `impl From<&SettingsDto> for Billable`
  (`commands.rs:613`). No change.

Non-goals for this slice are listed at the end.

## Data flow after this slice

`submit_job` → `store.upsert` → `runner.watch(job)` (`commands.rs:767-774`),
and on relaunch `resume_all` → `store.unfinished()` → `watch` per job
(`runner.rs:71-78`). Inside each spawned task the new flow is:

1. If the job is already tombstoned (`abandoned`), return (unchanged from today).
2. Compute the per-job budget once: `timeout_budget(&job.route_id, &job.settings)`.
3. If `provider_poll_needed(&job)` (i.e. **not** already terminal): acquire one
   per-provider concurrency permit, run `poll_until_terminal(job, budget, …)`,
   release the permit, then `download_outputs`.
4. Else (a Completed-phase job returned by `unfinished()` because its bytes are
   not local yet): **skip the provider poll entirely** and go straight to
   `download_outputs` (AC4).

The provider poll — the only rate-limited call — is what the permit gates; the
CDN download is not gated by the provider's in-flight cap.

## AC1 — per-job budget replaces the flat `DEFAULT_TIMEOUT`

Today `runner.rs:238` compares `started.elapsed() > DEFAULT_TIMEOUT` (a flat
600 s from `engine.rs:249`), imported at `runner.rs:13`.

**MUST** replace it with a budget derived from the job's provider and requested
work. Add a pure function (no clock, no I/O) so AC1 is testable without the loop:

```rust
/// Patience budget for one poll session: the provider's TimeoutPolicy applied
/// to the job's requested work. Pure — no Instant, no store — so a test can
/// assert the budget value directly (see AC1 test).
fn timeout_budget(route_id: &str, settings: &serde_json::Value) -> Duration {
    // route_id is `provider:slug` (see client_for, app.rs:34-36).
    let policy = route_id
        .split(':').next()
        .and_then(ProviderId::from_slug)
        .map(|p| p.features().timeout)   // provider.rs:123 -> Features.timeout
        .unwrap_or(TimeoutPolicy::HOSTED); // fallback: known-good hosted budget
    // settings is the serialized SettingsDto (camelCase) written at
    // commands.rs:763; From<&SettingsDto> is the same conversion submit/pricing
    // already uses (commands.rs:613, 728), so deadline and bill see one view.
    let work = serde_json::from_value::<crate::commands::SettingsDto>(settings.clone())
        .ok()
        .map(|s| Billable::from(&s));
    policy.timeout(work.as_ref()) // engine.rs:358; None -> policy.base, never 0
}
```

- **Billable source**: the persisted `job.settings` blob (deserialized to
  `SettingsDto`, then `Billable::from`). This is the same path `submit_job` and
  `estimate_cost` use, so patience and price are derived from one view of the job
  (the property `engine.rs:640` pins).
- **Fallback when unsizable**: `serde_json::from_value` failing or yielding an
  empty `SettingsDto` gives `work = None` (or a default `Billable`), and
  `TimeoutPolicy::timeout(None)` returns `policy.base` — never zero
  (`engine.rs:356-357`). A zero budget would fail every job on its first tick.
- **Fallback when provider/route unknown**: `ProviderId::from_slug` returns
  `None` → `TimeoutPolicy::HOSTED`. (In practice a job with an unroutable prefix
  never reaches the loop: `poll_until_terminal` returns early at the missing-
  client branch, `runner.rs:201-211`. The fallback is defensive.)

`poll_until_terminal` **MUST** take the budget as a parameter (see the shared
signature below) and the branch at `runner.rs:238-248` becomes:

```rust
if started.elapsed() > budget {
    if mark_timed_out(&mut job, budget) { let _ = store.upsert(&job); on_update(&job); }
    return job; // NOT terminal — see AC2
}
```

Threading `budget` as a parameter is deliberate: it is the seam that makes both
the budget *value* (AC1, via `timeout_budget`) and the timeout *behaviour* (AC2,
via a tiny budget in a test) checkable without a fake clock. See Risk below.

**Worked example.** `fal:…seedance` job, `settings = {"duration":10.0,
"resolution":"4k","aspect":"16:9"}`: `SettingsDto`→`Billable` gives
`seconds=10, width≈3840, height=2160` → `work_units = 10 × 9 = 90`
(`engine.rs:324`, 4K is 9× 720p per `engine.rs:645`) →
`HOSTED.timeout = 600 + 30×90 = 3300 s`, under the 3600 s cap. The same job at
`"720p"` gives `work_units = 10` → `600 + 300 = 900 s`. `3300 s > 900 s > 600 s`
(old flat), exactly the assertion AC1 requires.

## AC2 — a timed-out job stays resumable

Today the timeout branch (`runner.rs:239`) sets `JobStatus::Failed`. `Failed` is
terminal (`job.rs:143-148`), and a terminal, non-Completed job **is** settled
(`engine.rs:105-109`), so `unfinished()` (`engine.rs:167-173`) drops it forever —
the exact "paid output silently lost" bug the objective names, and the reason
`engine.rs:270-275` says the timeout must not mark terminal.

**MUST NOT** set a terminal status on timeout. Instead leave the last polled
status (`Queued`/`InProgress`/…) untouched and record *why polling paused*, via a
pure transition:

```rust
/// Timeout is a pause, not a death. Leaves job.status non-terminal so
/// is_settled() stays false (engine.rs:105) and unfinished() (engine.rs:167)
/// still returns the row, so resume_all re-attaches it on next launch and a
/// late provider result can still be downloaded. Records an advisory, never a
/// fail_reason. Returns whether it changed anything (to gate the upsert).
fn mark_timed_out(job: &mut JobSet, budget: Duration) -> bool {
    const NOTE: &str = "no result yet after"; // stable prefix, for de-dup
    if job.advisories.iter().any(|a| a.starts_with(NOTE)) { return false; }
    job.advisories.push(format!(
        "{NOTE} {} min of polling; still running at the provider — will keep \
         trying when the app next launches", budget.as_secs() / 60));
    job.updated_at = now_secs();
    true
}
```

Because `job.status` stays non-terminal, `is_settled()` is false, the row stays
in `unfinished()`, and `resume_all` (`runner.rs:71-78`) re-attaches it on the
next launch. The `request_id`/`endpoint` needed to re-poll or reattach were
persisted at submit (`commands.rs:767-773`) and by every prior transition via
`apply_poll` (`engine.rs:407` + `store.upsert`), so nothing extra needs
persisting for a late result to remain downloadable. Recording an advisory
(distinct from `fail_reason` by design, `engine.rs:52-59`) keeps the UI honest
about the pause instead of showing a silent stall.

- The pause applies to **one poll session**. Within the current session the loop
  stops; re-polling happens on the next `resume_all`. Re-enqueuing immediately in
  the same session is a Non-goal (would risk a hot loop against a provider that
  is deliberately slow). Recorded default in Open questions.
- **MAY** de-duplicate the advisory (as above) so repeated timeout/relaunch
  cycles do not stack identical notes.

**Worked example.** A job polled to `InProgress`, then its budget elapses:
`mark_timed_out` adds the advisory, status remains `InProgress`,
`is_settled()==false`. On relaunch `unfinished()` returns it; the fresh
`poll_until_terminal` polls, the provider now answers `Completed` with an output
URL, `apply_poll` records it, `download_outputs` saves the bytes. The paid
result is recovered.

## AC3 — per-provider concurrency cap

Today `watch` (`runner.rs:81-113`) spawns one unbounded `spawn_blocking` poll
loop per job — the amplified-throttling bug bounded by
`provider::tests::fal_is_capped_at_the_limit_fal_itself_enforces`
(`provider.rs:280-291`).

**MUST** cap simultaneous provider-poll loops per provider at
`ProviderId::features().max_concurrent.permits()` (`provider.rs:203`) — `2` for
fal (`Concurrency::Provider(2)`, `provider.rs:132`), `1` for `Local`, `4` for the
rest. Design a small blocking counting semaphore keyed by `ProviderId`, kept in
the runner (no new crate dependency — the poll loops are blocking
`spawn_blocking` tasks, so a `std::sync::Condvar` gate is the right primitive; a
tokio async `Semaphore` would need `.await` we do not have here):

```rust
struct Slot { avail: Mutex<u32>, free: Condvar }
struct Permit { slot: Arc<Slot> }              // RAII: releases on Drop
impl Drop for Permit {
    fn drop(&mut self) {
        *self.slot.avail.lock().unwrap() += 1;
        self.slot.free.notify_one();
    }
}
struct ConcurrencyLimiter { slots: HashMap<ProviderId, Arc<Slot>> }
impl ConcurrencyLimiter {
    fn new() -> Self { /* one Slot per ProviderId::ALL, avail = p.features()
                          .max_concurrent.permits() (>=1, provider.rs:203) */ }
    fn acquire(&self, p: ProviderId) -> Permit { /* lock; while avail==0 wait on
                          free; avail-=1; return Permit */ }
}
```

- Runner gains a field `limiter: Arc<ConcurrencyLimiter>`, built in
  `Runner::new` (`runner.rs:52-67`) from `ProviderId::ALL` (`provider.rs:34`).
- **Where to acquire/release**: inside the spawned task, `watch` **MUST** acquire
  the permit for the job's provider immediately before calling
  `poll_until_terminal` and release it immediately after (RAII drop at end of a
  `{ … }` block), *before* `download_outputs`. Gating only the provider poll —
  not the CDN download — is correct: fal's "2 in flight" limits generation
  requests, not result downloads.
- **Jobs beyond N are queued, not dropped**: the `watching` set insert
  (`runner.rs:83-86`) still happens synchronously in `watch` before the spawn, so
  every queued job is de-duplicated and remembered; the spawned task then parks
  on `acquire` until a slot frees. Nothing is dropped.
- **`resume_all` respects the cap with no deadlock**: it calls `watch` per
  unfinished job (`runner.rs:74-76`); each spawned task acquires its own permit
  and extras park. There is no deadlock because (a) `permits()` is guaranteed
  `>= 1` (`provider.rs:203`, and `every_provider_permits_at_least_one_job`
  pins it, `provider.rs:268-277`); (b) a running loop never waits on another
  job's permit; and (c) permits release on **every** exit path — terminal,
  timeout (AC2), abandoned, or missing-client — via `Permit`'s `Drop`.
- **Abandoned-while-queued**: `watch` **SHOULD** check `abandoned` *before*
  `acquire`, so a job deleted while parked never consumes a slot.

**Worked example.** Six fal jobs queued at once: all six enter the `watching`
set and spawn; `acquire(Fal)` lets exactly two into `poll_until_terminal`; the
other four park on `free`. As each of the two finishes (or times out per AC2,
releasing its permit), one parked job proceeds. Peak simultaneous poll loops = 2;
completed = 6.

## AC4 — a Completed-but-undownloaded job is not re-polled on relaunch

`unfinished()` returns two kinds of rows (`engine.rs:105-109`): non-terminal
jobs, **and** Completed-phase jobs whose bytes are not yet local (terminal but
not settled — pinned by `engine.rs:584-613` and `store.rs:377-401`). Today both
go through `poll_until_terminal`, whose first action is `client.poll(…)`
(`runner.rs:250`). If the provider's status URL has expired, that poll returns
`Err(Permanent("HTTP 404 …"))` (`engine.rs:198`), which falls to the final
`Err(e)` arm at `runner.rs:282-288` and overwrites the Completed success with
`JobStatus::Failed` — destroying a paid, recoverable result and its URLs.

**MUST** branch before polling so a Completed-phase job never reaches
`runner.rs:250`. Add a pure predicate and gate the poll in `watch`:

```rust
/// A terminal job is one the provider has finished with; re-polling it can
/// only overwrite what it already told us. Only Completed-but-undownloaded
/// rows reach here via unfinished() (engine.rs:105); those go straight to the
/// download/reattach retry instead of a poll.
fn provider_poll_needed(job: &JobSet) -> bool { !job.is_terminal() } // job.rs:143
```

In the spawned task: `if provider_poll_needed(&job) { <acquire permit; poll> }
else { <skip poll> }`, then `download_outputs` runs for both branches.
`download_outputs` already no-ops unless `phase()==Completed` with outputs
present (`runner.rs:157`), retries only outputs missing `local_path`
(`runner.rs:170`), and by contract never flips status back
(`runner.rs:147-150`). So a resumed Completed job keeps its terminal-success
record and result URLs and merely re-attempts the save. Using `!is_terminal()`
(rather than only excluding Completed) is strictly safer — no terminal job is
ever re-polled — and Failed/Canceled rows never appear here anyway because
`unfinished()` excludes them.

**Worked example.** Relaunch with a Completed job whose one output has
`local_path=None`: `provider_poll_needed` is false → the loop is skipped, so the
404 path at `runner.rs:250/282` cannot fire → status stays `Completed`, the URL
survives, and `download_outputs` retries the fetch.

## Shared signature change

```rust
fn poll_until_terminal(
    mut job: JobSet,
    budget: Duration,                 // NEW (AC1/AC2)
    store: Arc<dyn JobStore>,
    clients: ClientFactory,
    on_update: OnUpdate,
    abandoned: Arc<Mutex<HashSet<String>>>,
) -> JobSet
```

To make the AC4 branch and the permit lifetime testable off the tokio runtime,
extract the body of the `watch` closure into a private
`fn run_watched(job, budget, store, clients, on_update, abandoned, library,
limiter)`; `watch` becomes `spawn_blocking(move || run_watched(…))`. This keeps
every change inside `runner.rs`.

## AC5 — tests to add and regression commands

Test doubles already in `runner.rs` `#[cfg(test)]` (reuse, do not reinvent):
`MemStore` (`runner.rs:303-316`, a `JobStore` over a `HashMap`; note it inherits
the default `unfinished()` so `store.unfinished()` works in tests),
`ScriptedClient` (`runner.rs:319-354`, scripted `poll` with an `AtomicUsize`
call counter — reuse the counter to assert "poll never called"), and helpers
`job(id)` (`runner.rs:356`), `ok(status)` (`runner.rs:381`), `run(script)`
(`runner.rs:390`). `run` and the three direct `poll_until_terminal` calls
(`runner.rs:396, 480, 511`) **MUST** be updated to pass the new `budget`
argument (use a generous `TimeoutPolicy::HOSTED.timeout(None)` so existing tests
are unaffected — they finish in ms via scripted terminal results).

Add to the `runner.rs` tests module:

- **AC1** `a_4k_job_gets_a_bigger_budget_than_a_short_one` — call
  `timeout_budget("fal:m", &json!({"duration":10.0,"resolution":"4k",
  "aspect":"16:9"}))` vs `"720p"`; assert `4k > 720p` and `4k > Duration::from_secs(600)`.
  Plus `an_unknown_provider_falls_back_to_hosted_base` — `timeout_budget("nope:m",
  &json!({}))` == `TimeoutPolicy::HOSTED.timeout(None)`.
- **AC2** `mark_timed_out_pauses_without_failing` — pure: assert status
  unchanged, `fail_reason` still `None`, `!job.is_terminal()`, one advisory added,
  returns `true` then `false` (de-dup).
  `a_timed_out_job_stays_in_unfinished` — run `poll_until_terminal` with
  `budget = Duration::from_millis(1)` and an always-`InProgress` `ScriptedClient`
  (interval 1 ms); assert the returned job is non-terminal, `!is_settled()`, and
  `store.unfinished()` still contains it. (Exercises the real
  `started.elapsed() > budget` branch, fast and deterministic.)
  `a_late_result_is_reattached_after_a_timeout` — start from a job left
  `InProgress` with the timeout advisory in the store, run `poll_until_terminal`
  with a generous budget and a client scripted to return `Completed` + outputs +
  `actual_usd`; assert final status `Completed`, one result recorded, `actual_usd`
  captured — the late result survived the timeout.
- **AC3** `provider_cap_bounds_simultaneous_poll_loops` — build
  `ConcurrencyLimiter::new()`, spawn 6 `std::thread`s each doing
  `{ let _p = limiter.acquire(ProviderId::Fal); inflight.fetch_add; max.fetch_max;
  sleep(2ms); inflight.fetch_sub; } done.fetch_add`; join all; assert
  `max <= 2` (fal's `Concurrency::Provider(2)`) and `done == 6` (nothing dropped).
  `local_runs_one_at_a_time` — same shape, `ProviderId::Local`, assert `max == 1`.
- **AC4** `a_completed_job_is_not_re_polled` — pure:
  `assert!(!provider_poll_needed(&completed))` and
  `assert!(provider_poll_needed(&in_progress))`.
  `resuming_a_completed_job_keeps_its_success` — call `run_watched` with a
  `Completed` job (its output given a `local_path` already set so
  `download_outputs` no-ops offline), a `ScriptedClient` scripted to
  `Err(Permanent("HTTP 404"))`, over a temp-dir `Library`; assert the client's
  `calls` counter is `0` (never polled) and the stored job is still `Completed`
  with its result URL intact.

Regression gate (objective §A, slice AC5), run from repo root:

```sh
cargo test --workspace                                   # 0 failed, >= 718 passed
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings    # may need: rustup component add clippy
cargo deny check
./scripts/lint-provenance.py
```

App still builds/launches: `tauri build` (or `scripts/build-macos.sh`) →
`Hickeyfield.app` opens a non-blank window. Never `cargo build`
(objective Constraints).

## Risk needing a human eye

The runner measures elapsed time with `Instant::now()` (`runner.rs:226`) /
`started.elapsed()` (`runner.rs:238`); there is **no injectable clock**, and the
minimum `TimeoutPolicy` budget is 600 s (`engine.rs:290`), so a real long budget
cannot be waited out in a unit test. This design sidesteps that without adding a
clock seam by (a) computing the budget in the pure `timeout_budget` function
(AC1 tests the *value* with no clock) and (b) passing `budget` as a parameter to
`poll_until_terminal` so a test can force `Duration::from_millis(1)` and exercise
the real timeout branch in ~2 ms (AC2 tests the *behaviour*). The residual gap:
the exact production budget→branch coupling for a 600 s+ deadline is not
wall-clock-tested — only that a tiny budget fires and a large one is computed.
That is judged acceptable; a fuller time-injection seam is a Non-goal unless the
verifier disagrees.

## Non-goals (recorded)

- No immediate same-session re-poll after a timeout — resumption is on the next
  `resume_all` (default per Open question below).
- No billing reconciliation on timeout — preserve the resumable record only
  (objective Q6 default).
- No change to `download_outputs` semantics, `apply_poll`, the store schema, or
  any core API. No fake-clock injection. No UI change.
- No new crate dependency (the limiter is `std::sync::Condvar`-based).

## Open questions (each with a default)

1. **Re-poll a timed-out job within the same session, or only on relaunch?**
   Default: **only on relaunch** — leaving the row in `unfinished()` is the
   minimal, deadlock-free fix and avoids a hot loop against a slow provider.
2. **Park extra jobs on a blocking `Condvar`, or build an async queue?**
   Default: **park** — the poll loops are already `spawn_blocking`; parking a few
   tasks on the default 512-thread blocking pool is safe for a desktop app, and a
   proper async queue is a larger refactor out of this slice's surface.
3. **Advisory wording / de-dup for the timeout note.** Default: single advisory
   with a stable `"no result yet after"` prefix, de-duplicated across relaunches.

## Weakest parts (verifier, start here)

1. **The Condvar-based `ConcurrencyLimiter`** — hand-rolled concurrency is the
   likeliest place for a subtle bug (missed `notify`, `avail` underflow, permit
   leak on panic). Confirm `Permit`'s `Drop` releases on *every* exit path and
   that `acquire` cannot underflow, and that parked blocking tasks cannot exhaust
   the pool in a realistic queue.
2. **AC4 offline test fixture** — `resuming_a_completed_job_keeps_its_success`
   pre-sets `local_path` so `download_outputs` stays offline; verify this still
   proves the guard (poll count 0, status preserved) rather than accidentally
   proving nothing.
3. **`SettingsDto` round-trip assumption** — `timeout_budget` deserializes the
   persisted `settings` blob back into `SettingsDto`. Confirm the blob written at
   `commands.rs:763` is the camelCase `SettingsDto` shape for real jobs and that
   pre-policy/legacy rows degrade to `work=None` → `policy.base` (not zero).
