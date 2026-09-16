# S9-fal-schema-const-and-int-enums — SLICE (DISCOVER + PLAN)

## Delivers (one sentence, end-to-end)
The app's fal schema layer honours `const`-pinned fields and integer/number enum members, so the
keyless audit can see them, no offered model silently 422s on a pinned field, and the four
generation paths S8 had to withdraw (LTX 2.5 Pro, LTX 2.5 Pro Animate, Veo 3.1 Extend, Seedance 2.5
image-to-video) come back verified and runnable.

## Ground truth (verified 2026-09-16, S8 REVIEW/TEST evidence)
- `crates/hickeyfield-core/src/fal_schema.rs:216-226` (`parse`) keeps only STRING members of
  `properties[*].enum` and never reads `const`. Consequences, all measured live:
  · LTX 2.5 `duration` enum {6,8,10,"auto"} records as ["auto"] → `reconcile` refuses default 5 →
    `JobError::Permanent` on every submit.
  · Veo 3.1 extend-video pins `duration:"7s"`, `resolution:"720p"` by `const`; app sends 5.0/1080p
    → 422. Seedance 2.5 image-to-video pins `aspect_ratio:"auto"` by `const` → app sends "16:9" → 422.
  · `examples/audit_fal.rs:133` check 4 fires only when both sides enumerate, so it cannot see any
    of the above ("CHECKED / 0 findings" is hollow for const-pinned or int-enum fields).
  · `capability::fal_axis` reports such an axis as free-form → ChipRow hides the control, nothing
    downstream (`resolveSettings`, `reconcile`, app.rs unknown-key sweep) rescues the value.
- S8 withdrew: `ltx_2_5`, `ltx_2_5_animate`, `veo3_1_extend` (specs, priced_routes arms, JOB_TYPES,
  FAL_ROUTE_MODES 51→48, IMPORTED_2026_09 14→11, +30→+27 size literal) and Seedance 2.5's
  `InputMode::Image` (+ its `start_image`/`end_image` flags, `use_case.rs:317` negative guard).
  Their prior transcriptions are in git history (`b319c5d^..b319c5d`, `9e53dfd`, `55aa32f`).
- Wan 3.0 Prime `duration` is `anyOf[{integer 2..30},{null}]` (free-form int, no enum) — not this
  slice's problem but the parser must not regress it.
- All probing is keyless (`fal.ai/api/openapi/queue/openapi.json?endpoint_id=…`); no money.

## Acceptance criteria (testable)
- [ ] AC1 — Parser: `fal_schema::parse` records a `const` as a one-member enum and stringifies
  integer/number enum members (e.g. {6,8,10,"auto"} → ["6","8","10","auto"]); string enums
  unchanged; `anyOf`/nullable free-form integers still not enumerated. Fixture tests for: const
  string, mixed int+string enum, pure int enum, const alongside enum, nullable int (no enum).
- [ ] AC2 — Reconcile/coerce honour the new enums: a value outside a const-pinned field is
  coerced to the pinned value (or refused with a clear reason — design decides, test pins it);
  a numeric duration like 6.0 matches member "6". `reconcile` never sends a value fal would 422.
- [ ] AC3 — Audit sees it: `audit_fal` check 4 uses the enriched enums, so a spec offering a value
  fal pins/enumerates differently is reported; a new check reports a const-pinned field the app
  would send with a different value. Unit test on the finding with a const fixture.
- [ ] AC4 — Re-probe ALL existing fal routes with the fixed parser (keyless audit run recorded in
  evidence); every new finding on an existing model is either fixed in this slice or logged with
  the reason. UNREACHABLE stays 0.
- [ ] AC5 — Re-land, each passing the fixed audit with zero findings: `ltx_2_5`, `ltx_2_5_animate`
  (duration enum 6/8/10(/auto), default 6), `veo3_1_extend` (fixed 7s/720p: not offered as axes,
  cost model uses the fixed 7s), and Seedance 2.5 `InputMode::Image` with `start_image`/`end_image`
  restored (aspect const "auto" honoured on that mode; t2v unchanged). Counts/array literals and
  the `use_case.rs` guards updated (negative → positive where re-landed), not deleted.
- [ ] AC6 — Capability/UI honesty: a const-pinned axis is shown as fixed (single value or hidden
  with the pinned value sent), never as a free-form control whose value is discarded. Rust test on
  the capability axis; vitest only if UI code changes.
- [ ] AC7 (regression) — `cargo test --workspace` 0 failed (>= 779), fmt, clippy -D warnings,
  provenance lint, `cd ui && pnpm test && pnpm build`; keyless audit_fal; CHANGELOG entry;
  app builds + launches.

## Approach + why now
Owner order: this before pricing. It is the root cause behind four withdrawals in S8 and an
unknown number of silent 422s on existing routes; fixing the parser once makes the free audit
trustworthy for every later catalogue sync. No new UI surface; ~parser + audit + registry re-land.
