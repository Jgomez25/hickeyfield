# S20-workhorse-billing-accuracy — SECURITY (evidence)

Read-only pass over the uncommitted slice diff (`git diff` + untracked) on branch
`forge/s20-workhorse-billing-accuracy`. No source, docs, or config changed; no
`git add`/commit. All commands ran with `HICKEYFIELD_NO_KEYCHAIN=1` + dummy
`hickeyfield_*_KEY=x`; `HICKEYFIELD_ALLOW_SPEND` never set; `first_light` never
run; no cargo/pnpm build (a TEST agent owns the suite in this worktree). Static
inspection + the two lint scripts only.

Slice surface (16 files): `cost.rs` (basis note), `route.rs`/`registry.rs`/`lib.rs`
(`PromoRate` marker), `prices.rs` (hermetic `promo_recheck` + hand-rolled date
math), `commands.rs` (`RouteDto.promo_as_of`), `runner.rs` + `MetaCard.test.tsx`
(tests), `ModelPicker.tsx`/`picker.css`/`api.ts`/`mock.ts`/`types.ts` (UI badge).
Non-source: `.forge/{backlog,objective,state}` bookkeeping.

## 1. Money-safety structure (S17 keychain block + S18 spend gate): INTACT

- Guard files are entirely absent from the diff (`git diff --name-only`): no
  change to `spend_gate.rs`, `vault.rs`, `clients.rs`, `enhancer.rs`, `harness.rs`,
  or `engine.rs`. The gate and keychain-block are untouched.
- Slice-wide grep for the incident patterns — `ALLOW_SPEND`, `set_var`,
  `remove_var`, `keyring`/`get_password`, `TcpStream`, `spend_gate::enable(`,
  `first_light` — returns **nothing**. No new test strips/unsets the dummy env,
  sets the spend flag, reaches the keychain, or opens a socket.
- The three new `runner.rs` tests (`workhorse_768p_settles_on_output_seconds`,
  `workhorse_i2v_prices_the_same_output_seconds`, `grok_720p_settles_on_output_seconds`,
  runner.rs:942-1027) exercise only the pure S10 settled-cost path
  `compute_actual(&job, &Prices::bundled(), MediaDims{..})` with in-memory
  fixtures. `Prices::bundled()` is the embedded price table; `compute_actual` and
  `quantity_advisory` are offline pure functions. No provider client is
  constructed, no S18 fake-provider seam even needed, no network.
- `./scripts/lint-spend.py` → `ok (49 file(s) scanned)`, exit 0. Its scope still
  covers every touched `.rs` file (they are tracked). None of the touched files
  needs a key allowlist entry.

## 2. Under-quote / misleading-money: none introduced

- `promo_as_of` is **purely informational**. `commands.rs:480` maps it straight
  from `r.promo.as_ref().map(|p| p.as_of.clone())`; it is never read by any
  pricing arm. `usd` is computed only from the `CostModel` tiers, unchanged by
  this slice (`registry.rs` tier literals `(768,0.02)` etc. are byte-for-byte the
  same — only the free-text `note` string and the new `promo` field were added).
- `cost.rs` change appends a **number-free display clause** (`OUTPUT_SECONDS_NOTE`,
  cost.rs:135) to the `basis` string only. `per_second_basis_states_output_seconds`
  (cost.rs:402) pins `usd` unchanged and asserts no overrun constant leaks into
  the generic basis. No path makes an estimate or settled cost display lower than
  reality, and the settled source stays `ActualSource::Probe` (never relabelled
  "billed") — confirmed by the runner test assertions.
- A stale promo rate is never presented as current silently: the dated badge
  (`ModelPicker.tsx:158`) always renders the `as_of` date, and `promo_recheck`
  (prices.rs:1691) emits an audit row past `PROMO_STALE_DAYS` (30). The marker is
  advisory over compile-time registry literals, which is the intended, disclosed
  behaviour.

## 3. Input handling `parse_iso_date` / `days_from_civil`: total, no panic/overflow

- `parse_iso_date` (prices.rs:1707) is total: `split('-')` + `parse::<i32/u32>()`
  with `.ok()?`, rejecting extra segments and out-of-range month/day, returning
  `None` (not panic) on any malformed string. `days_from_civil` (prices.rs:1671)
  is pure i64 arithmetic; the only division is by the compile-time constants 4,
  100, 400, 5 (never zero) — no divide-by-zero. Worst-case hostile year is bounded
  by `i32::MAX` (parse ceiling): `era*146097 ≈ 7.8e11` stays far inside i64, so no
  overflow even in debug. DoS is impossible — no loop scales with input.
- Provenance of `as_of`: today it is **only** compile-time string literals passed
  to `Route::promotional` in `registry.rs`. `promo_recheck` reads solely
  `crate::registry::registry()` (code-built), never feed/DB JSON. `Route`/`PromoRate`
  do derive `Deserialize` (`#[serde(default, skip_serializing_if=...)]`, mirroring
  the `min_billed_seconds` precedent), so a stored/feed blob could one day carry an
  `as_of` — and even then the parse is total and the value only informational, so
  the boundary is safe by construction. Confirmed no clearance gap here.

## 4. UI injection: safe; CSP/allowlist unchanged

- `ModelPicker.tsx:158-161` renders `promo · {route.promoAsOf}` via plain React
  text interpolation — auto-escaped. No `dangerouslySetInnerHTML`, no `innerHTML`,
  no URL/href sink. The badge is additive; the test-locked selectors
  (`.chip-route`, `.model-item*`, `.meta-card-*`) are untouched.
- No Tauri config touched: `git diff --name-only` shows no `tauri.conf*`,
  `capabilities`, or CSP file. Allowlist unchanged.

## 5. Secrets / privacy / licence / supply-chain: clean

- No key, token, or private path in the diff (the only `grep` hits for
  private-name patterns are `.forge/objective.md` backlog prose, not secrets).
  Test fixtures use `example.com` / `cdn.example` / `/lib/x.mp4` placeholders.
- **No new dependency**: `git diff --name-only` shows no `Cargo.toml`,
  `Cargo.lock`, `ui/package.json`, `pnpm-lock.yaml`, or `deny.toml`. The
  hand-rolled date math is deliberately inlined to avoid a date crate (R-RULES.4).
- `./scripts/lint-provenance.py` → PASS (no third-party media hosts; no shipped
  string matches the reference corpus), exit 0.

## Findings (ranked)

**No Critical, High, Medium, or Low findings.** Every boundary in scope was
examined and is clear:
- Money gate / keychain block — untouched, no regression pattern present.
- Under-quote / mislabel — `promo_as_of` is informational; `usd` unchanged.
- Date parsing — total, no panic/overflow/DoS; input is compile-time literals.
- UI injection — escaped React text; CSP/allowlist unchanged.
- Secrets & supply-chain — no secrets, no new dep, both lints green.

### Informational (no exploit path — not a finding, no new slice)
`promo_recheck` (prices.rs:1691) flags marker *age* only; it reads the registry's
compile-time literals, so it cannot itself detect that a provider actually reverted
a promo rate. This is by design (hermetic, clock-injected) and the dated badge
communicates the uncertainty to the user. Live re-verification is already deferred
to S22 (settled-truth distinction) per the slice's own scope. No action for this
slice.

## No new slices recommended

There are no Critical/High findings, so no security follow-up slices are proposed
to the backlog or orchestrator.

VERDICT: PASS
