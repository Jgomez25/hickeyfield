# S4-prompt-enhancer-wired — SECURITY (evidence)

Read-only review of what THIS slice touched (uncommitted diff on
`forge/s3-prompt-enhancer`): `crates/hickeyfield-core/src/enhancer.rs`,
`src-tauri/src/harness.rs`, `src-tauri/src/commands.rs`, `src-tauri/src/lib.rs`,
and `ui/*`. Scope: the new `Rewriter::Hosted` path, `select_rewriter`, the vault
key read, `list_ollama_models`, and the UI picker.

## Primary boundary — OpenAI API key handling (CLEARED)

Traced the key end-to-end. It provably stays server-side (Rust) and never
reaches a persisted field, the webview, a log line, or the version pin.

- **Source → sink.** `submit_job` reads it with `vault::get(ProviderId::OpenAi,
  false)` into a local `openai_key` (`commands.rs:820`), borrows it into
  `Rewriter::Hosted { api_key }` via `select_rewriter` (`commands.rs:751-753`),
  which `compile` hands to `HostedEnhancer::new` (`harness.rs:246`). The only
  use of `self.api_key` is the request header: `Authorization: Bearer …`
  (`enhancer.rs:1147`) / `x-api-key` (`enhancer.rs:1160`). Never placed in a
  URL or query string.
- **Persisted `JobSet` fields carry no key.** `submit_job` copies only
  `compiled.original` → `prompt`, `compiled.enhanced` → `enhanced_prompt` (the
  LLM reply text), `compiled.version` → `enhancer_version`, `compiled.note` →
  `enhance_note` (`commands.rs:878-881`). None of these ever receive the key:
  - `enhanced` = `out.prompt`, the model's reply (`harness.rs` `finish_rewrite`).
  - `version` = `version_string(backend.slug(), model, system_prompt)`
    (`enhancer.rs:802,1305-1307`) — backend slug + model id + corpus prompt, no key.
  - `note` = honest fallback strings or `enhance_or_original`'s
    `"Enhancement failed ({e}) …"` (`enhancer.rs:534-538`), where `e` derives
    from `send_json` (`request failed: {reqwest err}` / `HTTP {status}: {body}`,
    `enhancer.rs:826-835`). The key is a header, not part of the reqwest error
    (which carries at most the constant host URL) or of the response body.
- **No logging of the key.** No `tracing!`/`log!`/`eprintln!`/`dbg!` on the
  hosted or selection paths. The only `println!` in the touched code are
  test-only (`enhancer.rs:2190`, `harness.rs:495` inside the `#[ignore]` live
  test).
- **Never crosses to the webview.** The UI's `RewriterChoice` wire shape is
  `{ backend, model }` only (`types.ts`, `commands.rs:702-712`) — no key field.
  `openaiAvailable` in `App.tsx` is derived from `states…hasKey` (a boolean),
  never the value. `EnhancerPicker.tsx` references "key" only in prose and the
  React `key={m}` prop — it handles no secret.

Conclusion: the key stays in Rust and out of every persisted/UI-visible field.

## Other boundaries examined

- **SSRF / `list_ollama_models` (CLEARED).** `commands.rs:797` calls
  `local_models(hickeyfield_core::clients::OLLAMA_URL)`; `OLLAMA_URL` is the
  hardcoded const `http://127.0.0.1:11434` (`clients.rs:673`). Not
  attacker-influenced, not settable from the UI. `local_models`
  (`enhancer.rs:1020`) only appends the fixed `/api/tags` path. No SSRF, no
  user-controlled egress.
- **Hosted base-URL override / TLS redirect (CLEARED).** `with_base_url`
  (`enhancer.rs:1100`) exists as a test seam only. Every non-test caller
  (`harness.rs:246`) uses `HostedEnhancer::new`, which leaves `base_url = None`,
  so requests go to the constant real hosts (`OPENAI_CHAT_URL` /
  `ANTHROPIC_MESSAGES_URL`). The UI's `RewriterChoice` cannot set a base URL.
  reqwest's default TLS verification is used; nothing disables it.
- **Prompt injection (LOW, no finding).** The raw prompt is sent to the LLM and
  the reply becomes the *generation* text prompt — it is never executed as code
  or a shell/SQL string, so there is no injection sink. The documented
  `<think>`-leak (a reasoning model's chain-of-thought bleeding into the prompt,
  `clean_reply` not stripping it) is cosmetic, not a security defect.
- **DoS / hang (LOW, no finding).** Every HTTP client is built with an explicit
  timeout: hosted 60s, local chat 120s, model-list 3s (`enhancer.rs:121-129`,
  built at `enhancer.rs:813-819`). `submit_job` runs `compile` synchronously, so
  a slow enhancer delays that one submission by at most the bounded timeout — the
  same bounded pattern the pre-existing Ollama path already had. No unbounded
  wedge.
- **Dependency / supply-chain (CLEARED).** No `Cargo.toml`/`Cargo.lock`/
  `package.json`/`pnpm-lock` change in the slice. Baseline h2/rustls advisories
  are unchanged and remain a separate slice's concern.
- **Anthropic not wired (CONFIRMED).** No `anthropic`/`Anthropic` reference in
  `commands.rs`, `harness.rs`, or `vault.rs`; `select_rewriter` handles only
  `"openai"`/`"ollama"`. `ProviderId` still has no Anthropic variant
  (`provider.rs`) — no new key slot was added.

## Informational (not a finding, no action required)

- On an incorrect/expired key, OpenAI's 401 body is passed through by
  `send_json` (`HTTP {status}: {body}`, `enhancer.rs:833`) into `enhance_note`,
  which persists and reaches the UI. OpenAI's own error text echoes a
  *provider-redacted* partial key form (`sk-…****…`), never the full secret, and
  only ever the user's own key on their own machine — not attacker-reachable and
  not a full-secret leak. Noted only for completeness; no fix needed for this
  slice.

## Findings ranked by severity

None. No Critical, High, Medium, or Low findings with an exploitation path. No
new backlog slice is warranted.

VERDICT: PASS
