# S17-keyframe-anchored-mode — SLICE (DISCOVER + PLAN)

## Delivers (one sentence, end-to-end)
"Keyframe mode" for a storyboard: the planner produces one still per shot boundary (image model, the
shared world block, the set's seed), every shot is generated image-to-video anchored on BOTH ends —
`image_url` = the actual last frame of the previous clip, `end_image_url` = the next planned still —
so each join is pinned from both sides, scored with the tier-2 thresholds, and the whole set (stills +
clips) is priced before anything is spent; the path from the 95% floor toward the owner's 99% ideal.

## Ground truth (S14 schemas, S15 memo D-D, S16 evidence)
- fal H3 Max i2v (and Turbo): `image_url` first frame, `end_image_url` last frame, either alone allowed
  ("end-only keyframe generation"), output canvas follows the frame; `seed` honoured;
  `prompt_expansion_mode: disabled` on chained shots (S15). Models without an end-frame key are in
  `NO_END_FRAME` (media.rs) → start-only chaining with an advisory (S15 behaviour).
- S16 landed: per-join score in `joins_json` (SSIM + Y/U/V deltas), floor 0.80/12/6, ideal 0.90/6/3
  reported; regenerate-from-frame; bounded auto-retry (opt-in, ≤2); world block + hold endings;
  fill-to-ceiling; ranges. S15: chain scheduler (settled hook), `last_frame(size)`, render per join.
- Image models in the registry with a priced fal route (GPT Image 2.5, Nano Banana Pro/2, Seedream 5,
  Qwen Image 3, FLUX.2) — S8/S9 verified; the still generator must be one of them, chosen per set.
- Money: S10 per-image pricing; S13 total-shown gate; S16 in-flight guard.

## Acceptance criteria (testable)
- [ ] AC1 — Still plan: in keyframe mode the plan gains `keyframes[N+1]` (prompt per boundary derived
  from the world block + the adjacent shots' hold descriptions, image model id, aspect = set's aspect,
  resolution = set's resolution); validator variants for count (N+1), model runnable for TextToImage,
  and aspect/resolution consistency; FakeChat fixtures.
- [ ] AC2 — Still generation as part of the set: stills are submitted first through the existing gated
  path (estimate + guard), stored on the set (`keyframes_json`, migration 11), priced into the total
  shown before Generate-all; a failed still parks the chain with a clear error.
- [ ] AC3 — Both-frame binding: shot i submits with `image_url` = last frame of clip i−1 (S15) and
  `end_image_url` = still i+1 (shot 0 uses still 0 as `image_url`); keyless bind test through the S14
  schemas proves both keys land in the body on H3 Max i2v; models in `NO_END_FRAME` fall back to
  start-only with an advisory recorded on the join.
- [ ] AC4 — Tier-2 scoring: keyframe-mode joins are scored against the ideal thresholds (0.90/6/3) and
  ALSO against the planned still (SSIM between the still and the clip's actual last frame — "end-frame
  adherence"), both recorded in `joins_json`; a below-ideal join offers regenerate (S16 path) and the
  bounded auto-retry when enabled; unknown → None never 0. Synthesised `--ignored` tests.
- [ ] AC5 — UI: Keyframe toggle in Storyboard (default off; enabled only when every shot's model has an
  end-frame-capable i2v route and an image model is available), still previews per boundary with
  regenerate-still, priced total including stills; Library set item shows both scores per join. vitest.
- [ ] AC6 — Money honesty: nothing submits without the total (stills + clips) shown; stills never
  re-generated silently; auto-retry bound applies to stills and shots separately; test on the gate.
- [ ] AC7 (regression) — cargo test --workspace (>= 924), ignored sidecar tests (>= 8), fmt, clippy -D
  warnings, provenance, keyless audit_fal unchanged, `cd ui && pnpm test && pnpm build` (>= 302);
  CHANGELOG; app builds + launches. No paid job by agents; threshold calibration remains human-run.

## Approach + why now
Owner's stated ideal (99%) and the memo's D-D. Builds only on shipped seams: S13 plan/gate, S15 chain +
frames, S16 score/retry. Non-goals: music/captions, cloud render, extend-model chaining.
