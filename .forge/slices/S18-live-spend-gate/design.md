# S18-live-spend-gate — DESIGN

Scope: THIS slice only. Make spending money **structurally impossible** without an explicit,
per-process opt-in, while the packaged app keeps generating unchanged. Grounded in the
2026-09-16 real-spend incident (`.forge/STATUS.md` "INCIDENT 2026-09-16"), the vault hotfix
(commit `45f2416`, `src-tauri/src/vault.rs::keychain_blocked`), S17 security.md's MEDIUM on
`first_light.rs`, and S17 review.md's M4 (no fake-provider seam).

This artifact is DESIGN. It changes no source; it specifies the edits the implementer will make.
The verifier (forge-doc-verifier) starts at "Riskiest parts / weakest".

---

## 0. Context I verified before designing

- **The only credential read is `vault::get`** (`src-tauri/src/vault.rs:58`), already guarded by
  `keychain_blocked()` (`cfg!(test)` OR truthy `HICKEYFIELD_NO_KEYCHAIN`). This slice does **not**
  touch the vault; it is the layer *below* the vault (what happens once a key exists).
- **`hickeyfield-core` owns the HTTP clients** (`crates/hickeyfield-core/src/clients.rs`), not
  `src-tauri`. The slice contract's "provider clients live in `src-tauri/src/clients.rs`" is
  wrong on path; the real file is `crates/hickeyfield-core/src/clients.rs`. This matters for the
  choke-point decision below: core is reachable by examples that bypass `src-tauri` entirely.
- **The billable POSTs are exactly two**, confirmed by grep of `.post(`/`.put(` in
  `crates/hickeyfield-core/src`:
  - `clients.rs:194` — `FalClient::submit` → `POST https://queue.fal.run/{model_id}` (a generation).
  - `clients.rs:353` — `HiggsfieldClient::submit` → `POST https://api.higgsfield.ai/{model_id}`.
  - Plus one **LLM** billable path outside the `ProviderClient` trait: `enhancer.rs:1361`
    `HostedEnhancer::call` → `POST` to OpenAI/Anthropic (`enhancer.rs:1370,1382`). This is real
    spend (LLM tokens) and is easy to overlook — see D1b.
  - Not billable and therefore **not gated**: fal poll `GET .../status` and result `GET`
    (`clients.rs:216-233`), cancel `PUT/POST .../cancel` (`clients.rs:249,403`), fal upload
    initiate `POST /storage/upload/initiate` + signed `PUT` (`clients.rs:567,591`), Higgsfield
    `POST /files/generate-upload-url` + `PUT` (`clients.rs:626,646`), and every Ollama call
    (`enhancer.rs`, localhost, free).
- **The submit path does not use the injectable factory.** `AppState` builds a
  `ClientFactory` and hands it to the `Runner` (`app.rs:147`) for *polling*, but the submit
  itself calls the free function `client_for` directly (`app.rs:276`), so the still/shot submit
  loops (`commands.rs::submit_one` at `commands.rs:1091` → `app::submit_to_provider` at
  `app.rs:260,633`) have no test seam — exactly S17 M4. See D4.
- **`first_light.rs` is the only key-reading example** (grep of `crates/hickeyfield-core/examples`
  for `FAL_KEY`/`hickeyfield_*_KEY`/`env::var`): only `first_light.rs:31` (`FAL_KEY`) and
  `first_light.rs:88` (`MODEL`, harmless). `audit_fal`, `fal_diff`, `price_check`, `tryharness`,
  `dump_catalog`, `dump_fal_slugs` are keyless. `tests/live_fal_pricing.rs:34` reads a key but is
  `#[ignore]` and hits only the read-only pricing GET (no generation) — a documented seam, kept.

### Term: "spend" (used consistently below)
**Spend** = any HTTP request that can create a **billable** generation or a billable model
completion. Precisely: the provider *submit* POST (`FalClient::submit`, `HiggsfieldClient::submit`)
and the *hosted-LLM* completion POST (`HostedEnhancer::call`). Polls, result fetches, cancels,
uploads (fal/Higgsfield storage is free), local endpoints (ComfyUI/Ollama), and read-only pricing
GETs are **not** spend and stay ungated — so a resumed job on relaunch can still be polled and
downloaded (that path is not billed) even in a process that never opted in.

---

## D1 — The gate primitive: a process-local flag consulted at the single billable choke point

### D1a. New module `crates/hickeyfield-core/src/spend_gate.rs` (add `pub mod spend_gate;` to
`crates/hickeyfield-core/src/lib.rs`).

The gate MUST live in **core**, not `src-tauri`, because the examples that can spend
(`first_light`) link core directly and never see `src-tauri`. A gate in `src-tauri` would not
cover them — the very path S17 security.md flagged.

Shape (normative):

```rust
//! One switch that decides whether this process may spend money.
use std::sync::OnceLock;

/// Set exactly once, by the packaged app's Tauri entrypoint. Unset in every
/// other process (tests, examples, integration crates), so the default is OFF.
static ALLOWED: OnceLock<bool> = OnceLock::new();

/// Called once from `src-tauri/src/lib.rs::run` — and nowhere else.
pub fn enable() {
    let _ = ALLOWED.set(true);
}

/// The pure decision, split out so the truth table is testable WITHOUT setting
/// the real global or the real env (both of which are banned in tests by D3).
pub(crate) fn decide(process_flag: Option<bool>, env_value: Option<&str>) -> bool {
    if let Some(v) = process_flag {
        return v;
    }
    matches!(env_value.map(str::trim), Some("1"))
}

/// True when this process may make a billable request.
pub fn allowed() -> bool {
    decide(
        ALLOWED.get().copied(),
        std::env::var("HICKEYFIELD_ALLOW_SPEND").ok().as_deref(),
    )
}
```

Rules:
- `allowed()` MUST return `false` by default. It returns `true` only when the entrypoint called
  `enable()` **or** the process was started with `HICKEYFIELD_ALLOW_SPEND` set to exactly `"1"`
  (trimmed). Any other env value (`"0"`, `"true"`, `"yes"`, empty) is OFF — exact-match `"1"` is
  deliberately stricter than `keychain_blocked`'s truthy parse, to remove ambiguity about a
  money switch.
- Tests MUST NOT call `enable()` and MUST NOT set `HICKEYFIELD_ALLOW_SPEND` (enforced by D3).
- Worked truth table (the D1 test):
  `decide(None, None) == false` (a test) · `decide(None, Some("1")) == true` (a keyed CLI/example
  run) · `decide(None, Some("0")) == false` · `decide(None, Some("true")) == false` ·
  `decide(Some(true), None) == true` (the packaged app after `enable()`).

### Why `OnceLock<bool>` set by the entrypoint, NOT the app mutating its own env
- `std::env::set_var` is **process-global and unsynchronized**; the app is multi-threaded (the
  `Runner` spawns poll threads) and reading env concurrently with a writer is a data race —
  Rust is making `set_var` `unsafe` for exactly this reason. `OnceLock::set` is race-free.
- An env var the app sets **leaks into child processes** — the app spawns the bundled ffmpeg
  sidecar — and into anything that inspects the environment. A process-local `OnceLock` leaks
  nowhere.
- The `OnceLock` is set by the **one true entrypoint** (`lib.rs::run`, which carries the
  `#[cfg_attr(mobile, tauri::mobile_entry_point)]` attribute, so both desktop and mobile launches
  cross it) and **cannot be forgotten by any other code path** or flipped back off. Every other
  entrypoint (unit tests, integration crates, examples, a future CLI) leaves it unset, so their
  default is OFF **by construction** — the failure mode is "blocked", never "silently spends".
- It cannot be flipped by test code: a test cannot un-set an `OnceLock`, and D3 forbids calling
  `enable()` outside `lib.rs`, so a test that tried would fail the lint before it could poison the
  shared test binary.

### D1a (cont). The choke point: `ProviderClient::submit`, not the constructor
The gate MUST be consulted as the **first statement** of each real `submit` implementation
(`FalClient::submit` at `clients.rs:191`, `HiggsfieldClient::submit` at `clients.rs:350`),
returning before `http()` is built or any POST is issued:

```rust
fn submit(&self, model_id: &str, body: &Value) -> Result<Submission, JobError> {
    if !crate::spend_gate::allowed() {
        return Err(JobError::SpendBlocked(format!(
            "spending is disabled in this process, so no request was sent to {model_id}. \
             The packaged Hickeyfield app enables spending automatically; to allow it from the \
             CLI or an example, start the process with HICKEYFIELD_ALLOW_SPEND=1."
        )));
    }
    /* … existing body … */
}
```

**Why `submit`, not the constructor** (D1's explicit question):
- `submit` is the single point **every** billable generation crosses, from either direction —
  `src-tauri`'s `submit_to_provider` (`app.rs:633`) *and* `first_light.rs:141`
  (`client.submit(&endpoint, &body)`), which bypasses `src-tauri`. Proven single: the two
  `.post(` billable sites are both inside these two `submit` bodies (grep above).
- Gating the **constructor** (`FalClient::new`) would over-block: `uploader_for`
  (`app.rs:72-92`) builds a `FalClient` purely to *upload* (free), and the app must upload media
  even before it decides to submit. Gating the constructor would refuse a free operation and
  couple the switch to object creation rather than to the billable act. `submit` is both
  narrower and exactly the billable act.
- MAY add a belt-and-suspenders assertion in the constructor later; NOT in scope here (a single
  well-placed gate is easier to verify than two that can drift).

### D1a (cont). New error variant `JobError::SpendBlocked(String)`
In `crates/hickeyfield-core/src/engine.rs` (enum at `engine.rs:230`):
- Add `SpendBlocked(String)` with a doc comment ("The process did not opt in to spending; no
  billable request was made.").
- `is_retryable` (`engine.rs:239`) MUST keep returning `false` for it (it already only matches
  `Transient`; SpendBlocked is not retryable — retrying without the flag re-blocks).
- `Display` (`engine.rs:256`) MUST add an arm printing the message.
- **Exhaustive-match fixups** (compile-required): `higgsfield_reason` (`clients.rs:314-317`)
  matches all variants — add `JobError::SpendBlocked(_) => return e` (it never actually receives
  one, since the gate precedes the POST, but the match must stay exhaustive). The implementer
  MUST grep `match` on `JobError` across `src-tauri` + `crates` and add a benign arm wherever the
  match is exhaustive; error-classification that already routes through `is_retryable()` or a
  wildcard `_` needs no change.

### D1b — The second billable choke point: `HostedEnhancer::call`
DECIDE (with rationale): gate it too, same primitive.
`HostedEnhancer::call` (`enhancer.rs:1361`) POSTs to OpenAI/Anthropic with a paid key. The owner
priority is "spending must be impossible without an explicit opt-in — structural". A submit-only
gate would leave a live LLM-spend path open, so `call` MUST check `spend_gate::allowed()` as its
first statement and return `Err("spending is disabled …".to_string())` otherwise (its signature
is `Result<String, String>`, not `JobError`). The local Ollama backend
(`enhancer.rs:1072,1629`) is free and MUST stay ungated so the offline enhancer keeps working
without the flag. This is the part most exposed to scope debate — see Riskiest parts.

---

## D2 — Examples: `first_light.rs` refuses without the flag and stops teaching key-copying

Edits to `crates/hickeyfield-core/examples/first_light.rs` (design only; **do not run it**):
1. **Refuse early.** Before reading `FAL_KEY` (currently `first_light.rs:31`), add:
   `if !hickeyfield_core::spend_gate::allowed() { eprintln!("{estimate line}"); eprintln!("refusing: set HICKEYFIELD_ALLOW_SPEND=1 to run a real generation"); std::process::exit(2); }`
   — and it MUST print the **estimated cost first** (it already computes `estimate` for the
   cheapest route at `first_light.rs:97,119`; move/duplicate a one-line "would cost ~$X" print
   into the refusal so a reader sees the number before opting in). Because `submit` is *also*
   gated, even if this refusal were bypassed the POST at `first_light.rs:141` still returns
   `SpendBlocked` — defence in depth.
2. **Read the key via a documented env only, never keychain-copy instructions.** Keep reading
   `FAL_KEY` (the documented dev seam, same var the README/objective already sanction), but
   **delete the docstring lines `first_light.rs:19-22`** that instruct
   `FAL_KEY=$(security find-generic-password -s ai.hickeyfield.keys -a fal -w) …`. Replace with:
   "Set `FAL_KEY` yourself (never copy it out of the keychain into your shell history) and set
   `HICKEYFIELD_ALLOW_SPEND=1` to acknowledge this spends real money." This is S17 security.md's
   exact recommendation.
3. **Keyless examples untouched.** `audit_fal`, `fal_diff`, `price_check`, `tryharness`,
   `dump_catalog`, `dump_fal_slugs` are not edited; D3's lint proves they stay keyless.

MAY (SHOULD-not this slice): `tests/live_fal_pricing.rs` reads a key but is `#[ignore]` and hits
only the read-only pricing GET (no generation). It is not "spend" by our definition; leave it, but
D3 allowlists its documented read so the lint does not flag it.

---

## D3 — Repo-wide guard: `scripts/lint-spend.py`, wired into the mechanical gate

DECIDE: a standalone Python linter (mirroring `scripts/lint-provenance.py`), not a Rust test.
Rationale: it must scan **the whole tree including examples, tests, and non-Rust files**, run
from the gate command list, and give one exit code — a Rust `#[test]` only sees its own crate's
compiled sources and cannot cheaply police the examples/tests of another crate.

`scripts/lint-spend.py` MUST scan tracked `.rs` files and fail (exit 1, naming file:line) on:
1. `keyring::` or `get_password` **outside** `src-tauri/src/vault.rs`.
2. A read of `FAL_KEY` or `hickeyfield_*_KEY` (regex `\bFAL_KEY\b` and
   `hickeyfield_[A-Z0-9_]*_KEY`) **outside** the allowlist:
   - `src-tauri/src/vault.rs` (the vault's `env_override`/guard tests),
   - `crates/hickeyfield-core/examples/first_light.rs` (the one documented `FAL_KEY` seam),
   - `crates/hickeyfield-core/tests/live_fal_pricing.rs` (documented read-only pricing seam).
3. `remove_var("hickeyfield_` **anywhere** (the incident's env-stripping pattern), and
   `remove_var("FAL_KEY` anywhere.
4. `HICKEYFIELD_ALLOW_SPEND` being **set** in test/any code: fail on `set_var("HICKEYFIELD_ALLOW_SPEND`
   anywhere, and on `spend_gate::enable(` **outside** `src-tauri/src/lib.rs`.

Worked example (what a violation looks like):
`crates/.../foo.rs:42: std::env::set_var("HICKEYFIELD_ALLOW_SPEND", "1")` → lint prints
`lint-spend: foo.rs:42 sets HICKEYFIELD_ALLOW_SPEND — a test must never enable spending` → exit 1.

The linter MUST allow references inside string literals / comments only where unavoidable; to
keep it simple and strict it treats the four patterns as substring hits and relies on the
allowlist for the two legitimate readers. It MUST be added to the gate command list (below) and
run green on the S18 tree.

---

## D4 — Fake-provider seam (S17 M4), scoped minimally

The seam is: **route the submit through the injectable `ClientFactory` that already exists**,
instead of the hard-wired free function.
- Today: `app::submit_to_provider` (`app.rs:260`) calls `client_for(&route.id())` directly
  (`app.rs:276`), so no test can substitute a client. The `ClientFactory` type
  (`runner.rs:44`, `Arc<dyn Fn(&str) -> Option<Arc<dyn ProviderClient>> + Send + Sync>`) is
  already handed to the `Runner` but not to submit.
- Change (minimal): store the `ClientFactory` on `AppState` (it is already constructed at
  `app.rs:147`), and give `submit_to_provider` an explicit `clients: &ClientFactory` parameter
  (or thread it through `submit_one`), replacing the direct `client_for` call with
  `clients(&route.id())`. Production wiring passes `state.clients`; a test passes a fake factory.
- The fake: a `RecordingClient` implementing `ProviderClient` whose `submit` records
  `(endpoint, body)` into a shared `Vec` and returns a canned `Submission` **without any HTTP** —
  the offline seam S17 M4 asked for. A worked unit test `submit_one_uses_the_injected_client`
  drives one still through `submit_one` with the fake factory and asserts exactly one recorded
  submit with the expected endpoint, no network.

Scope call: this slice lands **the seam and one seam test**. Driving the *full* still/shot submit
loops offline (writing `job_set_id` back per record, all-shots-`pending`) is larger and MAY be a
follow-up — recorded as an Open question default, not silently dropped. The seam is the
prerequisite either way.

---

## D5 — Docs (specified here; the DOCUMENT phase makes the edits)

- `README.md`: a short "Spending and credentials" note documenting the two flags together —
  `HICKEYFIELD_NO_KEYCHAIN=1` (never read the login keychain; every agent/CI run sets it) and
  `HICKEYFIELD_ALLOW_SPEND=1` (the only way any binary/example spends; the packaged app sets it
  itself at startup so the owner is unaffected) — and the rule: *no process spends unless it
  opted in; tests and examples never opt in.*
- `docs/FIRST-LIGHT.md`: update the run instructions to `HICKEYFIELD_ALLOW_SPEND=1 FAL_KEY=…
  cargo run -p hickeyfield-core --example first_light`, and remove any keychain-copy line to
  match the docstring change (D2).
- `CHANGELOG.md`: `[Unreleased] > Added` — the spend gate, the `SpendBlocked` error, the
  example refusal, and the `lint-spend` gate.

(This invocation writes **only** `design.md`; the above are the documenter's checklist.)

---

## Per-AC named tests

**AC1 — gate + `SpendBlocked` before any HTTP; app enables at startup**
- `spend_gate::decide_is_off_by_default` — `decide(None, None) == false`.
- `spend_gate::only_exactly_1_env_allows` — `"1"`→true; `"0"`,`"true"`,`""`→false.
- `spend_gate::the_process_flag_allows` — `decide(Some(true), None) == true` (documents the
  entrypoint state; does **not** touch the real global).
- `clients::a_fal_submit_is_blocked_before_any_http` — `FalClient::new("dummy").submit("fal-ai/x",
  &json!({}))` in a `cfg(test)` build (flag off) returns `JobError::SpendBlocked`, and
  `assert!(!matches!(err, JobError::Transient(_) | JobError::Permanent(_)))` — a real network
  attempt would yield a *different* variant, so `SpendBlocked` proves the gate short-circuited
  before the POST. (See D6 for the unroutable-proxy hardening.)
- `clients::a_higgsfield_submit_is_blocked_before_any_http` — same for `HiggsfieldClient`.
- `enhancer::a_hosted_llm_call_is_blocked_without_the_flag` — `HostedEnhancer::openai(...).call("x")`
  returns `Err` mentioning `HICKEYFIELD_ALLOW_SPEND`; the Ollama backend is unaffected.
- `lib.rs` wiring assertion `the_entrypoint_calls_spend_gate_enable` — a source-string check
  (like `app.rs`'s `has_adapter_matches_the_clients_actually_implemented`) that `run()`'s body
  contains `spend_gate::enable()`; we cannot execute `run()` in a test, and we must not flip the
  real `OnceLock` (it would poison sibling tests).

**AC2 — examples refuse without the flag, print cost, no keychain-copy**
- Covered by the shared `spend_gate` tests (the refusal reads `allowed()`), plus D3's lint
  asserting `first_light.rs` no longer contains `security find-generic-password` and does contain
  a `spend_gate::allowed()` guard.
- Verifier step (money-safe, human/agent-run under standing rules — no `FAL_KEY`, no
  `HICKEYFIELD_ALLOW_SPEND`): `cargo run -p hickeyfield-core --example first_light` exits non-zero
  with the refusal message and the printed estimate; **never actually run it with a key**.

**AC3 — repo-wide guard**
- `scripts/lint-spend.py` exits 0 on the S18 tree; the design's four rules each have a documented
  violating example the linter would catch.

**AC4 — docs** — presence check on README/FIRST-LIGHT/CHANGELOG (DOCUMENT phase).

**AC5 — regression** — full gate green (below); the app builds, launches, and can still generate
because `run()` calls `enable()`. Owner-verified, not agent-verified.

**D4 seam** — `submit_one_uses_the_injected_client` (offline, recording fake).

---

## D6 — Verifying without money

- **The gate blocks (before any connection):** the AC1 `clients::*_is_blocked_before_any_http`
  tests assert `SpendBlocked`. To make "before any connection" rigorous rather than inferred, the
  test MAY additionally set an unroutable proxy for the process
  (`reqwest`'s `HTTPS_PROXY` honoured by default, e.g. `http://127.0.0.1:1`) and still get
  `SpendBlocked` — proving the gate returns before `http()`/`send` runs (a proxy error would be
  `Transient`). Because the gate is `submit`'s first statement, no `reqwest::Client` is even
  built. This is the "real client + dummy key + unroutable, assert `SpendBlocked` first" the task
  asks for, achieved without a live endpoint.
- **The packaged app is allowed:** `decide(Some(true), None) == true` (the state `enable()`
  produces) plus the `lib.rs` wiring assertion that `run()` calls `enable()`. Together they show
  the app's process is in the allowed state without running Tauri.

---

## Order of work (for the implementer)

1. `spend_gate.rs` + `pub mod spend_gate;` in core `lib.rs`; the `decide` truth-table tests.
2. `JobError::SpendBlocked` in `engine.rs` (+ `Display`, keep `is_retryable` false); fix the
   exhaustive matches (`clients.rs:314` first, then grep the rest).
3. Gate `FalClient::submit` and `HiggsfieldClient::submit` (`clients.rs`); the two
   `*_is_blocked_before_any_http` tests.
4. Gate `HostedEnhancer::call` (`enhancer.rs`); its blocked test; confirm Ollama untouched.
5. `first_light.rs`: refusal + estimate print + docstring rewrite (D2).
6. `src-tauri/src/lib.rs::run`: `hickeyfield_core::spend_gate::enable()` as the **first** line of
   `run()`, and a startup `tracing::info!("spending enabled for this process")` so a regression is
   visible in the log; the wiring assertion test.
7. D4 seam: `ClientFactory` on `AppState`, thread into `submit_to_provider`/`submit_one`, the
   `RecordingClient` fake + seam test.
8. `scripts/lint-spend.py`; wire into the gate; run green.
9. Docs (DOCUMENT phase).

---

## Exact gate (all must pass; every command exports the dummy keys + `HICKEYFIELD_NO_KEYCHAIN=1`
and **never** `HICKEYFIELD_ALLOW_SPEND`; no hosted calls; no money)

Preamble for every command:
`export HICKEYFIELD_NO_KEYCHAIN=1 hickeyfield_FAL_KEY=x hickeyfield_HIGGSFIELD_KEY=x hickeyfield_GOOGLE_KEY=x hickeyfield_OPENAI_KEY=x hickeyfield_XAI_KEY=x hickeyfield_BFL_KEY=x hickeyfield_RECRAFT_KEY=x hickeyfield_VAIG_KEY=x`

1. `cargo test --workspace` — 0 failed; baseline held (>= the S17 count; new tests add to it).
2. `cargo fmt --all --check`
3. `cargo clippy --workspace --all-targets -- -D warnings`
4. `cargo deny check`
5. `./scripts/lint-provenance.py`
6. `python3 ./scripts/lint-spend.py` — the new guard, exit 0.
7. Keyless audit: `env -u hickeyfield_FAL_KEY -u FAL_KEY cargo run -p hickeyfield-core --example audit_fal`
   — unchanged, keyless.
8. UI, Node 24 at
   `/private/tmp/claude-501/-Users-jorge-Documents-GitHub-hickeyfield/79be25e2-8f71-4192-9e6d-cd0e9d068f1c/scratchpad/node-v24.8.0-darwin-arm64/bin`:
   `cd ui && pnpm test && pnpm build`.
9. Owner-only (AC5): the packaged `.app` builds, launches, and still generates (flag set at
   startup). Not agent-run.

Note on the running `tauri build`: a build is in flight from this working tree; the implementer
must not touch `ui/`, `src-tauri/`, or `crates/` while it runs. This DESIGN touches none of them.

---

## Non-goals (recorded, not skipped)
- Per-day budgets, rate limits, or a spend meter/cap — **N/A this slice**; logged as a follow-up
  in `.forge/backlog.md`. The gate is binary (allowed / blocked), which is what "impossible
  without opt-in" requires; budgets are a refinement on top.
- Changing `vault.rs` / `keychain_blocked` — **N/A**; the vault hotfix stands and is the layer
  above this one.
- A UI surface for the flag — **N/A**; the packaged app opts in automatically, so there is
  nothing for the owner to toggle. Deliberately no webview exposure of a spend switch.
- Gating polls/uploads/cancels/downloads — **N/A by definition of "spend"**; gating them would
  break resume-and-download on relaunch for no money-safety gain.

## Open questions (each with a recorded default)
- **Q1 — Gate the hosted enhancer (D1b) in this slice, or defer?** DEFAULT: **gate it now.** It
  is a live LLM-spend path and the same one-line primitive; leaving it open contradicts
  "structural". If the verifier judges it scope creep, the fallback is to land the
  `ProviderClient::submit` gate + `SpendBlocked` this slice and file the enhancer gate as an
  immediate follow-up — but the default is to close it now.
- **Q2 — Full still/shot submit-loop offline tests (beyond the seam)?** DEFAULT: **land the seam
  + one seam test now; defer the full-loop coverage** to a follow-up (it is the larger half of
  S17 M4). The seam is the blocker; the extra tests are additive.
- **Q3 — Exact env truthiness: only `"1"`?** DEFAULT: **yes, exactly `"1"`** (stricter than
  `keychain_blocked`), because a money switch should have no ambiguous "on" spellings.
- **Q4 — Should `enable()` also be idempotent-safe if called twice (e.g. mobile + desktop
  attrs)?** DEFAULT: **yes** — `OnceLock::set` returns `Err` on the second call and we ignore it
  (`let _ =`), so a double-enable is harmless.

## Riskiest parts / weakest (verifier: start here)
1. **A gate that accidentally blocks the real app** — the failure to design against. Mitigations:
   `enable()` is the **first** line of the single entrypoint `run()` (which both desktop and
   mobile launches cross); a startup log line makes a regression visible; the `SpendBlocked`
   message names the exact env var so even a broken build is recoverable by the owner; and the
   `decide(Some(true), None) == true` + wiring tests pin the contract. Still, this is the highest
   consequence: if a refactor moves generation off `run()` or drops the `enable()` call, the app
   silently refuses to generate. Weakest link: the wiring test is a source-string check, not an
   execution — it proves the call *exists*, not that it *runs first*.
2. **D1b scope (enhancer gate)** — the most debatable inclusion; it is a second, non-obvious
   billable path. If the verifier disagrees on scope, Q1's fallback applies.
3. **`SpendBlocked` exhaustive-match fan-out** — adding an enum variant can break compilation in
   matches I did not enumerate; I named `clients.rs:314` explicitly and instructed a repo grep,
   but an unlisted exhaustive match is a plausible miss.
4. **`lint-spend.py` substring strictness vs. false positives** — a coarse substring linter could
   flag a legitimate future mention of `FAL_KEY` in a comment; the allowlist is file-scoped, so a
   new legitimate reader would need an allowlist entry (intended friction for a money-adjacent
   read, but worth the verifier confirming the allowlist covers exactly the three current readers
   and nothing broader).
