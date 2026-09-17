# S18-live-spend-gate — DOCUMENT

## What shipped
- **Spend gate** (`crates/hickeyfield-core/src/spend_gate.rs`): a process-local flag, OFF by
  construction, consulted as the first statement of every billable path — `ProviderClient::submit`
  (the two provider POSTs), `HostedEnhancer::call`, and the hosted storyboard `ChatBackend` (real host
  only; local Ollama stays free). When not enabled the call returns `JobError::SpendBlocked` before any
  socket is opened. The packaged app's Tauri entrypoint enables it explicitly; every other process
  (tests, examples, agent-run binaries) must set `HICKEYFIELD_ALLOW_SPEND=1` deliberately.
- **first_light.rs** refuses without the flag, prints the estimate first, reads its key from a
  documented env only; the "copy your keychain key" instruction is gone.
- **`scripts/lint-spend.py`** (in the gate list): fails on keychain reads outside vault.rs, raw
  provider-key reads outside vault.rs / first_light.rs / live_fal_pricing.rs, `remove_var("hickeyfield_`
  anywhere outside vault.rs's own guard tests, and `HICKEYFIELD_ALLOW_SPEND` set in test code.
- **Fake-provider seam**: minimal injectable client factory with one seam test; full offline
  coverage of the still/shot submit loops is a follow-up.
- Together with S17's keychain block, spending from this repository is now impossible without an
  explicit per-process opt-in. Residual spenders on this Mac: the installed app with the owner's key,
  and a human running first_light with the flag.

## Docs changed
- `README.md`, `docs/FIRST-LIGHT.md` — the two flags (`HICKEYFIELD_NO_KEYCHAIN`, `HICKEYFIELD_ALLOW_SPEND`)
  and the rule; `CHANGELOG.md` `[Unreleased] › Fixed`, original wording.
- `.forge/backlog.md` — follow-ups: full submit-loop offline tests via the seam; daily budget cap
  (non-goal here); S19 shot trim; S20 per-token pricing accuracy (Seedance 2.5 3.2× under-quote seen on
  the owner's real job).

## Evidence
- TEST / REVIEW / SECURITY verdicts appended in this directory. cargo test --workspace 961 / 0 with
  the ignored sidecar tests unchanged; fmt · clippy · provenance · lint-spend clean; pnpm 312 / build ok;
  keyless audit unchanged. Nothing spent. The final proof that the packaged app is not blocked is the
  owner launching the installed build and seeing a normal estimate.
