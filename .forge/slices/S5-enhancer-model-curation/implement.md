# S5-enhancer-model-curation — IMPLEMENT CLAIM

Branch: `forge/s5-model-curation`. Built to the approved design. Single source of
truth for model suitability lives in Rust (`local_models` returns a tier-annotated,
already-sorted `Vec<LocalModel>`); the UI consumes it untouched and takes `[0]`, so
show == submit by construction.

## Files changed

### Rust
- `crates/hickeyfield-core/src/enhancer.rs`
  - Added `enum ModelTier { Recommended, Neutral, Discouraged }` (serde `rename_all="lowercase"`) and `struct LocalModel { name, tier }` (serde camelCase). — AC1/AC2
  - Added pure `model_tier(name, param_billions: Option<f64>) -> ModelTier` with Discouraged-first precedence (vision `-vl`/`llava`/`vision`; reasoning `deepseek-r1`/`qwq`/`-r1`; size `>=20.0`B from declared `param_billions` else a name scan), then Recommended (family before `:` starts_with `phi4-mini`|`llama3.2`|`gemma3`), else Neutral. — AC1
  - Added `parse_param_billions(&str) -> Option<f64>` (`"3.2B"`→3.2, name-token scan `\d+(\.\d+)?b`, keeps the largest match so `qwen3-coder:30b` reads 30 not 3). — AC1/AC2
  - Rewrote `parse_local_models` (made `pub` so the command layer can test the raw→parse→select chain): keeps the embedding-only filter, reads `m["details"]["parameter_size"]`, maps to `LocalModel`, stable-sorts by tier rank (Rec=0/Neu=1/Disc=2) preserving Ollama order within a tier. `local_models` now returns `Result<Vec<LocalModel>, String>`. Added `tier_rank` helper. — AC2
  - Softened the `LocalEnhancer.model` doc comment and the `HostedEnhancer` doc comment to the ranks-and-recommends stance (local path ranks/recommends but still runs any picked model; hosted still names none because ids aren't enumerable). — AC5
  - Tests: updated the two parse tests to the `LocalModel` shape (assert names + tier); added `the_curated_go_to_families_are_recommended`, `vision_and_reasoning_models_are_discouraged`, `large_models_are_discouraged_by_size_from_details_or_name`, `an_ordinary_chat_model_is_neutral`, `parse_param_billions_reads_a_declared_size`, `the_installed_list_is_stable_sorted_by_tier_reading_parameter_size`; fixed the ignored live-daemon test to match on `.name`. — AC1/AC2

- `src-tauri/src/commands.rs`
  - `list_ollama_models` → `Vec<LocalModel>`. — AC2
  - `select_rewriter`: param `ollama_models: &[LocalModel]`; auto branch `.first()` → `Rewriter::Ollama { model: &first.name }`; explicit membership → `m.name == model`. `submit_job` probe unchanged (type flows through). — AC2/AC3
  - Genericized `NOTE_OLLAMA_NO_MODELS` (recommends llama3.2/phi4-mini/gemma3 generically, keeps the mandatory "...sent exactly as you wrote it." ending). — AC5
  - Tests: added a `models(&[&str])` fixture helper (tiers each name via `model_tier`); converted fixtures to `Vec<LocalModel>`; renamed the first-model test → `auto_picks_the_top_ranked_model` (raw `/api/tags` → `parse_local_models` → `select_rewriter`, proving a recommended model listed *after* a vision model is picked); added `auto_with_only_discouraged_models_still_picks_one`; updated `a_list_of_installed_models_never_errors` to `Vec<LocalModel>`. — AC3

### UI
- `ui/src/types.ts` — added `type ModelTier` + `interface LocalModel`. — AC2
- `ui/src/api.ts` — `ollamaModels(): Promise<LocalModel[]>` (catch→`[]`). — AC2
- `ui/src/App.tsx` — state `useState<LocalModel[]>`, import `LocalModel`. — AC2
- `ui/src/components/SettingsRail.tsx` — prop `ollamaModels: LocalModel[]`, import `LocalModel`. — AC2
- `ui/src/components/EnhancerPicker.tsx` — `firstModel = ollamaModels[0]?.name ?? ""`; prop type `LocalModel[]`; list passed through UNTOUCHED (no re-sort/filter); `<option>` map adds `data-tier={m.tier}` + `tierSuffix()` label (`(recommended)` / `(may be slow / not ideal here)` / bare); genericized the zero-models reason string; softened the doc comment. mount-commit + pickBackend fallback kept. — AC3/AC4/AC5
- `ui/src/components/EnhancerPicker.test.tsx` — fixtures → `LocalModel[]` (`RANKED` recommended-first); auto-commits the recommended model on mount; commits a discouraged model when it is the only one; renders in given order (no re-sort); badges recommended / warns discouraged asserted via `data-tier`, every option selectable, discouraged still selectable. — AC4

## Verification (real output)

### AC6 — cargo test --workspace (0 failed, >=754)
```
$ cargo test --workspace 2>&1 | grep "test result:" | awk '{p+=$4;f+=$6;i+=$8} END {print "passed="p" failed="f" ignored="i}'
passed=761 failed=0 ignored=10
```
(baseline 754; +7 new Rust tests.)

### AC6 — cargo fmt --all --check
```
$ cargo fmt --all --check
FMT OK   (exit 0, no diff)
```

### AC6 — cargo clippy --workspace --all-targets -- -D warnings
```
    Checking hickeyfield-core v0.1.0 ...
    Checking hickeyfield-tauri v0.1.0 ...
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.72s
```
(exit 0, no warnings.)

### AC5/AC6 — ./scripts/lint-provenance.py
```
CDN hostnames
  PASS  no third-party media hosts referenced
Copy provenance
  PASS  no shipped string matches the reference corpus (80015 shingles indexed)
provenance exit: 0
```

### AC4/AC6 — cd ui && pnpm test
```
 ✓ src/components/EnhancerPicker.test.tsx (8 tests) 99ms
 ...
 Test Files  12 passed (12)
      Tests  224 passed (224)
```

### AC6 — cd ui && pnpm build
```
> tsc --noEmit && vite build
✓ 74 modules transformed.
✓ built in 835ms
```
(tsc --noEmit passed → all TS call sites consistent with the new `LocalModel[]` shape.)

## Deviations / notes
- **`parse_local_models` made `pub`** (design had it private). AC3 requires an
  end-to-end raw-`/api/tags`→parse→select test *in commands.rs*, which is impossible
  without either a public parse seam or duplicating the sort logic in the test. Made
  it `pub` with a doc note; it is the honest, minimal seam and the command test now
  exercises the real ranking through it.
- Design listed doc comment `enhancer.rs:1083` (the `HostedEnhancer` "no default
  model" comment) for the softened stance. That comment is about *hosted* ids, which
  genuinely remain un-recommended (rosters aren't enumerable). Rather than make it
  say something false, I clarified it to note the local path now ranks/recommends
  while hosted still names none — honoring the softened-stance intent truthfully.
- `tauri build` and the live Ollama generation were NOT run (per instructions — the
  ignored live test `a_local_rewrite_completes_against_a_real_daemon` is left for the
  TEST phase).

## Known limitations (not fixed, by design scope)
- Tiering is heuristic on the tag name + declared size; a discouraged model is a
  warning, never a ban — auto still runs the top of the list even when only
  discouraged models are installed (proven by `auto_with_only_discouraged_models_still_picks_one`).
- The 20B size threshold is a fixed constant, not tuned per-hardware.
