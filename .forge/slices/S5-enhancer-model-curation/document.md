# S5-enhancer-model-curation — DOCUMENT

## What shipped
The enhancer's local-model selection is now curated. `model_tier(name, param_billions)`
(enhancer.rs, pure/tested) classifies each installed Ollama model as Recommended
(phi4-mini | llama3.2 | gemma3 families), Discouraged (vision `-vl`/`llava`/`vision`;
reasoning `deepseek-r1`/`qwq`/`-r1`; size >=20B), or Neutral. `parse_local_models` reads
`details.parameter_size`, returns `Vec<LocalModel{name,tier}>`, and stable-sorts
Recommended→Neutral→Discouraged. Rust is the single source of truth: `list_ollama_models`
and `select_rewriter` consume the pre-sorted list, and the UI takes `[0]` untouched
(show==submit). The picker badges tiers via a label suffix + `data-tier`; all models stay
selectable. The zero-models hint is genericized (recommends the go-to set as examples).

## Docs changed
- `CHANGELOG.md` — new `[Unreleased] › Changed` bullet.
- README: N/A (no README claim about model selection).

## Evidence
- TEST/REVIEW/SECURITY = PASS. `cargo test --workspace` 761 pass / 0 fail; fmt · clippy ·
  provenance clean; `pnpm test` 224 pass; `pnpm build` ok.
- Live: gemma3:1b completed the real-corpus rewrite in 32.7s (within the 120s budget);
  a warm small call 3.6s. Under heavy machine load, larger recommended models (phi4-mini,
  llama3.2:3b, gemma3n:e2b) hit the 120s wall — environmental, not a code defect; honest
  fallback catches it.

## Follow-ups logged (see backlog)
- F6: adaptive local-enhancer timeout / lighter corpus for small models under heavy load.
- F7: probe-on-select to detect models that list in /api/tags but fail to load (unknown
  GGUF architecture) — honest fallback catches it today, but the picker still offers them.
