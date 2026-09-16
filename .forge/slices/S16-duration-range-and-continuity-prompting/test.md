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
