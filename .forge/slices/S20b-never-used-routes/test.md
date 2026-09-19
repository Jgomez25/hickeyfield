# S20b-never-used-routes — TEST (evidence)

Adversarial verifier (NOT the implementer). All commands ran with the R-RULES preamble
(`HICKEYFIELD_NO_KEYCHAIN=1` + dummy `hickeyfield_*_KEY=x` SET; `HICKEYFIELD_ALLOW_SPEND`
never set; `first_light` never run). Two source mutations were probed and each reverted to a
tree byte-identical to the captured baseline (`git diff` == `pre.diff`).

## Implementer claim (verbatim, implement.md:3-5)

> Built to design.md Revision 1 (sha256 237154c5…). Demote + hide: Seedance and Grok
> are hidden from every picker behind a persisted, default-OFF toggle; nothing deleted.
> Workhorse `minimax_h3_max_turbo` leads every default, fallback, mock and sample.

## Commands (real output)

**G1 full Rust suite** — `cargo test --workspace` (summed across binaries):
`passed=978 failed=0 ignored=20`. Matches the claim (baseline 971 + 7 new). OK.

**Named AC tests** (run via `--workspace <name>`; the `hickeyfield_tauri_lib` lib-target name
is why the implementer's `-p hickeyfield-tauri` filters showed 0 — the tests themselves pass):
- A1 `the_workhorse_constant_is_a_launch_video_route` … ok
- A2 `launch_families_lead_with_the_workhorse_and_ban_seedance_and_grok` … ok
- A3 `is_never_used_covers_exactly_the_registry_seedance_and_grok_ids` … ok (1 passed)
- A3 `list_models_hides_the_never_used_routes_by_default` … ok
- A3 `migration_12_adds_app_pref_and_roundtrips` … ok
- A4 `history_resolves_a_hidden_models_display_name` … ok
- A4 `submit_one_refuses_a_hidden_model_while_the_toggle_is_off` … ok (1 passed)
- S20 regression `grok_720p_settles_on_output_seconds` … ok (hidden ≠ deleted proven)

**G1 lints / build:**
- `cargo fmt --all --check` → exit 0 (clean).
- `cargo clippy --workspace --all-targets -- -D warnings` → `Finished dev profile`, 0 warnings.
- `./scripts/lint-provenance.py` → `PASS no third-party media hosts` / `PASS no shipped string …`, exit 0.
- `python3 ./scripts/lint-spend.py` → `lint-spend: ok (49 file(s) scanned)`, exit 0.
- `env -u hickeyfield_FAL_KEY -u FAL_KEY cargo run -p hickeyfield-core --example audit_fal` →
  `70 endpoint(s) audited against fal's own schema`, exit 0.
- `cd ui && pnpm test` → `Test Files 24 passed; Tests 321 passed`.
- `cd ui && pnpm build` → `✓ built in 829ms` (tsc clean).

All G1 gate numbers match implement.md exactly (978/0/20 Rust, 321 UI).

## Per-AC judgement

- **A1** PASS. `App.tsx:206-209` initial = workhorse ?? first launch video ?? `m[0]`;
  `:351-354` tab fallback = workhorse ?? first launch ?? `m[0]`; `Storyboard.tsx:216-221`
  new-shot = `last?.modelId ?? (workhorse present ? workhorse : models[0]?.id) ?? ""`. Empty
  list is guarded (`if (first)`, `?? null`, `?? ""`) — no crash. On the image tab (workhorse
  absent) it falls to first launch, never a never-used id (they are filtered server-side). One
  named constant per language (`registry::WORKHORSE_MODEL_ID`, `types.ts:17`). The unreachable
  empty-board storyboard branch is a defensive default, covered by two reachable substitute
  tests — acceptable.
- **A2** PASS. `launch_families_lead_with_…` asserts `LAUNCH_FAMILIES.len()==12`, contains the
  workhorse, and `!is_never_used(id)` for every flat launch id (prefix-based). Diff shows only
  the Seedance launch-membership row replaced by the workhorse row — no priced/capability row
  deleted.
- **A3** PASS. Exhaustive test asserts BTreeSet **equality** with the enumerated 11 ids (not
  non-emptiness), plus Seedream/workhorse on the KEPT side. Independent enumeration of
  `registry.rs` string literals returns exactly those 11; no id contains-but-does-not-start
  with seedance/grok. `list_models_hides…` derives the count (`full − 11`) and proves no
  never-used id survives the OFF default while the full set still carries them.
- **A4** PASS. `model_names()` is unfiltered → history renders "Seedance 2.0" with pref OFF.
  Refusal test drives both polarities: OFF ⇒ `Err` containing "never-used"+"Settings", 0
  clients reached; ON ⇒ guard passes, submit reaches the injected fake (1 client). The guard
  (`commands.rs:1162`) sits at the very top of `submit_one`, BEFORE `compiled_for_submit`,
  duration clamp, and `route::resolve`. All spend paths funnel through `submit_one`
  (standalone `:1080`, storyboard shot `:1978`, keyframe still `:1906/:2146`, chain.rs
  `:293/:643/:793`) — no bypass. No silent `modelId` re-point in the storyboard path.
- **A5** PASS. SettingsRail vitest passes; additive `.never-used-toggle`; locked selectors
  intact (see A7).
- **A6** PASS. `mock.test.ts` (4 tests) asserts `/seedance|grok/gi`=[] over
  MOCK_MODELS/PRESETS/JOBS, roster leads with the workhorse, every preset variant list leads
  with the workhorse, sample job is the workhorse. Independent grep of mock.ts/browser.ts/
  providers.ts/onboarding.rs shows only the allowed xAI `GROK_API_KEY` env rows remain; no
  blurb advertises Seedance/Grok. No duplicate mock ids.
- **A7** PASS. Diff has no `CostModel` value change, no deleted route/price/capability row, no
  `Cargo.toml`/`Cargo.lock`/`package.json`/`pnpm-lock`/`deny.toml` change, no `#[ignore]`
  added. Migration 12 is the only migration and is justified (spend boundary needs a
  server-readable, `AppHandle`-free pref home).
- **G1** PASS (all gates above green).
- **G2** PASS. Diff contains no `ALLOW_SPEND`, no `remove_var`/env strip, no socket/TcpListener
  in tests. Refusal test proves the early-`Err` no-spend path.

## Break attempts

1. **Mutation — `is_never_used` → `false`:** `is_never_used_covers_exactly…` FAILED
   (panic registry.rs:2856) and `list_models_hides_the_never_used_routes_by_default` FAILED
   (panic commands.rs:3216). Caught. Reverted; `git checkout` had also discarded the
   implementer's uncommitted registry.rs, so I re-applied the registry hunk from `pre.diff`
   and confirmed `git diff` byte-identical.
2. **Mutation — disable the `submit_one` guard (`if false && …`):**
   `submit_one_refuses_a_hidden_model_while_the_toggle_is_off` FAILED (panic commands.rs:3290).
   Caught. Reversed exactly; `git diff` byte-identical.
3. **Toggle ON path:** covered — the refusal test flips the pref ON and asserts the submit
   reaches the injected fake (1 client seen). Not OFF-only.
4. **Exhaustiveness:** independent literal enumeration = exactly the 11 ids; set-EQUALITY
   assertion present; `seedream*` diverges at char 4 and is not matched; no contains-not-starts id.
5. **A1 edges:** empty model list and workhorse-absent (image tab) both handled without crash
   and never resolve to a never-used id.
6. **Migration 12:** `migrate()` is append-only `if version < 12`; the three pre-existing
   `user_version` tests were bumped to 12, so the full 0→12 chain (and by sequential
   construction, a real v11→v12 upgrade of an existing DB) is exercised. `get_bool_pref` reads
   a garbage `app_pref` value as `false` via `.map(|v| v=="1")` — Some(false), never a panic;
   default only applies to an absent row. Round-trip + idempotent re-`migrate()` covered.
7. **Integrity:** final `git diff` == `pre.diff` (TREE BYTE-IDENTICAL); only untracked paths
   are the slice dir and the new `ui/src/mock.test.ts`.

## Non-blocking findings (do not block release)

- The A2 test `launch_families_lead_with_…` does not independently catch a broken predicate
  (with `is_never_used`→false its `!is_never_used(id)` clause is trivially satisfied); the A3
  exhaustive and default-hidden tests do catch it. Complementary coverage, not a gap.
- No explicit "existing v11 DB with data → v12" test; the sequential `if version < 12`
  structure plus the bumped v11-era assertions prove the upgrade path structurally.
- Runner resume-drain of an already-persisted queued job does not re-check the toggle, but any
  such job was authorized at `submit_one` time (a re-run goes back through `submit_one`); this
  is not a spend bypass.

VERDICT: PASS

---

## Repair round 1 — re-verification

Adversarial re-verification of the WHOLE slice after repair round 1 (store.rs v11→v12
upgrade test; App.test.tsx "real" A1 cross-tab test; mock.ts WORKHORSE_MODEL_ID constant).
All commands ran with the R-RULES preamble (`HICKEYFIELD_NO_KEYCHAIN=1` + dummy
`hickeyfield_*_KEY=x` SET; `HICKEYFIELD_ALLOW_SPEND` never set; `first_light` never run).
Baseline `git diff` captured to `pre.diff` (1705 lines); every mutation reverted by `cp`
and proven byte-identical to `pre.diff`.

### Repair claim (verbatim, implement.md:188-207, Fix 2)

> Fix 2 — make the A1 cross-tab test real (REVIEW non-blocking #1) … The test now:
> opens on the workhorse; clicks `[role="tab"][data-tab="image-to-video"]` (video list
> still has the workhorse) and asserts the selection is STILL `H3 Max Turbo`; clicks
> `[data-tab="text-to-image"]` (no workhorse) and asserts the selection falls to that
> list's first LAUNCH model, `Nano Banana Pro` — NOT the workhorse and not a never-used id.

### Commands (real output)

- `cargo test --workspace` → summed **passed=979 failed=0 ignored=20** (exit 0). Matches claim.
- `cargo fmt --all --check` → clean, exit 0.
- `cargo clippy --workspace --all-targets -- -D warnings` → `Finished dev profile`, 0 warnings.
- `./scripts/lint-provenance.py` → `PASS no third-party media hosts` / `PASS no shipped
  string … (80015 shingles)`, exit 0.
- `python3 ./scripts/lint-spend.py` → `lint-spend: ok (49 file(s) scanned)`, exit 0.
- `env -u hickeyfield_FAL_KEY -u FAL_KEY cargo run -p hickeyfield-core --example audit_fal`
  → `70 endpoint(s) audited against fal's own schema`, exit 0.
- `cd ui && pnpm test` → `Test Files 24 passed; Tests 321 passed`.
- `cd ui && pnpm build` → `✓ built in 1.04s` (tsc clean).

Named ACs re-run by name (all `... ok`):
`store::tests::an_existing_v11_database_with_data_upgrades_to_v12_intact`,
`store::tests::migration_12_adds_app_pref_and_roundtrips`,
`registry::tests::is_never_used_covers_exactly_the_registry_seedance_and_grok_ids`,
`registry::tests::launch_families_lead_with_the_workhorse_and_ban_seedance_and_grok`,
`registry::tests::the_workhorse_constant_is_a_launch_video_route`,
`commands::tests::list_models_hides_the_never_used_routes_by_default`,
`commands::tests::submit_one_refuses_a_hidden_model_while_the_toggle_is_off`,
`commands::tests::history_resolves_a_hidden_models_display_name`,
`runner::tests::grok_720p_settles_on_output_seconds` (S20 settle; model_id `grok_video_v15`,
runner.rs:997-1003 — hidden ≠ deleted proven).

### Check-by-check

- **Check 1 (full gate):** PASS. 979/0/20 Rust, 321 UI, build + all lints green (above).
- **Check 2 (v11→v12 upgrade real, not tautological):** PASS. Read store.rs:415-432 — migration
  12's `if version < 12` block adds ONLY `CREATE TABLE app_pref` + `PRAGMA user_version = 12`;
  nothing else distinguishes v11 from v12. The test (store.rs:1287-1394) builds a genuine v11
  file (`DROP TABLE app_pref; PRAGMA user_version = 11;`, asserts `v==11` before upgrade),
  re-opens through the real `SqliteStore::open`, and compares WHOLE pre-existing rows via
  `assert_eq!` on `JobSet` and `Storyboard` (both derive PartialEq), incl.
  `estimated_usd=Some(1.23)`/`actual_usd=Some(1.19)` and non-default storyboard fields; plus a
  3rd idempotent re-open. Temp-dir only, removed at end — never `~/Library/Application Support/`.
- **Check 4 (mock.ts):** PASS. `grep '"minimax_h3_max_turbo"' src/mock.ts` → NONE (data uses
  `WORKHORSE_MODEL_ID`, imported from `./types` at mock.ts:12; used at :58 roster id, all preset
  `models:` lists, RATE_PER_SECOND key, sample job). `mock.test.ts` passes (4 tests). 0
  `/seedance|grok/i` hits in mock.ts. No import cycle (build passes). browser.ts/providers.ts/
  onboarding.rs carry only the allowed xAI `GROK_API_KEY` env row.
- **Check 5 (tree integrity, registry):** PASS. registry.rs has `WORKHORSE_MODEL_ID`
  (:120), `is_never_used` (:125), `LAUNCH_FAMILIES` workhorse row `("MiniMax H3 Max Turbo",
  &[WORKHORSE_MODEL_ID])` (:228) and no Seedance launch row; the 3 new registry tests run in the
  979. Seedance/Grok registry model defs remain (registry.rs:1672,1710,…) — hidden ≠ deleted.
- **Check 6 (key ACs by name):** PASS (all `... ok`, listed above).
- **Check 7 (diff hygiene):** PASS. `git diff --stat` = 21 files, all slice files; untracked =
  slice dir + `ui/src/mock.test.ts`. No `Cargo.*`/`package.json`/lockfile/`deny.toml` change.
  `git diff | grep '#\[ignore'` empty.

### Mutation results

1. **Migration 12 made destructive** (store.rs, added `DELETE FROM job_sets;` inside the
   `version < 12` block): `an_existing_v11_database_with_data_upgrades_to_v12_intact` → **FAILED**
   (panic store.rs:1370, storyboard row gone). CAUGHT. Restored by `cp`; `git diff` byte-identical
   to `pre.diff`. Upgrade test is genuine.

2. **App.tsx tab-change fallback → `m[0]`** (App.tsx:352-354 `m.find(WORKHORSE) ?? m.find(isLaunch)
   ?? m[0]` reduced to `const next = m[0]`): `pnpm test src/App.test.tsx` → **3 passed (GREEN)**.
   NOT CAUGHT. Escalated: also reduced the INITIAL selection (App.tsx:207-209) to `const first =
   m[0]`, i.e. ALL workhorse-preference logic stripped from App.tsx → **still 3 passed (GREEN)**.
   Restored by `cp`; `git diff` byte-identical to `pre.diff`.

### Root cause (Check 3 — A1 tab test is NOT real)

Fix 2's headline deliverable was to de-tautologize the A1 cross-tab test (the pre-repair version
`void tab`-ed, a no-op). The repair replaced one tautology with a subtler **fixture-ordering**
tautology, so the test still does not guard the behaviour it names:

- The `vi.mock("./api")` shim (App.test.tsx:16-40) hands App a full-roster `listModels`
  (`...actual`) whose head is the workhorse (`MOCK_MODELS[0].id === WORKHORSE_MODEL_ID`,
  mock.ts:58) and an image list whose head is the first LAUNCH image model
  (`nano_banana_pro`, mock.ts:148, `isLaunch:true`). So in EVERY asserted state `m[0]` already
  equals the expected model:
  - initial roster head = workhorse ⇒ assertion `toContain("H3 Max Turbo")` holds under `m[0]`;
  - image-to-video list = the same video list (both video slugs return `video`), head = workhorse,
    and the selection is retained by the `current && m.some(...)` SURVIVAL branch anyway — the
    fallback preference (App.tsx:352) is never the deciding factor;
  - text-to-image list head = `nano_banana_pro` = first launch ⇒ `toContain("Nano Banana Pro")`
    holds equally under `m[0]` and under `m.find(isLaunch)`.
- Consequence proven by mutation 2: stripping ALL workhorse-preference from App.tsx (both :207 and
  :352 → `m[0]`) leaves all 3 tests GREEN. The test therefore verifies neither the workhorse
  *preference* nor "never a never-used id" (the mock roster contains no seedance/grok id to avoid).
  It cannot go RED on the mutation Check 3 requires.

This is a test-strength defect in the exact artifact repair round 1 set out to fix; the verifier
checklist (Check 3) requires the fallback mutation to go RED and it does not. Note: the underlying
App.tsx production code (App.tsx:207-209, :352-354) IS correct — mutation 2 exposed no runtime
bug, and the spend-safety / never-used-hiding behaviour is independently and strongly guarded by
the Rust A3/A4 tests and `mock.test.ts`. The gap is confined to UI test coverage of the A1
preference.

Repair guidance for round 2: make `m[0]` differ from the expected model in the asserted states so
the preference logic is actually exercised — e.g. order the test's mocked lists so the video/tab
list does NOT lead with the workhorse (forcing App.tsx:352's `m.find(WORKHORSE)` to do real work),
have the image list NOT lead with its first launch model, add a transition that lands on a
workhorse-containing tab FROM a non-workhorse selection (the only path that reaches the :352
preference rather than the survival branch), and include a never-used id in the mocked roster so
"never a never-used id" is a live assertion rather than vacuous.

VERDICT: FAIL

---

## Repair round 2 — re-verification

Adversarial re-verification of the WHOLE slice after repair round 2 (the last allowed).
Round 1 FAILed on ONE thing: the A1 cross-tab vitest was a fixture-ordering tautology
(`m[0]` already equalled the expected model in every asserted state, so stripping all
workhorse-preference from `App.tsx` left it GREEN). Round 2 moved that test into its own
file `ui/src/App.workhorse.test.tsx` with hand-ordered fixtures and removed the old test
from `App.test.tsx`. No production file changed in round 2 (proven byte-identical below).
All commands ran with the R-RULES preamble (`HICKEYFIELD_NO_KEYCHAIN=1` + dummy
`hickeyfield_*_KEY=x` SET; `HICKEYFIELD_ALLOW_SPEND` never set; `first_light` never run).
Baseline `git diff` captured to `pre3.diff` (1621 lines); every mutation reverted by `cp`
and proven tree-byte-identical to `pre3.diff`. No source edited, no commit/add/checkout/restore.

### Repair claim (verbatim, implement.md:236-253)

> ## Repair round 2 (A1 cross-tab test made falsifiable)
> TEST re-verification (repair round 1) FAILED on ONE thing: the A1 cross-tab vitest was a
> fixture-ordering tautology — `m[0]` already equalled the expected model in every asserted
> state, so stripping ALL workhorse-preference from `App.tsx` (both `:206-209` and `:351-354`)
> left it GREEN. Production code (`App.tsx`) is correct and was NOT changed (proven byte-identical
> below). This round only rebuilds the TEST so it can go RED.
> … NEW `ui/src/App.workhorse.test.tsx` … the workhorse is NEVER at index 0 and index 0 is never
> the expected answer … `ui/src/App.test.tsx` — removed the old tautological A1 `it(...)` … No
> production file changed. `App.tsx`, `types.ts`, `mock.ts` untouched.

### Check 1 — the round-1 FAIL root cause is closed (PASS)

- **Workhorse never at index 0; index 0 never the expected answer.** Verified against real
  `MOCK_MODELS` attributes (mock.ts): `gemini_omni` video/isLaunch:false, `kling_3_0`
  video/isLaunch:true, `wan_2_6` video/isLaunch:true, `recraft_v4_1` image/isLaunch:false,
  `nano_banana_pro` image/isLaunch:true, `seedream_5_pro` image/isLaunch:true.
  - initialRoster `[gemini_omni(¬launch), kling_3_0(launch), WORKHORSE, wan_2_6]` → m[0] is a
    non-launch video; first launch video is kling, NOT the workhorse. Only `m.find(WORKHORSE)` wins.
  - imageList `[recraft_v4_1(¬launch), nano_banana_pro(launch), seedream_5_pro]` → m[0] is
    non-launch, so first-launch answer (nano) differs from m[0].
  - videoB `[wan_2_6(launch), kling_3_0(launch), WORKHORSE]` → workhorse present but neither m[0]
    nor first-isLaunch (wan_2_6 precedes it), and it lacks the current image selection, so the
    survival branch fails and the fallback branch is the deciding factor.
- **The test really clicks tabs.** `clickTab(slug)` clicks `[role="tab"][data-tab="${slug}"]`
  and `await flush()`; `listUseCases` in the file's mock returns three real tabs, so the tab
  effect runs `modelsForUseCase(tab)`. Not a `void tab` no-op.
- **`vi.mock` scoped to its own file.** The mock lives only in `App.workhorse.test.tsx`;
  `App.test.tsx` no longer imports `vi` and runs against the plain browse-mode api.
- **`App.test.tsx` intact.** `git diff -- ui/src/App.test.tsx` is +6 lines, a header comment
  ONLY — no removed lines, no assertion weakened; both S12 tests present and meaningful
  (`mounts the meta rail on Library only…`; `opens the delete confirmation as a dialog with no
  button-in-button`). The old tautological A1 `it(...)` is gone (moved, not deleted).

### Check 2 — mutations redone independently (baseline GREEN → each mutation RED)

Baseline `pnpm vitest run src/App.workhorse.test.tsx` → 1 passed (GREEN). Each mutation was a
COPY-only edit of `ui/src/App.tsx`, restored from `$SCRATCH/App.tsx.keep` between attempts.

- **M1 — initial pick → `const first = m[0]`:** RED.
  `AssertionError: expected 'Gemini Omni' to contain 'H3 Max Turbo'`.
- **M2 — tab-change fallback → `const next = m[0]`:** RED.
  `AssertionError: expected 'Recraft v4.1' to contain 'Nano Banana Pro'`.
- **M3a — initial pick → first `isLaunch` (workhorse line dropped), fallback UNCHANGED:** RED.
  `AssertionError: expected 'Kling 3.0' to contain 'H3 Max Turbo'` (caught at the initial-pick
  assertion). Run SEPARATELY from M3b.
- **M3b — fallback → first `isLaunch` (workhorse line dropped), initial pick UNCHANGED:** RED.
  `AssertionError: expected 'Wan 2.6' to contain 'H3 Max Turbo'` (caught at step 3, the
  image-to-video → videoB fallback where first-isLaunch is `wan_2_6`, not the workhorse).
  Tested SEPARATELY from M3a — the combined run dies at the initial-pick assertion (Kling) and
  would MASK this fallback failure; run alone it is genuinely caught.
- **M4 (survival) — drop the `current && m.some(...)` keep-branch, re-pick by first `isLaunch`:**
  RED. `AssertionError: expected 'Kling 3.0' to contain 'H3 Max Turbo'`. Survival is covered too;
  no non-blocking survival gap.

All of M1, M2, M3a, M3b RED (the FAIL gate). M4 also RED.

### Check 3 — no production file changed in round 2 (PASS)

`git diff --name-only` = 21 tracked files (same set as the round-1 list). Untracked =
`.forge/slices/S20b-never-used-routes/`, `ui/src/mock.test.ts` (both from round 1), plus the new
`ui/src/App.workhorse.test.tsx`. The only round-2 deltas vs round 1 are `ui/src/App.test.tsx`
content (comment-only, A1 `it` removed) and the new untracked `App.workhorse.test.tsx` — exactly
what Check 3 permits. `App.tsx`/`types.ts`/`mock.ts` unchanged in round 2 (restore proof below).

### Check 4 — full gate (all real output, all green)

- `cargo test --workspace` → `773 + 6 + 2 + 198 = 979 passed; 0 failed; 20 ignored`.
- `cargo fmt --all --check` → exit 0 (clean).
- `cargo clippy --workspace --all-targets -- -D warnings` → `Finished dev profile`, 0 warnings.
- `./scripts/lint-provenance.py` → `PASS no third-party media hosts` / `PASS no shipped string …
  (80015 shingles)`, exit 0.
- `python3 ./scripts/lint-spend.py` → `lint-spend: ok (49 file(s) scanned).`, exit 0.
- `env -u hickeyfield_FAL_KEY -u FAL_KEY cargo run -p hickeyfield-core --example audit_fal` →
  `70 endpoint(s) audited against fal's own schema`, exit 0.
- `cd ui && pnpm test` → `Test Files 25 passed; Tests 321 passed`.
- `cd ui && pnpm build` → `✓ built in 1.00s` (tsc clean).

### Check 5 — test-count sanity (PASS)

UI total held at 321 (was 321). `App.test.tsx` now has 2 `it(...)` (both S12); the A1 test moved
to `App.workhorse.test.tsx` (1 `it(...)`). Net UI test count unchanged — coverage moved, not lost;
nothing dropped below 321.

### Restore proof (byte-identical)

After the five mutations: `cp $SCRATCH/App.tsx.keep ui/src/App.tsx`; `cmp ui/src/App.tsx
$SCRATCH/App.tsx.keep` → IDENTICAL; `git diff > post3.diff`; `diff pre3.diff post3.diff` → empty
(TREE BYTE-IDENTICAL); baseline `pnpm vitest run src/App.workhorse.test.tsx` re-run → 1 passed.
No `git checkout`/`restore`/`reset`/`stash`/`clean` used; no source committed or staged.

### Root cause (whole slice, post-repair)

The round-1 FAIL was confined to A1 UI test STRENGTH (a fixture-ordering tautology); the
underlying `App.tsx` preference code was already correct. Round 2 rebuilt that test as an
independent, falsifiable file: with the workhorse never at index 0 and index 0 never the expected
answer, every workhorse-preference site in `App.tsx` (initial pick `:206-209`, tab fallback
`:351-354`, survival keep-branch `:350`) is now load-bearing — M1/M2/M3a/M3b/M4 all go RED when
their site is broken, verified independently here. The rest of the slice (A2–A7, G1/G2) remains
as PASSed in the initial verification, unchanged and re-confirmed green in the full gate above
(979/0/20 Rust, 321 UI, build + all lints + keyless audit clean). No production file changed in
round 2. No spend path exercised. No defect remains.

VERDICT: PASS
