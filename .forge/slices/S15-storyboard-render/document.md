# S15-storyboard-render — DOCUMENT

## What shipped
- **Last-frame extraction** `ffmpeg::last_frame(clip, size)` through the S10 sandbox (`-sseof -3
  -update 1`, optional `-vf scale=W:H:flags=lanczos` so the continuity frame matches the set's
  resolution); typed error on failure; proven on the real sidecar incl. a keyframe-sparse clip.
- **Continuity chaining** (`src-tauri/src/chain.rs`, runner settled hook, migration 9): in a chained
  set, shot N+1 is submitted only after shot N completes and its clip is on disk, with that clip's last
  frame bound as the start image; models without an image-to-video route fall back to text with an
  advisory; a failed predecessor parks the chain; exactly-once and relaunch-resume proven. Every
  dependent shot re-runs estimate, the S6 guard and S7 final_prompt at actual submit.
- **Continuity levers** (Fable memo, verified against fal's schema): chained shots send
  `prompt_expansion_mode: "disabled"` (fal no longer rewrites the prompt between shots) and a fixed
  per-storyboard `seed`; standalone submits keep "balanced". Override seam is schema-gated to declared keys.
- **Render** (`src-tauri/src/render.rs` + compositor `Join` enum): normalize → joins (continuity joins
  are hard cuts; crossfade 0.5s otherwise) → concat, with progress events; single-shot = normalize only;
  the final ad lands in the Library folder as a set item with Σ actual cost; intermediates cleaned.
  Duration arithmetic proven ±0.2s on the real sidecar.
- **Plan arithmetic**: overlap-aware target sum; S13's shot floor is now 1..=8 so a 2×15 H3 Max plan
  (and a single-shot native-30s plan) is reachable.
- **UI**: Chain toggle in Storyboard (default on when every shot's model has an i2v route); Render
  button, progress and Open result on the Library set item; render failures surface in the app banner.

## Docs changed
- `CHANGELOG.md` — `[Unreleased] › Added`: storyboard render and chaining, in original wording.
- `.forge/backlog.md` — follow-ups: per-join continuity SCORE (SSIM + luma/chroma deltas, `joins_json`
  reserved) with threshold badge and regenerate-from-frame → S16; planner world block + hold endings +
  validator variants → S16; keyframe/end-frame anchored mode → S17; partially-chained fallback join is
  a hard cut (per-join overlap comes with `joins_json`); minors: render.rs comment vs code, blocking
  progress read vs deadline, `chainable_model` coarse check, vacuous phase-guard test (behaviour
  covered by two other layers), chain::advance read/submit TOCTOU (UNIQUE index), override seam
  allow-list when no schema is cached.

## Evidence
- TEST / REVIEW / SECURITY = PASS on HEAD e82d2cd; UI error-surfacing fix 1139b13 re-reviewed.
  cargo test --workspace 888 / 0 + 2 real-sidecar ignored tests; fmt · clippy -D warnings · provenance
  clean; pnpm test 292 / build ok; keyless audit unchanged. Nothing spent; no key.
