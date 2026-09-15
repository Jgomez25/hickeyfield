# S7-enhancer-prompt-preview — DESIGN

Condensed from the approved Plan-agent design (all file:line verified). One vertical slice: Rust preview command + submit final-prompt path + UI preview panel.

## Verified seams
- `submit_job` (commands.rs:808) selects a rewriter (:823-850) then calls `harness::compile` (:856-863); paid generation is `submit_to_provider` (app.rs:162) called ONLY at commands.rs:909.
- `harness::compile` (harness.rs:116-252) = preset camera clause + enhance decision + `enhance_or_original` (incl. S6 refusal guard via `finish_rewrite` :76-113). NO fal reference. Only external I/O is the enhancer HTTP call (free Ollama / small OpenAI). `Compiled.prompt` is ALWAYS the complete wire prompt incl. camera clause (rewrite re-runs `PromptParts::compile()` at :90-94).
- DTO chain wired (S4): JobSet.enhanced_prompt/enhancer_version/enhance_note set at commands.rs:889-891 → api.ts toJobSet → types.ts → MetaRail. Command registration: lib.rs:64-89.

## Decision 1 — `preview_prompt` command (commands.rs, registered lib.rs:81)
Factor the impure probe out of submit_job (both commands need it; borrow lifetimes require owned locals):
`fn probe_enhancer_env(enhance: bool) -> (Option<String> /*openai key*/, bool /*ollama up*/, Vec<enhancer::LocalModel>)` — exactly commands.rs:824-840 lifted.
Command (NO `State` — structural guarantee it can't reach fal/store/prices):
```rust
#[derive(Serialize)] #[serde(rename_all="camelCase")]
pub struct PreviewDto { pub prompt: String, pub original: String,
  pub enhanced: Option<String>, pub version: Option<String>, pub note: Option<String> }
#[tauri::command]
pub fn preview_prompt(input: SubmitInput) -> Result<PreviewDto, String> {
  let reg = registry(); let model = reg.get(&input.model_id).ok_or(...)?;
  let enhance = input.settings.enhance;
  let (key, up, models) = probe_enhancer_env(enhance);
  let rewriter = if enhance { select_rewriter(input.rewriter.as_ref(), key.as_deref(), up, &models) }
                 else { Rewriter::None };
  let c = harness::compile(model, &input.prompt, input.preset_id.as_deref(), &input.media, enhance, rewriter)?;
  Ok(PreviewDto{ prompt:c.prompt, original:c.original, enhanced:c.enhanced, version:c.version, note:c.note })
}
```
Reuses SubmitInput verbatim (ignores route_id); no logic duplication.

## Decision 2 — submit_job accepts `final_prompt: Option<String>` (THE riskiest invariant)
Add `#[serde(default)] pub final_prompt: Option<String>` to SubmitInput (commands.rs:682-700). Replace the rewriter-select+compile block (:823-863) with:
```rust
let compiled = match input.final_prompt.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
  Some(text) => harness::Compiled {   // previewed/edited: send VERBATIM, do NOT re-compile
     prompt: text.to_string(),        // already includes the camera clause
     original: input.prompt.clone(),
     enhanced: Some(text.to_string()),
     version: None,                   // a human edit has no reproducible recipe
     note: Some("Previewed before generating; sent exactly as shown.".into()),
  },
  None => { let (key,up,models)=probe_enhancer_env(enhance);
     let rewriter = /* existing select_rewriter / Rewriter::None */;
     harness::compile(model,&input.prompt,input.preset_id.as_deref(),&input.media,enhance,rewriter)? }
};
```
CRITICAL: the previewed text already has the camera clause (compile appended it), so the final-prompt branch MUST NOT call compile again — a 2nd `PromptParts::compile()` double-appends the camera template and re-charges/re-changes the rewrite. `preset_id` still stored for provenance (:893) but the clause is not re-derived. Recommend extracting a `fn compiled_for_submit(model, &SubmitInput) -> Result<Compiled,String>` holding this match, so AC2 is unit-testable off the store/fal path.

## Decision 3 — UI
Keep BOTH paths: quick Generate unchanged (silent enhance in submit_job); new opt-in Preview.
- New `ui/src/components/PromptPreview.tsx`: inline panel (not modal) in SettingsRail between GenerateButton and disclaimer. Props: `preview: PromptPreview`, editable `value`/`onChange` (textarea seeded `enhanced ?? prompt`), read-only Original block, `note` line, `onRetry` ("Retry for a different rewrite" + LLM-varies caveat), `onGenerate`, `pending`/`previewing`.
- `SettingsRail.tsx`: add "Preview prompt" button beside GenerateButton (:190-197); reuse `blockedReason` (:122-129) keyed on `previewValue.trim()` for the preview Generate.
- `App.tsx`: state `preview: PromptPreview|null`, `previewText`, `previewing`. `onPreview` guards model/route → `previewPrompt(input)` → store + `setPreviewText(enhanced ?? prompt)`. `onRetryPreview` re-calls. Generalize `send` (:405-433) to accept optional `finalPrompt`; `onGenerateFromPreview` = `send(previewText)` then clear preview. Quick `onSubmit` (:435-438) = `send()` unchanged.
- `api.ts`: `previewPrompt(input): Promise<PromptPreview>` (wrapped `{input}` like submitJob :599-602; DTO already camelCase; accept both spellings defensively). `mockPreview` alongside mockSubmit (mock.ts:593) for browser dev. submitJob already forwards whole input (carries finalPrompt).
- `types.ts`: `interface PromptPreview{prompt;original;enhanced?;version?;note?}` + `finalPrompt?: string|null` on SubmitInput.
- `lib.rs:81`: register preview_prompt.

## Decision 4 — Retry = re-call preview_prompt (nonzero temp → different rewrite). No seed plumbing.

## Decision 5 — edge cases
enhance off/no backend → Rewriter::None/Unavailable → enhanced None, prompt=original+camera, note=reason (harness.rs:157-201); panel shows editable text + note. Slow/120s → inherits enhance_or_original fallback; spinner via `previewing`. Blank edit → UI blockedReason (previewText.trim()<2) + Rust `.filter(!empty)` → falls to normal path (never empty submit).

## Tests
Rust: `preview_returns_a_compiled_prompt_and_creates_no_job` (enhance=false, deterministic); `a_final_prompt_is_used_verbatim_and_never_re_enhanced` (via extracted `compiled_for_submit`: final_prompt=Some + bogus rewriter → prompt verbatim, version None); `a_blank_final_prompt_falls_back_to_the_normal_compile_path`.
Vitest: PromptPreview.test.tsx (renders enhanced/original/note; edit→onChange; Retry→onRetry; Generate→onGenerate); api previewPrompt maps DTO + wraps {input}; submitJob forwards finalPrompt; optional App-level Preview→edit→Generate carries edited finalPrompt.

## Riskiest part
The `final_prompt` bypass: must send verbatim and NOT re-run compile (double camera clause + re-charge/re-change). `a_final_prompt_is_used_verbatim_and_never_re_enhanced` pins it.
