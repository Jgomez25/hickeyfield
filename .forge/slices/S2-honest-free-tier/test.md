# S2-honest-free-tier — ADVERSARIAL TEST

## Implementer's claim (verbatim, from implement.md)

> Approach: Option (a) per design + owner decision **HIDE** — `has_adapter()` returns `true`
> only for providers with a real client (`Fal | Higgsfield`); `Local` dropped. The lie surfaced
> in three places (`route_state`, `route::resolve`, `use_case::route_serves`); one predicate edit
> corrects all three. No new reason string introduced — the existing
> `"Hickeyfield has no client for {display_name} yet"` branch of `route_state`
> (`commands.rs:208-216`) now covers Local, yielding `"Hickeyfield has no client for Local yet"`.

Claimed gate: `cargo test --workspace` "TOTAL passed: 740 failed: 0 (plus 9 ignored)", fmt OK,
clippy clean, provenance exit 0, `pnpm test` 216 passed, `pnpm build` OK.

I re-ran everything myself; I did not trust any pasted output.

## Commands (real output captured this session)

### Scope / diff (branch `forge/s2-honest-free-tier`)
`git diff --stat HEAD` and `git status --porcelain`:
```
 crates/hickeyfield-core/src/provider.rs        |  11 +-
 crates/hickeyfield-core/src/registry.rs        |  11 +-
 crates/hickeyfield-core/src/route.rs           |  37 +++-
 src-tauri/src/app.rs                           |  20 +-
 src-tauri/src/commands.rs                      |  45 ++++-
?? ui/src/components/ModelPicker.test.tsx
 (plus .forge/slices/S2-honest-free-tier/{design,implement,slice}.md — forge artifacts)
```
Production change is a single predicate: `has_adapter()` body
`matches!(self, Fal | Higgsfield)` (was `Fal | Higgsfield | Local`). No manifest/lock churn:
`git diff --stat HEAD -- '*Cargo.toml' '*Cargo.lock' 'ui/package.json' 'ui/pnpm-lock.yaml'` is
empty. No unrelated churn. `ui/dist` (from `pnpm build`) is `.gitignore`d — tree left as found.

### AC1 — Local-only route is UNAVAILABLE with an honest reason
`cargo test -p hickeyfield-tauri` (targeted) and `cargo test -p hickeyfield-core --lib` (targeted):
```
test app::tests::local_needs_no_key_but_has_no_client_yet ... ok
test app::tests::has_adapter_matches_the_clients_actually_implemented ... ok
test commands::tests::local_route_is_unavailable_with_an_honest_reason ... ok
test route::tests::a_local_only_route_cannot_be_resolved_until_a_client_exists ... ok
test registry::tests::the_unroutable_models_are_a_known_short_list ... ok
```
The AC1 test asserts the REASON STRING, not just the bool: `!ok`, then
`reason.contains("no client")` AND `!reason.contains("Needs a")`.
Reason string is REAL, not invented — `commands.rs:212` is
`format!("Hickeyfield has no client for {} yet", provider.display_name())` and
`ProviderId::Local.display_name() == "Local"` (`provider.rs:69`), so the produced string is
exactly `"Hickeyfield has no client for Local yet"`. `route_state` reaches it via the
`!provider.has_adapter()` branch (`commands.rs:208`) BEFORE the `needs_key()` branch, so a Local
route is unavailable regardless of what is in `configured` (the "add a key" path is unreachable
for Local).

### AC3 — reachable fal/Higgsfield routes stay available
```
test commands::tests::real_routes_stay_available_after_the_local_fix ... ok
test commands::tests::a_usable_route_carries_no_reason ... ok
```
`real_routes_stay_available_after_the_local_fix` asserts `route_state(Fal,
"fal-ai/nano-banana-2", &["fal"]) == (true, None)` side-by-side with Local `(false, _)`. The drift
guard `has_adapter_matches_the_clients_actually_implemented` confirms Fal+Higgsfield still hold
adapters and mirrors `client_for`'s `Some`-returning arms (`app.rs:39-48`, `_ => None` at :52).

### AC2 (HIDE) — z_image is not a runnable/Generate option
`cd ui && pnpm test`:
```
 ✓ src/components/ModelPicker.test.tsx (2 tests) 121ms
 Test Files  11 passed (11)
      Tests  216 passed (216)
```
Rust data layer: `models_for_use_case` (`commands.rs:403`) filters via
`use_case::supports` (`commands.rs:412`) → `route_serves` (`use_case.rs:207-210`) returns `false`
when `!route.provider.has_adapter()`. z_image's ONLY route is `local:z-image`, so `supports` now
returns false and z_image drops out of every use-case tab (the HIDE). Pinned by
`registry::the_unroutable_models_are_a_known_short_list`, whose set grew to include `z_image`.

### AC4 — global regression gate (all re-run)
```
cargo test --workspace:
  hickeyfield_core:      636 passed; 0 failed; 9 ignored
  hickeyfield_tauri_lib: 104 passed; 0 failed; 0 ignored
  TOTAL = 740 passed, 0 failed, 9 ignored   (>= 727 floor)
cargo fmt --all --check                         -> exit 0
cargo clippy --workspace --all-targets -D warnings -> exit 0, no warnings
./scripts/lint-provenance.py                    -> exit 0 (both PASS lines)
cd ui && pnpm test                              -> 216 passed (11 files)
cd ui && pnpm build (tsc --noEmit && vite build)-> built in 797ms, exit 0
```
cargo-deny not run (baseline advisories only, no manifest change — noted, not blocking).
`tauri build` deliberately deferred to release, per instructions.

## Break attempts

1. **Did dropping Local break anything other than z_image?**
   `grep -c "ProviderId::Local"` in `registry.rs` production = 1 occurrence (line 1336, z_image's
   sole route). z_image is Local-ONLY; NO model routes to both Local and fal/Higgsfield, so there
   is no dual-route model that could silently lose a reachable route. Every `has_adapter()`
   consumer (`route::resolve` :133/:153/:162/:172, `route::cheaper_alternative` :220,
   `use_case::route_serves` :208, `use_case::supports` Higgsfield-rescue :177, `route_state` :208,
   the app.rs drift guard) only changes behavior where a Local route is involved. Full workspace
   (740 tests) green confirms no fal/Higgsfield/other route regressed. RESULT: only z_image
   affected. Not refuted.

2. **Universal fal uploader path.** `uploader_for` (`app.rs:64-85`) is keyed off the vault
   (`vault::get(Fal)`), NOT `has_adapter`. Unaffected by the predicate change. Not refuted.

3. **Local-endpoint detection / Ollama enhancer.** `detect_local()`/`LocalEndpoints`
   (`clients.rs:678-709`) are HTTP probes; `is_ready()` (`commands.rs:48-53`) uses
   `detect_local().any()`. Neither reads `has_adapter`. Dropping Local from `has_adapter` does not
   touch local detection. Not refuted.

4. **Is the AC2 vitest test vacuous?** No. The component (`ModelPicker.tsx:49`) computes
   `runnable = Boolean(routes.find(r => r.available !== false))` and sets `disabled={!runnable}` /
   `data-unrunnable`. The z_image fixture (`available:false`) asserts disabled + no `onSelect`;
   the SAME test's control model (`available:true`) asserts ENABLED + `onSelect` fires with
   `("nano_banana_2","fal:fal-ai/nano-banana-2")`. The two assertions discriminate on the
   `available` flag against the same component — flipping z_image's flag to available would break
   `expect(main.disabled).toBe(true)`. Non-vacuous.

5. **Reason robustness with `configured` containing "local".** `route_state` hits the
   `!has_adapter()` branch first, so a Local route is unavailable with the honest no-client reason
   even if "local" were configured — the keyless "add a key" lie is structurally unreachable.

6. **Idempotency.** Targeted AC tests and the full suite were run repeatedly with identical
   results; tests are pure (no network, dummy keychain env). Stable.

7. **Q2 deviation (HIDE vs objective Q4 keep-visible-in-tab).** The slice's AC2 explicitly
   records DECISION=HIDE and the flat-list render still exercises the disabled/reason tile
   (AC2 vitest) while the data layer produces the honest unavailable state (AC1). The removal from
   use-case tabs matches the app's existing treatment of the 3 other unroutable models. This is a
   documented, owner-sanctioned decision consistent with slice AC2, not an ambiguity — acceptable.

## Per-AC evidence table

| AC | Requirement | Evidence | Result |
|----|-------------|----------|--------|
| AC1 | Local-only route UNAVAILABLE + honest reason (not "$0.00", not "add a key") | `commands::local_route_is_unavailable_with_an_honest_reason` ok (asserts reason string); `route::a_local_only_route_cannot_be_resolved...` ok (NoAdapter); reason `"Hickeyfield has no client for Local yet"` verified real at `commands.rs:212` + `provider.rs:69` | PASS |
| AC2 | z_image NOT a runnable/Generate option (HIDE) | `ModelPicker.test.tsx` 2 tests ok (non-vacuous, control model proves discrimination); `use_case::supports`→`route_serves` filters z_image out of tabs; `registry::the_unroutable_models...` pins z_image in set | PASS |
| AC3 | reachable fal/Higgsfield routes stay available; no false negatives | `commands::real_routes_stay_available_after_the_local_fix` ok; `a_usable_route_carries_no_reason` ok; drift guard ok; only z_image affected (single Local route, no dual-route model) | PASS |
| AC4 | global gate green | workspace 740/0/9; fmt exit 0; clippy exit 0; provenance exit 0; pnpm test 216; pnpm build ok | PASS |

All acceptance criteria hold, no real (fal/Higgsfield/other-keyed) route regressed, the uploader
and local-detection paths are untouched, and the full Rust + UI gate is green. I could not refute
the claim.

VERDICT: PASS
