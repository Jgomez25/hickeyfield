# S16-duration-range-and-continuity-prompting — SLICE (DISCOVER + PLAN)

## Delivers (one sentence, end-to-end)
A model's duration control offers the full range the provider actually accepts — MiniMax H3 Max
5–15s, Wan 3.0 Prime 2–30s, every free-form-duration model its real ceiling — instead of the UI's
generic ladder that stops at 12, and a 30-second storyboard can pick any model whose chained
clips reach the target.

## Ground truth (verified 2026-09-16)
- `ui/src/lib/variants.ts:45-53` `durationLadder`: when `caps.durations` is empty and the axis is
  free-form it returns a hardcoded `[3,4,5,6,8,10,12]` (+ the default) — the source of the owner's
  "MiniMax capped at 12" report. Rust is honest: `registry.rs` H3 Max spec is `ValueSpec::Number` with
  constraint "5–15 seconds"; fal's schema declares `duration` integer default 5 (no min/max published
  for H3 Max; Wan 3.0 Prime publishes `minimum 2 / maximum 30` via `anyOf`).
- `capability.rs`: `Axis { support, values, default, source }`, `is_free_form()`; no `range`.
  `fal_schema::parse` ignores `minimum`/`maximum` (S9 handled `const`/int enums only).
- S15 chaining makes a 30s ad reachable on any model via N shots; S13's clamp uses the axis values.

## Acceptance criteria (testable)
- [ ] AC1 — Schema: `fal_schema::parse` records `minimum`/`maximum` for integer/number fields
  (direct or inside `anyOf`) as `EndpointSchema.ranges[key] = (min, max)`; fixture tests (Wan 3.0
  Prime anyOf; a direct integer with max; a field without a range stays None).
- [ ] AC2 — Capability: `Axis` gains `range: Option<(f64, f64)>` sourced from the live schema, else
  from a registry-declared range on the spec (new `ValueSpec::Range { min, max }` or a
  `duration_range` on the spec — design decides) for models whose range fal states only in docs (H3
  Max 5–15); `CapabilitiesDto` carries `durationRange`. Tests for both sources and precedence.
- [ ] AC3 — UI: `durationLadder` builds from the range when present (integers min..=max, thinned to
  ≤12 chips with the default always included) and never emits a value outside it; the generic ladder
  is used only when neither enum nor range exists. vitest: H3 Max caps → 15 offered, 12 not the
  ceiling; Wan → up to 30; no range → old behaviour.
- [ ] AC4 — Reconcile/estimate: a duration outside the range is clamped with an advisory before
  submit (S9 `Pinned`-style), the estimate uses the clamped value; test.
- [ ] AC5 — Storyboard: S13's clamp uses the range for free-form models (12s on H3 Max stays 12; 20s
  → 15 with advisory; single-shot 30s allowed only if the range reaches 30); test.
- [ ] AC6 — Audit: keyless `audit_fal` gains a check that a spec-declared range lies within the
  schema's published range when fal publishes one; run recorded, no new findings on landed models.
- [ ] AC7 (regression) — cargo test --workspace, fmt, clippy -D warnings, provenance, keyless
  audit_fal, `cd ui && pnpm test && pnpm build`; CHANGELOG; app builds + launches. No paid job.

- [ ] AC8 — Continuity prompting (memo D-E/D-F, `.forge/slices/S15-storyboard-render/h3max-30s-memo.md`):
  the storyboard plan gains a `world` block (subject, wardrobe, palette, lighting, lens, grade) that the
  system prompt requires and the planner composes verbatim into every shot prompt; non-final shots must
  end on a static hold and the next shot opens on the same hold; validator variants `MissingWorld`,
  `PromptMissingWorld`, `IdenticalShots`, `NoHoldEnding`, `CutWords`, `MixedResolution`; duration policy
  for models with a hard per-call ceiling = fill to the ceiling (`n = ceil(target / ceiling)`), one
  resolution per set (768P default on H3 Max). Rust tests per variant; FakeChat fixtures; the S15
  `1..=8` shot bound preserved.
- [ ] AC9 — Per-join continuity SCORE (memo §0, tier 1): when shot N+1 settles in a chained set, compute
  SSIM (`-lavfi scale2ref,ssim`) plus signalstats Y/U/V average deltas between shot N's continuity frame
  and shot N+1's first frame through the S10 sandbox; store per join in the reserved `joins_json`
  (migration 9 column) with the join kind (Continuity/Crossfade) so a partially-chained set renders
  per-join overlaps; floor thresholds as named constants (SSIM 0.80, ΔY 12, ΔC 6); a per-join badge in
  the Library set item; "Regenerate shot N+1 from the same frame" through the S13 retry path with the
  same PNG and seed+attempt, cost shown first. `--ignored` real-sidecar tests: identical frames → ≈1.0
  passes; different lavfi sources → below floor flags; unit tests on the parser and thresholds.
- [ ] AC10 — Bounded auto-retry (memo §0 tier-2 groundwork): optional toggle; at most 2 regenerations
  of a below-floor join, seed+attempt, cost per attempt shown, through the S13 retry path; test with a
  fake scorer; never spends without the total shown.

## Approach + why now
Owner report; small and cross-cutting (schema → capability → UI → storyboard clamp). Runs after
S15 render lands so the storyboard clamp change sits on the shipped chaining code.
