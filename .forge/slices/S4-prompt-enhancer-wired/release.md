# S4-prompt-enhancer-wired — RELEASE

**Gate:** `python3 .forge/gate.py` → exit 0. TEST + REVIEW + SECURITY = PASS.

**What shipped:** the prompt enhancer now actually rewrites prompts. `Rewriter::Hosted`/`Unavailable`
+ shared `finish_rewrite` (harness.rs); `select_rewriter` backend selection with honest fallback
(commands.rs); `list_ollama_models` command + `EnhancerPicker.tsx` (pick backend + Ollama model, commits
its default on mount so UI == submit). Hosted = OpenAI (existing key slot); Anthropic deferred (no
ProviderId/vault slot — its own future slice).

**Review-driven repair (before release):** REVIEW first FAILed on a MAJOR — the picker showed a
selection it never committed, so a key+Ollama config silently sent `rewriter: null`. Fixed two-front
(picker commits default on mount + auto-branch prefers runnable Ollama). Re-REVIEW + re-TEST PASS.

**Regression:** `cargo test --workspace` 754 passed / 0 failed / 10 ignored; fmt · clippy · provenance
clean; `pnpm test` 222 passed; `pnpm build` ok; tauri build + launch recorded at install.
`cargo deny check` — baseline h2/rustls advisories only (no manifest change) → separate slice.

**LIVE proof (user's Ollama, gemma3:1b):** "a banana with a smile" → a 222-char cinematic prompt,
version pin `enhancer.v1/image+ollama/gemma3:1b`, distinct from raw, no `<think>` leak.

**Security:** OpenAI key stays server-side (Authorization header only) — never in enhancedPrompt /
enhancerVersion / enhanceNote / logs / webview. `list_ollama_models` is localhost-only (no SSRF).

**Landing:** commit on branch `forge/s3-prompt-enhancer`, then fast-forward merged to `main` (local
only, no push, per owner), rebuilt + reinstalled to /Applications.

**Status:** released (local main).
