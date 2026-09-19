# S20-workhorse-billing-accuracy — DOCUMENT

## What shipped (user-facing)
- Every per-second video quote now states it bills on **delivered output seconds**, not the
  requested seconds, and that a clip can run longer than asked (generic clause on the `PerSecond`
  and `PerSecondTiered` basis; reaches the Generate-button note unchanged).
- A completed job's settled cost is **re-priced from the file's measured duration** at the same
  route rate and the job card shows the estimate-vs-settled delta. Proven on the workhorse
  (`minimax_h3_max_turbo`, 6 s → 6.592 s @ 768p ⇒ $0.13184, not $0.12) and on `grok_video_v15`
  (5 s → 5.04 s @ 720p ⇒ $0.7056). Settled figures stay `ActualSource::Probe` — re-priced, not a
  provider invoice; true billed reconciliation is S22.
- The two H3 tiers carry a **dated promotional-rate marker** (`as_of` 2026-09-16), surfaced as a
  small "promotional rate" badge in the model picker, plus a **30-day stale re-check**
  (`prices::promo_recheck`, injected clock) reachable from the keyless
  `cargo run -p hickeyfield-core --example price_check`. The marker flags marker age only; it
  cannot detect an actual provider price change.

## Docs changed (file + heading)
- `CHANGELOG.md` › `[Unreleased]` › `### Added` — two new lead entries (per-second output-seconds
  clause + re-priced settle with the workhorse/Grok figures; dated promo marker + picker badge +
  30-day stale re-check; test counts 961→971 Rust, 312→313 UI). Placed above the S18 spend-gate
  entry, original wording.
- `docs/FIRST-LIGHT.md` › new `## How a per-second video estimate settles (output seconds,
  promotional rate)` subsection, inserted directly before `## Verifying prices against the live
  fal API`. Brief: output-seconds billing, the workhorse/Grok worked examples (re-priced, not
  billed), and the dated promo marker + monthly re-check.
- `.forge/backlog.md` — new `### S20 follow-ups (2026-09-19)` heading after the S18 follow-ups
  block (see Follow-ups below).
- `README.md` — N/A, no user-facing flag or new command; the price behaviour is documented in
  FIRST-LIGHT.md where the existing cost-accuracy and price-audit material already lives.

## Owner decisions recorded this slice
- **Seedance is never used** (owner, 2026-09-19, "too expensive"). S20's per-token half (the
  R1.1 per-token table and the $1.95→$6.18 case) was dropped from the slice and the contract;
  `PerToken` was not read or changed. No Seedance example appears in any doc.
- Design defaults O1–O4, as built:
  - **O1** — the output-seconds clause is **generic** across all per-second video routes (not
    H3-only), because output-seconds billing is a property of every per-second route.
  - **O2** — promo staleness threshold is **30 days**, matching the project's existing `>30 d`
    staleness convention and the monthly feed cadence.
  - **O3** — `promo.as_of` is an **ISO `YYYY-MM-DD` string** (readable in the registry; no
    serde-size cost since the snapshot stores no `Route`).
  - **O4** — the promo marker is a **`RouteDto.promo_as_of` field + a one-line additive picker
    badge** (`.route-promo`); the load-bearing proof is the Rust DTO test, the badge is cosmetic
    and touches no test-locked selector.

## Follow-ups
- Remove pre-existing Seedance entries (e.g. `seedance_2_0`) from `ui/src/mock.ts` demo data and
  sweep defaults/samples for any remaining Seedance, per the owner's "never" rule.
- The mock uses a flat 0.02/s for the tiered workhorse route (`RATE_PER_SECOND`) — demo-only, not
  the real 480/768/1080 tiers.
- `parse_iso_date` accepts day 1–31 without per-month validation (panic-free; registry literals
  only, so a bad literal like `2026-02-30` would parse — deliberate non-guard, not a bug).
- `promo_recheck` is not surfaced in the app UI (example printer only); live price
  re-verification belongs with S22.
- The HANDOFF pointer to a settled Seedance job did not resolve in the app DB — moot after the
  owner's "Seedance never" decision.
- Optional owner-run (agents never spend): one paid workhorse clip (~$0.13, 768p, 6 s) to
  live-confirm the output-seconds settle. Owner's call.

## Evidence (verifier writes verdicts)
TEST / REVIEW / SECURITY verdicts and the repair round live in this directory. Per implement.md:
`cargo test --workspace` 971 passed / 0 failed / 20 ignored; fmt · clippy · provenance ·
lint-spend clean; keyless `audit_fal` (70 endpoints) unchanged; `cd ui && pnpm test` 313 passed
and `pnpm build` ok. Nothing spent (`HICKEYFIELD_ALLOW_SPEND` never set; `first_light` never run).
