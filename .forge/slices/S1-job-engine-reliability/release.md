# S1-job-engine-reliability — RELEASE

**Gate:** `python3 .forge/gate.py` → exit 0. All phases evidenced; TEST + REVIEW + SECURITY = PASS.

**Regression (objective §A):**
- `cargo test --workspace` → 727 passed / 0 failed / 9 ignored (baseline was 718).
- `cargo fmt --all --check` → 0 · `cargo clippy --workspace --all-targets -- -D warnings` → 0.
- `./scripts/lint-provenance.py` → exit 0 (shipped-source teeth verified intact).
- `tauri build --bundles app` → built `Hickeyfield.app` in 2m26s; launch smoke test → process ran (PID observed), then terminated. App still builds and launches.
- `cargo deny check` → exit 1 on baseline advisories RUSTSEC-2026-0258 (h2) + RUSTSEC-2026-0285 (rustls); NO manifest change in S1, pre-existing → tracked as follow-up F2, not an S1 regression.

**Landing decision (owner-approved):** branch + commit **locally only**, no push, no PR. `.forge/` evidence trail included in the commit.

**Branch:** `forge/s1-job-engine-reliability` (off `main`).
**Commit sha:** recorded in git log for this branch (reported to owner in chat at release).

**Changes shipped in this slice:**
- `src-tauri/src/runner.rs` — per-provider `TimeoutPolicy` budget (was flat 600s); timed-out jobs stay resumable; per-provider poll cap (fal=2); no relaunch flip of Completed→Failed.
- `scripts/lint-provenance.py` — `.forge` added to `SKIP_DIRS` (owner-approved toolkit housekeeping; see `.forge/PROVENANCE-SCOPE.md`).
- `CHANGELOG.md` (created), `docs/PARITY.md` (§3.3 annotated).

**Follow-ups logged (not blockers):** F1 bound resumable timeout state (Medium), F2 dependency-advisory bump (Low), F3 clear stale advisory on completion (Minor). See `.forge/backlog.md`.

**Status:** released (local branch).
