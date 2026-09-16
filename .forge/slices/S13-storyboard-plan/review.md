# S13-storyboard-plan — REVIEW (evidence)

Independent, read-only production-standards review of the S13 slice diff
(`git diff main...HEAD`, HEAD 7452861). No source, docs, or backlog changed. The
full gate was re-run locally (results at the end), so the counts below are
measured, not asserted.

## What I re-ran (measured)

- `cargo test --workspace` (dummy `hickeyfield_FAL_KEY=x FAL_KEY=x`): **841 passed, 0 failed**, 1 ignored (≥ 820 floor). No network hit.
- `cargo fmt --all --check`: clean (exit 0).
- `cargo clippy --workspace --all-targets -- -D warnings`: clean (0 warnings).
- `./scripts/lint-provenance.py`: PASS (CDN + copy provenance, 80015 shingles).
- keyless `audit_fal` (env-unset): PINNED 0, UNREACHABLE 0 — unchanged.
- `cd ui && pnpm test`: **281 passed** (23 files, ≥ 272 floor). `pnpm build`: built, dist emitted.
- Not re-run: `tauri build`/launch (8-min bundle; unchanged from S12 tooling, no verdict rests on it).

The regression gate (AC7) is green. The blockers below are correctness/error-handling
defects that the gate does not exercise, because the one thing S13 adds — the
generate-all fan-out — has no happy-path test (the implementer flagged this).

---

## Blocker

### B1 — Generate-all collapses same-model shots into one job (silent data loss on the money path)
`src-tauri/src/commands.rs:1019` (id minting, inside `submit_one`) reached from the
loop at `src-tauri/src/commands.rs:1240` (`generate_storyboard_inner`).

Every job id is `format!("{}-{}", now_secs(), model.id)` — second-granularity time
plus the model id, with no per-submission disambiguator (`now_secs()` is whole
seconds, `runner.rs:35`). `generate_storyboard_inner` calls `submit_one` for every
shot in a tight loop; each shot has `media: []`, so `submit_to_provider` is a single
fast enqueue POST that returns well inside one second. Two shots that share a model
therefore mint the **same** id in the same wall-clock second, and `store.upsert`
uses `ON CONFLICT(id) DO UPDATE` (`store.rs:438`) — so the second shot **overwrites
the first shot's row**. A 4-shot storyboard on one model (the common, corpus-
encouraged case — `prompts/storyboard.v1.md` says "repeat a model across shots when
it fits", and the module's own `good_json` fixture uses one model for all four
shots) collapses to as few as one row.

Standard violated: correctness / money honesty. AC4 requires "submits every shot as
jobs linked in one JobSet"; AC6 requires the user gets what they paid for. Here the
user is billed for N generations (each POST fires) but the Library shows one, and the
survivor is a Frankenstein row — `storyboard_id`/`shot_index` are write-once so it
keeps shot 0's index, while `status`/`enhanced_prompt`/`results` are overwritten by
the last writer.

Why the gate misses it: the only offline generate test
(`generate_records_the_set_even_when_shots_fail`) uses unknown model ids that fail at
the registry lookup **before** id minting, and `jobs_in_a_storyboard_are_ordered_by_shot_index`
injects hand-made ids (`"a"/"b"/"c"`). Neither exercises real id minting under the
loop. This path was never a loop caller before S13 (`submit_job` is one click = one
call), so S13 introduces the collision.

Concrete fix: make the per-shot id unique — e.g. incorporate `shot_index`/`storyboard_id`
(both already passed into `submit_one`) into the id when part of a set, or add a
monotonic/random suffix to the id generally. Then add a store test that submits ≥2
same-model shots and asserts distinct rows survive with dense `shot_index`.

## Major

### M1 — Per-shot failures are swallowed by the UI (partial failure rendered as success)
`ui/src/components/Storyboard.tsx:198-225` (`generate`) and `ui/src/App.tsx:558`
(`onStoryboardGenerated`).

The backend deliberately returns a per-shot `ShotOutcome { shot_index, job_set_id,
error }` (`commands.rs` `StoryboardResult`) precisely so the UI can tell the user
which shots failed. The UI throws it away: `generate()` calls `onGenerated(result)`
without inspecting `result.shots`, and `onStoryboardGenerated((_result) => …)`
ignores the argument entirely and just switches to the Library. `generate()`'s
`catch` only fires if the whole invoke rejects; a resolved result carrying refusals /
missing-key / submit errors on individual shots is silent.

Consequence: a storyboard where shot 3 is refused or errors lands the user in the
Library seeing only the shots that started, with **no indication** that a shot failed
and no way to retry only the failed ones (question (e)). This is the "failures
rendered as empty/success" standard, on the AC6 money-honesty path — the batch's
whole reason for returning per-shot outcomes is defeated.

Concrete fix: in `generate()` (or `onGenerated`), surface `result.shots.filter(s =>
s.error)` — at minimum a message naming the failed shot indices and their reasons;
ideally keep the failed shots in the view so the user can retry just those. Add a
`Storyboard.test.tsx` case where `onGenerate` resolves with a mixed result and assert
the failure is shown.

## Minor

### m1 — Storyboard id can also collide across two generate-alls
`generate_storyboard_inner` mints `sid = format!("sb-{}-{}", now_secs(), input.shots.len())`
(`commands.rs:~1226`). Two generate-alls with the same shot count in the same second
produce the same `sid`; `upsert_storyboard` is `ON CONFLICT DO UPDATE`, so the second
overwrites the first's brief/plan and both batches' jobs group under one header. Low
likelihood given the flow (the view switches to Library after generate, so a human
cannot re-trigger within a second), but it is the same non-unique-id smell as B1 and
should be fixed with the same disambiguator.

### m2 — Open preview is not reset on reorder/move
`ui/src/components/Storyboard.tsx:157` (`moveShot`) resets `finalPrompts` but leaves
`previewShot`/`preview`/`previewText` intact. If a shot's inline preview is open and
the user reorders, the panel keeps showing the old shot's `preview` content bound to
an index that now holds a different shot, and `acceptPreview(i)` would write the stale
text to the wrong shot's `finalPrompts[i]`. Reset `previewShot` to `null` in
`moveShot`/`removeShot` (as `draft` already does). Combined with index-as-key
(`key={i}`, line 294) this is the main reorder-correctness gap; the index key itself
is acceptable since `Shot` has no stable id and all fields are controlled.

### m3 — Clearing the target length silently guarantees rejection
`ui/src/components/Storyboard.tsx:270`: `Number(e.currentTarget.value) || 0` lets
`targetSeconds` become 0. `validate` then computes tolerance `0.10 * 0 == 0`, so any
non-zero-sum plan is rejected as `DurationSum` (`storyboard.rs:287`). Not a crash and
the error is honest, but a 0 target is a confusing dead end; clamp the input to its
`min={2}` or guard drafting on `targetSeconds > 0`.

---

## Checked and found clean (silence is not clearance)

- **`submit_one` factoring is byte-for-byte the old `submit_job` body** (diff
  reviewed line by line): error mapping, `configured_providers`/`ProviderId` filter,
  `compiled_for_submit` (S6 guard + S7 verbatim `final_prompt`), `route::resolve` with
  `RoutePolicy::Cheapest`, `estimate_with_provenance` + `priced_from`/`priced_at`
  write-once, the `JobSet` literal, both `upsert`s, `submit_to_provider`, and
  `runner.watch`. The only diffs are the two new `storyboard_id`/`shot_index` fields,
  `None`/`None` on the standalone path, and `&input`. Single-job submit behaviour is
  genuinely unchanged (existing tests still pass). Note this does **not** clear B1,
  which is about the *new loop caller*, not the factored body.
- **Chat seam reuses the wire plumbing, no duplication.** `ChatBackend::complete`
  (`enhancer.rs`) calls the existing private `client`/`send_json`/`ollama_body`/
  `openai_body`/`anthropic_body` + text readers + `check_ollama_read_everything`, with
  the same URL construction (`OPENAI_CHAT_URL`/`PATH`, `ANTHROPIC_*`, `ANTHROPIC_VERSION`)
  and the same `LOCAL_TIMEOUT`/`HOSTED_TIMEOUT` as `HostedEnhancer::rewrite`
  (lines 1362-1390) and `LocalEnhancer::rewrite` (1069-1077). No new HTTP client, no
  new request shape. Errors are typed `JobError::Permanent` (`chat_err`) so a bad
  draft is surfaced, not silently retried in a loop. `send_json`/`client` emit no logs
  and the prompt/brief never touch a logging call, so no brief leakage.
- **Validator robustness / taxonomy.** `refusal_reason` runs before parse so a decline
  is `Refused`, not `NotJson`; `clean_reply` unwraps a ```` ```json ```` fence/quote
  pair, then strict `serde_json::from_str`. Trailing prose → serde trailing-token error
  → `NotJson`; numbers-as-strings (`"seconds":"8"`) → serde type error → `NotJson`;
  unicode is fine (owned `String`). `PlanError` is a closed enum with a `Display` per
  variant carrying shot index / value — stable and actionable for the UI. Camera `None`
  allowed, unknown slug rejected (`camera::get`, no nearest-guess); count `3..=8`; sum
  ±10% on *proposed* seconds. The post-clamp sum can drift from the target — documented
  (design §10.4) and acceptable for a plan the user then edits.
- **`chat_backend_for` mirrors `select_rewriter` exactly** (priority + `NOTE_*` prose,
  lines 762-799 vs 1098-1123), including auto-with-hosted-key-only returning
  `NOTE_NO_MODEL_CHOSEN` — intentional consistency with the enhancer, not a bug. Tested
  for auto/nothing/explicit-openai.
- **Migration 8** is additive and safe on existing v7 DBs: `CREATE TABLE IF NOT EXISTS
  storyboards` + two nullable `ALTER TABLE job_sets ADD COLUMN`, then `PRAGMA
  user_version = 8`; the v1-upgrade downgrade drop-list and the version assertion (7→8)
  are updated. Write-once columns are absent from the `ON CONFLICT UPDATE SET` and the
  behaviour is tested (`migration_8_..._roundtrips`). `row_to_job` reads by column name,
  so `jobs_in_storyboard`'s `SELECT *` is order-safe. `list_storyboards` is a single
  `ORDER BY created_at DESC` query — no N+1; the Library groups client-side off the
  already-loaded job list, so no extra per-set queries.
- **`clamp_shot`**: enumerated axis snaps to nearest offered value with an advisory;
  free-form/unknown axis left as-is (Seedance 30s survives); no-duration still left
  untouched and silent. Uses the catalogue-only `|_| None` capability source (S9-honest,
  offline). Tested LTX 12→10 and Seedance 30.
- **Estimate effect race** (`Storyboard.tsx:93-105`) is guarded by a monotonic
  `estimateToken`, so an out-of-order async resolution can't overwrite a newer estimate.
  The bridge handlers (`storyPlan`/`storyEstimate`/`storyPreview`/`onStoryboardGenerated`)
  are `useCallback`-memoized in `App.tsx`, so the effect doesn't loop per render.
- **Cost honesty** (`lib/cost.ts` `totalCostCaption`): an unpriced shot is counted
  separately and named, never summed as $0; all-unpriced reads "unpriced" (tone amber),
  never "$0.00". Built on the same `usable`/`formatUsd` as `stageCostCaption`. Tested.
- **Accessibility**: move/delete buttons have `aria-label`s; every shot input/textarea/
  select is labelled; the picker overlay is reused unchanged. Generate-all is disabled
  until the summed total renders (`canGenerate`), honouring "total before submit".
- **No invented APIs / dead-code / stale-count tells**: all referenced enhancer wire
  fns and constants exist with matching signatures; `store::storyboard`/`jobs_in_storyboard`
  carry honest `#[allow(dead_code)]` notes as the S14 surface; corpus token-budget test
  and CHANGELOG/README wording are original (provenance PASS). The recipe/runner/app
  `JobSet` literals gained the two new fields (compile-only) — verified they build.

---

## Blocking findings (must clear before merge)

- **B1** — generate-all collapses same-model shots into one job via a non-unique job id
  (`commands.rs:1019` + loop `:1240`). Silent data loss on the paid path; breaks AC4.
- **M1** — the UI swallows per-shot failures (`Storyboard.tsx:198-225`, `App.tsx:558`);
  a partial-failure batch is rendered as success with no report and no retry-failed.

Both sit on the AC4/AC6 critical path and are untested. Fix, add tests that exercise
the happy/partial-failure fan-out (using unknown-model or a fake submit so no money is
spent), and re-review.

VERDICT: FAIL

---

## RE-REVIEW (HEAD 64f29b9)

Read-only re-review of the repair-1 delta (`git show 64f29b9`) against the prior
findings, plus the whole slice diff for regressions. No source, docs, or backlog
changed. The full gate was re-run locally (below); counts are measured.

### Gate re-run (measured, dummy `hickeyfield_FAL_KEY=x FAL_KEY=x`, Node 24)
- `cargo fmt --all --check`: clean (exit 0).
- `cargo clippy --workspace --all-targets -- -D warnings`: clean (0 warnings).
- `cargo test --workspace`: **848 passed, 0 failed**, 1+ ignored (≥ 841 floor). No network.
- `cd ui && pnpm test`: **285 passed** (23 files, ≥ 281 floor). Matches implement.md.
- `tauri build` not re-run (no native/tooling change this round — Rust + TS + CSS only);
  no verdict rests on it. No hosted/fal call, no money.

### Prior findings — verdicts

- **B1 (job-id collision) — FIXED, verified.** `mint_job_id` = `{secs}-{model}-{seq}`
  off a process-wide `static ID_SEQ: AtomicU64` (`commands.rs` top). `fetch_add(…, Relaxed)`
  is an atomic RMW, so every caller — any thread, the generate-all loop, or the single-job
  path — gets a distinct seq; uniqueness holds regardless of clock granularity (Relaxed is
  correct here: we need atomicity of the increment, not cross-variable ordering). `submit_one`
  mints through it (single-job path benefits too). Id-shape change is safe for every consumer:
  grepped the whole tree — **no code parses a job id** (the only `-`/`:` splits are
  `browser.ts categoryLabel`, `catalog.rs`, `clients.rs` path/ext, `enhance.rs`, none touching
  a job id; runner/engine use `job.id` as an opaque key: `runner.rs:113,130,743`,
  `watching.insert`/map key). UI React keys stay unique. Tests
  `minted_job_ids_do_not_collide_within_one_second` (200 distinct + two immediate mints differ)
  and `same_model_shots_survive_as_distinct_rows_in_the_store` (3 same-model rows, dense
  `shot_index [0,1,2]`, no `ON CONFLICT` clobber) drive the real production `mint_job_id` + real
  store. Relaunch: the counter resets to 0 but the id also carries `now_secs()`; a cross-run
  collision needs a relaunch inside the same wall-clock second *and* the same model submitted in
  that second — infeasible for an app restart, and strictly better than the pre-S13
  `{secs}-{model}` id (zero disambiguation). Acceptable residual, no action.

- **M1 (per-shot failures swallowed) — SURFACING FIXED; RETRY BROKEN (see M2 below).**
  The surfacing half is fixed and verified: `generate()` inspects `result.shots`, keeps the
  result on partial failure (does NOT hand off), and renders `.storyboard-outcome`
  (`role="alert"`) naming each failed `shot_index+1` and its `error`, with a retry total shown
  before any resubmit. `App.tsx:558 onStoryboardGenerated` is now only reached on full success,
  so its switch-to-Library is correct. That satisfies M1's minimum ("surface which shots
  failed"). The retry-only-failed half, however, is broken in production — new **M2**.

- **m1 (storyboard-id collision) — FIXED.** `mint_storyboard_id()` = `sb-{secs}-{seq}` off the
  same counter; asserted distinct in `minted_job_ids_do_not_collide_within_one_second`.
- **m2 (stale preview on reorder) — FIXED.** `closePreview()` resets `previewShot`/`preview`/
  `previewText` and is called from `moveShot`, `removeShot`, and `draft`; vitest
  `reordering closes an open prompt preview` proves the panel is gone after a move.
- **m3 (cleared target) — FIXED.** `targetInvalid = !(targetSeconds > 0)` renders
  `.storyboard-target-invalid` (`role="alert"`) and disables Draft; vitest covers it.

### Server-side re-validation (question c) — correct in isolation, but see M2
`generate_storyboard_inner` now, before minting: caps `brief` (4 KiB) and `plan_json` (64 KiB),
rejects empty / `> MAX_SHOTS (8)` shot lists, and — when `plan_json` is present — parses it,
resolves `plan.use_case`, re-runs the **same** `hickeyfield_core::storyboard::validate(&plan.shots,
plan.target_seconds, uc)` the draft path uses, then requires the submitted list to match the plan
by length and per-index `model_id`. It deliberately does NOT compare prompt/seconds, so a user's
legitimate prompt/duration edits are allowed (the plan_json is rebuilt from the current shots, so
the match check mainly defends a direct/tampered caller) — not brittle. Caps are typed `Err(String)`
and tested (`generate_rejects_a_brief_over_the_size_cap`, `…plan_json_over_the_size_cap`,
`…more_than_eight_shots`, `…a_plan_with_a_bad_model` asserting nothing was written,
`…shots_that_do_not_match_the_plan`). L1/L2/L3 all addressed. **But** `validate` also enforces the
`3..=8` shot-count bound and the ±10% duration-sum gate — and that is exactly what breaks retry:

### NEW — M2 (Major): retry-failed is rejected by the same-commit server re-validation
`ui/src/components/Storyboard.tsx` `retryFailed`/`submitBatch` (lines ~223–320) vs
`src-tauri/src/commands.rs` `generate_storyboard_inner` (the new `plan_json` re-validation block).

`retryFailed()` resubmits only the failed shots via `submitBatch(failedIndices)`. `submitBatch`
always builds `planJson = JSON.stringify({ brief, targetSeconds, useCase, shots: batch })` where
`batch` is just the failed subset, and `targetSeconds` is the **full** target (e.g. 30).
`generateStoryboard` (`api.ts:716`) `invoke`s `generate_storyboard`; a Rust `Err` rejects the
promise, which `submitBatch`'s `await` throws and `retryFailed`'s `catch` turns into `setError`.

On retry, the server sees `plan_json` present and runs `validate(&plan.shots /* 1–2 shots */,
30.0, uc)`:
- `!(3..=8).contains(&len)` → `PlanError::ShotCount` whenever fewer than 3 shots failed — the
  dominant partial-failure case (e.g. the repair's own test: 1 of 4 shots refused);
- even with 3+ failed shots, their seconds-sum is essentially never within ±10% of the full
  30s target → `PlanError::DurationSum`.

So a "Retry N failed" click is rejected server-side with a confusing, unrelated error
("a storyboard has at most…"/"…3..8…"/duration-sum) and **no shot is resubmitted**. The
retry-only-failed feature — the explicit remedy the repair advertised for M1, and the thing
question (b) asks me to judge — does not work in production for the common case. Standard
violated: correctness / feature does what its comment and its test claim it does. It is an
AI-code tell that the two repairs in one commit contradict each other and the gap is
masked by a mock. No money is lost (validation is pre-loop, so nothing double-submits), so this
is a Major, not a Blocker — but it is on the AC6 retry path and is untested against reality.

Why the gate misses it: `Storyboard.test.tsx` "a partial failure shows the failed shots and
retry submits only those" stubs `onGenerate` with a `vi.fn` that ignores `planJson`/`targetSeconds`
and returns success for a 1-shot retry. The mock cannot see the real `generate_storyboard`
re-validation, so the test passes while the integrated path fails. There is no test that runs the
retry `plan_json` through the real `validate`.

Concrete fix (pick one, implementer's call): on the retry path, either omit `plan_json` (so the
server skips plan re-validation for a partial resubmit), or scope the server re-validation so a
strict subset of an already-validated plan is not re-count/re-sum-gated (e.g. carry a
storyboard/set id and validate against the stored plan, submitting only the named indices), or
send the full plan with a `retryIndices` field the server uses to submit just those. Then add a
Storyboard test whose `onGenerate` actually applies the same count/sum rule (or an integration
test) so a sub-3-shot retry is proven to succeed.

### Other checks (silence is not clearance)
- **No double-submit / state cleanup (b):** on partial failure `generate` does not call
  `onGenerated`; retry submits only `failedIndices`; `merged` replaces failed entries by index;
  full success clears `outcome` and hands off once. The "retry forms a new storyboard group"
  gap is real and documented — acceptable in isolation, but moot while M2 stands (retry can't
  reach the group at all).
- **Money-gate reuse:** `submit_one` body still byte-for-byte the old `submit_job` (re-diffed);
  both generate and retry go through it, so per-shot estimate + S6 guard + S7 verbatim prompt are
  unchanged. `retryTotal` = `totalCostCaption(failedIndices.map(i => estimates[i]))` — correct
  original-index estimates, shown before the retry button.
- **New constants / dead code / stale counts:** `MAX_BRIEF_BYTES`/`MAX_PLAN_JSON_BYTES`/`MAX_SHOTS`
  all referenced; `#[derive(Debug)]` added to `ShotOutcome`/`StoryboardResult` is used by the new
  `unwrap_err` tests, no secret in either. `implement.md`'s "848" and "285" match the measured
  gate. No CHANGELOG/README change in this delta; no comment rot except M2's contradiction.
- **Regression scan of the full slice diff:** the pre-existing
  `generate_records_the_set_even_when_shots_fail` was correctly switched to `plan_json: None`
  (it exercises per-shot continue-on-failure, now distinct from pre-loop re-validation) — a
  legitimate adjustment, not a weakened test.

### Blocking findings (must clear before merge)
- **M2 (Major, AC6 retry path)** — retry-failed is rejected by the same-commit `generate_storyboard`
  plan re-validation (`3..=8` count + ±10% sum) because `submitBatch` sends a subset `plan_json`
  with the full target; the feature is non-functional for the common partial-failure case and the
  contradiction is masked by a mocked test.

B1, m1, m2, m3, and the L1–L3 hardening are genuinely fixed. M1's surfacing is fixed; its retry
remedy is broken by M2.

VERDICT: FAIL
