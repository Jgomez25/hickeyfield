# S7-enhancer-prompt-preview — ADVERSARIAL VERIFY

## Implementer's claim (verbatim, from implement.md)

> Built exactly the design's file-by-file plan: a Rust `preview_prompt` command
> (no `State`), a verbatim `final_prompt` submit path, and the opt-in UI preview
> panel. Quick Generate is unchanged.

And the riskiest-invariant claim:

> **No silent deviation from the final_prompt invariant.** The verbatim branch in `compiled_for_submit` does not call `harness::compile`; the AC2 test pins `prompt == "edited text"` exactly (no double camera clause) and `version == None` (no re-charged/re-recorded rewrite).

I tried to refute all of it and could not. Real output below.

## Commands

### Scope — `git diff --stat`
```
 .forge/state.json                  |   8 +-
 src-tauri/src/commands.rs          | 303 +++++++++++++++++++++++++++++++++----
 src-tauri/src/lib.rs               |   1 +
 ui/src/App.tsx                     |  65 +++++++-
 ui/src/api.ts                      |  32 ++++
 ui/src/components/SettingsRail.tsx |  50 ++++++
 ui/src/components/rail.css         | 128 ++++++++++++++++
 ui/src/mock.ts                     |  29 ++++
 ui/src/types.ts                    |  21 +++
 9 files changed, 602 insertions(+), 35 deletions(-)
```
Plus untracked new files: `PromptPreview.tsx`, `PromptPreview.test.tsx`, `api.test.ts`.
Scope is exactly the claimed set: commands.rs, lib.rs, ui (types/api/mock/App/SettingsRail
+ new PromptPreview.tsx + tests). No provider/vault/route/pricing source touched. PASS.

### AC5 regression gate (run first, all green)
```
core:  test result: ok. 653 passed; 0 failed; 9 ignored   (running 662 tests)
shell: test result: ok. 120 passed; 0 failed; 1 ignored   (running 121 tests)
main/doctests: 0/0/0
```
653 + 120 = 773 passed, 0 failed (baseline 770 + 3 new S7 tests). Matches claim.
```
cargo fmt --all --check          → FMT_OK
cargo clippy --workspace --all-targets -- -D warnings → Finished, 0 warnings
./scripts/lint-provenance.py     → PASS (no third-party hosts; no corpus match); EXIT=0
cd ui && pnpm test               → Test Files 14 passed (14); Tests 232 passed (232)
cd ui && pnpm build              → tsc clean; vite built in 814ms
```

## Per-AC results

| AC | What I verified | Evidence | Verdict |
|----|-----------------|----------|---------|
| AC1 | `preview_prompt(input) -> PreviewDto`, NO `State`, reuses `harness::compile`, no path to billing. Test non-vacuous. | Signature is `pub fn preview_prompt(input: SubmitInput) -> Result<PreviewDto, String>` — no `State`. Body: registry → probe_enhancer_env → select_rewriter/None → `harness::compile` → return DTO, and it ends *before* `route::resolve` (commands.rs:982), `state.prices.usd` (:987/:1013), and `submit_to_provider` (:1025) — all of which live only in `submit_job`. Test `preview_returns_a_compiled_prompt_and_creates_no_job` asserts prompt contains raw text AND `prompt.len() > raw.len()` (camera clause appended), `original` preserved, `enhanced` None, `version` None for enhance=false. Ran: ok. | PASS |
| AC2 | `final_prompt` used VERBATIM, `compile` bypassed. | `compiled_for_submit` matches `final_prompt.trim().filter(!empty)` → returns `Compiled{prompt=text, original=input.prompt, enhanced=Some(text), version=None, note="Previewed before generating; sent exactly as shown."}` WITHOUT calling `compile`. Test `a_final_prompt_is_used_verbatim_and_never_re_enhanced` uses enhance=true + bogus ollama choice + `final_prompt=Some("edited text")`; asserts `prompt == "edited text"` byte-for-byte, `version None`, verbatim note. Ran: ok. See adversarial reasoning below. Storage in submit_job maps `prompt=compiled.original` (raw), `enhanced_prompt=compiled.enhanced`, `enhancer_version=None`, `enhance_note=note`, wire=`compiled.prompt` — matches AC2's storage spec exactly. | PASS |
| AC3 | UI panel + api mapping; quick Generate unchanged. | `PromptPreview.test.tsx` (5 tests): renders enhanced (textarea value) + original + note; edit fires onChange with new text; Retry click → onRetry x1; Generate click → onGenerate x1; empty value + blockedReason → generate button disabled. `api.test.ts` (3 tests): `previewPrompt` calls invoke with `("preview_prompt", {input})` and maps DTO; keeps optional fields null; `submitJob` forwards `finalPrompt` verbatim. Quick path: `App.onSubmit` still calls `send()` with no arg → `finalPrompt: null`; only `onGenerateFromPreview` calls `send(previewText)`. All pass under `pnpm test`. | PASS |
| AC4 | Edges: enhance-off preview, blank final_prompt filtered in Rust + blocked in UI, fallback. | `a_blank_final_prompt_falls_back_to_the_normal_compile_path`: whitespace-only `final_prompt=Some("   ")` → `.filter(!empty)` drops it → normal compile (prompt contains raw + camera clause), note != verbatim note. Ran: ok. UI blocks preview Generate at `previewText.trim().length < 2` (SettingsRail.tsx). Enhance-off preview inherits `harness::compile`'s honest note (mockPreview + note passthrough). | PASS |
| AC5 | Full gate green. | See commands above: 773 rust / 232 ui pass, fmt/clippy/provenance/build all clean. | PASS |

## Break attempts

1. **Would AC2 catch a re-run of compile?** If `compile` had run on the verbatim
   branch, it would receive `&input.prompt = "original scene"` (not the edited text)
   and append the camera clause — producing a prompt containing "original scene" plus
   the clause, never the literal string `"edited text"`. The test asserts
   `compiled.prompt == "edited text"` exactly. So a re-compile (double camera clause,
   fallback note, or a changed rewrite) would flip the assertion. Additionally enhance=true
   + a bogus Ollama model would force compile to attempt/refuse a rewrite, further
   diverging. The invariant is genuinely pinned. Could not break.

2. **final_prompt with enhance=false → still verbatim?** `compiled_for_submit` matches on
   `final_prompt` BEFORE consulting `enhance`; the enhance flag is only read in the `None`
   arm. So a non-empty edit is always sent verbatim regardless of enhance. Confirmed by code
   path; consistent with the design intent.

3. **Whitespace-only edit submits empty?** No — `.map(str::trim).filter(|s| !s.is_empty())`
   drops it and the `None` arm compiles the raw prompt. Test `a_blank_final_prompt_...` proves
   the fallback path is taken (note != verbatim note, camera clause present). UI independently
   blocks at `trim().length < 2`. Two guards; could not submit empty.

4. **Path from preview_prompt to billing?** Grepped `submit_to_provider`, `route::resolve`,
   `prices` — all occur at commands.rs:982/987/1013/1025, strictly inside `submit_job`, after
   `preview_prompt` returns. `preview_prompt` takes no `State`, so it has no store/prices/fal
   handle. Structurally cannot bill.

5. **Idempotency / job creation.** DB job_sets count before and after the live enhancer run:
   both `3`. `preview_prompt` has no store access, so structurally creates no job.

## LIVE (bonus) — real Ollama gemma3:1b through the exact preview compile path

Ran the ignored live test that calls `harness::compile(..., true, Rewriter::Ollama{model:"gemma3:1b"})`
— the same call `preview_prompt` makes:
```
OLLAMA_ENHANCE_MODEL=gemma3:1b cargo test -p hickeyfield-tauri --lib \
  a_banana_gets_a_cinematic_rewrite_against_a_real_daemon -- --ignored --nocapture

running 1 test
enhanced: Some("A ripe banana with a subtle, slightly winking smile, positioned in the
center of a slightly blurred, close-up image. The banana's textured skin is rendered with
a slight gradient, indicating age and ripeness – the gradient color should gently shift
from dark green to yellowish-green.")
version: Some("enhancer.v1/image+ollama/gemma3:1b")
test harness::tests::a_banana_gets_a_cinematic_rewrite_against_a_real_daemon ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 120 filtered out; finished in 8.14s
```
The enhancer returns an expanded prompt with a version pin, distinct from the raw text —
the exact output a preview would show. Job_sets count in
`~/Library/Application Support/ai.hickeyfield.studio/hickeyfield.sqlite` was `3` before and
`3` after: no job created.

## Tree left as found
`git status --short` shows only the pre-existing modifications and the slice's own untracked
files (plus gitignored `ui/dist`). No source edited by me.

All AC1–AC5 hold, the final_prompt verbatim invariant is proven line-by-line (unit test pins
`prompt == "edited text"` + `version None`, and storage maps raw/final/None/note correctly),
and the full gate (773 rust / 232 ui, fmt, clippy, provenance, build) is green.

VERDICT: PASS
