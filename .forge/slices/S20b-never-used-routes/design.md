# S20b-never-used-routes — DESIGN

## Revision 1 (2026-09-19) — design-gate FAIL C11/C12 repair
The gate failed because A3.3 changed the two picker-feed commands to take `State` and filter
never-used ids by default, but the collateral-test plan listed only UI vitests. Five in-crate Rust
tests call `list_models()` with no args (compile break E0061), and
`the_catalogue_is_exposed_and_launch_models_come_first` asserts `models.len() >= 55` against the
~57 roster — which the default (pref-OFF) filter would drop to ~46, a semantic break — while the
design claimed the 971-test Rust baseline stays green (A7/G1). This revision:
1. **Chooses a pure, `State`-free inner seam:** `models_matching(show_never_used: bool, use_case)` is
   a pure fn; the `#[tauri::command]` wrappers `list_models`/`models_for_use_case` read the pref from
   the store and delegate. Rust tests call the pure fn directly (no `State`, no store needed to
   exercise the filter). Replaces the earlier `models_matching(store: &SqliteStore, use_case)`.
2. Adds **§Collateral Rust test updates** naming all five callers by test name + line, today's
   assertion, chosen mode (`show_never_used = true` to keep the full-catalogue intent honest), and the
   exact repair — plus one NEW default-hidden test whose expected count is **derived** (`full − 11`),
   not a magic number.
3. Folds in the verifier's non-blocking notes: `ui/src/components/Storyboard.tsx` at line **133**
   (was `:134`); mock ids deliberately differ from registry ids (`veo_3_1_fast` vs `veo3_1_fast`,
   `kling_3_0` vs `kling3_0`) and stay distinct; `a_subtitle_describes_the_model…` (`:2527`) reads the
   unfiltered registry and MUST NOT be swept.
4. Records the tauri `generate_handler!` additions for the three NEW commands (`model_names`,
   `get_show_never_used`, `set_show_never_used`) and re-confirms `api.ts` invoke calls (`:504,:629`)
   pass no arg, so the `State`-injection wrapper change is client-invisible.

Everything the gate confirmed is unchanged: migration 12 justified (C4); 11-id set (C2); prefix
predicate boundary (C3); A4 refusal at the `submit_one` funnel (C5); mock/samples lead with the
workhorse (C8/#6).

---

Contract: `.forge/slices/S20b-never-used-routes/slice.md` (A1–A7, G1–G2, out-of-scope).
Upstream: `.forge/objective.md` §R0.3 (WORKHORSE), §R-RULES; `.forge/STATUS.md` "Test-locked class
names to KEEP" (:118–120). House style: `.forge/slices/S20-workhorse-billing-accuracy/design.md`.
Built on the S18 fake-provider seam (`src-tauri/src/app.rs:260`, `submit_to_provider`'s injected
`clients`) and the S20 `Route.promo` / `RouteDto.promo_as_of` fields (already landed,
`route.rs`, `commands.rs:480`). This artifact changes no source; it specifies the implementer's
edits. The verifier starts at §Riskiest.

**WORKHORSE = `minimax_h3_max_turbo`** (`fal:minimax/h3-max-turbo/{text,image}-to-video`, priced
`registry.rs:1175`; native audio + lipsync; bills on OUTPUT seconds). Owner's ONLY paid video route.
**Owner rules, firm: Seedance is NEVER used; Grok paid API is NEVER used.** Every Seedance and every
Grok id (video AND image) is a "never-used route". Owner depth: **Demote + hide** — hidden from
pickers behind a persisted setting (default OFF), NOT deleted (registry, prices, capability tables,
S20's `grok_video_v15` settle test all stay).

## 0. Session constraint & ground truth (read, not run)
Authored with read-only tools — no `cargo`/`pnpm` ran. Every `file:line` was read from this worktree
and is first-hand. Any predicted count/behaviour is a hypothesis the implementer's gate confirms; if
a run disagrees, **the run wins** and the design is amended in `implement.md`.

### The exhaustive never-used id set (enumerated from the registry, verified 2026-09-19)
`registry()` (`registry.rs:99–116`) assembles the catalogue minus `EXCLUSIONS` plus
`picker_only_specs`. Cross-checking every `seedance*`/`grok*` reference (grep, priced-route arms
`registry.rs:678–1369`, `picker_only_specs` `:1999,:2026`, the `other => higgsfield_only` fallback
`:1609`), the registry contains **exactly these 11 ids**:

| Family | ids in the registry | route home |
|---|---|---|
| Seedance (6) | `seedance1_5`, `seedance_2_0`, `seedance_2_0_fast`, `seedance_2_0_mini`, `seedance_2_5`, `seedance_pro` | fal + vaig literals `registry.rs:678–790` |
| Grok (5) | `grok_video`, `grok_video_v15`, `grok_image`, `grok_edit_video`, `grok_extend_video` | `grok_video` via `higgsfield_only` fallback `:1609`; others fal literals `:937,:980,:1110,:1361` |

`grok_video` has no explicit priced-route arm — it is catalogued (`vendor/higgsfield-cli-MODELS.md`)
and priced through the `other =>` fallback, so it IS a real assembled model and MUST be covered.
No other id begins `seedance`/`grok`. **The predicate is prefix-based** so a future
`seedance_*`/`grok_*` id is auto-covered; the exhaustive test (A3) proves the prefix matches exactly
these 11 and nothing else.

### Facts the changes rest on
| Fact | Read at |
|---|---|
| `LAUNCH_FAMILIES: [_;12]` has a `"Seedance 2.0"` row (3 ids) and NO MiniMax H3 entry | `registry.rs:214–233` |
| `launch` marker derived by membership scan of `LAUNCH_FAMILIES` | `registry.rs:140–142` |
| test `there_are_twelve_launch_families` asserts `LAUNCH_FAMILIES.len()==12`; `only_launch_models_carry_the_marker` sums `ids.len()` | `registry.rs:2775,2824–2833` |
| the command boundary where use-case filtering already lives | `commands.rs:451–494` `models_matching` |
| `list_models` / `models_for_use_case` are the two picker feeds; both call `models_matching`; neither takes `State` today | `commands.rs:436–449` |
| the only callers of `list_models()`/`models_for_use_case()`/`models_matching()` are `commands.rs` (2 wrappers + pure fn + 5 tests at `:2291,:2309,:2337,:2356,:2520`); **zero** callers in `crates` | grep `src-tauri/src`+`crates`, Rev 1 |
| all Tauri commands are registered in one `generate_handler![…]` list (incl. `list_models` `:87`, `models_for_use_case` `:89`); adding a command requires a line here | `lib.rs:79–109` |
| `api.ts` invoke sites pass no store/pref arg: `invoke("list_models")` `:504`, `invoke("models_for_use_case", {useCase})` `:629` | `ui/src/api.ts` |
| every spend enters through `submit_one` (standalone, storyboard shots, keyframe regen, chain auto-retry) — it holds `store: &Arc<SqliteStore>` | `commands.rs:1096–1104`, called `:1029,:1838,:1910,:2078`, `chain.rs` |
| id→model lookups that DON'T filter (must keep working for hidden models): `model_capabilities` `:540`, `estimate_with` `:711`, `preview_prompt` `:985`, `submit_job` `:1023` | `commands.rs` |
| store migrations are append-only, currently at **11**; no app-level key/value pref table exists (the `settings` column is per-job) | `store.rs:202–401,1049` |
| only two persistence mechanisms exist: client `localStorage` (`App.tsx:108,786`, key `hickeyfield.onboarding.skipped`) and a per-pref config FILE reachable only via `AppHandle` (`app.rs:95–106` library-root) | as cited |
| UI initial model = first `isLaunch` video (`?? m[0]`); tab-change fallback = first `isLaunch` (`?? m[0]`) | `App.tsx:194,336` |
| storyboard new-shot default = `models[0]?.id` | `ui/src/components/Storyboard.tsx:210`; planner prefs = `models.map(m=>m.id)` `:173` |
| history name resolution reads the filtered `models` list, falls back to raw id | `App.tsx:691–697` `featuredModelName`; `ui/src/components/Storyboard.tsx:133` `modelName` (correct path + line per verifier) |
| mock/blurb hits to sweep | `mock.ts:88–103,216–394,430,490–496`; `browser.ts:547–548`; `onboarding.rs:43,53,83`; `providers.ts:44,84` |
| vitests pinned to seedance fixtures | `Stage.test.tsx`, `SettingsRail.test.tsx:36–40,139`, `Library.test.tsx:21`, `presets.test.ts:36,83`, `browser.test.ts:47,582–587` |

---

## A2 — launch set: workhorse in, Seedance/Grok out (real gap)

**Change `registry.rs`.** Add a named workhorse constant and replace the Seedance launch row:

```rust
/// The everyday paid video route (objective R0.3). The one id every default,
/// mock and sample leads with; also the anchor of the launch set.
pub const WORKHORSE_MODEL_ID: &str = "minimax_h3_max_turbo";
```

In `LAUNCH_FAMILIES` (`:214–233`) replace the whole `("Seedance 2.0", &["seedance_2_0",
"seedance_2_0_fast", "seedance_2_0_mini"])` row with:

```rust
("MiniMax H3 Max Turbo", &[WORKHORSE_MODEL_ID]),
```

- Array stays `[_; 12]` → `there_are_twelve_launch_families` (`:2775`) stays green with **no edit**.
- `only_launch_models_carry_the_marker` (`:2824`) sums `ids.len()`; it recomputes from the array, so
  it stays green (launch id count 14→12; the test is self-referential, not a literal).
- MUST: `LAUNCH_FAMILIES` now contains `WORKHORSE_MODEL_ID` and NO id with prefix `seedance`/`grok`.

Back-compat: `LAUNCH_FAMILIES` is compile-time data; **N/A — no migration, no serde.**

---

## A3 — the core predicate + the command-boundary filter (real gap)

### A3.1 The predicate (core, `registry.rs`)
Add beside `launch_models` (`:118`):

```rust
/// True for a route the owner has marked never-used (Seedance and Grok, video
/// AND image). Prefix-based so a future `seedance_*`/`grok_*` id cannot slip in
/// unhidden. Hidden ≠ deleted: the id stays priced, capable and testable.
pub fn is_never_used(model_id: &str) -> bool {
    model_id.starts_with("seedance") || model_id.starts_with("grok")
}
```

MUST be pure and clockless. Worked example: `is_never_used("grok_video")==true`,
`is_never_used("seedance_2_5")==true`, `is_never_used("minimax_h3_max_turbo")==false`,
`is_never_used("seedream_v5_pro")==false` (Seedream is ByteDance's image line, not Seedance — the
prefix boundary the exhaustive test guards).

### A3.2 The persisted setting (server-authoritative)
**Decision — migration 12 (justified).** A4 requires the SERVER to refuse a re-run at the spend
boundary. That boundary is `submit_one`, which holds `store` but **no `AppHandle`**. The two existing
mechanisms cannot host the toggle there: `localStorage` is client-only (a client-passed bool is a
UI-only guard, which A4 forbids); the library-root config FILE is reachable only via `AppHandle`,
which `submit_one` lacks. The store is already threaded through every model-list and every submit
path, so an append-only key/value table is the minimal reuse-consistent home. This is the "settings
store genuinely needs one" case A7 sanctions.

`store.rs` migration **12** (next after 11), append-only:

```sql
CREATE TABLE IF NOT EXISTS app_pref (key TEXT PRIMARY KEY, value TEXT NOT NULL) STRICT;
```

with two methods on `SqliteStore`:

```rust
pub fn get_bool_pref(&self, key: &str, default: bool) -> bool; // absent row ⇒ default
pub fn set_bool_pref(&self, key: &str, on: bool) -> rusqlite::Result<()>;
```

One key constant: `pub const SHOW_NEVER_USED: &str = "show_never_used";`, **default `false`**.
Back-compat: a pre-S20b DB has no `app_pref` table; migration 12 creates it; every existing row is
untouched; the default reads OFF. Existing migration/idempotency tests (`store.rs:1423`) extend to
version 12.

### A3.3 The command-boundary filter — a pure, `State`-free inner seam (`commands.rs`)
**Seam decision (Revision 1).** The filter lives in a **pure fn** that takes a plain `bool`, not a
`State` or `&SqliteStore`. This is the testable seam: Rust tests exercise the filter by passing the
bool directly (no store, no `State` to construct), and the store is read only in the thin command
wrappers. `models_matching` today is `fn models_matching(use_case: Option<UseCase>)` (`:451`, no
`store`/`State`); it gains a leading `show_never_used: bool` param and one filter clause:

```rust
fn models_matching(show_never_used: bool, use_case: Option<hickeyfield_core::UseCase>) -> Vec<ModelDto> {
    registry().into_values()
        .filter(|m| show_never_used || !hickeyfield_core::registry::is_never_used(&m.id))
        .filter(|m| match use_case { Some(uc) => use_case::supports(m, uc), None => true })
        ...
}
```

The two `#[tauri::command]` wrappers gain `state: State<'_, AppState>` (Tauri injects it server-side;
the JS `invoke("list_models")` / `invoke("models_for_use_case", {useCase})` signatures are unchanged —
`api.ts:504,:629` pass no store arg, so **no `api.ts` change**). Each reads the pref once and delegates:

```rust
#[tauri::command]
pub fn list_models(state: State<'_, AppState>) -> Vec<ModelDto> {
    let show = state.store.get_bool_pref(store::SHOW_NEVER_USED, false);
    models_matching(show, None)
}
#[tauri::command]
pub fn models_for_use_case(state: State<'_, AppState>, use_case: String) -> Vec<ModelDto> {
    let show = state.store.get_bool_pref(store::SHOW_NEVER_USED, false);
    models_matching(show, hickeyfield_core::UseCase::from_slug(&use_case))
}
```

The hide clause goes **before** the use-case clause (cheapest, order-independent). MUST: with
`show_never_used == false`, no `ModelDto.id` satisfies `is_never_used`; with it `true`, the list is
exactly today's list. The existing sort (`:488`, launch-first then name) is unchanged. `AppState.store`
is `Arc<SqliteStore>` (`app.rs:20–21`); the wrappers deref it for `get_bool_pref`.

---

## A4 — no dead ends: history renders; the server refuses a hidden re-run

### The A4 decision — separate unfiltered display-name lookup (NOT a `hidden` DTO flag)
A3 mandates the picker feeds **omit** hidden ids at the command boundary, so a historical job on a
hidden model is absent from `models` and `featuredModelName`/`modelName` fall back to the raw id
(`App.tsx:695`, `ui/src/components/Storyboard.tsx:133`). A `hidden: true` flag on `ModelDto` with client-side
filtering is **rejected**: it would keep never-used ids in the picker feed (violating A3's server-side
omission) and push the guard into the UI. Instead add one tiny **unfiltered** display-name command:

```rust
#[derive(Serialize)]
pub struct ModelNameDto { pub id: String, pub display_name: String }

#[tauri::command]
pub fn model_names() -> Vec<ModelNameDto> {  // EVERY registry model, hidden or not
    registry().into_values()
        .map(|m| ModelNameDto { id: m.id.clone(), display_name: m.display_name.clone() })
        .collect()
}
```

- **Register `model_names` in `generate_handler![…]` (`lib.rs`, after `list_models` `:87`)** — a new
  `#[tauri::command]` is unreachable from JS until listed there.
- `api.ts`: `export async function modelNames(): Promise<Record<string,string>>` (invoke →
  fold to `{id: displayName}`; mock mode folds `MOCK_MODELS`). App fetches it once in the startup
  `Promise.all` (`App.tsx:183`) into a `modelNames` map.
- `App.tsx` `featuredModelName` (`:691`): resolve `modelNames[featured.modelId] ??
  models.find(...)?.displayName ?? featured.modelId`. `ui/src/components/Storyboard.tsx` `modelName` (`:133`): same
  map-first fallback. A hidden model's history now shows "Seedance 2.0", never `seedance_2_0`.
- **Cost** needs no new path: a job's estimated/actual USD is recorded on the `JobSet` and rendered
  from the job record (`MetaCard`), not recomputed from the picker list. Re-estimating a hidden
  model still works because `estimate_with`/`model_capabilities` resolve by id unfiltered (`:711,:540`).

### The server-side refusal (the spend guard)
Top of `submit_one` (`commands.rs:1096`, before compile/estimate/route), using the `store` it already
holds:

```rust
if hickeyfield_core::registry::is_never_used(&model.id)
    && !store.get_bool_pref(store::SHOW_NEVER_USED, false) {
    return Err(format!(
        "{} is a never-used route and is hidden. The owner marked Seedance and Grok as \
         never used because this app bills the provider for them. Turn on \"Show never-used \
         routes\" in Settings to run it.",
        model.display_name));
}
```

This covers EVERY entry (standalone re-run, storyboard shot, keyframe regen, chain auto-retry) with
one check, because all of them funnel through `submit_one`. It is a server refusal — the UI hiding
the button is not relied on. MUST: a hidden model with the toggle OFF returns `Err(..)` naming the
toggle and never reaches `route::resolve` or a client; with the toggle ON it proceeds unchanged.

Worked example: a job row `modelId:"seedance_2_0"` exists from before the sweep. History shows
"Seedance 2.0" + its recorded `$1.71`. Pressing Regenerate (toggle OFF) → the Err above; no socket,
no spend. Owner flips the toggle → the same Regenerate proceeds.

---

## A1 — startup + fallback + new-shot default = the workhorse (real gap)

The launch marker (A2) makes the workhorse `isLaunch`, but the picker sort is alphabetical among
launch models, so "first isLaunch video" is not guaranteed to be the workhorse. A1 requires the
workhorse **by name**. One constant per language.

- **TS constant** — `ui/src/types.ts`: `export const WORKHORSE_MODEL_ID = "minimax_h3_max_turbo";`
- `App.tsx:194` (initial): `const first = m.find(x => x.id === WORKHORSE_MODEL_ID) ?? m.find(x =>
  x.isLaunch && x.modality === "video") ?? m[0];`
- `App.tsx:336` (tab fallback): `const next = m.find(x => x.id === WORKHORSE_MODEL_ID) ??
  m.find(x => x.isLaunch) ?? m[0];` (workhorse used only when it survived the tab's use-case filter,
  which the `?? m.find(isLaunch)` fallback guarantees).
- `Storyboard.tsx:210` (new shot): `modelId: last?.modelId ?? (models.some(m => m.id ===
  WORKHORSE_MODEL_ID) ? WORKHORSE_MODEL_ID : models[0]?.id) ?? ""`.

Planner default is already correct once the filter lands: `Storyboard.tsx:173` passes
`modelPrefs: models.map(m=>m.id)` — with Seedance/Grok filtered out and the workhorse leading, the
LLM is never offered a never-used route. **No core planner change.** `storyboard.rs` `plan()` is
LLM-driven from prefs (`:539,:594`); N/A.

**Rust constant** — `WORKHORSE_MODEL_ID` (added in A2) is referenced by the `LAUNCH_FAMILIES` row and
pinned by an A1 Rust test asserting it is a launch video model that resolves to a route.

---

## A5 — the toggle (UI, `SettingsRail.tsx` + api)

Add one labelled control to the model settings area (`SettingsRail.tsx`, above `.rail-disclaimer`
`:322`, additive — no test-locked selector touched; the locked names `.model-item*`, `.chip-route`,
`.enhancer-*`, `.prompt-preview-*`, `.generate-button` per STATUS.md:118–120 are untouched):

```tsx
<label className="never-used-toggle">
  <input type="checkbox" checked={showNeverUsed} onChange={e => onToggleNeverUsed(e.target.checked)} />
  Show never-used routes (Seedance, Grok)
  <span className="never-used-warn">These bill the provider — hidden by default.</span>
</label>
```

- `api.ts`: `getShowNeverUsed(): Promise<boolean>` (invoke `get_show_never_used`, mock ⇒ read
  `localStorage["hickeyfield.showNeverUsed"]`) and `setShowNeverUsed(on: boolean): Promise<void>`
  (invoke `set_show_never_used {on}`, mock ⇒ write localStorage).
- `commands.rs`: `get_show_never_used(state) -> bool` = `state.store.get_bool_pref(SHOW_NEVER_USED,
  false)`; `set_show_never_used(state, on: bool) -> Result<(),String>` = `set_bool_pref`.
- **Register both `get_show_never_used` and `set_show_never_used` in `generate_handler![…]`
  (`lib.rs`)** alongside `model_names` — three new command lines total.
- `App.tsx`: `showNeverUsed` state seeded from `getShowNeverUsed()` at startup; `onToggleNeverUsed`
  calls `setShowNeverUsed(on)` then re-fetches the picker feed (`view==="studio"`:
  `modelsForUseCase(tab)`, else `listModels()`) so flipping re-lists immediately (A5).

---

## A6 — mocks / samples / blurbs sweep (`mock.ts`, `browser.ts`, `onboarding.rs`, `providers.ts`)

- **`mock.ts`** — replace the `seedance_2_0` MODEL (`:88–103`) with a neutral in-roster model
  (`veo_3_1_fast`, "Veo 3.1 Fast", already referenced in presets) keeping the list length; swap every
  preset `models:` entry (`:216,:232,:240,:255,:278,:319,:334,:364,:394`) from `seedance_2_0` to
  `veo_3_1_fast`, and make each list **lead with `minimax_h3_max_turbo`** where the workhorse fits;
  rename the rate key (`:430`) `seedance_2_0` → `veo_3_1_fast`; the sample job (`:490–496`) →
  `modelId:"minimax_h3_max_turbo"`, `route:"minimax/h3-max-turbo"`, `estimatedUsd:0.13`. The workhorse
  entry (`:57`) already leads the roster — keep it first.
- **Mock id namespace is deliberately distinct** (verifier note): the mock uses `veo_3_1_fast` /
  `minimax_h3_max_turbo` / `kling_3_0`, while the registry uses `veo3_1_fast` / `kling3_0`. These are
  NOT typos to reconcile — keep the mock ids as written (they match existing `mock.ts` fixtures); do
  not "fix" them to the registry spelling during the sweep.
- **A6 grep test** (`mock.ts` sibling vitest, new): import the exported data (`MOCK_MODELS`,
  `MOCK_PRESETS`, `MOCK_JOBS`, the rate map) and assert `JSON.stringify(...)` has **0** matches of
  `/seedance|grok/i`.
- **`browser.ts:547–548`** — the one sample browse entry `bytedance/seedance-2.0/text-to-video` →
  the workhorse (`minimax/h3-max-turbo/text-to-video`, "H3 Max Turbo Text to Video"). Update
  `browser.test.ts:582–587` (which looks it up by that exact id) to the new id — assertion shape
  unchanged (still asserts `acceptedRoles`/`blockReason`), fixture id only.
- **`onboarding.rs`** blurbs: `:43` drop "Seedance" → "Most of the roster on one key — Kling, Wan,
  Veo, FLUX, upscaling."; `:53` → "Cheaper than fal on MiniMax and Seedream."; `:83` (xAI provider
  **row stays** — it is a provider, not a default) reword off Grok → "xAI's image API — direct."
- **`providers.ts`** blurbs: `:44` drop "Seedance"; `:84` reword off "Grok" (row/`envNames` stay).

---

## Collateral Rust test updates (G1 green, no weakened assertion) — Revision 1
Grepped `list_models(` / `models_for_use_case(` / `models_matching(` across `src-tauri/src` and
`crates` (Revision 1): the ONLY callers are `commands.rs` — the two wrappers (`:437,:447`), the pure
fn (`:451`), and **five `#[cfg(test)]` tests** that call `list_models()` with no args. `crates` has
**zero** callers. No prod caller passes a store today, so re-pointing tests at the pure fn is the whole
repair. Each of the five tests exercises catalogue/route *metadata* (not the hide default), so each is
re-pointed at `models_matching(true, None)` — `show_never_used = true` — which is **exactly what
`list_models()` returned before this slice** (unfiltered), keeping every assertion's intent honest and
the full-catalogue `>= 55` claim true. The wrappers now need `State`, which tests cannot cheaply
construct; the pure-fn seam is precisely why these tests bypass the wrapper.

| Test (name · line) | asserts today | mode | exact repair |
|---|---|---|---|
| `the_catalogue_is_exposed_and_launch_models_come_first` · `:2290` | `list_models()`; `len() >= 55`; launch contiguous at top; `first_non_launch >= 12` | `true` (full catalogue) | `let models = models_matching(true, None);` — nothing else changes. Full roster (~57) keeps `>= 55` true; launch count is exactly 12 after A2 (14→12) so `first_non_launch >= 12` still holds. |
| `route_dto_carries_the_promo_as_of` · `:2306` | `list_models()`; workhorse promo route `2026-09-16`; a non-promo route reports `None` | `true` | `let models = models_matching(true, None);` — workhorse is not never-used (present either way); `true` keeps the non-promo search over the full route set. |
| `every_model_offers_at_least_one_route` · `:2337` | every model in `list_models()` has ≥1 route | `true` (must cover ALL models, incl. hidden) | `for m in models_matching(true, None) {` — the "every model" intent requires the unfiltered set. |
| `a_known_model_and_route_quotes_a_positive_price` · `:2355` | `list_models()`; a launch video model quotes `usd > 0` | `true` | `let models = models_matching(true, None);` — workhorse (launch video) is present either way; `true` preserves original set. |
| `routes_report_availability_so_the_ui_can_explain_itself` · `:2519` | `list_models()`; any route has non-empty provider + `':'` in id | `true` | `let models = models_matching(true, None);` |

**NEW default-hidden test (adds the assertion the old `list_models()` never made):**
`list_models_hides_the_never_used_routes_by_default` · `commands.rs`:

```rust
#[test]
fn list_models_hides_the_never_used_routes_by_default() {
    let full  = models_matching(true, None);   // owner opted in
    let shown = models_matching(false, None);  // the pref-OFF default
    // Exact expected count is DERIVED (full − the 11 never-used ids), not a literal.
    let never = full.iter().filter(|m| hickeyfield_core::registry::is_never_used(&m.id)).count();
    assert_eq!(never, 11, "the enumerated never-used set");
    assert_eq!(shown.len(), full.len() - never);
    assert!(shown.iter().all(|m| !hickeyfield_core::registry::is_never_used(&m.id)));
}
```

**MUST NOT sweep:** `a_subtitle_describes_the_model_rather_than_repeating_its_slug` (`:2527`) reads
`reg["seedance_2_0"]` straight from the **unfiltered** `registry()` and asserts its subtitle. Hidden ≠
deleted, so this test stays UNCHANGED and green — do not touch it (nor `most_models_get_a_subtitle`
`:2534`, which iterates the full `registry()` and likewise still sees all 11). The A3 exhaustive test
(`is_never_used_covers_exactly_the_registry_seedance_and_grok_ids`) already pins the count 11 used above.

## Collateral UI test updates (G1 green, no weakened assertion)
Each swaps the fixture model to the workhorse or a neutral in-roster model; no assertion loosened:
- `Stage.test.tsx` (`:20,:69,:79,:91,:109,:129`): `seedance_2_5`/"Seedance 2.5" →
  `minimax_h3_max_turbo`/"H3 Max Turbo" (component prop fixtures; the tests assert the *passed*
  `modelName` renders — same shape).
- `SettingsRail.test.tsx` (`:36,:39–40,:139`): ROUTE `fal:seedance`→`fal:minimax`, MODEL
  `seedance_2_5`→`minimax_h3_max_turbo`/"H3 Max Turbo"; assertion at `:139` follows the new name.
- `Library.test.tsx:21`: `modelId:"seedance_2_5"` → `"minimax_h3_max_turbo"`.
- `presets.test.ts` (`:36,:83`): swap `seedance_2_0` → `veo_3_1_fast` (a model genuinely NOT in
  `catalog[0]`'s variant list), preserving the `supportsModel(...)===false` semantics.
- `browser.test.ts` local query fixtures (`:47,:233–239,:452,:517,:533`) use a bare `"seedance"`
  string only to exercise substring search — rename that local fixture id/title to a neutral term
  (e.g. `"clip"`/"Wan Clip") so the A6 spirit holds; the matcher assertions keep their true/false
  pattern against the renamed tokens.

---

## Test plan — one named test per AC (all hermetic; R-RULES.1 preamble on every command)

Rust tests live in the `#[cfg(test)] mod tests` beside the code; no sockets (predicate + registry are
pure; the submit-refusal test drives `submit_one` to an early `Err` before `route::resolve`, the S18
offline pattern `commands.rs:3068`). Every UI fixture leads with the workhorse.

| AC | test (name · file) | asserts |
|---|---|---|
| A1 | `the_workhorse_constant_is_a_launch_video_route` · `registry.rs` | `registry()[WORKHORSE_MODEL_ID]` is `Modality::Video`, `.launch`, has ≥1 route |
| A1 | `studio opens on the workhorse and keeps it across a tab change` · `App.test.tsx` | initial `modelId===WORKHORSE_MODEL_ID`; after `modelsForUseCase`, still the workhorse when present |
| A1 | `a new storyboard shot defaults to the workhorse` · `Storyboard.test.tsx` | `addShot` on an empty board yields `modelId===WORKHORSE_MODEL_ID`, not `models[0]` |
| A2 | `launch_families_lead_with_the_workhorse_and_ban_seedance_and_grok` · `registry.rs` | `LAUNCH_FAMILIES` flat ids contain `WORKHORSE_MODEL_ID`; **none** starts `seedance`/`grok`; `len()==12` |
| A3 | `is_never_used_covers_exactly_the_registry_seedance_and_grok_ids` · `registry.rs` | enumerate `registry()`: `is_never_used(id)` ⇔ `id∈{the 11}`; asserts the set equals the enumerated 11 (none missed, Seedream/others not over-matched) |
| A3 | `list_models_hides_the_never_used_routes_by_default` · `commands.rs` | pure seam: `models_matching(false, None)` ⇒ no `is_never_used` id and `len()==full−11`; `models_matching(true, None)` ⇒ every never-used id present; non-never counts identical (see §Collateral Rust test updates) |
| A3 | `migration_12_adds_app_pref_and_roundtrips` · `store.rs` | version==12; `get_bool_pref` absent⇒default; set→get roundtrips; idempotent re-open |
| A4 | `history_resolves_a_hidden_models_display_name` · `commands.rs` | `model_names()` includes `("seedance_2_0","Seedance 2.0")` even with pref OFF |
| A4 | `submit_one_refuses_a_hidden_model_while_the_toggle_is_off` · `commands.rs` | `submit_one(store,..,&reg["seedance_2_0"],..)` pref OFF ⇒ `Err` containing "never-used" and "Settings"; no client reached; pref ON ⇒ passes the guard (fails later only at offline route resolve) |
| A5 | `the settings toggle re-lists the picker` · `SettingsRail.test.tsx` | the labelled checkbox renders with the cost warning; `onChange` calls `setShowNeverUsed` then re-fetch; locked selectors present |
| A6 | `mock module exports no seedance or grok` · `mock.test.ts` | `/seedance|grok/i` = 0 over exported `MOCK_MODELS`/`MOCK_PRESETS`/`MOCK_JOBS`/rates; sample job + preset lists lead with the workhorse |
| A7 | (guard, no test of its own) | see below |

Mutation checks for the TEST agent: drop the `is_never_used` clause in `models_matching` ⇒ A3 hide
test fails; flip the default to `true` ⇒ A3 + A4 refusal tests fail; add `seedance_x` to the registry
without touching the predicate ⇒ the exhaustive A3 test fails (prefix still matches, set assertion
catches the new id); remove the workhorse from `LAUNCH_FAMILIES` ⇒ A1/A2 fail.

### A7 no-collateral guard
No `CostModel` value changes (A2 edits only `LAUNCH_FAMILIES` membership; A3 adds a predicate + a KV
table; no priced-route arm touched). No route/price/capability row deleted — `grok_video_v15` and the
whole Seedance line stay priced and probed, so S20's `grok_720p_settles_on_output_seconds`
(`runner.rs`) and the registry regression tests (`registry.rs:3489,:3527,:3546,:3602`) stay green. No
new dependency. The one migration (12) is justified in A3.2. Proof: the existing registry/prices/
store suites plus fmt/clippy (G1).

### G1 gate (exact; R-RULES.1 preamble, never `HICKEYFIELD_ALLOW_SPEND`, never `first_light`, never bare `cargo build`)
Preamble for EVERY command:
`export HICKEYFIELD_NO_KEYCHAIN=1 hickeyfield_FAL_KEY=x hickeyfield_HIGGSFIELD_KEY=x hickeyfield_GOOGLE_KEY=x hickeyfield_OPENAI_KEY=x hickeyfield_XAI_KEY=x hickeyfield_BFL_KEY=x hickeyfield_RECRAFT_KEY=x hickeyfield_VAIG_KEY=x`
1. `cargo test --workspace` — 0 failed, baseline **971 Rust** + new tests.
2. `cargo fmt --all --check`  3. `cargo clippy --workspace --all-targets -- -D warnings`
4. `./scripts/lint-provenance.py`  5. `python3 ./scripts/lint-spend.py`
6. Keyless audit: `env -u hickeyfield_FAL_KEY -u FAL_KEY cargo run -p hickeyfield-core --example audit_fal`.
7. UI: `cd ui && pnpm test && pnpm build` — baseline **313 UI** + new tests.

G2 spend hygiene: every command sets the dummy keys + `HICKEYFIELD_NO_KEYCHAIN=1`; no test opens a
socket; `HICKEYFIELD_ALLOW_SPEND` never set; `first_light` never run; the refusal test asserts the
NO-spend path (early `Err`).

---

## Documentation (DOCUMENT phase; this invocation writes only design.md)
- `README`: add a short "Never-used routes" note (Seedance and Grok hidden by default; the Settings
  toggle to reveal them; they still bill the provider).
- `CHANGELOG.md` `[Unreleased] > Added`: workhorse-first defaults (Studio, tab fallback, storyboard
  shot); a "Show never-used routes (Seedance, Grok)" setting, OFF by default; hidden routes still
  render in history and are refused server-side while hidden.
- `CHANGELOG.md > Changed`: `LAUNCH_FAMILIES` leads with the workhorse; mocks/samples/blurbs no
  longer advertise Seedance or Grok.
- `document.md`: file+line for each change (registry.rs const/predicate/launch row, store.rs
  migration 12, commands.rs pure `models_matching(bool, use_case)` seam + `State`-reading wrappers +
  refusal + `model_names` + toggle commands + five re-pointed Rust tests, lib.rs `generate_handler!`
  (three new command lines), App.tsx/`ui/src/components/Storyboard.tsx`
  defaults + name resolution, SettingsRail.tsx toggle, api.ts, types.ts constant, the mock/blurb sweep).

## Non-goals (recorded, not skipped)
- **Deleting any Seedance/Grok route, price, or capability row — N/A (owner: hide, not delete).**
- **Higgsfield preset glob tables in `preset.rs` — N/A (out of scope: vendor data, not defaults).**
- **Corpus / historical-doc mentions of the names — N/A** (out of scope: comments/docs that merely
  mention; the planner cannot pick a hidden model because `modelPrefs` is the filtered list).
- **`CostModel` value changes / new dependency — none** (A7).
- **A client-controlled toggle — rejected**: the setting is server-persisted so the refusal is
  server-authoritative (A4).

## Open questions (each with a default this build assumes)
- **O1 — predicate: prefix vs explicit const list?** DEFAULT: **prefix** (`starts_with "seedance"|
  "grok"`), so a future id is auto-hidden; the exhaustive test pins it to exactly the 11 today.
  Reversible to an explicit `NEVER_USED: &[&str]` if the owner ever ships a `grok_*` route they DO use.
- **O2 — toggle home: model-settings rail vs a dedicated Settings screen?** DEFAULT: the model
  **settings rail** (`SettingsRail.tsx`), adjacent to the picker so "flipping re-lists" is immediate;
  an additive `.never-used-toggle` touches no locked selector.
- **O3 — mock replacement model for `seedance_2_0`?** DEFAULT: **`veo_3_1_fast`** (already in the mock
  presets), keeping the roster length and a distinct non-workhorse video model.
- **O4 — refusal wording?** DEFAULT: the text in A4 (names Seedance/Grok, the billing reason, and the
  exact toggle label). One-line change if the owner prefers shorter copy.

## Riskiest / weakest (verifier: start here)
1. **The migration-vs-reuse call (A3.2).** The strongest challenge is A7's "no migration unless
   genuinely needed." My justification hinges on the claim that `submit_one` has `store` but no
   `AppHandle`, and that a client-passed bool would be a UI-only guard (A4-forbidden). Verify both:
   grep `submit_one`'s callers for any `AppHandle`, and confirm the config-file mechanism (`app.rs:95`)
   truly needs `AppHandle`. If a cheaper server-readable home exists, the migration should be dropped.
2. **Exhaustiveness of the 11-id enumeration.** If the registry contains a never-used id I missed
   (e.g. a picker-only spec or a fallback-priced id not grepped), the prefix predicate still hides it
   but the *set-equality* assertion in the A3 exhaustive test would flag it — confirm the test asserts
   equality with the enumerated set, not mere non-emptiness, so a miss fails loudly.
3. **`is_never_used` over/under-match at the prefix boundary.** `seedream*` (image, KEPT) must NOT
   match `seedance`, and no kept id may start `grok`. The exhaustive test covers it; verify Seedream
   and any `grok`-adjacent id are explicitly present on the "kept" side of the assertion.
4. **The `models_matching` seam + its Rust callers/tests (was the C11/C12 FAIL; now specified).**
   Revision 1 makes `models_matching` a pure `fn(show_never_used: bool, use_case)` and reads the store
   only in the `State` wrappers. The grep (`src-tauri/src` + `crates`) found the only affected callers
   are the five `list_models()` tests, each re-pointed at `models_matching(true, None)` in §Collateral
   Rust test updates, plus one NEW default-hidden test. Verifier: confirm the grep still shows no other
   caller, that `show_never_used = true` reproduces the pre-slice `list_models()` set (so `>= 55` and
   `first_non_launch >= 12` stay honest), and that the `>= 55` intent is preserved (not weakened to a
   smaller number).
5. **A4 history for storyboard shots.** `ui/src/components/Storyboard.tsx:133` `modelName` also reads the filtered list;
   I route it through the same `modelNames` map. Verify no storyboard code path re-points a shot's
   `modelId` when its model is hidden (it must display, never silently swap).
