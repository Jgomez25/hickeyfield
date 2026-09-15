# FORGE STATUS

Project: hickeyfield — production-hardening + tool surfaces (personal/local build).
Backlog: 10 slices, reliability-first. See .forge/backlog.md.

## Released
- **S1-job-engine-reliability** — branch `forge/s1-job-engine-reliability` (909cdea + 6f9b3d7).
  Timeout/concurrency/relaunch fixes + F1.2/F3/F5 hardening. 738 tests. F1 data-loss residual CLOSED.
- **S2-honest-free-tier** — branch `forge/s2-honest-free-tier` (c9e832c). has_adapter() drops Local;
  z_image no longer shown as a $0.00 tile that fails; reported unavailable honestly. 740 tests, 216 UI.

## Next
- **S3-reachable-logs** (pending) — packaged .app writes rolling file logs + panic-hook crash file
  to the app log dir, with an in-app "reveal logs" affordance (logs are stdout-only today, lib.rs:16).

## Open follow-ups (logged, not blockers)
- F4 (HIGH) retry_job + stalled UI (reset created_at/poll_cycles on retry).
- F1b bounded per-provider resume_all drain.
- dep-bump h2>=0.4.16, rustls>=0.23.45 (RUSTSEC-2026-0258 / -0285).
- minor: stalled-advisory pluralization; ModelPicker test-title wording; empty Local section in clients.rs.
