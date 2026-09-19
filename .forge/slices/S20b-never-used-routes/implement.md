# S20b-never-used-routes — IMPLEMENT (claim)

Built to design.md Revision 1 (sha256 237154c5…). Demote + hide: Seedance and Grok
are hidden from every picker behind a persisted, default-OFF toggle; nothing deleted.
Workhorse `minimax_h3_max_turbo` leads every default, fallback, mock and sample.

Money safety: every command below ran with the R-RULES preamble
(`HICKEYFIELD_NO_KEYCHAIN=1` + dummy `hickeyfield_*_KEY=x` SET; `HICKEYFIELD_ALLOW_SPEND`
never set; `first_light` never run; no sockets in any new test). The keyless audit is the
sole command that unsets the fal key.

## Files changed (one line why)

Rust core (`crates/hickeyfield-core/`):
- `src/registry.rs` — added `pub const WORKHORSE_MODEL_ID = "minimax_h3_max_turbo"` and
  `pub fn is_never_used(id)` (prefix `seedance`/`grok`); replaced the 3-id "Seedance 2.0"
  `LAUNCH_FAMILIES` row with `("MiniMax H3 Max Turbo", &[WORKHORSE_MODEL_ID])` (array stays
  `[_;12]`); added 3 tests (A1/A2/A3).
- `src/onboarding.rs` — 3 blurbs reworded off Seedance/Grok (xAI provider row + envNames
  kept; it is a provider, not a default).

Tauri (`src-tauri/`):
- `src/store.rs` — migration **12** (`app_pref` KV, append-only, STRICT); `SHOW_NEVER_USED`
  key const; `get_bool_pref`/`set_bool_pref`; bumped 3 existing `user_version` assertions
  11→12; added `migration_12_adds_app_pref_and_roundtrips`.
- `src/commands.rs` — `models_matching` is now a pure `fn(show_never_used: bool, use_case)`
  with the hide clause before the use-case clause; `list_models`/`models_for_use_case`
  wrappers take `State<'_, AppState>` and read the pref; new `ModelNameDto`, `model_names`
  (unfiltered), `get_show_never_used`, `set_show_never_used`; server refusal at the top of
  `submit_one` (before route/estimate/client); re-pointed 5 catalogue tests at
  `models_matching(true, None)`; added 3 tests (A3 hide, A4 history, A4 refusal).
- `src/lib.rs` — registered `model_names`, `get_show_never_used`, `set_show_never_used` in
  `generate_handler!`.

UI (`ui/`):
- `src/types.ts` — `export const WORKHORSE_MODEL_ID`.
- `src/api.ts` — `modelNames()` (folds to `{id:name}`; mock folds `MOCK_MODELS`),
  `getShowNeverUsed()`/`setShowNeverUsed()` (mock ⇒ localStorage).
- `src/App.tsx` — startup fetches `modelNames()` + `getShowNeverUsed()`; initial model and
  tab fallback prefer `WORKHORSE_MODEL_ID` by name; `featuredModelName` resolves through the
  unfiltered map first; `onToggleNeverUsed` persists then re-lists; passes
  `showNeverUsed`/`onToggleNeverUsed` to SettingsRail and `modelNames` to Storyboard.
- `src/components/SettingsRail.tsx` — additive `.never-used-toggle` label + `.never-used-warn`
  above `.rail-disclaimer` (no test-locked selector touched); two new props.
- `src/components/Storyboard.tsx` — `modelName` resolves through the `modelNames` map first;
  new-shot default is workhorse-first; new optional `modelNames` prop.
- `src/mock.ts` — replaced the `seedance_2_0` MODEL with `wan_2_6` (neutral, non-duplicate,
  keeps roster length); every preset `models:` list leads with `minimax_h3_max_turbo` and
  drops seedance; rate key `seedance_2_0`→`wan_2_6`; sample job → workhorse
  (`minimax/h3-max-turbo`, $0.13). `/seedance|grok/i` = 0 in the module.
- `src/lib/browser.ts` — the one sample browse entry → `minimax/h3-max-turbo/text-to-video`.
- `src/lib/providers.ts` — fal + xAI blurbs reworded off Seedance/Grok (rows + envNames kept).

UI test collateral (fixtures swapped, no assertion weakened):
- `src/mock.test.ts` (NEW, A6), `src/App.test.tsx` (A1), `src/components/Storyboard.test.tsx`
  (A1 ×2), `src/components/SettingsRail.test.tsx` (A5 + props + fixture),
  `src/components/Stage.test.tsx`, `src/components/Library.test.tsx`,
  `src/lib/__tests__/presets.test.ts`, `src/lib/__tests__/browser.test.ts` (local "seedance"
  fixture renamed to a neutral "clip"/"Wan Clip"; the MOCK_BROWSE lookup id updated).

MUST-NOT-sweep confirmed untouched: `a_subtitle_describes_the_model_rather_than_repeating_its_slug`
and `most_models_get_a_subtitle` (both read the unfiltered `registry()` and still see all 11).

## AC → named test

- **A1** `registry::tests::the_workhorse_constant_is_a_launch_video_route`;
  `App.test.tsx` › "A1: Studio opens on the workhorse and keeps it across a tab change";
  `Storyboard.test.tsx` › "A1: the planner is offered the workhorse first, never a never-used
  route" + "A1: a new shot inherits the workhorse when it leads the empty-board default".
- **A2** `registry::tests::launch_families_lead_with_the_workhorse_and_ban_seedance_and_grok`.
- **A3** `registry::tests::is_never_used_covers_exactly_the_registry_seedance_and_grok_ids`
  (set-equality with the 11); `commands::tests::list_models_hides_the_never_used_routes_by_default`
  (derived `full − 11`); `store::tests::migration_12_adds_app_pref_and_roundtrips`.
- **A4** `commands::tests::history_resolves_a_hidden_models_display_name`;
  `commands::tests::submit_one_refuses_a_hidden_model_while_the_toggle_is_off` (OFF ⇒ `Err`
  with "never-used"+"Settings", 0 clients reached; ON ⇒ guard passes, reaches the S18 fake).
- **A5** `SettingsRail.test.tsx` › "A5: the never-used toggle renders with its cost warning
  and re-lists on change" (locked `.generate-button`/`.model-item` still present).
- **A6** `mock.test.ts` › 4 tests (`/seedance|grok/i` = 0 over MOCK_MODELS/PRESETS/JOBS;
  roster, every preset variant list, and the sample job lead with the workhorse).

## Re-run commands

Rust (from worktree root, R-RULES preamble):
- `cargo test --workspace`
- `cargo test -p hickeyfield-core registry::tests::is_never_used_covers_exactly_the_registry_seedance_and_grok_ids -- --exact`
- `cargo test -p hickeyfield-tauri commands::tests::submit_one_refuses_a_hidden_model_while_the_toggle_is_off -- --exact`
- `cargo test -p hickeyfield-tauri store::tests::migration_12_adds_app_pref_and_roundtrips -- --exact`

UI (`cd ui`): `pnpm test` · `pnpm build`
Single files: `pnpm test src/mock.test.ts` · `pnpm test src/App.test.tsx`

## Gate results (real tails)

1. `cargo test --workspace` → **passed=978 failed=0 ignored=20** (baseline 971 + 20 ignored;
   +7 new: 3 registry, 1 store, 3 commands). Added NO `#[ignore]`.
2. `cargo fmt --all --check` → clean (FMT CLEAN; `cargo fmt --all` was applied first).
3. `cargo clippy --workspace --all-targets -- -D warnings` → `Finished dev profile` (0 warnings).
4. `./scripts/lint-provenance.py` → `PASS no third-party media hosts`; `PASS no shipped string
   matches the reference corpus (80015 shingles)`; exit 0.
5. `python3 ./scripts/lint-spend.py` → `lint-spend: ok (49 file(s) scanned)`; exit 0.
6. `env -u hickeyfield_FAL_KEY -u FAL_KEY cargo run -p hickeyfield-core --example audit_fal` →
   `70 endpoint(s) audited against fal's own schema`; exit 0.
7. `cd ui && pnpm test` → **Test Files 24 passed; Tests 321 passed** (baseline 313 + 8 new:
   A6 ×4, A1 App ×1, A1 Storyboard ×2, A5 ×1).
8. `cd ui && pnpm build` → `✓ built in 867ms`; exit 0 (`tsc --noEmit` clean).

## Where a design number differed from a real run (the run wins)

- **Roster size.** design.md says the `>= 55` test sees "~57"; the real `list_models()` roster
  is **87** (`registry().len()` = `57 − 2 EXCLUSIONS + 32`). `models_matching(true, None)`
  reproduces exactly that 87; `>= 55` holds with a wide margin. Kept `>= 55` and
  `first_non_launch >= 12` AS-IS (not tightened, not loosened) per the verifier note.
- **Launch-id count.** design.md says "14→12"; the real flat launch-id count is **16→14** after
  the A2 swap (Seedance's 3 ids replaced by the 1 workhorse id, a drop of 2). `first_non_launch
  >= 12` holds (14 ≥ 12); `there_are_twelve_launch_families` (12→12) and
  `only_launch_models_carry_the_marker` (self-referential sum) stayed green with no edit.
- **`user_version` assertions.** design.md names one migration/idempotency test; the real tree
  has **three** `user_version == 11` assertions (`store.rs` ~:925, ~:1092, ~:1179). All three
  were bumped to 12 — required, since migration 12 now runs in every `in_memory()`.
- **Mock replacement model.** O3's default `veo_3_1_fast` was already present in `MOCK_MODELS`
  (:118), so reusing it for the removed `seedance_2_0` MODEL block would duplicate an id.
  Used **`wan_2_6`** instead (neutral, in the registry launch set, not previously in the mock),
  keeping the roster length and a distinct non-workhorse video model. Preset `models:` lists
  still swap seedance→`veo_3_1_fast` and lead with the workhorse; the A6 grep enforces 0 hits.

## Deviations from design (proposed, not silent)

- **Storyboard A1 test shape.** The design's "addShot on an empty board ⇒ workhorse" is not
  reachable through the DOM: the `+ Add shot` button lives inside the `shots.length > 0` block
  (`Storyboard.tsx:452`), so an empty board renders no add control. The workhorse-first
  new-shot default expression is present and correct, but its no-prior-shot branch is a
  defensive default. I covered A1-storyboard with two reachable tests instead: (a) the planner
  is handed `modelPrefs` led by the workhorse and containing no `seedance_2_0`, and (b) adding
  a shot after a workhorse-led draft keeps the workhorse. This is a test-shape change, not a
  behaviour change; the code matches the design.

## Known gaps / limitations (not fixed)

- The thin `State` wrappers (`list_models`/`models_for_use_case`/`get_show_never_used`/
  `set_show_never_used`) have no direct in-crate unit test — Tauri injects `State`, so a direct
  test would have to construct `AppState`. Their behaviour is a two-line "read pref → delegate";
  the pure fn (`list_models_hides_the_never_used_routes_by_default`, both modes), the pref
  plumbing (`migration_12_adds_app_pref_and_roundtrips`), and the spend boundary reading the
  same pref (`submit_one_refuses_a_hidden_model_while_the_toggle_is_off`) are all covered. The
  design-verify ruling marked this acceptable/optional.
- Comments/historical mentions of the names remain out of scope (e.g. `MediaInputs.tsx:15`
  mentions Seedance in a comment; `App.tsx`/`SettingsRail.tsx` name Seedance/Grok in the toggle
  label and code comments, which is intended). `GROK_API_KEY` envName kept (provider, not a
  default). `preset.rs` Higgsfield glob tables untouched (out of scope).
- No live/paid path was exercised (agents never spend). The A4 refusal test proves the NO-spend
  early `Err`; the ON path reaches the S18 offline fake, not a real provider.

## Repair round 1 (post-gate non-blocking fixes)

All three verifiers PASSED; these are the non-blocking follow-ups, fixed minimally.
Every command below ran with the R-RULES preamble (`HICKEYFIELD_NO_KEYCHAIN=1` +
dummy `hickeyfield_*_KEY=x` SET; `HICKEYFIELD_ALLOW_SPEND` never set; `first_light`
never run). No `CostModel` change, no new dependency, no new migration, no private
names. Worktree only.

### Fix 1 — real v11→v12 upgrade-with-data proof (TEST non-blocking #2)
`src-tauri/src/store.rs` — added `an_existing_v11_database_with_data_upgrades_to_v12_intact`
(new test, immediately after `migration_12_adds_app_pref_and_roundtrips`, ~store.rs:1288).

How the v11 DB was constructed (documented in a test comment): migration 12 adds
ONLY the `app_pref` table, so a genuine v11 file is a fully-migrated file with
`app_pref` dropped and `user_version` rolled back to 11. The test opens a
**file-backed** store under `std::env::temp_dir()` (no tempfile dep — matches the
existing `library.rs`/`chain.rs` temp-dir convention), writes a `job_sets` row with
non-default values incl. `estimated_usd=Some(1.23)`/`actual_usd=Some(1.19)` and a
`storyboards` row (`chained`, `overlap_seconds`, `auto_retry` all non-default), then
`DROP TABLE app_pref; PRAGMA user_version = 11;` and asserts the file is really at 11
before the upgrade. Temp dir only — never `~/Library/Application Support/`; the dir is
removed at the end.

Re-opening through the normal constructor (`SqliteStore::open`) asserts: `user_version
== 12`; `app_pref` table exists (`sqlite_master` count == 1); every pre-existing row/
column value byte-for-byte unchanged (`assert_eq!` on the whole `JobSet` and whole
`Storyboard`, which derive `PartialEq`); `get_bool_pref(SHOW_NEVER_USED, false) ==
false`; a set→get round-trip (both polarities) works; and a THIRD open is idempotent
(still 12, both rows still intact, pref still default).

Run:
`cargo test -p hickeyfield-tauri store::tests::an_existing_v11_database_with_data_upgrades_to_v12_intact -- --exact`
→ `test result: ok. 1 passed; 0 failed; 0 ignored`.

### Fix 2 — make the A1 cross-tab test real (REVIEW non-blocking #1)
`ui/src/App.test.tsx` — the old test collected tab buttons then `void tab` (a no-op,
because in plain browse mode `listUseCases()` returns `[]` so no tabs render and
`modelsForUseCase()` serves the same full roster for every slug). Added a **test-only**
`vi.mock("./api", …)` that spreads the real mock-mode api and overrides just two
functions: `listUseCases` (three real tabs: text-to-video, image-to-video,
text-to-image) and `modelsForUseCase` (the two video slugs carry the workhorse, the
image slug does not — filtered from the real `MOCK_MODELS` by modality). The test now:
opens on the workhorse; clicks `[role="tab"][data-tab="image-to-video"]` (video list
still has the workhorse) and asserts the selection is STILL `H3 Max Turbo`; clicks
`[data-tab="text-to-image"]` (no workhorse) and asserts the selection falls to that
list's first LAUNCH model, `Nano Banana Pro` — NOT the workhorse and not a never-used
id. Selection is read off the composer's `.model-item-composer .model-item-name`
summary row (always the currently selected model).

No real App.tsx bug exposed: `App.tsx:349-357` fallback (`workhorse ?? first launch ??
m[0]`) behaves exactly as designed — workhorse survives when present, otherwise the
first launch model is chosen. Fix 2 exposed NO production bug.

Run: `pnpm test src/App.test.tsx` → `Test Files 1 passed; Tests 3 passed`.

### Fix 3 — mock.ts uses WORKHORSE_MODEL_ID (REVIEW non-blocking #4)
`ui/src/mock.ts` — replaced every raw `"minimax_h3_max_turbo"` literal with the TS
`WORKHORSE_MODEL_ID` constant: the roster `id` (mock.ts:58), all 15 preset `models:`
lists, the `RATE_PER_SECOND` key (`[WORKHORSE_MODEL_ID]: 0.02`, computed key), and the
sample job `modelId` (mock.ts:491). Imported it from `./types` (folded into the
existing `./types` import as a value alongside the `type` imports — no duplicate-import
lint, and no cycle: `types.ts` imports nothing from `mock.ts`). The `mock.test.ts`
drift guard (`MOCK_MODELS[0].id === WORKHORSE_MODEL_ID`) still holds.

### Full gate (real tails)
- `cargo test --workspace` → `773 + 6 + 2 + 198 = 979 passed; 0 failed; 20 ignored`
  (was 978; +1 new store test).
- `cargo fmt --all --check` → clean (`FMT_OK`).
- `cargo clippy --workspace --all-targets -- -D warnings` → `Finished dev profile`, 0 warnings.
- `./scripts/lint-provenance.py` → `PASS no third-party media hosts` / `PASS no shipped
  string … (80015 shingles)`.
- `python3 ./scripts/lint-spend.py` → `lint-spend: ok (49 file(s) scanned).`
- `env -u hickeyfield_FAL_KEY -u FAL_KEY cargo run -p hickeyfield-core --example audit_fal`
  → `70 endpoint(s) audited against fal's own schema`.
- `cd ui && pnpm test` → `Test Files 24 passed; Tests 321 passed`.
- `cd ui && pnpm build` → `✓ built in 840ms` (tsc clean).

### Not done / left as-is (per instructions, accepted)
- `Storyboard.tsx:216-219` unreachable defensive branch — left as-is (accepted).
- `get_bool_pref` non-`"1"` → false — left as-is (safe direction, accepted).
- Resume-drain not re-checking the toggle — left as-is (no new spend, accepted).

## Repair round 2 (A1 cross-tab test made falsifiable)

TEST re-verification (repair round 1) FAILED on ONE thing: the A1 cross-tab vitest was a
fixture-ordering tautology — `m[0]` already equalled the expected model in every asserted
state, so stripping ALL workhorse-preference from `App.tsx` (both `:206-209` and `:351-354`)
left it GREEN. Production code (`App.tsx`) is correct and was NOT changed (proven byte-identical
below). This round only rebuilds the TEST so it can go RED.

### What changed (files)
- **NEW `ui/src/App.workhorse.test.tsx`** — the A1 test now lives in its own file with its own
  scoped `vi.mock("./api")`. The mock overrides `listModels` (initial roster) AND
  `modelsForUseCase` (per-tab lists), hand-ordered so the workhorse is NEVER at index 0 and
  index 0 is never the expected answer. One test, four asserted states.
- **`ui/src/App.test.tsx`** — removed the old tautological A1 `it(...)` and its dedicated
  `vi.mock`; the header comment now points to `App.workhorse.test.tsx`. The two S12 tests
  (view switch, delete-confirmation button-in-button) remain and pass against plain browse-mode
  api. Unused `vi` import dropped.
- No production file changed. `App.tsx`, `types.ts`, `mock.ts` untouched.

### Fixture ordering (App.workhorse.test.tsx), workhorse never at [0]
- `listModels()` initial roster: `[gemini_omni (video, isLaunch:false), kling_3_0 (video, launch),
  minimax_h3_max_turbo (workhorse), wan_2_6]`. m[0] is non-launch; first LAUNCH video is kling.
  Only `m.find(WORKHORSE)` yields H3 Max Turbo.
- `text-to-video` (initial tab) → `videoA [gemini_omni, kling_3_0, WORKHORSE, wan_2_6]` — contains
  the workhorse (idx 2); the initial workhorse selection SURVIVES.
- `text-to-image` → `imageList [recraft_v4_1 (image, isLaunch:false), nano_banana_pro (launch),
  seedream_5_pro]` — no workhorse; m[0] is NON-launch, so the fallback answer (first LAUNCH =
  nano_banana_pro) differs from m[0]. Test also asserts the landing model `isLaunch===true` and
  `id==="nano_banana_pro"`.
- `image-to-video` → `videoB [wan_2_6 (launch), kling_3_0 (launch), WORKHORSE]` — contains the
  workhorse (idx 2, and NOT first isLaunch — wan_2_6 precedes it) and does NOT contain the
  currently selected image model, so the SURVIVAL branch fails and the FALLBACK branch runs;
  only `m.find(WORKHORSE)` yields H3 Max Turbo.
- Survival step: switch text-to-video ↔ image-to-video (both list the workhorse, neither at [0]).

"Never a never-used id" is a server-side guarantee; per round-2 guidance it is NOT faked in the
UI mock. A comment in the new file points to the Rust owners:
`list_models_hides_the_never_used_routes_by_default` + the A3 set-equality test
`is_never_used_covers_exactly_the_registry_seedance_and_grok_ids`.

### Mutation proof (COPIES only; `cp $SCRATCH/App.tsx.orig` between each; A1 file = App.workhorse.test.tsx)
- **M1 — initial pick → `const first = m[0]`:** RED.
  `AssertionError: expected 'Gemini Omni' to contain 'H3 Max Turbo'` (App.workhorse.test.tsx:165).
- **M2 — tab-change fallback → `const next = m[0]`:** RED.
  `AssertionError: expected 'Recraft v4.1' to contain 'Nano Banana Pro'` (App.workhorse.test.tsx:174).
- **M3 — both sites → first `isLaunch` (workhorse line dropped):** RED.
  `AssertionError: expected 'Kling 3.0' to contain 'H3 Max Turbo'` (App.workhorse.test.tsx:165)
  (initial catches it first; the video-into step would also catch the fallback half via wan_2_6).

### Restore proof (byte-identical)
After the three mutations: `cp $SCRATCH/App.tsx.orig ui/src/App.tsx`;
`cmp ui/src/App.tsx $SCRATCH/App.tsx.orig` → identical; the `git diff -- ui/src/App.tsx` hunk is
unchanged vs the captured `pre2.diff`. Production `App.tsx` is exactly as it was.

### Gate (all green)
- `cd ui && pnpm test` → `Test Files 25 passed; Tests 321 passed` (was 24 files/321; one A1 test
  moved out, one robust A1 test moved in — count held at 321).
- `cd ui && pnpm build` → `✓ built in 851ms` (tsc clean).
- `cargo test --workspace` → summed `passed=979 failed=0 ignored=20`.
- `cargo fmt --all --check` → exit 0.
- `cargo clippy --workspace --all-targets -- -D warnings` → `Finished dev profile`, 0 warnings.
- `./scripts/lint-provenance.py` → PASS / PASS.
- `python3 ./scripts/lint-spend.py` → `lint-spend: ok (49 file(s) scanned).`
