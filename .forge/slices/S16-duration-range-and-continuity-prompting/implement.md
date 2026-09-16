# S16-duration-range-and-continuity-prompting — IMPLEMENT (claim)

Author: implementer agent. Built exactly the S16 slice per `slice.md` (AC0–AC10) and
`design.md` (D1–D10). Every file changed, the exact commands that prove each AC with observed
output, and the known limitations I did NOT hide.

## Baseline (measured before any change, on a clean `git stash`)

`cargo test --workspace` → **888 passed, 0 failed** (725 tauri-lib + 156 core + 5 + 2, ignored 12+1+1).
UI baseline (stated by the launcher, re-measured after): **292** vitest.

## Result totals (after the slice)

- `cargo test --workspace` → **919 passed, 0 failed** (+31 net new tests).
- `cargo test -p hickeyfield-core --lib ffmpeg -- --ignored` → **7 passed, 0 failed** (S15's
  probe/last_frame/render + my `identical_frames_score_near_one`,
  `a_brightness_shift_fails_the_colour_gate`, `a_different_source_falls_below_the_ssim_floor`,
  `first_frame_writes_the_head_frame`).
- `cargo fmt --all --check` → clean (exit 0).
- `cargo clippy --workspace --all-targets -- -D warnings` → clean.
- `./scripts/lint-provenance.py` → PASS (v2 prompt indexed, no vendor copy).
- keyless `audit_fal` → **RANGE MISMATCH (0)**, CHECKED (70), no regression in checks 1–6.
- `cd ui && pnpm test` → **301 passed** (+9). `pnpm build` → built.
- `ui/node_modules/.bin/tauri build --bundles app` (from repo root) → exit 0:
  ```
      Finished `release` profile [optimized] target(s) in 4m 22s
         Built application at: /Users/jorge/Documents/GitHub/hickeyfield/target/release/hickeyfield-tauri
      Bundling Hickeyfield.app (…/target/release/bundle/macos/Hickeyfield.app)
      Finished 1 bundle at:
          /Users/jorge/Documents/GitHub/hickeyfield/target/release/bundle/macos/Hickeyfield.app
  ```
  The `.app` exists on disk. I did NOT open or launch it (the verifier launches it for the
  window/screenshot check, per the gate).

---

## What each AC delivers, with the proving command

### AC1 — schema records numeric ranges (`fal_schema.rs`)
`EndpointSchema.ranges: BTreeMap<String,(f64,f64)>` (serde-default). `parse` reads
`minimum`/`maximum` directly and — the **one** place `anyOf`/`oneOf` is now descended — from
the first member carrying both. `const`/enum fields carry no range (never collide with the
enum read). `Eq` dropped from `EndpointSchema` (an `(f64,f64)` map is not `Eq`).
Tests: `a_nullable_integer_range_is_recorded_from_anyof`, `a_direct_integer_min_max_is_recorded`,
`a_field_with_no_bound_has_no_range`; the S9 `a_nullable_integer_is_not_enumerated` still holds.
```
cargo test -p hickeyfield-core --lib fal_schema  → 47 passed, 0 failed
```

### AC2 — capability carries a range, live-first (`capability.rs`, `catalog.rs`, `registry.rs`)
`Axis.range: Option<(f64,f64)>` (serde-default). `ModelSpec.duration_range` (spec-level, NOT a
`ValueSpec::Range` — Q1) → `Capabilities.duration_range` → `fal_axis` sets `range =
schema.ranges.get(field).or(registry_range)` (live wins), `from_catalog` from the registry.
`duration_range=(5,15)` on `minimax_h3_max`/`_turbo`, `(2,30)` on `wan3_0_prime` (Q2). `Eq`
dropped from `Axis`/`ModelCapability`. `ModelCapability::duration_range()` accessor.
Tests: `an_axis_range_comes_from_the_live_schema_first`, `an_unpublished_range_falls_back_to_the_registry`.

### AC3 — UI ladder from the range (`ui/src/lib/variants.ts`, `types.ts`, `api.ts`)
`durationLadder`: enum → **range** → generic → none. `ladderFromRange([min,max],default)`:
integers `ceil(min)..=floor(max)` thinned to ≤12 chips, endpoints + default always kept, never
outside the bound. `resolveSettings` clamps a free-form duration into the range on model
switch. `CapabilitiesDto.duration_range` (snake wire, mapped to `durationRange` in `api.ts`).
Tests (`variants.test.ts`): `offers fifteen and is not capped at twelve`, `covers the full range
thinned`, `keeps the generic ladder when no range`, plus two `resolveSettings` clamp tests.
```
cd ui && pnpm exec vitest run src/lib/__tests__/variants.test.ts  → 26 passed
```

### AC4 — clamp-with-advisory at submit (`src-tauri/src/commands.rs`)
`clamp_duration_to_range(&mut settings, model.spec.duration_range, name)` runs in `submit_one`
BEFORE `Billable::from`, so the estimate prices the clamped value. Advisory
`"{name} runs {min}–{max}s; {d}s clamped to {c}s"`. Range read offline from the spec (Q3;
guarded by AC6). Tests: `a_duration_outside_the_range_is_clamped_with_an_advisory` (20→15,
estimate on 15s), `a_duration_inside_the_range_is_untouched`.

### AC5 — storyboard clamp + fill-to-ceiling (`storyboard.rs`)
`clamp_shot` gains a free-form range clamp (12 on H3 Max stays 12; 20→15 with advisory).
`model_ceiling` (free-form-range models only; an enumerated axis is still snapped) feeds
`validate` (`ShotOverCeiling` rejects a shot over the ceiling) and `build_user_message` (hands
each model's ceiling to the planner). Tests: `clamp_shot_uses_the_range_for_a_free_form_model`,
`a_single_shot_thirty_survives_only_when_the_range_reaches_it`, `fill_to_ceiling_splits_thirty_into_two_fifteens`.

### AC6 — audit range check (`examples/audit_fal.rs`, D10)
Check 7 `range_conflicts(spec, schema)`: fires only when fal publishes a duration range AND the
spec declares one, reporting a spec range wider than the endpoint's. Test
`range_conflicts_flags_a_spec_range_outside_the_schema`. Keyless run: **RANGE MISMATCH (0)**,
70 endpoints checked, checks 1–6 unchanged (H3 Max publishes no range → skipped; Wan 3.0 Prime's
(2,30) matches).

### AC8 — world block + validator (`storyboard.rs`, `prompts/storyboard.v2.md`, `corpus.rs`)
`StoryboardPlan.world`, `Shot.action`/`resolution`, `Shot.prompt` now serde-default. `plan`
composes `prompt = "{world} {action}"` verbatim, then `validate_continuity` + `validate_composition`.
New `PlanError`: `MissingWorld`, `PromptMissingWorld`, `IdenticalShots`, `NoHoldEnding`,
`CutWords`, `ShotOverCeiling`; `MixedResolution` is an advisory (`mixed_resolution_advisory`),
not a rejection, per D6. Prompt bumped to `storyboard.v2.md` (fixes the stale "3 and 8 shots"
→ "1 to 8"). The `1..=8` floor is preserved. Tests: `a_world_block_is_composed_verbatim_into_each_prompt`,
`identical_actions_are_rejected`, `a_non_final_shot_without_a_hold_is_rejected`,
`a_short_world_is_rejected`, `a_cut_word_is_rejected`, `a_broken_composition_is_caught`,
`the_one_to_eight_bound_is_preserved`, `a_mixed_resolution_set_is_advised_not_rejected`.

**Deviation (recorded):** continuity rules are **opt-in via a non-empty `world`**. A stored
pre-S16 / montage plan carries no world (the field is serde-default) and must still validate,
so `validate_continuity` gates the hold/identical/cut-word/composition rules on
`!world.is_empty()`. `MissingWorld` fires on a *present-but-thin* (<5-word) world in a
multi-shot plan. Reason: backward-compat with stored S13 plans (a hard rule). The v2 prompt
always emits a world for a continuous run, so this does not weaken the drafted path.

### AC9 — per-join score (`ffmpeg.rs`, `storyboard.rs`, `store.rs`, `chain.rs`, `render.rs`, UI)
- **Metric (`ffmpeg.rs`):** `first_frame`/`first_frame_args` (last-frame shape minus `-sseof`,
  `-frames:v 1`); `continuity_score` → `ContinuityMetrics{ssim,psnr,dy,du,dv}` via a new
  `run_capture` (the `run_probe` hardening skeleton, requires success). `ssim_psnr_args` uses
  `scale2ref` to normalise B to A's size (a mutation dropping it fails the reencode case).
  `parse_ssim`/`parse_psnr`/`parse_signalstats` key off `All:`/`average:`/`signalstats.*AVG=`.
- **Verified against the ACTUAL bundled sidecar** (`ffmpeg-aarch64-apple-darwin`, captured
  2026-09-16): the fixtures `REAL_SSIM_PSNR`/`REAL_SIGNALSTATS` in
  `parse_ssim_and_signalstats_read_the_captured_lines` are the real output text (SSIM/PSNR are
  on RGB planes for a PNG; the aggregate is `All:`, which is why the colour gate keys off it,
  not `Y:`).
- **Thresholds** are named constants in one place (`storyboard.rs`): `CONTINUITY_FLOOR
  {ssim:0.80,y:12,c:6}`, `CONTINUITY_IDEAL {0.90,6,3}`. `ContinuityThresholds`, `JoinScore`
  (all `Option<f64>` — unscored is `None`, never `0.0`), `join_passes` (unscored → not
  failing), `continuity_rate` (over scored joins; 19/20 ⇒ 0.95). Tests:
  `join_passes_at_the_four_boundaries`, `continuity_rate_over_a_fake_store`.
- **`--ignored` real-sidecar (synth lavfi, no money):** identical frames ≈1.0 pass;
  `eq=brightness=0.1` → dy over the floor with ssim high (non-vacuous colour gate);
  `mandelbrot` vs `testsrc` → ssim below floor. All pass on this Mac.
- **Store (`store.rs`):** `JoinRecord`, `parse_joins`, `upsert_join`; migration **10** adds a
  nullable `auto_retry` column (NOT the AC0-H4 unique index — Q7). Test
  `joins_and_the_auto_retry_toggle_round_trip`; `user_version` asserts updated to 10.
- **Wiring (`chain.rs`/`app.rs`):** `score_join` in the settled hook (beside `advance`) extracts
  the settled shot's first frame, scores against `frames/shot{i-1}.png`, writes `joins_json[i]`;
  a scoring failure records unscored (`a_scoring_failure_is_none_never_zero`).
- **Render (`render.rs`, D8):** per-join kind read from `joins_json` (hard cut vs crossfade),
  falling back to the media-presence heuristic only when no scored join exists.
- **Regenerate (`commands.rs`):** the S13 retry guard is relaxed to allow a **Completed** shot
  whose join `pass == false` (gated on `joins_json`), keeping a passing/running shot refused.
  Test `a_below_floor_join_may_be_regenerated_but_a_passing_one_may_not`.
- **UI:** `.storyboard-join` badge in the Library set item (min scored ssim, `data-pass`),
  `JoinRecord` type, `list_storyboards` now carries `joins_json`/`auto_retry`. Tests:
  `shows the set's minimum continuity-join score and flags a low join`, `shows no join badge
  for an unscored set`, and the `api.test.ts` join-parse case.

### AC10 — bounded auto-retry (`chain.rs`, `commands.rs`, UI)
`AUTO_RETRY_CAP = 2`; `should_auto_retry(rec, enabled)` = enabled && `pass==false` &&
`attempt < cap`. `maybe_auto_retry` (settled hook) bumps the attempt in `joins_json`, then
regenerates via `submit_one` (`seed + attempt`, same frame) — `submit_one` prices the row
before it spends, so nothing spends without the total shown. Off by default
(`StoryboardGenerateInput.auto_retry`, serde-default false; `Storyboard.auto_retry` column).
UI toggle `.storyboard-autoretry-toggle` (shown only when chaining). Tests:
`auto_retry_stops_after_two_attempts` (FakeScorer 0.6/0.6/0.95 → exactly 2, off → 0),
`the auto-retry toggle appears only when chaining, and rides the payload`.

### AC0 hygiene
- **H1 (partially-chained fallback join):** fixed by D8 (render reads `joins_json` per join).
- **H2 (`chainable_model` tightening): ATTEMPTED, REVERTED, RECORDED.** The per-route
  `can_bind(Start)` check is LOOSER than the coarse `use_case::supports` on `Dialect::Fal`,
  because `can_bind` is deliberately permissive for an unmeasured endpoint (the schema gate is
  the backstop). It reported `gemini_omni` — a measured media-ignoring model — as chainable, a
  regression the design's §10.4 escape hatch says to revert on. `chainable_model` is the coarse
  `use_case::supports(model, ImageToVideo)` (keeps H3 Max true, excludes gemini_omni). Test
  `chainable_model_keeps_h3_max_and_excludes_media_ignorers` sweeps the whole roster asserting
  agreement with the picker authority.
- **H3 (vacuous failed-guard test):** `a_completed_predecessor_would_advance_so_the_failed_guard_bites`
  proves the guard by an observable that precedes any submit (the set's `frames/` dir is created
  only past the `Phase::Completed` guard) — non-vacuous without needing a configured provider.
- **H4 (unique index):** NOT taken (Q7) — it would make the retry/regenerate-at-existing-index
  illegal. The read-guard is kept.

---

## Design §10 weakest points, addressed with evidence

1. **Thresholds 0.80/12/6 are named constants; synth `--ignored` tests prove the machinery.**
   `CONTINUITY_FLOOR`/`CONTINUITY_IDEAL` in `storyboard.rs`. The `--ignored` cases prove
   identical frames ≈1.0 pass and a brightness shift fails the colour gate (non-vacuous) and a
   different source fails the SSIM floor — all pass on this Mac. **Calibration on a real paid
   2×15 set is human-run (Q6) — the two numbers are reasoned, not measured.**
2. **`parse_ssim`/`parse_signalstats` verified against the ACTUAL bundled sidecar output.** The
   captured real text is the fixture in `parse_ssim_and_signalstats_read_the_captured_lines`;
   the `--ignored` tests run the real binary end-to-end.
3. **Submit-time clamp reads the offline (registry) range on the money path.** The D10 audit
   check is implemented and run keyless: RANGE MISMATCH (0), so no landed model's documented
   range exceeds fal's published range.
4. **`chainable_model` tightening kept H3 Max chainable — but regressed gemini_omni, so it was
   reverted (H2 above), keeping H3 Max true and the media-ignorer excluded.**
5. **Regenerate relaxation is gated on the `joins_json` floor verdict** and the subset total is
   shown by `submit_one` before spending; a passing/running shot is still refused (tested).

## Money / safety invariants held
- Nothing submits without the total shown: every regenerate/auto-retry goes through
  `submit_one`, which writes `estimated_usd` on the row before contacting the provider.
- Scoring and rendering spend nothing (local ffmpeg on saved PNGs; no network).
- Auto-retry ≤2 and off by default. Every ffmpeg spawn uses the S10 sandbox skeleton
  (`run_capture`/`run_to_file`: stdin closed, `-protocol_whitelist file` before `-i`, deadline
  + kill, bounded capture). No paid job submitted; `first_light.rs` never touched; no key exists.
- H3 Max range 5–15 and Wan 2–30 reach the chip row (vitest), the ladder never emits outside the
  range, unknown score → `None` never 0. `prompts/storyboard.v2.md` passes provenance.
  Test-locked class names intact; new classes `.storyboard-join`,
  `.storyboard-regenerate-join`, `.storyboard-autoretry-toggle` are additive.

## Known limitations / not done (not hidden)
- **World-block editor UI not added.** The world is composed server-side and visible in each
  shot's composed prompt (which the editor already shows); a dedicated world text field and the
  per-shot "Regenerate from frame" button on the Storyboard shot card are NOT surfaced. The
  Library set-item min-join badge IS shown, and the regenerate money-path (guard relaxation) is
  implemented and tested; the per-shot button and world editor are deferred UI polish. This is a
  partial delivery of the AC9/AC8 UI surface — the backend, scoring, storage, render, regenerate
  path, auto-retry, and Library badge are complete.
- **`MixedResolution`** fires only when shots carry an explicit per-shot `resolution` (a new
  serde-default field); the common case is one set-wide resolution, so it is usually silent —
  it is an advisory by design (D6), not a rejection.
- **Thresholds are reasoned, not measured** (Q6, N2) — the calibration commit against the first
  real paid set is human-run.
- Two unrelated `--ignored` tests fail in this offline/keyless environment
  (`a_local_rewrite_completes_against_a_real_daemon` needs Ollama; `live_feeds_answer...` needs
  the live pricing network). They are pre-existing and outside this slice; all **ffmpeg sidecar**
  `--ignored` tests pass.

---

## REPAIR ROUND 1

Surgical fixes for review findings M1 (dormant Major, money) plus minors m1/m2/m3/m4/m5.
Every gate re-run below with the dummy-key env and Node 24 (results at the end).

### M1 — regenerate/auto-retry admit an in-flight below-floor shot (double-bill)
`src-tauri/src/commands.rs` — the regenerate guard (`generate_storyboard_inner`, loop at
`:1557`) keyed only on the stored `joins_json` `pass == Some(false)`. Between submitting a
regenerate and its settle the join still reads `pass == false` while a Queued/Running job
already exists at that index, so a second regenerate was admitted — a duplicate in-flight job.
- New shared helper `shot_in_flight(jobs, idx)` (`commands.rs:1086`): true iff a job at that
  index is **non-terminal** (`!JobStatus::is_terminal()`), i.e. queued or running.
- Guard restructured so the below-floor relaxation only fires for a **settled** low-join shot:
  a started shot not in `below_floor` still errors `already started` (message preserved, so the
  existing `a_retry_of_an_already_started_shot_is_rejected` stays green); a below-floor index
  with an in-flight job now errors `still in flight` before it can resubmit.
- **Auto-retry uses the same guard.** `src-tauri/src/chain.rs:408` (`maybe_auto_retry`): after
  loading the set it calls `crate::commands::shot_in_flight(&jobs, i)` and returns without
  submitting when a job at that index is still in flight (the attempt-bump still bounds the loop
  across settles).
- Tests (`commands.rs`): `a_below_floor_join_that_is_still_in_flight_is_refused` — Queued → refused
  (`still in flight`), InProgress → refused (second regenerate while running), Completed below-floor
  → admitted once (no in-flight/already-started refusal); `shot_in_flight_reads_only_non_terminal_rows`
  — queued/running are in flight, Completed/Failed are not, a terminal row does not mask an in-flight
  one at the same index. The pre-existing `a_below_floor_join_may_be_regenerated_but_a_passing_one_may_not`
  and `a_retry_of_an_already_started_shot_is_rejected` remain green.

### m5 — score scratch frame not cleaned up
`src-tauri/src/chain.rs:319` (`score_join`): the `first{i}.png` scratch frame the score reads is
now `std::fs::remove_file`'d after `first_frame`+`continuity_score` run (Ok or Err), while the
`shot{i-1}.png` continuity frame the chain/render need is left in place.
- Test (`--ignored`, real sidecar): `score_join_cleans_up_the_scratch_first_frame` synthesises a
  prev frame + clip, scores the join, and asserts the frames dir has **no** `first*.png` while
  `shot0.png` survives. Passes with the bundled `ffmpeg-aarch64-apple-darwin`.

### m1 — ladder could emit 13 chips
`ui/src/lib/variants.ts:66` (`ladderFromRange`): rewritten so the endpoints and the default (when
in range) are mandatory and **consume from the 12-chip budget first**, then an even stride fills the
remainder only while `size < MAX_CHIPS`. `<= 12` is now guaranteed by construction, deterministic,
and always includes min/max/default. H3 Max `[5,15]` (unthinned) and Wan `[2,30]` unchanged.
- vitest: `never emits more than twelve chips when the default is not a stride pick` — range
  `[1,100]` default `6` (old code produced 13); asserts `<= 12`, endpoints + default present, all
  in range, and determinism (same inputs → same ladder).

### m2/m3 — cut-word/hold matching is a raw substring
`crates/hickeyfield-core/src/storyboard.rs`: new `contains_word(hay, needle)` (`:569`) matches a
whole word/phrase (bounded by a non-alphanumeric char or a string edge), used for `CUT_WORDS`
(`:624`); new `ends_on_hold(action)` (`:595`) accepts whole-word `hold`/`holds` (so "static hold",
"holds on", "comes to a hold" all pass) at the `NoHoldEnding` check (`:643`).
- Tests: `a_cut_word_matches_only_whole_words` (`later` fires in prose but not inside `slater`;
  `elsewhere` not inside `elsewheres`; `meanwhile`/`cut to` still fire), `hold_ending_accepts_several_phrasings`
  (`household` does NOT count), `a_holds_on_ending_passes` (a "holds on" ending plans clean). The
  existing `a_cut_word_is_rejected` / `a_non_final_shot_without_a_hold_is_rejected` remain green.

### m4 — doc overstatement
`CHANGELOG.md:12` and `README.md:75`: "every free-form-duration model its actual ceiling" / "each
model's real range" → "every model whose provider publishes or documents a range its real ceiling
(models that publish neither still use the generic ladder)". Provenance still PASS.

### Repair-round gate (this machine, dummy-key env, Node 24)
- `cargo fmt --all --check` → clean (`FMT_OK`).
- `cargo test --workspace` → **924 passed, 0 failed** (749 core + 6 + 2 + 167 tauri; 16+1+... ignored).
- `cargo clippy --workspace --all-targets -- -D warnings` → clean.
- `cargo test -p hickeyfield-core --lib ffmpeg -- --ignored` → **7 passed** (real sidecar).
- `cargo test -p hickeyfield-tauri --lib score_join_cleans_up_the_scratch_first_frame -- --ignored`
  → **1 passed** (real sidecar, no leftover `first*.png`).
- `./scripts/lint-provenance.py` → PASS.
- keyless `audit_fal` → **RANGE MISMATCH (0)**, CHECKED (70), checks 1–6 unchanged (11/2/21/0/0).
- `cd ui && pnpm test` → **302 passed** (+1); `pnpm build` → built (exit 0).

### Known limitations after this round (not hidden)
- The manual "Regenerate from frame" button is still deferred UI polish (unchanged from the base
  claim); the M1 money-path guard it will call is now correct and tested.
- Thresholds remain reasoned, not measured (Q6/N2) — unchanged, human-run calibration.
