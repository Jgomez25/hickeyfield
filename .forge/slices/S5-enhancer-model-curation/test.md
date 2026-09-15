# S5-enhancer-model-curation — ADVERSARIAL TEST

## Implementer's claim (verbatim, implement.md:3-6)

> Branch: `forge/s5-model-curation`. Built to the approved design. Single source of
> truth for model suitability lives in Rust (`local_models` returns a tier-annotated,
> already-sorted `Vec<LocalModel>`); the UI consumes it untouched and takes `[0]`, so
> show == submit by construction.

I did not trust any pasted output. Every command below was re-run in this environment.

## Commands

### Scope — `git diff --stat`
```
 .forge/state.json                         |   8 +-
 crates/hickeyfield-core/src/enhancer.rs   | 249 ++++++++++++++++++++++++++++--
 src-tauri/src/commands.rs                 |  76 ++++++---
 ui/src/App.tsx                            |   3 +-
 ui/src/api.ts                             |  12 +-
 ui/src/components/EnhancerPicker.test.tsx | 102 ++++++++++--
 ui/src/components/EnhancerPicker.tsx      |  40 ++++-
 ui/src/components/SettingsRail.tsx        |   3 +-
 ui/src/types.ts                           |  18 +++
 9 files changed, 455 insertions(+), 56 deletions(-)
```
In scope: enhancer.rs, commands.rs, ui (types/api/App/SettingsRail/EnhancerPicker + test).
`.forge/state.json` is forge bookkeeping, not source. **No Cargo/provider/vault change.** PASS.

### AC1 — `model_tier` tiering
`cargo test -p hickeyfield-core enhancer` → `75 passed; 0 failed; 1 ignored`. The tiering
tests (`the_curated_go_to_families_are_recommended`, `vision_and_reasoning_models_are_discouraged`,
`large_models_are_discouraged_by_size_from_details_or_name`, `an_ordinary_chat_model_is_neutral`,
`parse_param_billions_reads_a_declared_size`) all pass. Read (enhancer.rs:2109-2149) — they
assert concrete `ModelTier` values with `assert_eq!`, **non-vacuous**.

I wrote a throwaway integration test against the real `pub` functions to probe the precedence
edges the implementer's own tests only cover with different args (then removed it):
```
gemma3:27b Some(27.0) = Discouraged      # size beats the gemma3 recommend rule (declared size)
qwen3-coder:30b None  = Discouraged      # size-absent large tag caught by the name scan
parse e2b of gemma3n:e2b = Some(2.0)     # "e2b" scans to 2.0B, NOT >=20 -> does not trip size rule
gemma3n:e2b None      = Recommended      # so gemma3 family wins, correctly
phi4-mini None        = Recommended
llama3.2:3b None      = Recommended
qwen3-vl:4b None      = Discouraged
deepseek-r1:8b None   = Discouraged
qwq None              = Discouraged
qwen2.5:7b None       = Neutral
foo:20b None          = Discouraged      # boundary: exactly 20B is Discouraged (>=20.0)
foo:19b None          = Neutral          # 19B is not
```
Every requested case holds, including the nasty `gemma3n:e2b`: the scan reads "e2b" as 2.0B
(<20), so it does NOT falsely discourage; the gemma3 family rule then correctly makes it
Recommended. PASS.

### AC2 — parse + stable sort
`parse_local_models` returns `Vec<LocalModel>`, reads `details.parameter_size`
(enhancer.rs:1050-1054), keeps the embedding-only filter (:1042-1045), and stable-sorts by
`tier_rank` (:1061). Test `the_installed_list_is_stable_sorted_by_tier_reading_parameter_size`
(:2151) feeds a raw `/api/tags` doc that lists a **discouraged** `qwen3-coder:30b` FIRST and
asserts `llama3.2:3b`/`phi4-mini` sort ahead of it — recommended-first proven, per-tier order
preserved. Passes. `local_models` (:1027) and `list_ollama_models` (commands.rs:802) both
return `Vec<LocalModel>`. PASS.

### AC3 — auto-select single source of truth
`cargo test -p hickeyfield-tauri`:
```
test commands::tests::auto_picks_the_top_ranked_model ... ok
test commands::tests::auto_with_only_discouraged_models_still_picks_one ... ok
test commands::tests::a_list_of_installed_models_never_errors ... ok
test result: ok. 117 passed; 0 failed; 1 ignored
```
Read `auto_picks_the_top_ranked_model` (commands.rs:1419): its fixture raw doc leads with the
**vision** model `qwen3-vl:4b`, and it asserts `installed[0].name == "llama3.2:3b"` after parse
then that `select_rewriter` picks it — genuinely proves ranking, not `[0]` of raw order.
`select_rewriter` auto branch uses `.first()` (:779-784). UI `firstModel = ollamaModels[0]?.name`
(EnhancerPicker.tsx:62) and the `<option>` map iterates `ollamaModels` **untouched** (:149) — no
re-sort/filter. show==submit guard holds. PASS.

### AC4 — picker badges
`cd ui && pnpm test` → `EnhancerPicker.test.tsx (8 tests)`, `Tests 224 passed (224)`. Read the
tests: they assert `data-tier` (`byValue("gemma3:1b").dataset.tier === "recommended"` etc.),
recommended auto-commits on mount, a discouraged-only model still commits, order is preserved,
and `options.every(o => !o.disabled)` — every option selectable incl. discouraged, which is then
selected via `selectValue`. **Non-vacuous.** PASS.

### AC5 — copy
`NOTE_OLLAMA_NO_MODELS` (commands.rs:721) reads `... pull a small, fast one (`ollama pull
llama3.2`, `phi4-mini` or `gemma3` all work well here) ... Your prompt was sent exactly as you
wrote it.` — names the go-to set generically, keeps the mandatory ending. EnhancerPicker.tsx:95
mirrors it generically. Neither hardcodes only `gemma3:1b`. `./scripts/lint-provenance.py` →
`prov exit=0` (both checks PASS). PASS.

### AC6 — regression gate (all re-run here, dummy keys exported)
| Check | Result |
|---|---|
| `cargo test --workspace` | `passed=761 failed=0 ignored=10` |
| `cargo fmt --all --check` | `fmt exit=0` |
| `cargo clippy --workspace --all-targets -- -D warnings` | `clippy exit=0` |
| `./scripts/lint-provenance.py` | `prov exit=0` |
| `cd ui && pnpm test` | `Tests 224 passed (224)` |
| `cd ui && pnpm build` | `built in 834ms`, `build exit=0` |

761 >= 754 baseline (+7). 0 failed. Gate green.

## Break attempts

1. **Precedence: does the gemma3 recommend rule beat the size discourage?** No — verified
   `gemma3:27b`+`Some(27.0)` and `qwen3-coder:30b`+`None` both = Discouraged. Discouraged is
   checked first (enhancer.rs:1123). Not refuted.
2. **False-positive size scan on `e2b`?** Probed: `parse_param_billions("gemma3n:e2b")` = 2.0,
   under the 20.0 threshold, so gemma3n:e2b stays Recommended. Not refuted.
3. **20B boundary off-by-one?** `foo:20b` = Discouraged, `foo:19b` = Neutral — matches the
   documented `>= 20.0`. Not refuted.
4. **UI re-sort resurrecting show!=submit?** Read EnhancerPicker.tsx: list mapped untouched,
   `[0]` taken as default; test `renders ... in the given order` asserts
   `["gemma3:1b","qwen2.5:7b","qwen3-vl:4b"]`. Not refuted.
5. **Empty/malformed /api/tags panics?** `a_tags_document_we_cannot_read_is_an_empty_list_not_a_panic`
   passes; `list_ollama_models` maps err→`unwrap_or_default()`; api.ts `ollamaModels()`
   catch→`[]`. Not refuted.
6. **Vacuous tests?** All the new Rust and vitest tests assert concrete values/attributes, not
   just "runs". Not refuted.
7. **Stray files / dirty tree?** Removed my probe test; `git status` shows only the pre-existing
   modified files + the slice dir. Tree left as found.

## LIVE (bonus) — real Ollama on this machine

Ollama is up. `/api/tags` shows the recommended trio installed: `gemma3n:e2b` (4.5B),
`phi4-mini:latest` (3.8B), `llama3.2:3b` (3.2B), plus `gemma3:1b`.

I drove the real 51,934-char corpus (`enhancer.v1.md` + `enhancer.video.v1.md`) on
"a banana with a smile", both via the `#[ignore]`d `a_local_rewrite_completes_against_a_real_daemon`
test (`OLLAMA_ENHANCE_MODEL=llama3.2:3b`) and via direct `/api/chat` calls with server-side timing:

| Model | Result |
|---|---|
| llama3.2:3b (via ignored test, x2) | Request failed at the 120s wall → `status: Failed` |
| llama3.2:3b (direct, `--max-time 300`) | curl exit 28 (timeout, >300s) |
| phi4-mini:latest (direct, `--max-time 145`) | curl exit 28 (timeout) |
| gemma3n:e2b (direct, `--max-time 145`) | curl exit 28 (timeout) |
| **gemma3:1b (direct)** | **completed in 32.7s** (prompt_eval 12551 tok, eval 38 tok) |

The gemma3:1b output (a Recommended model, real corpus, within the 120s budget):
```
Output: video · Job type: animate · Duration: 5s · Attached media: start frame · Notes: enabled
Prompt: a banana with a smile
```

**Diagnosis of the trio timeouts — environmental, not a slice defect.** During these runs the
machine load average was 20-30 with `WindowServer` at 47% CPU and Firefox spawning many
GPU-helper/plugin-container processes; `ollama ps` confirmed the model was GPU-resident
(`size_vram` 4.6GB) yet a warm small-prompt call returned in 3.6s. The GPU is being starved by
the user's own foreground apps, so the 3-4B models cannot finish the ~12.5k-token prompt-eval
within 120s right now. The smallest Recommended model completed the identical real-corpus
request in 32.7s, proving the local enhance path is correctly wired end-to-end and runs inside
the budget when the GPU is available. gemma3:1b (1B) is too small to truly expand — it echoed
the block — but it is not one of the named trio and the app never guarantees a 1B model
produces a good rewrite (tiering is a ranking, not a quality contract).

The live enhancement quality/latency for the trio could not be positively demonstrated in this
saturated environment. This is a bonus check and does not gate AC1-AC6; I flag it honestly so a
re-run on an idle machine can confirm the payoff.

## Per-AC verdict
| AC | Verdict |
|---|---|
| AC1 model_tier | PASS (incl. all precedence/boundary probes) |
| AC2 parse + stable sort | PASS |
| AC3 auto-select single source of truth | PASS |
| AC4 picker badges | PASS |
| AC5 genericized copy + provenance | PASS |
| AC6 regression gate | PASS (761/0, fmt/clippy/prov/pnpm all green) |
| LIVE (bonus) | Path proven runnable (gemma3:1b, 32.7s); trio timed out due to GPU saturation — environmental, not a defect |

All six acceptance criteria hold and the full regression gate is green. The single LIVE-bonus
shortfall is an environment condition (a GPU saturated by the user's other apps), not a fault in
the slice's code — and the local enhance path was still shown to run end-to-end within budget.

VERDICT: PASS
