# S9-fal-schema-const-and-int-enums — REVIEW (evidence)

Independent, read-only, production-standards review of `git diff main...HEAD` (HEAD 71aea29).
Scope: `fal_schema.rs`, `capability.rs`, `examples/audit_fal.rs`, `registry.rs`, `media.rs`,
`use_case.rs`, `src-tauri/src/app.rs`, `Cargo.toml`, `CHANGELOG.md`, `backlog.md`. No source
was modified. test.md / security.md are written concurrently by others and were not read.

## Gate re-run in this session (dummy keys; keyless audit; never first_light)

- `cargo test --workspace` — summed `test result: ok` counts = **801 passed, 0 failed** (baseline 779). Matches implement.md.
- `cargo clippy --workspace --all-targets -- -D warnings` — clean.
- `cargo fmt --all --check` — clean.
- `./scripts/lint-provenance.py` — PASS (CDN + copy provenance).
- `cargo run -q -p hickeyfield-core --example fal_diff -- --offline` — "Registry reaches 66".
- `env -u hickeyfield_FAL_KEY -u FAL_KEY cargo run -q ... --example audit_fal` (**live, keyless**) —
  `11 / 2 / 21 / 9`, PINNED **0**, UNREACHABLE **0**, CHECKED **66**. No re-landed id
  (`ltx_2_5|veo3_1_extend|seedance_2_5`) appears in any finding section. Reproduces implement.md exactly.
- Not run: `scripts/build-macos.sh` (no signing identity / no display) and `first_light.rs` (spends money).
  App-launch half of AC7 is unproven but no `ui/` or startup path changed; `cargo build --workspace` +
  `pnpm build` both link. Accepted as a documented limitation, not a finding.

## What I checked and found clean

- **Parser (AC1), `fal_schema.rs:289-352`.** `const`-first, scalar-only, `continue` after a pin;
  enum pass is all-or-nothing with `null` skipped-not-voiding; `anyOf`/`oneOf` not descended.
  Reuses the existing `scalar_text` so both sides spell `6.0`→`"6"` identically. Six struct-literal
  fixtures updated with `pinned: Default::default()`; `#[serde(default)]` on the new field. Ten
  fixture tests, all copied from real documents. Correct.
- **Enum transcription (d, e).** Verified every re-landed spec value-for-value against the checked-in
  `schemas/*.json`: LTX t2v/i2v `duration [6,8,10,auto]` + `resolution [720p,1080p]`; Veo `duration
  const 7s`, `resolution const 720p`; Seedance i2v `aspect_ratio const auto`. **No invented value.**
  The LTX aspect divergence (t2v `[16:9,9:16]` vs i2v `[auto,16:9,9:16]`) is handled by declaring
  *no* aspect flag on either LTX spec and letting the per-endpoint schema decide — the correct fix,
  not a fabricated union.
- **reconcile / Pinned (AC2), `fal_schema.rs:184-250`.** Pin pass before enum pass (type contract
  holds); skip-if-already-equal via `coerce` (so bare `7`↔`"7s"` reports nothing); Class-B
  (`prompt` + media keys) refused not rewritten; Class-C (absent key) left alone. Matches D2.
- **Result<Vec<Pinned>, Refusal> compatibility (c).** Only one production caller,
  `app.rs:438`, which maps the error and consumes the vec; all other call sites are tests using
  `.unwrap()/.expect()/.expect_err()`. Compiles and passes. `app.rs` does **not** drop the vec on
  the floor — it pushes a per-pin advisory (design §12.5's concern is closed).
- **Cost models (f/e).** `PerSecondTiered {(720,0.12),(1080,0.17)}` picks `height <= max_h` then
  falls to the last tier (720p→0.12, 1080p→0.17, 4K→0.17 but 4K is unselectable). Veo
  `per_second_audio(0.20, 2.0)` × pinned 7s = $1.40 / $2.80 — fal's published pair; asserted by
  `the_extend_endpoint_is_priced_at_its_fixed_seven_seconds`. Prices match the recorded fal values.
- **Audit check 4/5 (AC3), `audit_fal.rs`.** Duplicate network `schema_enum` deleted; check 4 reads
  the already-fetched `schema.enums`; `pin_conflicts` is pure/network-free with 3 fixture tests, run
  in-workspace via the new `[[example]] test = true`. Correct.
- **Capability honesty (AC6).** Zero production edits; pin recorded in `enums` makes `fal_axis`
  return `supported([P])` and `is_free_form()` false. `one_spec_two_endpoints_...` proves the pin is
  per-endpoint (i2v `[auto]`, t2v 7 values). `a_catalogue_default_outside_a_pin_is_dropped` guards
  the 1080p→720p case.
- **Re-land counts (AC5).** `FAL_ROUTE_MODES` 48→51, `IMPORTED_2026_09` 11→14, registry size +27→+30,
  JOB_TYPES outer length still 16, `use_case` negative flipped to positive. All array-length literals
  compile and their guarding tests pass. Veo `("video", true, Media)` binds to wire key `video_url`
  (media.rs role map), matching `required ['prompt','video_url']`.
- **Media binding (b, video key).** `MediaRole::Video` catalogue key `video` → fal key `video_url`;
  correct for the extend endpoint.
- **Docs.** CHANGELOG `### Fixed` bullet added and the stale S8 "text-to-video for now" qualifier
  removed; module docs ("Three bugs"→fourth, "Four checks"→"Five"); `picker_only_specs`, `media.rs`
  and `use_case.rs` withdrawal comments rewritten (not left contradicting code); backlog entry marked
  DONE. No stale count or comment-vs-code contradiction found.
- **AI-code tells.** No invented APIs; `scalar_text` reused rather than re-implemented; deleted
  fetcher removes duplication; no copy-paste divergence between the two LTX specs (they intentionally
  differ only where fal differs). Clean.

## Findings

### Minor 1 — estimate is priced from pre-reconcile settings, so a Class-A pin coercion can under-quote (money)
`src-tauri/src/commands.rs:1013` computes `estimated_usd` from `billable` (settings) **before**
`app.rs:438` calls `reconcile`, which may coerce a pinned field. For a stale body carrying
`duration: 5` on `veo3_1_extend`, the job would show an advisory saying "renders at 7s … your 5
does not apply" while `estimated_usd` was priced at 5s (a ~$0.40, or $0.80 audio, under-quote).
Standard violated: money honesty (estimate should reflect what will actually be billed).
Why not blocking: the only price-affecting pin among the landed models is Veo `duration`, and the UI
clamps it to the single-member enum `[7s]` on capability change, so the normal path always estimates
at 7s and the divergence is effectively unreachable (Veo is newly landed, no stale reruns exist).
This is a pre-existing property of the reconcile-after-estimate ordering (enum coercion already
mutates values post-estimate), not introduced here, and pricing is the explicitly-next slice.
Concrete fix (recommend as a follow-up, not this slice): recompute `estimated_usd` from the
reconciled body, or when `reconcile` returns a `Pinned` touching a billed field, re-price and flag.
This aligns with the prior security pass's Medium rating — real, worth tracking, but not a blocker
for S9 given the clamp makes it unreachable for the landed set.

### Minor 2 — the pin advisory string is not covered by a test
`src-tauri/src/app.rs:441-453` formats and pushes the per-pin advisory; implement.md §5.6 concedes
it is compile-checked only (`submit` is not unit-testable without a live route). `reconcile`
returning the rows *is* tested. Standard: tests on user-facing output. Non-blocking (the honesty
mechanism — the `Pinned` rows — is tested; only the sentence is not). Recommend a small unit test on
the format string, or extracting the sentence builder into a testable helper.

## Answers to the posed questions

- **(a) Seedance narrowing vs design §6.4.** The implementer's D-1 deviation (spec `aspect_ratio`
  narrowed to `["auto"]`) is the right call and does **not** regress AC6. The pinned axis (i2v) is
  honoured; online t2v still shows all seven ratios (per-endpoint schema, proven by
  `one_spec_two_endpoints_...`). The only cost is that the *offline* t2v fallback collapses to
  `auto` — a safe reduction (auto is valid on t2v), strictly better than design §6.4's 7-member
  list which would 422 the animate half offline and leave a re-landed id in a finding section. The
  rule "the value valid on every endpoint this model serves" is stated in code and honest.
- **(b) money honesty of coercing a Class-A pin.** See Minor 1. Real but unreachable for the landed
  set because the pinned billed field (Veo duration) is UI-clamped to the pin. Not a blocker for S9;
  recommend fixing in the pricing slice.
- **(c) signature compatibility.** `Result<Vec<Pinned>, Refusal>` is source-compatible; the single
  production caller handles `Err` via `map_err(...)?` and consumes the vec. Clean.
- **(d) transcribed enums.** Match `schemas/*.json` value-for-value; the LTX aspect divergence was
  handled without inventing a value (no aspect flag declared on either LTX spec).
- **(e) cost models.** Match recorded fal prices ($0.12/$0.17 tiered for LTX; 7×$0.20/$0.40 for Veo).
- **(f) dead code / invented APIs / stale counts.** None found; duplicate fetcher deleted, all
  counts and comments updated consistently.

Both findings are Minor and neither is on a critical path (the money divergence is unreachable in the
shipped UI flow for the landed models). No blocker or major.

VERDICT: PASS
