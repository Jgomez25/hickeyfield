# S18-live-spend-gate — TEST (adversarial evidence)

Verifier: a different agent than the implementer. I did not write this code. Every command below
exported `HICKEYFIELD_NO_KEYCHAIN=1` + the dummy `hickeyfield_*_KEY` keys and **never**
`HICKEYFIELD_ALLOW_SPEND` inside the repo. The only place `HICKEYFIELD_ALLOW_SPEND=1` was set was a
scratch crate OUTSIDE the repo, with dummy keys behind an unroutable proxy `http://127.0.0.1:9`.

## Implementer's claim (verbatim, from implement.md)

> Built exactly the design (`design.md`, D1–D6 + weakest points) against `forge/s18-spend-gate`. …
> the gate as the **first statement** of `FalClient::submit` and `HiggsfieldClient::submit`
> (before `http()`); … gate in `HostedEnhancer::call` **and** the `ChatBackend::Hosted` branch of
> `complete` … `src-tauri/src/lib.rs::run` — `hickeyfield_core::spend_gate::enable()` as the
> **first** line … `cargo test --workspace → 961 passed / 20 ignored / 0 failed`.

## Commands (real output, all under the mandated env, no ALLOW flag in-repo)

### AC5 mechanical gate
- `cargo test --workspace` → aggregated **PASSED=961 FAILED=0 IGNORED=20** (matches claim exactly).
  The 14 spend-relevant tests all `ok`:
  ```
  spend_gate::tests::{decide_is_off_by_default, only_exactly_1_env_allows, the_process_flag_allows} ... ok
  clients::tests::a_fal_submit_is_blocked_before_any_http ... ok
  clients::tests::a_higgsfield_submit_is_blocked_before_any_http ... ok
  enhancer::tests::a_hosted_llm_call_is_blocked_without_the_flag ... ok
  enhancer::tests::a_stubbed_hosted_call_is_not_gated_so_wire_tests_run_without_the_flag ... ok
  spend_gate_wiring_tests::{the_entrypoint_calls_spend_gate_enable, enable_precedes_any_client_construction_in_run, the_entrypoint_enable_path_actually_allows_spending} ... ok
  commands::tests::submit_one_uses_the_injected_client ... ok
  ```
- `cargo fmt --all --check` → exit 0.
- `cargo clippy --workspace --all-targets -- -D warnings` → **exit 0** (re-run after `touch` to force a fresh build; no warnings/errors).
- `python3 ./scripts/lint-spend.py` → `lint-spend: ok (49 file(s) scanned).` exit 0. (Implementer said 48; benign drift, not material.)
- `python3 ./scripts/lint-provenance.py` → both checks PASS.
- `env -u hickeyfield_FAL_KEY -u FAL_KEY cargo run … --example audit_fal` → `70 endpoint(s) audited …`, keyless, unchanged.
- `cd ui && pnpm test` (Node v24.8.0) → **Test Files 23 passed, Tests 312 passed**.
- `cd ui && pnpm build` → `✓ built in 1.84s`, exit 0.
- `cargo deny check`: NOT run — the design mentioned it but the task's authoritative gate list omits it. Accepted deviation; everything in the authoritative list is green.
- `tauri build --bundles app` + install/launch: owner-only per AC5 ("verified by the owner, not an agent"). Not run.

### AC1 — the money-safety core, proven TWO independent ways

1. **Variant discriminator (in-repo tests, above):** each `*_is_blocked_before_any_http` asserts
   `SpendBlocked` AND `!Transient && !Permanent` — a real POST with a dummy key would be
   Transient/Permanent, so SpendBlocked proves short-circuit before `http()`.

2. **Independent out-of-repo unroutable-proxy harness** (scratch crate depending on
   `hickeyfield-core` by path, exercising the *real* public submit/complete methods):
   - RUN 1 — **no flag**, `HTTPS_PROXY=http://127.0.0.1:9`:
     ```
     allowed()=false
     FAL        => SpendBlocked
     HIGGSFIELD => SpendBlocked
     HOSTED_CHAT=> Permanent(spending is disabled … HICKEYFIELD_ALLOW_SPEND=1 …)
     ```
     No connection error → the gate returned before any socket. (A proxy/connection error here
     would have been a FAIL; none occurred.)
   - RUN 2 — **`HICKEYFIELD_ALLOW_SPEND=1`**, dummy keys, same unroutable proxy (outside the repo):
     ```
     allowed()=true
     FAL        => Transient(request failed … https://queue.fal.run/…)
     HIGGSFIELD => Transient(request failed … https://api.higgsfield.ai/…)
     HOSTED_CHAT=> Permanent(request failed … https://api.openai.com/v1/chat/completions)
     ```
     With the flag, all three now reach the network layer (connection error) — proving the gate is
     the ONLY thing blocking, for the FalClient, the Higgsfield mirror, and the hosted chat backend.
   - `HostedEnhancer::call` (private) is covered by `enhancer::tests::a_hosted_llm_call_is_blocked_without_the_flag`
     and shares the byte-identical guard clause (`self.base_url.is_none() && !crate::spend_gate::allowed()`)
     with the ChatBackend path proven above.
   - Ollama stays ungated (free): only the `HostedBackend` arms were touched; the Local/Ollama
     branches are unchanged.
- **App enable is executed, not grepped:** `the_entrypoint_enable_path_actually_allows_spending`
  runs the very `enable()` call and asserts `allowed()`. Ordering guarded by
  `enable_precedes_any_client_construction_in_run`.

### AC2 — `first_light`
- `cargo build -p hickeyfield-core --example first_light` → Finished, exit 0.
- Run WITHOUT flag, WITHOUT `FAL_KEY` (`HICKEYFIELD_ALLOW_SPEND` UNSET):
  ```
  first_light would run Qwen Image 3 and cost about $0.0400 for one image.
  refusing: this example spends real money. Set HICKEYFIELD_ALLOW_SPEND=1 (and FAL_KEY) …
  ```
  **exit code = 2** (non-zero). Estimate printed before the refusal.
- `grep -c "security find-generic-password" …/first_light.rs` → `0` (keychain-copy removed). Never run with a key.

### AC3 — repo-wide guard, verified independently
- Repo scan clean (49 files). Independent grep for `var(_os)?("FAL_KEY"|"hickeyfield_*_KEY")` finds
  only the three allowlisted readers (first_light.rs, live_fal_pricing.rs, vault.rs). `onboarding.rs`
  uses `"FAL_KEY"` only as a constant/parse_env test datum — not a process read — so the
  `var(`-scoped rule (implementer deviation) correctly does not flag it. No real reader is missed.

### AC4 — docs
CHANGELOG (SpendBlocked, lint-spend), README "Spending and credentials" + `HICKEYFIELD_ALLOW_SPEND`
+ lint-spend in gate list, FIRST-LIGHT flag instructions with keychain-copy removed — all present.

### UI surfacing of SpendBlocked
`submit_one` sets `job.fail_reason = Some(e.to_string())` on error; `ResultCard.tsx:94` renders
`job.failReason` verbatim. The SpendBlocked Display message (naming `HICKEYFIELD_ALLOW_SPEND=1`)
reaches the user as a clear, recoverable message.

## Break attempts

- **Env truthiness (independent, out-of-repo `allowed()`):** `[1]`→true, `[ 1 ]`→true (trimmed),
  `[0]`,`[true]`,`[yes]`,`[]`,`[01]`,`[10]`→false. Exact-`"1"` rule holds; no ambiguous "on" spelling.
- **lint-spend mutation battery (7 scratch files outside the repo):** each forbidden pattern fails
  exit 1 with a clear message — raw `var("FAL_KEY")`, `var("hickeyfield_OPENAI_KEY")`,
  `keyring::/get_password`, `remove_var("hickeyfield_…")`, `remove_var("FAL_KEY")`,
  `set_var("HICKEYFIELD_ALLOW_SPEND")`, `spend_gate::enable()`. A `src-tauri/src/vault.rs`-suffixed
  file reading `FAL_KEY` is correctly exempt (exit 0).
- **Mutation testing of the new Rust tests (repo copied outside the tree, mutated, re-run):**
  - M1 `decide()`→always-true: all 3 `spend_gate` tests FAILED. ✔ caught
  - M2 remove FalClient::submit gate: `a_fal_submit_is_blocked…` FAILED. ✔
  - M3 remove HiggsfieldClient::submit gate: `a_higgsfield…` FAILED. ✔
  - M4 remove HostedEnhancer::call gate: `a_hosted_llm_call_is_blocked…` FAILED. ✔
  - M5 weaken SpendBlocked message (drop env-var name): `a_fal…` FAILED. ✔
  - M6 delete the real `enable()` call from `run()`: `enable_precedes_any_client_construction_in_run`
    FAILED. ✔ (See finding below.)
- **`base_url` carve-out attack:** confirmed the only production hosted constructors
  (`harness.rs:238` `HostedEnhancer::new`, `commands.rs:1334` `ChatBackend::hosted`) build with
  `base_url = None`, so every real hosted call is gated. Every `with_base_url` caller is under
  `#[cfg(test)]` (enhancer tests; `harness.rs:428` inside `#[cfg(test)] mod tests` at 254). No
  production bypass.
- **Idempotency / re-runs:** cargo test, lints, and both proxy runs are deterministic across repeats.

## Findings (non-blocking)

- **F1 (quality, not a defect): `the_entrypoint_calls_spend_gate_enable` is partially vacuous.** It
  asserts `include_str!("lib.rs").contains("hickeyfield_core::spend_gate::enable();")`, but the file's
  own test module contains that exact string (an executed call + a literal), so the test still
  PASSES when the real `run()` call is deleted (verified in M6). This does NOT compromise
  money-safety (the default-OFF property is proven independently and is bulletproof) and the
  app-not-blocked regression IS still caught by the non-vacuous `enable_precedes_…` test, so at
  least one guard fails on deletion. Recommend a follow-up to make the call-presence test scan only
  the `run()` body. Not blocking: no AC depends on this test alone, and AC5's live launch is
  owner-verified.

## Root-cause diagnosis
None required — the money-safety claim holds. Default is OFF; every billable path (FalClient,
Higgsfield, hosted enhancer, hosted chat) returns SpendBlocked before any socket, proven by the
variant discriminator AND an independent unroutable-proxy harness that reaches the network only when
the flag is set. Examples refuse, the lint catches all four forbidden patterns, docs are present,
and the full gate is green (961/20/0, clippy clean, UI 312 + build). Deviations (extra hosted-chat
gate, `base_url` carve-out, `var(`-scoped key rule, cargo-deny omission) were each checked and are
sound. F1 is a test-quality nit with redundant coverage, not a broken acceptance criterion.

VERDICT: PASS
