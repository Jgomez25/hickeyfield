# S4-prompt-enhancer-wired — TEST (adversarial re-verification, round 2)

Re-run after the review-driven fix to the submit/selection path:
(1) `EnhancerPicker` commits its resolved default via `onChange` on mount;
(2) `select_rewriter`'s auto branch prefers a runnable Ollama over the no-model note.

## Implementer's claim (verbatim excerpt)

> - **MAJOR (UI truth == submit truth).** `EnhancerPicker` now commits its resolved default via
>   `onChange` in a `useEffect` as soon as availability/models resolve, so `App.enhancer` is
>   never `null` while the picker shows a concrete backend+model. ...
> - **MAJOR (backend fallback).** `select_rewriter`'s auto branch now prefers a runnable Ollama
>   (up + ≥1 model → `Rewriter::Ollama { first }`) over declining. ...
> Re-run gate: `cargo test --workspace` **754 passed, 0 failed** (638 core + 116 shell; 10
> ignored); `cargo fmt --all --check` clean; `cargo clippy --workspace --all-targets -- -D
> warnings` clean; `./scripts/lint-provenance.py` exit 0; `pnpm test` **222 passed**;
> `pnpm build` built.

I re-ran everything myself. Every claim reproduced.

## Scope check

`git diff --stat` — enhancer surface only, no Anthropic/vault/Cargo creep:

```
 crates/hickeyfield-core/src/enhancer.rs | 100 +++++++++-
 src-tauri/src/commands.rs               | 263 +++++++++++++++++++++++++-
 src-tauri/src/harness.rs                | 323 ++++++++++++++++++++++++++++----
 src-tauri/src/lib.rs                    |   1 +
 ui/src/App.tsx                          |  24 ++-
 ui/src/api.ts                           |  13 ++
 ui/src/components/SettingsRail.tsx      |  24 +++
 ui/src/components/rail.css              |  38 ++++
 ui/src/types.ts                         |  12 ++
```

- `grep -rn "ProviderId::Anthropic" src-tauri crates` → **0 hits**.
- Cargo manifest/lock in diff → **0**. `vault.rs`/`provider.rs` in diff → **0**.
- Anthropic deferral honoured; `HostedBackend::Anthropic` exercised only by the additive core socket test, wired to no user path. No scope creep.

## Commands (per AC, real output)

| AC | Check | Result |
|----|-------|--------|
| MAJOR-1 | Rust auto-select with key AND Ollama-up returns Ollama, not the no-model note | PASS — `auto_with_a_key_and_ollama_still_runs_ollama_not_the_no_model_note` ok; non-vacuous |
| MAJOR-2 | vitest: picker forwards concrete `{backend,model}` on mount, no interaction; silent when Enhance off / nothing available | PASS — 6/6 EnhancerPicker cases ok |
| AC1 | `Rewriter::Hosted`/`Unavailable`; hosted-compile TcpListener stub via `with_base_url` | PASS — harness + enhancer socket tests ok |
| AC2 | `select_rewriter` precedence + honest note (never silent raw); auto declines OpenAI-w/o-model | PASS — 7 branch tests ok |
| AC3 | `list_ollama_models` registered, degrades to empty; picker wired + tested | PASS |
| AC4 | LIVE Ollama gemma3:1b end-to-end | PASS — pasted below |
| AC5 | full gate green | PASS |

### MAJOR-1 — auto prefers a runnable Ollama over declining (the fixed bug)

`select_rewriter` (commands.rs:779-791) auto branch matches `ollama_models.first()`:
`Some(first) if ollama_up => Rewriter::Ollama { first }` comes **before** the
`_ if openai_key.is_some() => NOTE_NO_MODEL_CHOSEN` arm. Non-vacuous: the test drives
`select_rewriter(None, Some("sk-live"), true, &["gemma3:1b","qwen3-vl:4b"])` and asserts
`Rewriter::Ollama{"gemma3:1b"}`. Under the old bug (OpenAI-key check first) it would return
`Unavailable{NOTE_NO_MODEL_CHOSEN}` and the `match` arm would panic. So the test genuinely fails
the old behaviour.

```
test commands::tests::auto_with_a_key_and_ollama_still_runs_ollama_not_the_no_model_note ... ok
test commands::tests::auto_with_only_ollama_available_picks_the_first_installed_model ... ok
test commands::tests::an_explicit_openai_choice_with_a_key_and_model_reaches_the_hosted_backend ... ok
test commands::tests::an_explicit_openai_choice_without_a_key_is_an_honest_note ... ok
test commands::tests::an_explicit_ollama_choice_with_an_installed_model_runs_local ... ok
test commands::tests::auto_with_nothing_available_is_the_honest_nothing_note ... ok
test commands::tests::ollama_up_with_no_models_says_none_are_installed ... ok
test commands::tests::auto_never_silently_runs_openai_without_a_chosen_model ... ok
test commands::tests::a_list_of_installed_models_never_errors ... ok
test result: ok. 33 passed; 0 failed; 0 ignored; 0 measured; 84 filtered out
```

### MAJOR-2 — picker commits its default on mount (UI truth == submit truth)

`EnhancerPicker.tsx:49-63` — a guarded `useEffect` fires `onChange` when Enhance is on, a backend
is available, and `value` is missing/names an unavailable backend; self-terminating (the guard
`if (value && currentBackendAvailable) return` short-circuits on the next render). vitest proves
it emits the concrete rewriter with NO interaction, and stays silent otherwise:

```
✓ src/components/EnhancerPicker.test.tsx (6 tests) 116ms
```

- `commits a concrete default rewriter on mount, no interaction needed` → `onChange` called with
  `{backend:"ollama", model:"gemma3:1b"}` even though `openaiAvailable` is true (Ollama preferred,
  matching the select_rewriter auto fix — consistent both ends).
- `does not emit on mount when Enhance is off or nothing is available` → `onChange` NOT called in
  either case. Honest silence preserved.
- `renders the installed Ollama models and emits the chosen rewriter` → options
  `["gemma3:1b","qwen3-vl:4b"]`, selecting emits `{backend:"ollama",model:"qwen3-vl:4b"}`.

### AC1 — hosted compile path + socket proof

```
test harness::tests::a_hosted_rewrite_enhances_and_keeps_the_camera_clause ... ok
test harness::tests::a_hosted_rewriter_with_no_key_falls_back_to_the_original_prompt ... ok
test harness::tests::an_unavailable_rewriter_surfaces_its_honest_note ... ok
test enhancer::tests::a_hosted_openai_call_posts_to_the_overridden_base_and_reads_the_reply ... ok
test enhancer::tests::a_hosted_anthropic_call_posts_to_the_overridden_base_and_reads_the_reply ... ok
```

`compile()` feeds `raw_prompt` (harness.rs:209) to the rewriter and re-appends the camera clause
after via a shared `finish_rewrite`; the hosted test binds a real `TcpListener`, asserts the exact
stub reply body was read/parsed and the camera clause survived. Non-vacuous.

### AC2 — honest fallback fires ONLY when nothing runnable

The four note constants (commands.rs:720-723) each end "Your prompt was sent exactly as you wrote
it." `Rewriter::Unavailable{note}` is surfaced verbatim by `compile`
(`an_unavailable_rewriter_surfaces_its_honest_note` ok) — never silent raw. Empty hosted key
falls back honestly (`a_hosted_rewriter_with_no_key_falls_back…` ok). Auto with an OpenAI key but
no runnable Ollama and no model declines with the pick-a-model note (never invents an id).

### AC4 — LIVE end-to-end against the running Ollama

`curl :11434/api/tags` → `['qwen3:8b', 'deepseek-coder:1.3b', 'deepseek-r1:1.5b', 'deepseek-r1:8b',
'gemma3:1b', 'qwen3-coder:30b', 'qwen3-vl:4b']`. Ran the `#[ignore]`d live test with the mandated
non-reasoning `gemma3:1b`:

```
OLLAMA_ENHANCE_MODEL=gemma3:1b cargo test -p hickeyfield-tauri --lib harness -- --ignored --nocapture a_banana_gets_a_cinematic_rewrite_against_a_real_daemon

enhanced: Some("A perfect yellow banana, illuminated by a sunbeam, positioned squarely in the foreground, facing directly toward the left, slightly wet from condensation, with textured peel, shallow depth of field, warm, softly lighting.")
version: Some("enhancer.v1/image+ollama/gemma3:1b")
test harness::tests::a_banana_gets_a_cinematic_rewrite_against_a_real_daemon ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 116 filtered out; finished in 6.33s
```

The enhanced prompt (222 chars) is materially richer than the raw "a banana with a smile" (21
chars), clearly distinct, pins `ollama/gemma3:1b`, and carries no `<think>` leakage. Live check
genuinely ran on the user's daemon.

### AC5 — regression gate

```
cargo test --workspace   → 638 passed + 116 passed (+0/0/0) = 754 passed, 0 failed (9+1 ignored)
cargo fmt --all --check   → FMT_EXIT=0
cargo clippy --workspace --all-targets -- -D warnings → CLIPPY_EXIT=0, 0 warnings/errors
./scripts/lint-provenance.py → PROV_EXIT=0 (CDN PASS, Copy provenance PASS, 80015 shingles)
cd ui && pnpm test → Test Files 12 passed, Tests 222 passed (incl. 6 EnhancerPicker)
cd ui && pnpm build → ✓ built in 832ms
```

`cargo deny check advisories` → `advisories FAILED` on RUSTSEC-2026-0258 (h2 unbounded empty DATA
frames) and RUSTSEC-2026-0285 (tokio-rustls TLS 1.3 handshake). Transitive-dep advisories; the
diff touches no `Cargo.toml`/`Cargo.lock`, so they predate S4. Per the gate instruction: noted, not
blocking.

## Break attempts

1. **MAJOR-1 vacuity** — traced the auto arm order: Ollama-first precedes the OpenAI-key arm, so
   the key+Ollama test would panic under the old ordering. Genuinely refutes the old bug. PASS.
2. **MAJOR-2 render loop** — the `useEffect` guard `if (value && currentBackendAvailable) return`
   is self-terminating once the default commits; vitest mount test terminates (176ms) with a
   single `onChange`. No loop. PASS.
3. **Stale hosted id leaking as an Ollama tag** — `pickBackend` (EnhancerPicker.tsx:85-98) keeps
   the model only when the backend is unchanged; switching OpenAI→Ollama resolves to `firstModel`,
   Ollama→OpenAI clears it. No stale hosted id carried as an Ollama tag. PASS.
4. **Single /api/tags probe per submit** — `submit_job` (commands.rs:827-838) calls
   `enhancer::local_models` once (Ok=up-with-models, Err=down), gated on `enhance`, replacing the
   old `detect_local()`+`list_ollama_models` double probe. One probe, and skipped entirely when
   Enhance is off. PASS.
5. **Enhance OFF** — `submit_job` passes `Rewriter::None` and skips the vault/daemon read; picker
   emits nothing (vitest "does not emit … when Enhance is off"). Raw sent honestly, no rewrite. PASS.
6. **Empty/missing hosted key** — falls back to the original prompt with a note, never errors. PASS.
7. **Reasoning-model `<think>` leak** — avoided by mandating gemma3:1b; pasted prompt has no
   residue. Design Non-goal, in-bounds. PASS.
8. **Tree left as found** — dummy keychain env scoped to subshells; gemma3:1b already installed,
   nothing pulled. `git status` shows only the expected modified files + the two new EnhancerPicker
   files + the slice dir. No stray temp files.

## Verdict

Both MAJORs are fixed and proven: the auto branch now runs a live Ollama even when an OpenAI key
is stored (Rust, non-vacuous), and the picker commits its resolved default on mount so submit
truth equals what is shown (vitest, no interaction) while staying silent when Enhance is off or
nothing can run. AC1–AC5 hold: hosted backend reaches `HostedEnhancer` through a shared recompile
tail that feeds raw prompt and preserves the camera clause; selection is honest and never silent;
the list-models command and picker are wired and tested; the LIVE Ollama run produced an expanded,
distinct cinematic prompt with a model-pinned version; and the full gate is green (754/0,
fmt/clippy/provenance clean, 222 UI tests, build ok). cargo-deny advisories are the pre-existing
transitive baseline, correctly noted and out of this slice's scope. Edge checks (single probe, no
stale-model carryover) confirmed.

VERDICT: PASS
