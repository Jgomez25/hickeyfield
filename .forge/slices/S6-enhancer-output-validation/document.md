# S6-enhancer-output-validation — DOCUMENT

## What shipped
`enhance_or_original` (crates/hickeyfield-core/src/enhancer.rs) — the single funnel every
enhancer backend passes through — now rejects a model reply that is a refusal/apology/meta
comment or too short to be a prompt, via the pure `refusal_reason(reply)`. On such a reply it
restores the user's ORIGINAL prompt (`Rewritten::failed`, so `changed()` is false) with an
honest note, instead of sending the refusal to the paid generator. Anchored to the reply START
so a legitimate rewrite containing "sorry"/"cannot"/"as an ai" mid-sentence is not flagged.

Fixes the real bug: "Dancing singing banana" → llama3.2:1b replied "I can't complete this task."
→ that string was sent to Seedance ($0.23 for garbage). Now it falls back to the original.

## Docs changed
- CHANGELOG.md — `[Unreleased] › Fixed` bullet.
- README: N/A.

## Evidence
- TEST/REVIEW/SECURITY = PASS. `cargo test --workspace` 770 pass / 0 fail; fmt · clippy ·
  provenance clean; pnpm test 224 pass; pnpm build ok.
- AC4 (no false positives): verifier ran refusal_reason against the mandated mid-sentence-"sorry"
  case + 6 more adversarial legit rewrites — none flagged.
- Model-agnostic: the guard is in the sole submit-path funnel used by both Local and Hosted.

## Minor follow-ups (non-blocking, from REVIEW)
- Redundant REFUSALS list entries subsumed by shorter prefixes (dead but harmless).
- Too-short fallback note wording ("declined to rewrite … the reply was too short") reads awkwardly.
