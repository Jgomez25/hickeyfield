# S2-honest-free-tier — IMPLEMENT claim

Branch: `forge/s2-honest-free-tier` (created off `forge/s1-job-engine-reliability` HEAD).
Approach: Option (a) per design + owner decision **HIDE** — `has_adapter()` returns `true`
only for providers with a real client (`Fal | Higgsfield`); `Local` dropped. The lie surfaced
in three places (`route_state`, `route::resolve`, `use_case::route_serves`); one predicate edit
corrects all three. No new reason string introduced — the existing
`"Hickeyfield has no client for {display_name} yet"` branch of `route_state`
(`commands.rs:208-216`) now covers Local, yielding `"Hickeyfield has no client for Local yet"`.

## Files changed

### Production (1 file)
- `crates/hickeyfield-core/src/provider.rs` — **AC1/AC2/AC3 root fix.**
  `has_adapter()` body changed from `Fal | Higgsfield | Local` to `matches!(self, Fal | Higgsfield)`.
  Added a doc paragraph noting `Local` is keyless but has no client yet, so `z_image` is
  unreachable until a local adapter lands (re-enable = add `Local` back here). No signature/DTO
  change; `client_for` (`app.rs:52 _ => None`) already returned `None` for Local and is untouched.

### Tests — flipped to the new truth (were asserting the old lie)
- `src-tauri/src/app.rs`
  - `has_adapter_matches_the_clients_actually_implemented` (drift guard) — hand-mirrored match
    dropped `ProviderId::Local` → now `Fal | Higgsfield`, so it is true against `client_for`.
  - `local_is_reachable_without_a_credential` → **renamed** `local_needs_no_key_but_has_no_client_yet`:
    keeps `assert!(!ProviderId::Local.needs_key())` (free-tier-keyless fact survives) and flips the
    adapter claim to `assert!(!ProviderId::Local.has_adapter())`.
- `src-tauri/src/commands.rs`
  - `local_is_available_with_no_key_at_all` → **renamed/reworked** `local_route_is_unavailable_with_an_honest_reason`
    (**AC1**): `route_state(Local, "z-image", &[])` returns `(false, Some(reason))` where `reason`
    contains `"no client"` and does NOT contain `"Needs a"`.
- `crates/hickeyfield-core/src/registry.rs`
  - `the_unroutable_models_are_a_known_short_list` — expected set grew by `"z_image"` →
    sorted `["image_background_remover", "nano_banana", "outpaint", "z_image"]`. This is the
    reviewed record of the blast radius.

### Tests — added
- `src-tauri/src/commands.rs` — `real_routes_stay_available_after_the_local_fix` (**AC3**):
  a keyed fal route (`route_state(Fal, "fal-ai/nano-banana-2", &["fal"])`) stays `(true, None)`
  while the Local `z-image` route is `(false, _)`. AC3 is also still guarded by the pre-existing
  `a_usable_route_carries_no_reason`.
- `crates/hickeyfield-core/src/route.rs` — `a_local_only_route_cannot_be_resolved_until_a_client_exists`
  (resolver reinforcement): a Local-only route set with Local "available" now resolves to
  `Err(RouteError::NoAdapter { held: [Local] })` instead of `Ok`, so submit can no longer pick an
  unrunnable Local route (kills the misleading `"no credentials for Local — add a key"` path).
  Also updated the stale `routes()` fixture doc-comment (it claimed Local was executable).
- `ui/src/components/ModelPicker.test.tsx` (new, **AC2 + UI AC3**): renders `ModelPicker` via
  `react-dom/client` `createRoot` under the existing jsdom vitest setup (repo has no RTL). A
  `z_image`-shaped model (single `local:z-image` route, `available:false`,
  `unavailableReason:"Hickeyfield has no client for Local yet"`) renders with the main tile
  `disabled`, `data-unrunnable="true"`, the route chip `disabled` + `data-unavailable="true"` +
  `title` = the reason, the reason text present in the DOM, and clicking either disabled affordance
  fires no `onSelect`. A control fal model with `available:true` renders enabled and clicking it
  fires `onSelect("nano_banana_2", "fal:fal-ai/nano-banana-2")`.

No production change to `route.rs` / `use_case.rs` / `commands.rs::route_state` / `clients.rs` /
`app.rs::client_for` / `ModelPicker.tsx` / `api.ts` — they consume `has_adapter` (or already honor
`available:false`) and inherit the correction, exactly as the design predicted.

## Acceptance criteria — evidence (commands + observed output)

ENV for Rust: `source "$HOME/.cargo/env"` + dummy `hickeyfield_*_KEY=x` overrides (avoids the
`commands::tests` macOS Keychain hang). ENV for UI: Node 24 + pnpm shim on PATH.

### AC1 — Local-only route reports UNAVAILABLE with an honest reason
```
$ cargo test -p hickeyfield-tauri  (grep)
test commands::tests::local_route_is_unavailable_with_an_honest_reason ... ok
test app::tests::local_needs_no_key_but_has_no_client_yet ... ok
$ cargo test -p hickeyfield-core a_local_only_route_cannot_be_resolved
test route::tests::a_local_only_route_cannot_be_resolved_until_a_client_exists ... ok
test result: ok. 1 passed; 0 failed; ...
```
Reason string is the reused existing one, `"Hickeyfield has no client for Local yet"` (no new
string; provenance lint clean — see AC4).

### AC2 — z_image / Local-only is NOT a runnable/Generate option in the picker
```
$ cd ui && pnpm test  (excerpt)
 ✓ src/components/ModelPicker.test.tsx (2 tests) 118ms
 Test Files  11 passed (11)
      Tests  216 passed (216)
```
z_image tile is disabled with the reason and fires no `onSelect`. At the data layer it is filtered
out of use-case tabs by `use_case::supports` (like the other 3 unroutable models) and pinned as
unroutable by `registry::the_unroutable_models_are_a_known_short_list`.

### AC3 — reachable routes (fal/Higgsfield with a key) stay available
```
$ cargo test -p hickeyfield-tauri  (grep)
test commands::tests::real_routes_stay_available_after_the_local_fix ... ok
```
Plus `commands::tests::a_usable_route_carries_no_reason` (unchanged, still green) and the UI
control-model assertion in `ModelPicker.test.tsx` (enabled + selectable). The drift guard
`has_adapter_matches_the_clients_actually_implemented` confirms Fal/Higgsfield still have adapters.

### AC4 — global gate green
```
$ cargo test --workspace
=== TOTAL passed: 740  failed: 0   (plus 9 ignored)     # >= 727 floor
$ cargo fmt --all --check
FMT OK
$ cargo clippy --workspace --all-targets -- -D warnings
    Finished `dev` profile ...        # no warnings
$ ./scripts/lint-provenance.py
  PASS  no third-party media hosts referenced
  PASS  no shipped string matches the reference corpus (80015 shingles indexed)
provenance exit: 0
$ cd ui && pnpm test
 Test Files  11 passed (11)   Tests  216 passed (216)
$ cd ui && pnpm build          # tsc --noEmit && vite build
 ✓ built in 799ms
```
`tauri build` deliberately NOT run (verified at release, per instructions).

## Known limitations / honest gaps
- **z_image is filtered OUT of use-case tabs rather than shown as a disabled tile inside a tab.**
  This is the design's Q2 default and matches the app's existing treatment of the other 3
  unroutable models (`nano_banana`, `outpaint`, `image_background_remover`) and the owner's HIDE
  decision (AC2). It is a defensible deviation from objective Q4's "keep visible, render unavailable
  in-tab" default. The unavailable+reason state IS still exercised in the flat `ModelPicker`
  list-view (AC2 vitest test) and at the data layer (`route_state`, AC1). If the verifier treats
  Q4 as binding, showing an in-tab disabled tile is a follow-up that decouples "shown in tab" from
  "executable" — out of this slice's scope.
- No Local/ComfyUI client built (explicit non-goal). No pricing change: `z_image` keeps
  `CostModel::Flat { usd: 0.0 }`; the bug was availability, not price. No `RouteError`/DTO shape
  change. No change to `needs_key`/`is_configured`/`is_ready`.
- `.forge/slices/S2-honest-free-tier/{slice.md,design.md}` show as modified in the working tree;
  those were pre-existing (carried from the branch point) and I did not touch them.
