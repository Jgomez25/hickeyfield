# S17-keyframe-anchored-mode — SLICE (DISCOVER + PLAN)  (parked; the 99% path per the S15 memo D-D)

## Delivers
"Keyframe mode" for a storyboard: the planner produces N+1 stills (image model, shared world block,
fixed seed), each shot is generated image-to-video with `image_url` = actual last frame of the previous
clip and `end_image_url` = the next planned still (H3 Max i2v supports both; canvas follows the frame),
so every join is anchored on both sides; continuity score per join, tier-2 thresholds (0.90 / ΔY 6 /
ΔC 3), bounded auto-retry; cost = stills + clips shown before generating.

## Ground truth
fal H3 Max i2v schema: `image_url` first frame, `end_image_url` last frame, either alone allowed; S14
constant fields; S15 last_frame + joins score; S16 world block + auto-retry. Models without
`end_image_url` (NO_END_FRAME list) fall back to start-only chaining with an advisory.

## Acceptance criteria (draft)
AC1 stills plan + generation as part of the set (priced); AC2 both-frame binding per shot with keyless
bind test; AC3 fallback for start-only models; AC4 tier-2 thresholds + 2-retry loop; AC5 UI keyframe
toggle + still previews; AC6 regression gate; no paid job by agents.
