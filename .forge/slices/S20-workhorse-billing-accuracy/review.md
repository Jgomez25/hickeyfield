# S20-workhorse-billing-accuracy — REVIEW (evidence)

Read-only production-standards review of the uncommitted slice diff (16 files). Reviewer did
not implement it. Scope: only what the slice touched. Preamble/spend rules honoured; no source
changed, nothing built (a separate TEST agent runs the gate).

## What I checked and found clean

- **A1/B1 tests drive the real settle path, not a tautology.** `compute_actual`
  (`src-tauri/src/runner.rs:328`) is the production write path, called at `runner.rs:250` from
  `download_outputs`. The new tests (`runner.rs:942,967,993`) call it directly with real
  fixtures and assert dollar values, distinctly rejecting the requested-seconds price
  (`(usd-0.12).abs() > 0.001`). Registry literals confirmed: workhorse `PerSecondTiered
  (768,0.02)` → 0.02×6.592 = **0.13184**; grok `(720,0.14)` (`registry.rs:1118`) →
  0.14×5.04 = **0.7056**. Not tautological.
- **Date math is correct.** Independently recomputed: `days_from_civil(2026,9,16)=20712`
  (matches the assert at `prices.rs`), age to 2026-10-16 = 30 (not `>30`, not stale), to
  2026-10-17 = 31 (stale), to 2026-10-20 = 34. Boundary `>` at `prices.rs` is right; the
  boundary test pins it. No panics: `parse_iso_date` returns `None` on bad shapes; `i64`
  arithmetic, no overflow at these years.
- **B2 no collateral.** No `CostModel` literal value changed (A2 edits only `basis` display
  strings; A3 adds a `Route` field + note text). `PerToken`/Seedance untouched.
- **Settled numbers stay re-priced.** `ActualSource` still `{Probe,OutputCount,Provider}` —
  no `Billed` variant added; tests assert `Some(Probe)`. The string "billed on delivered
  output seconds" describes the provider's *billing method* (the estimate assumption), never
  labels our settled figure as billed truth. Contract satisfied.
- **A2 basis clause is number-free** (`cost.rs:135` `OUTPUT_SECONDS_NOTE`); the dated
  observation (6 s → 6.592 s, 2026-09-18) lives only in the two H3 route notes
  (`registry.rs:1165,1177`). Tests assert the generic basis contains no `6.592`.
- **A3 marker consistency.** `PromoRate{as_of}` on `Route`; all four ctors set `promo`
  consistently (`new`/`noted`/`floored` → `None`, `promotional` → `Some`). `#[serde(default,
  skip_serializing_if="Option::is_none")]` mirrors the `min_billed_seconds` precedent, so
  stored blobs deserialise without migration; the vendored snapshot stores `Quote`, not
  `Route`, so it is unaffected. `PROMO_STALE_DAYS=30`.
- **DTO / TS parity.** `RouteDto.promo_as_of: Option<String>` (`commands.rs:162`) set at the
  single construction site (`commands.rs:480`); TS `promoAsOf?: string` (`types.ts`) with
  snake+camel intake in `toRoute` (`api.ts:463`). Field name/casing match.
- **A4 vitest** asserts the locked field order `[Route, Estimated cost, Actual cost, Delta,
  Enhancer]`, delta cell `"+$0.012"`, `data-tone="over"` — test-locked selectors exercised.
- Badge is additive: an extra `<span className="route-promo">` inside the existing
  `.chip-route` button (`ModelPicker.tsx`); locked `.model-item*`/`.chip-route` selectors
  untouched.

## Blocking

None.

## Non-blocking

1. **`ui/src/components/picker.css:331` — dangling CSS var, no fallback.**
   `border-radius: var(--radius-sm);` — no `--radius-sm` token exists (the scale is
   `--radius-050..--radius-600`, `--radius-card`, `--radius-panel`, `--radius-full`; verified
   in `ui/src/tokens.css:152-166`) and there is no fallback, so the badge renders with square
   corners. This is exactly the "no dangling CSS vars / uses existing tokens" bar the contract
   sets. Fix: use `var(--radius-100)` (0.25rem) or `--radius-050`.

2. **`ui/src/components/picker.css:332` — undefined token, masked by a fallback.**
   `background: var(--surface-raised, rgba(255,255,255,0.08));` — `--surface-raised` is not
   defined anywhere (surfaces are `--bg-primary..--bg-tertiary`, `--bg-glass`). It renders only
   via the literal fallback, i.e. it never consults the theme. Fix: use an existing surface
   token such as `var(--bg-secondary)` (or `--bg-glass`) so the badge tracks the theme.

3. **`crates/hickeyfield-core/src/prices.rs` (`only_promotional_routes_appear_in_the_recheck`,
   the `days_from_civil` reference comment) — comment contradicts code.** The comment states
   "2026-09-16 is 20713 days after the epoch" but the assertion (and the correct value) is
   `20712`. AI-code tell; harmless but misleading. Fix: change the comment to 20712.

4. **`ui/src/mock.ts:58` — demo promo placed on the wrong model; mock does not lead with the
   workhorse.** The demo `promoAsOf` was attached to `kling_3_0`'s fal route, which carries no
   real promo, while the true promo tiers are the H3 models; the catalogue also leads with
   `kling_3_0` (and still lists `seedance_2_0`), not `minimax_h3_max_turbo`. Cosmetic and
   demo-only (labelled as such), and the mock ordering is pre-existing, but the demo would read
   truer on an H3 route. Fix: move the demo value onto an H3 route, or leave a comment that the
   placement is arbitrary.

5. **`crates/hickeyfield-core/src/prices.rs` `parse_iso_date` — no per-month day validation.**
   `2026-02-30` / `2026-04-31` parse successfully (day is only range-checked 1..=31). The
   implementer disclosed this as a deliberate non-guard; it is safe because a bad date drops
   the route from the re-check (no panic) and registry dates are authored, so this is a note,
   not a defect. No fix required; a one-line comment already covers it.

## Ruling on the un-wired `promo_recheck`

**Honestly meets AC A3; not a blocker — but it is one line short of real parity with its
sibling.** AC A3's wording is: "`prices::audit` (or its sibling) emits a re-check row once the
marker is older than a stated threshold — proven with an injected clock, no network." The
function `promo_recheck(today)` (`prices.rs`) does exactly that: iterates the registry, emits a
`PromoRecheck` per stale promo route, clock injected as a parameter, registry-only (hermetic),
proven by three tests including the 30-vs-31-day boundary. The AC requires the emit + the
injected-clock proof, and does not require live wiring; the design (O4 / known-gap) explicitly
scoped live surfacing to a later slice. So the contract is met at its word.

However, the implementer's own framing calls it "a sibling of `audit()`", and that parity is
incomplete: `audit()` is reachable by a human via `examples/price_check.rs:27`, whereas
`promo_recheck` has **zero non-test callers** — the design's optional `price_check.rs` printer
was left out. A `pub` function with doc-links (`route.rs:19,96`) and passing tests is a
legitimate, if minimal, library API surface, not truly dead code, so this does not block. I
recommend the follow-up the design already sketched: print `promo_recheck(<today>)` rows under
the audit sections in `examples/price_check.rs` (keyless, offline) so the re-check is actually
visible to the human who runs the audit and the sibling relationship is real.

## Recommended follow-ups (do not apply here)

- Replace `--radius-sm` (and prefer an existing surface token over `--surface-raised`) in
  `picker.css`.
- Wire `promo_recheck` into `examples/price_check.rs` so a stale promo is surfaced to a human.
- Correct the 20713→20712 comment; move/annotate the `mock.ts` demo promo.

VERDICT: PASS — no blockers and no majors on a critical path. The money path (A1/B1
output-seconds settle) is correct and proven against the production `compute_actual`; A2/A3/A4
meet contract. The findings are cosmetic (two picker.css token issues, a wrong-value comment, a
misplaced demo value) and one honestly-met-but-unwired API surface, all non-blocking.

## Repair round 1 — re-review

Re-review after repair round 1. Read-only; no source changed, nothing built (a separate
TEST agent runs the gate). Scope: only the four files the repair touched, plus the delta's
security surface. Verified against the uncommitted slice diff.

### Prior non-blocking findings — status

1. **CLOSED — `picker.css` dangling `--radius-sm`.** Now `border-radius: var(--radius-100);`
   (`picker.css:328`). `--radius-100 = 0.25rem` exists (`tokens.css:153`). No dangling var,
   no fallback needed.
2. **CLOSED — `picker.css` undefined `--surface-raised`.** Now `background: var(--bg-secondary);`
   (`picker.css:329`). `--bg-secondary` exists (`tokens.css:83`) and tracks the theme (no
   literal-color fallback). Matches the sibling surface token convention.
3. **CLOSED — `prices.rs` "20713 days" comment.** The reference comment now reads
   "2026-09-16 is 20712 days after the epoch" (`prices.rs`, in
   `only_promotional_routes_appear_in_the_recheck`), agreeing with the assert
   `assert_eq!(days_from_civil(2026, 9, 16), 20712)`. Independently recomputed: 20712 is
   correct.
4. **CLOSED — `mock.ts` demo promo on the wrong model / roster did not lead with workhorse.**
   `promoAsOf` removed from `kling_3_0` (`route("fal","kling-video/v3/standard")` now carries
   no promo — kling has no real promo). `minimax_h3_max_turbo` added as the FIRST entry in
   `MOCK_MODELS` (`mock.ts:47`), carrying `promoAsOf: "2026-09-16"` on its `fal:minimax/h3-max-turbo`
   route — the model that actually holds the real promo in the registry
   (`registry.rs:1174`). Shape matches `interface Model` (`types.ts:71`: id/displayName/
   modality/routes/isLaunch/subtitle?/capabilities?) and `Route.promoAsOf?` (`types.ts:26`).
   `RATE_PER_SECOND["minimax_h3_max_turbo"] = 0.02` added (`mock.ts:428`). R0.3 (mock leads
   with the workhorse) now satisfied.
5. **N/A — `parse_iso_date` per-month day validation.** Explicitly "no fix required" in the
   prior review; unchanged. Still safe (a bad date drops the route, no panic).

### New code reviewed

- **`price_check.rs` — `today_civil()` / `promo_recheck_section()` (the recommended printer).**
  `today_civil` inlines Howard Hinnant's `civil_from_days` inverse. Checked term-by-term
  against the canonical algorithm (z-offset 719468, era 146097, `yoe`, `doy`, `mp`,
  `d = doy-(153*mp+2)/5+1`, `m = mp<10?mp+3:mp-9`, `y+1` when `m<=2`) — correct. Live proof:
  the gate output shows `as of 2026-09-19`, matching today's real date, so the wall-clock
  inverse round-trips. No panic path: `duration_since(UNIX_EPOCH)` is `unwrap_or(0)`, not
  `unwrap()`. `promo_recheck_section()` reads only the registry via the injected date, maps
  to formatted rows, and prints via the existing `section()` helper — output shape and count
  header are consistent with the `audit()` sections. Comment density matches the file's
  prose-heavy style. The library `promo_recheck(today)` keeps its injected clock; only the
  example reads the wall clock, as documented. This closes the prior "unwired sibling"
  recommendation: `promo_recheck` now has a human-reachable caller.
- **Duplication judgement (`civil_from_days` in the example vs `days_from_civil` in the
  library).** The two are inverses, not the same function — the library needs
  days-from-civil, the printer needs civil-from-days. For a size-S slice, inlining the
  inverse in the single example that needs it (rather than adding a `pub` library API or a
  date crate) is the proportionate choice and honours the no-new-dep rule. Acceptable; a
  future slice could promote both into one library module if a second caller appears.

### Security delta

`grep -nE "reqwest|http|TcpStream|set_var|remove_var|ALLOW_SPEND|vault|keyring|env"` over the
repaired `examples/price_check.rs` returns **no matches**. The only new runtime input is
`SystemTime::now().duration_since(UNIX_EPOCH)` (a local clock read, `unwrap_or(0)`). No
network call, no key/env read, no env mutation, no `HICKEYFIELD_ALLOW_SPEND`, nothing that
can spend. Example is keyless and offline (registry-only), same posture as `audit()`. The
`mock.ts`/`picker.css`/`prices.rs` repair edits add no I/O. **Security delta: clean.**

### New findings

- **Blocking:** none.
- **Non-blocking:**
  1. **`ui/src/mock.ts:428` — a single flat `0.02` mock rate for a tiered route.** The real
     workhorse is `PerSecondTiered [(480,0.0125),(768,0.02),(1080,0.04)]`; the mock's flat
     `0.02` is the 768p/720p middle tier, so browser-mock mode will under-state a 1080p
     estimate (~2x) and over-state 480p. This is the *same* modelling as every other mock
     entry (`kling_3_0: 0.28`, tiered in reality) and mock mode is explicitly a non-production
     demo, so it is honest-enough and consistent with its siblings — but 0.02 is a
     representative mid-tier, not the tier the picker's default resolution necessarily uses.
     No fix required for this slice; note only.
  2. **`ui/src/mock.ts:88` — `seedance_2_0` still present in the mock roster and presets.**
     Pre-existing demo scaffolding, untouched by this slice and outside the S20 diff; flagged
     only because the owner rule is "Seedance is never used." Not introduced or worsened by
     the repair; recommend a follow-up sweep of mock demo data, not a change here.

### Verdict rationale (whole slice, post-repair)

All five prior non-blocking findings are closed or correctly deferred. The money path
(A1/B1 output-seconds settle via the production `compute_actual`) was already correct and
proven; the repair touched none of it and changed no `CostModel` literal. The new example
code is correct (Hinnant inverse verified), adds a legitimate human-reachable caller for
`promo_recheck`, and is keyless/offline. Security delta clean. Two residual non-blocking
notes are cosmetic/pre-existing demo-data items with no critical-path impact.

VERDICT: PASS — no blockers, no majors on any critical path. All prior findings resolved;
the repair introduced no regressions and no spend surface. Residual items are non-blocking
mock-demo cosmetics.
