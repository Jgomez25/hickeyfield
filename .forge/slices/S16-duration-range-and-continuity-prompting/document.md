# S16-duration-range-and-continuity-prompting — DOCUMENT

## What shipped
- **Duration ranges reach the UI**: `fal_schema::parse` records `minimum`/`maximum` (direct and inside
  `anyOf`); `capability::Axis` carries `range` (live schema first, then a registry-declared range, e.g.
  MiniMax H3 Max 5–15s, else none); `CapabilitiesDto.durationRange`; the chip row's ladder is built from
  the range (min, max and default always included, ≤12 chips) — the generic ladder that stopped at 12
  is used only when neither an enum nor a range exists. Submit-time and storyboard-time clamps with an
  advisory; the estimate prices the clamped value. Keyless audit check "RANGE MISMATCH" flags a registry
  range outside fal's published range (0 today).
- **Continuity prompting**: plan-level `world` block (subject, wardrobe, palette, lighting, lens, grade)
  composed verbatim into every shot prompt; non-final shots end on a hold, the next opens on it;
  validator variants `MissingWorld`, `PromptMissingWorld`, `IdenticalShots`, `NoHoldEnding`, `CutWords`,
  `MixedResolution`; storyboard prompt v2 (original wording); fill-to-ceiling planning (30s on H3 Max →
  2×15; on LTX → 3×10); one resolution per set; 1..=8 shots kept.
- **Measured continuity**: when shot N+1 settles in a chained set, SSIM plus Y/U/V average deltas
  between the continuity frame and N+1's first frame are computed with the bundled ffmpeg (S10
  sandbox), stored per join in `joins_json` (kind, ssim, dy, dc, pass, attempts), floor constants
  SSIM 0.80 / ΔY 12 / ΔC 6 (ideal 0.90 / 6 / 3 reported); unknown → None, never 0. Library set item
  shows the per-join badge; the render uses per-join overlaps (continuity → hard cut, else crossfade),
  which also fixes the partially-chained hard-cut limitation from S15. "Regenerate shot N+1 from the
  same frame" goes through the S13 gated retry with the same PNG and seed+attempt, subset total shown
  first; bounded auto-retry (≤2) is opt-in and off by default.
- Hygiene: non-vacuous failed-shot guard test; the `chainable_model` tightening was tried and reverted
  (the per-route check admitted a media-ignoring model); a whole-roster sweep test pins the coarse check.
- Deferred UI polish: world-block text editor; per-shot Regenerate button on the Storyboard card (the
  action exists on the Library set item).

## Docs changed
- `CHANGELOG.md`, `README.md` — ranges, continuity score, planner rules (original wording).
- `.forge/backlog.md` — S17 keyframe-anchored mode (the 99% path), S18 shot trim handles, threshold
  calibration on one real paid 2×15 set (human-run), minors from REVIEW.

## Evidence
- TEST / REVIEW / SECURITY = PASS on HEAD c76ac43; repair round 1 (regenerate in-flight guard, temp
  frame cleanup, ladder ≤12) re-verified — see appended verdicts. cargo test --workspace 919+ / 0 with
  7 real-sidecar ignored tests; fmt · clippy · provenance clean; pnpm test 301+ / build ok; keyless
  audit RANGE MISMATCH 0. Nothing spent; no key; thresholds are reasoned, calibration is human-run.
