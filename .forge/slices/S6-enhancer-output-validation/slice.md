# S6-enhancer-output-validation — SLICE (DISCOVER + PLAN)

## Delivers (one sentence, end-to-end)
When the enhancer model replies with a refusal / apology / meta-comment instead of a rewritten scene, the app detects it and submits the user's ORIGINAL prompt with an honest note, instead of sending the refusal text to a paid generator.

## Context (the bug, from a real job record)
Prompt "Dancing singing banana" → `llama3.2:1b` replied **"I can't complete this task."** → the app stored that as `enhanced_prompt` (status Rewritten, no note) and sent it to the paid Seedance model ($0.23 for garbage). Root cause: `enhance_or_original` (enhancer.rs:516) — the single funnel every backend passes through — validates only for an EMPTY reply (:524), not for a reply that is a refusal/non-rewrite. `RewriteStatus::Refused` exists but is only used for "we declined to ask" (precheck: empty prompt), never for a bad model REPLY.

## Acceptance criteria (testable)
- [ ] AC1 — A pure `refusal_reason(reply: &str) -> Option<&'static str>` (or equivalent) in `enhancer.rs` flags a reply that is not a plausible rewrite: refusal/apology/meta patterns at/near the start — "i can't", "i cannot", "i can not", "i'm unable"/"i am unable", "i'm sorry"/"i am sorry"/"sorry,", "i apologize", "as an ai"/"as a language model", "i won't"/"i will not", "i'm not able", "unable to", "i cannot fulfill", "i can't help", "cannot assist" — case-insensitive. Also structural: reply trimmed shorter than a small floor, or reply equals the original (echo). Rust tests: "I can't complete this task." → flagged; a real cinematic rewrite ("A ripe banana dances under warm stage light…") → NOT flagged (no false positive); echo/too-short → flagged.
- [ ] AC2 — `enhance_or_original` applies AC1 after the empty check: when a "changed" reply is a refusal/non-rewrite, return the ORIGINAL prompt via the honest-fallback path with a clear note (e.g. "The model declined to rewrite this prompt, so your original was used."). Status is a non-changed status (Failed or Refused) so `changed()` is false and the original is what gets submitted. Rust test drives a fake `Enhancer` returning "I can't complete this task." and asserts the funnel returns the original + note, NOT the refusal.
- [ ] AC3 — Model-agnostic: the guard lives in the ONE funnel (`enhance_or_original`), so it protects Local (Ollama) AND Hosted (OpenAI) backends identically. A test confirms both the local and hosted paths route through it (or that the funnel is the sole submit-path entry, per its doc).
- [ ] AC4 — No false positives on legitimate rewrites: a small corpus of real cinematic rewrite samples all pass (not flagged). The guard must not trip on prompts that merely CONTAIN a word like "sorry" mid-sentence — anchor to reply start / whole-reply shape.
- [ ] AC5 (regression) — `cargo test --workspace` (0 failed, >=761), fmt, clippy, provenance, `cd ui && pnpm test && pnpm build`. App builds+launches. (No UI change strictly required; the honest note already surfaces via `enhanceNote`.)

## Approach + why now
User hit this live and lost a paid generation to a refusal. The fix is small and lands in the single enforcement funnel `enhance_or_original`, so every current and future backend inherits it. Feeds S7 (prompt preview) — the preview will show the honest fallback instead of a refusal.
