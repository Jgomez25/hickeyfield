# S4-prompt-enhancer-wired — SLICE (DISCOVER + PLAN)

## Delivers (one sentence, end-to-end)
The prompt enhancer actually rewrites a prompt through the filmmaking corpus using an available LLM backend — auto-detected local Ollama (the user picks the model) or a hosted OpenAI/Anthropic key — and when none is configured says so honestly instead of silently sending the raw prompt.

## Context (verified this session)
- Root cause: UI never sends a `rewriter`; `submit_job` therefore hits `Rewriter::None` and returns the prompt "sent as written" ([harness.rs:109](../../../src-tauri/src/harness.rs), [commands.rs:723](../../../src-tauri/src/commands.rs)). The hosted OpenAI/Anthropic enhancer is fully built in `crates/hickeyfield-core/src/enhancer.rs` (`HostedEnhancer`/`HostedBackend`) but is NOT a `Rewriter` variant, so the shell cannot reach it.
- Ollama is live on this machine (v0.34.0, 6 models); `detect_local()` (clients.rs:691) already probes `:11434/api/tags` but returns only a boolean — no model list is exposed to the UI.
- WRINKLE for DESIGN: `ProviderId` has `OpenAi` (keychain slot exists) but NO `Anthropic` variant — the vault has no Anthropic key slot. Hosted-Anthropic wiring needs a new key slot; hosted-OpenAI can use the existing slot. DESIGN decides how to handle Anthropic (add a slot, or defer Anthropic and ship OpenAI + Ollama first).

## Acceptance criteria (testable)
- [ ] AC1 — `Rewriter` (harness.rs) + `compile()` gain hosted backend(s) alongside Ollama, reaching the existing `HostedEnhancer`. A Rust test proves `compile()` with a hosted rewriter produces an enhanced prompt (HostedEnhancer HTTP mocked).
- [ ] AC2 — Backend auto-selection + HONEST fallback: `submit_job` picks a backend from what's actually available (OpenAI key in keychain, or detected Ollama). When `enhance` is on but NO backend is available, the job's `enhanceNote`/note says so (never silently raw). Rust test on selection + note.
- [ ] AC3 — A command lists installed Ollama models (from `/api/tags`) for the UI, and the UI exposes an enhancer control: enable/disable + choose backend + (for Ollama) pick a model; when unavailable it shows the honest reason, not a dead toggle. `vitest` test on the picker.
- [ ] AC4 — The submit path sends the chosen rewriter, and enhancement is observable: with a backend available, the resulting job carries an `enhancedPrompt` distinct from the raw prompt and an `enhancerVersion`. Unit-verified with a mock; ALSO a LIVE end-to-end check against the user's running Ollama ("a banana with a smile" → expanded cinematic prompt) recorded in the TEST evidence.
- [ ] AC5 (regression) — Full gate green: `cargo test --workspace` (0 failed, >=740), `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `./scripts/lint-provenance.py`, `pnpm build`, `pnpm test`. App builds + launches.

## Approach + why now
A user just hit the dead enhancer: "a banana with a smile" produced a literal banana because the prompt went to the model verbatim. Highest visible-value fix. The core rewrite engine already exists; this slice connects it (shell `Rewriter` + backend selection), exposes it (list-models command + UI control), and makes the "no backend" case honest. Backends: local Ollama (auto-detect, model-picker) + hosted OpenAI (existing key slot); Anthropic per DESIGN's call.
