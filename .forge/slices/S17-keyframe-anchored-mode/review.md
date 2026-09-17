# S17-keyframe-anchored-mode — REVIEW (evidence)

Read-only production-standards review of the slice diff (`git diff main...HEAD`, HEAD b3939cd).
Scope: `vault.rs`, `commands.rs`, `chain.rs`, `store.rs`, `storyboard.rs`, `fal_schema.rs`,
`corpus.rs`/`storyboard.v3.md`, `app.rs`, `lib.rs`, and the UI (`App.tsx`, `Storyboard.tsx`,
`Library.tsx`, `api.ts`, `types.ts`, `mock.ts` + tests). No source changed.

Gate re-run myself (dummy keys + `HICKEYFIELD_NO_KEYCHAIN=1`, Node 24):
- `cargo test -p hickeyfield-core --lib storyboard` → 39 passed / 0 failed.
- `cargo test -p hickeyfield-tauri --lib` → 183 passed / 0 failed / 3 ignored.
- `cargo test -p hickeyfield-tauri --lib chain -- --ignored` → 2 passed (incl.
  `end_adherence_reads_the_actual_last_frame` on the REAL ffmpeg sidecar).
- `cd ui && pnpm test` → 23 files, 309 passed / 0 failed.
Consistent with the implementer's claimed counts. I did not re-run the full `--workspace`, clippy,
or `tauri build`; TEST owns those.

---

## Blocking findings

### B1 (MAJOR, critical path) — the end-adherence score never renders in the real UI: snake/camel mismatch, masked by camelCase test fixtures
- `src-tauri/src/store.rs:22` — `JoinRecord` has **no** `#[serde(rename_all = "camelCase")]`, so
  Rust serialises the new fields as `end_ssim`, `end_psnr`, `end_dy`, `end_du`, `end_dv`,
  `end_pass` (snake_case) into `joins_json`.
- `ui/src/api.ts:838` — `listStoryboards` parses joins with a raw cast:
  `joins = JSON.parse(joinsJson) as JoinRecord[]` — **no** snake→camel remap (unlike
  `parseKeyframeRecords` at `api.ts:866`, which defensively reads both).
- `ui/src/components/Library.tsx:44` — `endAdherenceSummary` gates on
  `all.some((j) => j.endSsim !== undefined)` and reads `j.endSsim` / `j.endPass` (camelCase).
  Against real Rust data the key is `end_ssim`, so `j.endSsim` is always `undefined`,
  `hasEndField` is always `false`, and the function returns `null` — the badge is **never**
  rendered, not even the "end —" unscored state.
- Verified empirically: `JSON.parse('[{...,"end_ssim":0.91,"end_pass":true}]')[0].endSsim`
  → `undefined`; `hasEndField` → `false`.
- Why tests are green: `ui/src/components/Library.test.tsx:324,369` build fixtures with
  camelCase `endSsim`/`endPass` and pass them straight to the component, bypassing the
  `api.ts` `JSON.parse` path — so the tests assert a shape the production path never produces.
- Standard violated: a delivered acceptance criterion (AC4/AC5 — "end-adherence recorded AND
  shown, both scores per join in the Library") is non-functional in production, and the test
  suite gives false confidence (classic AI-code tell: fixtures diverge from the real wire shape).
  The cut-score half (`joinSummary`, single-word fields) works; only the S17 end half is dead.
- Concrete fix (pick one): (a) add a `parseJoinRecords(json)` in `api.ts` that maps
  `end_ssim→endSsim … end_pass→endPass` (mirroring `parseKeyframeRecords`) and call it at
  `api.ts:838`; or (b) have `Library.tsx` read the snake keys. Do **not** simply add
  `#[serde(rename_all="camelCase")]` to the Rust `JoinRecord` without an `alias` — that would
  break Rust's own `parse_joins` round-trip of already-stored snake_case `joins_json`. Add a
  test that drives an end score through the real `listStoryboards`/`JSON.parse` mapping, not a
  hand-built camelCase object, so the regression can't reappear green.

---

## Non-blocking findings

### M1 (minor) — end-frame capability is decided on `routes.first()`, but the submit resolves a route via `Cheapest`
- `crates/hickeyfield-core/src/storyboard.rs:604` (`shot.end_frame_capable = model.routes.first()…can_bind(End)`)
  and `src-tauri/src/chain.rs` `end_frame_capable(model)` both inspect only `model.routes.first()`.
- `submit_one` resolves the actual route via `route::resolve(…, RoutePolicy::Cheapest, …)`
  (`commands.rs`, per design §0.1). For a multi-route model whose first route and cheapest route
  differ in end-frame capability, the toggle/advisory decision can disagree with the route
  actually submitted — either suppressing a usable end anchor or (the S9 harm the design flags as
  the AC3 heart) attaching an end frame to a route that drops it. Benign today (H3 Max is
  effectively single-route), but latent. Fix: gate on the same route `submit_one` will resolve,
  or on "any route can_bind(End)".

### M2 (minor) — `validate_keyframes` dropped the design's priced-route check
- `crates/hickeyfield-core/src/storyboard.rs` `validate_keyframes` accepts a still model on
  `use_case::supports(m, TextToImage)` alone. Design D1 specified "runnable **and** with a priced
  fal route (`route::resolve(Cheapest).is_ok()`)". A TextToImage model without a priced route now
  passes the validator. Not money-unsafe (the gate honestly shows the still as "unpriced" and
  `submit_one` still prices before spending), but it is a divergence from the stated contract.

### M3 (minor) — keyframe cut score still gated against the FLOOR, not the IDEAL
- `src-tauri/src/chain.rs` `score_join`: `pass = cut … join_passes(&cut, CONTINUITY_FLOOR)`.
  Design D4/Q7 says keyframe-mode joins are the 99% path and the cut should read the IDEAL tier.
  Only `end_pass` uses `CONTINUITY_IDEAL`. `should_auto_retry` therefore triggers a cut retry at
  the floor, not the ideal. Behaviourally safe (never over-retries), but contradicts the design's
  stated tier for the cut half; reconcile the code or the design note.

### M4 (minor) — AC2's real generate branch is not exercised end-to-end
- `src-tauri/src/commands.rs:~3202` `keyframe_generate_holds_the_shots_and_stores_the_boundaries`
  reproduces the derive→validate→price→store block instead of calling
  `generate_storyboard_inner`, because qwen→fal would POST on the live-network sandbox
  (documented, money-safe). The consequence: the actual keyframe branch of
  `generate_storyboard_inner` (the still-submit loop, writing `job_set_id` back into each record,
  and returning all shots `pending`) has **no** direct test. The downstream barrier
  (`chain.rs`) and the store round-trip are covered, so the gap is narrow, but it is a real
  coverage hole on AC2. Recommend a fake-runner/fake-client seam (see question (a)) so this path
  can be driven without a network.

### M5 (minor) — duplicated derivation logic in JS
- `ui/src/components/Storyboard.tsx:761` `keyframeStillText` reimplements `opening_clause` /
  `ending_hold_clause` (`storyboard.rs:697/711`) in JavaScript for the preview cards. Preview-only
  (the server derivation is authoritative), but two copies of the same parsing rule drift over
  time. Acceptable as debt; note it.

### M6 (minor) — `stillEstimate === null` conflates "unpriced model" with "transient failure"
- `ui/src/App.tsx:556-575`: a failed `estimateCost` (no key, transient error) sets
  `stillEstimate = null`, which `Storyboard.tsx:264` folds into the total as "unpriced" — the same
  signal a genuinely unpriced GPT tier produces. For the priced default (qwen, $0.04) a transient
  failure would mislabel the total. Server-side pricing is still correct and gates the spend, so
  this is display accuracy only.

### M7 (doc drift, not code) — canvas example
- `design.md` §D1/Q1 says 768p×16:9 → `1360×768`; the implementation's `keyframe_canvas` (via
  `width_for`) yields `1366×768` (asserted in `commands.rs` test). Internally consistent
  (derive and validate share `keyframe_canvas`); only the design prose is stale.

---

## Checked and found clean (silence is not clearance)

- **Keychain money guard (`vault.rs:38-73`).** `get()` is the single read choke point; `entry()`
  / `get_password()` (`:52-73`) are reached only from `get()`, and `keychain_blocked()` short-
  circuits to `None` before them (`grep` confirms no other `get_password`/`keyring::Entry` call
  site in the tree). `cfg!(test)` covers in-crate unit tests; `HICKEYFIELD_NO_KEYCHAIN` covers the
  cases where `cfg(test)` is *false* — integration tests, examples, and the built binary all
  compile the lib without `cfg(test)`. So the `cfg!(test)` arm alone is **not** sufficient for
  those; safety there rests entirely on the mandated env var being set. That is the current
  session discipline and the two guard tests pass, but the compiler does not enforce it — worth a
  standing note (and is why every agent command must keep exporting the flag). No
  `strip_provider_env`/`remove_var` on a provider key remains in any test (only the vault guard
  tests use `remove_var`, as intended).
- **Store migration 11 (`store.rs:396`).** Additive nullable `keyframes_json`; INSERT/`ON
  CONFLICT`/`row_to_storyboard`/v1-replay all updated consistently; `migration_11_adds_keyframes_
  json_and_roundtrips` and `a_pre_s17_join_record_deserialises_with_absent_end_fields` prove the
  version bump, round-trip, and backward-compatible deserialise. `KeyframeRecord`/`JoinRecord`
  end-* are `Option` + `#[serde(default)]` → no data migration needed. Clean.
- **Barrier exactly-once + resume idempotence (`chain.rs`).** `stills_ready_for_shot_zero` gates
  `submit_shot_zero` on "no shot-0 row exists"; `keyframes_advance` early-returns when
  `settled.shot_index.is_some()`; `advance`/`score_join` early-return for a still
  (`shot_index == None`), so a settling still never trips the shot chain, and the hook and
  `resume_all` cannot double-submit shot 0. Verified by the passing chain tests.
- **Both-frame binding + `NO_END_FRAME` fallback (`chain.rs` `prepare_next_shot_keyframe`).** End
  frame attached only when `end_frame_capable`; otherwise start-only with the advisory
  `"end-frame anchoring unavailable: …"`. `fal_schema.rs` adds keyless proof that a
  `{width,height}` `image_size` lands verbatim on the qwen schema and is swept on an endpoint
  that does not declare it (the S9 harm) — both pass.
- **End-adherence scoring machinery.** `end_adherence_reads_the_actual_last_frame` passes on the
  real ffmpeg sidecar; `score_join` extracts the last shot's own last frame, scores vs `K_{i+1}`,
  cleans up the scratch, and records `None` (never 0) on failure. The *scoring* is sound — only
  the *display* is broken (B1).
- **Separate retry budgets & the total-shown invariant.** `KEYFRAME_RETRY_CAP` (`chain.rs`) is
  distinct from `AUTO_RETRY_CAP`; `regenerate_keyframe_still_inner` refuses an in-flight still
  (`"still in flight"`), bumps its own `attempt`, and routes every regenerate through `submit_one`
  (priced before submit). `a_still_regenerate_in_flight_is_refused` passes.
- **Money gate (AC6).** Stills priced per boundary into `keyframes_json.estimated_usd` and summed
  with the shot estimates before Generate; an unpriced model marks the whole total "unpriced",
  never $0 — the S10/S16 rule, upheld.
- **No dead code / no invented APIs.** New Rust items are all wired (`keyframes_advance` into
  `app.rs:185`, `regenerate_keyframe_still` into `lib.rs:87`). `prepare_next_shot` was renamed to
  `prepare_next_shot_keyframe` and all call sites updated; no stale name remains.
- **CHANGELOG** entry present under `[Unreleased] > Added`, wording matches the shipped behaviour
  (stills priced into the total, both-end anchoring, park-on-failed-still, separate ≤2 bounds).
- **Weakened assertions:** `git diff main...HEAD -- '*test*'` shows **no** removed
  `assert`/`expect`; the `prepare_next_shot` test edits are equivalent (`adv.is_none()` →
  `advs.is_empty()`, etc.), not weakenings.

---

## Answers to the launcher's questions
- (a) Test-suite money safety: "pin an unavailable route" is robust *today* but relies on the
  route staying unresolvable AND the env being set; it is not a structural seam. There is no fake
  provider-client injection point — `submit_one` calls the real `runner`/client. Recommend a
  follow-up slice adding a trait-object provider seam so the generate/submit paths (M4) can be
  driven offline without depending on an unresolvable route. Acceptable as a follow-up.
- (b) Barrier + resume vs the S15 hook: correct and idempotent (see clean list).
- (c) Both-frame binding incl. `NO_END_FRAME` advisory: correct, with the M1 caveat that
  capability is judged on `routes.first()` not the resolved route.
- (d) End-adherence storage additive + versioned: yes (store.rs, backward-compatible).
- (e) Money gate / budgets / regenerate-through-gate: correct.
- (f) `imageModelAvailable` default-true: acceptable — qwen is always registered, and the toggle
  still disables per-shot on end-frame capability; documented limitation.
- (g) Dead code/contradictions/stale counts: none blocking; doc drift only (M7); the headline
  display defect is B1.

The one blocking issue:
- **B1 — the end-adherence score (AC4/AC5's "second badge per join") never renders in the real
  Library because Rust emits `end_ssim`/`end_pass` (snake_case) and the UI reads `endSsim`/
  `endPass` (camelCase) through a raw `JSON.parse` cast; the tests pass only because their
  fixtures use camelCase and bypass the real parse path.**

VERDICT: FAIL
