# S1-job-engine-reliability — SECURITY (evidence)

Read-only security review of the **FINAL state** of S1: the F5 (AC9) amendment landed on
top of the F1.2 + F3 amendment, all uncommitted on top of committed S1 `909cdea`. This run
**supersedes** the prior on-disk verdict. No source, docs, config, or backlog changed by
this reviewer — this file is the only artifact authored.

## Scope confirmation (final state)

- Working-tree diff vs `909cdea` (`git diff HEAD --stat`): code files `engine.rs` (+150),
  `runner.rs` (+329), `store.rs` (+64), `recipe.rs` (+4), `app.rs` (+2), `commands.rs` (+3);
  the rest are `.forge/` evidence docs. The F5 delta is confined to
  `src-tauri/src/runner.rs` — grep for the F5-specific tokens (`STALL_AGE_BACKSTOP`,
  `Rerun`, the removed `age_cap`) lands only in `runner.rs`; the engine/store/commands/app/
  recipe deltas are all the prior F1.2/F3 work. `engine.rs` needed no F5 change
  (`TIMEOUT_ADVISORY_PREFIX` + the `apply_poll` clear already do what AC9b relies on).
- **No dependency/manifest change.** `git diff HEAD --name-only | grep -i cargo` → empty.
  `Cargo.lock` still pins **h2 0.4.15** (`Cargo.lock:1551`) and **rustls 0.23.43**
  (`Cargo.lock:3337`) — identical to baseline. The F5 machinery is `std`-only
  (`Duration` const + comparison); no new crate.
- Boundaries examined: whether the 7-day backstop closes the F1.2 early-abandonment
  data-loss path; whether the accumulation/DoS bound from F1.2 is still intact (no new
  unbounded path); cap-defeat / monotonicity of `poll_cycles`/`stalled`; secrets in the new
  const/advisory wording; new log lines; the re-charge implication of "Use Rerun"; and the
  two pre-existing dependency advisories.

---

## Primary question: is the F1 data-loss residual now CLOSED?

**Yes — the F5-specific early-abandonment path is CLOSED, and the residual is reduced to an
acceptable, documented remainder.**

The prior LOW finding was that the age backstop's *effective* value was ~1–5h for hosted
providers (because `clamp(budget*5, 1h, 7d)` never reached its 7-day ceiling — the largest
budget, local 6h, caps `budget*5` at 30h), so a slow-but-alive hosted job could be stalled
and dropped from `unfinished()` **hours inside** the provider's ~7-day retention window,
with no in-app free recovery (F4 unbuilt; the advisory even pointed at a phantom "reopen").

F5 fixes exactly this (`runner.rs:257`, `runner.rs:276-300`):

- The `age_cap(budget)` function and its `budget*5`/clamp scheme are **removed** (grep
  confirms no `fn age_cap` and no live `clamp` in `runner.rs`; only prose/test comments
  remain).
- The age backstop is now a fixed `const STALL_AGE_BACKSTOP: Duration =
  Duration::from_secs(7 * 24 * 3600)` = 604800s (`runner.rs:257`), matched to the provider
  result-retention horizon the comment cites (Higgsfield deletes results after 7 days).
- The stall condition is `job.stalled = job.poll_cycles >= MAX_POLL_CYCLES || aged_out`
  where `aged_out = age > STALL_AGE_BACKSTOP.as_secs()` (`runner.rs:279-280`) — an **OR** of
  two finite bounds, verified by two tests that would each fail under AND
  (`the_fifth_timeout_stalls_the_job` stalls with age=0; `age_past_seven_days_...` stalls at
  `poll_cycles=1`).

**Why this closes the recoverable-loss path.** After F5 the age branch fires **only** once a
job is older than 7 days ≈ the provider's own retention. Past that horizon the completed
result is (about to be) deleted by the provider anyway, so auto-resuming has nothing left to
recover — abandoning it is correct, not lossy. The window where F1.2 abandoned a
still-retrievable paid result (a hosted job stalled at ~1–5h while the provider still held
the output for days) **no longer exists**: within the retention window the age branch cannot
fire. The regression is proven by `a_six_hour_old_job_is_not_stalled_by_age` (a 6h-old job
that the old cap would have stalled now stays resumable).

**Is a job still-InProgress after 5 cycles OR 7 days realistically dead?** Yes, on both legs:
- 7 days ≈ retention expiry — the paid output is gone regardless, so nothing recoverable is
  lost by stalling.
- 5 timeout cycles is the codebase's own "five tries then give up" convention
  (`runner.rs:237-242`, reusing `Backoff::max_attempts`); because a session re-polls only on
  the next launch, 5 cycles means the same job ran a full budget to timeout across **five
  separate app launches** without settling — "never settles", not "slow once".

**Remaining recoverable-loss surface (narrow, LOW, gated on F4).** Two edges survive, both
far narrower than the prior ~1–5h window:
1. The `poll_cycles >= 5` active bound can still stall a genuinely-slow-but-alive job that
   times out across 5 launches while the provider result is still retrievable (< 7 days).
   This bound is **unchanged from F1.2** — F5 did not touch it — and it preserves the row
   (`status`/`request_id`/`results`), so the output is recoverable the moment F4 ships. It is
   defensible by the 5-tries convention and is what F4 (manual retry) exists to cover.
2. A hypothetical provider with retention **longer than 7 days** plus a job that genuinely
   delivers between day 7 and its true expiry would be stalled by the age branch before
   delivery. For realistic video-gen providers (a job running > 7 days is dead, not slow;
   Higgsfield retention is exactly 7 days) this is vanishingly narrow, and Rerun is available.

Net: the F5 age-backstop change **closes** the early-abandonment data-loss path the prior
review flagged. The residual is now a bounded, record-preserving, F4-gated remainder around
the (unchanged) 5-cycle active bound — an acceptable reduction, documented here.

---

## Accumulation / DoS bound: still intact, no new unbounded path

**Confirmed intact.** F5 did not loosen the F1.2 bound into a new unbounded path:

- `poll_cycles` is `saturating_add(1)` exactly once per timed-out session, only in
  `mark_timed_out` (`runner.rs:277`), reachable only from the single timeout branch
  `started.elapsed() > budget` → `mark_timed_out(...)` → `upsert` → `return job`
  (`runner.rs:487-495`). Grep confirms `mark_timed_out` has exactly one production caller
  (`runner.rs:492`); the rest are tests.
- `stalled` is set only at `runner.rs:280` (and initialized `false` at each `JobSet`
  construction). `apply_poll` (`engine.rs:421-455`) — the only place a provider poll payload
  is applied — never writes `poll_cycles`/`stalled` (verified by grep of all writers). So a
  misbehaving/compromised provider returning `InProgress` forever **cannot** rewind or clear
  the cap, and cannot starve the timeout branch (`started.elapsed()` is monotonic wall-clock,
  budget is finite and hard-capped at `HOSTED.cap=1h`/`LOCAL.cap=6h`).
- Every never-settling job leaves `unfinished()` (`engine.rs` filter `!is_settled() &&
  !is_stalled()`) within **≤ 5 active timeout sessions OR** the first timed-out session after
  it is 7 days old — whichever comes first. Both bounds are finite; the set remains
  self-draining. `stalled` is sticky (a stalled row leaves `unfinished()`, so it never
  re-enters `mark_timed_out` to be flipped back; no reset path exists — F4 deferred).

**Effect of the age widening (~5h → 7d) on accumulation — bounded, intended.** F5 lets a
never-settling job that is *not* being actively polled linger in `unfinished()` up to 7 days
(vs ~5h under F1.2) before the age backstop drains it. This marginally raises the possible
steady-state count of unfinished-but-not-stalled jobs, which feeds the (deferred, LOW) F1b
single-launch fan-out. It does **not** restore an unbounded path: the 5-cycle bound still
caps *active* re-attachment regardless of age, the set still drains, and the peak fan-out is
still bounded by a finite (7-days-of-submissions) set on a single-user desktop. This is the
intended correctness/DoS trade-off and stays within the existing LOW F1b residual.

**Pre-existing "close-before-budget-every-time" edge (NOT introduced by F5).** Because both
the `poll_cycles` increment and the age check live *inside* the timeout branch, a job whose
session is always ended (app closed) before its budget elapses never accrues cycles and
never has its age checked, so it never stalls. This is identical under F1.2 and F5 — the
branch structure is unchanged — and requires a degenerate, non-attacker usage pattern on a
single-user desktop with no resource consumption while the app is closed. Noted for
completeness; not an F5 regression and not blocking.

---

## Findings (ranked by severity)

### LOW — F4 (manual free-retry) still unbuilt; the 5-cycle active bound can stall a possibly-recoverable job whose only in-app recovery re-charges (reduced from prior)
- **Where:** `runner.rs:280` (`poll_cycles >= MAX_POLL_CYCLES`), interplay with the deferred
  F4 (no `retry_job` command — grep `retry_job` → no matches; no `ui/` change in the diff).
- **Failure/loss path:** a hosted job on a congested provider queue times out across 5
  separate launches while still legitimately `InProgress` and < 7 days old (provider still
  holds the output). On the 5th timeout it is marked `stalled` and dropped from
  `unfinished()`, so the runner never auto-polls it again. There is no in-app **free**
  recovery: F4 is unbuilt, and the only affordance is Rerun, which **re-charges** (it is
  `delete_job` + a fresh `submit_job`). The paid output the provider still holds is not
  auto-recovered.
- **Why only Low / why reduced:** the row (`status`/`request_id`/`results`) is fully
  preserved (`runner.rs:269` doc; `a_stalled_job_is_dropped_from_the_resume_set` proves
  `request_id` survives), so the output is recoverable in principle the instant F4 ships — the
  loss is of *automatic* recovery, not of the record. 5 cycles = the codebase's own
  "never settles" threshold, so giving up is defensible. This is **narrower** than the prior
  finding: the age branch no longer contributes an early-abandonment window (F5 closed that),
  so the only remaining premature-stall trigger is this defensible active bound. Single-user
  desktop, no attacker.
- **Fix (recommended follow-up, not a blocker):** ship **F4** (`retry_job` + a "Stalled —
  retry?" affordance) as the free in-app recovery the deferred design promises. F4 must reset
  `poll_cycles = 0`; note that with the 7-day backstop measured from `created_at`, a
  `poll_cycles`-only reset is now **sufficient inside the recoverable window** (age < 7d does
  not trip the age branch), and if `created_at` is already > 7d the immediate re-stall is
  *correct* (result expired) — so F5 has largely defused the prior "F4 must also reset
  `created_at`" hazard, though F4 should still be aware of it.

### LOW — Deferred F1b leaves a single-launch fan-out residual, now with a wider (still bounded) accumulation window
- **Where:** `runner.rs:91` (`resume_all` per-job loop, byte-identical to S1), per-job
  `spawn_blocking` in `watch`.
- **Failure path:** `resume_all` still spawns one `watch` → one `spawn_blocking` per
  not-yet-stalled unfinished job; if the per-provider permit (fal=2, local=1) is taken, the
  blocking-pool thread parks on the Condvar until a slot frees. A relaunch with many
  not-yet-stalled jobs can peak the tokio blocking pool. F5's 7-day age window lets more such
  jobs accumulate before the age backstop drains them than F1.2's ~5h did.
- **Why only Low:** the 5-cycle bound still caps active re-attachment; the set still
  self-drains (age ≤ 7d or ≤ 5 cycles); each parked thread releases within one session budget
  (≤ 1h hosted / ≤ 6h local). Bounded, self-healing burst, not a permanent leak; reaching it
  needs an atypical number of simultaneously-unsettled paid jobs on a single-user desktop.
- **Fix (recommended follow-up = the deferred F1b):** replace the per-job `spawn_blocking`
  fan-out in `resume_all` with a bounded per-provider drain that respects the limiter.

### LOW — Pre-existing dependency advisories on the outbound HTTP/TLS stack (NOT this amendment)
Unchanged and **confirmed still baseline**: no `Cargo.toml`/`Cargo.lock` change in the final
tree; `Cargo.lock` still pins **h2 0.4.15** (`Cargo.lock:1551`) and **rustls 0.23.43**
(`Cargo.lock:3337`):
- **RUSTSEC-2026-0258 — h2** (unbounded empty HTTP/2 DATA frames; Low; reachable only via a
  malicious provider endpoint streaming to this HTTPS *client*). Fix: `cargo update -p h2`
  (>= 0.4.16).
- **RUSTSEC-2026-0285 — rustls** (TLS 1.3 handshake messages accepted at the wrong encryption
  level; transcript stays authenticated; protocol-conformance laxity only). Fix:
  `cargo update -p rustls` (>= 0.23.45).

Not this amendment's regression (dependency graph identical to baseline) and Low for this
desktop-client usage. Remains a separate dependency-bump slice.

---

## Note (correctness/UX, not a security finding — do not block)

The stalled advisory now reads (`runner.rs:287-291`): `"{TIMEOUT_ADVISORY_PREFIX} — the
provider hasn't returned a result after N attempts; it may still finish. Use Rerun to try
again."` This is a strict improvement over the prior phantom "reopen this job": **Rerun
exists**. But Rerun **re-charges** (delete + fresh submit), whereas the deferred F4 would be
a free retry against the same `request_id`. Directing a user to a paying action when the
provider may still hold the paid output they already bought is a **cost/UX correctness**
matter, not a security vulnerability: the charge is user-initiated on a single-user desktop,
not attacker-driven cost/abuse. It is also now mostly moot — by the time a job stalls by
*age* (7d) the provider result is expired anyway, so Rerun is the correct action; only the
5-cycle case leaves a still-retrievable output that Rerun re-pays for. Noted; F4 is the
proper fix. Not blocking.

---

## Boundaries examined and found clean (silence is not clearance)

- **F5 const / arithmetic — CLEAN.** `Duration::from_secs(7 * 24 * 3600)` = 604800 fits u64
  trivially; no overflow. `age = now.saturating_sub(created_at).max(0) as u64`
  (`runner.rs:278`): a future `created_at` (clock skew) or epoch-0 row yields age 0 → never a
  spurious stall; `saturating_sub` avoids i64 overflow; the u64 cast is safe. `aged_out`
  compares two u64 seconds. No panic, no underflow, no premature stall.
- **Cap-defeat / monotonicity — CLEAN.** `poll_cycles`/`stalled` are written in production
  only by `mark_timed_out` (`runner.rs:277,280`) and initialized to `0`/`false` at
  construction (`commands.rs:767-768` etc.); no decrement or reset exists. `apply_poll`
  (`engine.rs:421-455`) never touches either field, so a provider poll payload cannot rewind
  or clear the cap. `stalled` is sticky; the count is persisted every session (unconditional
  `store.upsert` after `mark_timed_out`, `runner.rs:493`). No provider-driven defeat path.
- **Secrets in the new const / advisory / logs — CLEAN.** The two advisory strings
  (`runner.rs:286-298`) contain only a `poll_cycles` count and `budget.as_secs()/60` (whole
  minutes) — no `request_id`, `route_id`, `endpoint`, status URL, or key. Diff of added lines
  shows **no new `tracing`/`log`/`println!`/`eprintln!`/`dbg!`** line anywhere in the F5
  delta. The `request_id`/`endpoint` tokens in the added lines are doc-comment prose and a
  test assertion, not new emissions. Keys remain in the OS keychain; none crossed into a
  persisted or logged field.
- **F3 clear still matches after the reword — CLEAN.** The reworded stalled note still begins
  with the `TIMEOUT_ADVISORY_PREFIX` constant (`engine.rs:285` = `"no result yet"`), and
  `apply_poll` on `Phase::Completed` does `retain(|a| !a.starts_with(TIMEOUT_ADVISORY_PREFIX))`
  (`engine.rs:437-438`), so a late completion still clears the note while `fail_reason` and
  other advisories survive (proven by
  `the_stalled_advisory_points_at_rerun_and_still_clears_on_completion` and the three engine
  F3 tests).
- **Migration v6 / store integrity & injection — CLEAN (unchanged by F5).** v6 adds two
  `INTEGER NOT NULL DEFAULT` columns under a `version < 6` guard, static DDL (no string
  interpolation → no SQL injection); upsert uses positional binds. `row_to_job` reads
  `poll_cycles` via `.max(0) as u32` and `stalled` via `!= 0` with `.unwrap_or` defaults
  (`store.rs:224-228`) — a pre-v6/NULL/hand-edited row loads without panicking.
- **Injection / authz / SSRF / destructive-op gates — N/A for this amendment.** F5 adds no
  route, endpoint, credential surface, filesystem path, or external input; `route_id`/
  `endpoint` are used exactly as before; the abandoned-job guard in `run_watched` is
  unchanged. No prompt-injection surface (advisory strings are runner-generated from a
  count/duration, not from fetched or user-supplied content).

---

## Recommended new backlog slices (call-out to the orchestrator)

**No new Critical/High finding**, so **no** mandatory new-slice-per-finding is triggered by
this review. The three follow-ups below are the **only open items** and are all LOW /
pre-existing (the orchestrator/backlog owner edits the backlog — this reviewer does not):

1. **F4 — surface `stalled` + a `retry_job` command (LOW; the named free-recovery for the
   5-cycle residual).** Ship the manual-retry affordance the stalled advisory's intent
   implies. Reset `poll_cycles = 0` (sufficient inside the < 7d recoverable window under the
   new backstop). Until then, a job stalled by the 5-cycle bound with a still-retrievable
   output has only the re-charging Rerun path.
2. **F1b — bounded per-provider drain for `resume_all` (LOW, defense-in-depth).** Replace the
   per-job `spawn_blocking` fan-out with a limiter-respecting drain so a large launch set
   (now potentially wider under the 7-day window) cannot peak the blocking pool.
3. **Dependency-advisory bump (LOW, pre-existing, not this amendment).** `cargo update -p h2`
   (>= 0.4.16, RUSTSEC-2026-0258) and `cargo update -p rustls` (>= 0.23.45,
   RUSTSEC-2026-0285); re-green `cargo deny check`.

---

## Verdict rationale

F5 introduces **no new Critical/High**. The prior LOW data-loss residual — the age backstop
abandoning a still-retrievable hosted result hours inside the retention window — is **CLOSED**:
the fixed 7-day `STALL_AGE_BACKSTOP` fires only at ≈ provider retention expiry (nothing
recoverable left) or, on the unchanged and defensible 5-cycle active bound, when a job has
failed to settle across five launches. F5 did **not** loosen the F1.2 accumulation/DoS bound
into a new unbounded path: the set is still finite and self-draining (≤ 5 cycles OR ≤ 7 days),
the cap is still monotonic, persisted, and provider-proof, and the age widening only enlarges
an already-bounded, self-healing set. No secrets in the new const or advisory wording; no new
log lines. The two `cargo deny` advisories are pre-existing baseline (no manifest change) and
Low for this desktop client. Remaining open items are exactly F4 (manual free-retry), F1b
(bounded drain), and the dependency bump — all LOW/pre-existing and none blocking. The
"Use Rerun" advisory re-charge is a cost/UX correctness note, not a security defect. The F1
data-loss residual is CLOSED/acceptably-reduced and documented.

VERDICT: PASS
