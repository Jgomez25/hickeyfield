# S20b-never-used-routes — DESIGN VERIFY

- ARTIFACT: `.forge/slices/S20b-never-used-routes/design.md`
- ARTIFACT-HASH (`shasum -a 256`, now): `853d63ec505bf4030e396036633fd8625ee9903d4569fa40a073c1a7c2f512ae`
- SOURCES OF TRUTH: `.forge/slices/S20b-never-used-routes/slice.md` (A1–A7, G1–G2, out-of-scope);
  `.forge/objective.md` §R0.3, §R-RULES; `.forge/STATUS.md:118–120` (test-locked selectors).
- Method: read-only inspection of this worktree (dummy keys set, `HICKEYFIELD_NO_KEYCHAIN=1`,
  never `HICKEYFIELD_ALLOW_SPEND`, `first_light` never run, test suite NOT run per instruction).

## Scripted / first-hand checks (real output)

- `shasum -a 256 design.md` → `853d63ec…512ae` (exit 0).
- `grep -oE '"(seedance|grok)[a-z0-9_.-]*"' registry.rs | sort -u | wc -l` → **11** ids, exactly:
  `grok_edit_video, grok_extend_video, grok_image, grok_video, grok_video_v15, seedance1_5,
  seedance_2_0, seedance_2_0_fast, seedance_2_0_mini, seedance_2_5, seedance_pro`. Matches the
  design's enumerated set (design:31–32). `grok_video` is a real JOB_TYPES/catalogue id
  (registry.rs:303) priced via `other => higgsfield_only` (registry.rs:1609) — correctly included.
- `seedream*` ids (`seedream_v4_5/v5_lite/v5_pro/v5_pro_edit`) do NOT start with `seedance`
  (diverge at char 4) → not over-matched. Hyphenated strings `seedance-2.0`, `grok-imagine-video`
  are route slugs, not model ids, so the id-keyed prefix predicate never sees them. Enumeration and
  boundary are correct.
- `LAUNCH_FAMILIES: [_;12]` confirmed (registry.rs:214), Seedance 2.0 row = 3 ids (registry.rs:218–220).
- `there_are_twelve_launch_families` asserts only `len()==12` + per-id launch marker (registry.rs:2775);
  `only_launch_models_carry_the_marker` computes `expected` from `LAUNCH_FAMILIES.iter().map(ids.len).sum()`
  (registry.rs:2830) — fully self-referential. Swapping the 3-id Seedance row for the 1-id workhorse row
  (launch ids 14→12, families 12→12) keeps both green with no edit. Design:77–79 correct.
- `submit_one` = `pub(crate) fn submit_one(store: &Arc<SqliteStore>, …)` (commands.rs:1096–1104): holds
  `store`, takes NO `AppHandle`. Callers: commands.rs:1029/1838/1910/2078 and chain.rs:293/643/793 —
  none threads an `AppHandle`. Design:48,385 correct.
- `store.rs` migrations append-only via `PRAGMA user_version`, top at **11** (store.rs:405; tests assert
  `user_version==11` at 878/1049/1136). The `settings TEXT` column is on `job_sets` (store.rs:228) — per
  job, not a global KV. No `app_pref`/kv/meta table exists. Design:50,110–114 correct.
- Config-file pref home (`library_root`) requires `app: &AppHandle` (app.rs:94–106) — unreachable from
  `submit_one`. Design:51,387 correct.
- `list_models()` (commands.rs:437, no args) and `models_for_use_case(use_case)` (:447) both call
  `models_matching(use_case)` (:451, no `store`/`State`). Registered as tauri commands (lib.rs:87,89).

## Checklist

- [PASS] C1 (A2 launch swap keeps 12, self-referential tests green) — quote: "Array stays `[_; 12]` →
  `there_are_twelve_launch_families` (`:2775`) stays green with **no edit**." Verified against
  registry.rs:2775,2830.
- [PASS] C2 (A3 exhaustive 11-id set; prefix boundary) — quote: "the registry contains **exactly these 11
  ids**". Enumeration matches; `seedream*` on the KEPT side is explicit — quote: "`is_never_used("seedream_v5_pro")==false`".
- [PASS] C3 (A3 test is set-EQUALITY, not non-emptiness) — quote: "`is_never_used(id)` ⇔ `id∈{the 11}`;
  asserts the set equals the enumerated 11 (none missed, Seedream/others not over-matched)".
- [PASS] C4 (#1 migration-vs-reuse justified) — quote: "That boundary is `submit_one`, which holds `store`
  but **no `AppHandle`**." Confirmed submit_one:1096–1104, app.rs:94, store.rs:405, no kv table. Migration
  12 is append-only and follows the file's `CREATE TABLE IF NOT EXISTS … PRAGMA user_version` pattern.
- [PASS] C5 (A4 server refusal precedes route/client; covers all entries) — quote: "Top of `submit_one`
  (`commands.rs:1096`, before compile/estimate/route)". All spend paths funnel through submit_one:1096
  (verified callers) so one check covers standalone/storyboard/keyframe/chain.
- [PASS] C6 (A4 unfiltered display-name lookup) — quote: "`pub fn model_names() -> Vec<ModelNameDto>`" over
  `registry().into_values()`; unfiltered id→name resolvers `estimate_with`/`model_capabilities` stay
  (commands.rs:711,540). Sound.
- [PASS] C7 (A1 workhorse by name, per-language constant) — quote: "`const first = m.find(x => x.id ===
  WORKHORSE_MODEL_ID) ?? m.find(x => x.isLaunch && x.modality === "video") ?? m[0];`" — App.tsx:194/336
  and Storyboard.tsx:210 match current source.
- [PASS] C8 (A6 mock/sample lead with workhorse; grep=0) — quote: "the sample job (`:490–496`) →
  `modelId:"minimax_h3_max_turbo"`" and "make each list **lead with `minimax_h3_max_turbo`**"; test row
  asserts "sample job + preset lists lead with the workhorse" and `/seedance|grok/i` = 0. Presets are all
  video (mock.ts:210–286) so the video workhorse can lead each. O3's `veo_3_1_fast` is a neutral swap, not
  the lead — acceptable because the lists lead with the workhorse and MOCK_MODELS[0] is the workhorse
  (mock.ts:57). Satisfies A6/R0.3.
- [PASS] C9 (A7 no CostModel change / no delete / no new dep; grok_video_v15 settle test stays) — quote:
  "No route/price/capability row deleted — `grok_video_v15` and the whole Seedance line stay priced".
- [PASS] C10 (Non-goals + Open questions present) — quote: "## Non-goals (recorded, not skipped)" and
  "## Open questions (each with a default this build assumes)".
- [FAIL] C11 (A7/G1: 971 Rust baseline stays green after threading `State` into the picker feeds) —
  quote: "`list_models` and `models_for_use_case` gain `state: State<'_, AppState>`". Changing
  `list_models`'s signature breaks FIVE in-crate Rust tests that call `list_models()` with no arguments —
  `the_catalogue_is_exposed_and_launch_models_come_first` (commands.rs:2291),
  `route_dto_carries_the_promo_as_of` (:2309), `every_model_offers_at_least_one_route` (:2337),
  `a_known_model_and_route_quotes_a_positive_price` (:2356),
  `routes_report_availability_so_the_ui_can_explain_itself` (:2520) — a compile error (E0061) that fails
  the whole `cargo test --workspace` build. Worse, `the_catalogue_is_exposed…` asserts `models.len() >= 55`
  (commands.rs:2292) against a ~57-item roster (registry.rs:486 "a 57-item flat list"); once the default
  (pref-OFF) filter hides the 11 never-used ids the list is ~46, so the assertion is now FALSE — a
  semantic break that no mechanical signature update repairs. The design's "Collateral test updates"
  section (design:283–296) enumerates only UI tests and nowhere lists these Rust tests or a repair, while
  the test plan claims "baseline **971 Rust** … stays green" (design:337) and A7 asserts "the existing
  registry/prices/store suites … stay green" (design:331). Design:396 only *flags* this as a risk to
  "confirm the build stays green" — flagging is not specifying the fix. This contradicts G1/A7.
- [FAIL] C12 (#4 every `models_matching`/`list_models` caller-and-test accounted for) — quote: "param
  ripples to every `models_matching(...)` call site and any test calling `list_models` directly." The
  design commits to the risk but does not enumerate the five call sites above nor state the required
  edit (route them at the new `models_matching(&store, use_case)` seam AND decide the corrected count /
  which pref state the count test asserts). Left unspecified, forge-spec-author cannot repair without
  guessing.

## Rulings requested

- **#1 migration (A3.2): JUSTIFIED, not a violation.** `submit_one` holds `store` and no `AppHandle`
  (commands.rs:1096); the only server-readable alternative pref home is the `library_root` config file,
  which is reachable only via `AppHandle` (app.rs:94); no existing app-level KV/settings/meta table exists
  (the `settings` column is per-job, store.rs:228); migrations are append-only at 11 and migration 12 uses
  the file's `CREATE TABLE IF NOT EXISTS … STRICT; PRAGMA user_version` pattern. A7's "no migration unless
  the settings store genuinely needs one (design must justify)" is satisfied.
- **#6 mock lead (A6/R0.3): SATISFIED.** O3 replacing the removed `seedance_2_0` MODEL with the neutral
  in-roster `veo_3_1_fast` is fine because the design additionally requires the sample job to be the
  workhorse (`minimax_h3_max_turbo`) and the preset `models:` lists to LEAD with the workhorse (all mock
  presets are video, so the video workhorse fits every one), and the A6 grep test enforces both the
  workhorse-lead and `/seedance|grok/i`=0. No preset or sample job would lead with a non-workhorse model.

## Root cause (blocking)

The design's A3.3 changes the two picker-feed commands `list_models` / `models_for_use_case` to take
`State<'_, AppState>` and makes `models_matching` filter never-used ids by default, but its collateral-test
plan only covers UI vitests. It leaves five existing `#[cfg(test)]` Rust tests in `commands.rs` that call
`list_models()` directly (2291, 2309, 2337, 2356, 2520) uncompilable, and `the_catalogue_is_exposed_and_
launch_models_come_first`'s `models.len() >= 55` assertion becomes false once the ~11 never-used ids are
hidden by default — while the design claims the 971-test Rust baseline stays green (A7/G1). To repair:
add these five tests to the collateral-update section, specify each is re-pointed at the new
`models_matching(&store, use_case)` seam with an explicit `SqliteStore`, and state the corrected count /
pref state for the `>= 55` assertion (e.g. assert against the pref-ON list, or against
`registry().len() - never_used_count`), so G1/A7 hold as written.

## Non-blocking notes for the implementer

- `Storyboard.tsx` lives at `ui/src/components/Storyboard.tsx`, not the bare `ui/src/Storyboard.tsx` the
  design cites; `modelName` is at line **133**, not `:134` (off-by-one). `modelPrefs` :173 and the
  `models[0]` new-shot default :210 are exact.
- The MOCK id namespace differs from the registry (`kling_3_0` vs `kling3_0`, `veo_3_1_fast` vs
  `veo3_1_fast`); the design's `veo_3_1_fast`/`minimax_h3_max_turbo` mock ids match mock.ts — fine, just
  keep mock-vs-registry ids distinct in the A6 sweep.
- `a_subtitle_describes_the_model_rather_than_repeating_its_slug` (commands.rs:2528) reads
  `reg["seedance_2_0"]` directly from the unfiltered registry — it correctly survives hiding (hidden ≠
  deleted); confirm it is NOT accidentally swept.
- Refusal code uses `model.display_name` / `model.id`; `submit_one`'s `model: &Model` (commands.rs:1100)
  exposes both — the snippet compiles against the real signature.

VERDICT: FAIL

---

## Re-verification of Revision 1

- ARTIFACT: `.forge/slices/S20b-never-used-routes/design.md`
- ARTIFACT-HASH (`shasum -a 256`, NOW): `237154c5f946dfd59ddbe087f416490050611e0cf1c4a012527cef7e289bff50`
  (changed from the prior `853d63ec…512ae` — this is a genuinely revised artifact).
- SOURCES OF TRUTH: `slice.md` (A1–A7, G1–G2), `.forge/objective.md` §R0.3/§R-RULES, `.forge/STATUS.md:118–120`.
- Method: read-only inspection of this worktree; dummy `hickeyfield_*_KEY=x` + `HICKEYFIELD_NO_KEYCHAIN=1`
  set on every command; never `HICKEYFIELD_ALLOW_SPEND`, never `first_light`; suite NOT run (counts read
  from source per instruction).

### Scripted / first-hand checks (real output)

- `grep -rn "list_models(\|models_for_use_case(\|models_matching(" src-tauri/src crates ui/src` (exit 0):
  only `commands.rs` — 2 wrappers (`:437,:447`), pure fn (`:451`), and the FIVE test callers
  `list_models()` at `:2291,:2309,:2337,:2356,:2520`. **Zero** callers in `crates`, **zero** in `ui/src`.
  Matches the design's Rev-1 grep claim (design:342–351,77) exactly. No caller is missed.
- `the_catalogue_is_exposed_and_launch_models_come_first` (commands.rs:2290): `let models = list_models();`
  then `assert!(models.len() >= 55)` (`:2292`) and `assert!(first_non_launch >= 12)` (`:2302`) —
  read first-hand.
- `LAUNCH_FAMILIES: [_;12]` (registry.rs:214) has 16 flat launch ids today (counted from `:215–233`):
  kling3_0, kling2_6, seedance_2_0/_fast/_mini, wan2_7, wan2_6, veo3_1/_fast/_lite, gemini_omni,
  nano_banana_2, nano_banana_flash, seedream_v5_pro, flux_2, gpt_image_2.
- Full-roster arithmetic: catalogue = **57** `###` models (`vendor/higgsfield-cli-MODELS.md`); the pinned
  regression `registry_covers_the_catalogue_minus_its_exclusions` asserts
  `reg.len() == cat.len() - EXCLUSIONS.len() + 32` (registry.rs:2614) ⇒ `57 - 2 + 32 = 87`. So
  `list_models()` returns **87** models today; `models_matching(None)` iterates `registry().into_values()`
  unfiltered (commands.rs:453). `is_never_used` grep = new (none pre-existing). `submit_one` holds
  `store: &Arc<SqliteStore>` and NO `AppHandle` (commands.rs:1096–1104). Store `user_version` top = 11
  (store.rs). Workhorse present + priced (registry.rs:318,1175) and pinned in-catalogue (registry.rs:3012).

### The real counts (task #2)

- **Full roster the `>= 55` test sees today = 87** (`list_models()` = `registry().len()` =
  `57 − 2 + 32`), NOT ~57. Rev 1 re-points all five tests at `models_matching(true, None)`, which
  reproduces exactly today's unfiltered `list_models()` set (87). `>= 55` holds with a 32-item margin.
  Even the pref-OFF default (`87 − 11 = 76`) clears `>= 55` — so the compile break (E0061) was the only
  real prior blocker; the "semantic `>= 55` break" the first pass alleged rested on a ~57 mis-count and
  would not have fired. The Rev-1 repair closes the real (compile) blocker regardless.
- **Launch-id count: 16 today → 14 after the A2 swap** (Seedance's 3 ids replaced by the 1 workhorse id =
  drop of 2, i.e. N→N−2). The assertion that depends on it, `first_non_launch >= 12` (commands.rs:2302),
  holds either way (14 ≥ 12). `there_are_twelve_launch_families` (family count 12→12) and
  `only_launch_models_carry_the_marker` (self-referential sum) stay green with no edit.
  NOTE: the design's stated "14→12" (design:111,355) is the wrong absolute (real 16→14); the DROP of 2 and
  the `>= 12` conclusion are correct, so this does not change any assertion outcome — non-blocking.

### Re-checklist

- [PASS] R-C11 (prior blocker closed: pure `State`-free seam) — quote: "`models_matching(show_never_used:
  bool, use_case)` is a pure fn; the `#[tauri::command]` wrappers … read the pref from the store and
  delegate. Rust tests call the pure fn directly (no `State`, no store needed)." Grep confirms the only
  affected callers are the five named tests; each re-pointed at `models_matching(true, None)`.
- [PASS] R-C12 (all five callers enumerated + repaired, intent preserved) — quote: "each is re-pointed at
  `models_matching(true, None)` — `show_never_used = true` — which is **exactly what `list_models()`
  returned before this slice** (unfiltered)". Table (design:353–359) names each test, its assertion, mode
  `true`, and the one-line edit. `>= 55` and `first_non_launch >= 12` preserved (not weakened).
- [PASS] R-C13 (new default-hidden test derives the count, no magic number) — quote: "`assert_eq!(never,
  11 …); assert_eq!(shown.len(), full.len() - never); assert!(shown.iter().all(|m|
  !hickeyfield_core::registry::is_never_used(&m.id)))`". Derived (`full − never`), pins enumeration at 11,
  proves no hidden id survives the default.
- [PASS] R-C14 (MUST-NOT-sweep unfiltered tests protected) — quote: "`a_subtitle_describes_the_model_rather
  _than_repeating_its_slug` (`:2527`) reads `reg["seedance_2_0"]` straight from the **unfiltered**
  `registry()` … this test stays UNCHANGED and green". Correct: hidden ≠ deleted.
- [PASS] R-C15 (three new commands registered) — quote: "Register both `get_show_never_used` and
  `set_show_never_used` in `generate_handler![…]` (`lib.rs`) alongside `model_names` — three new command
  lines total." `generate_handler!` is at lib.rs:77 with `list_models`:87/`models_for_use_case`:89 — the
  home the design names exists.
- [PASS] R-C16 (nothing the first pass confirmed regressed) — migration 12 still justified (`submit_one`
  store+no-AppHandle confirmed `:1096`; top `user_version`=11); 11-id prefix set intact; A4 refusal still
  at the `submit_one` funnel; unfiltered `model_names()` for history; mock leads with the workhorse; A5
  additive toggle touches no locked selector — quote: "Everything the gate confirmed is unchanged:
  migration 12 justified (C4); 11-id set (C2); prefix predicate boundary (C3); A4 refusal at the
  `submit_one` funnel (C5); mock/samples lead with the workhorse (C8/#6)."

### Ruling on the thin wrappers (task #4) — ACCEPTABLE (non-blocking)

Leaving `list_models`/`models_for_use_case` (and `get_show_never_used`/`set_show_never_used`) without a
direct unit test is acceptable here, because BOTH rule conditions hold:
1. Each wrapper is trivially correct — a two-line "read pref → delegate": e.g.
   `let show = state.store.get_bool_pref(store::SHOW_NEVER_USED, false); models_matching(show, None)`
   (design:186–194). The behaviour lives in the pure fn, which IS unit-tested
   (`list_models_hides_the_never_used_routes_by_default`, both modes).
2. The pref plumbing is covered: `migration_12_adds_app_pref_and_roundtrips` covers `get_bool_pref`
   default-false + set→get round-trip on a temp store (design:414), and
   `submit_one_refuses_a_hidden_model_while_the_toggle_is_off` covers the spend boundary reading the SAME
   pref (design:416). The A5 vitest covers the toggle's `onChange → setShowNeverUsed → re-fetch`.
Note (non-blocking): the wrapper's own `pref → pure-fn` hand-off is only exercised indirectly (Tauri
injects `State`, so a direct in-crate test would have to construct `AppState`). Given (1)+(2) that gap is
immaterial; if the implementer wants belt-and-braces, one integration test that seeds the pref via
`set_show_never_used` and asserts `list_models` omits/includes the 11 would close it — optional, not required.

### Non-blocking notes for the implementer

- The design's roster figure "~57" (design:355) is the CATALOGUE count; the real `list_models()` roster is
  **87** (`57 − 2 + 32`). The conclusion (`>= 55` stays true under `models_matching(true, None)`) is
  correct; only the stated magnitude is off. Do not "correct" the test to a tighter number — keep `>= 55`.
- The design's "launch id count 14→12" is really **16→14**; the drop-of-2 and `first_non_launch >= 12`
  reasoning are sound. No edit to `there_are_twelve_launch_families` / `only_launch_models_carry_the_marker`.
- Test-line citations in the Rev-1 table (`:2290,:2306,:2337,:2355,:2519`) are the `#[test]`/fn-name lines;
  the actual `list_models()` call sites are `:2291,:2309,:2337,:2356,:2520`. Names are unambiguous — cosmetic.
- `models_matching` is private; the re-pointed tests live in the same `#[cfg(test)] mod tests` in
  `commands.rs`, so `models_matching(true/false, None)` is reachable. Implementer must add `use tauri::State`
  and confirm `AppState.store` derefs for `get_bool_pref` — implementation detail, not a design gap.

### Root-cause status

The single prior blocker (C11/C12: threading `State` into `list_models` broke five in-crate tests that call
`list_models()` with no args, an E0061 compile failure across `cargo test --workspace`) is CLOSED by
Revision 1's pure `State`-free `models_matching(show_never_used: bool, use_case)` seam: the five tests are
each re-pointed at `models_matching(true, None)` (naming test, line, assertion, mode, and exact edit), which
reproduces the pre-slice unfiltered set so `>= 55` and `first_non_launch >= 12` stay honest, plus one NEW
derived default-hidden test. Grep confirms no other caller in `src-tauri/src`, `crates`, or `ui/src`. No new
blocker introduced; the remaining findings are cosmetic numeric misstatements that change no assertion.

VERDICT: PASS
