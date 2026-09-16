# S13-storyboard-plan — IMPLEMENT (claim)

Author: implementer agent. Built exactly the one slice per `slice.md` (AC1–AC7) and
`design.md` (D1–D7). Every claim below has a pasted run. Branch `forge/s13-storyboard-plan`,
working tree only — no commit, no PR.

## Two design defaults the owner has NOT separately confirmed (stated prominently)

1. **Grouping table is `storyboards`, not `job_sets`.** The contract's Ground truth said the
   grouping table should be `job_sets`, but `job_sets` is already the *jobs* table
   (`store.rs` `version < 1` block). Reusing it would collide. Per design D3 / Q5 I created a
   new `storyboards` table and put two write-once columns `storyboard_id` / `shot_index` on the
   existing `job_sets` (jobs) table. If the owner wanted the literal name, this is the deviation.
2. **Shot failure is best-effort continue.** Per design D4 / Q3, `generate_storyboard` records
   the storyboard row first, then submits each shot; a shot that fails carries its error in its
   outcome and the batch **continues** (a refusal on shot 3 must not erase shots 1–2 the user
   already paid to start). It is not all-or-nothing.

## Other stated deviations / refinements from the design

- **Parse shape.** Design D1.3 shows `StoryboardPlan` fields without `serde(default)` and parses
  the reply directly into it. I gave `brief`/`target_seconds`/`use_case` `#[serde(default)]` so a
  reply of just `{"shots":[…]}` parses, and `plan()` overwrites those three with the authoritative
  command inputs (the model only drafts the shots). Still `serde_json::from_str::<StoryboardPlan>`,
  as the design's test names assume. This is more robust (the model can't corrupt the target).
- **`clamp_shot` signature** takes a `shot_index` so the advisory reads "shot N: … clamped" exactly
  as design D2 specifies.
- **`generate_storyboard` happy path is not unit-tested** (see AC4/AC6 note and §10.2). A successful
  submit needs a live paid provider, which agents never call. The offline test drives the
  failure/continue/record path with unknown models only, so it cannot spend a cent even on a
  machine holding a real key.

## Files changed

Core (`crates/hickeyfield-core/src/`):
- `storyboard.rs` — **new**. `Shot`/`StoryboardPlan`/`ShotRole`/`Continuity`/`PlanError` +
  `Display`, `plan(&dyn Chat, …)`, `validate`, `clamp_shot`, `build_user_message`, and 12 tests
  with a `FakeChat`.
- `enhancer.rs` — added the `Chat` trait + `ChatBackend` enum (Ollama/Hosted) implementing `Chat`
  by reusing the existing private wire fns (`ollama_body`/`openai_body`/`anthropic_body` + text
  readers + `client`/`send_json`/`check_ollama_read_everything`). No new HTTP client. `Enhancer`
  and its tests untouched.
- `corpus.rs` — added `STORYBOARD_SYSTEM` (`include_str!("../../../prompts/storyboard.v1.md")`) +
  two budget/shape tests.
- `engine.rs` — two `#[serde(default)]` fields on `JobSet`: `storyboard_id: Option<String>`,
  `shot_index: Option<u32>`.
- `lib.rs` — `pub mod storyboard;` + re-exports `Continuity/PlanError/Shot/ShotRole/StoryboardPlan`.
- `recipe.rs` — the two new `JobSet` literals in tests gained the two fields (compile only).

Prompts:
- `prompts/storyboard.v1.md` — **new** original-prose system prompt (passes provenance lint).

Shell (`src-tauri/src/`):
- `store.rs` — migration 8 (`storyboards` table + `ALTER TABLE job_sets ADD storyboard_id/shot_index`);
  `Storyboard` struct; `upsert_storyboard`/`storyboard`/`storyboards`/`jobs_in_storyboard`; INSERT
  columns + params for the two write-once columns (absent from `ON CONFLICT DO UPDATE SET`);
  `row_to_job` reads both; migration-7 version assertion bumped 7→8; v1-upgrade drop-list gains the
  two columns + `DROP TABLE storyboards`; two new migration-8 tests.
- `commands.rs` — factored `submit_one` out of `submit_job`; new `chat_backend_for`, `plan_storyboard`,
  `generate_storyboard` (+ inner), `list_storyboards`; DTOs `StoryboardGenerateInput`/`ShotOutcome`/
  `StoryboardResult`/`StoryboardDto`; 5 new tests.
- `lib.rs` — registered `plan_storyboard`, `generate_storyboard`, `list_storyboards`.
- `runner.rs`, `app.rs` — `JobSet` literals gained the two fields (compile only).

UI (`ui/src/`):
- `types.ts` — `Shot`/`StoryboardPlan`/`ShotOutcome`/`StoryboardResult`/`Storyboard`/`Continuity`/
  `ShotRole`; `storyboardId?`/`shotIndex?` on `JobSet`.
- `api.ts` — `planStoryboard`/`generateStoryboard`/`listStoryboards` + `toJobSet` maps the two new
  fields; browser-mode mocks wired.
- `mock.ts` — `mockPlanStoryboard`/`mockGenerateStoryboard`.
- `lib/cost.ts` — `totalCostCaption` (sums priced shots, marks the total unpriced, never $0).
- `components/Storyboard.tsx` — **new** view (brief + target + Draft, editable shot cards reusing
  `ModelPicker` and `PromptPreview` unchanged, reorder/add/delete, per-shot preview, summed total,
  Generate-all gated on the total).
- `components/Library.tsx` — additive `storyboards` prop → groups a set under its brief; standalone
  jobs render exactly as before (optional prop, default `[]`, so the S12 test is unchanged).
- `App.tsx` — third `view` value `"storyboard"`, titlebar tab, storyboard state + bridge wrappers,
  passes `storyboards` to Library.
- `base.css` — `.workspace[data-view="storyboard"]` grid template + storyboard/group styles (no new
  app.css import).
- Tests: `components/Storyboard.test.tsx` (new, 3), `lib/__tests__/cost.test.ts` (+3), `api.test.ts`
  (+3).

Docs: `CHANGELOG.md` `[Unreleased] → Added`; `README.md` "What it does today".

## Test-locked names reused, not renamed
`ModelPicker` (`li.model-item`/`button.model-item-main`/`chip-route`/`onSelect`), `PromptPreview`
(`textarea.prompt-preview-text`/`button.prompt-preview-retry`/`button.generate-button`), the
`.library-tile[data-job-anchor]` gallery contract, and `stageCostCaption`/`formatUsd` — all reused
verbatim. Storyboard markup adds new classes only.

## submit_one factoring — single-job submit unchanged (§10.2 / AC4)

`submit_job` is now `submit_one(&state, model, &input, None, None)`. `submit_one` is the former
`submit_job` body verbatim. The only diffs (from `git diff src-tauri/src/commands.rs`):
- signature change + wrapper (the two lines above);
- `compiled_for_submit(model, &input)` → `(model, input)` because `input` is now `&SubmitInput`;
- the `JobSet` literal gained `storyboard_id: storyboard_id.map(str::to_string)` and `shot_index`,
  which are `None`/`None` on the standalone path — i.e. identical to before.
No estimate, route, guard, upsert, submit or watch step changed. Every existing submit/preview test
stays green (see gate).

## Acceptance criteria → proof

- **AC1 (shot-list drafting + validator, no network):** `storyboard.rs` tests
  `a_valid_plan_is_accepted`, `a_bad_model_id_is_rejected_with_a_reason`,
  `a_model_that_cannot_run_the_tab_is_rejected`, `a_sum_outside_ten_percent_is_rejected`,
  `a_refusal_is_a_typed_error_not_garbage`, `garbage_that_is_not_json_is_rejected`,
  `an_unknown_camera_is_rejected`, `a_backend_error_is_an_honest_plan_error`,
  `the_shot_count_bound_is_three_to_eight`; unavailable-backend honest error at the command boundary:
  `commands.rs` `chat_backend_with_nothing_available_is_an_honest_note`. All use `FakeChat` / pure
  selection — no network.
- **AC2 (model-aware clamp):** `a_twelve_second_ltx_shot_clamps_to_ten_with_an_advisory` and
  `a_single_shot_seedance_thirty_survives`.
- **AC3 (UI):** `Storyboard.test.tsx` `draft renders cards and editing a shot updates the total`,
  `an unpriced shot marks the total`, `does not draft with an empty brief`; `cost.test.ts`
  `totalCostCaption` group.
- **AC4 (generate-all + set relation + single-job unchanged):** `store.rs`
  `migration_8_adds_storyboards_and_columns_and_roundtrips`,
  `jobs_in_a_storyboard_are_ordered_by_shot_index`; `commands.rs`
  `generate_records_the_set_even_when_shots_fail`, `generate_numbers_outcomes_densely_from_zero`;
  single-job unchanged via the untouched existing submit/preview tests + the diff above.
- **AC5 (continuity recorded + round-trips):** `storyboard.rs`
  `a_plan_with_continuity_and_roles_round_trips_through_serde`; store round-trip via the migration-8
  test (plan_json in/out).
- **AC6 (money honesty):** every shot goes through `submit_one`'s estimate+guard+route path (no shot
  bypasses it — validator rule 4 and `route::resolve` gate on the same runnable predicate); the total
  is a UI gate (`totalCostCaption` + Generate-all disabled until estimates render); no unrunnable
  model receives a shot (`a_model_that_cannot_run_the_tab_is_rejected`). Drafting takes no `State`, so
  it cannot bill fal; documented as one enhancer completion (D7, CHANGELOG, README, Storyboard header).
- **AC7 (regression gate):** pasted below.

## Gate — every command and its observed output

```
$ hickeyfield_FAL_KEY=x … cargo test --workspace
test result: ok. 700 passed (core lib) …
test result: ok. 132 passed; 0 failed; 1 ignored (shell lib) …
   Doc-tests … 0 passed
total passed (summed across suites): 841        # >= 820 ✓, 0 failed

$ cargo fmt --all --check
FMT_CLEAN (exit 0)

$ cargo clippy --workspace --all-targets -- -D warnings
Finished `dev` profile … (0 warnings, 0 errors)

$ ./scripts/lint-provenance.py
CDN hostnames        PASS  no third-party media hosts referenced
Copy provenance      PASS  no shipped string matches the reference corpus (80015 shingles indexed)

$ env -u hickeyfield_FAL_KEY -u FAL_KEY cargo run -q -p hickeyfield-core --example audit_fal
=== PINNED FIELDS … (0) ===
=== UNREACHABLE … (0) ===          # unchanged: PINNED 0, UNREACHABLE 0

$ cd ui && pnpm test
Test Files  23 passed (23)
     Tests  281 passed (281)        # >= 272 ✓

$ cd ui && pnpm build
tsc --noEmit && vite build → ✓ built in ~1s (dist emitted)

$ ui/node_modules/.bin/tauri build --bundles app        # from repo root
    Finished `release` profile [optimized] target(s) in 8m 38s
    Built application at: …/target/release/hickeyfield-tauri
    Bundling Hickeyfield.app (…/target/release/bundle/macos/Hickeyfield.app)
    Finished 1 bundle at:
        …/target/release/bundle/macos/Hickeyfield.app
    [exited with code 0]
```

`first_light.rs` was never run; no hosted LLM or fal was ever called with a real key (none exists);
no money was spent. The `.app` was built, not installed and not opened (no automated screenshot tool
exists — the VERIFY agent launches it and captures the Storyboard view, as in S12).

## Design §10 — the three weakest points, addressed

1. **The JSON contract is an LLM's free text (§10.1).** Mitigated by reusing `clean_reply`
   (fence/quote unwrap) then strict serde, with `refusal_reason` run *first* so a decline is
   `Refused`, not `NotJson`. Non-vacuity is real: `a_refusal_is_a_typed_error_not_garbage` proves the
   S6 pre-check fires (dropping it would reclassify it as `NotJson`); `garbage_that_is_not_json_is_rejected`
   asserts `NotJson` carries a non-empty cause; `the_shot_count_bound_is_three_to_eight` pins the
   `3..=8` bound. **Honest gap:** I did not run a real Ollama against the corpus (no daemon in this
   environment), so the corpus's real-world hit rate is a claim, not a measurement — the verifier's
   local Ollama run is the honest confidence check the design asked for.
2. **`submit_one` factoring (§10.2).** Addressed by the line-for-line diff above; the body is
   verbatim. **Honest gap:** the *successful* submit path is not exercised by any new unit test,
   because a real submit needs a paid provider. The offline generate test uses only unknown model
   ids so it fails before `submit_one` — this cannot spend money even if a key sits in the machine's
   keychain (an earlier draft that used a real model id risked exactly that and was removed). The
   happy path relies on the unchanged existing single-job wiring, which the diff proves is untouched.
3. **Write-once columns + the `job_sets`/`storyboards` name split (§10.3).**
   `migration_8_adds_storyboards_and_columns_and_roundtrips` asserts `user_version == 8`, the plan-JSON
   round-trip, that a tagged job round-trips its set id/index, that a standalone job loads `None`, and
   the write-once contract (a later upsert with a different set id does **not** re-tag, while `status`
   still updates). Adding `storyboard_id` to the `ON CONFLICT UPDATE SET` would fail that assertion.

Design §10.4 (catalogue-sourced clamp is best-effort) and §10.5 (total-before-submit gate lives in the
UI) are honoured as written: the clamp only ever narrows toward an offered value and always advises,
and Generate-all is disabled until the summed total renders — with the caveat that `generate_storyboard`
itself does not re-enforce "total shown first" (its only caller is the Storyboard view).

## Known limitations not fixed (not hidden)
- No rendering/stitching/last-frame extraction — S14 (N1). `compositor.rs`/`ffmpeg.rs` untouched.
- Continuity is recorded, never applied (N2/D6); nothing binds an `end_image_url`.
- No hosted JSON/structured-output mode (N3); the plan is parsed from text.
- `store::storyboard` (singular) and `store::jobs_in_storyboard` are the store surface for S14 and the
  tests; nothing in S13 production calls them, so both carry `#[allow(dead_code)]` with a note.
- The successful generate-all path (real job ids) is not unit-tested offline; see §10.2 gap above.

---

## REPAIR ROUND 1

Fresh implementer, repair round 1 of at most 2. Fixed every item in review.md
(B1, M1, m1/m2/m3) and security.md (L1–L3). Working tree only, no commit. Full
gate re-run and pasted at the end.

### B1 (money, blocker) — non-unique job ids collapsed same-model shots
`src-tauri/src/commands.rs`.
- Added a process-wide monotonic counter and two minting helpers:
  `static ID_SEQ: AtomicU64`, `next_id_seq()`, `mint_job_id(model_id)`
  (`= "{now_secs}-{model_id}-{seq}"`) and `mint_storyboard_id()`
  (`= "sb-{now_secs}-{seq}"`). The counter is the disambiguator the review asked
  for; no new dependency (std `AtomicU64`).
- `submit_one` now mints via `mint_job_id(&model.id)` (was
  `format!("{}-{}", now_secs(), model.id)`), so **both** the generate-all loop
  and the single-job path get collision-proof ids. Outward id shape changes only
  by a trailing `-{seq}` segment; grep confirmed **nothing parses the job id**
  (`split('-')` appears nowhere; only `route_id.split(':')` is parsed, untouched).
- `generate_storyboard_inner` mints the storyboard id via `mint_storyboard_id()`
  (fixes m1/L2 with the same counter).
- Tests (`commands.rs`): `minted_job_ids_do_not_collide_within_one_second`
  (200 same-model ids all distinct; two immediate single-job mints differ; two
  storyboard ids differ) and `same_model_shots_survive_as_distinct_rows_in_the_store`
  (three same-model shots minted as `submit_one` mints them, tagged to one
  storyboard, upserted, then read back via `jobs_in_storyboard` — three rows,
  dense `shot_index [0,1,2]`, none clobbered by `ON CONFLICT`).
  - Honest note on why this is a store-write test and not a live loop test: the
    router (`route::resolve`) only ever yields fal/higgsfield routes (the only
    providers with `has_adapter()`), and reaching them needs a configured key
    that then makes `submit_to_provider` POST over the network. So the loop's
    own submit cannot run offline without a network call. The collision itself,
    however, lives entirely in id minting + the store's first upsert, which the
    two tests drive with the real production `mint_job_id` and the real store.

### M1 (major) — UI swallowed per-shot failures
`ui/src/components/Storyboard.tsx`.
- Added `outcome` state. `generate()` now inspects `result.shots`: on **full
  success** it clears the outcome and calls `onGenerated(result)` (App switches
  to Library, unchanged); on **partial failure** it keeps the result and does
  NOT hand off, so the failed shots stay editable in their cards.
- New outcome summary (`.storyboard-outcome`, `role="alert"`): "n of N shots
  submitted; m failed", a `.storyboard-outcome-fail` line per failed shot
  carrying its `shot_index` and error text, the retry total
  (`totalCostCaption` of just the failed shots — money shown first), and a
  `.storyboard-retry` button.
- `retryFailed()` resubmits ONLY the failed shots through the same money-gated
  `submitBatch` (estimate + S6 guard + S7 verbatim prompt per shot), remaps the
  retried `shotIndex` back onto the original card numbering, merges, and hands
  off to the Library only once every shot has started.
- `submitBatch(indices)` factors the per-shot `SubmitInput` build so generate
  and retry share one path. `App.tsx` `onStoryboardGenerated` is now only ever
  called on full success, so switching to Library there is correct.
- vitest (`Storyboard.test.tsx`): `a full success switches to the Library`
  (onGenerated once, no summary) and `a partial failure shows the failed shots
  and retry submits only those` (summary + error text rendered, onGenerated not
  called; retry calls onGenerate a 2nd time with exactly 1 shot; on success the
  summary clears and onGenerated fires).

### Review minors
- **m1** storyboard id collision — fixed by `mint_storyboard_id()` (above).
- **m2** stale preview on reorder/delete — added `closePreview()` (resets
  `previewShot`/`preview`/`previewText`) and call it from `moveShot`,
  `removeShot`, and `draft`. vitest `reordering closes an open prompt preview`
  opens a shot's inline `PromptPreview`, moves the shot, and asserts the
  `.prompt-preview-text` panel is gone.
- **m3** cleared target length — a 0/blank target now renders
  `.storyboard-target-invalid` (`role="alert"`) and disables Draft, instead of
  silently guaranteeing a `DurationSum` rejection. vitest `a cleared target
  length shows a validation message and blocks drafting`.

### Security L1–L3 — server-side bounds in generate_storyboard
`src-tauri/src/commands.rs` `generate_storyboard_inner`.
- **L1** — before minting anything: reject `input.shots.len() > 8` (const
  `MAX_SHOTS`), reject an empty shot list, and — when `plan_json` is present —
  parse it and re-run `hickeyfield_core::storyboard::validate(&plan.shots,
  plan.target_seconds, use_case)` (the same validator the draft path uses), then
  require the submitted shot list to match the plan (same length, same per-index
  `model_id`) so no shot is injected beyond the plan the user saw and priced.
- **L3** — cap `brief` at 4 KiB (`MAX_BRIEF_BYTES`) and `plan_json` at 64 KiB
  (`MAX_PLAN_JSON_BYTES`) with typed `Err(String)` messages at the boundary,
  before the DB write.
- Tests: `generate_rejects_more_than_eight_shots`,
  `generate_rejects_a_plan_with_a_bad_model` (asserts nothing was written),
  `generate_rejects_shots_that_do_not_match_the_plan`,
  `generate_rejects_a_brief_over_the_size_cap`,
  `generate_rejects_a_plan_json_over_the_size_cap`. The pre-existing
  `generate_records_the_set_even_when_shots_fail` was switched to `plan_json:
  None` (it exercises the per-shot continue-on-failure path, which is distinct
  from the new pre-loop plan re-validation).
- Added `#[derive(Debug)]` to `ShotOutcome`/`StoryboardResult` so the new
  `unwrap_err()` tests compile.

### Gate — every command, observed output
```
cargo fmt --all --check                         → FMT_CLEAN (exit 0)
cargo clippy --workspace --all-targets -- -D warnings → Finished, 0 warnings
cargo test --workspace                          → 704 + 139 + 3 + 2 = 848 passed, 0 failed, 12 ignored (>= 841)
./scripts/lint-provenance.py                    → CDN PASS, Copy provenance PASS (80015 shingles)
env -u hickeyfield_FAL_KEY -u FAL_KEY cargo run -q -p hickeyfield-core --example audit_fal
                                                → PINNED 0, UNREACHABLE 0 (unchanged)
cd ui && pnpm test                              → 23 files, 285 passed (>= 281)
cd ui && pnpm build                             → tsc + vite ✓ built (dist emitted)
```
`tauri build` (8-min bundle) was NOT re-run this round — no native/tooling change,
only Rust command logic + TS view logic + CSS; the S12 bundle path is unchanged.
`first_light.rs` never run; no hosted LLM or fal call with a key; no money spent.

### Known limitations not fixed (not hidden)
- **Retry creates a new storyboard group.** A "Retry failed" batch goes through
  `generate_storyboard` again and mints a fresh `storyboard_id`, so retried
  shots land in a separate Library group from the first batch's successes. The
  merged in-view outcome is correct, but the two groups are not reunited under
  one header. Reuniting them needs a "generate into an existing set" command,
  which is out of this slice's scope.
- **The successful generate-all loop remains not unit-tested offline** (the
  router-only-yields-fal/higgsfield constraint above). The id-minting and store
  writes it depends on are now tested with the real production helpers; the live
  submit itself is still covered only by a real run.
- Everything from the original claim's limitations list still stands (no
  render/stitch/last-frame — S14; continuity recorded not applied; no hosted
  JSON mode; `store::storyboard`/`jobs_in_storyboard` are the S14 surface).

---

## REPAIR ROUND 2

Repair round 2 of 2 (the last). Fixed the re-review's **M2** (retry-failed rejected
by the same-commit server re-validation) properly, and closed security **L4** (priced
duration/route not pinned to the plan) since it was under ~20 lines. Working tree only,
no commit. Full gate re-run and pasted at the end.

### M2 (Major, AC6 retry path) — retry now sends the FULL plan + a subset of indices

The bug: repair-1's `retryFailed`/`submitBatch` built `plan_json` from ONLY the failed
subset (with the full `targetSeconds`), so `generate_storyboard_inner`'s round-1
re-validation re-ran `storyboard::validate` on that shrunken list and rejected any retry
of fewer than 3 shots (`ShotCount`) or with a subset sum outside ±10% (`DurationSum`).
The vitest passed only because the old `vi.fn` ignored `planJson`.

Fix — server (`src-tauri/src/commands.rs`, `StoryboardGenerateInput` + `generate_storyboard_inner`):
- Added two optional fields to the command input:
  - `shot_indices: Option<Vec<u32>>` — the plan index each submitted shot maps to.
    `None` means "the shots ARE the whole plan, in order" (the first generate-all, exactly
    as before). `Some` (a retry) carries the original indices of the failed shots.
  - `storyboard_id: Option<String>` — an existing group to write a retry into, so retried
    shots join the ORIGINAL storyboard set instead of a fresh group (closes the round-1
    "separate group" gap).
- The re-validation now runs `storyboard::validate` on the **FULL** plan carried in
  `plan_json` (still 3..=8 shots, ±10% sum), then matches **each submitted shot to its plan
  index** (`indices` maps submitted shot k → plan slot `idx`), checking `idx` is in range and
  the model matches the plan slot. Only that subset is submitted. A sub-3-shot retry is no
  longer re-count/re-sum-gated — the full validated plan is the authority.
- The first-generate contract is unchanged: with `shot_indices == None` the old length-match
  ("does not match the plan") check still fires, so no shot is injected beyond a 1:1 plan.
- Group id: a retry (`storyboard_id: Some`) verifies the group exists and writes its new job
  rows into it (same `shot_index`); a first generate mints a fresh group as before. A retry
  that names an index whose row **already started** (a non-`Failed` job at that index) is
  rejected before any submit — a successful shot is never resubmitted / double-billed.

Fix — UI (`ui/src/components/Storyboard.tsx`, `submitBatch`/`generate`/`retryFailed`):
- `submitBatch(indices, retryStoryboardId?)` now always sends the **full** plan in `planJson`
  (`shots`, not the subset). On the first generate-all it sends no `shotIndices`/`storyboardId`
  (positional, as before). On a retry it passes `retryStoryboardId`, which makes it send
  `shotIndices = indices` and the existing `storyboardId`.
- `retryFailed` resubmits only the failed indices into `outcome.storyboardId`; the server now
  returns the real plan indices, so the old 0-based re-mapping was removed.
- `api.ts generateStoryboard` and `mock.ts mockGenerateStoryboard` carry/honour the two new
  optional fields (the mock now keys outcomes off `shotIndices` and reuses `storyboardId`).

Tests:
- Rust (`commands.rs`):
  - `a_retry_with_the_full_plan_targets_the_existing_group_and_only_the_failed_index` — a
    4-shot `kling3_0` plan where shot 2 failed; retry with the FULL plan + `shot_indices=[2]`
    + the existing `storyboard_id` is **accepted**, mints no new group (`out.storyboard_id ==
    sid`), submits exactly one shot at `shot_index 2`, and leaves the 3 successful rows
    untouched. The single submit is driven **offline** by pinning `kling3_0`'s Vaig route
    (belongs to the model so the route-pin passes, but has no adapter → `route::resolve`
    `PinUnusable`, never a network POST).
  - `a_retry_of_an_already_started_shot_is_rejected` — a retry naming an index whose row is
    `Queued` is refused ("already started") before any submit.
  - `a_retry_subset_plan_is_rejected_with_a_clear_error` — a 1-shot subset plan (the old bug)
    is rejected with the honest count error ("3 to 8 shots"), and nothing is written.
  - `a_retry_shot_whose_duration_does_not_match_the_plan_is_rejected` — the L4 duration pin.
- UI (`Storyboard.test.tsx`, "a partial failure shows the failed shots and retry submits only
  those") — the `onGenerate` mock is now **server-faithful**: it parses `planJson`, enforces
  the 3..=8 count rule, and honours `shotIndices`/`storyboardId`. So a subset plan would throw
  (the mock can no longer mask the integration gap). The test asserts the subset retry total is
  shown before submit, and that the retry call passes a **4-shot** `planJson`, `shotIndices ==
  [2]`, and `storyboardId == "sb1"`.

### Security L4 (Low, residual of L1) — priced duration/route pinned to the plan (~15 lines)

In the plan re-validation loop, each submitted shot's `settings.duration` must equal the
plan's clamped `seconds` for its index (±0), and any pinned `route_id` must belong to the
plan model — else a typed `Err(String)` before any submit. So a direct/tampered caller can no
longer pass `validate` with cheap in-catalogue durations while billing an inflated magnitude.
Since the UI rebuilds `plan_json` from the current shots, a legitimate user's edits still pass
(plan seconds == submitted seconds by construction). Tested by
`a_retry_shot_whose_duration_does_not_match_the_plan_is_rejected`.

### Money / invariants held
`submit_one` is still byte-for-byte the audited single-job body (unchanged this round). Every
shot — generate and retry — goes through it (S6 guard + S7 verbatim `final_prompt` + S10
estimate + `route::resolve`). The UI shows the subset total before a retry. No hosted LLM/fal
call with a key; `first_light.rs` never run; no money spent. Test-locked class names intact
(only additive assertions). Provenance passes.

### Gate — every command, observed output
```
cargo fmt --all --check                         → FMT_CLEAN (exit 0)
cargo clippy --workspace --all-targets -- -D warnings → Finished, 0 warnings
cargo test --workspace (8 dummy keys + FAL_KEY=x)
  core lib 704 ; live_fal_pricing 0/1 ign ; audit_fal 3 ; fal_diff 2 ; shell lib 143
  → 704 + 3 + 2 + 143 = 852 passed, 0 failed (>= 848)
./scripts/lint-provenance.py                    → CDN PASS, Copy provenance PASS (80015 shingles)
env -u hickeyfield_FAL_KEY -u FAL_KEY cargo run -q -p hickeyfield-core --example audit_fal
                                                → PINNED 0, UNREACHABLE 0 (unchanged)
cd ui && pnpm test                              → 23 files, 285 passed (>= 285)
cd ui && pnpm build                             → tsc + vite ✓ built (dist emitted)
```
`tauri build` (8-min bundle) NOT re-run — no native/tooling change this round (Rust command
logic + TS view logic + one CHANGELOG line only); the S12 bundle path is unchanged.

### Known limitations not fixed (not hidden)
- **The successful generate/retry submit loop is still not unit-tested against a live
  provider** (a real submit needs a paid POST agents never make). The new retry test drives
  the retry to `submit_one`'s route-resolve stage offline via the Vaig-pin, proving the
  routing/targeting decision (correct group, correct single index, reached submit) but not a
  provider-accepted job row. The store-level write-once/ordering is covered separately by the
  existing `same_model_shots_survive_as_distinct_rows_in_the_store` and migration-8 tests.
- Everything from the round-1 limitations list still stands **except** the retry-into-a-new-
  group gap, which this round closes (a retry now writes into the original `storyboard_id`).
- Still S14's job: rendering/stitching/last-frame extraction; continuity recorded, not applied;
  no hosted JSON/structured-output mode.
