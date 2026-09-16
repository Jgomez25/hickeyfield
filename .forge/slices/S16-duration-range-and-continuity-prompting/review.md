# S16-duration-range-and-continuity-prompting — REVIEW (evidence)

Independent, read-only production-standards review of the slice diff (`git diff main...HEAD`,
HEAD `c76ac43`). Scope: only what this slice touched. No source, docs, or backlog changed by
this review. Gate re-run first-hand (results below), then the diff read line by line.

## Gate re-run (this machine, dummy-key env, Node 24 as specified)

- `cargo fmt --all --check` → clean (exit 0).
- `cargo test --workspace` → **919 passed, 0 failed** (746 core + 165 tauri-lib + 6 + 2; 16+1+1
  ignored). Matches the implementer's claim.
- `cargo clippy --workspace --all-targets -- -D warnings` → clean (0 warnings/errors).
- `cargo test -p hickeyfield-core --lib ffmpeg -- --ignored` → **7 passed, 0 failed** — the real
  bundled sidecar end-to-end: `identical_frames_score_near_one`,
  `a_brightness_shift_fails_the_colour_gate`, `a_different_source_falls_below_the_ssim_floor`,
  `first_frame_writes_the_head_frame` (plus the S15 three). This is the load-bearing evidence that
  the argv and the `parse_ssim`/`parse_signalstats`/`parse_psnr` parsers key off the actual output.
- keyless `audit_fal` → **RANGE MISMATCH (0)**, CHECKED (70); checks 1–6 unchanged
  (11 / 2 / 21 / 9 / 0 / 0) — no regression.
- `./scripts/lint-provenance.py` → PASS (v2 prompt indexed, no vendor copy, no third-party hosts).
- `cd ui && pnpm test` → **301 passed** (23 files); `pnpm build` → built (exit 0).

## What I checked and found clean

- **(a) Range precedence.** `fal_axis` (capability.rs:646) sets
  `range = schema.ranges.get(field).copied().or(registry_range)` — the live schema wins, the
  registry is only the fallback. A **narrower live range therefore overrides a wider stale
  registry range** (proven by `an_axis_range_comes_from_the_live_schema_first`, schema `(2,30)`
  beats registry `(9,9)`). No stale-override path. `numeric_range`/`range_of` (fal_schema.rs:449-481)
  read `minimum`/`maximum` directly or from the first `anyOf`/`oneOf` member, skip `const`/`enum`
  fields, and require `min<=max` — so the LTX-class "invent enum members" bug is not reintroduced,
  and `a_nullable_integer_is_not_enumerated` still holds. `Eq` correctly dropped from the four
  structs carrying `(f64,f64)`.
- **(a) Submit clamp on the money path.** `clamp_duration_to_range` (commands.rs:1046) runs on a
  clone **before** `Billable::from(&settings)` (commands.rs:1109), so the estimate prices the
  clamped value; the clamped `settings` is what is persisted (commands.rs:1158). Never widens,
  no advisory when inside range, advisory text is honest (`"{name} runs {min}–{max}s; {d}s clamped
  to {c}s"`). Reads the offline spec range (Q3) — the D10 audit (clean) guards it against being
  wider than a live range.
- **(b) Ladder algorithm.** `ladderFromRange` (variants.ts:66) is deterministic (even stride +
  `Set`), keeps both endpoints (i=0→min, i=MAX-1→max always land), includes the default, and never
  emits outside `[ceil(min),floor(max)]`. H3 Max `[5,15]`→`[5..15]` (11 chips, 15 offered, verified);
  Wan `[2,30]`→12 chips both endpoints kept. `resolveSettings` clamps a free-form duration into the
  range and returns the same object reference when nothing changed (variants.ts:200) — safe in a
  render path.
- **(c) Validator variants are behavioural, not asserted.** Each variant has a fixture that violates
  exactly that rule (`identical_actions_are_rejected`, `a_non_final_shot_without_a_hold_is_rejected`,
  `a_short_world_is_rejected`, `a_cut_word_is_rejected`, `a_broken_composition_is_caught`). World
  gating is opt-in on a non-empty `world` (documented backward-compat deviation) — a stored S13 /
  montage plan still validates. The `1..=8` floor is preserved. `prompts/storyboard.v2.md` is
  consistent with the validator (cut-word list identical, "static hold" phrasing dictated, "1 to 8
  shots" copy fixed) and free of vendor copy (provenance PASS).
- **(d) Score pipeline.** `scale2ref` ordering is correct: `[1:v][0:v]scale2ref[b][a]` scales input
  B to reference A, and `[a1][b1]ssim` compares A vs scaled-B (ffmpeg.rs). `parse_ssim` keys off
  `All:` (aggregate, so a desaturation is caught — the `--ignored` brightness test proves the colour
  gate is non-vacuous). `signalstats.YAVG/UAVG/VAVG` parsed via a substring needle that matches the
  real `lavfi.signalstats.` prefix. `run_capture` reuses the `run_probe` hardening (stdin null,
  bounded drain, deadline+kill) and **requires success** — a filter-graph failure is a real error the
  caller turns into an unscored (`None`, never `0.0`) join. `None`-never-`0` is consistent end to end
  (`JoinScore`, `join_passes` treats `None` as not-failing, `continuity_rate` excludes unscored,
  Library `joinSummary` filters `ssim != null`). `render.rs` reads the per-join `kind` from
  `joins_json` with the media-presence heuristic only as a pre-slice fallback (D8, folds H1).
- **(e) Regenerate guard + auto-retry.** Relaxation (commands.rs:1550) refuses a **passing** or
  unscored started shot and only lets through a `pass == Some(false)` index — verified by
  `a_below_floor_join_may_be_regenerated_but_a_passing_one_may_not`. Auto-retry is off by default,
  bounded at `AUTO_RETRY_CAP = 2` (matches the "two repairs then escalate" memory rule), bumps the
  attempt in `joins_json` and persists **before** submitting (chain.rs), routes through `submit_one`
  which prices the row before spending, and `advance`'s existing read-guard prevents the regenerated
  shot's re-settle from double-submitting the next shot. `auto_retry_stops_after_two_attempts` pins
  the bound; `a_scoring_failure_is_none_never_zero` proves an unscored join is never auto-retried.
- **(f) Reverted `chainable_model`.** Correct call: the tightened per-route `can_bind(Start)` is
  *looser* on `Dialect::Fal` (permissive for unmeasured modes) and wrongly reported the
  media-ignoring `gemini_omni` as chainable. The revert to `use_case::supports(model, ImageToVideo)`
  keeps H3 Max chainable and excludes the media-ignorer. The sweep test
  `chainable_model_keeps_h3_max_and_excludes_media_ignorers` is meaningful — it asserts agreement
  with the picker authority across the **whole roster**, so the two cannot silently drift.
- **Store migration.** Migration 10 adds `auto_retry` (additive, `NOT NULL DEFAULT 0`); `user_version`
  bumped and asserted; upsert/select/round-trip all updated; `parse_joins`/`upsert_join` keep the
  list index-ordered. No `UNIQUE(storyboard_id, shot_index)` (Q7) — correct, it would make retry
  illegal.
- **AC0-H3 non-vacuity.** `a_completed_predecessor_would_advance_so_the_failed_guard_bites` uses the
  `frames/` dir (created past the `Phase::Completed` guard, before submit) as the observable — genuinely
  non-vacuous without a configured provider.
- **Tests in `git diff main -- '*test*'`.** No weakened assertions found; the UI tests are behavioural
  (badge min-score + `data-pass`, toggle visibility + payload), the Rust tests assert concrete values
  and error variants.

## Findings

### Major (latent / not reachable in the shipped UI — fix before wiring the manual button)

- **M1 — Regenerate relaxation keys only on the stored `pass` verdict, not on live in-flight job
  status.** `src-tauri/src/commands.rs:1536-1555`. `below_floor` is built purely from
  `joins_json` `pass == Some(false)`; the guard allows a resubmit whenever
  `started && below_floor.contains(idx)`. `joins_json` is only updated when a shot settles and is
  re-scored, so between submitting a regenerate and its settle the join stays `pass=false` while a
  **Running/Queued** job already exists at that index — a second regenerate at the same index is
  therefore admitted, minting a second in-flight job (a double-bill). The S13 guard this replaced
  refused *any* non-`Failed` row at the index, so this is a narrowing of the in-flight protection.
  It is **currently dormant**: the manual "Regenerate from frame" button is deferred (not wired in
  `Storyboard.tsx`), and auto-retry bypasses this guard via `submit_one` with its own attempt-bump
  bound — so no shipped path reaches it today. Standard violated: money-path guard must not admit a
  resubmit of an already-in-flight shot (design risk #5). Fix: also require that no non-`Failed`
  job at `idx` is currently `Queued`/`Running` (only a `Completed` low-join shot is eligible), or
  clear/park the `pass` verdict at submit time. The existing test uses a `Completed` job and does
  not cover the in-flight case — add that case with the fix.

### Minor

- **m1 — `ladderFromRange` can emit 13 chips.** `ui/src/lib/variants.ts:82-90`. The stride loop
  yields ≤12 picks (endpoints always among them), but the subsequent `kept.push(d)` for a default
  that is not a stride pick can push the count to 13, violating the AC3 "≤12 chips" bound. No landed
  model with a range hits this (H3 Max is unthinned; Wan's default 5 is a stride pick), so it is
  latent. Fix: after pushing the default, drop the nearest non-endpoint pick, or reserve a slot for
  the default in the stride budget.
- **m2 — Cut-word matching is a raw `contains` on short words.**
  `crates/hickeyfield-core/src/storyboard.rs:559,592`. `"later"` and `"elsewhere"` are matched as
  bare substrings, so a legitimate action containing e.g. `"slater"`/`"elsewhere"`-in-a-URL could
  false-positive `CutWords`. Low risk (the v2 prompt dictates the vocabulary and actions are prose),
  but a word-boundary match would be safer.
- **m3 — Hold-ending detection is a literal `"static hold"` substring.**
  `crates/hickeyfield-core/src/storyboard.rs:611`. Robust only because the v2 prompt dictates that
  exact phrasing; a model that writes "holds static" / "static, held frame" would trip
  `NoHoldEnding`. Acceptable given the prompt coupling, but brittle — noted so the coupling is
  explicit.
- **m4 — Doc overstatement.** `CHANGELOG.md` ("every free-form-duration model its actual ceiling")
  and `README.md` ("each model's real range"). Only models with a live-published range (D1) or a
  documented registry `duration_range` (H3 Max/turbo, Wan 3.0 Prime) get their real ceiling; the many
  other free-form-duration models in `registry.rs` (1714/1731/1757/1777/1795/1809/1873/1891/2190)
  that publish neither still fall back to the generic `[3..12]` ladder. The fallback is honest (the
  app does not invent a ceiling it cannot verify), but the prose implies universal coverage.
- **m5 — Score temp frames are not cleaned up.** `src-tauri/src/chain.rs:312`. `score_join` writes
  `frames/first{i}.png` for the scored first frame and never removes it; harmless disk cruft beside
  the retained `shot{i}.png`, but unbounded across regenerate attempts of long sets.

## Coverage of the acceptance criteria

AC1–AC10 and the AC0 hygiene items are each backed by a real, non-vacuous test (Rust unit + `--ignored`
sidecar + vitest), confirmed by the gate re-run above. The two threshold constants (0.80/12/6) are
reasoned, not measured — correctly recorded as human-run calibration (Q6/N2), and the `--ignored`
synth cases prove the machinery and that the colour gate is non-vacuous, which is the honest claim.

## Verdict rationale

All gates pass on this machine; no blocker. The single Major finding (M1) is on the money path but is
**not reachable from the shipped UI** (manual regenerate deferred; auto-retry bypasses the guard with
its own bound), so it is not on a currently-live critical path. It must be closed before the manual
"Regenerate from frame" button is wired; recording it here as a required follow-up. The minor findings
are latent or cosmetic.

VERDICT: PASS

---

## RE-REVIEW (HEAD 48cb68d — repair round 1 of 2)

Re-read `git show 48cb68d` and re-ran the gate on the repaired tree. All five prior findings are
closed with real tests; no new issue introduced.

### Gate re-run (repaired tree)

- `cargo fmt --all --check` clean; `cargo clippy --workspace --all-targets -- -D warnings` clean (0).
- `cargo test --workspace` → **924 passed, 0 failed** (749 core + 167 tauri-lib + 6 + 2). Matches the
  coordinator's 924/0.
- `cargo test -p hickeyfield-core --lib ffmpeg -- --ignored` → 7 passed; the new
  `chain::tests::score_join_cleans_up_the_scratch_first_frame` (real sidecar) → 1 passed.
- `cd ui && pnpm test` → **302 passed** (+1: the new ladder edge case).

### Findings verified closed

- **M1 (major, double-bill) — CLOSED.** New shared `shot_in_flight(jobs, idx)`
  (`src-tauri/src/commands.rs:1084`) = `any(shot_index == idx && !status.is_terminal())`. The
  **manual** path (`generate_storyboard_inner`, commands.rs:1561-1585) now: `!started → continue`;
  `started && !below_floor → "already started"` (above-floor/unscored started shot still refused —
  the invariant held); `started && below_floor && shot_in_flight → "still in flight"`. Only a
  **terminal** (settled) below-floor shot is admitted. The **auto-retry** path
  (`maybe_auto_retry`, chain.rs:402-410) applies the *same* `shot_in_flight` guard before submitting,
  so a not-yet-settled prior attempt cannot mint a duplicate. Tests:
  `a_below_floor_join_that_is_still_in_flight_is_refused` (Queued and InProgress both refused,
  Completed admitted) and `shot_in_flight_reads_only_non_terminal_rows` (a terminal row does not mask
  an in-flight one at the same index). Double-bill closed on both paths; above-floor refusal preserved.
- **m1 (ladder ≤12 invariant) — CLOSED.** `ladderFromRange` (variants.ts:66) now seeds `min`, `max`,
  and the in-range default into the `Set` FIRST, then fills the remaining budget by an even stride
  with the loop bounded on `kept.size < MAX_CHIPS`, ending with `.slice(0, 12)`. Endpoints and default
  are therefore guaranteed present and the count can never reach 13. Deterministic (Set + `Math.round`
  of a fixed stride). Proven by `never emits more than twelve chips when the default is not a stride
  pick` (`[1,100]` default 6 → ≤12, contains 1/100/6, same output twice).
- **m2 (cut words) — CLOSED.** `contains_word` (storyboard.rs) matches on non-alphanumeric boundaries;
  `a_cut_word_matches_only_whole_words` proves `later` does not fire inside `slater` and `elsewhere`
  not inside `elsewheres`, while `meanwhile`/`cut to` still match.
- **m3 (hold phrasing) — CLOSED.** `ends_on_hold` accepts whole-word `hold`/`holds` (so `static hold`,
  `holds on`, `a hold` all pass) and rejects `household`; `hold_ending_accepts_several_phrasings` and
  `a_holds_on_ending_passes` cover it. The relaxation is deliberate and does not weaken the guard
  against a genuinely hold-less non-final shot (`a_non_final_shot_without_a_hold_is_rejected` still
  green).
- **m4 (doc overstatement) — CLOSED.** CHANGELOG and README now say the range covers "every model
  whose provider publishes or documents a range … (models that publish neither still use the generic
  ladder)" — accurate.
- **m5 (scratch frames) — CLOSED.** `score_join` removes `first{i}.png` after scoring while keeping
  the `shot{i-1}.png` continuity frame (chain.rs:315); the ignored real-sidecar test asserts no
  `first*.png` remains and the continuity frame survives.

No regressions and no new findings. All acceptance criteria remain covered by the (now larger) test
suite; the two continuity thresholds remain reasoned-not-measured with the human-run calibration
correctly recorded.

VERDICT: PASS
