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

---

# Amendment: F1 (bound resumable state) + F3 (clear stale advisory)

Upstream read for this amendment (all prior-phase S1 artifacts, no downstream):
`.forge/slices/S1-job-engine-reliability/security.md` (F1, **MEDIUM** — "Timeout-as-
non-terminal is unbounded across sessions", cites `runner.rs:437-445`, `:86-93`,
`:118`) and `review.md` (F3, **MINOR** — "stale timeout advisory survives a late
completion", cites `runner.rs:239-251` + `engine.rs:385-409`). Terms are reused
verbatim from the code and the original design above: `unfinished()`, `is_settled`,
`is_terminal`, `advisory`, `poll session`, `budget`, `Permit`, "reattach". No
glossary/ADR set exists in this repo (recorded in the design preamble), so no ADR
link is owed; this section IS the decision record for the two findings.

This amendment is **DESIGN only** — no code is written here, and (unlike a shipped
slice) there is nothing yet to record in `README`/`document.md`; DOCUMENT is
deferred to the implementing slice. It does not restate or supersede AC1–AC5
above; it adds a bound *on top of* the AC2 pause and re-uses the AC3 limiter.

## New vocabulary (one term)

- **Stalled** — a job the runner has *given up auto-resuming* because it timed out
  across the cap (below) without ever settling. It is **not** a provider status
  and **not terminal**: the last provider-reported `JobStatus` (usually
  `InProgress`) is preserved, the `request_id`/`endpoint` are preserved, and a
  manual retry can still re-poll and recover a late result. "Stalled" is a
  *runner* decision, exactly parallel to how `LocalStage` (`job.rs:80-104`) keeps
  a process-known state out of the provider `JobStatus` vocabulary.

---

## F1 — bound the resumable timeout state

### F1.1 Representation — recommendation and the rejected alternative

**RECOMMEND (a')**: a persisted **counter + flag on the row**, *not* a new
`JobStatus` variant. Concretely two `#[serde(default)]` fields on `JobSet`
(`engine.rs:20-78`):

```rust
/// How many poll sessions have timed out on this job without it settling.
/// Incremented once per timed-out session (one per app launch — a session
/// re-polls only on the next resume_all, per the AC2 Non-goal). The cross-
/// session bound; reset to 0 by a manual retry.
#[serde(default)]
pub poll_cycles: u32,
/// The runner has stopped auto-resuming this job after the cap (F1.2). NOT
/// terminal: status/request_id are preserved and a manual retry clears it.
/// Serialized so the UI can render an honest "Stalled — retry?" affordance
/// without duplicating the cap constant across the bridge.
#[serde(default)]
pub stalled: bool,
```

**Why a field, not `JobStatus::Stalled`** (the alternative the finding hints at):

1. `job.rs:1-32` establishes that `JobStatus` is the *provider wire vocabulary*
   parsed by `clients::normalize_status`; `job.rs:72-87` documents the deliberate
   choice to keep process-known states (`Uploading`/`Downloading`) **out** of
   `JobStatus` and in `Phase`/`LocalStage`, "so the next person to read the enum
   [does not] go looking for the response field that produces it". "Stalled" is
   precisely such a process-only state, so it belongs beside `LocalStage`, not
   inside `JobStatus`.
2. A new terminal `JobStatus` widens the blast radius: `is_terminal`/`phase`
   (`job.rs:131-148`), `refund_expected`/`is_refunded` (`engine.rs:413`,
   `job.rs:152`), the UI's hard-coded `TERMINAL` set (`ui/src/lib/status.ts:43-49`)
   and every `match` on `Phase` would each need a new arm. A field touches none of
   those.
3. Preserving the provider's *last-seen* status on a stalled row is useful truth
   for the retry ("the provider last said InProgress" ≠ "the provider said
   Failed"). A status variant would erase it.

**MUST NOT** call a stalled job terminal. `is_terminal()` stays driven by the
provider status; `stalled` is orthogonal.

`is_stalled()` helper and the `unfinished()` change (`engine.rs:98-110`,
`:167-173`):

```rust
impl JobSet {
    /// The runner has given up auto-resuming this one (F1). Distinct from
    /// is_settled(): a stalled job is owed nothing *automatically*, but a
    /// manual retry can still recover it.
    pub fn is_stalled(&self) -> bool { self.stalled }
}

// JobStore::unfinished default (engine.rs:167-173) — the ONLY behavioural change:
fn unfinished(&self) -> Result<Vec<JobSet>, JobError> {
    Ok(self.all()?
        .into_iter()
        .filter(|j| !j.is_settled() && !j.is_stalled()) // was: !j.is_settled()
        .collect())
}
```

`is_settled()` (`engine.rs:105-109`) is left **pure** — folding "gave up" into
"everything the user is owed has been fetched" would be dishonest. The auto-resume
set (`unfinished()`) is where "and we have not given up" belongs, because that set
is *exactly* what `resume_all` re-attaches (`runner.rs:87`). This is the single
lever: `SqliteStore` and both `MemStore`s inherit the default `unfinished()` (no
override — confirmed `store.rs:209-292`, `runner.rs:501-513`, `engine.rs:459-470`),
so one edit changes every caller.

### F1.2 The cap

Stamped by the runner (which has the clock) inside `mark_timed_out`
(`runner.rs:239-251`), which becomes:

```rust
// cap constants — runner policy, kept next to timeout_budget in runner.rs
const MAX_POLL_CYCLES: u32 = 5;           // matches Backoff::max_attempts (engine.rs:222)
// age cap ties X to the provider TimeoutPolicy per the finding: five full
// session budgets of wall-clock, floored at the hosted 1h cap and ceilinged at
// the 7-day provider result-retention horizon (Higgsfield deletes after 7 days,
// engine.rs:84-88 / job.rs:60-66 — past it even a completed result is gone).
fn age_cap(budget: Duration) -> Duration {
    budget.saturating_mul(MAX_POLL_CYCLES)
        .clamp(TimeoutPolicy::HOSTED.cap, Duration::from_secs(7 * 24 * 3600))
}

/// Now takes `now` (for the age branch) and returns whether it changed the row.
fn mark_timed_out(job: &mut JobSet, budget: Duration, now: i64) -> bool {
    job.poll_cycles = job.poll_cycles.saturating_add(1);       // once per session
    let aged_out = now.saturating_sub(job.created_at) as u64 > age_cap(budget).as_secs();
    job.stalled = job.poll_cycles >= MAX_POLL_CYCLES || aged_out;

    // Advisory: same stable prefix so F3 clears it on a late completion. De-dup
    // by prefix so relaunch cycles do not stack it; swap wording once stalled.
    job.advisories.retain(|a| !a.starts_with(TIMEOUT_ADVISORY_PREFIX)); // engine.rs const (F3)
    job.advisories.push(if job.stalled {
        format!("{TIMEOUT_ADVISORY_PREFIX} {} attempts — Hickeyfield stopped retrying \
                 automatically; reopen this job to try again", job.poll_cycles)
    } else {
        format!("{TIMEOUT_ADVISORY_PREFIX} {} min of polling; still running at the \
                 provider — will keep trying when the app next launches",
                budget.as_secs() / 60)
    });
    job.updated_at = now;
    true // poll_cycles always changed, so the upsert must always fire (persist the count)
}
```

- **`MUST` increment `poll_cycles` every call** (one call = one timed-out
  session). The old early-return-on-dedup (`runner.rs:241-243`) is removed: it
  short-circuited before any state change, which would freeze the counter. The
  advisory *string* is still de-duplicated (retain-then-push), so the UI never
  sees it stacked; only the return contract changes (now always `true`).
- **`MUST` preserve `request_id`/`endpoint`/`status`/`results`** — stalling only
  writes `poll_cycles`, `stalled`, `advisories`, `updated_at`. Nothing a retry
  needs is discarded.

**Cap values + rationale.** `MAX_POLL_CYCLES = 5` reuses the existing
`Backoff::max_attempts = 5` (`engine.rs:222`) rather than coining a new number —
"five tries then give up" is already this codebase's give-up count. Because a
session re-polls only on the *next launch* (AC2 Non-goal, design §"Non-goals"),
5 cycles means the same job survived **5 separate app launches** without settling —
unambiguously "never settles across sessions", not "slow once". The **age cap**
(`budget × 5`, floored 1h, ceilinged 7d) is the backstop for the job the user
leaves for days across long app-closed gaps, where cycles cannot accrue: past
`budget × 5` of wall-clock (e.g. a 720p fal job: `900 s × 5 = 4500 s ≈ 75 min`;
an unsizable job: floored to 1h) it is stalled even at `poll_cycles < 5`. Both
bounds err long and both are **recoverable**, so an over-eager stall costs one
manual click, never a lost paid result.

### F1.3 Bound the `resume_all` fan-out

Today `resume_all` (`runner.rs:86-93`) calls `watch` per unfinished job; each
`watch` does one `spawn_blocking` (`runner.rs:118`) that then **parks on the
Condvar limiter** waiting for a slot — so N unfinished jobs park N blocking-pool
threads at launch (the finding's cost (a)). The F1.2 cap is the **load-bearing**
fix here: it shrinks `unfinished()` so the never-settling set can no longer grow
without bound. This F1.3 change is **defense-in-depth** for the remaining edge
(a user with many genuinely-in-flight jobs at one relaunch).

**SHOULD** replace the per-job spawn in `resume_all` with a **bounded per-provider
drain**, reusing the S1 limiter as the cap authority (AC3 unchanged):

```rust
pub fn resume_all(&self) -> Result<usize, JobError> {
    let pending = self.store.unfinished()?;            // now excludes stalled (F1.1)
    let n = pending.len();
    // Bucket by provider (route_id prefix, provider.rs:107 from_slug).
    let mut by_provider: HashMap<ProviderId, VecDeque<JobSet>> = HashMap::new();
    let mut unknown = Vec::new();
    for job in pending {
        match job.route_id.split(':').next().and_then(ProviderId::from_slug) {
            Some(p) => by_provider.entry(p).or_default().push_back(job),
            None => unknown.push(job),                 // no provider -> old path
        }
    }
    for (p, queue) in by_provider {
        let queue = Arc::new(Mutex::new(queue));
        let workers = (p.features().max_concurrent.permits() as usize).min(
            queue.lock().unwrap().len());              // never more workers than jobs
        for _ in 0..workers { self.spawn_drain_worker(Arc::clone(&queue)); }
    }
    for job in unknown { self.watch(job); }            // unroutable: unchanged
    Ok(n)
}
```

Each `spawn_drain_worker` is one `spawn_blocking` that loops: pop the next job
under the lock (return when empty), then run the **existing** single-job body —
i.e. the `watching`-set dedup insert (`runner.rs:100-106`), `run_watched(...)`
(`runner.rs:339`, which still `acquire`s its permit), and the `watching`/
`abandoned` cleanup (`runner.rs:130-131`). Extract that body into a shared
`fn attach_one(&self, job: JobSet)` used by both `watch`'s spawn and the worker,
so there is one code path.

- **Blocking-pool bound**: at most `Σ over providers of min(permits, count)` tasks
  exist during relaunch — `≤ Σ permits = 2 (fal) + 1 (local) + 4×7 (rest) = 31`,
  independent of queue depth. Because a provider spawns `≤ permits` workers and
  each holds one slot, its workers never park on the limiter (no wasted parked
  threads); the limiter still bounds the *union* with any steady-state `watch`
  calls to `permits`, so **AC3 is preserved** and the cap authority is unchanged.
- **Nothing is dropped**: every pending job is in exactly one queue and every
  queue is fully drained; the `watching` insert still de-duplicates.
- **No deadlock**: `permits() >= 1` (`provider.rs:203`,
  `every_provider_permits_at_least_one_job` `provider.rs:268-277`); a worker holds
  only the queue lock while popping (released before `run_watched`), so it never
  waits on a permit while holding the queue lock.
- Steady-state `watch` from `submit_job` (`commands.rs:774`) is **unchanged** —
  the accepted single-job tradeoff from S1 (design Open question #2).

**Worked example.** Relaunch with 40 unfinished fal jobs + 3 local: `resume_all`
buckets them, spawns `min(2,40)=2` fal drain workers and `min(1,3)=1` local worker
— **3 blocking tasks total**, not 43. The 2 fal workers chew through 40 jobs two
at a time (peak fal in-flight = 2, AC3 held); the 1 local worker runs the 3
serially. Any job that stalls mid-drain is simply absent from next launch's
`unfinished()`.

### F1.4 Surfacing and recovery

- **Surfaced without a new status DTO.** `list_jobs` (`commands.rs:787-790`)
  returns the raw core `JobSet`, so the new `stalled: bool` field crosses the
  bridge automatically (serde field name `stalled`). A stalled job also (i) keeps
  a non-terminal `status`, so it is *not* in `watching_jobs` (`commands.rs:878-893`)
  — the UI's existing "non-terminal in the DB but nothing is watching it → offer
  a resume" heuristic (that command's own doc, `:880-882`) already covers it — and
  (ii) carries the "…stopped retrying automatically; reopen to try again" advisory,
  rendered today by `MetaCard.tsx:166-175`. So a minimal honest surface needs **no
  new command and no new UI in this slice**.
- **Recovery — follow-up, not built here** (per the task's "note it as a follow-up"
  instruction). No manual-retry command exists (`commands.rs` has
  submit/list/cancel/delete/reveal/watching, none re-attach a row). RECOMMEND a
  follow-up `retry_job(job_set_id)` command that resets `poll_cycles = 0`,
  `stalled = false`, clears the timeout advisory, `store.upsert`s, and calls
  `runner.watch(job)` (idempotent via the `watching` set). Until it ships, a
  stalled job is still recoverable via the existing Rerun (`delete_job` +
  re-`submit_job`) path.

### F1.5 Ripple flags (for TEST/REVIEW)

- **Core public API**: `JobSet` (`engine.rs:20`) gains two `pub` fields. Backward-
  compatible on the wire (both `#[serde(default)]`), but **every `JobSet` struct
  literal must add them** — the test helpers at `engine.rs:423-446`,
  `runner.rs:553-576`, `store.rs:300-323`, and the real construction in
  `submit_job` (`commands.rs:741-766`). There is no `Default` impl to lean on.
- **Store schema**: migration **v6** in `store.rs:56-145` adds
  `poll_cycles INTEGER NOT NULL DEFAULT 0` and `stalled INTEGER NOT NULL DEFAULT 0`;
  `row_to_job` (`:166-207`) reads both (`.unwrap_or(0)` / `!= 0`), `upsert`
  (`:210-269`) writes both in the INSERT + `ON CONFLICT` clauses. The
  `a_v1_database_upgrades_without_losing_rows` test (`store.rs:466-502`) **MUST**
  add the two columns to its DROP list so it still replays from v1. Status is a
  TEXT column parsed by serde (`:177`), so — a second reason to prefer the field
  over a `JobStatus` variant — **no status enum migration is needed** and old rows
  degrade to `poll_cycles=0, stalled=false` (correct: never-yet-stalled).
- **UI DTO**: `ui/src/types.ts` `JobSet` (`:179-209`) should gain optional
  `stalled?: boolean` and `pollCycles?: number`; `ui/src/api.ts` `RawJobSet`
  (`:355-383`) + `toJobSet` (`:432-458`) map them. `ui/src/lib/status.ts`'s
  `TERMINAL`/`isTerminal` (`:43-65`) do **not** need a new entry (stalled is not a
  status), but the job card SHOULD read `job.stalled` to swap the "Generating"
  chip for a "Stalled — retry?" affordance rather than showing a spinner — else
  the chip contradicts the advisory (the same class of bug F3 fixes). All UI work
  is **follow-up**, gated on the `retry_job` command; flag it, do not build it.

### F1.6 AC1–AC4 preservation

- **AC1 (per-job budget)** — untouched. `timeout_budget` (`runner.rs:217-230`) and
  the budget→branch coupling are unchanged; the cap counts *sessions*, orthogonal
  to the budget's *value*.
- **AC2 (a timed-out job stays resumable) — THE critical one.** Within the cap the
  behaviour is byte-for-byte today's: one timeout → `poll_cycles=1`,
  `stalled=false`, non-terminal, `is_settled()==false`, `is_stalled()==false` →
  still in `unfinished()` → re-attached → a late `Completed` still downloads. The
  cap changes behaviour **only** after 5 sessions / age cap, and even then the
  result is not lost — the request_id survives and a manual retry re-polls it. The
  existing AC2 tests (`a_timed_out_job_stays_in_unfinished`,
  `a_late_result_is_reattached_after_a_timeout`, `runner.rs:814-894`) both operate
  at `poll_cycles=1` and **still pass unchanged** (beyond the mechanical `now`
  argument to `mark_timed_out`).
- **AC3 (per-provider cap)** — the Condvar limiter (`runner.rs:294-329`) and its
  unit tests (`:898-972`) are **unchanged**; F1.3's drain workers funnel through
  the same `acquire`, and worker count `≤ permits`, so peak in-flight per provider
  is still `permits`.
- **AC4 (Completed-but-undownloaded not re-polled)** — `provider_poll_needed =
  !is_terminal()` (`runner.rs:259-261`) unchanged. A stalled job is non-terminal,
  but it is excluded from `unfinished()` so `resume_all` never hands it to
  `run_watched`; if a user manually retries it we *want* it re-polled (it is
  genuinely non-terminal), so `provider_poll_needed==true` is correct. AC4's
  Completed job is terminal and skipped regardless.

### F1.7 Tests to ADD (reuse `runner.rs`/`engine.rs`/`store.rs` doubles)

Reuse `MemStore`, `ScriptedClient`, `job(id)`, `ok(status)` in `runner.rs`
(`:500-585`); `MemStore`/`job(status)` in `engine.rs` (`:456-470`); `job(id,status)`
+ `SqliteStore::in_memory` in `store.rs` (`:300-323`). Existing direct
`mark_timed_out(&mut j, dur)` call sites (`runner.rs:798,809,859`) take a `now`
argument.

- `engine.rs`: **`a_stalled_job_leaves_the_auto_resume_set`** — a job with
  `stalled=true` (status `InProgress`) is absent from `store.unfinished()` while a
  `stalled=false` `InProgress` job is present; assert `is_terminal()==false` and
  `is_settled()==false` on the stalled one (stalled ≠ terminal ≠ settled).
- `runner.rs`: **`the_fifth_timeout_stalls_the_job`** — call `mark_timed_out` five
  times (fresh `created_at`, a `now` inside the age cap); assert `poll_cycles==5`,
  `stalled` flips to `true` on the fifth only, status still `InProgress`,
  `fail_reason` still `None`, exactly one advisory whose text says "reopen".
- `runner.rs`: **`age_stalls_a_job_before_the_cycle_cap`** — one `mark_timed_out`
  with `created_at` far in the past (`now - created_at > age_cap(budget)`); assert
  `stalled==true` at `poll_cycles==1`.
- `runner.rs`: **`a_timeout_within_the_cap_still_stays_resumable`** — the AC2
  guard, re-pinned: `poll_cycles=1`, `stalled==false`, `store.unfinished()` still
  returns the row. (Guards against a future cap change silently regressing AC2.)
- `runner.rs`: **`resume_all_bounds_workers_per_provider`** — seed a `MemStore`
  with e.g. 6 `fal:` unfinished jobs against an always-`InProgress`
  `ScriptedClient`; drive the drain and assert peak concurrent poll loops `<= 2`
  and every job eventually attached (mirror the AC3
  `provider_cap_bounds_simultaneous_poll_loops` shape, `:898-937`). If timing a
  live `spawn_blocking` drain is flaky, unit-test the bucketing/worker-count math
  on the pure helper instead and note the wiring is inspection-verified (as S1 did
  for the permit wiring, review.md's minor coverage note).
- `store.rs`: **`stall_fields_round_trip`** — upsert a job with `poll_cycles=3,
  stalled=true`, reopen, assert both survive; extend
  `a_v1_database_upgrades_without_losing_rows` to prove a pre-v6 row loads as
  `poll_cycles=0, stalled=false`.

---

## F3 — clear the stale timeout advisory on terminal completion

**Where**: `apply_poll` (`engine.rs:385-409`). **Guard**: only when the *new*
status is terminal `Completed`, and only advisories carrying the timeout prefix.

**Shared constant.** The prefix currently lives as a private `const NOTE` in
`runner.rs:240`; `apply_poll` (core) cannot see it. **MUST** promote it to a
`pub const` in `engine.rs` so producer (runner) and clearer (core) cannot drift:

```rust
// engine.rs, near TimeoutPolicy (used by runner::mark_timed_out AND apply_poll)
pub const TIMEOUT_ADVISORY_PREFIX: &str = "no result yet";
```

Note the prefix is shortened to `"no result yet"` (from S1's `"no result yet
after"`) so it matches **both** the running note ("no result yet after N min…")
and the F1.2 stalled note ("no result yet after N attempts…"). `runner::
mark_timed_out` references this constant instead of its local `NOTE` (F1.2 already
shows it doing so). The existing AC2 assertion `a.contains("no result yet")`
(`runner.rs:840`) still holds.

**The edit** in `apply_poll`, after `job.status = poll.status;` (`engine.rs:392`):

```rust
if poll.status.phase() == Phase::Completed {
    // A completed job has its bytes; the "still running, will keep trying"
    // advisory now contradicts its own state (review.md F3). Clear ONLY the
    // timeout-family advisory — real fail_reason and any other advisory (e.g. a
    // dropped-setting note) are untouched.
    job.advisories.retain(|a| !a.starts_with(TIMEOUT_ADVISORY_PREFIX));
}
```

- `Phase` is already imported (`engine.rs:13`), so **no import change**.
- **Guarded to `Completed` only**, per the finding: a `Failed`/`Nsfw` job keeps
  the advisory (its `fail_reason` dominates the UI, and clearing it there could
  hide context) — recorded default, below.
- **`MUST NOT` touch `fail_reason`** or non-timeout advisories — the `retain`
  predicate is the whole guard.
- The clear runs inside the `changed` mutation block (`apply_poll` early-returns
  at `:389-391` when nothing moved). The transition into `Completed` from a
  non-`Completed` status is always a status change, so `changed==true` and the
  clear fires and is persisted by the caller's `store.upsert`
  (`runner.rs:451`). **Edge (recorded default)**: a row already `Completed` that
  re-polls `Completed` with identical outputs returns early and is not re-cleared;
  it does not arise on the timeout→resume→complete path (that path *transitions*),
  so it is out of scope — noted, not fixed.

**Worked example.** A job timed out at `InProgress` carrying "no result yet after
15 min… will keep trying"; on the next launch it re-polls `Completed` with an
output URL. `apply_poll` sets `status=Completed`, the guard fires and drops that
one advisory, `download_outputs` saves the bytes. The card now shows "Ready" with
**no** contradicting "still running" line — and if the job had also carried a real
"audio setting ignored" advisory, that one **survives**.

### F3 AC preservation + tests

Preserves AC1–AC4 (apply_poll's status/results/actual_usd handling is untouched;
only the advisory vector is filtered on the Completed transition). AC5 gate
unaffected.

- `engine.rs`: **`completing_clears_the_timeout_advisory`** — a job at
  `InProgress` with `advisories = ["no result yet after 15 min…"]`; `apply_poll`
  with a `Completed` `PollResult`; assert the advisory is gone, `status==Completed`.
- `engine.rs`: **`completion_keeps_real_advisories`** — same but advisories =
  `["no result yet after…", "audio not supported on this route"]`; assert only the
  timeout one is dropped and `fail_reason` stays `None`.
- `engine.rs`: **`a_failed_job_keeps_its_timeout_advisory`** — pins the
  `Completed`-only guard: `apply_poll` to `Failed` leaves the advisory in place.
- `runner.rs` (integration): extend `a_late_result_is_reattached_after_a_timeout`
  (`:851-894`) to assert the returned job's `advisories` no longer contains "no
  result yet" — the end-to-end F3 proof through the real poll loop.

---

## Amendment Non-goals (recorded)

- No `retry_job`/`resume_job` command or stalled-state UI in this slice — F1.4
  follow-up (task instruction). Recovery via Rerun works meanwhile.
- No same-session re-poll after a timeout — still next-launch only (S1 Non-goal).
- No billing reconciliation on stall (objective Q6 default).
- No change to the AC3 Condvar limiter primitive, `download_outputs`, or the
  provider `JobStatus` vocabulary.
- F3 clears advisories only on `Completed`, not on other terminal states.

## Amendment Open questions (each with a default)

1. **`MAX_POLL_CYCLES` value?** Default: **5** (reuses `Backoff::max_attempts`).
   Raise later if real fal queues legitimately exceed five relaunch cycles.
2. **Age cap shape?** Default: **`budget × 5`, floored 1h, ceilinged 7d** — ties X
   to the `TimeoutPolicy` per the finding while respecting the provider result-
   retention horizon.
3. **One field or two?** Default: **two** (`poll_cycles` counter + `stalled`
   decision). `stalled` is derivable from `poll_cycles`+age, but persisting it (a)
   gives the UI a clean boolean without the cap constant crossing the bridge and
   (b) lets an age-stall be represented without clamping the counter. Collapsing to
   a single serialized computed field is possible but awkward given `list_jobs`
   returns the raw `JobSet` with no DTO layer.
4. **Clear the advisory on `Failed` too?** Default: **no** — `fail_reason` is the
   UI's headline there and the advisory is comparatively harmless.

## Amendment weakest parts (verifier, start here)

1. **`resume_all` drain-worker lifecycle (F1.3)** — the highest-surface change and
   the likeliest place for a bug: worker/queue lock ordering vs. the `watching`
   and `abandoned` locks, the `min(permits, count)` worker count, ensuring every
   queue fully drains and no worker exits with jobs still queued, and that a worker
   never holds the queue lock across `run_watched`. Confirm the extracted
   `attach_one` is the *same* path `watch` uses (dedup + cleanup) so the two cannot
   diverge. This is the single riskiest decision — see summary.
2. **`mark_timed_out` contract change (F1.2)** — it now always returns `true` and
   always increments `poll_cycles`; verify no caller relied on the old
   `false`-on-dedup return, and that the counter is persisted on every timed-out
   session (the upsert at `runner.rs:440-443` must still fire).
3. **Two-field migration + struct-literal ripple (F1.5)** — verify v6 round-trips,
   the pre-v1 upgrade test still replays, and every `JobSet` literal compiles.
4. **F3 `changed`-gate interaction** — confirm the Completed transition always sets
   `changed=true` so the advisory clear is actually persisted, and that the retain
   predicate cannot catch a non-timeout advisory that happens to start with the
   prefix (it will not — the prefix is a full literal, but worth a glance).
