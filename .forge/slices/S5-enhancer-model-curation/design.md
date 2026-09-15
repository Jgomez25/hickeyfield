# S5-enhancer-model-curation — DESIGN

Design approved by the owner in plan mode (see `/Users/jorge/.claude/plans/let-me-import-some-ticklish-crescent.md`, Part 2). Condensed here as the slice's design of record. All file:line verified against current code.

## Single source of truth: tiering + ordering live in Rust
The UI consumes a tier-annotated, **already-sorted** list and never re-sorts — this makes UI auto-pick and Rust auto-pick agree by construction (guards the show==submit invariant a prior S4 review bug exposed).

### `crates/hickeyfield-core/src/enhancer.rs`
- `enum ModelTier { Recommended, Neutral, Discouraged }` + `struct LocalModel { name: String, tier: ModelTier }` (serde: ModelTier `rename_all="lowercase"`, LocalModel `camelCase`).
- `fn model_tier(name: &str, param_billions: Option<f64>) -> ModelTier` — pure. Precedence (Discouraged FIRST so it wins ties and catches small vision/reasoning models + gemma3:27b before the gemma3 recommend rule):
  1. Discouraged: lowercased name contains `-vl`|`llava`|`vision`; or `deepseek-r1`|`qwq`|`-r1`; or size `>=20.0`B (from `details.parameter_size` when present, else scan the tag for `\d+(\.\d+)?b`).
  2. Recommended: family (before first `:`) starts_with `phi4-mini`|`llama3.2`|`gemma3` (covers `gemma3n`).
  3. Neutral: otherwise.
- `fn parse_param_billions(&str) -> Option<f64>` helper (`"3.2B"`→3.2).
- Rewrite `parse_local_models` (enhancer.rs:1027): keep embedding-only filter (:1033-1036), read `m["details"]["parameter_size"]`, map to `LocalModel`, **stable-sort by tier rank** (Rec=0/Neu=1/Disc=2) preserving per-tier Ollama order. Return `Vec<LocalModel>`. `local_models` (:1020) return type follows.
- Update "names no default model" doc comments (enhancer.rs:847-849, 1083; and EnhancerPicker.tsx:13-15) to the softened stance: ranks + recommends, still runs any model the user picks.

## Command return-shape change (riskiest — all land together)
`list_ollama_models` (commands.rs:800) → `Vec<LocalModel>`.
- `select_rewriter` (commands.rs ~:732-793): signature `ollama_models: &[LocalModel]`; auto branch `.first()` → bind `Rewriter::Ollama { model: &first.name }`; explicit membership (:765) → `m.name == model`. `submit_job` probe (:831-838) passes the new type through unchanged.
- TS: `ui/src/types.ts` add `ModelTier` + `LocalModel`; `ui/src/api.ts:248` `ollamaModels(): Promise<LocalModel[]>` (catch→`[]`); `ui/src/App.tsx:130` state `LocalModel[]`; `ui/src/components/SettingsRail.tsx:104` prop.

## Picker warn (`ui/src/components/EnhancerPicker.tsx`)
- `firstModel = ollamaModels[0]?.name ?? ""` (pre-sorted → top = best tier); keep mount-commit (:53) + pickBackend fallback (:90-91). **Pass the list through untouched — no re-sort/filter.**
- `<option>` map (:124): label suffix by tier (recommended `(recommended)`, discouraged `(may be slow / not ideal here)`, neutral bare) + `data-tier={m.tier}`. All options enabled.

## Genericized zero-models copy (provenance-safe)
`NOTE_OLLAMA_NO_MODELS` (commands.rs:721) and `EnhancerPicker.tsx:70`: recommend the go-to set generically (llama3.2/phi4-mini/gemma3 as examples), keep the Rust note's mandatory closing "...sent exactly as you wrote it." (asserted commands.rs:1429-1431). Run lint-provenance.py.

## Tests
Per slice.md AC1–AC6. Update existing fixtures: enhancer.rs parse tests (:1937-1958) → LocalModel shape; commands.rs select_rewriter tests (:1392-1421) → Vec<LocalModel>, and `auto_..._picks_the_first_installed_model` → `auto_picks_the_top_ranked_model` with a fixture whose input leads with a non-recommended entry; EnhancerPicker.test.tsx fixtures → LocalModel[].

## Context clamp: unchanged
`[8192,32768]` (enhancer.rs:882-887) is model-agnostic; safe for all kept models. No change.

## Riskiest part
The `Vec<String>`→`Vec<LocalModel>` shape change across core fn + command + submit_job probe + select_rewriter signature + TS type + App state + 2 component props + 2 test suites, atomically. A missed call site fails to compile; a UI re-sort/stale-string path would resurrect show≠submit. Mitigation: Rust pre-sorts once; UI takes `[0]` untouched.
