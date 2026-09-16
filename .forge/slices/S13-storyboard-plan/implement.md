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
