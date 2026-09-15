# S6-enhancer-output-validation — RELEASE

**Gate:** gate.py exit 0. TEST + REVIEW + SECURITY = PASS.
**What shipped:** `enhance_or_original` rejects a model refusal/apology/non-prompt (pure
`refusal_reason`, anchored to reply start) and submits the user's ORIGINAL prompt with an
honest note instead of sending the refusal to a paid generator. Fixes the "Dancing singing
banana" → "I can't complete this task." → $0.23 garbage bug. Model-agnostic (single funnel).
**Regression:** cargo test --workspace 770 pass / 0 fail; fmt · clippy · provenance clean;
pnpm test 224 pass; pnpm build ok; tauri build + launch at install.
**Minor follow-ups (logged):** redundant REFUSALS entries; too-short note wording.
**Landing:** commit on forge/s6-enhancer-validation, fast-forward to main (local, no push),
rebuilt + reinstalled to /Applications.
**Status:** released (local main).
