# S16-duration-range-and-continuity-prompting — TEST (evidence)

Author: adversarial verifier (a different agent than the implementer). Every command below was
run first-hand; output is pasted verbatim, not copied from `implement.md`.

## Claim under test (implement.md, verbatim excerpts)

> - `cargo test --workspace` → **919 passed, 0 failed** (+31 net new tests).
> - `cargo test -p hickeyfield-core --lib ffmpeg -- --ignored` → **7 passed, 0 failed**
> - `cargo fmt --all --check` → clean (exit 0).
> - `cargo clippy --workspace --all-targets -- -D warnings` → clean.
> - `./scripts/lint-provenance.py` → PASS (v2 prompt indexed, no vendor copy).
> - keyless `audit_fal` → **RANGE MISMATCH (0)**, CHECKED (70), no regression in checks 1–6.
> - `cd ui && pnpm test` → **301 passed** (+9). `pnpm build` → built.

Deviations the implementer recorded (not hidden): AC0-H2 `chainable_model` tightening REVERTED to the
coarse `use_case::supports` (kept coarse to exclude `gemini_omni`); world-block text editor and the
per-shot Storyboard "Regenerate from frame" button DEFERRED as UI polish (backend/money-path,
Library badge, auto-retry all landed); thresholds 0.80/12/6 reasoned not measured (Q6, human-run).

## Commands (each acceptance gate, real output)

### AC7 gate — `cargo test --workspace` (dummy keys set)
```
test result: ok. 746 passed; 0 failed; 16 ignored; 0 measured; 0 filtered out   (tauri-lib)
test result: ok.   6 passed; 0 failed;  0 ignored                                (examples)
test result: ok.   2 passed; 0 failed;  0 ignored
test result: ok. 165 passed; 0 failed;  1 ignored; 0 measured; 0 filtered out    (core)
```
746 + 6 + 2 + 165 = **919 passed, 0 failed**. Matches the claim.

### AC9 real-sidecar — `cargo test -p hickeyfield-core --lib ffmpeg -- --ignored`
bundled `src-tauri/binaries/ffmpeg-aarch64-apple-darwin` (51,576,912 bytes) present.
```
test ffmpeg::tests::first_frame_writes_the_head_frame ... ok
test ffmpeg::tests::a_brightness_shift_fails_the_colour_gate ... ok
test ffmpeg::tests::identical_frames_score_near_one ... ok
test ffmpeg::tests::a_different_source_falls_below_the_ssim_floor ... ok
test ffmpeg::tests::probe_file_reads_a_real_sidecar_output ... ok
test ffmpeg::tests::last_frame_writes_a_png_with_the_clips_dimensions ... ok
test ffmpeg::tests::render_stitches_to_the_expected_duration ... ok
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 755 filtered out
```
The identical-frame (≈1.0 pass), brightness-shift (colour gate non-vacuous), and different-source
(below SSIM floor) cases run the REAL binary end-to-end — stronger than a fixture comparison.

### fmt / clippy / provenance
```
cargo fmt --all --check          -> FMT_OK (exit 0)
cargo clippy --workspace --all-targets -- -D warnings -> REAL_EXIT=0 (no warning/error lines)
./scripts/lint-provenance.py     -> PASS (80015 shingles, no shipped string matches corpus); exit 0
```

### AC6 keyless audit — `env -u hickeyfield_FAL_KEY -u FAL_KEY cargo run ... --example audit_fal`
```
=== PINNED FIELDS (0) ===
=== BAD ROUTE CONSTANTS (0) ===
=== RANGE MISMATCH — a spec duration range wider than the endpoint's (0) ===
=== UNREACHABLE (0) ===
=== CHECKED — fal answered with a schema (70) ===
```
RANGE MISMATCH section present, count **0**; 70 endpoints checked. Matches the claim.

### AC7 UI — `pnpm test` / `pnpm build` (Node v24.8.0)
```
Test Files  23 passed (23)
     Tests  301 passed (301)
...
✓ built in 872ms   (BUILD_EXIT=0)
```
301 vitest pass; production build succeeds. Matches the claim.

## Break attempts

All mutations were applied to a scratch copy OUTSIDE the repo
(`.../scratchpad/mut`); the repo working tree was never modified (only this test.md).

**M1 — AC1 schema range drops `max`** (`range_of` returns `(min,min)`): `a_direct_integer_min_max_is_recorded`,
`a_nullable_integer_range_is_recorded_from_anyof`, `a_field_with_no_bound_has_no_range` all FAILED. Caught.

**M2 — AC2 precedence flipped** (`registry_range.or(schema...)` = registry-first):
`an_axis_range_comes_from_the_live_schema_first` FAILED; `..._falls_back_to_the_registry` still ok.
Live-first precedence is real. Caught.

**M3 — AC9 `join_passes` treats unscored `None` as failing** (`_ => false`):
`join_passes_at_the_four_boundaries` FAILED. Confirms "unscored ≠ failed" (no spurious paid regenerate). Caught.

**M4 — AC8 `NoHoldEnding` disabled**: `a_non_final_shot_without_a_hold_is_rejected` FAILED. Validator non-vacuous. Caught.

**M5 — AC5 `clamp_shot` free-form range clamp disabled**: `clamp_shot_uses_the_range_for_a_free_form_model`
FAILED. The 20→15 clamp is real. Caught.

**M6 — AC6 `range_conflicts` never flags**: `range_conflicts_flags_a_spec_range_outside_the_schema` FAILED. Caught.

**M7 (live audit teeth) — widen `wan3_0_prime` registry range to (2,40)** and re-run the keyless
audit binary:
```
=== RANGE MISMATCH — a spec duration range wider than the endpoint's (1) ===
  wan3_0_prime               alibaba/wan-3.0-prime/text-to-video
      spec duration range (2, 40) is not within the endpoint's (2, 30)
```
The check has real teeth against fal's live-published (2,30). Caught. (The real repo's declared
(2,30) matches, hence the honest 0 in the clean run.)

**M8 — AC3 `ladderFromRange` emits `min-1`** (out-of-bound value): vitest
`offers fifteen and is not capped at twelve` and `covers the full range thinned` both FAILED
(2 failed | 24 passed). The "never outside the bound" invariant is enforced. Caught.

### Targeted refutation probes (no mutation needed)

- **12-second cap fix**: `durationLadder` (`variants.ts:45`) is enum → range → generic → none;
  `ladderFromRange` builds integers `ceil(min)..=floor(max)`, never outside. Registry declares
  `duration_range=(5.0,15.0)` on H3 Max/Turbo and `(2.0,30.0)` on wan3_0_prime; `derived()` inherits
  it. vitest asserts 15 is offered and 12 is not the ceiling. Owner's cap bug is closed.
- **AC0-H2**: `chainable_model` (`chain.rs:48`) = coarse `use_case::supports(_, ImageToVideo)`.
  `gemini_omni` IS registered and IS in `NO_MEDIA_MODELS` (`use_case.rs:38`), so the guard in
  `chainable_model_keeps_h3_max_and_excludes_media_ignorers` is non-vacuous; H3 Max stays true,
  gemini_omni excluded, whole roster agrees with the picker authority. Verified.
- **AC4 clamp-before-estimate**: `clamp_duration_to_range` is called at `commands.rs:1105`, BEFORE
  `Billable::from` (1111) and `estimate_with_provenance` (1123); it reads `model.spec.duration_range`
  (offline). Estimate prices the clamped value. Verified.
- **AC9 regenerate guard**: relaxation (`commands.rs:1536-1554`) admits resubmit only for an index in
  `below_floor` (`pass == Some(false)` from `joins_json`); a started shot not below floor is refused
  (`shot N has already started`). Money-gated via `submit_one`. Verified.
- **AC10 auto-retry**: `AUTO_RETRY_CAP=2`; `should_auto_retry = enabled && pass==Some(false) &&
  attempt<cap`; off by default (`Storyboard.auto_retry` serde-default false). Unscored never retried.
  Verified.
- **AC9 Library badge UI**: `Library.test.tsx` has "shows the set's minimum continuity-join score and
  flags a low join" and "shows no join badge for an unscored set" — both in the passing 301.

## Findings / recorded deviations (not defects, within accepted scope)

1. **Continuity thresholds are reasoned, not measured** (0.80/12/6). The `--ignored` tests prove the
   machinery and non-vacuity of the gates, not that the numbers are the right line. Q6/N2 record the
   calibration as human-run against a real paid 2×15 set. The owner's "95% floor" is delivered as the
   `continuity_rate` machinery + flag; the calibration commit is explicitly out of this slice.
2. **UI partial delivery**: the per-shot Storyboard "Regenerate from frame" button and a world-block
   text editor are NOT surfaced (deferred UI polish, disclosed). The backend money-path, guard
   relaxation, Library min-join badge, and auto-retry toggle ARE implemented and tested. The manual
   regenerate command path exists and is guarded; only the dedicated shot-card button is absent.
3. **AC8 world rules are opt-in on a non-empty `world`** (serde-default) for backward-compat with
   stored pre-S16 plans; `MissingWorld` fires on a present-but-thin world in a multi-shot plan, and
   the v2 prompt always emits a world for a continuous run. Consistent design choice, disclosed.

These are all recorded in `implement.md` and match the launching agent's stated accepted deviations;
the owner-priority items (12-second cap fix, per-join continuity score machinery + floor) landed and
are proven. No mutation of a new test survived; no gate failed.

VERDICT: PASS

## RE-TEST (HEAD 48cb68d)

Repair commit `48cb68d` ("S16 repair 1") landed after the reviewer found a dormant double-bill:
the regenerate/auto-retry below-floor relaxation admitted a shot without checking live job status,
so a second submit between a regenerate and its settle would mint a duplicate in-flight job. I
re-tested the delta (`git show 48cb68d`). Same rules held: no key, no money, repo read-only apart
from test.md; all mutations in a scratch copy OUTSIDE the repo.

### What changed (verified against the diff)
- `commands.rs`: new `pub(crate) fn shot_in_flight(jobs, idx)` = any row at `idx` not
  `is_terminal()`. The regenerate relaxation now: `!started → continue`; started & not below-floor →
  "already started"; below-floor & `shot_in_flight` → "still in flight" refusal. Only a settled
  (terminal) low-join shot is admitted.
- `chain.rs`: `maybe_auto_retry` gains the same `shot_in_flight` guard before it regenerates; and
  `score_join` removes the scratch `first{i}.png` after scoring (the `shot{i-1}.png` continuity frame
  survives).
- `variants.ts`: `ladderFromRange` rewritten so min/max/default are mandatory and consume the budget
  first, then a stride fill, then `.slice(0, MAX_CHIPS)` — deterministically ≤12 chips.
- `storyboard.rs`: `contains_word` (whole-word/phrase, boundary-checked) replaces naive
  `hay.contains` for CUT_WORDS; `ends_on_hold` accepts "hold"/"holds"/"static hold" as whole words.

### Gate (re-run on HEAD 48cb68d, real output)
```
cargo test --workspace  -> 749 + 6 + 2 + 167 = 924 passed, 0 failed   (matches coordinator's 924/0)
cargo test -p hickeyfield-core --lib ffmpeg -- --ignored          -> 7 passed, 0 failed
cargo test -p hickeyfield-tauri --lib score_join_cleans -- --ignored -> 1 passed, 0 failed  (8 total sidecar)
cargo fmt --all --check   -> FMT_OK
./scripts/lint-provenance.py -> PROV_OK
cargo clippy --workspace --all-targets -- -D warnings -> CLIPPY_EXIT=0
cd ui && pnpm test -> Tests 302 passed (302);  pnpm build -> ✓ built
```

### Guard driven directly (real HEAD, `cargo test -p hickeyfield-tauri --lib in_flight`)
```
test commands::tests::shot_in_flight_reads_only_non_terminal_rows ... ok
test commands::tests::a_below_floor_join_that_is_still_in_flight_is_refused ... ok
test result: ok. 2 passed; 0 failed
```
`a_below_floor_join_that_is_still_in_flight_is_refused` asserts: Queued below-floor → refused
("still in flight"); InProgress below-floor → refused; Completed below-floor → NOT refused by the
guard (the one eligible case, i.e. admitted once). A second call while in flight is exactly the
Queued/InProgress case → refused. `shot_in_flight_reads_only_non_terminal_rows` proves
Completed/Failed are terminal and a terminal row does not mask an in-flight row at the same index.
`maybe_auto_retry` calls the same `shot_in_flight(&jobs, i)` before regenerating (chain.rs:404-408),
so auto-retry skips an in-flight index — code-verified, sharing the tested helper.

### Mutation checks (scratch copy OUTSIDE the repo — each new guard/test proven non-vacuous)
- **M-a — `shot_in_flight` returns `false`** (guard defeated): `shot_in_flight_reads_only_non_terminal_rows`
  FAILED. The integration test then proceeds PAST the guard toward a real `submit_one` and hangs on
  the (keyless) provider path — itself demonstrating the double-bill the guard prevents. Caught.
- **M-c — ladder `<=12` cap defeated** (drop the size-bounded fill + final `.slice`):
  `never emits more than twelve chips when the default is not a stride pick` FAILED. Caught.
- **M-d — `ends_on_hold` reverts to `action.contains("hold")`**: `hold_ending_accepts_several_phrasings`
  FAILED ("household" wrongly counts as a hold). Caught.
- **M-e — `contains_word` reverts to naive substring**: `a_cut_word_matches_only_whole_words` FAILED
  ("later" fires inside "slater"). Caught.
- **M-f — remove the `remove_file(&first)` cleanup**: `score_join_cleans_up_the_scratch_first_frame`
  (`--ignored`, real sidecar) FAILED — `first1.png` left behind. The `shot0.png` continuity frame
  still survives. Caught.

### New ladder still yields the ceilings (real `ladderFromRange`, executed)
```
H3 Max (5,15) default 5 -> [5,6,7,8,9,10,11,12,13,14,15]  (11 chips, includes 15)
Wan    (2,30) default 5 -> [2,5,7,10,12,15,17,20,22,25,27,30] (12 chips, includes 2 and 30, all in-bound)
```
15 is offered on H3 Max, 30 on Wan; ≤12 chips; nothing outside the bound; the default is kept.

### Assessment
The repair closes the dormant double-bill: a below-floor regenerate and an auto-retry now refuse an
index with a live queued/running job and admit only a settled (terminal) low-join shot. The scratch
frame is cleaned without touching the continuity frame the chain/render need. The ladder is
deterministically capped at 12 while still exposing 15/30. All four repair points are guarded by
tests that fail under mutation; the full gate is green (924/0, 8 sidecar, fmt/clippy/provenance,
UI 302 + build). No new defect; the earlier PASS stands and the reviewer's finding is resolved.

VERDICT: PASS
