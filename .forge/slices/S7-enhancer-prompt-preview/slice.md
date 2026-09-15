# S7-enhancer-prompt-preview — SLICE (DISCOVER + PLAN)

## Delivers (one sentence, end-to-end)
Before spending money, the user can Preview the enhancer's rewritten prompt, edit it, or Retry for a different rewrite, then Generate with exactly the shown text — the quick one-click Generate stays unchanged.

## Acceptance criteria (testable)
- [ ] AC1 — New `preview_prompt(input: SubmitInput) -> PreviewDto{prompt,original,enhanced,version,note}` command (commands.rs), registered in lib.rs. It reuses `harness::compile` (enhancer + camera clause + the S6 refusal guard) and takes NO `State` — it never resolves a route, reads prices, or calls `submit_to_provider`, so it cannot bill a generation. Rust test: returns the compiled prompt (with camera clause) for `enhance=false`, `enhanced=None`; the command taking no State is the structural proof it creates no job.
- [ ] AC2 — `final_prompt: Option<String>` on `SubmitInput`. When present & non-empty, `submit_job` uses it VERBATIM as the wire prompt and does NOT re-run `compile` (no double camera clause, no re-charged/re-changed rewrite). Stores: `prompt`=raw `input.prompt`, `enhanced_prompt`=final text, `enhancer_version`=None (a human edit has no recipe), `enhance_note`="Previewed before generating; sent exactly as shown." Rust test pins: `final_prompt=Some("edited")` + `enhance=true` + a bogus rewriter → compiled.prompt == "edited" verbatim, version None (proves compile/rewrite bypassed).
- [ ] AC3 — UI: a "Preview prompt" button beside Generate → `PromptPreview.tsx` panel showing the editable enhanced prompt (seeded `enhanced ?? prompt`), the original for reference, the note (incl. S6 "declined to rewrite" / enhance-off reasons), a Retry (re-call preview) and a Generate-from-preview (submits the shown text as `finalPrompt`). Quick Generate unchanged (enhance-and-submit as today). vitest: panel renders enhanced+original+note; edit fires onChange; Retry re-calls; Generate submits the edited text.
- [ ] AC4 — Edge cases: enhance OFF / no backend → preview shows original+camera-clause+honest note, still editable/generatable; blank edit → blocked in UI (blockedReason on previewText) AND Rust `.filter(!empty)` so an empty final_prompt falls through to the normal path (never submits empty); slow/timeout inherits `enhance_or_original`'s fallback (preview always resolves to something generatable).
- [ ] AC5 (regression) — `cargo test --workspace` (0 failed, >=770), fmt, clippy, provenance, `cd ui && pnpm test && pnpm build`. App builds+launches. Quick-Generate path unchanged.

## Approach + why now
User asked to see/edit/retry the enhanced prompt before paying — and it directly complements S6 (the preview shows the honest fallback instead of a refusal). See design.md for the file-by-file plan.
