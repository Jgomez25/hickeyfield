# S1-job-engine-reliability — SLICE (DISCOVER + PLAN)

## Delivers (one sentence, end-to-end)
A generation run no longer silently loses paid output: the runner honors each provider's `TimeoutPolicy` and per-provider concurrency cap, keeps a timed-out job resumable, and never flips a completed-but-undownloaded job to Failed on relaunch.

## Acceptance criteria (testable — the tester will run these)
- [ ] AC1 — Runner uses `provider.rs::features().timeout` / `engine.rs` `TimeoutPolicy` instead of the flat `DEFAULT_TIMEOUT` (`runner.rs:238`); a test asserts a long/4K `Billable` yields a larger timeout budget than a short job.
- [ ] AC2 — On timeout a job stays in `unfinished()` / resumable (not permanently Failed); a runner test proves a late provider result can still be downloaded/reattached after a timeout.
- [ ] AC3 — Concurrency capped at `Concurrency::Provider(2)` for fal; queuing more than N jobs spawns no more than N simultaneous poll loops and drops no job (replaces the uncapped per-job spawn at `runner.rs:81`); a runner test proves it.
- [ ] AC4 — A Completed-but-undownloaded job is not flipped to Failed on the relaunch re-poll (`runner.rs:250`); a test resumes such a job and asserts the terminal-success record + result URLs survive.
- [ ] AC5 (regression) — Global gate green: `cargo test --workspace` (0 failed, >=718 passed), `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo deny check`, `./scripts/lint-provenance.py`. App still builds+launches.

## Approach + why this slice now
Highest-value, lowest-surface fix: real money is lost today on the one path the README calls "verified." All changes localized to `src-tauri/src/runner.rs` (+ possibly small helper exposure in `engine.rs`/`provider.rs`). The `TimeoutPolicy` and `Concurrency` machinery already exist and are unit-tested; this slice wires them into the runtime poll loop. No UI change. Every other slice depends on the generator being trustworthy, so it goes first.

---
## Amendment ACs (F1 core + F3) — reopened
- [ ] AC6 (F1.2 — bound the resumable state) — A timed-out job that never settles is capped: after N re-poll cycles OR age > budget×N it transitions to a `stalled` state and is EXCLUDED from `unfinished()`, so `resume_all` stops re-attaching it indefinitely. Represented as core fields on `JobSet` (`poll_cycles`, `stalled`; serde-default), NOT a new `JobStatus` variant. Store migration (v6) round-trips the new fields. Tests: cap reached → stalled+excluded; within-cap a late result STILL auto-reattaches (AC2 preserved).
- [ ] AC7 (F3 — clear stale advisory) — On terminal `Completed`, `apply_poll` clears ONLY the timeout NOTE advisory (via a `TIMEOUT_ADVISORY_PREFIX`), leaving `fail_reason`/other advisories intact. Test asserts the advisory is gone on completion but real failure reasons survive.
- [ ] AC8 (preserve S1) — AC1–AC4 still pass unchanged; full regression gate green (`cargo test --workspace` >=727, fmt, clippy, provenance, `pnpm build`/`pnpm test` unaffected).

### DEFERRED from this amendment (logged, not built):
- **F1b** — `resume_all` bounded per-provider drain worker rewrite (defense-in-depth; riskiest change per design). The F1.2 cap is the load-bearing fix; F1b is a separate future slice.
- **F4** — surface `stalled` in the UI DTO + a `retry_job` command so a stalled job is manually recoverable in-app. **CORRECTION (from S1 SECURITY/REVIEW):** the effective age horizon is ~1–5h for hosted jobs (not 7d — `budget×5` maxes at 30h, so the 7d ceiling never binds), so a slow-but-alive hosted job CAN be stalled while its output is still retrievable, and until F4 ships there is no in-app recovery (only Rerun, which re-charges). F4 is therefore raised in priority; note F4 must reset `created_at`, not just `poll_cycles`, or the age branch immediately re-stalls.

---
## AC9 (F5 — honest age-cap + advisory wording) — pulled into scope
- [ ] AC9a — The age backstop is a defensible horizon: replace `age_cap = clamp(budget*5, 1h, 7d)` (effective ~1–5h) with a real **7-day** age backstop, leaving `poll_cycles >= MAX_POLL_CYCLES` as the active-polling bound. A hosted job is NOT stalled by age at a few hours; the comment matches behavior. Test: a job aged e.g. 6h with <5 cycles is NOT stalled; a job aged >7d (or >=5 cycles) IS.
- [ ] AC9b — The stalled/timeout advisory references a REAL recovery affordance (Rerun), not the non-existent "reopen this job." Must retain `TIMEOUT_ADVISORY_PREFIX` so F3's clear-on-Completed still matches. Test: advisory text no longer says "reopen"; prefix intact so completion still clears it.
- [ ] AC9c — All prior ACs (AC1–AC4, AC6–AC8) still pass; regression gate green.
