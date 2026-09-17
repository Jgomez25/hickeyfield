# S18-live-spend-gate — SECURITY (evidence)

Read-only pass over `git diff main...HEAD` (HEAD 07da1b8), money-safety as the
sole lens, in the shadow of the 2026-09-16 real-spend incident. No source, docs,
or config changed. All commands ran with `HICKEYFIELD_NO_KEYCHAIN=1` + dummy
`hickeyfield_*_KEY=x`; `HICKEYFIELD_ALLOW_SPEND` was never set; no hosted calls.

## 1. Every billable path crosses `spend_gate::allowed()` before network I/O

Enumerated every submit / hosted-LLM / upload / poll / cancel site in
`crates/hickeyfield-core/src` and `src-tauri/src` (grep of `.post(`/`.put(`/
`reqwest`/`Client::new`/`Client::builder`/`fn submit(`). The two production
`ProviderClient::submit` impls are the only billable-generation entrypoints and
both gate **before** `http()` builds a client, so no socket opens on refusal:

- `crates/hickeyfield-core/src/clients.rs:205-207` — `FalClient::submit` returns
  `spend_blocked(model_id)` first (POST `queue.fal.run` at `:210`).
- `crates/hickeyfield-core/src/clients.rs:368-370` — `HiggsfieldClient::submit`
  returns `spend_blocked(model_id)` first (POST `api.higgsfield.ai` at `:374`).

Hosted-LLM token spend (the other paid egress) is gated at the call sites, not
construction, which is the correct chokepoint — `harness.rs:238` builds a
`HostedEnhancer` but the paid request only leaves through the gated call:

- `crates/hickeyfield-core/src/enhancer.rs:1368-1370` — `HostedEnhancer::call`
  refuses when `base_url.is_none() && !allowed()`.
- `crates/hickeyfield-core/src/enhancer.rs:1665-1667` — `ChatBackend::Hosted`
  (storyboard planner) refuses on the same condition.
- Ollama (`enhancer.rs:1072,1650`, local, free) is deliberately ungated so the
  offline enhancer keeps working — correct.

Non-billable network I/O examined and correctly left ungated (money lens):
- **Uploads** — `clients.rs:601` (fal `storage/upload/initiate` + signed PUT,
  doc "public and free") and `:647` (Higgsfield `files/generate-upload-url` +
  temporary-retention PUT). Neither bills; both run before `submit`, so with the
  gate off a file can still be uploaded (egress, not spend) and the run then
  dies at `SpendBlocked`. No money path.
- **poll** (`clients.rs` fal `:250`, Higgsfield `:410`) and **cancel**
  (`:260`, `:420`) — status GET / cancel, not billed, and reachable only for a
  job a real `submit` already created, which the gate now prevents outside the
  app.
- **extend-video** — `grok_extend_video` is an ordinary registry model; its
  generation goes through `submit`, so it is gated like any other. Confirmed no
  separate extend endpoint bypasses the trait.
- **Price feeds** (`prices.rs`), **fal schema/catalogue probes**
  (`fal_schema.rs:506`, `fal_catalogue.rs:797`), **local detect**
  (`clients.rs:713`), **key validate** (`validate.rs:82`), **downloads**
  (`library.rs`, `runner.rs:223`) — all free/read-only; confirmed non-billable.

Test-only `ProviderClient` impls (`runner.rs:813` `ScriptedClient`,
`commands.rs:3001` `RecordingClient`) open no socket. **No ungated billable path
found.**

## 2. The opt-in is exactly `"1"`, set only by the packaged entrypoint

- `spend_gate.rs:47` `decide` — `matches!(env_value.map(str::trim), Some("1"))`;
  `"true"`/`"yes"`/`"0"`/`""` all reject (proven `spend_gate.rs:71-77`). Stricter
  than the vault's truthy parse, deliberately.
- `enable()` is called from exactly one place: `src-tauri/src/lib.rs:23`, before
  `AppState::init` builds the client factory (static source-order test
  `lib.rs:141-155`). `scripts/lint-spend.py` `RE_ENABLE` bans it everywhere else.
- Repo-wide grep for `ALLOW_SPEND` (incl. `.claude/`, `.forge/`, `scripts/`,
  `docs/`; no `.env`, CI file, or `launch.json` present): every hit is either
  documentation of the flag or the `decide`/message string. **Nothing sets the
  env var or calls `enable()` outside the entrypoint.** Tests and examples
  inherit neither, so their default is OFF by construction.

## 3. `first_light.rs`: refuses before reading a key; no keychain-copy left

`examples/first_light.rs:39-53` — the flag check `!spend_gate::allowed()` runs
first, prints the offline estimate, and `exit(2)` **before** `std::env::var(
"FAL_KEY")` at `:55`. Even if the refusal were bypassed, `FalClient::submit` is
independently gated (§1) — defence in depth, correct. The old
`security find-generic-password … | FAL_KEY=` recipe (the S17 MEDIUM) is gone
from both `examples/first_light.rs:19` and `docs/FIRST-LIGHT.md:8`, replaced by
"never copy it out of the keychain into your shell history." Grep confirms no
`find-generic-password` / keychain-copy instruction remains in `docs/` or the
example. **The S17 MEDIUM is closed.** Key source is the documented `FAL_KEY`
env only.

## 4. `scripts/lint-spend.py` catches the incident patterns

Ran the linter clean (`ok (49 files scanned)`, exit 0) and drove all four rules
on a scratch file — each fired (exit 1):
- Rule 3 `remove_var("hickeyfield_…"` / `FAL_KEY` — the exact env-stripping that
  caused the incident — caught outside `vault.rs`.
- Rule 1 `keyring::`/`get_password` caught outside `vault.rs`.
- Rule 2 raw `var("FAL_KEY"|"hickeyfield_*_KEY")` caught outside the 3-file
  allowlist.
- Rule 4 `set_var("HICKEYFIELD_ALLOW_SPEND"` and `spend_gate::enable(` caught
  outside `lib.rs`.

Allowlist entries genuinely need keys: `vault.rs` (the single vault +
keychain-guard regression tests), `examples/first_light.rs` (documented FAL_KEY
dev seam, now double-gated), `tests/live_fal_pricing.rs` (`#[ignore]`, read-only
`GET models/pricing`, never logs the key, not run by default `cargo test`).

## 5. No key material in `SpendBlocked` / logs

`clients.rs:22-29` and `enhancer.rs:1633-1642` build fixed strings interpolating
only `model_id` (a route slug). `JobError::Display` (`engine.rs:264`) writes the
message verbatim — no credential. The one new log line
(`lib.rs:32` "spending enabled for this process (packaged app)") carries no key.

## 6. `cargo deny` unchanged

`Cargo.lock`, `deny.toml`, and every `Cargo.toml` are unchanged in the slice
diff — no new dependency or supply-chain surface.

## Findings (ranked) — no Critical/High

### LOW-1 — the linter scans only tracked `.rs`; a non-Rust file exporting the flag would be missed
`scripts/lint-spend.py:70-78` (`tracked_rs()` globs `*.rs`) and
`RE_SET_ALLOW_SPEND` matches only Rust `set_var`. Path: someone adds
`export HICKEYFIELD_ALLOW_SPEND=1` to a future CI step, Makefile, shell script,
`.env`, or `launch.json` that also runs `cargo test`/an example with a reachable
real key → that process spends, and the linter never sees it. Not currently
exploitable: a repo-wide grep found no non-Rust file that sets the var. Fix
(own backlog note, LOW): have `lint-spend.py` also scan tracked non-Rust files
for a `HICKEYFIELD_ALLOW_SPEND` assignment, or add a CI assertion that the var
is unset in the test environment.

### LOW-2 — the entrypoint proof test flips the real process-global for the whole `src-tauri` test binary
`src-tauri/src/lib.rs:145-153` (`the_entrypoint_enable_path_actually_allows_spending`)
calls the real `spend_gate::enable()`, so `allowed()` is true for every test in
the `src-tauri` test binary (one process, shared `OnceLock`, tests run
concurrently). Contained today: no `src-tauri` test constructs a real
`FalClient`/`HiggsfieldClient` (grep: `FalClient::new` in `src-tauri` appears
only in production `app.rs:49,79,91`), and keychain is blocked under `cfg(test)`
with dummy keys, so even a real submit would carry a dummy key. Residual: if a
future `src-tauri` test builds a real provider client and reaches `submit`, this
test's global-enable would let it through the gate — reopening an incident-class
hole from a different direction. The core-crate block tests (`clients.rs:1027`,
`enhancer.rs:2820`) correctly prove the OFF state without touching the global.
Fix (LOW): prove the entrypoint via `decide(...)` / source-string rather than
flipping the real `OnceLock`, or isolate this assertion in its own test binary.

### LOW-3 — hosted-enhancer gate keys off `base_url.is_none()`, a test-seam convention rather than the flag
`enhancer.rs:1368,1665` skip the gate whenever `base_url.is_some()`.
`with_base_url` is a documented test-only stub seam, never set in production
(hosted enhancers are built via `HostedEnhancer::new`/`openai` with `base_url =
None`), so today this only exempts local stubs. Residual: a test that points
`with_base_url` at a real hosted API with a real key would spend without the
flag. Contained by dummy keys + local stubs in the current suite. Note only.

## Residual: who can still spend (stated plainly)

Exactly two, both intended:
1. **The installed packaged app** — `lib.rs:23` calls `enable()` at startup and
   the vault serves the owner's real keychain key. This is the product.
2. **A human deliberately running `first_light`** with both
   `HICKEYFIELD_ALLOW_SPEND=1` and `FAL_KEY=…` set by hand — an explicit,
   acknowledged opt-in. (`live_fal_pricing.rs --ignored` also needs a key but is
   read-only and bills nothing.)

Confirmed nothing else: `cargo test` leaves the gate OFF (no `enable()` call, no
env) and any real submit it reached would return `SpendBlocked`; every example
but `first_light` uses only offline/local paths; no agent-run binary or script
sets the flag. There is no path by which `cargo test`, an example, or an
agent-run binary spends without the flag.

VERDICT: PASS
