# S7-enhancer-prompt-preview — IMPLEMENT claim

Built exactly the design's file-by-file plan: a Rust `preview_prompt` command
(no `State`), a verbatim `final_prompt` submit path, and the opt-in UI preview
panel. Quick Generate is unchanged.

## Files changed

### Rust
- `src-tauri/src/commands.rs`
  - Added `#[serde(default)] pub final_prompt: Option<String>` to `SubmitInput`.
  - Extracted `fn probe_enhancer_env(enhance: bool) -> (Option<String>, bool, Vec<enhancer::LocalModel>)` — the impure key/Ollama probe lifted verbatim out of `submit_job` (was ~:824-840). Called from both `submit_job` (via `compiled_for_submit`) and `preview_prompt`.
  - Extracted `fn compiled_for_submit(model, &SubmitInput) -> Result<crate::harness::Compiled, String>` holding the match: non-empty trimmed `final_prompt` → `Compiled { prompt: text verbatim, original: input.prompt, enhanced: Some(text), version: None, note: Some("Previewed before generating; sent exactly as shown.") }` **without calling `harness::compile`**; else the existing probe + `select_rewriter` + `compile` path. `submit_job` now calls this instead of the inline block.
  - Added `#[derive(Serialize)] #[serde(rename_all="camelCase")] pub struct PreviewDto { prompt, original, enhanced, version, note }`.
  - Added `#[tauri::command] pub fn preview_prompt(input: SubmitInput) -> Result<PreviewDto, String>` — takes **no `State`**; reuses `harness::compile` (camera clause + enhance rules + S6 refusal guard). Structurally cannot resolve a route, read a price, or call `submit_to_provider`.
  - Added 3 `#[cfg(test)]` tests (below).
- `src-tauri/src/lib.rs` — registered `commands::preview_prompt` next to `submit_job` in the invoke handler.

### UI
- `ui/src/types.ts` — added `interface PromptPreview { prompt; original; enhanced?; version?; note? }` and `finalPrompt?: string|null` on `SubmitInput`.
- `ui/src/api.ts` — added `previewPrompt(input): Promise<PromptPreview>` (wrapped `{ input }` like submitJob, maps DTO defensively, `mockPreview` fallback off-desktop). `submitJob` unchanged — already forwards the whole input, so `finalPrompt` rides along.
- `ui/src/mock.ts` — added `mockPreview(input)` alongside `mockSubmit` for browser dev.
- `ui/src/components/PromptPreview.tsx` (NEW) — inline panel: editable `<textarea>` (seeded by caller with `enhanced ?? prompt`), read-only Original block, the `note` line, a Retry button, and a Generate button (reuses `GenerateButton` for cost + blocked/pending styling). `previewing`/`pending` disable + relabel the controls.
- `ui/src/components/SettingsRail.tsx` — added a "Preview prompt" button beside GenerateButton and renders `PromptPreview` when a preview exists; threads the new props; the preview Generate is blocked via `previewText.trim().length < 2`.
- `ui/src/components/rail.css` — styles for `.prompt-preview*` and `.preview-trigger`, following existing `.enhancer-picker`/`.generate` patterns and tokens.
- `ui/src/App.tsx` — state `preview`, `previewText`, `previewing`; `runPreview` (guards, calls `previewPrompt`, seeds `previewText` from `enhanced ?? prompt`); Retry re-calls `runPreview`; generalized `send(finalPrompt?)` (returns success bool); `onGenerateFromPreview = send(previewText)` clearing the panel on success. Quick `onSubmit` stays `send()` with no argument — behavior unchanged.

### Tests added
- `ui/src/api.test.ts` (NEW) — mocks the Tauri bridge; asserts `previewPrompt` wraps `{ input }` and maps the DTO, and that `submitJob` forwards `finalPrompt` verbatim.
- `ui/src/components/PromptPreview.test.tsx` (NEW) — renders enhanced/original/note; textarea edit fires onChange; Retry→onRetry; Generate→onGenerate; empty edit blocks Generate.

## Acceptance criteria → evidence

Env for all Rust runs:
`source "$HOME/.cargo/env"` + dummy `hickeyfield_*_KEY=x` overrides.

### AC1 — `preview_prompt` command, no State, reuses compile, creates no job
Test `preview_returns_a_compiled_prompt_and_creates_no_job` (enhance=false, deterministic, no network): asserts the wire prompt contains the raw text and is longer than it (camera clause appended), original preserved, `enhanced` None, `version` None. The command's signature takes no `State` — structural proof it can't create a job.

### AC2 — `final_prompt` used verbatim, never re-compiled/re-enhanced
Test `a_final_prompt_is_used_verbatim_and_never_re_enhanced` via `compiled_for_submit`: `final_prompt=Some("edited text")` + `enhance=true` + a bogus `RewriterChoice{backend:"ollama", model:"definitely-not-installed"}`. Asserts `compiled.prompt == "edited text"` byte-for-byte (no camera clause re-appended), `version` None, note == "Previewed before generating; sent exactly as shown." — proving `harness::compile`/rewrite was bypassed.

### AC4 — blank final_prompt falls back; edge cases
Test `a_blank_final_prompt_falls_back_to_the_normal_compile_path`: whitespace-only `final_prompt` is filtered (`str::trim` + `filter(!empty)`) and falls through to the normal compile path (raw prompt + camera clause), NOT the verbatim note — so an empty edit can never submit empty. UI also blocks the preview Generate at `previewText.trim().length < 2`. Enhance-off / no-backend previews inherit `harness::compile`'s honest note and stay editable (see `mockPreview` and the note passthrough).

### AC3 — UI panel + Preview button
`PromptPreview.test.tsx` (5 tests) and `api.test.ts` (3 tests) pass; component renders enhanced+original+note, edit→onChange, Retry→onRetry, Generate→onGenerate, empty→blocked.

### AC5 — regression gate

```
$ cargo test --workspace
core:  test result: ok. 653 passed; 0 failed; 9 ignored
shell: test result: ok. 120 passed; 0 failed; 1 ignored
(main/doctests: 0/0/0)
```
773 passed total (baseline 770 + 3 new S7 Rust tests), 0 failed.

```
$ cargo fmt --all --check      → FMT_OK (clean)
$ cargo clippy --workspace --all-targets -- -D warnings → Finished, 0 warnings
$ ./scripts/lint-provenance.py → PASS (no third-party hosts; no corpus match); EXIT=0
$ cd ui && pnpm test           → Test Files 14 passed (14); Tests 232 passed (232)
$ cd ui && pnpm build          → tsc --noEmit clean; vite built in 839ms
```
(Did NOT run `tauri build` — release phase.)

## Deviations / known limitations
- **No silent deviation from the final_prompt invariant.** The verbatim branch in `compiled_for_submit` does not call `harness::compile`; the AC2 test pins `prompt == "edited text"` exactly (no double camera clause) and `version == None` (no re-charged/re-recorded rewrite).
- **Minor duplication (matches design):** the `select_rewriter`/`Rewriter::None` selection appears in both `preview_prompt` and the `None` branch of `compiled_for_submit`. Design Decision 1's pseudocode has the same shape (both call `select_rewriter`); `probe_enhancer_env` is the shared impure part. Not factored further because borrow lifetimes require each caller to own the key/models locals across the `compile` call.
- `preview_prompt` intentionally ignores `route_id` (it never routes/prices), so a preview succeeds even before a route is resolvable, as long as the model id is known.
- App-level Preview→edit→Generate is exercised through the unit seams (`api.test.ts` proves `submitJob` forwards `finalPrompt`; `PromptPreview.test.tsx` proves the panel wiring) rather than a full App render test, matching the existing no-App-integration-test convention in this repo.
