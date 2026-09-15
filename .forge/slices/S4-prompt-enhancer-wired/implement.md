# S4-prompt-enhancer-wired — IMPLEMENT claim

## REVIEW round 2 — fixes for the MAJOR + two minors (all addressed)

- **MAJOR (UI truth == submit truth).** `EnhancerPicker` now commits its resolved default via
  `onChange` in a `useEffect` as soon as availability/models resolve, so `App.enhancer` is
  never `null` while the picker shows a concrete backend+model. The effect is guarded
  (fires only when Enhance is on, a backend is available, and the current value is missing or
  names an unavailable backend) so it is self-terminating — no render loop.
  `ui/src/components/EnhancerPicker.tsx`.
- **MAJOR (backend fallback).** `select_rewriter`'s auto branch now prefers a runnable Ollama
  (up + ≥1 model → `Rewriter::Ollama { first }`) over declining. It only emits the
  pick-a-model note when there is no Ollama model to run *and* an OpenAI key is stored, and
  the honest "nothing available" note only when nothing can run at all.
  `src-tauri/src/commands.rs`.
- **MINOR 1.** Switching OpenAI→Ollama (or back) no longer carries the stale hosted model id
  as the Ollama tag: `pickBackend` keeps the model only if the backend is unchanged, else
  resolves to the first installed model (Ollama) / clears it (OpenAI).
- **MINOR 2.** `submit_job` probes `/api/tags` once via `enhancer::local_models` (`Ok` = up,
  `Err` = down) instead of `detect_local()` + `list_ollama_models()` back to back.

New tests for the MAJOR: `auto_with_a_key_and_ollama_still_runs_ollama_not_the_no_model_note`
(Rust — key+Ollama config enhances, does not return the no-model note) and two vitest cases
proving the picker forwards a concrete rewriter on mount without interaction (and does *not*
when Enhance is off / nothing is available). The honest-fallback tests are unchanged and
still green.

Re-run gate: `cargo test --workspace` **754 passed, 0 failed** (638 core + 116 shell; 10
ignored); `cargo fmt --all --check` clean; `cargo clippy --workspace --all-targets -- -D
warnings` clean; `./scripts/lint-provenance.py` exit 0; `pnpm test` **222 passed**;
`pnpm build` built. **The key+Ollama config now enhances** — proven by the new Rust selection
test (auto returns `Rewriter::Ollama { "gemma3:1b" }`) and the vitest mount-commit test
(picker emits `{backend:"ollama", model:"gemma3:1b"}` with no user interaction), and belt-and-
suspenders by the auto-branch fallback should `enhancer` ever be null.

---


Built exactly the design: ship OpenAI + Ollama, Anthropic deferred (extensibility
preserved). No `ProviderId`/`features()`/onboarding change. No `JobSet`/store schema
change. No `app.rs` change.

## Files changed (by AC)

### Rust
- **`crates/hickeyfield-core/src/enhancer.rs`** (AC1 test-seam) — the one intended core
  public-API addition: `HostedEnhancer::with_base_url(url)` (mirrors `LocalEnhancer`),
  a `base_url: Option<String>` field (default `None` = production hosts, so every existing
  hosted test is untouched), and `call()` now routes each backend through
  `{base}{OPENAI_CHAT_PATH|ANTHROPIC_MESSAGES_PATH}` when overridden. Added two tests:
  `a_hosted_openai_call_posts_to_the_overridden_base_and_reads_the_reply` and the Anthropic
  twin — a std `TcpListener` stub proves the socket path with no new dependency.
- **`src-tauri/src/harness.rs`** (AC1, AC2 surfacing, AC4 live) — added
  `Rewriter::Hosted { backend, api_key, model }` and `Rewriter::Unavailable { note }`;
  refactored the Ollama-only tail into a shared `finish_rewrite(&dyn Enhancer, …)` so Local
  and Hosted run the identical `mode_for`/`system_prompt_for`/`enhance_or_original`/recompile
  flow; `compile()` still feeds `raw_prompt` (not the camera clause) to the rewriter. Tests:
  `a_hosted_rewrite_enhances_and_keeps_the_camera_clause` (socket-mocked, camera clause
  survives, version pin = `openai/gpt-test`), `a_hosted_rewriter_with_no_key_falls_back…`,
  `an_unavailable_rewriter_surfaces_its_honest_note`, and the `#[ignore]`d live
  `a_banana_gets_a_cinematic_rewrite_against_a_real_daemon` (AC4, human-run in TEST).
- **`src-tauri/src/commands.rs`** (AC2, AC3) — replaced the unused `rewriter: Option<String>`
  with a structured `RewriterChoice { backend, model }` on `SubmitInput`; added the pure
  `select_rewriter(choice, openai_key, ollama_up, ollama_models)` (explicit choice → auto →
  honest note), four original provenance-safe note strings each ending "sent exactly as you
  wrote it.", the `list_ollama_models` command (Ollama-down → empty list, never an error),
  and wired `submit_job` to select the rewriter (skipping the vault read + daemon probe when
  enhance is off, passing `Rewriter::None`). Tests: seven `select_rewriter` branch tests
  (explicit-openai-with-key, explicit-openai-no-key, explicit-ollama-installed, auto-ollama-
  first, auto-nothing, ollama-up-no-models, auto-openai-declines) + a `list_ollama_models`
  smoke test.
- **`src-tauri/src/lib.rs`** (AC3) — registered `commands::list_ollama_models` in
  `invoke_handler!`.

### UI
- **`ui/src/types.ts`** (AC3) — `RewriterChoice { backend; model? }` + optional
  `rewriter?: RewriterChoice | null` on `SubmitInput`.
- **`ui/src/api.ts`** (AC3) — `ollamaModels()` wrapper (invoke → `[]` on any failure);
  `submitJob` already forwards the whole `input`, so `rewriter` rides along.
- **`ui/src/App.tsx`** (AC3/AC4) — `enhancer` + `ollamaModelList` state, loaded on mount and
  in `recheckLocal`; passes availability (`ollamaUp`, models, `openaiAvailable` derived from
  the `openai` key state) + `enhancer`/`onEnhancerChange` into `SettingsRail`; sends
  `rewriter: enhancer` in the `submitJob` payload (added to the `send` deps).
- **`ui/src/components/EnhancerPicker.tsx`** (new, AC3) — backend `<select>` (only available
  options) + Ollama model `<select>` / OpenAI free-text model field; honest reason text when
  nothing can run; inert when Enhance is off.
- **`ui/src/components/EnhancerPicker.test.tsx`** (new, AC3/AC5) — four cases: models render +
  selecting emits `{backend:"ollama",model:"qwen3-vl:4b"}` (the exact wire shape App
  forwards), honest-unavailable state with no dropdown, OpenAI option when a key is stored,
  nothing rendered when Enhance is off.
- **`ui/src/components/SettingsRail.tsx`** (AC3) — renders `EnhancerPicker` after `PromptCard`.
- **`ui/src/components/rail.css`** (AC3) — minimal styling for the picker.

## AC4 confirmation
The wire fix alone closes AC4, as the design predicted: `submit_job` already copied
`compiled.enhanced`/`.version`/`.note` onto the `JobSet` and sent `&compiled.prompt`; serde
snake_case → `RawJobSet` → `toJobSet` already mapped them to `enhancedPrompt`/`enhancerVersion`/
`enhanceNote`. The only missing hop was the UI sending a `rewriter`. Proven by
`a_hosted_rewrite_enhances_and_keeps_the_camera_clause` (`out.enhanced` distinct, `out.version`
set).

## Verification (all run; outputs observed)
- `cargo test --workspace` → **754 passed, 0 failed** (638 core + 116 shell; 10 ignored),
  baseline 740. ✅
- `cargo fmt --all --check` → clean. ✅
- `cargo clippy --workspace --all-targets -- -D warnings` → Finished, no warnings. ✅
- `./scripts/lint-provenance.py` → PASS (CDN hosts, copy provenance), exit 0. The four note
  strings are original prose about this app and did not trip the shingle check. ✅
- `cd ui && pnpm test` → **222 passed** (incl. 6 EnhancerPicker tests). ✅
- `cd ui && pnpm build` (tsc --noEmit + vite build) → built. ✅
- Not run (per instructions): `tauri build` (RELEASE), the live Ollama generation (TEST phase
  runs the `#[ignore]`d `a_banana_gets_a_cinematic_rewrite_against_a_real_daemon` with
  `OLLAMA_ENHANCE_MODEL=gemma3:1b cargo test -p hickeyfield-tauri --lib harness -- --ignored --nocapture`).

## Deviations / known limitations (nothing hidden)
- **No full `submit_job` integration test with a stub-provider `AppState`.** The design
  suggested one "like the existing submit tests," but `commands.rs` has no such harness — all
  its tests are pure-function. The honest-note-persistence contract is instead proven at the
  two seams the note actually flows through: `select_rewriter` returns `Unavailable { note }`
  (commands tests assert the exact strings), and `compile` surfaces that note unchanged
  (`an_unavailable_rewriter_surfaces_its_honest_note`). `submit_job` copies `compiled.note` →
  `enhance_note` via wiring that is unchanged from before this slice.
- **"Submit payload carries the chosen rewriter" is verified at the component boundary**, not
  end-to-end through App: `EnhancerPicker`'s `onChange` emits the exact `{backend, model}`
  shape, and `App.tsx` passes `rewriter: enhancer` straight into `submitJob`.
- **Auto-mode never invents a hosted OpenAI model** (Q-AUTO-MODEL): with an OpenAI key
  present but no chosen model *and no runnable Ollama*, `select_rewriter` returns the "pick a
  model" note rather than inventing a hosted id. Per the round-2 fix, a runnable Ollama is now
  preferred first, so auto only declines when there is genuinely nothing to run.
- **Reasoning-model `<think>` leakage is out of scope** (design Non-goal): the live-check
  model must be non-reasoning (`gemma3:1b`); `clean_reply` does not strip `<think>` blocks.
