# S20-workhorse-billing-accuracy — DESIGN

Contract: `.forge/slices/S20-workhorse-billing-accuracy/slice.md` (A1–A4, B1–B2, G1–G2).
Upstream: `.forge/objective.md` §R0.3, §R1 OWNER AMENDMENT 2026-09-19, §R-FIELD "bill on OUTPUT
seconds", §R-RULES. Built on the S10 settled-actual path
(`.forge/slices/S10-pricing-accuracy-and-cost-tracker/design.md`) and the S18 fake-provider seam
(`.forge/slices/S18-live-spend-gate/design.md`, `spend_gate.rs`). This artifact changes no source;
it specifies the implementer's edits. The verifier starts at §Riskiest.

**WORKHORSE = `minimax_h3_max_turbo`** (`fal:minimax/h3-max-turbo`, `PerSecondTiered
[(480,0.0125),(768,0.02),(1080,0.04)]`, `registry.rs:1174-1183`). Seedance is never used; `PerToken`
is not touched anywhere in this slice.

---

## 0. Session constraint & ground truth (read, not run)

Authored with read-only tools — no `cargo`/`pnpm` ran. Every `file:line` below was read from the
worktree this session and is first-hand. Because the implementer will run the gate, any figure I
predict is a hypothesis the run confirms; if a run disagrees, **the run wins** and the design is
amended in `implement.md`.

The single most important finding, up front: **the A1/B1 machinery already exists and works.** S10
shipped `compute_actual` + `settled_billable` + `ActualSource::Probe` (`src-tauri/src/runner.rs:320-400`,
called at `:248-254`), which price the settled output at the same route rate the estimate used.
Neither the workhorse nor grok appears in the bundled snapshot (grep of
`vendor/prices-snapshot.json`: only `vaig:xai/grok-imagine-video-1.5`), so both settle via the
**registry literal** `PerSecondTiered` through `Prices::estimate`'s literal fallback
(`src-tauri/src/pricing.rs:95-100`). So A1 and B1 are **satisfied in code today**; their real gap is
**proof** (named fixtures). A2 and A3 are genuine code gaps. A4 renders today (`MetaCard.tsx:57,164-171`);
its gap is a workhorse-valued vitest.

| Fact | Read at |
|---|---|
| `compute_actual` prices settled billable via `prices.estimate`; returns `ActualSource::Probe` when a dim was probed, `OutputCount` otherwise | `runner.rs:328-355` |
| the write site: `download_outputs` probes the first local output and writes `actual_usd`/`actual_source` once, iff currently `None` | `runner.rs:245-259` |
| `Prices::estimate` = feed quote → **registry literal fallback** (`model.estimate`) | `pricing.rs:95-100` |
| workhorse literal `PerSecondTiered [(480,0.0125),(768,0.02),(1080,0.04)]` | `registry.rs:1174-1183` |
| `grok_video_v15` fal literal `PerSecondTiered [(480,0.08),(720,0.14)]` | `registry.rs:1110-1120` |
| neither `minimax/h3-max*` nor `xai/grok-imagine-video/v1.5` is in the snapshot → literal is used | grep `vendor/prices-snapshot.json` |
| `PerSecond`/`PerSecondTiered` estimate `basis` strings say only "`{s}s x ${rate}/s at {h}p`" — **no** output-seconds assumption | `cost.rs:190,209-214` |
| the promo rate lives ONLY in a free-text `Route::noted(...)` string ("promotional launch rate (fal index 2026-09-16)") | `registry.rs:1168,1178` |
| `Route` already has an optional serde-default field precedent (`min_billed_seconds`) + a `floored()` ctor | `route.rs:37-70` |
| `RouteDto` carries `note` to the UI picker (title) | `commands.rs:148-162,473`; `ModelPicker.tsx:148` |
| estimate `basis` reaches the UI: `CostEstimate.basis` → `costBasisNote` → GenerateButton note | `api.ts:380,532`; `cost.ts:45-55`; `GenerateButton.tsx:35,73` |
| MetaCard delta renders when both figures present; `costDelta`/`formatDelta` exist | `MetaCard.tsx:57,164-171`; `cost.ts:80-99` |
| `prices::audit()` is hermetic, clockless; `AUDIT_TOLERANCE=1.2`; a sibling can hold the clock | `prices.rs:1421,1619-1649` |
| `hickeyfield-core` has **no** date crate (`chrono`/`time` absent) | `crates/hickeyfield-core/Cargo.toml` |
| quantity advisory fires when settled vs requested seconds differ by >0.5s | `runner.rs:405-418` |
| `ActualSource` = `{Probe, OutputCount, Provider}` — **no `Billed`** (that is S22) | `cost.rs:355-364` |

---

## A1 — output-seconds settle, workhorse (already passes; add proof)

**No code change.** `compute_actual` already prices the settled billable from probed dims at the
registry rate. Design only the fixture test.

Worked example (A1's named case): a `minimax_h3_max_turbo` job requested at 6 s, 768P, probed to
`MediaDims{seconds:6.592, width:1366, height:768}`. `settled_billable`
(`runner.rs:359-400`) overwrites the quantity with the probed values; `Prices::estimate` finds no
feed quote and falls back to the literal `PerSecondTiered`; `height=768` selects the `(768,0.02)`
tier (`cost.rs:180-192`, `height <= max_h`); `usd = 0.02 × 6.592 = 0.13184`. `actual_source =
Probe` (a dim was probed), never `Billed`.

"Same for i2v": the H3 route is input-mode-independent — one route `fal:minimax/h3-max-turbo`
serves both `text-to-video` and `image-to-video`, and the `PerSecondTiered` literal is identical
for both (the suffix resolves at submit, not in the cost model). The test asserts this by pricing
the same route with the settled billable; an explicit i2v flavor adds a `MediaRole::Start` ref to
the job and asserts the price is unchanged.

MUST: `actual_usd == 0.13184 ± 0.001`, distinctly **not** `0.02 × 6 = 0.12`; `actual_source ==
Some(Probe)`.

---

## A2 — estimate states the output-seconds assumption (real gap)

**Change `cost.rs`.** Append a fixed, number-free clause to the `basis` of the two per-second video
shapes so the assumption is stated wherever a per-second video is quoted:

- `CostModel::PerSecond` basis build (`cost.rs:209-215`): after the existing
  `"{s}s x ${usd}/s[, audio…][, batch…]"`, append `" — billed on delivered output seconds; a clip
  can run longer than requested"`.
- `CostModel::PerSecondTiered` basis (`cost.rs:188-192`): same clause appended to
  `"{s}s x ${rate}/s at {h}p"`.

Normative:
- The clause MUST carry **no number** (no invented overrun constant). A2's permitted number — the
  measured, dated observation `6 s → 6.592 s (2026-09-18)` — is H3-specific, so it does NOT go in
  the generic basis; it goes in the two H3 route notes (below), which are the only place a dated H3
  number is shown.
- It MUST reach the UI unchanged: `basis` → `CostEstimate.basis` (`api.ts:532`) → `costBasisNote`
  (`cost.ts:54`) → the Generate-button note (`GenerateButton.tsx:73`). No UI code changes; the
  clause simply renders.

**Route-note dated observation.** Append `" (measured 2026-09-18: 6 s requested → 6.592 s
delivered)"` to both H3 route notes (`registry.rs:1168`, `:1178`). This is the dated observation
A2 sanctions; it surfaces via `RouteDto.note` (`commands.rs:473`) → picker title
(`ModelPicker.tsx:148`).

Back-compat: `basis` is a display string; appending to it changes no `usd`. Existing assertions are
`contains(...)` (`cost.rs:387,400`; `SettingsRail.test.tsx`/`cost.test.ts` feed their OWN basis
inputs, not the core output) so they stay green. **N/A — no migration.**

Why generic (both shapes) rather than only the two H3 routes: R-FIELD and B1 make "bill on output
seconds" a property of *every* per-second route, not H3 alone; a generic clause is the honest and
smaller change. Open question O1 records the route-specific alternative.

---

## A3 — dated, structured promotional marker + stale re-check (real gap)

Today the promo rate is free text in `Route::noted`. A3 wants it **structured**, dated, surfaced,
and re-checked against an injected clock.

### A3.1 The marker on the route
Add to `crates/hickeyfield-core/src/route.rs`:

```rust
/// A launch/promotional rate that must be re-verified before it silently rots.
/// Distinct from a permanent rate: the provider will revert it, so the estimate
/// carries the date it was transcribed and `prices::promo_recheck` flags it stale.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PromoRate {
    /// ISO `YYYY-MM-DD` the rate was read from the provider index.
    pub as_of: String,
}
```

and a field on `Route` (mirroring the `min_billed_seconds` precedent, `route.rs:37-38`):

```rust
#[serde(default, skip_serializing_if = "Option::is_none")]
pub promo: Option<PromoRate>,
```

plus a constructor `Route::promotional(provider, slug, note, as_of: &str) -> Route` beside
`floored()` (`route.rs:63-70`), setting `promo: Some(PromoRate{ as_of: as_of.into() })`. The two
existing ctors (`new`, `noted`) set `promo: None`.

Apply to the two H3 tiers (`registry.rs:1164-1183`): swap `Route::noted(...)` →
`Route::promotional(ProviderId::Fal, "minimax/h3-max", <note>, "2026-09-16")` and the same for
`minimax/h3-max-turbo`. `as_of = "2026-09-16"` matches the date already in the note text.

Back-compat: new serde-default field; stored `Route` blobs and the vendored snapshot are unaffected
(the snapshot stores `Quote`, not `Route`). **N/A — no migration.**

### A3.2 Surface to the UI
Add `promo_as_of: Option<String>` to `RouteDto` (`commands.rs:148-162`) set from
`r.promo.as_ref().map(|p| p.as_of.clone())` at `:469-476`. The UI renders it as a small dated badge
in the picker row (e.g. `promotional rate · as of 2026-09-16`); to keep test-locked selectors intact
(`.model-item*`, `.chip-route`, STATUS.md:118) it is an additive `<span className="route-promo">`
inside the existing route line, plus a `mock.ts` value on a route. Proof is a Rust test that the DTO
carries the date (§Test plan); the React badge is a one-line additive render.

### A3.3 The stale re-check (injected clock, hermetic)
`prices::audit()` is intentionally clockless; keep it so. Add a **sibling** in `prices.rs`:

```rust
/// A promotional rate old enough to re-verify against the provider index.
pub const PROMO_STALE_DAYS: i64 = 30;

#[derive(Debug, Clone, PartialEq)]
pub struct PromoRecheck { pub route_id: String, pub as_of: String, pub age_days: i64 }

/// Emit a re-check row for every promotional-rate route whose marker is older
/// than `PROMO_STALE_DAYS` as of `today` (civil Y-M-D). Hermetic: the clock is a
/// parameter, so the test injects a fixed date and never reads the wall clock.
pub fn promo_recheck(today: (i32, u32, u32)) -> Vec<PromoRecheck>;
```

`promo_recheck` iterates `registry::registry()`, and for each route with `promo: Some(p)` parses
`p.as_of` and `today` to days-since-epoch via a small private `days_from_civil(y,m,d) -> i64`
(Howard Hinnant's ~5-line algorithm — no date crate, satisfies R-RULES.4 no-new-dep). It emits a
`PromoRecheck` when `today_days - as_of_days > PROMO_STALE_DAYS`. Routes with `promo: None` never
appear.

**Threshold = 30 days.** Justification: it matches the project's existing staleness convention
(objective R1.7 degrades the sandbox hint when `>30 d`; the price feed refreshes daily and is
"weeks-to-months" volatile, `pricing.rs:23-26`). A launch promo should be re-verified monthly. This
is a stated, overridable default (O2).

Worked example: as-of `2026-09-16`; `promo_recheck((2026,10,20))` → age 34 d > 30 → one row per H3
tier. `promo_recheck((2026,09,20))` → age 4 d → empty.

Optional printer: `examples/price_check.rs` MAY print the `promo_recheck(<today>)` rows beneath the
audit sections (keyless, offline), so a human running the audit sees stale promos. Not load-bearing.

---

## A4 — delta on the card (renders today; add workhorse-valued proof)

**No component change.** `MetaCard` draws the Delta row via `costDelta`/`formatDelta` when both
figures are present (`MetaCard.tsx:57,164-171`). Gap: a vitest with the slice's exact fixture.

Add a case to `ui/src/components/MetaCard.test.tsx` (the existing suite, so the test-locked
`.meta-card-fields`/`.meta-card-delta` selectors are exercised): a settled workhorse job with
`estimatedUsd: 0.12`, `actualUsd: 0.13184`. `costDelta` → `{ usd: 0.01184, sign: "over" }`;
`formatDelta` → `formatUsd(0.01184)` = `"$0.012"` (0.01≤x<0.1 → 3 dp, `cost.ts:24`) → **`"+$0.012"`**;
`data-tone="over"`. Assert `fields()` contains `"Delta"`, the cell text is `"+$0.012"`, and the tone
is `over`. Test-locked selectors preserved (additive test only).

---

## B1 — output-seconds settle is general (already passes; add grok proof)

**No code change** — the rule lives in `compute_actual`, which is route-agnostic. Add the second
measured fixture as a `runner.rs` test: `grok_video_v15`, route `fal:xai/grok-imagine-video/v1.5`,
requested 5 s @ 720p, probed `MediaDims{seconds:5.04, width:1280, height:720}`. Literal
`PerSecondTiered`, `height=720` → `(720,0.14)` tier → `0.14 × 5.04 = 0.7056`. MUST: `actual_usd ==
0.7056 ± 0.005`, `actual_source == Some(Probe)`.

Note (5.04 vs 5.0 is 0.04 s < 0.5 s): this grok job does **not** trip the quantity advisory
(`runner.rs:411`), whereas the workhorse 6 → 6.592 (0.592 s) does — a real behavioural difference,
recorded so the tester expects it.

---

## B2 — no collateral (guard, not a change)

This slice changes **no `CostModel` literal value**: A2 edits only `basis` display strings; A3 adds
a `Route` field + note text and a new `promo_recheck` fn. Therefore `prices::audit()`'s `Mismatch`
count stays **0** and the 10×/1.2× guards (`prices.rs:1737-1770`, `AUDIT_TOLERANCE`) stay green.
`PerToken` is untouched. The B2 proof is the existing `prices.rs` audit tests plus a fmt/clippy-clean
build (G1).

---

## Test plan — one named test per AC (all hermetic; R-RULES.1 preamble on every command)

Rust tests live in the `#[cfg(test)] mod tests` beside the code. Every fixture leads with the
workhorse. No sockets: `compute_actual` reads the registry + bundled snapshot only; `promo_recheck`
reads the registry only; the S18 fake-provider seam is available if a submit path is ever needed
(not needed here — these tests call the pure functions directly).

| AC | test (name · file) | asserts |
|---|---|---|
| A1 | `workhorse_768p_settles_on_output_seconds` · `runner.rs` | `compute_actual` on the workhorse with `MediaDims{6.592,1366,768}` ⇒ `Some(0.13184) ± 0.001`, `Some(Probe)`; **not** 0.12 |
| A1 | `workhorse_i2v_prices_the_same_output_seconds` · `runner.rs` | same route with a `Start` ref (i2v) ⇒ identical `0.13184`, `Probe` |
| A2 | `both_h3_routes_state_output_seconds_in_the_basis` · `registry.rs` | `m.estimate(route,b).basis` for both H3 tiers contains "output seconds" and "longer than requested"; carries no digit-bearing overrun constant |
| A2 | `per_second_basis_states_output_seconds` · `cost.rs` | `PerSecond` and `PerSecondTiered` estimate basis contain the clause; `usd` unchanged from the pre-clause value |
| A3 | `the_h3_tiers_carry_a_dated_promo_marker` · `registry.rs` | both H3 routes have `promo == Some(PromoRate{as_of:"2026-09-16"})`; every other route `promo == None` |
| A3 | `promo_recheck_flags_a_stale_marker_with_an_injected_clock` · `prices.rs` | `promo_recheck((2026,10,20))` yields ≥1 row (age 34 d); `promo_recheck((2026,09,20))` is empty; no wall-clock read |
| A3 | `route_dto_carries_the_promo_as_of` · `commands.rs` | the workhorse `RouteDto.promo_as_of == Some("2026-09-16")`; a non-promo route is `None` |
| A4 | `workhorse settled job renders the est-vs-settled delta` · `MetaCard.test.tsx` | est 0.12 / actual 0.13184 ⇒ "Delta" field, cell `"+$0.012"`, `data-tone="over"`; locked selectors present |
| B1 | `grok_720p_settles_on_output_seconds` · `runner.rs` | `compute_actual` on grok with `MediaDims{5.04,1280,720}` ⇒ `Some(0.7056) ± 0.005`, `Some(Probe)` |
| B2 | existing `prices.rs` audit tests + `no_bundled_quote_is_...` | `Mismatch` count 0; 10× guard green; unchanged |

Mutation checks for the TEST agent: drop the A2 basis clause ⇒ the A2 tests fail; change
`PROMO_STALE_DAYS` to a large number ⇒ `promo_recheck_flags_a_stale_marker` fails; revert a workhorse
tier to a wrong rate ⇒ A1 fixture + `prices::audit` both fail.

### G1 gate (exact; R-RULES.1 preamble, never `HICKEYFIELD_ALLOW_SPEND`, never `first_light`)
Preamble for every command:
`export HICKEYFIELD_NO_KEYCHAIN=1 hickeyfield_FAL_KEY=x hickeyfield_HIGGSFIELD_KEY=x hickeyfield_GOOGLE_KEY=x hickeyfield_OPENAI_KEY=x hickeyfield_XAI_KEY=x hickeyfield_BFL_KEY=x hickeyfield_RECRAFT_KEY=x hickeyfield_VAIG_KEY=x`
1. `cargo test --workspace` — 0 failed, baseline + new tests.
2. `cargo fmt --all --check`  3. `cargo clippy --workspace --all-targets -- -D warnings`
4. `./scripts/lint-provenance.py`  5. `python3 ./scripts/lint-spend.py`
6. Keyless audit: `env -u hickeyfield_FAL_KEY -u FAL_KEY cargo run -p hickeyfield-core --example audit_fal` (unchanged).
7. UI (Node 24): `cd ui && pnpm test && pnpm build`.

G2 spend hygiene: every command sets the dummy keys; no test opens a socket (the two settle
functions and `promo_recheck` are pure over registry/snapshot); `HICKEYFIELD_ALLOW_SPEND` never set;
`first_light` never run.

---

## Documentation (DOCUMENT phase writes these; this invocation writes only design.md)
- `CHANGELOG.md` `[Unreleased] > Added`: per-second video quotes now state they bill on delivered
  output seconds; the H3 tiers carry a dated promotional-rate marker with a monthly stale re-check.
- `CHANGELOG.md` `[Unreleased] > Fixed`: (if applicable) none — A1/B1 were already correct; note the
  workhorse/grok fixtures now pin them.
- `README`: checked at implement time for a promo/price note; likely **N/A** (no user-facing flag).
- `document.md`: file + line for each change (route.rs `PromoRate`, registry H3 tiers, cost.rs basis,
  prices.rs `promo_recheck`, commands.rs `RouteDto.promo_as_of`, MetaCard.test.tsx, runner.rs tests).

---

## Non-goals (recorded, not skipped)
- **`ActualSource::Billed` / balance-delta truth — N/A (S22).** Settled numbers here stay `Probe`
  (re-priced), never labelled billed (Q-R2 default, slice §Approach).
- **Estimate ranges + free-sandbox hint — N/A (S24).** No `{low,expected,high}`, no `sandbox_free.json`.
- **Calibration store — N/A (S23).** No `price_observation` table, no median ratio.
- **Image tiers / `PerImageTiered` — N/A (S21).**
- **`PerToken` / Seedance — N/A (owner: never used).** Not read, not changed.
- **Overrun multiplier constant — deliberately none.** Only the dated measured observation is shown.
- **No new dependency** (no date crate; `days_from_civil` is inlined) — R-RULES.4.

## Open questions (each with a default this build assumes)
- **O1 — A2 clause: generic (all per-second) or only the two H3 routes?** DEFAULT: **generic** —
  R-FIELD/B1 make output-seconds billing a property of every per-second route, and a generic basis
  clause is smaller and honest. Reversible: gate the clause on `route.promo.is_some()` if the owner
  wants it H3-only.
- **O2 — promo stale threshold?** DEFAULT: **30 days**, matching R1.7's `>30 d` staleness and the
  daily/monthly feed cadence. One-line change if the owner prefers 14/60.
- **O3 — `promo.as_of` type: ISO string vs unix seconds?** DEFAULT: **ISO `YYYY-MM-DD` string** —
  authored dates are readable in the registry and the snapshot stores no `Route`, so there is no
  serde-size cost; the `days_from_civil` parse is trivial and testable.
- **O4 — surface the promo badge in the picker now, or DTO-only?** DEFAULT: **DTO field + a
  one-line additive badge** (`.route-promo`), proven by the Rust DTO test; the React badge is
  cosmetic and does not touch locked selectors.

## Riskiest / weakest (verifier: start here)
1. **A1/B1 are "prove, don't build" — the honest risk is redundancy.** I assert the settle machinery
   already exists (`runner.rs:320-355`) and that both fixtures resolve via the **registry literal**
   because neither is in the snapshot. If a future snapshot refresh adds a `minimax/h3-max-turbo` or
   `xai/grok-imagine-video/v1.5` quote at a different rate, the feed would win and the fixture would
   shift. Mitigation: the tests assert against the registry literal path; if the run shows a feed
   quote exists, the implementer pins the test to `model.estimate` directly and records it.
2. **A2's clause is generic — it appends to *every* per-second basis.** Low blast radius (display
   string only, `contains` assertions survive), but the verifier should confirm no test asserts an
   exact `basis` equality that the appended clause would break.
3. **`days_from_civil` correctness.** A hand-rolled civil-day function is where an off-by-one hides.
   The `promo_recheck` test pins two dates on either side of the 30-day boundary; the verifier should
   confirm the boundary case (exactly 30 vs 31 days) matches the stated `> PROMO_STALE_DAYS`.
4. **`RouteDto` field addition ripple.** Adding `promo_as_of` to a `#[derive(Serialize)]` struct is
   additive, but the verifier should confirm every `RouteDto { .. }` literal is updated (grep
   `RouteDto {`) so the build stays green.
