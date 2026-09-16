# S14-minimax-h3-max-constant-fields — IMPLEMENT (claim)

Author: implementer agent. Built exactly the S14 contract (AC1–AC6) on the working tree of
`forge/s14-h3-max`. No commit, no PR, no touch to `main` or `.forge/state.json`. No paid job
run (`first_light.rs` never invoked).

## Baseline recorded first (pre-change, at HEAD `9493727`)

- `cargo test --workspace`: **852 passed; 0 failed; 12 ignored** (matches the S13 release count).
  Log: `scratchpad/baseline-cargo-test.log`.
- `pnpm test`: **285 passed (23 files)**. Log: `scratchpad/baseline-pnpm-test.log`.

## Ground truth re-confirmed (design §0 MUST)

Re-read all four vendored schemas AND re-fetched `minimax/h3-max/text-to-video` keyless (HTTP 200).
They agree with design §0.1 exactly: `required: [prompt, prompt_expansion_mode]`;
`prompt_expansion_mode` string, default `balanced`, **no enum/const**; `resolution` enum
`["480P","768P","1080P"]` (UPPERCASE) default `768P`; `duration` integer min 5 / max 15 / default 5;
`aspect_ratio` six ratios on **t2v only**; `image_url`+`end_image_url` on **i2v only**. No design
amendment needed on the schema facts.

## Files changed

- `crates/hickeyfield-core/src/media.rs`
  - `FAL_ROUTE_MODES` length literal **51 → 53**; added two family-root rows
    `("minimax/h3-max", …, [Text, Image])` and `("minimax/h3-max-turbo", …, [Text, Image])`.
  - New `FAL_CONSTANT_FIELDS` table + `pub fn fal_constants(slug) -> &'static [(&str,&str)]`
    (doc-commented, incl. the offline rule). `minimax/h3` (the different model) untouched.
  - Test `fal_constants_names_the_required_field_for_h3_max`.
- `crates/hickeyfield-core/src/fal_schema.rs`
  - New `pub fn apply_constants(constants, schema: Option<&EndpointSchema>, body)` — the ~40-line
    mechanism (never overrides an existing key; with a schema writes only an `accepts`ed key; with
    no schema writes anyway). Module doc gains the 5th failure class.
  - Tests: `a_constant_is_written_when_the_schema_accepts_it`,
    `a_constant_never_overrides_a_user_value`, `a_constant_is_dropped_when_the_schema_rejects_the_key`,
    `a_constant_is_sent_when_no_schema_is_cached`, `resolution_case_matches_the_uppercase_members`.
- `src-tauri/src/app.rs`
  - One call site: `apply_constants(media::fal_constants(&route.slug), fal_schema.as_ref(), &mut body)`
    placed **after** the schema fetch (`:314-318`) and **before** the AspectPlan/sweep/reconcile
    block, guarded by `route.provider == ProviderId::Fal`.
- `crates/hickeyfield-core/examples/audit_fal.rs`
  - Check 2 now subtracts route constants (extracted to pure `unsupplied_required(schema, constants)`).
  - New check 6 (pure `constant_issues(constants, schema)`) + `BAD ROUTE CONSTANTS` section.
  - Module doc → "Six checks"; describes check 6 and the check-2 subtraction.
  - Tests: `a_required_constant_is_not_flagged_as_missing`, `a_constant_the_endpoint_rejects_is_reported`.
- `crates/hickeyfield-core/src/registry.rs`
  - Two specs (`minimax_h3_max`, `minimax_h3_max_turbo`) in `picker_only_specs`, `Modality::Video`,
    resolution enum verbatim UPPERCASE, duration `Number`, constraint notes. **`aspect_ratio` NOT on
    the spec** — see the deviation below.
  - Two `priced_routes` arms, `PerSecondTiered` tiers `(480,·)(768,·)(1080,·)`, route note ending
    "promotional launch rate (fal index 2026-09-16)". `minimax_h3` route untouched.
  - `JOB_TYPES` "video" group +2 ids. Registry size test `+30 → +32` with rewritten comment.
    New `IMPORTED_2026_09_H3: [&str; 2]` chained into both resolve/adapter loops. `IMPORTED_2026_09`
    left at 14. Picker-only-present list +2.
  - Tests: `the_h3_max_models_are_present`, `h3_max_offers_only_the_resolutions_fal_enumerates`,
    `h3_max_aspect_is_per_endpoint_not_per_spec`, `h3_max_is_priced_per_the_launch_tiers`.
- `src-tauri/src/commands.rs`
  - `resolution_height`: added `"768p" => Some(768)` (case-insensitive via the existing lowercase).
    Test `the_768p_tier_is_reachable` (asserts `"768P"`→`Some(768)` and a `{resolution:"768P"}`
    `SettingsDto` yields `Billable.height == Some(768)`).
- `crates/hickeyfield-core/src/use_case.rs`
  - Four positive rows: both ids on `TextToVideo` and `ImageToVideo`.
- `CHANGELOG.md`
  - One `### Added` bullet (H3 Max / H3 Max Turbo, priced at launch rates, 768P priced) and one
    `### Changed` bullet (required-with-default constant mechanism + audit). Existing `[Unreleased]`
    bullets left in their original wording.

## The four weakest points the launching agent named

**(1) `resolution_height` lacked `768p`.** Fixed in `src-tauri/src/commands.rs` (case-insensitive).
Proof that "768P" reaches Billable height 768:

```
$ cargo test -p hickeyfield-tauri the_768p_tier_is_reachable -- --nocapture
running 1 test
test commands::tests::the_768p_tier_is_reachable ... ok
```

The test asserts both `resolution_height("768P") == Some(768)` and `Billable::from(&{resolution:
Some("768P"), ...}).height == Some(768)`. It is non-vacuous: reverting the map line fails it, and it
also unblocks the middle price tier (`the_h3_max_models_are_priced_per_the_launch_tiers` prices 768p).

**(2) `apply_constants` ordering + offline branch.** The call sits after the schema fetch and before
AspectPlan/sweep/reconcile in `app.rs`. The mechanism is exercised in both directions by the
`fal_schema.rs` tests: `a_constant_is_written_when_the_schema_accepts_it` (preloaded/parsed schema
accepts → inserted), `a_constant_is_dropped_when_the_schema_rejects_the_key` (schema without the key →
absent), `a_constant_is_sent_when_no_schema_is_cached` (`None` → inserted, the offline branch), and
`a_constant_never_overrides_a_user_value` (pre-set value survives). Ordering is proven end-to-end by
the clean keyless audit (below): the constant is present for check 2 yet not swept by the online
unknown-key pass, and reconcile leaves it verbatim (no enum).

**(3) One spec / two endpoints (aspect on t2v only).** Proven via the `model_capabilities`-equivalent
call `ModelCapability::from_fal` for BOTH modes in `h3_max_aspect_is_per_endpoint_not_per_spec`:
text-to-video → `aspect.values == [21:9,16:9,4:3,1:1,3:4,9:16]`; image-to-video → `aspect.support ==
Support::No`. `for_route` (what `model_capabilities` calls) uses `from_fal` whenever a schema is
cached, so this is the online path the picker actually shows. **Deviation from design D3/Q2 (see
below):** to keep the four endpoints CHECKED with zero findings I did NOT put `aspect_ratio` on the
shared spec.

**(4) Bundled snapshot + `price_check`.** The bundled `crates/hickeyfield-core/vendor/
fal-catalogue-snapshot.json` (snapshot_date 2026-09-16) **does carry all four** H3 Max endpoints with
the exact prose transcribed into the tiers. `price_check` therefore classifies each as **PROSE** (not
NoSnapshot, not Mismatch), because the prose is multi-figure + resolution-conditioned which
`parse_pricing` refuses by design. Q3 answer: **present → Prose.** Full run:

```
=== MISMATCH (build-failing) (0) ===
...
=== PROSE (fal conditional/multi-figure — live-seam only) (50) ===
  minimax/h3-max-turbo/image-to-video   Video costs **$0.0125** ... **$0.02** ... **$0.04** ...
  minimax/h3-max-turbo/text-to-video    Video costs **$0.0125** ... **$0.02** ... **$0.04** ...
  minimax/h3-max/image-to-video         Video costs **$0.025** ... **$0.04** ... **$0.08** ...
  minimax/h3-max/text-to-video          Video costs **$0.025** ... **$0.04** ... **$0.08** ...
=== NO SNAPSHOT (endpoint absent from vendored index) (1) ===   (not an H3 Max row)
MISMATCH 0 / OK 11 / UNMODELLABLE 0 / PROSE 50 / UNPUBLISHED 8 / NO-SNAPSHOT 1
```

## Deviation from the design (honest, proposed as accepted)

**Design D3/Q2 put `aspect_ratio` on the shared spec (defaulting to "accept" the offline-i2v
residual). I omitted it.** With the enum on the spec, `spec.capabilities().supports_aspect` is true,
and the keyless audit's check 3 (DROPPED SETTINGS) then correctly reports both i2v endpoints:
"UI offers `aspect` but the endpoint has no `aspect_ratio`" — i.e. two of the four landed endpoints
carried a finding, which fails AC5's "the 4 endpoints CHECKED with zero findings" and the hard rule
"every landed endpoint must be CHECKED with zero findings". I confirmed this: the first audit run
(with aspect on the spec) showed DROPPED SETTINGS **23** with the two i2v rows; removing the flag
returns it to the baseline **21** and leaves the four endpoints under CHECKED only. Because the online
picker gets aspect from the per-endpoint schema (`from_fal`, proven in weak-point 3), the normal path
is unchanged: t2v still offers six ratios, i2v none. The only cost is the rare offline-t2v case, which
now falls back to fal's own 16:9 default; as a bonus the offline-i2v residual Q2 flagged disappears
(no undeclared `aspect_ratio` is ever sent). The `aspect control applies to text-to-video only`
constraint note is retained. This is the safest reading of the two conflicting hard requirements.

## Hard rules — status

- Constants written only for accepted keys online; sent-anyway offline (documented in
  `FAL_CONSTANT_FIELDS` / `apply_constants` docs). Never overrides a user-set field (test).
- audit check 2 treats constants as supplied; new check 6 catches an unaccepted key OR an
  out-of-enum value (fixture tests, both directions).
- Four endpoints CHECKED, zero findings; full before/after runs at `audit_fal-before.log`
  (CHECKED 66) and `audit_fal-after.log` (CHECKED 70, +4; UNREACHABLE 0, PINNED 0, BAD ROUTE
  CONSTANTS 0, MISSING REQUIRED 2 — both pre-existing `inworld`/`mirelo`, unchanged).
- Prices `PerSecondTiered` with "promotional launch rate (fal index 2026-09-16)" in the note.
- Duration: no invented ceiling — the spec constraint note states 5–15s, which is fal's own
  schema (`minimum:5, maximum:15`), transcribed, not invented.
- `minimax_h3` untouched (still `minimax/h3`, own tiers). `IMPORTED_2026_09` still 14.
- Test-locked names intact; provenance lint passes.

## Gate — all green, pasted

- `cargo test --workspace`: **865 passed; 0 failed; 12 ignored** (baseline 852 + 13 new tests).
  Per-binary: core lib 714, core `audit_fal` example 5, core `price_check`/other 2, tauri lib 144.
- `cargo fmt --all --check`: clean (`FMT_OK`).
- `cargo clippy --workspace --all-targets -- -D warnings`: **Finished, 0 warnings**
  (fixed one `clippy::unnecessary_map_or` → `is_none_or`).
- `./scripts/lint-provenance.py`: **PASS** (CDN hosts PASS, copy provenance PASS).
- keyless `audit_fal`: see `audit_fal-after.log` (deltas above).
- `price_check`: **MISMATCH 0** (H3 Max rows PROSE).
- `cd ui && pnpm test`: **285 passed (23 files)**. `pnpm build`: **built in ~0.9s, exit 0**.
  No `ui/` source changed → no new vitest (D6 held).

## Known limitations not fixed (not hidden)

- **N1** `prompt_expansion_mode` is not surfaced as a user control — it ships as the constant
  `"balanced"` (a future capability slice, per design N1).
- **N2** H3 Max prices are transcribed, not hermetically verified — the conditional prose is Prose by
  design; live verification is the S10 AC6 seam.
- **Offline text-to-video** no longer offers an aspect chip (falls back to fal's 16:9), a consequence
  of the deviation above. Online is unaffected.
- `docs/FIRST-LIGHT.md`, README, and `.forge/backlog.md` doc touch-ups are the DOCUMENT phase's job;
  not written here.
