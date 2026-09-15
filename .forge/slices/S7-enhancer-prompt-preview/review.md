# S7-enhancer-prompt-preview — REVIEW (production-standards)

Read-only review of the slice diff (working tree vs `main`): `commands.rs`, `lib.rs`,
`App.tsx`, `api.ts`, `SettingsRail.tsx`, `PromptPreview.tsx`, `types.ts`, `mock.ts`,
`rail.css`, and the two new test files. Scope limited to what this slice touched.

## Verdict summary
No blockers, no majors. Two minor findings and a set of explicitly-cleared checks.
The load-bearing AC2 invariant (verbatim final prompt, never re-compiled) is correct
and pinned by a genuine test. Gate is green.

## The load-bearing invariant — CLEAN
`compiled_for_submit` (commands.rs:853) handles the `final_prompt` branch by building a
`harness::Compiled` **directly** (commands.rs:865-874): `prompt = text`, `original =
input.prompt`, `enhanced = Some(text)`, `version = None`, `note = "Previewed before
generating; sent exactly as shown."`. It does NOT call `harness::compile` on this path,
so there is no double camera clause and no re-charged/re-changed rewrite. `submit_job`
sends `&compiled.prompt` over the wire (commands.rs:1025). Verified byte-for-byte by
`a_final_prompt_is_used_verbatim_and_never_re_enhanced`, which pairs a non-empty
`final_prompt` with `enhance = true` and a bogus Ollama choice — proving `compile` never
ran (no clause appended, no rewriter reached). Test passes.

## preview_prompt is side-effect-free — CLEAN
`preview_prompt` (commands.rs:928) takes **no `State`**. With no store/prices/fal handle
it structurally cannot resolve a route, read a price, or call `submit_to_provider`
(the only call site is commands.rs:1025, inside `submit_job`). It reuses the same
`harness::compile` and returns a `PreviewDto`. Registered in lib.rs:81. It never reads
`input.final_prompt`, so a preview always compiles fresh and cannot be tricked into
echoing a stale verbatim text.

## probe_enhancer_env extraction — CLEAN (no regression)
`probe_enhancer_env` (commands.rs:824) is the old submit_job block lifted verbatim: same
`enhance`-off short-circuit, same single `/api/tags` probe, same `Err → (false, [])`
mapping. `submit_job`'s quick path now flows through `compiled_for_submit`'s `None`
branch (commands.rs:877), preserving the enhance-and-submit behaviour. Workspace suite
(773 passed, 0 failed) shows no regression.

## Stored fields for an edited-preview submit — CLEAN
On the verbatim path the JobSet stores `prompt = compiled.original` (raw input,
commands.rs:1004), `enhanced_prompt = compiled.enhanced` (the final text),
`enhancer_version = None`, `enhance_note = "Previewed…"`. Matches AC2. The MetaRail's
enhanced chip still renders (both `prompt` and `enhanced_prompt` present).

## UI state — CLEAN with one caveat (see Minor 1)
- `previewText` seeded `p.enhanced ?? p.prompt` (App.tsx runPreview) — the full wire text
  in both the rewritten and no-rewrite cases. Correct.
- Retry (`onRetryPreview`) re-calls `runPreview`, which overwrites `previewText` — expected
  per AC3 ("Retry re-calls").
- Generate-from-preview sends `previewText` as `finalPrompt` via `send(previewText)`;
  quick Generate calls `send()` with no arg → `finalPrompt: null` → unchanged behaviour.
- Empty-edit blocked twice: UI `blockedReason` when `previewText.trim().length < 2`
  (SettingsRail) AND Rust `.filter(|s| !s.is_empty())` (commands.rs:864). Belt and braces.

## Tests — CLEAN (non-tautological)
Rust: `preview_returns_a_compiled_prompt_and_creates_no_job`,
`a_final_prompt_is_used_verbatim_and_never_re_enhanced`,
`a_blank_final_prompt_falls_back_to_the_normal_compile_path` — all assert real behaviour
(camera clause appended, verbatim passthrough, blank fallback). Vitest: PromptPreview (5)
covers render/edit/retry/generate/blocked; api.test.ts pins the `{ input }` wrapping, DTO
mapping, and that submitJob forwards `finalPrompt`. All pass (UI 232, Rust 773 workspace,
clippy clean, fmt clean, `pnpm build` succeeds).

## Findings

### Minor 1 — Stale preview provenance after editing the raw prompt (ui/src/App.tsx:452, commands.rs:1004)
The `preview` panel is invalidated only on a successful Generate-from-preview; it is not
cleared when the user edits the main prompt box, switches model, or toggles enhance. If
the user previews "a cat" (→ previewText "a cat, shot on 35mm.") then edits the raw prompt
to "a dog" without re-previewing, Generate-from-preview still sends the previewed text
verbatim — which is correct (the user gets exactly what the panel showed) — but
`submit_job` stores `prompt = input.prompt.clone()` = "a dog" (commands.rs:1004), so the
stored/displayed "original" no longer matches the "Your original: a cat" the panel showed
and the text the enhanced prompt was derived from. Standard: misleading provenance /
state consistency. The paid generation and wire prompt are correct, so this is cosmetic,
not a billing or correctness defect. Fix (follow-up, not blocking): reset `preview`/
`previewText` to null in a `useEffect` keyed on `prompt`/`modelId`/`settings`/`enhancer`,
so a changed input forces a re-preview before Generate-from-preview is available.

### Minor 2 — Duplicated probe+select_rewriter+compile (commands.rs:877-926 vs 934-963)
The `None` branch of `compiled_for_submit` and the body of `preview_prompt` both run
`probe_enhancer_env` → `select_rewriter`/`Rewriter::None` → `harness::compile`. This is
deliberate: `preview_prompt` must ALWAYS compile fresh and must NOT honor `final_prompt`,
whereas `compiled_for_submit` must honor it — so `preview_prompt` cannot simply delegate
to `compiled_for_submit`. The duplication is defensible on those grounds. Optional
follow-up: extract a `fn compile_fresh(model, &SubmitInput) -> Result<Compiled, String>`
(the ~15 shared lines without the verbatim branch) and call it from both, removing the
copy without touching the final_prompt semantics. Not blocking.

## AI-code tells / dead code — none found
No invented APIs (`harness::compile`, `select_rewriter`, `vault::get`, `local_models` all
pre-exist and are used with correct signatures). Comments match the code. No unused
imports or dead branches introduced. serde casing consistent: Rust `SubmitInput` is
`rename_all = "camelCase"`, so `final_prompt` ⇄ UI `finalPrompt`; `PreviewDto` is
camelCase and the UI reads `enhanced`/`version`/`note` directly.

VERDICT: PASS
