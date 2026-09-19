# S20-workhorse-billing-accuracy — DESIGN VERIFY

- ARTIFACT: `.forge/slices/S20-workhorse-billing-accuracy/design.md`
- ARTIFACT-HASH (`shasum -a 256`, now): `8bda66ab9d11d1c8c7bda9acb6c7a5958946bf681e61a8a0a95ed0a02c0bfa63`
- SOURCES OF TRUTH:
  - `.forge/slices/S20-workhorse-billing-accuracy/slice.md` (ACs A1–A4, B1–B2, G1–G2)
  - `.forge/objective.md` §R0.3, §R1 OWNER AMENDMENT 2026-09-19, §R-FIELD, §R-RULES
  - Code read first-hand this session (file:line below)
- ROLE: I did not author this design and did not fix it. I try to FAIL it. Read-only inspection
  only (no `cargo`/`pnpm` run — per invocation rules; the gate belongs to the implementer). Every
  Bash command carried the `HICKEYFIELD_NO_KEYCHAIN=1` + dummy-key preamble; `HICKEYFIELD_ALLOW_SPEND`
  never set; `first_light` never run.

## Scripted / read-only checks (real output)

- `shasum -a 256 design.md` → `8bda66ab…bfa63` (pinned above). Exit 0.
- `find . -name prices-snapshot.json` → `./crates/hickeyfield-core/vendor/prices-snapshot.json`
  (the design cites the path as `vendor/prices-snapshot.json`; the file actually lives under
  `crates/hickeyfield-core/vendor/`). Exit 0.
- `grep '"fal:' crates/hickeyfield-core/vendor/prices-snapshot.json | grep -i 'minimax|grok|h3'` →
  **empty**. The only snapshot entries touching these models are `vaig:minimax/minimax-h3` (:496)
  and `vaig:xai/grok-imagine-video-1.5` (:538). Neither fixture route id
  (`fal:minimax/h3-max-turbo`, `fal:xai/grok-imagine-video/v1.5`) is present → both settle via the
  registry literal. LOAD-BEARING CLAIM CONFIRMED. Exit 0/1 (grep no-match).
- `grep -n basis … | grep assert/==/toBe/toEqual` across `cost.rs`, `prices.rs`, `registry.rs`,
  `src-tauri`, `ui/src` → every core-`basis` assertion is `.contains(...)` (cost.rs:386,400,579;
  prices.rs:1718; registry.rs:2809,3517). No exact-equality assertion on the core estimate `basis`.
  The two `ui/src/.../cost.test.ts` lines feed their OWN `basis` literal as input, not core output.
  A2's appended clause cannot break any of them. Exit 0.
- No `deny_unknown_fields` on `Route`/`RouteDto` (`grep -rn deny_unknown_fields crates src-tauri`
  → empty). Adding a serde-`default` `promo` field is back-compatible. Exit 1 (no match).
- No date crate in the workspace (`grep chrono|\btime\b` in Cargo.tomls/deny.toml → only tokio's
  `"time"` FEATURE, not the `time` crate). Hand-rolled `days_from_civil` justified under R-RULES.4.

## Checklist

- [PASS] C1 (A1 output-seconds settle, workhorse): the settle machinery exists and prices the probed
  duration at the route rate — quote: "compute_actual already prices the settled billable from probed
  dims at the registry rate". Verified: `runner.rs:248-259` writes `actual_usd`/`actual_source` once
  when `None`; `compute_actual` (`runner.rs:328-355`) calls `settled_billable` (overwrites
  `b.seconds/width/height` with probed `MediaDims`) then `prices.estimate`; `ActualSource::Probe` set
  when `dims.seconds.is_some() || dims.width.is_some()`. `PerSecondTiered` selects `height <= max_h`
  (`cost.rs:186-190`), so 768p→(768,0.02); `0.02×6.592 = 0.13184`. Math and tier correct.

- [PASS] C2 (768P maps to the 768 tier): quote: "`height=768` selects the `(768,0.02)` tier".
  `commands::resolution_height("768P") → Some(768)` (`commands.rs:627`, test `:2414`); `768 <= 768`
  hits the middle tier literal `(768,0.02)` (`registry.rs:1181`).

- [PASS] C3 (fixtures resolve via registry literal, not a feed/snapshot quote): quote: "neither
  `minimax/h3-max*` nor `xai/grok-imagine-video/v1.5` is in the snapshot → literal is used".
  `Prices::estimate` is feed→literal (`pricing.rs:100-104`: `feed.cost_model(route.id()).…or_else(||
  model.estimate(route,b))`). Snapshot holds no `fal:` minimax/grok/h3 entry (grep empty). Fixture
  route ids differ from the snapshot's `vaig:` keys, so `feed.cost_model` misses → registry literal.
  Note: design line 26 says the snapshot grep shows "only `vaig:xai/grok-imagine-video-1.5`"; it in
  fact ALSO contains `vaig:minimax/minimax-h3`, but that entry keys on a different route id and does
  not affect either fixture. Non-blocking imprecision; the conclusion holds.

- [PASS] C4 (settled numbers re-priced, never labelled billed — S22 boundary): quote: "`actual_source
  = Probe` (a dim was probed), never `Billed`". `ActualSource = {Probe, OutputCount, Provider}`
  (`cost.rs:356-364`) — no `Billed` variant. Consistent with Q-R2 default / R0.3.

- [PASS] C5 (A2 estimate states the assumption, no invented constant): quote: "The clause MUST carry
  **no number** (no invented overrun constant)". Append targets the real basis builders
  (`PerSecondTiered` `cost.rs:189`, `PerSecond` `cost.rs:209-215`). The only number sanctioned is the
  measured dated observation (6 s → 6.592 s, 2026-09-18), routed to the H3 note text, matching slice
  A2. No overrun multiplier — consistent with Non-goal "Overrun multiplier constant — deliberately
  none".

- [PASS] C6 (A2 no test breakage / reaches UI): quote: "Existing assertions are `contains(...)` …
  so they stay green". Confirmed by grep (all `.contains`). UI path `basis → CostEstimate.basis →
  costBasisNote → GenerateButton note` matches `cost.ts` + `MetaCard`/`GenerateButton` render; additive.

- [PASS] C7 (A3 dated structured promo marker + stale re-check, injected clock): quote: "Emit a
  re-check row for every promotional-rate route whose marker is older than `PROMO_STALE_DAYS` as of
  `today`". `Route` has the `min_billed_seconds` serde-`default`/`skip_serializing_if` precedent
  (`route.rs:37-38`) the new `promo` field mirrors; no `deny_unknown_fields` → back-compat holds.
  Both H3 tiers use `Route::noted(..., "…promotional launch rate (fal index 2026-09-16)")`
  (`registry.rs:1168,1178`), so `as_of = "2026-09-16"` is faithful. Clock is a parameter → hermetic.
  Boundary `> PROMO_STALE_DAYS` (30) is unambiguous: 30 d does not flag, 31 d flags.

- [PASS] C8 (A3 no new dependency / date math): quote: "Howard Hinnant's ~5-line algorithm — no date
  crate, satisfies R-RULES.4 no-new-dep". Confirmed: no `chrono`/`time` crate in the workspace; a
  hand-rolled civil-day fn is the only no-dep option. Size-S preserved.

- [PASS] C9 (A3 surface to UI preserves test-locked selectors): quote: "an additive `<span
  className=\"route-promo\">` inside the existing route line". STATUS.md:118 locks `.model-item
  .model-item-name .model-item-main .chip-route` — the design keeps all four and adds `.route-promo`;
  `RouteDto` has a single struct literal (`commands.rs:469`) so the additive `promo_as_of` field
  touches exactly one construction site (build stays green).

- [PASS] C10 (A4 delta on card, workhorse-valued): quote: "`formatDelta` → `formatUsd(0.01184)` =
  `\"$0.012\"` … → **`\"+$0.012\"`**". Verified against code: `costDelta(0.12, 0.13184)` → diff
  `0.01184`, sign `over` (`cost.ts:80-92`); `formatUsd(0.01184)` hits the `>= 0.01` branch → `toFixed(3)`
  → `$0.012` (`cost.ts:22-27`); `formatDelta` → `+$0.012` (`cost.ts:95-99`). `MetaCard` renders
  `.meta-card-delta` with `data-tone` when both figures present (`MetaCard.tsx:57,164-171`). Additive test.

- [PASS] C11 (B1 general output-seconds settle, grok fixture): quote: "`grok_video_v15`, route
  `fal:xai/grok-imagine-video/v1.5`, requested 5 s @ 720p, probed … `0.14 × 5.04 = 0.7056`". Literal
  `PerSecondTiered {tiers: [(480,0.08),(720,0.14)]}` at `registry.rs:1116-1120`; `720 <= 720` →
  `(720,0.14)`; route id absent from snapshot → literal. Math correct.

- [PASS] C12 (behavioural: quantity advisory boundary): quote: "5.04 vs 5.0 is 0.04 s < 0.5 s … does
  **not** trip the quantity advisory". `runner.rs:411` = `(got - asked).abs() > 0.5`; workhorse 0.592 s
  trips, grok 0.04 s does not. Correct — recorded so the tester expects it.

- [PASS] C13 (B2 no collateral, PerToken untouched): quote: "This slice changes **no `CostModel`
  literal value**". A2 edits `basis` display strings only; A3 adds a field + note text + a new fn.
  `audit()` compares literal-vs-feed prices, not `basis` → `Mismatch` count unaffected;
  `AUDIT_TOLERANCE = 1.2` (`prices.rs:1421`). `PerToken`/Seedance not read or changed.

- [PASS] C14 (every AC has a named hermetic test): quote: "one named test per AC (all hermetic…)".
  The test table (design §Test plan) names one+ test per A1–A4/B1–B2 with file and asserts; all call
  pure functions over registry/snapshot, no sockets — consistent with G2.

- [PASS] C15 (Non-goals present): quote: "**`ActualSource::Billed` / balance-delta truth — N/A
  (S22).**" Full §Non-goals lists S21/S22/S23/S24/S25 boundaries + no-overrun-constant + no-new-dep.
  No downstream slice is invoked as if in scope.

- [PASS] C16 (Open questions present): quote: "**O2 — promo stale threshold?** DEFAULT: **30 days**".
  O1–O4 each carry an assumed default. Threshold justified against objective R1.7 (`>30 d` staleness).

- [PASS] C17 (referenced paths/interfaces exist or marked new): `audit_fal.rs` and `price_check.rs`
  exist under `crates/hickeyfield-core/examples/`; `compute_actual`/`settled_billable`/`quantity_advisory`
  (`runner.rs`), `ActualSource` (`cost.rs`), `RouteDto` (`commands.rs`), `MetaCard`/`cost.ts` all
  present. New items (`PromoRate`, `Route::promotional`, `promo_recheck`, `promo_as_of`, `PROMO_STALE_DAYS`,
  `days_from_civil`) are explicitly marked as additions.

- [PASS] C18 (no private brand/person names): design uses no private brand/person names; the owner's
  private cost-workflow notes are referenced as external, never copied.

## Non-blocking notes for the implementer (carry forward)

1. Path imprecision: the design cites `vendor/prices-snapshot.json`; the file is
   `crates/hickeyfield-core/vendor/prices-snapshot.json`. Also line 26's grep is stated as showing
   "only `vaig:xai/grok-imagine-video-1.5`" — the snapshot ALSO contains `vaig:minimax/minimax-h3`.
   Neither changes the load-bearing conclusion (both fixture routes resolve via the registry literal),
   but fix the wording in `implement.md` so the tester greps the right path.
2. The A3 named test pins two dates well off the boundary (34 d and 4 d). The `> PROMO_STALE_DAYS`
   off-by-one (exactly 30 d not-stale vs 31 d stale) is NOT exercised by those dates. Add a boundary
   case at exactly 30 and 31 days so the hand-rolled `days_from_civil` off-by-one is caught.
3. When adding the `promo` field to `Route`, remember every existing ctor (`new`, `noted`, `floored`)
   must set `promo: None`, or the build breaks — the design says so for `new`/`noted` but not `floored`
   (`route.rs:63-70`); include it.
4. `.meta-card-delta`/`.meta-card-fields` are exercised by the existing `MetaCard.test.tsx` but are
   not listed in STATUS.md:118 (which locks only the picker `.model-item*`/`.chip-route` names). The
   A4 change is additive so this is fine; just do not assume STATUS.md guards the MetaCard selectors.
5. G1 step 6 uses `env -u hickeyfield_FAL_KEY -u FAL_KEY … --example audit_fal`; `audit_fal.rs` exists.
   The optional `promo_recheck` printer in `price_check.rs` is explicitly "not load-bearing" — keep it
   out of the gate assertions.

## Root cause

None — no blocking finding. The three author-flagged risks were attacked and all hold:
(1) the A1/B1 settle path genuinely exists and prices from the PROBED duration via the registry
literal for both fixtures (feed/snapshot cannot win — no matching `fal:` snapshot entry, and 768P
maps to the 768 tier); (2) the 30-vs-31-day boundary is specified unambiguously as `> PROMO_STALE_DAYS`
with an injected clock, and no workspace date crate exists to make the hand-roll avoidable; (3) no
exact-equality `basis` assertion exists that the appended A2 clause would break. Serde back-compat,
one-per-AC hermetic tests, preserved test-locked selectors, size-S, and no new dependency all check
out. Non-blocking notes above are quality carries, not gate failures.

VERDICT: PASS
