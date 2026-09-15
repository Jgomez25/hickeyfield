# S4-prompt-enhancer-wired — DOCUMENT

## What shipped
The prompt enhancer is wired end-to-end. Previously `Enhance` defaulted on but the UI
never sent a rewriter, so prompts went to the model verbatim (a user's "a banana with a
smile" produced a literal banana). Now:
- `Rewriter::Hosted`/`Unavailable` variants + a shared `finish_rewrite` path (harness.rs).
- `select_rewriter` (commands.rs) picks a backend: explicit UI choice → auto (runnable Ollama,
  else OpenAI key) → honest note. Never silently raw.
- `list_ollama_models` command + `EnhancerPicker.tsx`: choose backend + Ollama model; the
  picker commits its resolved default on mount so what you see is what's submitted.
- Hosted = OpenAI (existing keychain slot). Anthropic DEFERRED (no ProviderId/vault slot yet;
  `HostedBackend::Anthropic` stays extensible — a future one-arm change).

## Docs changed
- `CHANGELOG.md` — new `[Unreleased] › Added` bullet (user-facing, original wording).
- README: N/A (no README claim about the enhancer to correct).

## Evidence
- TEST live: gemma3:1b rewrote "a banana with a smile" → a 222-char cinematic prompt,
  version pin `enhancer.v1/image+ollama/gemma3:1b`. TEST/REVIEW/SECURITY = PASS.
- Gate: cargo test --workspace 754 passed / 0 failed; fmt · clippy · provenance clean;
  pnpm test 222 passed; pnpm build ok.
