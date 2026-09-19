# S20-workhorse-billing-accuracy — RELEASE
**Gate:** `python3 .forge/gate.py S20-workhorse-billing-accuracy` exit 0. DESIGN verified (sha256 8bda66ab…),
TEST + REVIEW + SECURITY = PASS; ONE repair round (REVIEW's non-blocking findings), then TEST + REVIEW
re-verified the delta (REVIEW carried the security delta: clean). Distinct agents per phase.
**Shipped (workhorse first — `minimax_h3_max_turbo`, $0.02/s at 768P):** per-second video quotes now say
they bill on OUTPUT seconds (number-free clause); the settle path is proven to price the DELIVERED
duration (6 s asked → 6.592 s → $0.13184, t2v + i2v; `grok_video_v15` 5 s → 5.04 s → $0.7056), source
`Probe` = re-priced, never "billed"; both H3 routes carry a structured promotional-rate marker
(as of 2026-09-16) surfaced as `RouteDto.promo_as_of` + picker badge; `prices::promo_recheck(today)`
flags it after 30 days (injected clock; printer in `examples/price_check`); est-vs-settled delta on the
job card proven with the workhorse fixture; mock data leads with the workhorse.
**Scope change (owner, 2026-09-19):** "we're not using seedance... ever" — the per-token half of S20 was
DROPPED; objective R1.1 + backlog amended. No `CostModel` value changed; `PerToken` untouched.
**Regression:** cargo test --workspace 971 / 0 (+20 ignored, all pre-existing gated tests; slice added
none); fmt · clippy -D warnings · provenance · lint-spend clean; keyless audit_fal UNREACHABLE 0 / 70;
pnpm test 313 / build ok. Mutation check: settling on requested seconds turns A1 + B1 RED.
**Money:** nothing spent. Optional owner-run: one paid workhorse clip (~$0.13, 768P, 6 s) to live-confirm
the output-seconds settle — owner's call only.
**Follow-ups:** backlog "S20 follow-ups (2026-09-19)" (Seedance sweep of mock/defaults first).
**Landing:** commit on forge/s20-workhorse-billing-accuracy; ff to main (local fork only, never pushed),
`tauri build --bundles app`, ad-hoc codesign, reinstall to /Applications — on the owner's go.
