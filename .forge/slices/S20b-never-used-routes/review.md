# S20b-never-used-routes — REVIEW (evidence)

Read-only production-standards review of the slice diff (`git diff` + untracked
`ui/src/mock.test.ts`). No source changed. Scope: the 21 tracked files + one new test.

## Method
Read slice.md / design.md / design-verify.md / implement.md, then the full diff of every
touched file. Traced the spend funnel (`submit_one` guard placement vs `route::resolve` /
`submit_to_provider`), the pref fail-open/closed direction, migration 12, the predicate/prefix
boundary, and every re-pointed test for weakened assertions. Did NOT run builds/tests (a TEST
agent owns that; instructed). Findings rest on the stable diff.

## What I checked and found clean
- **registry.rs** — `WORKHORSE_MODEL_ID` and `is_never_used` added with WHY-comments (owner rule).
  Prefix predicate boundary is correct: `seedream_*` diverges at char 4 so it is NOT over-matched;
  the A3 test (`is_never_used_covers_exactly_...`) is true **set-equality** against the enumerated 11
  (not non-emptiness) and explicitly pins Seedream + workhorse on the KEPT side. `LAUNCH_FAMILIES`
  Seedance-row → single workhorse row keeps `[_;12]`, so `there_are_twelve_launch_families` and the
  self-referential `only_launch_models_carry_the_marker` stay green with no edit. A2 test bans any
  `seedance`/`grok` prefix in the launch set.
- **commands.rs (critical path)** — `submit_one` guard sits at the top of the fn (`:1155`), BEFORE
  `route::resolve` (`:1196`) and `submit_to_provider` (`:1259`). It is **fail-CLOSED**:
  `get_bool_pref` returns `default=false` on an absent row and on any read error
  (`.ok()...unwrap_or(false)`), so a pref failure refuses (never silently spends). Error message is
  plain, names the toggle + the billing reason, leaks no internals. Grep of all spend entries
  (standalone `:1080`, storyboard `:1906/:1978`, keyframe `:2146`, chain `293/643/793`) confirms
  **every** path funnels through `submit_one` — **no bypass**. `model_names` is unfiltered so history
  renders hidden ids by name; `estimate_with`/`model_capabilities` remain unfiltered for cost. The
  five re-pointed catalogue tests use `models_matching(true, None)` = pre-slice unfiltered set, so
  `>= 55` / `first_non_launch >= 12` intents are preserved, not weakened; the new default-hide test
  derives the count (`full − 11`) rather than hard-coding it. `a_subtitle_...`/`most_models_...`
  (unfiltered registry readers) correctly left untouched.
- **store.rs** — migration 12 is append-only, `CREATE TABLE IF NOT EXISTS ... STRICT` +
  `PRAGMA user_version = 12`, matching the file's pattern; guarded by `if version < 12` so it is
  idempotent (the roundtrip test re-runs `migrate()` and the stored value survives). All three
  `user_version == 11` assertions bumped to 12 coherently. SQL is parameterised (`?1/?2`, upsert via
  `ON CONFLICT`). `SHOW_NEVER_USED` default false.
- **UI** — `App.tsx` initial = workhorse → launch video → `m[0]`; tab fallback = workhorse → launch →
  `m[0]` (no never-used fallback, empty-list safe). `featuredModelName` resolves the unfiltered
  `modelNameMap` first. `onToggleNeverUsed` persists server-side then re-lists the correct feed.
  `SettingsRail` toggle is a real `<input type="checkbox">` inside a `<label>` (keyboard-operable,
  labelled), with the cost warning, additive above `.rail-disclaimer` — no test-locked selector
  touched. `api.ts` mock-mode parity: `modelNames` folds `MOCK_MODELS`, get/set fall back to
  `localStorage` — no crash off-desktop. DTO casing (`display_name` → folded to `{id:name}`) correct.
- **Sweep** — `grep -i "seedance|grok"` over mock.ts / browser.ts / providers.ts / onboarding.rs = 0
  hits (only the intentional `GROK_API_KEY` envName kept — a provider, not a default). `mock.test.ts`
  greps the **exported** data (`MOCK_MODELS/PRESETS/JOBS`), not source text, and asserts workhorse-lead.
  Every preset seed with a `models:` list leads with the workhorse; seeds without one are skipped by
  the test. Re-pointed UI fixtures (Stage/Library/SettingsRail/presets/browser) keep their assertion
  shapes; `presets.test.ts` swap keeps `supportsModel(...)===false` semantics (swapped id absent from
  `catalog[0]`).
- **A7** — no `CostModel` value change, no route/price/capability row deleted, no new dependency; only
  migration 12 added, justified (the spend boundary has `store` but no `AppHandle`).

## Blocking
None.

## Non-blocking
- `ui/src/App.test.tsx:~100–114` — the A1 test titled "…and keeps it across a tab change" collects the
  tab buttons into `tab`, then does `void tab` and never clicks; the second `expect(name())` re-checks
  the same rendered value after a no-op `flush()`. The cross-tab claim is not actually exercised (the
  initial-selection half is real). Fix: click a tab and await the re-list, or rename to reflect it only
  asserts the initial selection.
- `ui/src/components/Storyboard.tsx:216–219` + `Storyboard.test.tsx` — the new-shot default's
  workhorse-first branch (`models.some(workhorse) ? WORKHORSE : models[0]`) is only reached when
  `last` is undefined, but implement.md notes the "+ Add shot" control renders only when
  `shots.length > 0`, so `last?.modelId` always short-circuits it: the branch is defensive/unreachable
  via DOM and has no direct test (the substitute test proves inheritance, not this branch). Acceptable
  as a defensive default, but the test name overpromises. Fix: unit-cover the default expression or
  rename the test.
- `src-tauri/src/store.rs:437–445` — `get_bool_pref` maps `value == "1"` to true and everything else
  (including unexpected/corrupt values) to false, ignoring `default` for non-`"0"/"1"` rows. Only
  `"0"/"1"` are ever written and false is the safe direction for this toggle, so risk is low; consider
  honoring `default` on an unparseable value for general reuse.
- `ui/src/mock.ts` — the workhorse id appears as the raw literal `"minimax_h3_max_turbo"` throughout
  the mock/preset/job fixtures rather than importing `WORKHORSE_MODEL_ID`. Drift is guarded by
  `mock.test.ts` (asserts `MOCK_MODELS[0].id === WORKHORSE_MODEL_ID`), so this is minor; the one-named-
  constant rule is satisfied in prod code (types.ts / registry.rs).

## Rulings
- **Fail-open vs fail-closed:** fail-CLOSED, correct. Both the picker filter and the `submit_one`
  refusal read `get_bool_pref(..., false)`, which returns `false` (hidden/refuse) on an absent row or
  any read error — a pref failure can never open a spend path.
- **Submit bypasses:** none. All five spend entry points (standalone, storyboard, keyframe, chain
  x3) route through `submit_one`, whose guard precedes route resolution and any provider client.

VERDICT: PASS — no blocking or major findings; critical path (server-authoritative refusal,
fail-closed pref, migration 12, workhorse defaults, name/cost history for hidden models) is correct
and covered by real tests. Four minor non-blocking follow-ups noted above.

## Repair round 1 — re-review

Read-only re-review of the three repaired files against `implement.md` "## Repair round 1".
No source changed. TEST agent's concurrent mutations on `store.rs`/`App.tsx` ignored; findings
rest on the stable diff. Did not run builds/tests.

### Addressed findings — closed / not closed
- **REVIEW non-blocking #1 (App.test.tsx cross-tab A1 was a no-op)** — CLOSED.
  `ui/src/App.test.tsx:127-169` now actually clicks tabs. `clickTab("image-to-video")` (a video
  slug whose mock list still carries the workhorse) asserts the selection stays `H3 Max Turbo`
  (`:157`); `clickTab("text-to-image")` (image list, no workhorse) asserts it falls to the first
  LAUNCH model `Nano Banana Pro` and is NOT the workhorse (`:165-168`). The cross-tab claim in the
  title is now exercised, including the money-relevant "never snaps to a never-used id" direction.
- **REVIEW non-blocking #4 (mock.ts raw workhorse literals)** — CLOSED. `ui/src/mock.ts:11-12`
  imports `WORKHORSE_MODEL_ID` from `./types`; the roster id (`:58`), all 15 preset `models:`
  heads, the `RATE_PER_SECOND` computed key (`:429` `[WORKHORSE_MODEL_ID]: 0.02`) and the sample
  job `modelId` (`:491`) all use the constant. `git grep` shows no remaining raw
  `"minimax_h3_max_turbo"` literal in `mock.ts`. `types.ts` has zero imports (confirmed
  `grep -c '^import'` = 0), so no `types.ts ← mock.ts` cycle; the computed key is valid TS.
- **TEST non-blocking #2 (no real v11→v12 upgrade-with-data proof)** — CLOSED.
  `src-tauri/src/store.rs:1288-1394` adds `an_existing_v11_database_with_data_upgrades_to_v12_intact`.
- **Storyboard.tsx:216-219 unreachable branch / get_bool_pref non-"1"→false / resume-drain toggle**
  — NOT CLOSED, explicitly left as-is per instructions (accepted; all safe-direction/defensive).

### New code review
- **Upgrade test honesty** — HONEST. Migration 12 (`store.rs:415-432`) is verified append-only: its
  ONLY effect is `CREATE TABLE IF NOT EXISTS app_pref (...) STRICT` + `PRAGMA user_version = 12`.
  So building a v11 file by `DROP TABLE app_pref; PRAGMA user_version = 11;` (`:1333`) is a faithful
  v11, and the pre-upgrade `assert_eq!(v, 11, ...)` (`:1338`) pins that the upgrade genuinely starts
  from 11. Re-open asserts `user_version == 12`, `app_pref` present (`sqlite_master` count == 1),
  default pref false, and — the key part — FULL-ROW equality on the whole `JobSet` and whole
  `Storyboard` via `assert_eq!` (`:1369-1370`, `:1388-1389`), not counts, so preserved
  `estimated_usd=1.23`/`actual_usd=1.19` and the non-default storyboard flags are all covered. A
  set→get round-trip (both polarities, `:1373-1376`) and an idempotent THIRD open (`:1380-1390`) are
  included.
- **Temp-file hygiene** — ACCEPTABLE, not a flaky-test finding. Path is
  `temp_dir()/hf-v11-upgrade-{process::id()}/jobs.sqlite` (`:1301-1303`): PID-scoped, not a fixed
  filename, so concurrent `cargo test` binaries (different PIDs) cannot collide, and within one
  binary this single test owns the path. `remove_dir_all(&dir)` (`:1393`) sweeps the whole dir
  incl. any WAL/SHM sidecars. No reliance on test ordering. Minor residual: cleanup is not RAII-
  guarded, so a panicking assertion leaks the dir until the next same-PID run overwrites it; because
  assertions match rows by id (not counts), a stale leftover is idempotent-safe. Non-blocking.
- **App.test.tsx mock scoping** — correct hoisting (`vi.mock` at module top, `:16-40`, spreads
  `importOriginal` and overrides only `listUseCases`/`modelsForUseCase`). Async handled via
  `flush()` (act-wrapped setTimeout loop) after each click; selection read off the always-present
  `.model-item-composer .model-item-name`. One caveat below (non-blocking).
- **mock.ts constant usage** — consistent; `mock.test.ts` drift guard
  (`MOCK_MODELS[0].id === WORKHORSE_MODEL_ID`) still holds.

### New findings
Blocking: none.

Non-blocking:
- `ui/src/App.test.tsx:16-40` — the `vi.mock("./api")` is module-wide, so the file's OTHER two
  tests (view-switch S12, delete-confirmation) now also render with three synthetic use-case tabs
  they previously never saw (browse mode returned `[]`). Their assertions (`.rail-meta`, `.stage`,
  `button button`) remain meaningful and orthogonal to use-case tabs, so this is a fixture-scope
  smell, not a broken test. Fix (optional): scope via `vi.doMock`/per-test override, or add a
  one-line assertion in the S12 tests acknowledging the injected tabs.
- `ui/src/App.test.tsx:168` — the image-tab assertion checks `toContain("Nano Banana Pro")`, but
  since the mock's image list has no never-used ids at all (mock.ts is seedance/grok-free), the
  "never a never-used id" guarantee here leans on the fixture rather than on App's fallback logic.
  The workhorse-vs-launch direction is still genuinely exercised. Minor.

### Security delta
No security-relevant change. `git diff` of the three repaired files (`store.rs`, `App.test.tsx`,
`mock.ts`) grepped for `reqwest|http|TcpStream|set_var|remove_var|ALLOW_SPEND|vault|keyring|
Application Support`: the ONLY hit is a benign comment in the upgrade test asserting it writes to
`temp_dir()` and "never `~/Library/Application Support/`" (`store.rs` test body). No network, no
socket, no key/env read or mutation, no `ALLOW_SPEND`, no touch of the owner's real DB path. The
upgrade test's SQL is parameterised or fixed literal DDL on a throwaway temp DB; `CostModel`
unchanged, no new dependency, migration count still 12. Security posture unchanged from the prior
PASS.

VERDICT: PASS — all three addressed follow-ups are genuinely closed (real v11→v12 data-preservation
proof with full-row equality and PID-scoped temp hygiene; A1 cross-tab test now clicks and asserts
the workhorse survives a video tab and falls to a launch model — never a never-used id — on an
image tab; mock.ts routed through `WORKHORSE_MODEL_ID` with no import cycle). No new blocking or
major findings; the two new non-blocking items are test-fixture smells only. Critical path
(fail-closed server refusal, migration 12, workhorse defaults) remains correct and covered.

## Repair round 2 — re-review

Delta re-review after repair round 2 (TEST FILES ONLY). Read-only; nothing implemented,
no source changed, no builds/tests run (TEST re-ran the gate: 979 Rust / 321 UI, five
independent mutations RED). Scope: the new `ui/src/App.workhorse.test.tsx` and the
`git diff` of `ui/src/App.test.tsx`; production untouched.

### Prior fixture-scope note (round-1 non-blocking #1) — CLOSED
The round-1 concern was that `App.test.tsx`'s module-wide `vi.mock("./api")` bled three
synthetic use-case tabs into the two orthogonal S12 tests. Round 2 removes that mock
entirely: `ui/src/App.test.tsx` now imports no `vi` (`:1`), has no `vi.mock`, and its diff
vs HEAD is +6 lines — a header comment pointing to the new file, zero removed lines, no
assertion weakened. The two S12 tests (Library meta-rail, delete-confirmation
button-in-button) run again against the plain browse-mode api. Note closed.

### New test file `ui/src/App.workhorse.test.tsx` — clean
Checked and found honest:
- **Fixture design falsifiable.** Workhorse is at index 2 in every list it appears in
  (`:59-80`); index 0 is never the expected answer (initialRoster[0]=gemini_omni non-launch;
  imageList[0]=recraft_v4_1 non-launch; videoB[0]=wan_2_6 launch-but-not-workhorse). The
  videoB step (`:183-189`) forces the FALLBACK branch (current image selection absent from the
  list, workhorse present but not first-isLaunch), so `m.find(WORKHORSE)` is the sole decider —
  matching TEST's M2/M3b/M4 RED results.
- **Attributes consistent with `mock.ts`/`types.ts`.** Verified every fixture id's
  modality/isLaunch/displayName against `mock.ts`: gemini_omni video/false, kling_3_0
  video/true, H3 Max Turbo video/true, wan_2_6 video/true, recraft_v4_1 image/false,
  nano_banana_pro image/true ("Nano Banana Pro"), seedream_5_pro image/true. `byId` (`:52-56`)
  throws if a fixture id is missing, so drift can't render a silent tautology.
- **Uses the constant, not a literal.** Fixtures go through `WORKHORSE_MODEL_ID` (`:62,:68,:79`);
  the only `minimax_h3_max_turbo` occurrence is in the explanatory header comment (`:22`), not
  code.
- **Really clicks tabs / async correct.** `clickTab` clicks `[role="tab"][data-tab=...]`
  (`:155-160`) and every interaction is followed by `await flush()` (act-wrapped setTimeout(0)
  loop, `:129-136`); render is `await act(async...)` (`:145-147`). No un-awaited state updates,
  no arbitrary/real timeouts, no `findBy` needed since reads happen post-flush.
- **Mock scoped to this file** (`:82-108`), spreads `importOriginal` and overrides only
  `listModels`/`listUseCases`/`modelsForUseCase`. No leakage to other files.
- **Never-used guarantee attributed correctly.** The header (`:44-49`) and inline comment
  (`:175-178`) both state that excluding Seedance/Grok is a SERVER-SIDE guarantee owned by the
  Rust A3 tests, and the file only asserts the image-tab landing model isLaunch + expected id —
  not faked in the UI mock. Honest.
- **Readability / comment density** matches neighbouring tests; comments describe what the code
  actually does.
- **No private names, no network, no key/env handling.** Grep for
  `reqwest|http|set_var|process.env|KEY` finds nothing; the only `localStorage` use is the
  standard onboarding-skip flag (`:117,:126`), identical to `App.test.tsx`.

### No production change in round 2
`git diff --stat` shows `ui/src/App.tsx` at +40/-4 — the same hunk reviewed and PASSed in round 1
(the fallback logic at ~:351 is unchanged). The only round-2 deltas are the comment-only
`App.test.tsx` diff and the new untracked `App.workhorse.test.tsx`. `App.tsx`/`types.ts`/`mock.ts`
untouched (TEST proved byte-identical via `cmp`).

### No new findings
Blocker: none. Major: none. Minor: none. Round-1 non-blocking #2 (image-tab guarantee leaning on
the fixture) is now explicitly and correctly reframed as a server-side/Rust-owned guarantee, so
it is no longer even a smell.

VERDICT: PASS — the round-1 fixture-scope note is closed (module-wide mock removed from
App.test.tsx; A1 moved into a scoped, hand-ordered, genuinely-falsifiable own file that the
mutation battery drives RED). Production code unchanged since the round-1 PASS. The whole slice —
fail-closed server refusal of never-used routes (Rust A3), migration 12 data preservation,
workhorse defaults/fallback — remains correct and covered. No unresolved blocker or major.
