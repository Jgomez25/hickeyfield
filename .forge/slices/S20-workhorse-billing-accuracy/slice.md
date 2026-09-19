# S20-workhorse-billing-accuracy — SLICE (DISCOVER + PLAN)

Owner pick 2026-09-19 ("Start S20, defaults on Q-R2 and Q-R3"). Owns objective **R1.1** + the R-FIELD
lesson "bill on OUTPUT seconds, not requested". Branch `forge/s20-workhorse-billing-accuracy`.

**WORKHORSE = `minimax_h3_max_turbo`** (fal `minimax/h3-max-turbo/{text,image}-to-video`, $0.02/s at
768P, native audio + lipsync, bills on OUTPUT seconds). Seedance is NEVER used (owner, 2026-09-19). Every
default, fixture, example and test in this slice leads with the workhorse.

**SCOPE CHANGE (owner, 2026-09-19, mid-slice): "we're not using seedance... ever. too expensive."**
Per-token pricing is Seedance-family only, so the R1.1 per-token table and the $1.95→$6.18 case are
DROPPED from this slice and from the contract (objective R1.1 amended). Also moot: the HANDOFF pointer to
that job (`1789605336-seedance_2_5-0`) does not resolve — the app DB holds 5 rows, none Seedance.
Owner directed the slice at his private cost-workflow notes (outside the repo; restated, never copied): paid video = workhorse i2v ~$0.02/s; Grok i2v is web-sandbox-only because the API
is ~7× dearer (~$0.14/s); free-sandbox finals, turbo drafts, confirm every paid run.

## Delivers (one sentence, end-to-end)
A workhorse clip's quote says it bills on output seconds and carries a dated promotional-rate flag, its
settled cost is priced from the delivered duration (6 s asked → 6.592 s → $0.13) with the est-vs-settled
delta on the job card — and the same output-seconds rule is proven on the formula's other video route.

## Acceptance criteria (testable — the tester will run these)
Part A — workhorse (do FIRST)
- [ ] **A1 output-seconds settle.** Hermetic: a `minimax_h3_max_turbo` 768P job requested at 6 s whose
      probed output is 6.592 s settles `actual_usd` = 0.02 × 6.592 = **$0.13184 ± $0.001**, not $0.12.
      Same for i2v. `actual_source` stays a re-priced source (Probe), never labelled billed.
- [ ] **A2 estimate states the assumption.** The estimate `basis` for both H3 routes says billing is on
      OUTPUT seconds and that delivered clips can run longer than requested. No invented overrun
      constant: any number shown is the measured, dated observation (6 s → 6.592 s, 2026-09-18).
- [ ] **A3 dated promotional flag + re-check.** `minimax_h3_max` and `minimax_h3_max_turbo` tiers carry
      a structured promotional-rate marker with its as-of date (2026-09-16), surfaced to the UI, and
      `prices::audit` (or its sibling) emits a re-check row once the marker is older than a stated
      threshold — proven with an injected clock, no network.
- [ ] **A4 delta on the card.** vitest: a settled workhorse job with estimated $0.12 / settled $0.13184
      renders the est-vs-settled delta on the job card; test-locked selectors intact.
Part B — the formula's other paid video route (second)
- [ ] **B1 output-seconds settle is general.** The A1 rule holds for every `PerSecond` /
      `PerSecondTiered` video route, not only H3. Second measured fixture: `grok_video_v15` i2v, 720p,
      5 s asked → 5.04 s probed settles **$0.7056 ± $0.005** (owner-measured $0.71, balance delta
      60.59→59.88, 2026-09-18). Hermetic.
- [ ] **B2 no collateral.** No `CostModel` literal changes value without a cited source; `prices::audit`
      and the 10×/1.2× guards stay green. `PerToken` is NOT touched (see scope change above).
Gates (every slice)
- [ ] **G1** `cargo test --workspace`, `cargo fmt --all --check`, `cargo clippy --workspace
      --all-targets -- -D warnings`, `./scripts/lint-provenance.py`, `./scripts/lint-spend.py`, keyless
      `audit_fal`, `cd ui && pnpm test && pnpm build` all pass.
- [ ] **G2 spend hygiene.** Every command ran with `HICKEYFIELD_NO_KEYCHAIN=1` + dummy
      `hickeyfield_*_KEY=x` SET; `HICKEYFIELD_ALLOW_SPEND` never set; `first_light` never run; no test
      opens a socket (fixtures + the S18 fake-provider seam).

## Owner-run (agents never spend)
- One paid workhorse clip (~$0.13, 768P, 6 s) to live-confirm A1. Optional; state cost and get a go first.
- No Seedance purchase, ever. No Grok API purchase (sandbox-only per the owner's formula).

## Approach + why this slice now
Money trust is the owner's first priority and S20 has no deps; it extends the S10 settled-actual path
(`cost.rs`, `prices.rs`, `registry.rs` priced_routes ~:1158-1180, `runner.rs` actual write, MetaCard
delta). Q-R2 default stands: settled numbers here are RE-PRICED (Probe/OutputCount), not billed truth —
that distinction lands in S22. Out of scope: ranges + free-sandbox hint (S24), calibration store (S23),
image tiers (S21 — the formula's hero-still route, next most valuable), anything per-token.
