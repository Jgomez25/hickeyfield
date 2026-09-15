# S5-enhancer-model-curation — RELEASE

**Gate:** `python3 .forge/gate.py` → exit 0. TEST + REVIEW + SECURITY = PASS.

**What shipped:** enhancer local-model curation. `model_tier` classifies installed Ollama
models (Recommended: phi4-mini/llama3.2/gemma3 families; Discouraged: vision, reasoning
`-r1`/`qwq`, >=20B; else Neutral). `parse_local_models` returns tier-annotated,
stable-sorted `Vec<LocalModel>`; Rust is the single source of truth, UI consumes `[0]`
untouched (show==submit). Picker badges tiers (`data-tier` + label suffix), all selectable.
Zero-models hint genericized.

**Regression:** cargo test --workspace 761 pass / 0 fail; fmt · clippy · provenance clean;
pnpm test 224 pass; pnpm build ok; tauri build + launch at install. cargo-deny baseline
h2/rustls only (no manifest change).

**Live:** gemma3:1b real-corpus rewrite 32.7s (within 120s). Under heavy GPU load the larger
recommended models hit the wall — environmental; honest fallback catches it.

**Companion housekeeping (done, not code):** Ollama pruned to the 4 go-to models
(gemma3n:e2b, phi4-mini, llama3.2:3b, gemma3:1b). SparkLLM/Spark-X2.5-1.7B was tried and
removed — unloadable (`unknown model architecture: spark2_5` on Ollama 0.34.0).

**Follow-ups logged:** F6 adaptive timeout/lighter corpus under load; F7 probe-on-select for
unloadable models.

**Landing:** commit on `forge/s5-model-curation`, fast-forward to `main` (local, no push),
rebuilt + reinstalled to /Applications.

**Status:** released (local main).
