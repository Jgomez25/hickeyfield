# S4-prompt-enhancer-wired — REVIEW (read-only, re-review after prior FAIL)

Re-reviewed the working-tree diff for this slice (HEAD == main; all changes uncommitted)
after the prior REVIEW FAIL was addressed. Scope: `git diff` over harness.rs, commands.rs,
enhancer.rs, lib.rs, and the UI (types.ts, api.ts, App.tsx, SettingsRail.tsx, rail.css,
EnhancerPicker.tsx + test). I ran the full gate this time (results below). Focus: whether the
prior MAJOR (UI shows a selection it never submits) and the two MINORs are genuinely fixed,
and no regression to the parts previously found clean.

## Prior findings — verification

### MAJOR (uncommitted picker default → silent no-enhance) — FIXED, two-front

**Front 1, UI commit.** `EnhancerPicker.tsx:46-63` adds a guarded `useEffect` that commits a
concrete `{backend, model}` via `onChange` when the picker can offer a backend but the current
`value` is missing or names a now-unavailable backend. The guard is correct and self-terminating:
- returns early when `!enhance` (line 50) and when neither backend is available (line 51);
- returns early when `value && currentBackendAvailable` (line 52), where `currentBackendAvailable`
  (lines 46-48) is true iff `value.backend` is one that is presently runnable;
- otherwise commits `{ollama, firstModel}` when Ollama is up with models, else `{openai, undefined}`.

Trace of the previously-failing config (OpenAI key stored AND Ollama up with models, Enhance on,
picker untouched): mount → `value=null` → effect commits `{ollama, firstModel}`. Next render:
`value` set and `currentBackendAvailable` true → effect early-returns; no further `onChange`, no
render loop. `firstModel` is non-empty because `hasOllama` requires `ollamaModels.length > 0`
(line 35). Displayed backend (`value.backend`) and model (`value.model`) now equal what submit
sends. `onChange` is `setEnhancer` (App.tsx:564 → SettingsRail.tsx:178), a stable setState
identity, and even were it not, the guard makes the effect idempotent. UI-shown == submitted.

**Front 2, backend auto precedence.** `select_rewriter` auto branch (commands.rs:779-791) now
matches `ollama_models.first()` with `ollama_up` FIRST and returns `Rewriter::Ollama{first}`, so a
runnable local Ollama is preferred over declining — even when an OpenAI key is present. The honest
`NOTE_NO_MODEL_CHOSEN` is emitted only when no Ollama model can run yet a key exists (line 786);
`NOTE_OLLAMA_NO_MODELS` and `NOTE_NOTHING_AVAILABLE` cover the remaining cases. No silent-raw path
remains in either the auto or explicit branches: every non-runnable arm returns
`Rewriter::Unavailable{note}`, and `harness::compile` surfaces that note with `enhanced=None` — a
note, never a fake success and never a silent raw send.

Both fronts are independently sufficient; together they close the contract even under a UI race.

### MINOR 1 (stale cross-backend model id) — FIXED
`EnhancerPicker.tsx:85-98` (`pickBackend`): switching to Ollama keeps `value.model` only when the
prior backend was already Ollama, otherwise resolves to `firstModel` (lines 90-91); switching to
OpenAI keeps the model only when already OpenAI, else `undefined` (line 95). No hosted id is
forwarded as an Ollama tag. The mount effect (line 53) likewise commits `firstModel`, never a
carried-over hosted id.

### MINOR 2 (double `/api/tags` probe) — FIXED
`commands.rs:831-838`: a single `enhancer::local_models(OLLAMA_URL)` call now yields both `ollama_up`
(`Ok` ⇒ true) and the model list; the earlier separate `detect_local()` probe is gone. `Err` ⇒
`(false, Vec::new())`, so a down daemon is one round trip, not two, and never an error on submit.

## Tests — non-vacuous, would fail under the old bug

- Rust `auto_with_a_key_and_ollama_still_runs_ollama_not_the_no_model_note` (commands.rs:1410-1421):
  asserts `select_rewriter(None, Some("sk-live"), true, &models)` returns `Ollama{"gemma3:1b"}`.
  Under the old auto branch (key present ⇒ `NOTE_NO_MODEL_CHOSEN` before considering Ollama) this
  matched `Unavailable` and panicked. Fails under the old bug — genuine.
- vitest `commits a concrete default rewriter on mount, no interaction needed`
  (EnhancerPicker.test.tsx:56-75): asserts `onChange` is called with `{ollama, "gemma3:1b"}` on
  mount with `value=null`. The pre-fix picker had no effect and never called `onChange` on mount →
  this assertion would fail. Genuine.
- vitest `does not emit on mount when Enhance is off or nothing is available` (lines 77-103):
  pins the two early-return guards (Enhance off; nothing runnable), guarding against an
  over-eager effect / render loop. Real.
- The remaining picker tests (model roster + emitted wire shape, honest-reason no-dropdown, OpenAI
  option + free-text model field, inert when off) and the seven `select_rewriter` branch tests
  cover AC2/AC3's honest-fallback family with concrete assertions, not tautologies.

## Checked and found clean (no regression to previously-sound parts)

- **Shared `finish_rewrite` / `compile` path (harness.rs):** unchanged in this re-review; the
  Local and Hosted branches still share one rewrite path, `EnhanceRequest` is built from the raw
  prompt, the camera clause is re-appended via `PromptParts{scene,..}.compile()`, the version pin is
  recorded only on `Rewritten`, and non-`Rewritten` falls back to the compiled prompt with the note
  and `enhanced=None`. No silent drop.
- **Honest fallback family (commands.rs:720-723, 745-792):** four original-prose notes, each ending
  "…sent exactly as you wrote it."; explicit-OpenAI requires key + non-empty model; explicit-Ollama
  requires daemon up + installed model (membership check); empty/whitespace models filtered. No
  panics, no invented hosted model id.
- **`with_base_url` (enhancer.rs):** test-only override defaulting to `None`; production
  URLs used unchanged when unset. Anthropic stays core-only (no vault slot) — deferral honored.
- **UI wiring:** App.tsx passes `enhancer`/`setEnhancer` through SettingsRail to the picker; the
  picker's `enhance` is bound to `settings.enhance` (SettingsRail.tsx:173), the real toggle.
  `submit_job` forwards `input.rewriter` into `select_rewriter` only when `enhance` is on
  (commands.rs:839-848); off ⇒ `Rewriter::None`, no vault read, no probe.
- **No dead code / AI-tells** introduced by the fix; comments (EnhancerPicker.tsx:39-45,
  commands.rs:775-790, 827-830) accurately describe the code they sit on.

Minor observation (not a finding, no fix required): the mount effect re-commits only on
*backend* availability change, not model-list churn, so if the installed set changes mid-session
such that a previously-chosen Ollama tag disappears, the model `<select>` can briefly show a stale
value until the user re-picks; `select_rewriter`'s membership check then declines with an honest
note rather than running a phantom model. Edge case, off the critical path, honest either way.

## Gate (run this review)

- `cargo test --workspace`: 754 passed, 0 failed, 1 ignored (>=740 required). GREEN.
- `cargo fmt --all --check`: clean. GREEN.
- `cargo clippy --workspace --all-targets -- -D warnings`: exit 0, no warnings. GREEN.
- `./scripts/lint-provenance.py`: PASS (no borrowed copy; no third-party media hosts). GREEN.
- `cd ui && pnpm test`: 222 passed (12 files), incl. 6 EnhancerPicker tests. GREEN.
- `cd ui && pnpm build`: built in 847ms. GREEN.

## Blocking findings

None. The prior MAJOR is genuinely fixed on both the UI-commit and backend-precedence fronts
(UI-shown selection == what is submitted), both MINORs are resolved, the new tests are
non-vacuous, and the full gate is green.

VERDICT: PASS
