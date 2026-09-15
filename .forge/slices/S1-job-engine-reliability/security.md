# S1-job-engine-reliability — SECURITY (evidence)

Read-only security review. No source, docs, or config changed by this reviewer.
Scope: only what S1 touched.

## Scope confirmation

- `git diff --stat` → **one** file changed: `src-tauri/src/runner.rs`
  (529 insertions, 24 deletions). No other tracked file modified.
- `git diff --name-only -- '*Cargo.toml' '*Cargo.lock'` → **empty**. S1 introduced
  **no** dependency/manifest change, so the dependency graph is identical to baseline.
- New machinery is `std`-only (`std::sync::{Condvar, Mutex}`, `HashMap`), no new crate.

Boundaries examined: injection / panic on parsed input, secrets in new
log/advisory/error strings, concurrency-limiter correctness (Condvar / poison /
underflow / hang), timeout-as-non-terminal bounding (DoS / resource exhaustion),
and the pre-existing dependency advisories flagged by `cargo deny`.

---

## Findings (ranked by severity)

### MEDIUM — Timeout-as-non-terminal is unbounded across sessions (S1-introduced)
- **Where:** `src-tauri/src/runner.rs:437-445` (timeout branch), `:239-252`
  (`mark_timed_out`), `:86-93` (`resume_all`), `:118` (per-job `spawn_blocking`).
- **Failure/exploit path:** Pre-S1, a job that ran out its deadline was set
  `Failed` (terminal) and left `unfinished()`, so it stopped being re-polled. S1
  deliberately makes a timeout a *pause*: `mark_timed_out` only pushes a
  de-duplicated advisory and bumps `updated_at`; it never terminalizes and there
  is **no attempt counter and no age cap**. A provider that holds a job
  non-terminal past its budget — a stuck provider-side queue, a `request_id` that
  perpetually returns `Queued/InProgress`, or a misbehaving/compromised provider
  that never returns terminal (a `Permanent` error would still fail it, but a
  perpetual "in progress" success will not) — leaves the job `InProgress`
  forever. It stays in `unfinished()` indefinitely. On **every** launch,
  `resume_all()` re-attaches all such jobs; each does its own
  `spawn_blocking(...)` and, if it needs to poll, **parks (blocked) on the
  per-provider Condvar semaphore** while it waits for a slot. As the user keeps
  submitting against such a provider, the never-terminal set grows without bound,
  and two costs grow with it: (a) each relaunch parks one blocking-pool thread per
  stuck job for that provider — once that count approaches tokio's blocking-pool
  ceiling, the pool saturates with parked threads and starves *all* other blocking
  work until an active poll's budget elapses (up to 1h hosted / 6h local — see
  `TimeoutPolicy::HOSTED.cap`/`LOCAL.cap`, `engine.rs:293`/`:305`); (b) dead
  `request_id`s are re-polled forever, perpetual outbound cost/traffic with no
  give-up. There is no automatic pruning — `delete_job` (`commands.rs:798`) is the
  only removal path and is user-initiated.
- **Why only Medium:** single-user desktop; no unauthenticated external attacker;
  requires a provider to keep jobs non-terminal (a real bug case, or a malicious
  provider the user is already paying) plus accumulation over time; degradation is
  gradual, and each *individual* poll session is bounded (the per-session budget is
  finite and hard-capped — see below). It is a slow resource leak, not an instant
  crash or a permanent deadlock.
- **Fix (recommended follow-up slice, not a blocker):** bound the resumable state.
  Either (1) track timeout/relaunch cycles or first-seen age on the job and, after
  N cycles / T age, move it to a distinct terminal-but-recoverable state
  ("stalled — provider never answered") that leaves the auto-resume set while
  staying visible/re-triable; and/or (2) feed `resume_all` through a bounded work
  queue that respects the limiter instead of doing one `spawn_blocking` per stuck
  job, so a large `unfinished()` set cannot park an unbounded number of
  blocking-pool threads at launch.

### LOW — Pre-existing dependency advisories on the outbound HTTP/TLS stack (NOT S1)
`cargo deny check` exits 1 on two advisories. S1 changed no manifest (confirmed
above), so these are **baseline / pre-existing**, not an S1 regression.

- **RUSTSEC-2026-0258 — h2 0.4.15** (`Cargo.lock:146`; via `reqwest 0.13.1 →
  hyper 1.11 → h2`). Unbounded queuing of empty HTTP/2 DATA frames → unbounded
  memory or a length-overflow panic. Advisory **self-rates Low**. This app is an
  outbound HTTPS *client* with `http2` enabled (`Cargo.toml:31`); reachability
  therefore requires a **malicious or compromised provider endpoint** (or a server
  the user is pointed at) to stream unbounded empty DATA frames to the client.
  Real but narrow. Fix: `cargo update -p h2` (>= 0.4.16).
- **RUSTSEC-2026-0285 — rustls 0.23.43** (`Cargo.lock:298`; via `hyper-rustls →
  reqwest`; app uses `rustls` + `rustls-native-certs`, `Cargo.toml:22-27`). TLS
  1.3 handshake messages accepted at the wrong encryption level. The advisory is
  explicit that **the transcript stays authenticated — a network-position attacker
  cannot alter or complete a handshake**; the practical effect is only
  protocol-conformance laxity (functionally the same as the Low Go CVE-2025-61730).
  Low impact for this client. Fix: `cargo update -p rustls` (>= 0.23.45).

**Judgment:** for this desktop-client usage neither advisory is High/Critical, so
they do **not** block S1 (and are not S1's regression). They **are** a legitimate
production-hygiene item — recommend a dedicated dependency-bump slice (below).

---

## Boundaries examined and found clean (silence is not clearance)

- **Injection / panic on parsed input — CLEAN.** `timeout_budget`
  (`runner.rs:217`) and the permit-acquire path (`run_watched`, `:339`) parse
  `route_id` with `split(':').next()` (always `Some`, even for `""`) →
  `ProviderId::from_slug` (`Option`, `unwrap_or(HOSTED)` / `.map`), and settings
  via `serde_json::from_value::<SettingsDto>(...).ok()` (`Result`, never panics on
  hostile JSON). `TimeoutPolicy::timeout` (`engine.rs:358`) clamps non-finite /
  absurd work-units and hard-caps the result, so a corrupt/hostile `settings` blob
  cannot drive `Duration::mul_f64` to panic the runner thread and cannot yield a
  zero budget. No filesystem path is constructed from `route_id`. No panic path
  found from malformed input.
- **Secrets in new log/advisory/error strings — CLEAN.** The only new
  user/persisted string S1 adds is the `mark_timed_out` advisory
  (`runner.rs:242-247`): it contains **only a whole-minute count**
  (`budget.as_secs() / 60`) — no `route_id`, no status URL, no `request_id`, no
  key. Keys live in the OS keychain and none crossed into a log/status/advisory
  field. (The pre-existing missing-credential message at `runner.rs:402-405` emits
  only the provider *slug*, not the key — unchanged by S1.)
- **Concurrency limiter correctness — CLEAN.** `Slot.avail` is recovered from a
  poisoned lock on **both** paths (`acquire` `:320`, `Permit::drop` `:285`) via
  `unwrap_or_else(|e| e.into_inner())`, so a panic in one poll loop cannot wedge
  the whole job subsystem. `acquire` loops `while *avail == 0 { wait }` under the
  lock (no spurious-wakeup / missed-notify escape) and decrements only a
  verified-positive count (no underflow). `Concurrency::permits()` is guaranteed
  `>= 1` (`provider.rs:203-213`), so no slot starts empty and the queue cannot
  deadlock on a zero-permit slot. `Permit`'s RAII `Drop` releases + `notify_one`
  on every exit path incl. panic unwind, so a slot cannot leak. The Condvar cannot
  hang *forever*: a parked acquirer is released when an in-flight poll ends, and
  every poll session is itself bounded by the finite, hard-capped budget. (The
  cross-session accumulation of *how many* jobs park is the Medium finding above,
  not a limiter-correctness defect.)
- **Authz / tenant isolation / SSRF / destructive-op gates — N/A for this slice.**
  S1 adds no new route, endpoint, or credential surface; `route_id`/`endpoint` are
  used exactly as before. The abandoned-job guard is re-checked before acquiring a
  permit and again before writing outputs (`run_watched`, `:352`/`:378`), so a
  user-deleted job neither consumes a slot nor reappears complete.

---

## Recommended new backlog slices (call-out to the orchestrator)

No Critical/High finding, so **no mandatory** new-slice-per-finding is triggered.
Two follow-ups are recommended (the orchestrator/backlog owner edits the backlog —
this reviewer does not):

1. **Dependency-advisory bump (LOW, pre-existing, not S1):** `cargo update -p h2`
   (>= 0.4.16, RUSTSEC-2026-0258) and `cargo update -p rustls` (>= 0.23.45,
   RUSTSEC-2026-0285); re-green `cargo deny check`. Real production item; not an S1
   regression and not an S1 blocker.
2. **Bound the resumable/non-terminal state (MEDIUM, S1-introduced):** cap
   re-poll cycles / age and/or bound `resume_all` fan-out, per the fix above, so
   never-completing jobs cannot accumulate unboundedly in `unfinished()` and park
   an unbounded number of blocking-pool threads.

---

## Verdict rationale

S1's own changes introduce **no new Critical/High** security issue. The single
S1-introduced finding (unbounded non-terminal accumulation) is **Medium**; the two
`cargo deny` advisories are **pre-existing baseline** (no manifest touched) and
**Low** for this desktop-client usage. Nothing S1 introduced is a blocker.

VERDICT: PASS
