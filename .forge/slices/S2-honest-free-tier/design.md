# S2-honest-free-tier — DESIGN

## Problem (restated from the code, not the backlog)

`ProviderId::has_adapter()` (`crates/hickeyfield-core/src/provider.rs:90-95`) returns `true`
for `ProviderId::Local`, but no Local client exists: `src-tauri/src/app.rs::client_for`
returns `None` for it (the `_ => None` arm at `app.rs:52`) and the "Local — auto-detected
ComfyUI / Ollama" section of `crates/hickeyfield-core/src/clients.rs:412-414` is an empty
comment with no client type. `has_adapter`'s own doc comment
(`provider.rs:75`) defines it as *"Whether a client exists that can actually execute a request
here"* and instructs *"Keep this list in step with `src-tauri/src/app.rs::client_for`."*
Local violates that documented invariant — it is listed aspirationally for the free tier.

The single affected model is `z_image` (`crates/hickeyfield-core/src/registry.rs:1334-1341`),
whose only route is `local:z-image`, priced `CostModel::Flat { usd: 0.0 }`. Because
`has_adapter(Local)` lies, the lie surfaces in **three** places that all consume it:

1. **Availability DTO** — `commands.rs::route_state` (`src-tauri/src/commands.rs:198-224`).
   Local passes `!has_adapter()` (adapter "present"), and `needs_key()` is `false`
   (`provider.rs:98-100`), so `route_state` returns `(true, None)` → the picker shows the
   route available at **$0.00 with a Generate affordance**.
2. **Resolver / submit** — `route::resolve` (`crates/hickeyfield-core/src/route.rs:151-154`)
   treats Local as reachable (`available.contains && has_adapter`). Because
   `vault::is_configured(Local)` is always `true` (`src-tauri/src/vault.rs:96-99` — Local
   needs nothing), `configured_providers()` includes `local`, so for `z_image` the resolver
   picks the Local route, and `app.rs::submit_to_provider` (`app.rs:174-179`) then calls
   `client_for` → `None` → the misleading `"no credentials for Local — add a key in Settings"`.
3. **Use-case filtering** — `use_case::route_serves` (`crates/hickeyfield-core/src/use_case.rs:208`)
   gates on `has_adapter`, so `supports(z_image, uc)` (`use_case.rs:166-192`) currently returns
   `true` and offers `z_image` as a runnable job option under its image tab.

All three are the same lie. The fix must tell the truth at the source.

## Recommended seam

**MUST: change `ProviderId::has_adapter()` to return `false` for `Local`** — i.e. the body
becomes `matches!(self, ProviderId::Fal | ProviderId::Higgsfield)` (option **(a)** in the slice's
evaluation). No new predicate; no per-consumer patch.

### Why (a) over (b)/(c)

- **It restores `has_adapter`'s own documented contract.** The comment at `provider.rs:75-89`
  already *says* `has_adapter` means "a client exists that can execute a request here" and must
  track `client_for`, which returns `None` for Local. This is not a redefinition — it is
  removing a member that was never true. Option **(c)** (a new `is_runnable`/`client_exists`
  predicate) would duplicate a predicate that already has this exact meaning, and would still
  have to be threaded through the same call sites.
- **It fixes all three symptoms at one edit**, including the submit-path misleading error.
  Option **(b)** (leave `has_adapter` alone; special-case Local only in `route_state`) fixes the
  DTO/picker but leaves symptom (2) latent: `resolve` still picks the Local route and
  `submit_to_provider` still produces `"no credentials for Local — add a key"`. It also leaves
  the drift guard `has_adapter_matches_the_clients_actually_implemented` (`app.rs:593-610`)
  *asserting a client for Local that does not exist* — the guard would keep passing while
  documenting a falsehood.
- **It future-proofs the re-enable path.** The slice wants "a clean path for the future
  Local/ComfyUI client to flip these back to available." With (a), the day a `LocalClient` lands
  in `clients.rs` and `client_for`, flipping `has_adapter` back to include `Local` (plus the
  drift-guard mirror) re-enables resolve, `route_serves`, *and* `route_state` in one edit — no
  special-case left to remember to delete. Special-casing `route_state` (option b) would leave a
  Local branch that must be found and removed later.

### Honest reason string (no new string; provenance-safe)

**MUST reuse the existing `!has_adapter()` branch of `route_state`
(`commands.rs:208-216`)** rather than add a Local-specific message. With (a), a Local route
falls through to that branch and yields:

> `Hickeyfield has no client for Local yet`

This is original wording (it will not match the reference corpus checked by
`scripts/lint-provenance.py` — that lint only flags CDN hosts and ≥8-word shingles lifted from
third-party product copy), it follows the established `RouteDto` unavailable-reason pattern, and
the "yet" keeps the free-tier door open. Adding no new string keeps the blast radius minimal and
avoids a Local branch to unwind when the client ships.

- **SHOULD NOT** introduce a bespoke `"Local inference isn't available yet"` arm. It reads
  slightly friendlier but adds a special-case in `route_state` that contradicts the
  future-proofing rationale above. Recorded as Open Question Q1 with this default.

### Worked example (`z_image`, machine with a fal key, no local endpoint)

| Path | Before (`has_adapter(Local)=true`) | After (`has_adapter(Local)=false`) |
|---|---|---|
| `route_state(Local, "z-image", ["fal","local"])` | `(true, None)` → tile "$0.00 / Generate" | `(false, Some("Hickeyfield has no client for Local yet"))` → disabled tile |
| `resolve([local:z-image], [Fal,Local], Cheapest, None, …)` | `Ok(local:z-image)` → submit → `"no credentials for Local — add a key"` | `Err(RouteError::NoAdapter { held:[Local] })` — no submit, no misleading key prompt |
| `supports(z_image, image-gen)` | `true` (offered as runnable) | `false` (dropped from the tab, like the 3 existing unroutable models) |
| `estimate_cost(z_image, local:z-image, …)` | `Some($0.00)` | `Some($0.00)` — **unchanged**; price is honest, availability is what was wrong |

## Files to touch

### Rust (core + shell)

- `crates/hickeyfield-core/src/provider.rs`
  - **`has_adapter()` body (:90-95)** — remove `| ProviderId::Local`. (MUST)
  - **Doc/comment (:75-95)** — no rewrite needed to the contract text (it is already correct);
    SHOULD add one sentence noting Local has no client yet so the reader is not surprised the
    keyless free tier is unreachable. The `Local` variant doc at `:29-30` ("No key, no cost")
    stays true (`needs_key` is unchanged).
- No change to `route.rs`, `use_case.rs`, `commands.rs::route_state`, `clients.rs`, or
  `app.rs::client_for` production code — they consume `has_adapter` and inherit the correction.
  `client_for` already returns `None` for Local. (Recorded: **N/A by design** — the seam is a
  single predicate the others already read.)

### Rust tests — UPDATE (intended flips; each is the assertion that encoded the bug)

- `src-tauri/src/app.rs`
  - `has_adapter_matches_the_clients_actually_implemented` (:593-610) — the hand-mirrored match
    at :599-602 MUST drop `ProviderId::Local`, becoming `Fal | Higgsfield`. This makes the drift
    guard *true* against `client_for`'s real arms.
  - `local_is_reachable_without_a_credential` (:612-619) — MUST flip: assert
    `!ProviderId::Local.has_adapter()` while keeping `assert!(!ProviderId::Local.needs_key())`
    (the free-tier-keyless fact survives; only the adapter claim was false). Rename to something
    like `local_needs_no_key_but_has_no_client_yet`.
- `src-tauri/src/commands.rs`
  - `local_is_available_with_no_key_at_all` (:1198-1203) — MUST flip to assert
    `route_state(Local, "comfy/x", &[])` returns `(false, Some(reason))` where `reason`
    contains `"no client"` and does **not** contain `"Needs a"`. This becomes the AC1 test;
    rename `local_route_is_unavailable_with_an_honest_reason`.
- `crates/hickeyfield-core/src/registry.rs`
  - `the_unroutable_models_are_a_known_short_list` (:2280-2296) — MUST add `"z_image"` to the
    expected set, giving sorted `["image_background_remover", "nano_banana", "outpaint", "z_image"]`.
    This test exists precisely to pin the adapter-gap blast radius; growing it by `z_image` is
    the intended, reviewed record of this change.

### Rust tests — ADD

- `src-tauri/src/commands.rs` (AC1 + AC3): the flipped test above is AC1. Add/confirm an AC3
  assertion in the same module that a **reachable** route stays available — reuse or mirror
  `a_usable_route_carries_no_reason` (:1187-1195): `route_state(Fal, "fal-ai/nano-banana-2",
  &["fal"])` → `(true, None)`. A single new test `real_routes_stay_available_after_the_local_fix`
  MAY assert both halves (Local unavailable, a keyed fal route available) so AC1 and AC3 sit
  together.
- `crates/hickeyfield-core/src/route.rs` (SHOULD, resolver reinforcement): a test that a
  Local-only route set with Local "available" resolves to `Err(RouteError::NoAdapter …)` rather
  than `Ok`, pinning that the submit path can no longer pick an unrunnable Local route.

### UI (React)

- `ui/src/components/ModelPicker.tsx` — **no production change required.** It already renders the
  unavailable state from the `RouteDto`: `usable = routes.find(r => r.available !== false)`
  (:49), main button `disabled={!runnable}` (:61), subtitle falls back to
  `routes[0]?.unavailableReason` (:75-78), and each route chip is `disabled={!ok}` with
  `data-unavailable` and `title={route.unavailableReason}` (:83-101). The data mapping is in
  `ui/src/api.ts::toRoute` (:413-429): `available: raw.available ?? true`,
  `unavailableReason: raw.unavailable_reason ?? …`. Types in `ui/src/types.ts:11-21`.
  Recorded: **N/A — the component already honors `available:false`; this slice supplies correct
  data and pins the render.**

### UI tests — ADD (AC2)

- `ui/src/components/ModelPicker.test.tsx` (new). vitest environment is already `jsdom` with
  `globals:true` (`ui/vite.config.ts:40-42`), and `react`/`react-dom` are dependencies, so the
  test renders with `react-dom/client` `createRoot` into a container — **no new tooling** (the
  repo has no React Testing Library; existing component tests such as
  `ui/src/components/CameraPreview.test.ts` test pure functions, so this introduces a small
  render pattern, kept to attribute/text assertions to stay robust). It MUST:
  - Render `ModelPicker` with a `z_image`-shaped fixture: one route
    `{ id:"local:z-image", provider:"local", slug:"z-image", available:false,
    unavailableReason:"Hickeyfield has no client for Local yet" }`.
  - Assert the model's main button carries `disabled` and the route chip carries
    `data-unavailable` / `disabled`, that the unavailable reason text is rendered, and that no
    enabled Generate/select affordance exists for it (i.e. clicking does not fire `onSelect`).
  - Also render a control model whose route has `available:true` and assert its button is
    **enabled** — the UI-level AC3 guard that the fix does not grey out real routes.

## Acceptance-criteria → evidence map

- **AC1** — `has_adapter(Local)=false` routes Local through `route_state`'s existing no-client
  branch. Rust test: flipped `commands.rs::local_route_is_unavailable_with_an_honest_reason`
  asserts `(false, reason)` with an honest, non-"add a key" reason.
- **AC2** — `ui/src/components/ModelPicker.test.tsx` asserts the tile renders disabled/greyed
  with the reason and no Generate affordance.
- **AC3** — existing guards stay green and are cited: `commands.rs::a_usable_route_carries_no_reason`
  (fal+key available), `commands.rs::a_key_without_a_client_is_not_reported_as_available`
  (Vaig still unavailable, unchanged), `route.rs::a_cheaper_route_we_cannot_execute_never_wins`,
  `registry.rs::cheapest_policy_picks_the_cheapest_executable_route`. New UI control-model
  assertion adds a UI-level AC3 guard.
- **AC4 (regression)** — this slice touches Rust *and* the React UI, so the gate includes
  `cargo test --workspace`, `cargo fmt --all --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`, `./scripts/lint-provenance.py`,
  **`pnpm build`** (`tsc --noEmit && vite build`) and **`pnpm test`** (`vitest run`). Net Rust
  test count rises (one added Rust test; the flips are edits not additions) — keep the workspace
  ≥ the slice's stated `727` floor.

## Blast radius (every caller of the changed function)

`has_adapter` is read in exactly these production sites; each inherits the correction:

- `route.rs::resolve` — `:133` (pin usability), `:153` (reachable filter), `:162` (held set),
  `:172` (needed set). Net effect: Local becomes unreachable/unpinnable. Only `z_image` has a
  Local route and it is Local-only, so the only behavioural change is `z_image` resolving to
  `NoAdapter` instead of `Ok(local:z-image)`.
- `route.rs::cheaper_alternative` — `:220`. Local no longer suggested as a cheaper alternative.
  No model routes both to Local and to something else, so this is inert in practice.
- `use_case.rs::route_serves` — `:208`; `use_case.rs` `:177` is the Higgsfield-rescue clause
  (Local irrelevant). Net effect: `z_image` drops out of `models_for_use_case` tabs.
- `commands.rs::route_state` — `:208`. Net effect: Local route reported unavailable + reason.

Verified **unaffected** (read before asserting): `registry.rs::every_launch_model_can_actually_be_run`
and the launch-set tests — `z_image` is **not** in `LAUNCH_FAMILIES` (`registry.rs:204-223`), so
its loss of an executable route is allowed; `registry.rs::a_local_route_is_free_and_needs_no_key`
(:2352-2362) asserts routing/pricing data, not `has_adapter`, and stays green; all `route.rs`
resolve/`cheaper_alternative` tests never place `Local` in the `available` slice; no `use_case.rs`
test references `z_image`/`Local`/`route_serves`; `is_ready` (`commands.rs:47-54`) uses
`detect_local()`, not `has_adapter`; the runner/engine do not consume `has_adapter`.

## Non-goals

- **No Local/ComfyUI client.** `clients.rs:412-414` stays empty; building a real Local adapter is
  a separate slice. This slice only makes the *absence* honest.
- **No pricing change.** `z_image` keeps `CostModel::Flat { usd: 0.0 }`; $0.00 is the true price
  *once a local client exists*. The bug was availability, not price.
- **No `RouteError`/DTO shape change.** We reuse `RouteError::NoAdapter` and the existing
  `RouteDto { available, unavailable_reason }` contract untouched.
- **No change to `needs_key`/`is_configured`/`is_ready`.** Local remains keyless and still counts
  as "configured"; readiness still keys off detected endpoints.

## Open questions (each with a recorded default)

- **Q1 — Reason wording: reuse the generic no-client message, or a Local-specific line?**
  DEFAULT: **reuse** `"Hickeyfield has no client for Local yet"` (no new string, provenance-safe,
  future-proof — see "Honest reason string"). A tailored `"Local inference isn't available yet"`
  is acceptable but adds a `route_state` special-case to unwind when the client ships.
- **Q2 — With (a), `z_image` is filtered out of use-case tabs by `use_case::supports`, not shown
  as a disabled tile there.** This matches the app's existing treatment of the other three
  unroutable models (`nano_banana`, `outpaint`, `image_background_remover`) and its stated
  "don't offer what submit would refuse" philosophy (`commands.rs:397-401`, `App.tsx:269-271`),
  but it differs from objective **Q4**'s default ("keep visible but render an explicit
  unavailable state"). DEFAULT: **accept removal from the job tab**; the unavailable-tile
  behaviour is still exercised by the flat `list_models()` view and pinned by the AC2 vitest
  test. If the verifier requires a live disabled tile *inside the use-case tab*, that is a
  follow-up decoupling "shown in tab" from "executable" and is out of this slice's scope. **This
  is the single riskiest decision in the slice** (see below).

## Weakest parts (verifier: start here)

1. **Q2 above — the visible-vs-removed tension.** Option (a) removes `z_image` from the
   use-case tab rather than showing a disabled tile there. It is internally consistent (same as
   the 3 existing unroutable models) and the ACs are met by the vitest component test + the flat
   list view, but it is a defensible deviation from objective Q4's recorded default. If that
   default is treated as binding, option (b) (special-case `route_state`, keep `has_adapter`)
   would keep the tile visible-disabled in the tab — at the cost of leaving the misleading
   submit-path error latent and the drift guard semantically false.
2. **AC2 test pattern.** The repo has no React Testing Library and no prior rendered-component
   test; the new `ModelPicker.test.tsx` renders via `react-dom/client` under jsdom. Assertions
   are kept to DOM attributes (`disabled`, `data-unavailable`) and text content to avoid
   brittleness and `act()` churn. If a rendered-DOM test proves flaky, the fallback is to extract
   the picker's `usable`/`runnable` decision (`ModelPicker.tsx:49-50`) into a pure helper and
   unit-test that in the style of `CameraPreview.test.ts`.
