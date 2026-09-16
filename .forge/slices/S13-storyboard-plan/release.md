# S13-storyboard-plan — RELEASE
**Gate:** `python3 .forge/gate.py S13-storyboard-plan` exit 0. TEST + REVIEW + SECURITY = PASS on
HEAD 7250335 (Opus 4.8 verifiers; two repair rounds used — the maximum).
**Shipped:** brief → validated, model-aware shot list via the enhancer backend; editable storyboard
with per-shot preview and summed estimate; generate-all as one linked set through the unchanged
single-job money path; partial-failure report + retry-failed into the same set; migration 8;
collision-proof job ids (pre-existing overwrite hazard fixed).
**Regression:** cargo test --workspace 852 / 0; fmt · clippy -D warnings · provenance clean; pnpm test
285 / build ok; keyless audit_fal PINNED 0 / UNREACHABLE 0; tauri build + launch at install.
**Money:** nothing spent; no hosted call; Ollama draft local and free.
**Owner-visible:** `storyboards` table name (contract said `job_sets`); shot failure = best-effort
continue. Migration 8 runs on first launch of this build.
**Landing:** commits on forge/s13-storyboard-plan, ff to main (local fork only, never upstream),
`tauri build --bundles app`, ad-hoc codesign, reinstalled to /Applications.
**Status:** released (local main).
