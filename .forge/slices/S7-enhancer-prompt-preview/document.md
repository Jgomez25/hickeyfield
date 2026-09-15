# S7-enhancer-prompt-preview — DOCUMENT

## What shipped
A "Preview prompt" path lets the user see/edit/retry the enhancer's rewrite before spending.
- `preview_prompt(input) -> PreviewDto` (commands.rs) reuses `harness::compile` (enhancer +
  camera clause + S6 guard); takes no State, so it structurally cannot resolve a route, read
  prices, or call submit_to_provider — a true dry run (live-verified: job count unchanged).
- `final_prompt: Option<String>` on SubmitInput: when set, `compiled_for_submit` sends it
  VERBATIM and never re-runs compile (no double camera clause, no re-charged rewrite). Stored:
  prompt=raw, enhanced_prompt=final, enhancer_version=None, note="Previewed before generating…".
- UI: `PromptPreview.tsx` panel (editable enhanced prompt + original + note + Retry + Generate);
  quick one-click Generate unchanged.

## Docs changed
- CHANGELOG.md — `[Unreleased] › Added` bullet.
- README: N/A.

## Evidence
- TEST/REVIEW/SECURITY = PASS. cargo test --workspace 773 pass / 0 fail; fmt · clippy ·
  provenance clean; pnpm test 232 pass; pnpm build ok.
- Live: preview_prompt path (gemma3:1b) rewrote a prompt with the DB job_sets count unchanged
  (3→3) — no generation billed. final_prompt invariant pinned by test.
- Security: OpenAI key stays server-side (never in PreviewDto/note/logs); preview can't bill.

## Follow-ups logged (non-blocking)
- F8: invalidate the preview when the user edits the raw prompt/model/settings after previewing
  (stale-preview provenance — stored `original` can diverge from what the panel showed). Cosmetic.
- Minor: preview_prompt vs compiled_for_submit share probe+select_rewriter (borrow-lifetime dup);
  optional shared helper.
