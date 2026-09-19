# S20-workhorse-billing-accuracy — TEST (evidence)

Author: TEST agent (did NOT implement). Every command carried the money-safety preamble
(`HICKEYFIELD_NO_KEYCHAIN=1` + dummy `hickeyfield_*_KEY=x`); `HICKEYFIELD_ALLOW_SPEND` never
set; `first_light` never run; the only key-unset was the sanctioned keyless `audit_fal`.

## Implementer's claim (verbatim, implement.md)

> WORKHORSE = `minimax_h3_max_turbo`; Seedance/`PerToken` untouched. No new dependency, no
> migration.
> 1. `cargo test --workspace` — **OK**. … **Total 971 passed / 20 ignored** … 0 failed.
> 2. `cargo fmt --all --check` — **OK** … 3. `cargo clippy … -D warnings` — **OK** …
> 4. `./scripts/lint-provenance.py` — **OK** … 5. `./scripts/lint-spend.py` — **OK** …
> 6. keyless `audit_fal` — **OK** … 7. `cd ui && pnpm test` — **OK**. `Tests 313 passed` …
> `pnpm build` — **OK**.

I re-ran every acceptance criterion myself. Real output below.

## Commands (per AC)

### A1 — output-seconds settle, workhorse (t2v + i2v). PASS
`cargo test -p hickeyfield-tauri --lib settles_on_output_seconds` /
`… workhorse_i2v_prices_the_same_output_seconds`
```
test runner::tests::workhorse_768p_settles_on_output_seconds ... ok
test runner::tests::grok_720p_settles_on_output_seconds ... ok
test runner::tests::workhorse_i2v_prices_the_same_output_seconds ... ok
test result: ok. 2 passed; 0 failed …   (+ i2v: 1 passed)
```
The tests drive the **production** settle path: `compute_actual(&j, &prices, dims)` →
`settled_billable` (overwrites `b.seconds/width/height` with the probed `MediaDims{6.592,1366,768}`)
→ `Prices::estimate` registry-literal fallback → `(768,0.02)` tier. Assert is `usd==0.13184 ±0.001`
AND `(usd-0.12).abs()>0.001` (must NOT price requested 6 s) AND `src==Some(Probe)`. i2v attaches a
`MediaRole::Start` ref and asserts the identical $0.13184. Not a two-number multiply — see Mutation 1.

### A2 — estimate states the assumption, number-free. PASS
`cargo test -p hickeyfield-core --lib output_seconds`
```
test cost::tests::per_second_basis_states_output_seconds ... ok
test registry::tests::both_h3_routes_state_output_seconds_in_the_basis ... ok
test result: ok. 2 passed; 0 failed …
```
Diff (`cost.rs`) appends a const `OUTPUT_SECONDS_NOTE` (" — billed on delivered output seconds; a
clip can run longer than requested") to BOTH `PerSecond` and `PerSecondTiered` basis; `usd`
unchanged (asserts `usd==0.672` / `usd==0.12`). Basis carries no `6.592`; the dated observation lives
only in the two H3 route notes (`registry.rs`, "measured 2026-09-18: 6 s requested → 6.592 s
delivered"). Number-free clause confirmed.

### A3 — dated promo marker + surface + stale re-check. PASS
`cargo test -p hickeyfield-core --lib promo_recheck` /
`… dated_promo_marker` / `… only_promotional_routes` / tauri `route_dto_carries_the_promo_as_of`
```
test prices::tests::promo_recheck_flags_a_stale_marker_with_an_injected_clock ... ok
test prices::tests::promo_recheck_boundary_is_exactly_thirty_days ... ok
test registry::tests::the_h3_tiers_carry_a_dated_promo_marker ... ok
test prices::tests::only_promotional_routes_appear_in_the_recheck ... ok
test commands::tests::route_dto_carries_the_promo_as_of ... ok
```
Structured `PromoRate{as_of:"2026-09-16"}` on both H3 tiers via `Route::promotional`; every other
route `promo==None`. Reaches UI DTO: `RouteDto.promo_as_of` (single construction site,
`commands.rs:481`) → `RawRoute.promo_as_of`/`promoAsOf` → `Route.promoAsOf`. Injected-clock boundary
proven at the EXACT edge: `promo_recheck((2026,10,16))` = 30 d → empty (not stale);
`promo_recheck((2026,10,17))` = 31 d → 2 rows (stale), `age_days==31`. Independently confirmed
30/31-day arithmetic and `days_from_civil(2026,9,16)==20712` against Python `datetime`.

### A4 — est-vs-settled delta on the card. PASS
`cd ui && pnpm test MetaCard`
```
✓ src/components/MetaCard.test.tsx (6 tests) 99ms
Test Files  1 passed (1)   Tests  6 passed (6)
```
New case: est 0.12 / actual 0.13184 → Delta cell `"+$0.012"`, `data-tone="over"`, and asserts the
locked field order `["Route","Estimated cost","Actual cost","Delta","Enhancer"]`.

### B1 — output-seconds settle is general (grok). PASS
Covered above (`grok_720p_settles_on_output_seconds ... ok`): grok i2v 720p, 5 s asked, probed 5.04 s
→ `(720,0.14)` literal → `usd==0.7056 ±0.005`, `src==Some(Probe)`, and asserts `quantity_advisory`
is `None` (0.04 s < 0.5 s). Same production `compute_actual` path — falsified by Mutation 1.

### B2 — no collateral, PerToken untouched. PASS
`git diff … | grep tiers/usd` on registry/cost/prices: NO `CostModel` literal tier or `usd` value
changed (the only `usd:` in the diff is a `PerSecond{usd:0.084}` built as a *test input*). `git diff
--name-only -G PerToken` → empty (PerToken appears only as unchanged context in `.forge/objective.md`,
never a changed line). Existing audit tests stay green (below).

### G1 — full gate. PASS
```
cargo test --workspace     → core lib: 770 passed; 0 failed; 16 ignored
                             tauri lib: 193 passed; 0 failed; 3 ignored
                             audit_fal ex 6, fal_diff ex 2, live-key ex 1 ignored; 0 failed anywhere
cargo fmt --all --check     → FMT OK
cargo clippy --workspace --all-targets -- -D warnings → Finished, no warnings
./scripts/lint-provenance.py → PASS no third-party media hosts; PASS 80015 shingles
./scripts/lint-spend.py     → lint-spend: ok (49 file(s) scanned).
keyless audit_fal           → UNREACHABLE (0); all four minimax/h3-max{,_turbo}/{text,image}-to-video
                              under CHECKED; 70 endpoint(s) audited
cd ui && pnpm test          → Test Files 23 passed (23); Tests 313 passed (313)
cd ui && pnpm build         → ✓ built in 941ms
```

### G2 — spend hygiene. PASS
`git diff | grep -iE 'ALLOW_SPEND|remove_var|TcpListener|TcpStream|bind\(|::connect|first_light'` →
exit 1 (no match). No test unsets/strips a key, sets `HICKEYFIELD_ALLOW_SPEND`, or opens a socket.
All new tests are pure functions over registry/bundled snapshot.

## Break attempts

1. **Mutation — does A1/B1 exercise the real settle path or just multiply two numbers?**
   Saved `git diff` (844 lines), replaced `b.seconds = dims.seconds;` in `settled_billable`
   (`runner.rs:368`) with a no-op (use requested seconds). Result:
   ```
   workhorse_768p_settles_on_output_seconds ... FAILED  (expected $0.13184 …, got 0.12)
   grok_720p_settles_on_output_seconds       ... FAILED  (expected $0.7056 …, got 0.7000000000000001)
   ```
   Both A1 and B1 go RED — they genuinely re-price from the PROBED duration through the production
   `compute_actual`. Reverted; `diff pre_mut.diff post_mut.diff` → identical ("REVERT CLEAN"),
   `git diff --stat` back to `16 files changed, 542 insertions(+), 9 deletions(-)`.

2. **Date-boundary off-by-one.** Boundary test pins exactly 30 d (not stale) and 31 d (stale);
   `days_from_civil(1970,1,1)==0`, `days_from_civil(2026,9,16)==20712`, both cross-checked against
   Python `datetime` (20712, and 30/31-day gaps). `> PROMO_STALE_DAYS` semantics correct. Leap year
   2028-02-29 is a valid civil date the algorithm handles.

3. **Malformed promo dates / panic.** `parse_iso_date` splits on `-`, `parse::<i32/u32>` each, rejects
   month ∉1..12, day ∉1..31, or extra parts → `None` (route silently dropped, no panic). `""`,
   `"2026-13-01"` → `None`; `"2026-9-16"` → `Some` (single-digit ok); `"2026-02-30"` → parses (no
   per-month validation) but `days_from_civil` is pure integer math → no panic, just a day number.
   Registry authors only real dates; documented non-guard, not a bug. `promo_recheck` cannot panic on
   any registry content.

4. **Serde back-compat.** `promo: Option<PromoRate>` carries `#[serde(default,
   skip_serializing_if="Option::is_none")]`, mirroring the working `min_billed_seconds` precedent; no
   `deny_unknown_fields` on `Route`/`RouteDto`. A stored `Route` blob without `promo` deserializes to
   `None`. Snapshot stores `Quote`, not `Route` — unaffected. Full suite (incl. snapshot load) green.

5. **Ignored-count question.** `git diff | grep '#\[ignore'` → NONE added by this slice.
   `#[ignore]` attrs = 21 at HEAD AND 21 in the working tree (unchanged). Runtime `cargo test
   --workspace` yields **20 ignored** and this is entirely PRE-EXISTING (ffmpeg/ollama/network/
   live-key gated tests). The slice added **zero** ignores. The "9 ignored" figure the launch note
   referenced does not match HEAD (which already yields 20); the difference is a pre-existing counting
   baseline, not a slice regression — a note, not a finding.

6. **UI selector lock + count.** `pnpm test` → 313 tests (313, was 312 +1 A4). Picker badge is an
   additive `<span className="route-promo">` inside the existing `.chip-route`; `.model-item*`/
   `.chip-route` (STATUS.md:118) untouched. A4 asserts the locked MetaCard field order intact.

7. **B2 audit guards.** Existing `the_audit_is_hermetic_…` / `no_bundled_quote_is_free` stay green in
   the workspace run; `Mismatch` guards unaffected since only `basis` display strings + a new field/fn
   were added.

## Non-blocking findings (do not block release)

- **N1** `prices.rs` test comment in `only_promotional_routes_appear_in_the_recheck` says
  "2026-09-16 is 20713 days after the epoch"; the assertion (authoritative, passing) uses **20712**,
  which is the correct value. Cosmetic comment typo only.
- **N2** `parse_iso_date` accepts day 1–31 with no per-month validation (e.g. `2026-02-30` would
  parse). Deliberate non-guard, documented in implement.md; the registry authors real dates and no
  panic path exists. Recorded, not blocking.

## Judgement

Every AC (A1, A2, A3, A4, B1, B2, G1, G2) verified by real command output. The settle path is
genuinely exercised through production `compute_actual`/`settled_billable` and provably falsifiable
(Mutation 1: RED on 0.12 / 0.70, GREEN on 0.13184 / 0.7056). Injected-clock boundary proven at the
exact 30/31-day edge. No `CostModel` literal value changed; `PerToken`/Seedance untouched. Serde
back-compat holds; no new dependency; test-locked selectors intact; spend hygiene clean; the slice
adds zero `#[ignore]`. Working tree left with only the implementer's changes + this file.

Root cause: none — no blocking finding survived.

VERDICT: PASS

## Repair round 1 — re-verification

Author: adversarial TEST agent (re-verify, did NOT implement). Prior PASS above kept intact.
Every command below carried the money-safety preamble (`HICKEYFIELD_NO_KEYCHAIN=1` + dummy
`hickeyfield_*_KEY=x`); `HICKEYFIELD_ALLOW_SPEND` never set; `first_light` never run; the only
key-unset was the sanctioned keyless `audit_fal`.

### Repair claim (verbatim, implement.md "## Repair round 1")

> REVIEW passed (no blockers). This round fixes only its five non-blocking findings. No
> `CostModel` value changed, no new dependency, no Seedance added, test-locked selectors
> untouched.
> 1. picker.css:331 `--radius-sm` → `var(--radius-100)`. 2. picker.css:332 `--surface-raised`
> → `var(--bg-secondary)`. 3. prices.rs:2720 comment 20713→20712. 4. mock.ts: promo removed
> from `kling_3_0`; `minimax_h3_max_turbo` added as the FIRST model (`promoAsOf: "2026-09-16"`
> + 0.02/s). 5. price_check.rs: added `promo_recheck_section()` (inlined `civil_from_days`).

### Check 1 — full gate. PASS
```
cargo fmt --all --check                       → FMT_OK
cargo clippy --workspace --all-targets -D warnings → Finished, no warnings
cargo test --workspace  → 770 + 6 + 2 + 193 = 971 passed; 0 failed; 20 ignored
  test result: ok. 770 passed; 0 failed; 16 ignored   (core lib)
  test result: ok.   6 passed; 0 failed;  0 ignored   (audit_fal ex)
  test result: ok.   2 passed; 0 failed;  0 ignored   (fal_diff ex)
  test result: ok. 193 passed; 0 failed;  3 ignored   (tauri lib)
./scripts/lint-provenance.py → PASS no third-party media hosts; PASS 80015 shingles (exit 0)
./scripts/lint-spend.py      → lint-spend: ok (49 file(s) scanned). (exit 0)
env -u hickeyfield_FAL_KEY -u FAL_KEY … audit_fal → 70 endpoint(s) audited (keyless)
cd ui && pnpm test  → Test Files 23 passed (23); Tests 313 passed (313)
cd ui && pnpm build → ✓ built in 2.06s
```

### Check 2 — named ACs re-run post-repair. PASS
```
runner::tests::workhorse_768p_settles_on_output_seconds ... ok      (A1)
runner::tests::workhorse_i2v_prices_the_same_output_seconds ... ok  (A1 i2v)
runner::tests::grok_720p_settles_on_output_seconds ... ok           (B1)
commands::tests::route_dto_carries_the_promo_as_of ... ok           (A3 surface)
cost::tests::per_second_basis_states_output_seconds ... ok          (A2)
registry::tests::both_h3_routes_state_output_seconds_in_the_basis ... ok  (A2)
registry::tests::the_h3_tiers_carry_a_dated_promo_marker ... ok     (A3)
prices::tests::promo_recheck_flags_a_stale_marker_with_an_injected_clock ... ok  (A3)
prices::tests::promo_recheck_boundary_is_exactly_thirty_days ... ok (A3 boundary)
prices::tests::only_promotional_routes_appear_in_the_recheck ... ok (A3)
```

### Check 3 — scope + no-collateral. PASS
`git diff --stat` → 17 files, 609 insertions / 10 deletions; the four repair-touched files
(picker.css, prices.rs, mock.ts, examples/price_check.rs) were all already in the pre-repair
changed set — nothing new outside the slice's declared files.
- `git diff -G'PerToken' --name-only` → empty: `PerToken` NOT touched (B2).
- `git diff registry.rs | grep '^[+-].*tiers|usd:'` → only test-fn names; NO `CostModel`
  literal tier/`usd` value changed (B2).
- No `#[ignore]` added; ignored count unchanged.

### Check 4 — HARD FACT workhorse. PASS
`ui/src/mock.ts`: `minimax_h3_max_turbo` is the FIRST `MOCK_MODELS` entry (line 57, ahead of
`kling_3_0` @71 and `seedance_2_0` @88). Its fal route `minimax/h3-max-turbo` carries the ONLY
`route(...,"2026-09-16")` promo in the file; `kling_3_0`'s fal route no longer has one.
`RATE_PER_SECOND.minimax_h3_max_turbo = 0.02` = the 768p tier of the registry literal
(`registry.rs:1183` `tiers: [(480,0.0125),(768,0.02),(1080,0.04)]`) — mock comment tiers agree.
No Seedance added: `git diff mock.ts | grep '^\+.*seedance'` → none (exit 1); `seedance_2_0`
is pre-existing at HEAD (12 refs), untouched by the slice — pre-existing baseline, not a
regression.

### Check 5 — price_check.rs keyless + civil_from_days. PASS
Read confirms the new `promo_recheck_section()` reads only the registry via `promo_recheck` +
the wall clock via `today_civil()` (inlined Hinnant `civil_from_days`); no network, no key,
no spend gate. Ran `cargo run -p hickeyfield-core --example price_check` (keyless):
```
=== PROMO RE-CHECK (older than 30 days as of 2026-09-19) (0) ===
  none
MISMATCH 0 / OK 11 / UNMODELLABLE 0 / PROSE 50 / UNPUBLISHED 8 / NO-SNAPSHOT 1
```
Correct: promo as-of 2026-09-16 is 3 days old vs today 2026-09-19 → not stale; MISMATCH still 0.
The example's `today_civil` reported today 2026-09-19 (matches the harness date). Ported the
inlined algorithm to Python and cross-checked against `datetime`:
```
1970-01-01 days=0     civil=(1970,1,1)   match
2000-02-29 days=11016 civil=(2000,2,29)  match
2028-02-29 days=21243 civil=(2028,2,29)  match
2026-12-31 days=20818 civil=(2026,12,31) match
2026-09-19 days=20715 civil=(2026,9,19)  match
2026-09-16 days=20712 civil=(2026,9,16)  match   (= days_from_civil assert value)
```

### Check 6 — CSS tokens exist. PASS
`ui/src/tokens.css`: `--radius-100: 0.25rem;` (line 153) and `--bg-secondary: var(--cool-300);`
(line 83). Both badge tokens resolve; no dangling `--radius-sm` / `--surface-raised` remain in
picker.css (diff shows the badge block now uses the two valid tokens).

## Break attempts — repair round 1

1. **Did the repair leave a dangling token?** `grep -nE 'radius-sm|surface-raised' picker.css`
   → none in the new `.route-promo` block; both replacements resolve in tokens.css. No refute.
2. **Mock roster order relied on by a vitest?** `pnpm test` full suite 313/313 green with the
   workhorse now first — no assertion keys on `MOCK_MODELS[0]`. No refute.
3. **Promo leakage onto a route without a real promo?** Only the workhorse fal route carries
   `promoAsOf` now; kling stripped. Registry side: only the two H3 tiers use `Route::promotional`;
   `promo_recheck` returns exactly those, and every other route is `promo==None` (core tests).
   No refute.
4. **Stale-clock correctness on the real wall clock.** The example uses today (3-day-old promo)
   → empty re-check, as it should; the injected-clock boundary tests still pin 30 d (not stale) /
   31 d (stale). Algorithm verified across leap/epoch/year-end dates vs Python. No refute.
5. **Collateral / value drift.** No `CostModel` literal changed; `PerToken` and Seedance code
   untouched; no new dependency; test-locked selectors intact (313 UI + 971 Rust green). No refute.

Every repair fix verified by real command output; every AC still holds; the full gate is green
post-repair with the expected counts (971 Rust / 313 UI). No blocking finding survived.

Root cause: none.

VERDICT: PASS
