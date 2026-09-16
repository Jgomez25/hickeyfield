# S13-storyboard-plan — SLICE (DISCOVER + PLAN)

## Delivers (one sentence, end-to-end)
From one brief ("a 30-second ad for …"), the app produces an editable storyboard — a model-aware
shot list (per shot: prompt, duration, camera move, model, continuity role) drafted by the local
enhancer LLM through the S6 guard — lets the user preview/edit each shot's compiled prompt (S7),
shows the summed estimate, and generates every shot as one linked JobSet, so the clips exist on disk
ready for the render slice (S14) to chain and stitch.

## Ground truth (STATUS "Storyboard" notes + S4–S12 evidence; re-verify file:line in DESIGN)
- Enhancer backends (S4/S5): hosted OpenAI/Anthropic key or local Ollama, tiered model picker;
  S6 output guard (refusal/validation); S7 `preview_prompt` dry run + `final_prompt` verbatim submit.
- Compositor (`compositor.rs`, ~1,700 lines) has concat/crossfade/trim/normalize/captions/ducking,
  tested, but NOTHING spawns ffmpeg for rendering; `ffmpeg::locate` has 0 render callers (S10 added
  the `-i` probe only). Missing: last-frame extract, render_timeline command, job→job plan
  (JobSet relation; next migration is 8), runner hook `download_outputs`/OnUpdate.
- Continuity today: `MediaRole::Start` → `image_url`, `End` → `end_image_url` (`NO_END_FRAME` list
  media.rs); extend models: `grok_extend_video`, `veo3_1_extend` (fixed 7s/720p). Seedance 2.5 =
  native 4–30s t2v (no stitch needed for a single-shot 30s); LTX 2.5 6/8/10s; Kling/Wan t2v.
- Cost: Σ `estimate_cost` per shot (S10 estimator: unknown never $0; actuals settle per job).
- Camera presets: 25 (`camera::slugs()`, S12 camera strip). Use cases: TextToVideo / ImageToVideo /
  EditVideo tabs; S12 nav + Storyboard titlebar entry exists as a placeholder label only.

## Acceptance criteria (testable)
- [ ] AC1 — Shot-list drafting: `plan_storyboard(brief, target_seconds, model_prefs) -> StoryboardDto`
  command uses the configured enhancer backend with a storyboard system prompt to return 3–8 shots
  (each: title, prompt, seconds, camera slug or none, suggested model id from the registry, role
  start|middle|end), validated by the S6 guard and a schema validator (durations sum to the target
  within ±10%, every model id exists and is runnable for the tab, camera slug exists); on an
  unavailable backend returns an honest error. Rust tests with a fake backend: valid plan accepted,
  bad model id / bad sum / refusal rejected with reasons. No network in tests.
- [ ] AC2 — Model-aware durations: each shot's seconds are clamped to the chosen model's duration
  axis (enum/range from capabilities, S9-honest); a single-shot plan may pick Seedance 2.5 native 30s;
  test proves a 12s shot on an LTX model becomes 10s with an advisory.
- [ ] AC3 — Storyboard UI (new `Storyboard` view under the S12 titlebar entry): brief input +
  target length + Draft button; shot cards editable (prompt, seconds, camera, model via the existing
  picker), reorder, add/delete; per-shot "Preview prompt" reuses S7's preview; total estimate = Σ
  per-shot estimates with "unpriced" honesty; vitest: draft renders cards, edit updates the total,
  a shot with an unpriced model marks the total.
- [ ] AC4 — Generate all: submits every shot as jobs linked in one JobSet (migration 8: `job_sets`
  + `jobs.set_id`, `jobs.set_index`) using the existing submit path (estimate + S7 final_prompt per
  shot, S6 guard); the Library groups a set; Rust store tests for the relation and ordering; the
  submit path is unchanged for single jobs (existing tests pass).
- [ ] AC5 — Continuity hints recorded, not yet applied: each shot stores its `role` and, when a
  previous shot exists, `continuity: last_frame_of(prev)` as a plan field (S14 applies it); the plan
  JSON round-trips through the store (test).
- [ ] AC6 — Money honesty: nothing is submitted without the total shown; drafting a plan costs
  nothing on hosted backends beyond the enhancer call the user configured (documented); no shot
  is submitted to a model that S9's audit marks unrunnable. Test on the submit gate.
- [ ] AC7 (regression) — `cargo test --workspace` 0 failed (>= 820), fmt, clippy -D warnings,
  provenance, `cd ui && pnpm test && pnpm build` (>= 272); keyless audit_fal unchanged; CHANGELOG;
  app builds + launches. No paid job run by agents.

## Approach + why now
Owner's stated end goal (one brief → 30s ad). Split: S13 plans and generates the shots as a set
(reusing S4–S7 enhancer/preview/guard and S10 costs, all shipped); S14 renders (last-frame extract,
continuity chaining, crossfade, stitch via the ffmpeg sidecar). Thin first: no rendering here.
