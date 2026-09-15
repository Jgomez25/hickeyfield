# FORGE STATUS

Project: hickeyfield — production-hardening + tool surfaces (personal/local build).
Backlog: 10 slices, reliability-first. See .forge/backlog.md.

## Done
- **S1-job-engine-reliability: RELEASED** (branch `forge/s1-job-engine-reliability`, 909cdea + 6f9b3d7).
  Core fix + F1.2 (bound resumable state) + F3 (clear stale advisory) + F5 (honest 7d age cap).
  gate.py exit 0; TEST/REVIEW/SECURITY PASS; 738 tests; app builds+launches. F1 data-loss residual CLOSED.

## Next
- **S2-honest-free-tier** (pending) — make the dead z_image/Local tier show an honest unavailable
  state instead of "$0.00 / Generate" that fails at submit. Independent; no fal key.

## Open follow-ups (logged, not blockers)
- F4 (HIGH) retry_job command + stalled UI surfacing (must reset created_at/poll_cycles on retry).
- F1b bounded per-provider resume_all drain.
- dep-bump h2>=0.4.16, rustls>=0.23.45 (RUSTSEC-2026-0258 / -0285).
- minor: stalled-advisory "after 1 attempts" pluralization → fold into F4.
