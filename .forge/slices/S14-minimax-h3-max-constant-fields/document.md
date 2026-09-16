# S14-minimax-h3-max-constant-fields — DOCUMENT

## What shipped
- **Per-route constant fields**: `media::FAL_CONSTANT_FIELDS` / `fal_constants()` (compile-time
  literals) + `fal_schema::apply_constants` (writes a constant only for keys the endpoint accepts
  online; sends the literal table offline; never overrides a user-set field) + one call site in
  app.rs before the unknown-key sweep and reconcile.
- **Audit**: check 2 (MISSING REQUIRED FIELDS) subtracts route constants; new check 6 BAD ROUTE
  CONSTANTS reports a constant the endpoint refuses or a value outside its enum. Six checks total.
- **MiniMax H3 Max** (`minimax_h3_max`) and **H3 Max Turbo** (`minimax_h3_max_turbo`), text-to-video
  and animate-image, on fal with `prompt_expansion_mode: "balanced"`; resolution enum verbatim
  ("480P"/"768P"/"1080P"), duration free-form (fal enforces), `image_url` + end frame on i2v; priced
  PerSecondTiered per fal's index prose (promotional launch rates, 2026-09-16: $0.025/$0.04/$0.08 per s;
  turbo $0.0125/$0.02/$0.04). `minimax_h3` untouched. Keyless audit: CHECKED 66→70, zero findings on
  the four, BAD ROUTE CONSTANTS 0, PINNED 0, UNREACHABLE 0.
- `resolution_height` now maps 768p (case-insensitive), so the default tier prices.
- Deviation (accepted by TEST/REVIEW): `aspect_ratio` is not on the shared spec — online it is
  schema-driven per endpoint (six ratios on t2v, none on i2v); offline t2v falls back to fal's 16:9.

## Docs changed
- `CHANGELOG.md` — `[Unreleased] › Added`: H3 Max + Turbo, in original wording.
- `.forge/backlog.md` — MiniMax H3 Max constant-fields item closed; minors: misattributed doc comment
  in `examples/audit_fal.rs` (~:230-260) — fix in passing; the four H3 Max routes are PROSE in the S10
  price audit (verified by hand today; the tiered-prose parser follow-up would automate it).

## Evidence
- TEST / REVIEW / SECURITY = PASS on HEAD e953d65, zero repair rounds. cargo test --workspace 865 / 0;
  fmt · clippy -D warnings · provenance clean; pnpm test 285 / build ok; audit logs before/after in
  this directory; live schemas in `schemas/`. Nothing spent; no key.
