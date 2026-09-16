# S14-minimax-h3-max-constant-fields — REVIEW (evidence)

Independent, read-only production-standards review of the slice diff `git diff main...HEAD`
(HEAD `e953d65`). No source, docs, or backlog changed by this review. Scope: only what this
slice touched (media.rs, fal_schema.rs, app.rs, audit_fal.rs, registry.rs, use_case.rs,
commands.rs, CHANGELOG). Every fal fact below was re-derived first-hand from the four
vendored `schemas/*.json` and from a live keyless audit run this session, not taken on the
implementer's word.

## Gate re-run this session (all green)

- `cargo test --workspace`: exit 0, 0 failed. Core lib now 724 tests (was 714 → +10:
  media 1, fal_schema 5, registry 4 — matches the claim; the `apply_constants` filter that
  reads "0 passed" is a name-prefix artifact, the four `a_constant_*` tests pass by full name).
  Tauri lib 144 passed. `audit_fal` example 5 passed. Each new test verified by name and each
  is non-vacuous (e.g. `unsupplied_required(&schema, &[])` still reports `prompt_expansion_mode`).
- `cargo fmt --all --check`: clean. `cargo clippy --workspace --all-targets -- -D warnings`:
  Finished, 0 warnings.
- `./scripts/lint-provenance.py`: PASS (CDN hosts, copy provenance).
- `cd ui && pnpm test`: 285 passed (23 files). `pnpm build`: built, exit 0. No `ui/` source
  in the diff (D6 held).
- Keyless `audit_fal` **re-run live this session** (`scratchpad/audit_live.txt`, exit 0):
  CHECKED **70** (before.log 66, +4), UNREACHABLE 0, PINNED 0, **BAD ROUTE CONSTANTS 0**,
  MISSING REQUIRED 2 (the pre-existing `inworld`/`mirelo`, not H3 Max), and the pre-existing
  SILENTLY-IGNORED 11 / DROPPED 21 / REJECTED 9 are byte-for-byte the before.log baseline —
  no regression. The four H3 Max endpoints (`minimax/h3-max{,-turbo}/{text,image}-to-video`)
  appear **only** under CHECKED, in zero findings sections. AC5 holds against the live API,
  not just the recorded log.

## What I checked and found clean

- **(d) Transcription fidelity, spec vs `schemas/*.json`, value for value.** Extracted every
  input-schema property from all four vendored schemas and confirmed against `registry.rs`:
  `required: [prompt, prompt_expansion_mode]` (all four); `resolution` enum
  `["480P","768P","1080P"]` UPPERCASE default `768P` → `en(&["480P","768P","1080P"])` verbatim
  (registry.rs:1859); `duration` integer min 5 / max 15 / default 5 → `ValueSpec::Number`
  free-form with a "5-15 seconds (default 5)" constraint note, no invented enum ladder;
  `prompt_expansion_mode` string, default `balanced`, **no enum/const** → sent as the constant
  `"balanced"` (media.rs:783). Media roles: i2v declares `image_url` + nullable `end_image_url`;
  the spec's `start_image`/`end_image` Media flags bind these, which the use_case.rs:314-318
  test asserts for both modes. t2v has `aspect_ratio` (six ratios) and no image keys. All
  correct.
- **(a) Home and shape of the constants table.** `FAL_CONSTANT_FIELDS` in media.rs keyed by
  family-root slug mirrors the existing `FAL_ROUTE_MODES`/`FAL_MISSING_ROUTES` pattern in the
  same file; the pure writer lives in fal_schema.rs and the single call site in app.rs. Not on
  `Route` — correct, `Route` is provider-agnostic and these keys are fal-schema-specific. No
  duplication with S9's `Pinned`: `Pinned` reports a `const` the endpoint *fixes* and the app
  would have overridden; a constant *supplies* a required-with-default key the app has no
  control for. Orthogonal concerns.
- **(b) `apply_constants` semantics and ordering.** Logic is correct: skips a key already in
  the body (never overrides user/binder), and with a schema writes only an `accepts`ed key,
  else (offline) writes regardless. Ordering in app.rs:324-336 is sound: the call runs after
  the schema fetch and before the unknown-key sweep. Online, the key is written only if
  `accepts` is true, so the sweep (which drops `!accepts` keys) cannot drop it. Offline the
  sweep block is `if let Some(schema)` — it does not run at all, so the constant survives. I
  traced both paths in `submit_to_provider`; the sweep cannot strip a constant in either case.
  The one real risk — the offline branch sending a key a *new* endpoint would 422 on — is
  called out explicitly in both the `FAL_CONSTANT_FIELDS` and `apply_constants` doc comments
  ("revisit the offline rule before adding a constant for a family whose modes disagree"), and
  check 6 catches any mismatch whenever a schema is present. Acceptable and documented.
- **(c) The aspect deviation (design D3/Q2, S9 Seedance precedent).** I confirmed the forced
  choice is real: the t2v schema enumerates six ratios, the i2v schema has no `aspect_ratio`
  key at all. Putting the enum on the shared spec makes `supports_aspect` true, which audit
  check 3 correctly reports as a DROPPED SETTINGS finding on the two i2v endpoints — failing
  the "four endpoints CHECKED with zero findings" rule. Omitting it keeps the online picker
  correct (it reads per-endpoint `from_fal`, proven by `h3_max_aspect_is_per_endpoint_not_per_spec`)
  and stops the offline i2v path from emitting an undeclared key. The only cost — offline t2v
  falls back to fal's 16:9 default — is documented (implement.md N-limitations, registry.rs
  comment). This is the right call given the hard AC5 constraint; it diverges from S9 Seedance
  (which is single-shape) for a defensible reason. Recommend the follow-up in the note below.
- **(e) Pricing + promotional note.** Tiers transcribed value-for-value from the vendored
  snapshot prose: H3 Max `$0.025/$0.04/$0.08` per second at 480/768/1080p; Turbo `$0.0125/
  $0.02/$0.04` (exactly half). The snapshot prose also says "promotional launch rates, 50% off
  for a limited time," which the route notes render as "promotional launch rate (fal index
  2026-09-16)." `price_check` classifies all four as **PROSE**. What PROSE means for the owner:
  the audit confirms the fal prose *exists* in the vendored snapshot, but does NOT machine-check
  that the tier numbers in code match it (the multi-figure, resolution-conditioned prose is
  deliberately un-parsed). I re-read the snapshot prose by hand and the four tier sets match it
  exactly — but a future transcription typo in the tiers would not be caught until live
  verification (S10 AC6 seam). Owner should treat these numbers as hand-verified, not
  hermetically pinned, and expect the promotional rates to lapse.
- **(f) The 768p height map.** commands.rs:619 adds `"768p" => Some(768)`. `resolution_height`
  lowercases its input first (`to_ascii_lowercase`), so fal's `"768P"` resolves; the test
  asserts both cases and that a `{resolution:"768P"}` `SettingsDto` yields `Billable.height ==
  Some(768)`. Purely additive — every existing label unchanged, no regression. Incidentally
  also fixes the pre-existing `minimax_h3` 768p tier.
- **(g) Dead code / invented APIs / stale counts.** `FAL_ROUTE_MODES` length literal 51→53
  (compile-checked, +2 rows). Registry size test `+30 → +32` passes. `IMPORTED_2026_09`
  correctly left at 14; the two new ids go in a distinct `IMPORTED_2026_09_H3` which both
  resolve/adapter loops iterate via `.chain(...)`. `JOB_TYPES` still 16 groups (ids added to
  the video group's slice, not a new group). `fal_constants`, `apply_constants`,
  `unsupplied_required`, `constant_issues`, `IMPORTED_2026_09_H3` are all referenced — no dead
  code. No invented APIs: `parse`, `coerce`, `accepts`, `enums`, `ModelCapability::from_fal`
  all exist and are `pub`. `minimax_h3` (the different model) untouched, still on `minimax/h3`
  (asserted by `the_h3_max_models_are_present`). CHANGELOG wording is accurate and matches the
  behavior (defaulted-yet-required field, six aspect ratios on prompt-start only, 768P priced).

## Findings

### Minor — 1 finding

1. **Misattributed doc comment in the audit example.**
   `crates/hickeyfield-core/examples/audit_fal.rs:230-260`. The new `unsupplied_required`
   (:260) and `constant_issues` (:278) were inserted *between* the `pin_conflicts` doc block
   (:230-251, "Fields this endpoint pins where the app can still produce a different value …"
   with the Online/Offline list and the `aspect_ratio` paragraph) and `fn pin_conflicts`
   (:295). The result: that entire block now documents `unsupplied_required` (with the
   unrelated "Required fields nothing in the app supplies (check 2)." line grafted onto its
   end at :252), and `fn pin_conflicts` is left with **no** doc comment. Standard violated:
   comments must not contradict the code they sit above (clarity/AI-insertion-divergence tell).
   It compiles and all tests pass — behavior is unaffected — hence minor, and this is a
   non-shipping example binary. Concrete fix: move the `:230-251` block back to immediately
   above `fn pin_conflicts` (:295), leaving only the `Required fields nothing in the app
   supplies (check 2).` + its paragraph (:252-259) above `fn unsupplied_required`.

## Recommended follow-ups (not blocking; do not apply here)

- Surface `prompt_expansion_mode` as a real control in a future capability slice (implement.md
  N1) so `disabled`/`balanced`/`quality` are user-selectable rather than pinned to `balanced`.
- Restore an offline-t2v aspect choice for the H3 Max family without re-introducing the i2v
  check-3 finding (e.g. a per-endpoint-mode spec rather than one shared spec), tracked as the
  cost of the D3/Q2 deviation.
- Consider a live-key price verification pass (S10 AC6 seam) before the promotional rates
  lapse, since `price_check` can only classify these tiers as PROSE.

No blocker or major finding on any critical path. The mechanism is correct, the four endpoints
are clean against the live schema, pricing is faithfully transcribed, and the full gate is
green this session.

VERDICT: PASS
