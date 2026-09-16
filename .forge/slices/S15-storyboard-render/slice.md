# S15-storyboard-render — SLICE (DISCOVER + PLAN)

## Delivers (one sentence, end-to-end)
A generated storyboard becomes one finished 30-second ad on disk: each shot's clip is chained with
last-frame continuity (the previous shot's final frame becomes the next shot's start frame), the
clips are crossfaded and stitched through the bundled ffmpeg sidecar with progress, and the result
opens from the Library with its total actual cost.

## Ground truth (STATUS "Storyboard" notes + S10/S13 evidence; re-verify file:line in DESIGN)
- `crates/hickeyfield-core/src/compositor.rs` (~1,700 lines): concat/crossfade/trim/normalize/
  variants/captions/ducking — builds ffmpeg argument plans, unit-tested, but NOTHING spawns ffmpeg
  for rendering. `ffmpeg::locate` finds the sidecar (`src-tauri/binaries/ffmpeg-aarch64-apple-darwin`);
  S10 added the only spawn so far: the `-i` probe with 20s timeout + kill, `-nostdin`,
  `-protocol_whitelist file`, bounded stderr — the pattern to reuse for rendering.
- Missing: last-frame extraction (`ffmpeg -sseof -0.05 -i clip -frames:v 1 out.png` ~15 lines);
  a `render_timeline` command (sidecar + progress reader → UI events); continuity chaining at
  generate time (S13 records `role` + `continuity: last_frame_of(prev)` in the plan but does not
  apply it); a job→job dependency so shot N+1 waits for shot N's clip; runner hook after
  `download_outputs` / `OnUpdate`.
- Continuity binding today: `MediaRole::Start` → `image_url`, `MediaRole::End` → `end_image_url`
  (`NO_END_FRAME` list in media.rs); i2v modes exist for Seedance 2.5, LTX 2.5 Animate, H3 Max/Turbo
  (S9/S14); extend models `veo3_1_extend` (fixed 7s/720p), `grok_extend_video`.
- Seedance 2.5 native 30s: a single-shot storyboard needs no stitch (render = copy/normalize).
- Cost: Σ per-shot actual (S10) + nothing for the local render; crossfade overlap reduces total length
  (design must define the target-length arithmetic).
- S13 store: `storyboards` + `jobs.storyboard_id/shot_index`; per-shot outcomes; retry into the set.

## Acceptance criteria (testable)
- [ ] AC1 — Last-frame extraction: `ffmpeg::last_frame(clip) -> PathBuf` via the sidecar with the S10
  sandbox (timeout/kill, -nostdin, protocol whitelist, bounded stderr); test on a synthesised clip
  (lavfi testsrc) asserts a PNG exists with the clip's dimensions; failure → typed error, never a
  silent skip.
- [ ] AC2 — Continuity chaining: generating a storyboard in "chained" mode submits shot N+1 only after
  shot N completes and downloads, attaching shot N's last frame as `MediaRole::Start` (image_url)
  when the chosen model serves image-to-video, else falls back to text-only with an advisory;
  all through the S13 money-gated path (total shown first; per-shot estimate + guard). Rust tests
  with a fake provider: order enforced, frame attached, fallback advisory, failure stops the chain.
- [ ] AC3 — Render: `render_storyboard(storyboard_id, options)` builds the compositor plan (normalize →
  crossfade N-1 joins → concat → optional captions) and runs the sidecar with progress events to the
  UI; output written under the app data dir and registered as a Library item linked to the set; test
  on synthesised clips asserts the output exists and its probed duration equals Σ clip durations −
  Σ overlaps (±0.2s); the S10 probe verifies dimensions.
- [ ] AC4 — Target-length arithmetic: the plan's durations + crossfade overlap are reconciled with the
  target (30s) at plan time (S13 validator gains an overlap-aware sum) so the stitched result lands
  within ±10% of the target; test.
- [ ] AC5 — UI: Storyboard view gains Chain toggle (default on when every shot's model has an i2v
  route), a Render button (enabled when every shot is Done), progress bar, and "Open result"; the
  Library shows the rendered ad as a set item with Σ actual cost. vitest for states.
- [ ] AC6 — Money honesty: rendering costs nothing and says so; chaining never submits a shot whose
  predecessor failed; the set's Σ actual updates as shots settle (S10). Test on the gate.
- [ ] AC7 (regression) — cargo test --workspace (>= 865), fmt, clippy -D warnings, provenance, keyless
  audit_fal unchanged, `cd ui && pnpm test && pnpm build` (>= 285); ffmpeg `--ignored` tests with the
  real sidecar run and pass on this Mac; CHANGELOG; app builds + launches. No paid job by agents.

## Approach + why now
Completes the owner's end goal on top of S13 (plan/generate) and S14 (H3 Max). Order: last-frame
extract → render command over synthesised clips (no money) → chaining in the runner → UI.
