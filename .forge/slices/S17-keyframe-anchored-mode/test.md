# S17-keyframe-anchored-mode — TEST (adversarial verification)

Author: adversarial verifier (not the implementer). Branch `forge/s17-keyframe`, HEAD
`b3939cd`, working tree clean. Every command below was run by me with the mandated env
`HICKEYFIELD_NO_KEYCHAIN=1 hickeyfield_{FAL,HIGGSFIELD,HIGGSFIELD_SECRET,GOOGLE,OPENAI,XAI,BFL,RECRAFT,VAIG}_KEY=x`
(always SET, never unset). Output pasted is real and captured by me.

## Implementer's claim (implement.md, verbatim excerpts)

> "cargo test --workspace → **947 passed; 0 failed** … Exit 0."
> "cargo test -p hickeyfield-core --lib ffmpeg -- --ignored → **7 passed; 0 failed** (real sidecar)."
> "cargo test -p hickeyfield-tauri --lib -- --ignored → `chain::end_adherence_reads_the_actual_last_frame` **ok**, `chain::score_join_cleans_up_the_scratch_first_frame` **ok**; **1 failure**: `harness::a_banana_gets_a_cinematic_rewrite_against_a_real_daemon` — needs a running Ollama daemon … fails offline, as flagged."
> "cargo fmt --all --check → clean … cargo clippy … clean … ./scripts/lint-provenance.py → PASS … audit_fal → **RANGE MISMATCH 0, UNREACHABLE 0, CHECKED 70** … cd ui && pnpm test → **309 passed** … pnpm build → OK."
> "No `strip_provider_env` / `std::env::remove_var` on a provider key exists in any test I touched or added (grep confirms only vault.rs's own guard tests use `remove_var`)."
> "Every keyframe test that could reach a submit either (a) stops at the offline `PinUnusable`/`UnknownPin` route error … or (b) exercises only the offline derive/store half."

## Money-safety proof (the primary thing to refute — INCIDENT 2026-09-16)

### (0) Env-stripping grep across the whole tree
`grep -rn 'remove_var' --include='*.rs'` — the ONLY matches:
```
src-tauri/src/vault.rs:148:  std::env::remove_var("hickeyfield_RECRAFT_KEY");   (env_override_is_read_and_trimmed test)
src-tauri/src/vault.rs:155:  std::env::remove_var("hickeyfield_BFL_KEY");       (blank_env_override_is_ignored test)
src-tauri/src/vault.rs:167:  std::env::remove_var("hickeyfield_FAL_KEY");       (keychain_guard test)
src-tauri/src/vault.rs:176:  std::env::remove_var("hickeyfield_FAL_KEY");       (keychain_guard test)
```
`grep -rn 'strip_provider_env\|restore_provider_env'` — one hit, a NOTE comment at commands.rs:3468
documenting that no such helper may exist. `grep` for `set_var("..._KEY", "")` — none. CONFIRMED:
no test outside vault.rs's own guard tests mutates a provider-key env.

### Structural guard (vault.rs:39-76) — read and confirmed
`keychain_blocked()` returns `true` under `cfg!(test)` **unconditionally**; `get()` returns `None`
when blocked, before ever calling `entry(...).get_password()`. So under `cargo test` the owner's
real keychain key is unreachable regardless of env. The two guard tests
(`a_test_build_never_reaches_the_keychain_when_the_env_override_is_absent`,
`the_env_override_still_wins_in_tests`) both pass in the runs below.

### Clients build URLs with no proxy exemption → strong offline proof possible
`crates/hickeyfield-core/src/clients.rs:20` builds the reqwest client with NO `.no_proxy()`, and all
provider URLs are hardcoded (`https://queue.fal.run/{model_id}`, `https://api.higgsfield.ai/...`) with
no `*_BASE_URL` env override. Therefore reqwest honours `HTTPS_PROXY`. I ran the whole suite with every
external host forced through an unroutable proxy (`127.0.0.1:9`, nothing listening), exempting only
localhost (the enhancer unit tests use a localhost `TcpListener` stub, enhancer.rs:2770 — confirmed a
mock, not a provider). Any test that actually POSTed to a provider would die on the dead proxy.

**Run A — dummy keys + dead proxy (external hosts unreachable, localhost exempt):**
```
running 772 tests ... test result: ok. 756 passed; 0 failed; 16 ignored   (hickeyfield-core lib)
tests/live_fal_pricing.rs      ... ok. 0 passed; 0 failed; 1 ignored       (the live pricing GET stays ignored)
examples/audit_fal.rs          ... ok. 6 passed; 0 failed
examples/fal_diff.rs           ... ok. 2 passed; 0 failed
hickeyfield_tauri_lib          ... ok. 183 passed; 0 failed; 3 ignored
```
Total 756+6+2+183 = **947 passed, 0 failed** — matches the claim, and NO test reached a provider
(only localhost was reachable).

**Run B — HICKEYFIELD_NO_KEYCHAIN=1 ONLY, no dummy keys, dead proxy (`env -i`):**
```
hickeyfield-core lib  ... ok. 756 passed; 0 failed; 16 ignored
live_fal_pricing      ... ok. 0 passed; 0 failed; 1 ignored
audit_fal / fal_diff  ... ok. 6 / 2 passed
hickeyfield_tauri_lib ... ok. 183 passed; 0 failed; 3 ignored
```
Also **947 green, zero provider calls.** (An early insurance run with the proxy but NO localhost
exemption failed exactly 2 tests — `enhancer::a_hosted_{openai,anthropic}_call_posts_to_the_overridden_base`
— which confirmed those two are localhost-mock POSTs, not provider calls; with localhost exempt they pass.)

### Keyframe tests reviewed line-by-line for submit reachability
- `chain::keyframe_board` (chain.rs:1419) pins every shot to `route_id = "unavailable:no-such-route"`.
  `submit_one` (commands.rs:1124) calls `route::resolve(...).map_err(...)?` **before** minting a job id,
  writing a Queued row, or any network — so any barrier that reaches submit errors first. Verified the
  ordering in source.
- `all_stills_ready_submits_shot_zero_with_both_anchors`, `a_failed_still_parks_the_set`,
  `a_still_settling_never_trips_advance_or_score_join`, `resume_submits_shot_zero_when_all_stills_are_ready`
  each either drive the barrier while stills are NOT all ready (no shot-0 submit) or assert
  `!jobs.any(shot_index.is_some())` (no shot row created). Read and confirmed.
- `commands::keyframe_generate_holds_the_shots_and_stores_the_boundaries` runs only the
  derive→validate→price→store half and asserts `jobs_in_storyboard(sid).is_empty()`.
  `keyframe_generate_rejects_a_non_text_to_image_still_model` rejects at the validator before store/submit.
  `a_still_regenerate_in_flight_is_refused` returns "still in flight" before `submit_one`.

## Commands (acceptance gate, AC7)

```
cargo fmt --all --check                                   -> exit 0 (FMT_EXIT=0)
cargo clippy --workspace --all-targets -- -D warnings     -> Finished, no warning/error lines
./scripts/lint-provenance.py                              -> PASS  no third-party media hosts;
                                                             PASS  no shipped string matches corpus (80015 shingles)
env -u hickeyfield_FAL_KEY -u FAL_KEY HICKEYFIELD_NO_KEYCHAIN=1 cargo run -q -p hickeyfield-core --example audit_fal
    -> RANGE MISMATCH 0 | UNREACHABLE 0 | CHECKED 70   (matches the claim; keyless public-schema GETs, no money)
cargo test -p hickeyfield-core --lib ffmpeg -- --ignored  -> ok. 7 passed; 0 failed  (real sidecar; binaries/ffmpeg-aarch64-apple-darwin present)
cargo test -p hickeyfield-tauri --lib -- --ignored:
    chain::end_adherence_reads_the_actual_last_frame        ... ok
    chain::score_join_cleans_up_the_scratch_first_frame     ... ok
    harness::a_banana_gets_a_cinematic_rewrite_against_a_real_daemon ... FAILED
    -> FAILED at harness.rs:484 `.expect("set OLLAMA_ENHANCE_MODEL to an installed tag")`
       environment-only (no Ollama daemon / no model tag), pre-existing, unrelated to S17 — as disclosed.
cd ui && pnpm test        -> full parallel run: 3 failed / 306 passed (see Break attempts #7); ALL 3 pass in isolation.
cd ui && pnpm exec vitest run --pool=forks --poolOptions.forks.singleFork -> 23 files, 309 passed, 0 failed.
cd ui && pnpm build       -> tsc --noEmit + vite build: "76 modules transformed; built in 10.86s", exit 0.
```
audit_fal note: run under the dead proxy it reported UNREACHABLE 70 / CHECKED 0 (schema fetches blocked).
Re-run with normal network (keyless, GET public schemas only — no job, no money) it reported the
claimed CHECKED 70 / UNREACHABLE 0 / RANGE MISMATCH 0.

I did NOT run `tauri build --bundles app` (a ~19-min RELEASE-step action; `pnpm build` already validates
the frontend and `cargo build` the shell). Recorded, not a gate failure for TEST.

## Break attempts

1. **Env-strip landmine (the incident).** grep whole tree; only vault.rs guard tests use `remove_var`;
   `cfg(test)` blocks the keychain unconditionally. RESULT: cannot refute — structurally safe.
2. **Force a provider POST during tests.** Ran the entire suite twice (with and without dummy keys) with
   every external host routed to a dead proxy. RESULT: 947 green, zero external reach. Cannot refute.
3. **Unavailable-route ordering.** Confirmed `route::resolve` fails before any row/network in `submit_one`.
   RESULT: a keyframe barrier that reaches submit errors before spending. Cannot refute.
4. **Ignored-test scoping.** `live_fal_pricing` (a keyless GET to fal's pricing API) is `#[ignore]`; my
   `--ignored` runs were scoped to `--lib ffmpeg` and `-p hickeyfield-tauri --lib`, never `--workspace -- --ignored`.
   RESULT: no ignored provider call was triggered.
5. **UI flakiness (idempotency/re-run).** Reran the two failing App tests and the failing Storyboard test
   individually: App.test.tsx 2/2 pass, Storyboard.test.tsx 14/14 pass. A single-fork full run is 309/309.
   RESULT: the 3 failures are jsdom/timer contention under parallel workers, not real defects.
6. **Ollama ignored test.** Confirmed it panics only because `OLLAMA_ENHANCE_MODEL` is unset — env-only.
7. **Mutation testing (≥6) in a scratch copy OUTSIDE the repo** (`scratchpad/mutcopy`, rsync minus target/.git/node_modules; deleted after):
   - M1 `derive_keyframes` drops K0  -> `derive_keyframes_builds_n_plus_one_stills` FAILED (left 2, right 3). KILLED.
   - M2 `end_frame_capable` -> always true -> `a_no_end_frame_model_falls_back_to_start_only` **SURVIVED**
     (the test uses `registry().find(chainable && !end_frame_capable)` and no-ops via `let Some(..) else return`
     when no model matches). A probe (panic on the else-arm) proved the test IS non-vacuous in the real
     roster (a chainable NO_END_FRAME model exists), so the shipped code is exercised; the survivor is a
     TEST-STRENGTH note only (a globally-broken capability check self-skips the finder).
   - M2b attach End regardless of capability -> `a_no_end_frame_model_falls_back...` FAILED ("end frame must
     NOT be attached to a NO_END_FRAME model", chain.rs:1541). KILLED — the fallback branch is guarded.
   - M3 `stills_ready_for_shot_zero` ignores missing `local_path` -> `all_stills_ready_submits_shot_zero`
     FAILED ("a missing still → parked", chain.rs:1613). KILLED — the money-critical barrier is non-vacuous.
   - M4 keyframe migration leaves `user_version = 10` -> `migration_11_adds_keyframes_json_and_roundtrips`
     FAILED (left 10, right 11). KILLED.
   - M5 drop `end_pass` from `should_auto_retry` -> `still_and_shot_auto_retry_bounds_are_separate` FAILED
     ("a below-ideal end adherence warrants a shot auto-retry"). KILLED.
   - M6 still retry cap +1 -> `still_and_shot_auto_retry_bounds_are_separate` FAILED (left 3, right 2). KILLED.
   - M7 never attach End in `prepare_next_shot_keyframe` -> `both_frames_land_in_the_body_on_h3_max_i2v`
     FAILED (chain.rs:1496). KILLED.
   SIX distinct tests killed (M1,M2b,M3,M4,M5/M6,M7); one survivor documented above.
8. **Test-locked CSS classes intact.** All present: model-item, chip-route, enhancer-picker,
   prompt-preview-text, prompt-preview-retry, generate-button, data-unrunnable, data-unavailable, data-tier.
9. **Validator/schema/regenerate tests exist and ran green:** `a_keyframe_count_mismatch_is_rejected`,
   `a_non_text_to_image_still_model_is_rejected`, `a_still_canvas_that_disagrees_with_the_set_is_rejected`
   (storyboard.rs), `the_keyframe_still_image_size_object_lands_in_the_qwen_body` +
   `a_still_image_size_is_dropped_when_the_endpoint_does_not_declare_it` (fal_schema.rs, §10.3 weak point),
   `a_still_regenerate_in_flight_is_refused` (commands.rs).

## Findings (non-blocking, disclosed by the implementer or minor)

- AC2 fidelity: `keyframe_generate_holds_the_shots_and_stores_the_boundaries` reproduces the
  derive→price→store block rather than calling `generate_storyboard_inner` end-to-end (a live still
  submit to qwen→fal would POST). The barrier logic, submit-ordering guard, and in-flight refusal are
  covered by separate tests. The end-to-end still-submit loop is intentionally never run in a test under
  the money rule — inherent, acceptable.
- The `image_size` seam (§10.3) is tested at the `apply_overrides` boundary, not through the full still
  submit; consistent with the same money constraint.
- M2 survivor: `a_no_end_frame_model_falls_back...` self-skips if `end_frame_capable` is globally broken.
  Recommend a future assertion that the finder actually found a model, but the test is non-vacuous today.
- `imageModelAvailable` left default `true` in App.tsx (disclosed, correct today: qwen_image_3 always registered).
- `tauri build` not independently re-run by me (RELEASE-step, ~19 min).

Root cause of the only red in the gate: the Ollama `--ignored` test and the 3 parallel-vitest reds are
both environment/contention artefacts, not S17 code defects — the Ollama test needs a live daemon, and
each vitest red passes deterministically in isolation and in a single-fork full run.

Money safety is proven three independent ways (grep, `cfg(test)` keychain block, and 947 tests green with
every provider host forced through a dead proxy). No test in this suite can reach a provider.

VERDICT: PASS

---

## RE-TEST (HEAD 5b353c0)

Repair commit `5b353c0` ("S17 repair 1") landed after review B1/M1/M2/M3. I re-ran the whole gate at
this HEAD (working tree clean) with the mandated env `HICKEYFIELD_NO_KEYCHAIN=1` + dummy `hickeyfield_*_KEY=x`
on every command, and kept the unroutable-proxy discipline (`HTTPS_PROXY=http://127.0.0.1:9`, only
`127.0.0.1,localhost` exempt) on the suite runs. `git show 5b353c0`: `parseJoinRecords` snake→camel in
api.ts; end badge gated on `isKeyframeSet` in Library.tsx; `end_frame_capable` on ANY route (chain.rs +
storyboard.rs `plan`); `validate_keyframes` adds a `route::resolve` check; `cut_threshold(is_keyframe)`
gates keyframe cut scores on the IDEAL tier.

### Gate (all pasted from my runs)
```
cargo test --workspace (dummy keys + dead proxy, localhost exempt):
   hickeyfield-core lib   757 passed; 0 failed; 16 ignored
   live_fal_pricing       0 passed; 1 ignored
   audit_fal / fal_diff   6 / 2 passed
   hickeyfield_tauri_lib  185 passed; 0 failed; 3 ignored
   => 757+6+2+185 = 950 passed, 0 failed  (matches the coordinator's 950/0; zero provider reach)
cargo test -p hickeyfield-core --lib ffmpeg -- --ignored   -> 7 passed; 0 failed
cargo test -p hickeyfield-tauri --lib -- --ignored:
   chain::end_adherence_reads_the_actual_last_frame     ... ok
   chain::score_join_cleans_up_the_scratch_first_frame  ... ok
   harness::a_banana_gets_a_cinematic_rewrite...        ... FAILED (OLLAMA_ENHANCE_MODEL unset; env-only, unchanged)
cargo fmt --all --check                    -> clean (FMT_OK)
cargo clippy --workspace --all-targets -- -D warnings  -> Finished, no warnings/errors
./scripts/lint-provenance.py               -> PASS (no media hosts; no corpus match)
audit_fal (keyless, normal net)            -> RANGE MISMATCH 0 | UNREACHABLE 0 | CHECKED 70 (unchanged)
ui: vitest --pool=forks --singleFork       -> 23 files, 312 passed, 0 failed
ui: pnpm build                             -> tsc --noEmit + vite: "76 modules transformed; built in 3.30s", exit 0
```

### Snake_case joins_json → Library badge (review B1) — verified via the shipped tests
The repair's new tests drive the EXACT wire shape (not camelCase fixtures), and all pass in the 312 run:
- `api.test.ts > maps the S17 end-adherence fields snake_case → camelCase (B1)` — feeds
  `joins_json` containing `"end_ssim":0.91 … "end_pass":true` through `listStoryboards`; asserts
  `joins[0].endSsim === 0.91`, `endPass === true`, cut half preserved (`ssim === 0.93`).
- `Library.test.tsx > renders the end badge from a REAL snake_case joins_json through the production
  mapping (S17 B1)` — same wire string via `parseJoinRecords`, keyframe set → `.storyboard-end`
  renders, text contains "0.91", `data-pass="true"`.
- `Library.test.tsx > shows NO end badge for a scored NON-keyframe set (S17 B1 guard)` — a scored
  S16 set with `"end_ssim":null … "end_pass":null` and NO keyframe boundaries → `.storyboard-join`
  present, `.storyboard-end` is null (badge suppressed).
- `Library.test.tsx > an unscored end shows a dash, never zero` — keyframe set, `end_ssim:null` →
  "end —", `data-pass="unscored"`.
- Pre-S17 `joins_json` (cut fields only, no `end_*`): covered by `api.test.ts > listStoryboards maps
  snake_case rows and degrades to [] on failure` (a cut-only row parses; end fields → null). This test
  stayed green even under mutation U1, confirming a stored pre-S17 set still parses.

### Mutation checks (fresh scratch copy OUTSIDE the repo, `scratchpad/mut2`, deleted after)
- **U1** `parseJoinRecords` reads only `endSsim` (drops the `end_ssim` snake mapping) -> FAILED both
  `maps the S17 end-adherence fields snake_case → camelCase` AND `renders the end badge from a REAL
  snake_case joins_json`. KILLED — the snake→camel remap is non-vacuous.
- **U2** remove the `if (!isKeyframeSet) return null` gate in `endAdherenceSummary` -> FAILED
  `shows NO end badge for a scored NON-keyframe set`. KILLED — the keyframe-set gate is non-vacuous.
- **R1** `cut_threshold` returns `CONTINUITY_FLOOR` regardless -> FAILED
  `the_cut_score_reads_the_ideal_tier_for_a_keyframe_set_and_the_floor_otherwise` ("keyframe → ideal",
  chain.rs:1581). KILLED — the ideal/floor tier selection is non-vacuous.
- **R2** `end_frame_capable` reverted to `routes.first()` -> FAILED
  `end_frame_capability_reads_any_route_not_just_the_first` ("a later route binds End → capable",
  chain.rs:1644). KILLED — the any-route capability is non-vacuous.
- **R3** `validate_keyframes` runnable = `use_case::supports(TextToImage)` only (route::resolve
  removed) -> **SURVIVED**: `a_still_model_without_a_runnable_route_is_rejected` still passed.
  Root cause: `use_case::supports` -> `route_serves` already checks `route.provider.has_adapter()`
  (use_case.rs:206), and `z_image`'s only route is `local:z-image` with no adapter, so
  `supports(z_image, TextToImage)` is ALREADY `false` — the model is rejected by `supports` before the
  added `route::resolve` line runs. The behavioral requirement (a still model with no runnable route is
  refused; a routable-but-unpriced GPT tier is accepted) IS enforced and the test asserts it correctly;
  but the test does NOT isolate the repair's new `route::resolve` line, and that line appears redundant
  (any model passing `supports` already has an adapter-backed serving route that `resolve` will find).
  Non-blocking: no money/correctness regression, the AC holds; recommend a follow-up test with a model
  that passes `supports` yet fails `resolve` if such a case is ever reachable.

### Money safety at this HEAD
- `git show 5b353c0` introduces NO `remove_var` / `strip_provider_env` / empty-key `set_var` (grep of
  the diff's added lines: none). Tree-wide `remove_var` still only in vault.rs guard tests. vault.rs is
  untouched by the repair — `cfg(test)` keychain block intact.
- The 950-test workspace run was executed with every external host forced through the dead proxy
  (localhost exempt) and stayed green -> zero provider reach still holds.

### Findings at re-test
- R3 survivor (validator route::resolve line redundant / its test doesn't isolate it) — non-blocking,
  documented above; the requirement is still enforced by `use_case::supports`.
- Carried forward from the first pass (unchanged by the repair): the M2-class self-skip in
  `a_no_end_frame_model_falls_back...`; AC2 still-submit loop / `image_size` seam tested below the full
  live submit (money-rule inherent); `imageModelAvailable` default `true`; `tauri build` not re-run.

The B1 defect the reviewer found (end-adherence badge never rendering on real snake_case data, fixtures
bypassing the wire mapping) is fixed and now guarded by production-path tests that fail under mutation.
The four behavioral changes (snake→camel mapping, keyframe-set badge gate, ideal-tier cut gating,
any-route capability) are non-vacuously tested. Gate is green; zero provider reach re-confirmed.

VERDICT: PASS
