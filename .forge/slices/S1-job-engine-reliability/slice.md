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
