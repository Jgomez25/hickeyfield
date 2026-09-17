# S17-keyframe-anchored-mode — IMPLEMENT (claim)

Author: resuming implementer (fresh, after the previous implementer was stopped mid-work per the
INCIDENT 2026-09-16). Branch `forge/s17-keyframe`, work in the tree, not committed. This claim covers
the WHOLE slice: the previous implementer's committed Rust/UI work (re-verified and, where it was
broken or unsafe, fixed) plus my changes. Every command below was run with the mandated env
`HICKEYFIELD_NO_KEYCHAIN=1 hickeyfield_{FAL,HIGGSFIELD,HIGGSFIELD_SECRET,GOOGLE,OPENAI,XAI,BFL,RECRAFT,VAIG}_KEY=x`.
No provider was ever paid: the keychain is blocked under `cfg(test)`, keys are dummy `"x"`, and the
one test that used to reach a real still submit now fails at route resolution offline (below).

## Starting state on the branch (measured, not the handoff's claim)
- The handoff said "Rust green (945)". It was **not**: `cargo test --workspace` had **1 failure** —
  `chain::tests::resume_submits_shot_zero_when_all_stills_are_ready` (`chain.rs:1852`,
  "the fully-reconciled set is ready to submit shot 0"). Root cause: the test asserted the set was
  still *ready* (no shot-0 row) **only** because the previous author had run with the provider env
  **stripped**, so `submit_one` failed at route resolution and wrote no row. Under the mandated dummy
  keys the route resolves, `submit_one` writes a Queued shot-0 row (and would POST to a **live**
  network — `curl https://fal.run` returns 404 here, so the sandbox is NOT offline), so the assertion
  broke. This is the exact stripped-env landmine the incident is about.
- The UI did not build (`ui/src/components/Library.tsx:259` used an undefined `keyframeStills`;
  `Storyboard.tsx:327` passed `keyframeMode` to an `onGenerate` type that lacked it) and 6 Library +
  1 api vitest cases failed.

## Files I changed (on top of the committed prior work)
- `src-tauri/src/chain.rs` — (1) `keyframe_board` test helper now pins each shot to an **unavailable
  route** (`"unavailable:no-such-route"`), so if any barrier drives `submit_shot_zero`→`submit_one`
  in a test, `route::resolve` returns `UnknownPin` **before** a Queued row is written or any provider
  is contacted. (2) `resume_submits_shot_zero_when_all_stills_are_ready` gains an explicit
  "no shot row was created" assertion (zero provider calls). This restores the previous author's
  intent (route resolution fails) **without** stripping env.
- `src-tauri/src/commands.rs` — **deleted** `strip_provider_env`/`restore_provider_env` (the
  process-global env mutation that caused the incident) and rewrote their two callers:
  `keyframe_generate_holds_the_shots_and_stores_the_boundaries` is now an **offline** derive→validate→
  price→store test (qwen has only a fal route, so running the live still-submit loop would POST — the
  codebase's own `same_model_shots_survive_as_distinct_rows` discipline is to never run the submit
  loop in a test); `keyframe_generate_rejects_a_non_text_to_image_still_model` drops the strip (the
  validator rejects before any submit) and asserts nothing was stored. A NOTE comment documents why
  no `strip_provider_env` helper may exist.
- `crates/hickeyfield-core/src/fal_schema.rs` — added two keyless tests for design §10.3 (the flagged
  "one thing this design could not run"): `the_keyframe_still_image_size_object_lands_in_the_qwen_body`
  (the `{width,height}` still canvas lands verbatim through the schema-gated override seam against the
  real Qwen Image 3 `image_size` anyOf) and `a_still_image_size_is_dropped_when_the_endpoint_does_not_
  declare_it` (the S9 harm — an endpoint without `image_size` sweeps it, never 422s).
- `ui/src/components/Library.tsx` — defined the missing `keyframeStills(storyboard, jobs,
  onRegenerateStill)`: one `.storyboard-keyframe-card` per boundary, `data-state` ready/pending/parked
  read from the still job's own status, a settled `.storyboard-keyframe-thumb`, the parked message,
  and `.storyboard-regenerate-still`. Returns null for a non-keyframe set. Imported `displayUrl`.
- `ui/src/components/Storyboard.tsx` — added `keyframeMode`/`stillModelId` to the `onGenerate` input
  type (the build blocker).
- `ui/src/App.tsx` — wired `onRegenerateStill` (calls `regenerateKeyframeStill`, then refreshes jobs +
  set headers) into `<Library>`, and a `stillEstimate` (qwen's known fal route, priced from the
  current settings) into `<Storyboard>` so the keyframe total shows the real $0.04/still.
- `ui/src/api.test.ts` — `listStoryboards` now maps `keyframes: []` (consistent with the always-present
  `joins: []`/`autoRetry: false`); the exact `toEqual` was updated to expect it.
- `ui/src/components/Library.test.tsx` — +4 tests: both cut+end scores per join; unscored end shows
  "—" never 0; settled still shows a thumbnail + Regenerate; failed still shows the parked state.
- `ui/src/components/Storyboard.test.tsx` — +3 tests: the Keyframe toggle enables only when every shot
  takes an end frame AND an image model exists; the total includes the N+1 stills and rides
  `keyframeMode`; an unpriced (GPT-tier) still marks the total "unpriced", never $0.
- `ui/src/base.css` — additive `.storyboard-end`, `.storyboard-keyframe-*`, `.storyboard-regenerate-
  still` styles.
- `CHANGELOG.md` — the `[Unreleased] > Added` keyframe-mode entry (design §7 wording).

The previous implementer's committed code (`storyboard.rs` keyframes/validator/`derive_keyframes`,
`store.rs` migration 11 + additive `JoinRecord` end-* fields, `chain.rs` `keyframes_advance`/
`stills_ready_for_shot_zero`/`submit_shot_zero`/`prepare_next_shot_keyframe`/`score_join` end
adherence/`resume_all`, `commands.rs` keyframe generate branch + `regenerate_keyframe_still`, `app.rs`
hook wiring, `lib.rs` command registration, the `types.ts`/`api.ts`/`Storyboard.tsx`/`mock.ts` UI) is
otherwise unchanged and re-verified below.

## Design §10 weakest points — evidence (each a passing test)
1. **Stills-ready→shot-0 barrier, exactly-once + resume.** `all_stills_ready_submits_shot_zero_with_
   both_anchors`, `a_failed_still_parks_the_set`, `a_still_settling_never_trips_advance_or_score_join`,
   and `resume_submits_shot_zero_when_all_stills_are_ready` (which I fixed) — all pass. The decision is
   the pure `stills_ready_for_shot_zero` (all boundaries on disk AND no shot-0 row → submit; else
   parked/idempotent).
2. **Both-frame binding keyless + `NO_END_FRAME` fallback.** `both_frames_land_in_the_body_on_h3_max_
   i2v` (body carries non-null `image_url` AND `end_image_url` against the vendored H3 Max i2v schema)
   and `a_no_end_frame_model_falls_back_to_start_only_with_an_advisory` (the end frame is NOT attached;
   the advisory `"end-frame anchoring unavailable: …"` is recorded).
3. **`image_size` shape accepted by the qwen schema.** The two new `fal_schema` tests above; the
   vendored `schemas/qwen_image-3_text-to-image.json` declares `image_size` as `anyOf[{$ref ImageSize
   {width,height}}, enum presets]`, default `square_hd`, so the `{width,height}` object is valid.
4. **End-adherence reads the ACTUAL last frame.** `end_adherence_reads_the_actual_last_frame`
   (`--ignored`, real sidecar) passes: a clip whose last frame equals the still scores ≥0.9, a
   different frame <0.8. `score_join_records_end_adherence_slot_even_without_a_sidecar` proves the
   slot is written (None, never 0) when no sidecar.
5. **Additive `JoinRecord` end-* fields + separate retry budgets.** `migration_11_adds_keyframes_json_
   and_roundtrips` (user_version 11, records round-trip, a pre-S17 row loads end-* None) and
   `still_and_shot_auto_retry_bounds_are_separate` (`KEYFRAME_RETRY_CAP` vs `AUTO_RETRY_CAP`, neither
   borrows the other, off→0).

## Gate — commands run and observed results
- `cargo test --workspace` → **947 passed; 0 failed** (was 945 at S16 baseline + my 2 `fal_schema`
  tests; the previously-failing resume test is now green). Exit 0.
- `cargo test -p hickeyfield-core --lib ffmpeg -- --ignored` → **7 passed; 0 failed** (real sidecar).
- `cargo test -p hickeyfield-tauri --lib -- --ignored` → `chain::end_adherence_reads_the_actual_last_
  frame` **ok**, `chain::score_join_cleans_up_the_scratch_first_frame` **ok**; **1 failure**:
  `harness::a_banana_gets_a_cinematic_rewrite_against_a_real_daemon` — needs a running Ollama daemon +
  `OLLAMA_ENHANCE_MODEL`; **fails offline, as flagged in the task**, unrelated to S17.
- `cargo fmt --all --check` → clean (exit 0).
- `cargo clippy --workspace --all-targets -- -D warnings` → clean (re-run on core after the fal_schema
  additions: clean).
- `./scripts/lint-provenance.py` → PASS (no third-party media hosts; no shipped string matches the
  reference corpus).
- `env -u hickeyfield_FAL_KEY -u FAL_KEY HICKEYFIELD_NO_KEYCHAIN=1 cargo run -q -p hickeyfield-core
  --example audit_fal` → **RANGE MISMATCH 0, UNREACHABLE 0, CHECKED 70** — unchanged (this slice adds
  no route).
- `cd ui && pnpm test` → **309 passed** (302 S16 baseline + 7 new keyframe tests), 23 files, 0 failed.
- `cd ui && pnpm build` → `tsc --noEmit` + `vite build` OK.
- `ui/node_modules/.bin/tauri build --bundles app` (repo root, Node 24) → exit 0:
  `Finished \`release\` profile [optimized] target(s) in 19m 26s` …
  `Bundling Hickeyfield.app` …
  `Finished 1 bundle at: /Users/jorge/Documents/GitHub/hickeyfield/target/release/bundle/macos/Hickeyfield.app`
  (built 17:10; NOT installed, NOT opened — RELEASE does that).

## Money safety (INCIDENT compliance)
- No `strip_provider_env` / `std::env::remove_var` on a provider key exists in any test I touched or
  added (grep confirms only vault.rs's own guard tests use `remove_var`, and they are the pre-existing
  keychain-guard proofs the task said to reuse). The two former callers were rewritten.
- Every keyframe test that could reach a submit either (a) stops at the offline `PinUnusable`/
  `UnknownPin` route error (no Queued row, no provider contacted — asserted), or (b) exercises only
  the offline derive/store half. `keyframe_generate_holds_the_shots_and_stores_the_boundaries` asserts
  `jobs_in_storyboard(sid).is_empty()`.
- The sandbox has LIVE network (verified), so this discipline is load-bearing: a resolvable route +
  dummy key would POST. It never does in the suite.

## Known limitations / deviations (not hidden)
- **`imageModelAvailable` is left at its default `true`** in App.tsx. The Storyboard tab's `models`
  list is use-case-filtered and excludes image models (qwen), so I could not derive it from that list
  without a separate fetch; `qwen_image_3` is always registered, so `true` is correct today (design
  D5/Q2 note the same). The toggle still correctly disables when a shot's model is not end-frame
  capable (server-computed `endFrameCapable`).
- **`keyframe_generate_holds_the_shots_and_stores_the_boundaries` no longer calls
  `generate_storyboard_inner` end-to-end** — it reproduces the derive→validate→price→store block and
  asserts the stored boundaries, because the live still submit (qwen→fal) would POST on this
  network-live sandbox. The submit path itself is the shared, audited `submit_one` (proven not to
  double-bill by `a_still_regenerate_in_flight_is_refused`; the server prices every still before it
  spends). This matches the codebase's `same_model_shots_survive_as_distinct_rows` pattern.
- **Threshold calibration (0.90/6/3 + end-adherence line) remains reasoned, not measured** (Non-goal
  N1) — a human-run paid 2×15 set. The `--ignored` synth tests prove the machinery, not the numbers.
- The Ollama `--ignored` test fails offline (above) — pre-existing, environment-only.
- I did NOT commit, merge, push, install, or open the app.

## REPAIR ROUND 1 (review VERDICT FAIL: B1 + M1–M4)

All commands run with the mandated env (`HICKEYFIELD_NO_KEYCHAIN=1` + dummy `hickeyfield_*_KEY=x`).
Work in the tree, not committed. No provider contacted, no money.

### B1 (MAJOR, fixed) — end-adherence badge never rendered on real data (snake/camel)
Root cause confirmed: Rust `JoinRecord` (`src-tauri/src/store.rs:22`) serialises `end_ssim`/`end_pass`
(snake) into `joins_json`, but `ui/src/api.ts` did a raw `JSON.parse` cast and `Library.tsx` reads
`endSsim`/`endPass` (camel) — so the badge never rendered; the camelCase vitest fixtures hid it.
Fix (UI side, per the reviewer — no `rename_all` on the Rust struct):
- `ui/src/api.ts` — added `parseJoinRecords(json)` (mirrors `parseKeyframeRecords`): maps
  `end_ssim→endSsim … end_pass→endPass`, accepts both spellings; `listStoryboards` now routes
  `joins_json` through it (was a raw `JSON.parse … as JoinRecord[]`).
- Second, latent defect I found while fixing B1: the Rust end-* fields are `#[serde(default)]` **without**
  `skip_serializing_if`, so `end_ssim:null` is emitted on **every** post-S17 join — including plain S16
  chained sets. Gating the badge on `endSsim !== undefined` (the old code) would then render "end —" on
  every scored non-keyframe set. Fixed the gate to the true signal: `ui/src/components/Library.tsx`
  `endAdherenceSummary(joins, isKeyframeSet)` now takes an `isKeyframeSet` flag, and the call site passes
  `(g.storyboard.keyframes?.length ?? 0) > 0`. End-adherence is a keyframe-mode concept, so only a
  keyframe set shows the badge.
- Tests (drive the REAL wire shape, not camelCase fixtures):
  - `ui/src/api.test.ts` "maps the S17 end-adherence fields snake_case → camelCase (B1)" — a real
    `joins_json` string with `end_ssim`/`end_pass` through `listStoryboards` → `endSsim`/`endPass` set.
  - `ui/src/components/Library.test.tsx` "renders the end badge from a REAL snake_case joins_json through
    the production mapping (S17 B1)" — `parseJoinRecords` on the exact wire string → the `.storyboard-end`
    badge renders "0.91". And "shows NO end badge for a scored NON-keyframe set (S17 B1 guard)" — a set
    with `end_ssim:null` but no keyframes shows the cut badge and NO end badge. The two existing S17
    end-badge tests were updated to carry a `keyframes` array (they now represent genuine keyframe sets).

### M1 (fixed) — end-frame capability judged on `routes.first()`, submit resolves `Cheapest`
- `crates/hickeyfield-core/src/storyboard.rs` (`clamp`/plan, the `shot.end_frame_capable` line) and
  `src-tauri/src/chain.rs` `end_frame_capable(model)` now judge on **any** route
  (`model.routes.iter().any(|r| can_bind(End, …))`), not just the first, so the toggle default and the
  chain's attach decision match the route `submit_one` actually resolves. `media::bind` re-checks per
  route at submit and errors before any POST if the resolved route cannot take the End role (no silent
  full-price drop — the S9 harm).
- Test: `src-tauri/src/chain.rs` "end_frame_capability_reads_any_route_not_just_the_first" — an H3 Max
  model with a NO_END_FRAME slug (`veo3.1`) prepended as `routes[0]`: the first route cannot bind End
  but a later one can → `end_frame_capable` is true; with only the non-end route → false.

### M2 (fixed) — validator dropped the design's priced/resolvable-route check
- `crates/hickeyfield-core/src/storyboard.rs` `validate_keyframes` now requires the still model be both
  runnable for `TextToImage` **and** have a resolvable adapter-backed route
  (`route::resolve(&m.routes, adapters, Cheapest, None, |_| Some(0.0)).is_ok()`, where `adapters` = every
  provider with an adapter) — design D1's "priced fal route (`route::resolve(Cheapest).is_ok()`)".
  `use_case::supports` only checks the modality/media shape, not routability.
- Test: `storyboard.rs` "a_still_model_without_a_runnable_route_is_rejected" — `z_image` (only route
  `local:z-image`, Local has no adapter) is now rejected with `KeyframeModelNotRunnable`, while a routable
  but **unpriced** GPT tier (`gpt_image_2_5_flare`) is still **accepted** (D6: the total is marked
  "unpriced", not rejected — the gate is "a resolvable route", not "priced to a figure").

### M3 (fixed) — keyframe cut score gated on the FLOOR, not the IDEAL
- `src-tauri/src/chain.rs` — added `cut_threshold(is_keyframe)` (IDEAL 0.90/6/3 for a keyframe set —
  the 99% path, design D4/Q7 — FLOOR 0.80/12/6 otherwise); `score_join`'s cut `pass` now uses it. End
  adherence stays IDEAL. Non-keyframe chained sets keep the S16 floor.
- Test: `src-tauri/src/chain.rs` "the_cut_score_reads_the_ideal_tier_for_a_keyframe_set_and_the_floor_
  otherwise" — `cut_threshold(true)==IDEAL`, `cut_threshold(false)==FLOOR`, and a mid-band score
  (0.85 SSIM) passes the floor but fails the ideal, so a keyframe set retries on the right tier.

### M4 (documented, coverage as far as offline-safe) — real generate branch end-to-end
The rejection path of the real `generate_storyboard_inner` keyframe branch **is** exercised end-to-end
offline by `keyframe_generate_rejects_a_non_text_to_image_still_model` (it calls the real function, hits
`validate_keyframes`, returns Err, stores nothing — asserted). The **success** path (records stored →
stills submitted → shots returned pending) cannot be driven offline here: the reviewer's suggested
"UnknownPin still route" is not reachable, because (a) the still uses `route_id: None` → `Cheapest` and
there is no public seam to pin its route, and (b) M2 now guarantees the still model has a resolvable
adapter route, so with the mandated fal key present it resolves and `submit_to_provider` would POST on
this **network-live** sandbox (verified: `curl https://fal.run` → 404, i.e. reachable). Making that POST
— even a 401 — violates the no-provider-call discipline. The proper fix is a fake-provider-client seam
(a trait object injected into the submit path) so the generate/submit path can run offline; the reviewer
endorsed this as a follow-up slice (their answer (a)). I did not add a network call. The success path's
constituent parts remain covered: derive/validate/price/store (`keyframe_generate_holds_the_shots_and_
stores_the_boundaries`), the stills-ready barrier + shot-0 submit + record writeback (`chain.rs`
barrier tests), and the still-submit money path is the shared, audited `submit_one`
(`a_still_regenerate_in_flight_is_refused` + S13/S16 submit tests).

### Repair-round gate (all green, pasted)
- `cargo test --workspace` → **950 passed; 0 failed** (947 + B1/M1/M2/M3 tests; B1 added 3 UI tests).
- `cargo test -p hickeyfield-core --lib ffmpeg -- --ignored` → 7 passed.
- `cargo test -p hickeyfield-tauri --lib chain -- --ignored` → 2 passed (incl.
  `end_adherence_reads_the_actual_last_frame` on the real sidecar).
- `cargo fmt --all --check` → clean (one line reflowed by `cargo fmt`).
- `cargo clippy --workspace --all-targets -- -D warnings` → clean.
- `./scripts/lint-provenance.py` → PASS.
- `cd ui && pnpm test` → **312 passed** (309 + 3 B1 tests), 23 files, 0 failed.
- `cd ui && pnpm build` → OK.
(The repair-round gate as posted did not require re-running `tauri build`; the round-1 build already
passed and no source touched here changes the bundle contract.)
