# S20-workhorse-billing-accuracy — IMPLEMENT (claim)

Author: implementer agent. Built exactly the S20 design (sha256 8bda66ab…), owner
defaults O1–O4 as stated, all design-verify non-blocking notes carried. WORKHORSE =
`minimax_h3_max_turbo`; Seedance/`PerToken` untouched. No new dependency, no migration.

Every command below ran with the money-safety preamble:
`export HICKEYFIELD_NO_KEYCHAIN=1 hickeyfield_FAL_KEY=x hickeyfield_HIGGSFIELD_KEY=x hickeyfield_GOOGLE_KEY=x hickeyfield_OPENAI_KEY=x hickeyfield_XAI_KEY=x hickeyfield_BFL_KEY=x hickeyfield_RECRAFT_KEY=x hickeyfield_VAIG_KEY=x`
`HICKEYFIELD_ALLOW_SPEND` never set; `first_light` never run; the keyless audit is the
one sanctioned `env -u` command. All new tests are hermetic (pure functions over
registry/bundled snapshot; no sockets).

Path note (design-verify #1): the snapshot lives at
`crates/hickeyfield-core/vendor/prices-snapshot.json`, and it contains BOTH
`vaig:minimax/minimax-h3` and `vaig:xai/grok-imagine-video-1.5` — neither is the
fixture route id (`fal:minimax/h3-max-turbo`, `fal:xai/grok-imagine-video/v1.5`), so
both fixtures settle via the **registry literal**, as designed.

## Files changed (with one-line why)

Core (`crates/hickeyfield-core/`):
- `src/route.rs` — new `PromoRate { as_of: String }` struct + serde-default `promo:
  Option<PromoRate>` field on `Route` + `Route::promotional(..)` ctor; the three existing
  ctors (`new`, `noted`, `floored`) set `promo: None` (design-verify #3). (A3)
- `src/lib.rs` — re-export `PromoRate` alongside `Route`.
- `src/registry.rs` — the two H3 tiers swap `Route::noted` → `Route::promotional(…,
  "2026-09-16")`; both notes gain the dated observation `(measured 2026-09-18: 6 s
  requested → 6.592 s delivered)`. No `CostModel` literal value changed. (A2 note, A3)
- `src/cost.rs` — new number-free const `OUTPUT_SECONDS_NOTE` appended to the `basis` of
  `PerSecondTiered` and `PerSecond`; `usd` unchanged. (A2)
- `src/prices.rs` — new `PROMO_STALE_DAYS = 30`, `PromoRecheck` struct, `promo_recheck(today)`
  sibling of `audit()` (injected clock), private `days_from_civil` (Hinnant, no date crate)
  + `parse_iso_date`. (A3)

Tauri (`src-tauri/`):
- `src/commands.rs` — `RouteDto.promo_as_of: Option<String>`, set from `r.promo` at the one
  `RouteDto {…}` site (design-verify #4: single construction site). (A3 surface)
- `src/runner.rs` — A1/A1-i2v/B1 fixture tests only (no code change to the settle path).

UI (`ui/`):
- `src/types.ts` — `Route.promoAsOf?: string`.
- `src/api.ts` — `RawRoute.promo_as_of`/`promoAsOf` + map to `promoAsOf` in `toRoute`.
- `src/components/ModelPicker.tsx` — additive `<span className="route-promo">` inside the
  existing `.chip-route` button; test-locked selectors (`.model-item*`, `.chip-route`,
  STATUS.md:118) untouched (O4). (A3 surface)
- `src/components/picker.css` — additive `.chip-route .route-promo` style.
- `src/mock.ts` — optional `promoAsOf` on the `route()` helper + one demo value so browser
  mode renders the badge (not load-bearing; real markers live in the registry).
- `src/components/MetaCard.test.tsx` — A4 workhorse-valued delta test (no component change).

## Named test per AC + how to re-run

Preamble as above; from the worktree root.

- **A1** `runner::tests::workhorse_768p_settles_on_output_seconds` — 6 s asked, probed
  6.592 s @ 768p ⇒ `actual_usd == 0.13184 ±0.001`, distinctly NOT 0.12; `actual_source ==
  Some(Probe)`.
  **A1 (i2v)** `runner::tests::workhorse_i2v_prices_the_same_output_seconds` — same with a
  `MediaRole::Start` ref ⇒ identical 0.13184, Probe.
  `cargo test -p hickeyfield-tauri --lib workhorse_768p_settles_on_output_seconds workhorse_i2v_prices_the_same_output_seconds`
- **A2** `cost::tests::per_second_basis_states_output_seconds` (both per-second shapes carry
  the clause, `usd` unchanged, no `6.592` in the generic basis) and
  `registry::tests::both_h3_routes_state_output_seconds_in_the_basis` (both H3 routes'
  estimate basis states it).
  `cargo test -p hickeyfield-core per_second_basis_states_output_seconds both_h3_routes_state_output_seconds_in_the_basis`
- **A3** `registry::tests::the_h3_tiers_carry_a_dated_promo_marker` (both H3 tiers
  `promo == Some(as_of "2026-09-16")` + note carries the dated observation; every other
  route `promo == None`), `prices::tests::promo_recheck_flags_a_stale_marker_with_an_injected_clock`
  (34 d flags both, 4 d empty), `prices::tests::promo_recheck_boundary_is_exactly_thirty_days`
  (design-verify #2: 30 d NOT stale, 31 d stale), `prices::tests::only_promotional_routes_appear_in_the_recheck`
  (+ `days_from_civil` reference values), `commands::tests::route_dto_carries_the_promo_as_of`
  (workhorse DTO `promo_as_of == Some("2026-09-16")`; a non-promo route `None`).
  `cargo test -p hickeyfield-core promo_recheck the_h3_tiers_carry_a_dated_promo_marker only_promotional_routes_appear_in_the_recheck`
  `cargo test -p hickeyfield-tauri --lib route_dto_carries_the_promo_as_of`
- **A4** `MetaCard.test.tsx > "workhorse settled job renders the est-vs-settled delta"` —
  est 0.12 / actual 0.13184 ⇒ Delta cell `"+$0.012"`, `data-tone="over"`, locked field
  order intact. `cd ui && pnpm test MetaCard`
- **B1** `runner::tests::grok_720p_settles_on_output_seconds` — grok i2v 720p, 5 s asked,
  probed 5.04 s ⇒ `0.7056 ±0.005`, Probe; and asserts NO quantity advisory fires (0.04 s <
  0.5 s, design C12). `cargo test -p hickeyfield-tauri --lib grok_720p_settles_on_output_seconds`
- **B2** existing `prices::tests::the_audit_is_hermetic_and_finds_no_literal_beyond_tolerance`
  + `no_bundled_quote_is_free` stay green; no `CostModel` literal value changed; `PerToken`
  untouched.

## Gate results (real tails)

1. `cargo test --workspace` — **OK**. `test result: ok. 770 passed; 0 failed; 16 ignored`
   (core lib) + `6` (audit_fal ex) + `2` (fal_diff ex) + `193 passed; 0 failed; 3 ignored`
   (tauri lib) + doc-tests 0. **Total 971 passed / 20 ignored** (baseline 961; +10 new Rust
   tests: A1×2, A2×2, A3×4 core-side + A3 commands×1… i.e. cost 1, registry 2, prices 3,
   commands 1, runner 3). 0 failed.
2. `cargo fmt --all --check` — **OK** (`FMT OK`).
3. `cargo clippy --workspace --all-targets -- -D warnings` — **OK**
   (`Finished dev profile … in 57.82s`, no warnings). One clippy nit fixed en route: a
   redundant `as i64` cast in `days_from_civil`.
4. `./scripts/lint-provenance.py` — **OK** (`PASS no third-party media hosts`; `PASS no
   shipped string matches the reference corpus (80015 shingles)`).
5. `./scripts/lint-spend.py` — **OK** (`lint-spend: ok (49 file(s) scanned).`).
6. `env -u hickeyfield_FAL_KEY -u FAL_KEY cargo run -p hickeyfield-core --example audit_fal`
   — **OK**, keyless. `UNREACHABLE … (0)`; `CHECKED … (70)` including all four H3 endpoints
   (`minimax_h3_max{,_turbo}/{text,image}-to-video`); `70 endpoint(s) audited`.
7. `cd ui && pnpm test` — **OK**. `Test Files 23 passed (23)`, `Tests 313 passed (313)`
   (baseline 312; +1 A4; `MetaCard.test.tsx (6 tests)`).
   `cd ui && pnpm build` — **OK** (`✓ built in 906ms`).

## Deviations from the design

None material. The design's optional `price_check.rs` `promo_recheck` printer was left
out (design-verify #5: explicitly not load-bearing, kept out of the gate). The one clippy
nit (redundant cast) was fixed; the value `days_from_civil` returns is unchanged and pinned
by the reference-value asserts (1970-01-01 → 0, 2026-09-16 → 20712).

## Known gaps / limitations (not fixed — by scope)

- Settled numbers stay `ActualSource::Probe` (re-priced), never labelled "billed" — the
  `Billed` variant / balance-delta truth is **S22**, out of scope (Q-R2 default).
- `promo_recheck` is a pure function with an injected clock; nothing calls it on a wall
  clock in the app yet (no scheduler wired). The re-check is proven, not surfaced live —
  surfacing is not in the A3 contract (A3 only requires the emit + injected-clock proof).
- The picker `.route-promo` badge and the `mock.ts` demo value are cosmetic (O4); the
  load-bearing A3 UI proof is the Rust DTO test, not a vitest.
- `days_from_civil` is valid for the proleptic Gregorian calendar; `parse_iso_date` accepts
  day 1–31 without per-month validation — a registry typo like `2026-02-30` would parse but
  the registry authors real dates, so this is a deliberate non-guard, not a bug.

## Repair round 1 (post-REVIEW non-blocking fixes)

REVIEW passed (no blockers). This round fixes only its five non-blocking findings. No
`CostModel` value changed, no new dependency, no Seedance added, test-locked selectors
untouched.

### Fixes
1. **picker.css:331 dangling `--radius-sm`** → `border-radius: var(--radius-100);`
   (`--radius-100 = 0.25rem`, confirmed `tokens.css:153`). No fallback needed now.
2. **picker.css:332 undefined `--surface-raised`** → `background: var(--bg-secondary);`
   (confirmed `tokens.css:83`; matches the sibling `.model-list` fill at `picker.css:235`).
3. **prices.rs:2720 comment "20713 days"** → "20712 days" (the assert at `:2722` already
   reads `20712` and is untouched).
4. **mock.ts demo promo on the wrong model + roster did not lead with the workhorse**
   (`ui/src/mock.ts`):
   - Removed `promoAsOf: "2026-09-16"` from `kling_3_0`'s fal route (`route("fal",
     "kling-video/v3/standard")` now has no promo — kling has no real promo).
   - Added `minimax_h3_max_turbo` as the FIRST model (R0.3), modelled on the registry entry
     (`fal:minimax/h3-max-turbo`), carrying `promoAsOf: "2026-09-16"` on its fal route.
     `minimax_h3_max` is not in the mock roster, so only the turbo was added.
   - Added `minimax_h3_max_turbo: 0.02` to `RATE_PER_SECOND` so browse-mode estimate is
     non-null (768p workhorse rate).
   - No vitest depends on `MOCK_MODELS` order (only `mock.ts`/`api.ts` reference it; the
     Storyboard/variants/presets tests use their own local `kling_3_0` fixtures). `mockPlan
     Storyboard` picks the first video model, now the workhorse — no assertion keys on it.
     All 313 UI tests stay green with no assertion weakened.
5. **price_check.rs — added `promo_recheck_section()`** (`crates/hickeyfield-core/examples/
   price_check.rs`), a human-reachable caller for `promo_recheck` alongside `audit()`. It
   reads the real wall clock ONLY in the example via `today_civil()` (inlined Hinnant
   `civil_from_days`, no date crate); the library `promo_recheck(today)` still takes its
   injected `today`. Keyless and offline (registry-only, no network) — safe to run. Printed
   under the audit sections in `main()`.

### Gate output (all pass; preamble sets dummy keys + `HICKEYFIELD_NO_KEYCHAIN=1`)

`cargo fmt --all --check` → FMT_OK.

`cargo clippy --workspace --all-targets -- -D warnings`:
```
Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.57s
```

`cargo test --workspace` → 971 passed (770 + 6 + 2 + 193), 0 failed, 20 ignored:
```
test result: ok. 770 passed; 0 failed; 16 ignored; 0 measured; 0 filtered out
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test result: ok. 193 passed; 0 failed; 3 ignored; 0 measured; 0 filtered out
```

`cargo run -p hickeyfield-core --example price_check` (keyless/offline):
```
=== PROMO RE-CHECK (older than 30 days as of 2026-09-19) (0) ===
  none
MISMATCH 0 / OK 11 / UNMODELLABLE 0 / PROSE 50 / UNPUBLISHED 8 / NO-SNAPSHOT 1
```
(as-of 2026-09-16 is 3 days old vs today 2026-09-19 → not stale, correct; MISMATCH still 0.)

`./scripts/lint-provenance.py` → PASS. `./scripts/lint-spend.py` → `ok (49 file(s) scanned)`.

`env -u hickeyfield_FAL_KEY -u FAL_KEY cargo run -p hickeyfield-core --example audit_fal`:
```
70 endpoint(s) audited against fal's own schema.
```

`cd ui && pnpm test && pnpm build`:
```
Test Files  23 passed (23)
      Tests  313 passed (313)
✓ built in 993ms
```

### Not done / limitations
- None. All five non-blocking findings fixed; nothing else touched. Finding #5 in REVIEW
  (`parse_iso_date` per-month day validation) was explicitly "no fix required".
