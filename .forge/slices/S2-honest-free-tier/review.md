# S2-honest-free-tier — REVIEW (evidence)

Read-only production-standards review. Scope: working-tree diff on branch
`forge/s2-honest-free-tier` (the S2 change is unstaged on top of the S1 commit).

## What the slice actually changes

Sole production change: `crates/hickeyfield-core/src/provider.rs:96-98` —
`has_adapter()` drops `ProviderId::Local`, becoming
`matches!(self, ProviderId::Fal | ProviderId::Higgsfield)`, plus a doc paragraph
(`provider.rs:91-95`) explaining Local has no client yet.

Everything else in the diff is test-only or evidence:
- `registry.rs` — change is inside `mod tests` (unroutable-set assertion, :2291-2304).
- `route.rs` — change is inside `mod tests` (new resolver test + fixture doc, :286-317).
- `app.rs` — change is inside `mod tests` (drift-guard mirror + flipped Local test).
- `commands.rs` — change is inside `mod tests` (AC1 + AC3 tests).
- `ui/src/components/ModelPicker.test.tsx` — new vitest test (no component change).
- `.forge/**` — planning/evidence docs.

I confirmed the sole production edit against the intent in `design.md` (seam (a):
one predicate). No out-of-scope production code was touched.

## Correctness — CLEAN

- Root-cause fix is correct and complete. `has_adapter` is read at exactly these
  production sites, all of which inherit the correction:
  `route.rs:133,153,162,172,220` (resolve/cheaper_alternative),
  `use_case.rs:177,208` (route_serves), `commands.rs:208` (route_state). The only
  model with a Local route is `z_image` (`registry.rs:1334-1341`, Local-only), so
  the sole behavioural change is `z_image` becoming unavailable/unroutable. Verified
  by grep of all call sites and by the full suite staying green.
- Branch order in `route_state` (`commands.rs:198-224`) is correct: the
  `!has_adapter()` branch (:208) fires before the `needs_key()` branch (:217), and
  Local's `needs_key()` is `false` regardless — so Local can never surface
  "Needs a … key". It falls through to the honest "Hickeyfield has no client for
  Local yet" (:211-214). Confirmed.
- Submit path is fixed, not just the picker. `route::resolve` (`route.rs:151-168`)
  now filters Local out of `reachable`, and with only a Local route it returns
  `Err(RouteError::NoAdapter { held: [Local] })` (:164-167) whose Display
  (`route.rs:95-99`) says "your key is fine, the adapter is not built" — no
  misleading "add a key". The new resolver test pins this.
- DTO linkage is real, not asserted: `models_matching` (`commands.rs:415-438`) maps
  `route_state_for_job` → `RouteDto { available, unavailable_reason }` (:427-436).
  For the flat list (`use_case=None`, :411-414 keeps all models) `z_image`'s Local
  route serialises `available:false, unavailable_reason:"Hickeyfield has no client
  for Local yet"` — exactly the fixture the vitest test renders. In use-case tabs
  `use_case::supports` filters `z_image` out (route_serves gates on `has_adapter`,
  `use_case.rs:208`), matching the three existing unroutable models.

## Contract / consistency — CLEAN

- `has_adapter`'s doc contract ("a client exists that can execute a request here …
  keep in step with `client_for`") is now actually satisfied: `client_for`
  (`app.rs:38-53`) implements only `Fal` and `Higgsfield` and returns `None`
  otherwise, which now matches `has_adapter`'s arm set.
- Drift guard `has_adapter_matches_the_clients_actually_implemented`
  (`app.rs:592-607`) was correctly updated to mirror `Fal | Higgsfield`, so it is
  true rather than asserting a Local client that does not exist.

## Tests — CLEAN (coverage repurposed, not dropped)

- Old behaviour-encoding tests were flipped, not deleted:
  `app.rs` `local_is_reachable_without_a_credential` → `local_needs_no_key_but_
  has_no_client_yet` (:609-621) asserts `!has_adapter()` while preserving
  `!needs_key()`. `commands.rs` `local_is_available_with_no_key_at_all` →
  `local_route_is_unavailable_with_an_honest_reason` (:1197-1220).
- New/updated tests are real and discriminating, not tautological:
  - `commands.rs:1197-1220` (AC1) asserts `!ok`, reason `contains("no client")`,
    and reason `!contains("Needs a")` — it discriminates the honest reason from the
    key-prompt lie.
  - `commands.rs:1221-1238` (AC3) asserts a keyed fal route stays `(true, None)`
    side-by-side with the now-unavailable Local route.
  - `route.rs:289-317` asserts `Err(RouteError::NoAdapter { held: [Local] })` for a
    Local-only route with Local "available".
  - `ModelPicker.test.tsx` discriminates on the `available` flag: disabled main +
    `data-unavailable` chip + reason text + `onSelect` NOT called for `z_image`,
    versus enabled + `onSelect("nano_banana_2", "fal:…")` for the control.
- Gate re-run independently, all green:
  - `cargo test -p hickeyfield-core` → 636 passed, 0 failed.
  - `cargo test -p hickeyfield-tauri` → 104 passed, 0 failed. (740 total ≥ 727 floor.)
  - `pnpm test` → 216 passed (ModelPicker 2/2).
  - `pnpm build` (tsc --noEmit + vite) → OK (test file type-checks).
  - `cargo fmt --all --check` → clean; `cargo clippy --workspace --all-targets
    -- -D warnings` → clean; `./scripts/lint-provenance.py` → PASS.
- Blast-radius checks confirmed: `z_image` is not in `LAUNCH_FAMILIES`
  (`registry.rs:204-223`), so `every_launch_model_can_actually_be_run` and the
  launch/resolve tests are unaffected; the unroutable-set test grows by exactly
  `z_image`, which is the intended reviewed record.

## Findings (all minor; none block)

1. MINOR — `ui/src/components/ModelPicker.test.tsx:8-12,92`. The `describe` title
   "ModelPicker hides unroutable Local-only models" and the header comment
   ("filtered out exactly like the three other unroutable models") describe a
   mechanism the code does not use: `ModelPicker.tsx` never filters/hides — it
   renders a disabled tile (`disabled={!runnable}`, `:61`; `data-unrunnable`, `:56`),
   which is what the test actually asserts. Hiding happens only in the use-case tab
   via `use_case::supports`, not in this component. Standard: comments must not
   contradict the code they document. Fix: reword to "renders unroutable Local-only
   models disabled with the honest reason"; note that the actual hide is in the
   use-case tab. No behavioural impact.

2. MINOR — `app.rs:595-598` (drift-guard comment). The comment claims adding an arm
   to `client_for` "fails here until it is mirrored," but the test compares
   `has_adapter` against a second hand-maintained `matches!` list, not against
   `client_for` — both could drift together without failing. Pre-existing design;
   the slice correctly kept the mirror in sync. Follow-up (not this slice): derive
   the expected set from a single source or soften the comment.

3. MINOR / pre-existing, out of scope — `crates/hickeyfield-core/src/clients.rs:412-414`.
   The empty section header "Local — auto-detected ComfyUI / Ollama" now labels a
   section with no client. This slice did not touch `clients.rs` and explicitly
   records leaving it empty as a non-goal; `has_adapter`'s new doc already states
   "clients.rs ships no local adapter." No action required for this slice.

## Checked and found clean (silence-is-not-clearance ledger)

- Only one production predicate changed; no unintended production edits.
- No swallowed errors, no failure-as-success: the unavailable state carries a
  reason on every path (route_state, resolve, DTO).
- No invented APIs / AI-tells: `RouteError::NoAdapter`, `RouteDto.available/
  unavailable_reason`, and the Model/route UI fields all exist and are exercised;
  `pnpm build` type-checks the new test.
- No dead code introduced; the free-tier re-enable path is a single documented edit
  (add `Local` back to `has_adapter` + mirror) — the doc/comments say so accurately.
- Pricing (`CostModel::Flat { usd: 0.0 }`), `needs_key`, `is_configured`, and
  `is_ready` are unchanged, as the design's non-goals require; `a_local_route_is_
  free_and_needs_no_key` (`registry.rs:2361-2371`) stays valid.

No unresolved blocker or major on any critical path.

VERDICT: PASS
