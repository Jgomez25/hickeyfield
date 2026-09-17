# S18-live-spend-gate — IMPLEMENT (claim)

Author: implementer agent. Built exactly the design (`design.md`, D1–D6 + weakest points)
against `forge/s18-spend-gate`. Worked in the tree; no commit, no merge, no push. Every command
below exported the dummy keys + `HICKEYFIELD_NO_KEYCHAIN=1` and **never** `HICKEYFIELD_ALLOW_SPEND`.
`first_light.rs` was edited but **never run** (session rule). The packaged `.app` was built but
**not installed or opened**.

## Baseline (measured before any change)
`cargo test --workspace` → **950 passed / 20 ignored / 0 failed** (S17 released). UI: **312 passed**.
(The "9 ignored" in the brief is the ffmpeg/compositor sidecar subset; the workspace total is 20 —
5 compositor::real, 4 ffmpeg, 1 enhancer local-daemon, 1 live_fal_pricing integration, plus others.
That total is unchanged by this slice.)

## What I built (files changed)

New:
- `crates/hickeyfield-core/src/spend_gate.rs` — the gate primitive: `OnceLock<bool>` set by the
  entrypoint via `enable()`, pure `decide(process_flag, env_value)`, public `allowed()`. Exact-`"1"`
  env match. 3 truth-table tests.
- `scripts/lint-spend.py` — repo-wide guard (D3), scans tracked `.rs` (or file args for scratch
  proofs). Executable.

Changed (core):
- `crates/hickeyfield-core/src/lib.rs` — `pub mod spend_gate;`.
- `crates/hickeyfield-core/src/engine.rs` — new `JobError::SpendBlocked(String)`; `Display` arm;
  `is_retryable` unchanged (still only `Transient` → not retryable).
- `crates/hickeyfield-core/src/clients.rs` — `spend_blocked()` helper; gate as the **first
  statement** of `FalClient::submit` and `HiggsfieldClient::submit` (before `http()`); exhaustive
  arm added to `higgsfield_reason`; 2 blocked-before-HTTP tests.
- `crates/hickeyfield-core/src/enhancer.rs` — `hosted_spend_blocked()` helper; gate in
  `HostedEnhancer::call` **and** the `ChatBackend::Hosted` branch of `complete` (storyboard
  planner); Ollama branches untouched; 2 tests (blocked + stubbed-not-gated).
- `crates/hickeyfield-core/examples/first_light.rs` — refuses without the flag, prints the offline
  cheapest-image estimate first, exits 2; docstring rewritten (keychain-copy line removed);
  `cheapest_image_estimate()` helper.

Changed (src-tauri):
- `src-tauri/src/lib.rs::run` — `hickeyfield_core::spend_gate::enable()` as the **first** line, ahead
  of `AppState::init`; a `tracing::info!("spending enabled …")` line for regression visibility; 3
  wiring tests (`the_entrypoint_calls_spend_gate_enable`, `enable_precedes_any_client_construction_in_run`,
  `the_entrypoint_enable_path_actually_allows_spending`).
- `src-tauri/src/runner.rs` — `Runner::clients()` accessor (D4 seam).
- `src-tauri/src/app.rs::submit_to_provider` — takes `clients: &ClientFactory` and calls
  `clients(&route.id())` instead of the hard-wired `client_for` (D4).
- `src-tauri/src/commands.rs` — `submit_one` passes `runner.clients()`; `RecordingClient` fake +
  `submit_one_uses_the_injected_client` seam test.

Docs (D5): `CHANGELOG.md` `[Unreleased] > Added`; `README.md` "Spending and credentials" +
`lint-spend.py` in the gate list; `docs/FIRST-LIGHT.md` run instructions + keychain-copy line removed.

## Acceptance criteria — evidence

### AC1 — spend gate at every provider submit; `SpendBlocked` before any HTTP; app enables at startup
```
$ cargo test --workspace 2>&1 | grep -E "is_blocked|_spend|wiring|injected|decide|only_exactly|process_flag"
test spend_gate::tests::decide_is_off_by_default ... ok
test spend_gate::tests::only_exactly_1_env_allows ... ok
test spend_gate::tests::the_process_flag_allows ... ok
test clients::tests::a_fal_submit_is_blocked_before_any_http ... ok
test clients::tests::a_higgsfield_submit_is_blocked_before_any_http ... ok
test spend_gate_wiring_tests::the_entrypoint_calls_spend_gate_enable ... ok
test spend_gate_wiring_tests::enable_precedes_any_client_construction_in_run ... ok
test spend_gate_wiring_tests::the_entrypoint_enable_path_actually_allows_spending ... ok
```
The `*_is_blocked_before_any_http` tests assert the variant is `SpendBlocked` and **not**
`Transient`/`Permanent`; a real POST to `queue.fal.run` / `api.higgsfield.ai` with a dummy key
would return `Transient` (connection) or `Permanent` (HTTP 401), so `SpendBlocked` proves the gate
short-circuited before `http()` built a client — no socket (D6). See "Weakest points" for why this
is stronger than the env-proxy variant the brief suggested.

### AC2 — examples refuse without the flag, print cost, no keychain-copy
`first_light.rs` was NOT run (session rule). Static proof of the required edits:
```
$ grep -c "security find-generic-password" crates/hickeyfield-core/examples/first_light.rs
0                       # keychain-copy instruction removed
$ grep -n "spend_gate::allowed\|would run\|refusing" crates/hickeyfield-core/examples/first_light.rs
42:    if !hickeyfield_core::spend_gate::allowed() {
45:      eprintln!("first_light would run {name} and cost about ${usd:.4} for one image.")
50:      "refusing: this example spends real money. Set HICKEYFIELD_ALLOW_SPEND=1 (and FAL_KEY) ..."
```
The refusal calls `spend_gate::allowed()` (unit-tested), prints the offline estimate first, then
exits 2. `submit` is gated too, so a bypass still cannot spend. The example compiles clean under
`cargo clippy --all-targets`. Keyless examples untouched — `audit_fal` proven below.

### AC3 — repo-wide guard
```
$ python3 ./scripts/lint-spend.py
lint-spend: ok (48 file(s) scanned).
```
Each of the four rules proven to FAIL on a scratch mutation **outside the repo**
(`$SCRATCH/lint-scratch/*.rs`, exit 1 each):
- `keyring::`/`get_password` outside vault.rs → caught.
- `std::env::var("FAL_KEY")` / `var("hickeyfield_HIGGSFIELD_KEY")` outside the allowlist → caught.
- `remove_var("hickeyfield_FAL_KEY")` / `remove_var("FAL_KEY")` outside vault.rs → caught.
- `set_var("HICKEYFIELD_ALLOW_SPEND", …)` (anywhere) → caught.
- `spend_gate::enable()` outside `src-tauri/src/lib.rs` → caught.
Allowlist is exactly `vault.rs`, `first_light.rs`, `live_fal_pricing.rs`. Verified
`crates/hickeyfield-core/tests/live_fal_pricing.rs:34` legitimately reads `hickeyfield_FAL_KEY`/`FAL_KEY`
and is `#[ignore]` (read-only pricing GET).

### AC4 — docs
`CHANGELOG.md` `[Unreleased] > Added`, `README.md` "Spending and credentials" (+ `lint-spend.py` in
the gate list), `docs/FIRST-LIGHT.md` (flagged run + keychain-copy line removed). Provenance lint
still PASS after the edits.

### AC5 — regression gate (all green, pasted)
```
cargo test --workspace   → 961 passed / 20 ignored / 0 failed   (baseline 950 + 11 new)
cargo fmt --all --check  → fmt-ok
cargo clippy --workspace --all-targets -- -D warnings → Finished (no warnings)
./scripts/lint-provenance.py → PASS (CDN hosts + copy provenance)
python3 ./scripts/lint-spend.py → ok (48 files)
env -u hickeyfield_FAL_KEY -u FAL_KEY HICKEYFIELD_NO_KEYCHAIN=1 \
  cargo run -p hickeyfield-core --example audit_fal → "70 endpoint(s) audited …" (keyless, unchanged)
cd ui && pnpm test  → 312 passed (23 files)   [Node 24.8.0]
cd ui && pnpm build → built in 3.83s
ui/node_modules/.bin/tauri build --bundles app →
    Finished `release` profile [optimized] target(s) in 10m 29s
    Bundling Hickeyfield.app (…/target/release/bundle/macos/Hickeyfield.app)
    Finished 1 bundle at: …/target/release/bundle/macos/Hickeyfield.app
    [exited with code 0]
```
Ignored count unchanged (20). App was **built, not installed or opened**. The final AC5 proof —
the owner launching the installed app and seeing a normal cost estimate rather than a refusal — is
owner-run, not agent-run.

## Weakest points (addressed with evidence)

1. **The packaged app must never be accidentally blocked.** `enable()` is the first line of `run()`,
   ahead of `AppState::init` (the client factory), with a startup log line. Two tests pin it:
   `enable_precedes_any_client_construction_in_run` (source order — matches `AppState::init(` with the
   paren so it does not trip on the comment), and `the_entrypoint_enable_path_actually_allows_spending`
   which **executes** the very `enable()` call `run()` makes and asserts `allowed()`. That execution
   lives in the src-tauri test binary, which has no test asserting a real-client block (those are in
   the separate core binary), so flipping the process-global `OnceLock` there poisons nothing. Final
   proof is owner-run (launch the installed app).
2. **Hosted enhancer gate (D1b).** Implemented in `HostedEnhancer::call`. I ALSO gated the second
   hosted LLM path the design named only in passing — `ChatBackend::Hosted` in `complete()` (the
   storyboard planner) — same primitive, since it also spends tokens (see Deviations). Ollama stays
   ungated (free). Tests: `a_hosted_llm_call_is_blocked_without_the_flag` (both seams blocked, message
   names the flag) and `a_stubbed_hosted_call_is_not_gated_so_wire_tests_run_without_the_flag`.
3. **`SpendBlocked` exhaustive-match fan-out.** Only two exhaustive matches on `JobError` exist:
   `engine.rs` `Display` and `clients.rs::higgsfield_reason` (both fixed). All other sites route
   through `is_retryable()` or a `_` wildcard. `cargo clippy --all-targets -D warnings` (which
   compiles every target) is the proof the enum change broke nothing. UI: no exhaustive JobError
   mapping — `ResultCard.tsx:94` renders `failReason` verbatim, so the `SpendBlocked` Display string
   ("spending is disabled in this process, so no request was sent …") reaches the user as-is.
4. **`lint-spend.py` allowlist = exactly the three current readers.** Confirmed; scratch proofs above.
5. **Gate before any connection.** The variant-discriminator proof (SpendBlocked, not Transient) —
   see AC1 / Deviations for why this is stronger than an env-proxy hack.

## Deviations from the design (proposed, not silent)

- **Second hosted LLM path gated too.** The design's D1b named `HostedEnhancer::call`; the same
  billable reasoning applies to `ChatBackend::Hosted` in `enhancer.rs::complete` (hosted storyboard
  planning). I gated both hosted branches and left both Ollama branches free. Strictly closes more,
  in the spirit of "structural". If the verifier judges this scope creep, it is one guard clause to
  drop — but leaving it open would contradict the slice's own goal.
- **Hosted gate is scoped to a real host (`base_url.is_none()`).** `with_base_url` is a documented
  test-only seam (verified: production builds `HostedEnhancer::new`/`ChatBackend::hosted` with
  `None` in `harness.rs` and `commands.rs`). Gating unconditionally would break the two existing
  wire-path tests (`a_hosted_openai_call_posts_to_the_overridden_base…`), which point at a local stub
  and legitimately spend nothing — and I cannot enable spending in a test (lint forbids it). So
  "spend" = a request to a real provider host; every real hosted call (always `None`) is still gated.
- **`SpendBlocked` verified via the variant discriminator, not an env-set proxy.** The brief suggested
  a real client + dummy key + unroutable `127.0.0.1:9` proxy. Because the gate is `submit`'s first
  statement, **no `reqwest::Client` is ever built**, so a proxy would never be consulted — the proxy
  variant proves nothing the discriminator does not, and setting a global env var in a parallel test
  binary is itself the env-mutation anti-pattern the 2026-09-16 incident warns against. The tests
  therefore assert `SpendBlocked` AND `!Transient && !Permanent`, which is the "SpendBlocked, not a
  connection error" the brief asked for, achieved without touching global env or the network.
- **`lint-spend.py` rule 2 matches `env::var`/`var_os` reads, not the bare `\bFAL_KEY\b` substring.**
  `onboarding.rs` legitimately lists `"FAL_KEY"` as a recognized env-name for the `.env` import
  feature and uses it as `parse_env` test data — neither reads the process env. A bare-substring rule
  false-positives on it; matching the actual read (`var("FAL_KEY")`) is precisely "raw key read" and
  keeps the allowlist at exactly three files.
- **`remove_var` rule scoped to "outside vault.rs".** The design said "anywhere", but `vault.rs`'s own
  keychain-guard tests (the incident fix, commit `45f2416`) legitimately `remove_var("hickeyfield_…")`
  to prove the guard. A blanket "anywhere" would fail the very regression tests that prevent the
  incident, so vault.rs (already the allowlisted owner of these env vars) is exempt for this rule too.
- **`lint-spend.py` NOT wired into `.forge/gate.py`.** The instruction said to wire it in "if that is
  where the other lints live". It is not: `gate.py` runs no mechanical commands (no cargo/provenance/
  pnpm) — it is a docs/evidence phase-state checker. `lint-spend.py` belongs in the mechanical gate
  command list alongside `lint-provenance.py`, and is added to the README gate list.

## Known limitations (not fixed, honestly logged)
- **D4 seam is the seam + one seam test only** (design Q2 default). Full still/shot submit-loop
  offline coverage (writing `job_set_id` per record, all-shots-pending) is the larger half of S17 M4
  and is deferred to a follow-up. `submit_one_uses_the_injected_client` proves the seam end-to-end
  (one real model through compile→guard→estimate→route→upsert→submit into the fake, no network).
- **No per-day budget / rate-limit / spend cap** — out of scope (design Non-goals); the gate is
  binary (allowed/blocked). Logged as a backlog follow-up would be appropriate.
- **`cargo deny check` not run** — not in the task's gate command list (the design mentioned it; the
  task's authoritative list does not). Everything in the task list is green.
- **AC5 app-launch is owner-only.** I built the bundle (exit 0) but did not install or open it; the
  "still generates with a normal estimate" proof is the owner's.
