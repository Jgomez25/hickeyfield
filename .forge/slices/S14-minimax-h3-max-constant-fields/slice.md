# S14-minimax-h3-max-constant-fields — SLICE (DISCOVER + PLAN)

## Delivers (one sentence, end-to-end)
MiniMax H3 Max and H3 Max Turbo (text-to-video and animate-image) appear in the picker and generate
on fal, because routes can now declare constant body fields the provider requires (here
`prompt_expansion_mode`), every such field is verified by the keyless audit, and the four endpoints
pass that audit with zero findings before landing.

## Ground truth (verified keyless 2026-09-16; re-fetch into `schemas/` in DESIGN)
- `minimax/h3-max/{text-to-video,image-to-video}` and `minimax/h3-max-turbo/{text-to-video,image-to-video}`:
  `required: [prompt, prompt_expansion_mode]`; `prompt_expansion_mode` string, default "balanced"
  (fal lists a default but still REQUIRES the key → the app must send it); `duration` integer default 5
  (free-form, fal enforces; documented 5–15s → constraint note, no invented enum); `resolution` enum
  ["480P","768P","1080P"] default "768P" (UPPERCASE — send these exact strings); t2v has `aspect_ratio`
  enum [21:9,16:9,4:3,1:1,3:4,9:16]; i2v has `image_url` (binder-compatible) + nullable `end_image_url`;
  `seed`, `sync_mode`, `enable_safety_checker`.
- S8 refused H3 Max solely for the missing constant-fields mechanism (S8/implement.md §8.1). The
  registry's `minimax_h3` (fal `minimax/h3`, PerSecondTiered after S10) is a DIFFERENT model — add
  alongside, never re-point.
- Body assembly `src-tauri/src/app.rs` (~:277-351, reconcile ~:438); routes in `registry.rs::priced_routes`;
  `fal_schema::reconcile` (S9) knows `required`; `audit_fal` check 2 "MISSING REQUIRED FIELDS" fires on
  exactly this case today.
- Price: fal index prose for `minimax/h3-max*` in `vendor/fal-catalogue-snapshot.json`; S10 `prices::audit`
  checks it if it parses.

## Acceptance criteria (testable)
- [ ] AC1 — Mechanism: `Route` gains `constants: &[(&str, &str)]` (or equivalent) written verbatim into
  the body before reconcile, only when the endpoint schema `accepts` the key; a constant never overrides
  a user-set field of the same name; unit tests (present; not duplicated; dropped when unaccepted).
- [ ] AC2 — Audit: check 2 treats a route constant as supplied; a NEW check reports a constant the endpoint
  does not accept or whose value is outside its enum/const. Fixture tests.
- [ ] AC3 — Land `minimax_h3_max` and `minimax_h3_max_turbo` (t2v + i2v via FAL_ROUTE_MODES Text+Image),
  constant `prompt_expansion_mode: "balanced"`, resolution enum verbatim (uppercase), aspect enum on t2v
  only, duration free-form + "5–15s" note, priced per fal prose (tiered if tiered), JOB_TYPES/specs/use_case
  tabs/counts bumped (FAL_ROUTE_MODES 51→53, import list, size literal), audit clean.
- [ ] AC4 — Resolution case: the app sends "768P"/"1080P" exactly (reconcile coerces "1080p"→"1080P" or the
  chip offers the members directly); test.
- [ ] AC5 — Keyless `audit_fal` full run recorded: UNREACHABLE 0, the 4 endpoints CHECKED with zero
  findings, MISSING REQUIRED FIELDS unchanged elsewhere, PINNED 0.
- [ ] AC6 (regression) — cargo test --workspace (>= S13 count), fmt, clippy -D warnings, provenance,
  `cd ui && pnpm test && pnpm build`, CHANGELOG, app builds + launches. No paid job.

## Approach + why now
Owner asked for H3 Max explicitly (S8 headline, still missing). ~40-line mechanism unlocks any future
"required-with-default" provider field; landing follows the S8/S9 import recipe.
