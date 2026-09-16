# S13-storyboard-plan — TEST (evidence)

Author: adversarial verifier (a different agent than the implementer). Every
output below was captured first-hand on branch `forge/s13-storyboard-plan`
(HEAD 64f29b9). I did not edit source, design, or `implement.md`; the only file
I wrote is this one. Mutation checks ran in a copy of the tree OUTSIDE the repo
(`…/scratchpad/mut`), never touching the repo working tree.

## Implementer's claim (verbatim, from implement.md)

> ### Gate — every command, observed output
> ```
> cargo fmt --all --check                         → FMT_CLEAN (exit 0)
> cargo clippy --workspace --all-targets -- -D warnings → Finished, 0 warnings
> cargo test --workspace                          → 704 + 139 + 3 + 2 = 848 passed, 0 failed, 12 ignored (>= 841)
> ./scripts/lint-provenance.py                    → CDN PASS, Copy provenance PASS (80015 shingles)
> env -u hickeyfield_FAL_KEY -u FAL_KEY cargo run -q -p hickeyfield-core --example audit_fal
>                                                 → PINNED 0, UNREACHABLE 0 (unchanged)
> cd ui && pnpm test                              → 23 files, 285 passed (>= 281)
> cd ui && pnpm build                             → tsc + vite ✓ built (dist emitted)
> ```
> Claim: 848 Rust / 285 UI.

Repair round 1 claims B1 (job-id collision) fixed via an `AtomicU64` counter in
`mint_job_id`/`mint_storyboard_id`; M1 (silent partial failure) fixed with
per-shot outcomes + "Retry failed"; server-side re-validation, `≤8` shots, brief
4 KiB / plan_json 64 KiB caps; preview reset on reorder; blank target blocks Draft.

## Commands (AC7 regression gate — re-run by me, real output)

`cargo test --workspace` (all eight dummy keys + `FAL_KEY=x`), exit 0:
```
Running unittests src/lib.rs (hickeyfield_core)
test result: ok. 704 passed; 0 failed; 10 ignored; 0 measured; 0 filtered out
Running tests/live_fal_pricing.rs
test result: ok. 0 passed; 0 failed; 1 ignored
Running unittests examples/audit_fal.rs
test result: ok. 3 passed; 0 failed; 0 ignored
Running unittests examples/fal_diff.rs
test result: ok. 2 passed; 0 failed; 0 ignored
Running unittests src/lib.rs (hickeyfield_tauri_lib)
test result: ok. 139 passed; 0 failed; 1 ignored
```
Total = 704 + 3 + 2 + 139 = **848 passed, 0 failed** (≥ 820 floor). Matches claim.

```
$ cargo fmt --all --check           → FMT_CLEAN exit=0
$ ./scripts/lint-provenance.py
CDN hostnames    PASS  no third-party media hosts referenced
Copy provenance  PASS  no shipped string matches the reference corpus (80015 shingles indexed)
$ env -u hickeyfield_FAL_KEY -u FAL_KEY cargo run -q -p hickeyfield-core --example audit_fal
=== PINNED FIELDS … (0) ===
=== UNREACHABLE … (0) ===
$ cargo clippy --workspace --all-targets -- -D warnings   → Finished (exit 0, 0 warnings)
$ cd ui && pnpm test    → Test Files 23 passed (23) / Tests 285 passed (285)   (≥ 272 floor)
$ cd ui && pnpm build   → tsc --noEmit && vite build → ✓ built in 1.80s (dist emitted)
```
Every gate item reproduced at the claimed numbers. `tauri build` (8-min bundle)
was NOT re-run — no native/tooling change this round; no verdict rests on it.

## Per-AC checks

- **AC1 (drafting + validator, no network):** all `storyboard.rs` validator tests
  green — `a_valid_plan_is_accepted`, `a_bad_model_id_is_rejected_with_a_reason`,
  `a_model_that_cannot_run_the_tab_is_rejected`, `a_sum_outside_ten_percent_is_rejected`,
  `a_refusal_is_a_typed_error_not_garbage`, `garbage_that_is_not_json_is_rejected`,
  `an_unknown_camera_is_rejected`, `a_backend_error_is_an_honest_plan_error`,
  `the_shot_count_bound_is_three_to_eight`. All use a `FakeChat` — no network.
- **AC2 (clamp):** `a_twelve_second_ltx_shot_clamps_to_ten_with_an_advisory` and
  `a_single_shot_seedance_thirty_survives` green.
- **AC3 (UI):** `Storyboard.test.tsx` 7 tests green (draft renders cards + edit
  updates total; unpriced marks total; empty brief blocks Draft; blank target
  blocks Draft; full success → Library; partial failure shows fails + retry-only;
  reorder closes preview).
- **AC4/AC6 (set relation, single-job unchanged, money):** store + command tests
  green (see below). Diff vs `main` confirms the ONLY id change is
  `format!("{}-{}", now_secs(), model.id)` → `mint_job_id(&model.id)`
  (adds a trailing `-{seq}`); `submit_job` just delegates to
  `submit_one(.., None, None)`. Nothing else in the submit body changed.
- **AC5 (continuity round-trip):** `a_plan_with_continuity_and_roles_round_trips_through_serde`
  and the migration-8 plan_json round-trip green.

## Break attempts

1. **B1 truly fixed — 3 same-model shots / two single submits in one second.**
   `minted_job_ids_do_not_collide_within_one_second` (200 same-model ids all
   distinct; two immediate single-job mints differ; two storyboard ids differ)
   and `same_model_shots_survive_as_distinct_rows_in_the_store` (3 same-model
   rows survive with dense `shot_index [0,1,2]`) both pass with the real
   production `mint_job_id`.
   Mutation (scratch copy, outside repo): reverted `mint_job_id` to
   `format!("{}-{}", now_secs(), model_id)` →
   ```
   test commands::tests::minted_job_ids_do_not_collide_within_one_second ... FAILED
   test commands::tests::same_model_shots_survive_as_distinct_rows_in_the_store ... FAILED
   test result: FAILED. 47 passed; 2 failed
   ```
   The tests genuinely pin the counter.
   **Nothing parses the job-id shape:** `grep` for `.split('-')` / id parsing in
   Rust finds only `route_id.split(':')` (runner.rs/app.rs) and, in the UI, one
   unrelated `split("-")` in `browser.ts` (a category-label formatter). The extra
   `-{seq}` segment is safe.

2. **M1 — partial failure rendered, retry submits only failed, total shown first.**
   `Storyboard.test.tsx` "a partial failure shows the failed shots and retry
   submits only those" asserts `.storyboard-outcome` renders the failed shot's
   error text, `onGenerated` is NOT called on partial failure, retry calls
   `onGenerate` a 2nd time with exactly 1 shot, and on full success the summary
   clears + `onGenerated` fires. "a full success switches to the Library" asserts
   `onGenerated` once and no summary.
   Mutation (scratch): made `generate()` always `onGenerated(result)` (the old
   swallow-bug) →
   ```
   × Storyboard > a partial failure shows the failed shots and retry submits only those
   Tests  1 failed | 6 passed (7)
   ```

3. **Validator taxonomy (refusal / not-JSON / bad model / unrunnable / unknown
   camera / bad sum / 2 shots).** All nine validator tests green; each asserts a
   specific typed `PlanError` variant, not a bare failure. Mutations:
   - widened `3..=8` → `0..=100`: `the_shot_count_bound_is_three_to_eight` FAILED.
   - removed the `shot.seconds = nearest` clamp:
     `a_twelve_second_ltx_shot_clamps_to_ten_with_an_advisory` FAILED.
   No panics in any path (unknown model, image model under video tab, garbage
   prose, refusal string, unknown camera slug all resolve to a typed error).

4. **Server-side generate bounds (L1/L3).** Green: `generate_rejects_more_than_eight_shots`,
   `generate_rejects_a_plan_with_a_bad_model` (asserts store is empty after),
   `generate_rejects_shots_that_do_not_match_the_plan`,
   `generate_rejects_a_brief_over_the_size_cap`, `generate_rejects_a_plan_json_over_the_size_cap`.
   Confirmed in source: empty-shots guard, `> MAX_SHOTS`, brief/plan byte caps,
   and `storyboard::validate` + per-index model match all run before the
   storyboard row is minted. Mutations:
   - `> MAX_SHOTS` → `> 10000`: `generate_rejects_more_than_eight_shots` FAILED.
   - removed the server-side `storyboard::validate(...)` call:
     `generate_rejects_a_plan_with_a_bad_model` FAILED.
   Caps use byte length (`String::len`) and reject at `+1` over the limit; the
   value exactly at the cap is allowed (correct boundary).

5. **Write-once set columns.** `migration_8_adds_storyboards_and_columns_and_roundtrips`
   asserts a later upsert with a different `storyboard_id`/`shot_index` does NOT
   re-tag while `status` still updates. Mutation: added
   `storyboard_id = excluded.storyboard_id` to the `ON CONFLICT DO UPDATE SET`
   →
   ```
   thread '…migration_8…' panicked: assertion `left == right` failed: storyboard_id is write-once
   test result: FAILED. 0 passed; 1 failed
   ```

6. **Single-job submit unchanged.** `git diff main...HEAD` on `commands.rs` shows
   the submit body's only change is the id call; all existing submit/preview
   tests are inside the green 139-test shell suite.

7. **Local Ollama JSON confidence check (free, local).** `localhost:11434`
   answered. Ran the shipped `prompts/storyboard.v1.md` system prompt + a
   cold-brew brief through `llama3.2:3b` three times; after stripping a fence,
   each reply parsed as `{"shots":[…]}` with a shot count in 3–8:
   ```
   run 1: PARSED, shots=8, in3to8=True
   run 2: PARSED, shots=6, in3to8=True
   run 3: PARSED, shots=7, in3to8=True
   ```
   3/3 JSON hit-rate on a small local model — the corpus's real-world confidence
   check the design asked for. No key, no money, no hosted call.

## Notes / accepted limitations (not defects)

- The successful generate-all loop is still not unit-tested offline (the router
  only yields fal/higgsfield, which would POST). The collision + store write it
  depends on ARE tested with the real `mint_job_id` + real store, and the submit
  body is diff-proven unchanged. This is disclosed, and no agent-run spends money.
- Retry mints a fresh storyboard group (retried shots land in a separate Library
  group). Disclosed in implement.md; out of slice scope; not an AC violation.
- L1's "total shown first" remains a UI invariant for the amounts; the server now
  bounds count/size/plan-match so a direct caller cannot submit an unbounded or
  off-plan batch. Consistent with the (PASS) security review.

All eight mutation checks (7 distinct new tests across core/shell/UI) flipped the
relevant test to red in a copy outside the repo, then were reverted — the new
tests are non-vacuous. The full gate reproduces at the claimed 848 Rust / 285 UI
with 0 failures, and B1/M1 and the L1–L3 hardening are genuinely fixed and pinned.

VERDICT: PASS

---

## RE-TEST (HEAD 7250335)

Repair 2 landed after a reviewer found **M2**: repair-1's "Retry failed" built
`plan_json` from ONLY the failed subset, so the server's round-1 re-validation
(`storyboard::validate`: 3..=8 shots, ±10% sum) rejected any retry of fewer than
3 shots — and the round-1 vitest passed only because its `vi.fn` mock ignored
`planJson`. I re-tested the delta `git show 7250335` first-hand. Same rules: no
key, no hosted call, no money; repo read-only apart from this file; mutations run
in a copy OUTSIDE the repo.

### Gate re-run at HEAD 7250335 (real output)

```
cargo test --workspace  → 704 (core) + 3 (audit_fal) + 2 (fal_diff) + 143 (shell) = 852 passed, 0 failed
cargo fmt --all --check  → FMT_CLEAN (exit 0)
cargo clippy --workspace --all-targets -- -D warnings → Finished (exit 0, 0 warnings)
./scripts/lint-provenance.py → CDN PASS, Copy provenance PASS (80015 shingles)
audit_fal (keys unset)   → PINNED 0, UNREACHABLE 0 (unchanged)
cd ui && pnpm test       → Test Files 23 passed (23) / Tests 285 passed (285)
cd ui && pnpm build      → tsc + vite ✓ built in 2.90s (dist emitted)
```
Shell suite rose 139 → 143 (+4 retry tests); total 848 → **852**, matching the
coordinator's report. `tauri build` not re-run (only Rust/TS/CSS logic changed).

### Retry path driven offline, exactly as the new Rust tests do

All four new `commands.rs` tests pass in a clean checkout:
```
a_retry_subset_plan_is_rejected_with_a_clear_error ... ok
a_retry_with_the_full_plan_targets_the_existing_group_and_only_the_failed_index ... ok
a_retry_of_an_already_started_shot_is_rejected ... ok
a_retry_shot_whose_duration_does_not_match_the_plan_is_rejected ... ok
```
Reading the code + tests confirms the four coordinator scenarios:
- **4-shot plan, index 2 failed → one shot into the SAME group at shot_index 2.**
  `a_retry_with_..._only_the_failed_index` asserts `out.storyboard_id == sid` (no
  fresh group), `out.shots.len() == 1`, `out.shots[0].shot_index == 2` (original
  index, not re-based 0), and that the 4 seeded rows are undisturbed. The single
  submit is driven to a deterministic offline error via the adapter-less Vaig
  route (no network POST), so no money path is exercised.
- **Retry with a subset plan rejected.** `a_retry_subset_plan_is_rejected...`
  sends a 1-shot `plan_json`; the server re-validates the whole plan and returns
  "…3 to 8 shots…", with `store.all().len() == 0` (nothing written).
- **Retry of an already-started index rejected.** `a_retry_of_an_already_started_shot`
  seeds a Queued (non-Failed) row at index 2; the retry is refused with "already
  started" BEFORE any submit — a started shot cannot be double-billed.
- **Duration mismatch rejected.** `a_retry_shot_whose_duration_does_not_match`
  submits 30s where the plan clamped index 2 to 5s; rejected with "clamped it to 5".

### Mutation checks — retry delta (scratch copy OUTSIDE the repo)

Each flipped its target test red, then was reverted:

1. Neuter the already-started guard (`j.shot_index == Some(idx)` → `Some(9999)`):
   `a_retry_of_an_already_started_shot_is_rejected ... FAILED`.
2. Neuter the duration pin (`if shot.settings.duration != Some(planned.seconds)`
   → `if false`): `a_retry_shot_whose_duration_does_not_match_the_plan_is_rejected
   ... FAILED` — without the pin the 30s shot proceeds to `submit_one` and only a
   provider-side "takes duration … not 30" error results, i.e. it would have billed.
3. Retry mints a fresh group instead of joining the existing one (`existing` →
   `mint_storyboard_id()`): the full-plan retry test FAILED on
   `assertion failed: retry joined the original group`.
4. Skip the whole-plan `storyboard::validate` on the plan_json branch:
   `a_retry_subset_plan_is_rejected_with_a_clear_error ... FAILED` (a sub-3-shot
   subset plan is no longer caught).

### UI: the mock can no longer mask the M2 defect

The round-2 vitest mock re-validates the FULL plan from `planJson` (throws
"3 to 8 shots" for a <3-shot plan) and honours `shotIndices`/`storyboardId`.
Two UI mutations (scratch copy) both flip the partial-failure test red:

5. Retry sends the submitted subset as the plan (`shots` → `indices.map(i=>shots[i])`):
   `× a partial failure shows the failed shots and retry submits only those` — the
   mock's 3..=8 gate rejects the 1-shot subset plan (this is exactly the defect the
   round-1 mock hid).
6. Drop `shotIndices` on retry (`isRetry ? indices : undefined` → `undefined`):
   same test FAILED on `expect(retryArg.shotIndices).toEqual([2])`.

The full UI suite (285) is green with the code intact; the test also asserts the
retry sends the FULL 4-shot plan, exactly one shot, `shotIndices === [2]`, and
`storyboardId === "sb1"`, plus that the retry total renders BEFORE resubmit.

### First-generate-all still positional

With `shot_indices: None` and `storyboard_id: None`, `indices = 0..shots.len()`,
a fresh group is minted, and shots submit positionally — pinned by the still-green
`generate_numbers_outcomes_densely_from_zero` (indices [0,1,2]),
`generate_records_the_set_even_when_shots_fail` ([0,1]), and
`generate_rejects_shots_that_do_not_match_the_plan` (one-for-one plan match). The
new per-index duration pin also holds on first-generate (the UI sends
`duration = shot.seconds`, equal to the plan's clamped seconds); no existing test
regresses (852/0).

### Conclusion

M2 is genuinely fixed: the retry sends the full validated plan + original shot
indices into the original group, the server pins each resubmit to its plan slot
by model + duration (+route), refuses an already-started index, and no longer
count/sum-gates a sub-3-shot subset. Six delta mutations (4 Rust + 2 UI) confirm
the new tests are non-vacuous, and the round-2 mock no longer masks the
integration defect. Gate reproduces at 852 Rust / 285 UI, 0 failures. No money,
no key, no hosted call. Everything the first PASS rested on still holds.

VERDICT: PASS
