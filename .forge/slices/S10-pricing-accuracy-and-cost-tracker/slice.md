# S10-pricing-accuracy-and-cost-tracker — SLICE (DISCOVER + PLAN)

## Delivers (one sentence, end-to-end)
Every fal route's price estimate is verified against fal's own published rate (a keyless,
hermetic audit that fails the build on a wrong literal), the two estimator bugs that quote $0.00 or
skip fal's 15-second floor are fixed, and each finished job records an actual cost derived from the
settled output (unit price × measured quantity) so the user sees estimated vs actual per job and a
running spend that is no longer just a sum of guesses.

## Ground truth (verified 2026-09-16 by explore + S8/S9 evidence; memory `fal-cost-apis`)
- Owner: "many prices are wrong; add a verifier". `actual_usd` is hardcoded `None` in both
  clients (`clients.rs` ~:241/:395); `apply_poll` only writes `Some`; `engine.rs` apply_poll
  early-return (~:423-427) would drop a cost-only delta. UI `MetaCard.tsx:140-154` always shows
  "Not reported"; spend meter (`MetaRail.tsx:26-43`, `lib/cost.ts:87`) = estimate sum;
  `price_status` command registered (`lib.rs:80`) with zero UI callers.
- Price sources: the live feed prices only ~5 of ~57 fal routes (`prices.rs::fal::parse_prose`
  accepts 5 sentence templates, ~:819-841); ~40 fal routes are hardcoded literals in
  `registry.rs::priced_routes`; some `Unknown`. Feed wins over literal (`src-tauri/src/pricing.rs:95-100`).
  Bundled `vendor/prices-snapshot.json` = 23 quotes (18 vaig, 5 fal). Only guard: `prices.rs:1483`
  test, 10× tolerance over the 23 bundled quotes. NO test compares a literal to fal's index
  `pricingInfoOverride` — yet `fal_catalogue.rs::parse_pricing` (~:1171, `Model::pricing()` ~:430)
  already reduces 193 of the 1,491 snapshot models to a `Rate` and is used only by `fal_diff`
  printing. `examples/price_check.rs` (~:44-87) already computes literal-vs-live diff — unwired.
- Estimator bugs: `cost.rs:181/:203` `seconds.unwrap_or(0.0)` → confident $0.00 instead of None
  (violates "unknown never zero"); `with_minimum_seconds` (`cost.rs:262`, fal's 15s floor) defined
  but NEVER called → short clips under-quoted up to 3×. S9 added: `estimated_usd` is priced
  (`commands.rs ~:1013`) BEFORE `reconcile` may coerce a pinned cost-bearing field (backlog
  `pin-aware-cost-estimate`).
- fal APIs (need the user's key, never in chat; NOT to be called by any test): price lookup
  `GET api.fal.ai/v1/models/pricing?endpoint_id=` → unit_price + unit ∈ {image, megapixel, second,
  request, compute_second}; usage `GET /v1/models/usage` (ADMIN key, minute buckets, NO request_id);
  requests `GET /v1/models/requests/by-endpoint` → per request_id `duration`, NO cost. Queue result
  bodies carry no billing fields. ⇒ per-job actual = unit_price × measured billable quantity.
- Store: `estimated_usd` write-once (`store.rs:258-270`); no provenance columns; migrations exist
  (next is 7).

## Acceptance criteria (testable)
- [ ] AC1 — Hermetic price audit: `prices::audit()` (promoted from `examples/price_check.rs`) compares
  every fal route's `CostModel` literal against the vendored fal index `pricingInfoOverride`
  (`fal_catalogue::Model::pricing()` → Rate) for every route whose row parses to a Rate; a unit test
  FAILS on any mismatch beyond a tight tolerance (design picks, ≤1.2×) and prints the prose for
  rows that do not parse. Every mismatch found is fixed in `priced_routes` in this slice (each fix
  cites the fal figure). Runs with no key, no network.
- [ ] AC2 — Estimator honesty: `seconds.unwrap_or(0.0)` paths return `None` (unknown never $0.00);
  `with_minimum_seconds` applied for fal per-second routes with a floor (fal's 15s minimum where
  fal states it — design verifies which routes); tests for both.
- [ ] AC3 — Actual cost per job: on completion the app computes `actual_usd` from the settled output
  (duration/resolution/image count actually produced, probed from the downloaded file or the
  provider result) × the route's rate, writes it via `PollResult.actual_usd` → `apply_poll` (fix the
  cost-only early return), and persists it. Provenance columns (migration 7): `priced_from`
  (literal|feed|snapshot), `priced_at`, `actual_source`. Test: a completed job with a settled 10s
  output on a per-second route gets `actual_usd` = 10 × rate; a job whose output cannot be measured
  gets `None`, never 0.
- [ ] AC4 — Estimate re-check at completion: when the settled quantity differs from the requested
  one (S9 pin coercion, floor, provider rounding), the job carries an advisory with both figures.
- [ ] AC5 — UI: MetaCard shows estimated vs actual (and the delta) per job; MetaRail's spend meter
  shows actual where known and estimated otherwise, labelled; `price_status` surfaced (feed age /
  source). vitest tests for the card states (estimate only, actual known, delta).
- [ ] AC6 — Live reconciliation seam (no money spent by the build): a `--ignored` example/test
  that, given the user's key in the keychain/env, calls the fal pricing API for every fal route and
  diffs against the literals; documented in docs (human-run). Not run by agents.
- [ ] AC7 (regression) — `cargo test --workspace` 0 failed (>= 801), fmt, clippy -D warnings,
  provenance lint, `cd ui && pnpm test && pnpm build`; keyless audit_fal unchanged (PINNED 0,
  UNREACHABLE 0); CHANGELOG entry; app builds + launches.

## Approach + why now
Owner order: money before looks. The catalogue is now trustworthy (S8/S9); the prices on it are
not. Build the hermetic audit first (it finds the wrong literals for free), fix the estimator bugs,
then plumb actual cost from the settled output — the only per-job actual fal makes possible.
