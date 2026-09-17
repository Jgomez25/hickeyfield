# S18-live-spend-gate — REVIEW (evidence)

Independent, read-only production-standards review of `git diff main...HEAD` (HEAD 07da1b8).
Changed no source. Re-ran the mechanical gate myself (results below). Scope: the spend-gate
primitive, its wiring into every billable path, the example/entrypoint refusals, the lint, and
docs. Other verifiers own test.md / security.md.

## Gate re-run (mine, under the mandated env)
All commands exported `HICKEYFIELD_NO_KEYCHAIN=1` + dummy `hickeyfield_*_KEY`/`FAL_KEY`, never
`HICKEYFIELD_ALLOW_SPEND`.

- `scripts/lint-spend.py` → `ok (49 file(s) scanned)`.
- `scripts/lint-provenance.py` → PASS (media hosts + copy provenance).
- `cargo fmt --all --check` → clean.
- `cargo clippy --workspace --all-targets -- -D warnings` → Finished, no warnings.
- Spend tests green: `spend_gate::tests::*` (3), `clients::tests::a_{fal,higgsfield}_submit_is_blocked_before_any_http` (2),
  `enhancer::tests::a_hosted_llm_call_is_blocked_without_the_flag` + `..._stubbed_hosted..._not_gated` (2),
  `hickeyfield-tauri` `spend_gate_wiring_tests::*` (3) + `submit_one_uses_the_injected_client`.
- Full `hickeyfield-tauri` lib suite → 189 passed / 0 failed.
- Lint-detection proof: a scratch file (outside the repo) exercising all five patterns
  (`var("FAL_KEY")`, `keyring::`, `remove_var("hickeyfield_…")`, `set_var("HICKEYFIELD_ALLOW_SPEND"…)`,
  `spend_gate::enable(`) → 5 violations, exit 1. Every rule fires.

## What I checked and found CLEAN

- **`spend_gate.rs` OnceLock semantics / thread-safety.** `enable()` is idempotent
  (`OnceLock::set` → `Err` ignored on a second call); `OnceLock` is thread-safe; `decide()` is a
  pure, unit-tested truth table split out precisely so the global/env need never be touched in a
  test. Exact-`"1"` (trimmed) match, stricter than the vault's truthy parse — correct for a money
  switch.
- **No cross-test state leak.** The core-crate tests assert `!allowed()` and never call `enable()`;
  the only `enable()` execution in a test (`the_entrypoint_enable_path_actually_allows_spending`,
  src-tauri/src/lib.rs:159) lives in the *separate* `hickeyfield-tauri` test binary, where no test
  asserts `!allowed()`/a real-client block (grep confirmed). Flipping the process-global there
  poisons nothing. Confirmed by running both binaries green.
- **Gate placement.** It is the FIRST statement of every real billable entry point, before
  `http()` builds any client: `clients.rs:205` (`FalClient::submit`), `clients.rs:369`
  (`HiggsfieldClient::submit`), `enhancer.rs:1367` (`HostedEnhancer::call`), `enhancer.rs:1664`
  (`ChatBackend::Hosted` in `complete`). The two real `ProviderClient` impls are the only ones
  (FalClient, HiggsfieldClient); the only spending `Chat`/`Enhancer` impls are ChatBackend/
  HostedEnhancer. Ollama branches stay ungated (free). Coverage is complete for current code.
- **Blocked-before-HTTP proof is real, not asserted.** Tests assert the variant is `SpendBlocked`
  AND not `Transient`/`Permanent` — a network attempt with the dummy key would be one of the
  latter, so the discriminator proves the short-circuit without a proxy/env hack. Sound (see
  Deviations).
- **`JobError::SpendBlocked` fan-out.** `is_retryable` correctly excludes it (only `Transient`
  retries — a blocked submit is not retried, engine.rs:245). The two exhaustive matches (Display
  engine.rs:264, `higgsfield_reason` clients.rs) both handle it; every other site is `_`/construct.
  `clippy --all-targets -D warnings` compiling every target is the proof nothing else broke.
- **Error surfaced, not swallowed.** `submit_one` (commands.rs:1201) stores `e.to_string()` (the
  flag-naming message) in `job.fail_reason` and returns it — reaches the UI verbatim.
- **Entrypoint enable is proven by EXECUTION, not just source order.** `run()` calls
  `spend_gate::enable()` as its first line (src-tauri/src/lib.rs:23), ahead of `AppState::init`
  (line 41, the ClientFactory). `main.rs` does nothing before `run()`. One test executes the exact
  call and asserts `allowed()`; another pins source order. The "app accidentally blocked" risk is
  therefore covered up to the owner's manual launch.
- **`base_url.is_none()` scoping is not a current production hole.** Every production hosted caller
  builds with `base_url = None` — `commands.rs:1334` (`ChatBackend::hosted`) and `harness.rs:238`
  (`HostedEnhancer::new`). `with_base_url` appears only in tests. So every real hosted call is
  gated. (Latent risk noted below.)
- **Nothing in the packaged app's normal flow is wrongly gated.** Polls, downloads/library writes,
  uploads, and the pricing GET are ungated; and `enable()` is on regardless at startup. `resume_all`
  can re-submit on startup but runs after `enable()`.
- **`first_light.rs`** refuses without the flag, prints the offline cheapest-image estimate first,
  exits 2; the `security find-generic-password` keychain-copy line is gone; the example compiles
  clean under `clippy --all-targets`.
- **`lint-spend.py`** allowlist is exactly the three legitimate key-readers; rule 2 matches the
  actual `var(...)` read (avoids the `onboarding.rs` "FAL_KEY" false-positive); all rules verified
  to fire. Docs (README "Spending and credentials", FIRST-LIGHT, CHANGELOG) are accurate and
  original.
- **D4 seam.** `submit_to_provider` now takes `clients: &ClientFactory`; the compiler enforces every
  caller (including the resume/chain path) was updated — clean build is the proof. Seam test passes.
- **Deviations (a–f in implement.md): sound.** Gating the second hosted path and Ollama-free are in
  the slice's spirit; the discriminator proof is stronger and avoids the env-mutation anti-pattern
  the 2026-09-16 incident warns against; the `remove_var`/`var(...)`/lint-not-in-gate.py scoping
  choices are each justified against a real regression or false-positive.

## Findings (all MINOR — none block; current behavior is correct and covered)

1. **MINOR (structural hardening; answers Q(a)).** The gate is duplicated as the first statement of
   each `submit`/hosted call rather than living on the trait's default path. `engine.rs:176`
   (`trait ProviderClient`) has `submit` as a *required* method with no default, and `lint-spend.py`
   does not require the guard string inside an `impl ProviderClient`. A NEW provider client added
   later could omit the guard: it would neither fail to compile nor fail the lint, and would spend.
   Standard: fail-safe-by-construction for a money path. Fix (follow-up, not this slice): either
   make `submit` a non-overridable default that gates then delegates to an inner `submit_inner`
   (omission = compile error), or add a `lint-spend.py` rule requiring `spend_gate::allowed()` in
   every `impl ProviderClient for … { fn submit`. Not blocking — both current impls are gated and
   tested.

2. **MINOR (latent; answers Q(a)/hosted).** The hosted gate is scoped to `base_url.is_none()`
   (`enhancer.rs:1367`, `:1664`). Correct today (no production caller sets `base_url`), but if a
   future feature ever exposes a configurable hosted base_url in production (a custom
   OpenAI-compatible endpoint), a real spend would silently bypass the gate. Fix (follow-up): gate
   regardless of base_url with an explicit test-only allowance, or add a lint/compile guard so this
   assumption can't rot.

3. **MINOR (informational).** `FalClient::upload`/`HiggsfieldClient::upload` (`clients.rs:578`,
   `:640`) make real network POST/PUT with the real key and are ungated. They are storage uploads
   (the fal `file_url` is documented "public and free"), so no direct billing, and the packaged app
   has the gate on anyway — but in a keyed dev/CLI process without the flag an upload still touches
   the network before the gated submit refuses. Worth a one-line note or a follow-up if uploads ever
   become billable.

4. **TRIVIAL (evidence doc only, not shipped).** implement.md AC3/AC5 report "48 file(s) scanned";
   the current count is 49. Update the evidence number.

No blocker or major on a critical path. The primitive is sound, every current billable path is
gated as its first statement, the entrypoint enable is proven by execution, the lint and its
detection are real, and the full suite is green on my re-run.

VERDICT: PASS
