# S8-fal-catalogue-sync — DOCUMENT

## What shipped
- `examples/fal_diff.rs` — keyless fal→registry diff tool: refreshes `vendor/fal-catalogue-snapshot.json`
  from fal's unauthenticated index (`--offline` reuses the snapshot), prints per-use-case the fal
  models the registry lacks, with title/family/price. Under test (`[[example]] test = true`).
- Snapshot refreshed 2026-09-16 (1,491 models).
- 11 verified fal frontier models, each probed keyless by `examples/audit_fal` before landing
  (UNREACHABLE 0, CHECKED 62, no finding on any of them): Seedance 2.5 (promoted from placeholder;
  native 4–30s, text-to-video only for now), Kling 3.0 Pro (t2v), Wan 3.0 Prime (t2v), GPT Image 2.5
  Flare + Sunburst (t2i + edit), Qwen Image 3, Nano Banana Pro edit, Nano Banana 2 edit, Seedream 5
  Pro edit.
- Withdrawn under AC3's own rule (probed, unrunnable, parser blind spot): LTX 2.5, LTX 2.5 Animate,
  Veo 3.1 Extend, and Seedance 2.5's image-to-video mode. MiniMax H3 Max not landed (fal requires
  `prompt_expansion_mode` with no default). All converge on backlog slice
  `fal-schema-const-and-int-enums`.

## Docs changed
- `CHANGELOG.md` — `[Unreleased] › Added`: the new models and the native 30s clip, with the
  Seedance text-to-video qualification.
- `docs/FIRST-LIGHT.md` — stale "input-mode suffix" section corrected: the resolver landed in
  `ab06e31` (2026-08-05) and `8d8ecfb` (2026-08-28).
- `.forge/backlog.md` — #4 bullet 1 marked resolved; new slice `fal-schema-const-and-int-enums`
  and the S8 minors logged.
- README: N/A (catalogue provenance paragraph still accurate).

## Evidence
- TEST / REVIEW / SECURITY = PASS (test.md RE-TEST 3, review.md RE-REVIEW 3, HEAD 55aa32f).
- cargo test --workspace 779 pass / 0 fail; fmt · clippy -D warnings · provenance clean;
  pnpm test 232 pass; pnpm build ok; keyless audit_fal log in this directory.
- Mutation-checked: restoring Seedance's Image mode or the withdrawn counts fails a test.

## Follow-ups logged (non-blocking, in backlog.md)
- `fal-schema-const-and-int-enums` (re-land the 3 + Seedance i2v; re-probe all routes).
- MiniMax H3 Max constant-fields mechanism; kling3_0 / kling3_0_pro / wan3_0_prime i2v
  `start_image_url` binder key.
- Minors: `nano_banana_2_edit` default 0.5K vs 1K price; `fal_diff --dump --offline` restamps;
  snapshot `thumbnailUrl` must stay unrendered; media.rs:627 comment generalisation loose;
  ~103-col doc line registry.rs:1538.
