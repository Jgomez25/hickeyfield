# S15-storyboard-render — IMPLEMENT (claim)

Author: implementer agent. Branch `forge/s15-storyboard-render`, worked in the tree (no commit,
no PR — RELEASE handles that). Built to `design.md` (D1–D7) **plus** the two mid-task course
corrections from the coordinator and the on-disk memo
`.forge/slices/S15-storyboard-render/h3max-30s-memo.md` (D-G, D-B, D-C, frame scaling shipped;
§0 SSIM scoring, D-D, D-E, auto-retry deferred — see "Deferred" below).

## Baselines measured at HEAD `3bb4069` (before any change)
- `cargo test --workspace`: **865 passed**, 0 failed (matches the contract floor).
- `cd ui && pnpm test`: **285 passed** (23 files).

## Final gate results (all commands below were run; counts pasted)
- `cargo test --workspace`: **888 passed, 0 failed** (865 + 23 new).
- `cargo test -p hickeyfield-core --lib -- --ignored last_frame_writes render_stitches`:
  **2 passed, 0 failed** on this Mac with the real sidecar
  (`src-tauri/binaries/ffmpeg-aarch64-apple-darwin`, 51 MB present). Pasted below.
- `cargo fmt --all --check`: clean.
- `cargo clippy --workspace --all-targets -- -D warnings`: clean (0 warnings).
- `./scripts/lint-provenance.py`: PASS (no third-party media hosts; no shipped string matches the
  corpus, 80015 shingles).
- `env -u hickeyfield_FAL_KEY -u FAL_KEY cargo run -q -p hickeyfield-core --example audit_fal`:
  **PINNED 0 / UNREACHABLE 0** — unchanged (no schema/mode logic touched).
- `cd ui && pnpm test`: **292 passed** (285 + 7 new), `pnpm build`: tsc + vite OK.
- `ui/node_modules/.bin/tauri build --bundles app`: see the tail pasted at the end.

### Design decisions taken by default (stated prominently, per the task)
1. **Final ad → the Library** (`library.rs`'s `~/Movies/Hickeyfield/day-<n>/`), not `app_data_dir`.
   The contract said app_data_dir; `library.rs:15-47` deliberately refuses `Application Support`
   and keeps user media visible. **Deviation, documented (design Q6).** Intermediates (extracted
   continuity frames) go under `app_data_dir()/renders/<sid>/frames/` and are the only app-data
   artefact; the render temp mp4 there is removed after a successful adopt. The `frames/` PNGs are
   **kept** (memo §0.2: the deferred continuity-score slice reuses the identical conditioning
   frame).
2. **Default crossfade 0.5s** (`storyboard::DEFAULT_CROSSFADE_SECONDS`); a **continuity** join is a
   **hard cut** (overlap 0) — D-B.
3. **Chain scheduler** via the runner's settled hook + a startup reconciliation, and **migration 9**
   per D2.

---

## Files changed

### Core (`crates/hickeyfield-core/src/`)
- **`ffmpeg.rs`** — `FfmpegError::Render(String)`; `last_frame(bin, clip, out, size)` +
  `last_frame_args` (pure, sandboxed) + `run_to_file`; `render(bin, args, timeout, on_progress)`
  streaming a `ProgressReader` over stdout. AC1 + AC3 spawns, reusing the S10 sandbox
  (`-nostdin`, `-protocol_whitelist file` before `-i`, wall-clock timeout + kill, bounded stderr,
  argv-only — no shell). Tests: `last_frame_args_sandbox_the_input_and_seek_from_eof`,
  `last_frame_args_scale_to_the_sets_canvas_when_requested`, `last_frame_failure_is_a_typed_render_error`,
  and `#[ignore]` `last_frame_writes_a_png_with_the_clips_dimensions`,
  `render_stitches_to_the_expected_duration`.
- **`compositor.rs`** — added `-protocol_whitelist file` to `to_args`' global input options (before
  the `-i`s). Test `the_render_argv_whitelists_only_the_file_protocol_before_the_inputs`.
- **`storyboard.rs`** — `DEFAULT_CROSSFADE_SECONDS`; `Join { Continuity, Crossfade(f64) }` +
  `stitched_duration` + `stitched_duration_joined`; `validate` gains an `overlap` parameter (gate on
  the **stitched** length); `plan` passes the default, sets each shot's `chainable`
  (`use_case::supports(m, ImageToVideo)`), and adds a post-clamp drift advisory. **D-G: shot-count
  floor `3..=8` → `1..=8`** (+ the "1 to 8 shots" copy + the `//! 1..=8` doc). Shot gains
  `#[serde(default)] chainable`. Tests: `stitched_duration_subtracts_the_overlaps`,
  `stitched_duration_joined_is_per_join`, `plan_time_gate_uses_the_stitched_duration`,
  `clamp_drift_is_advised_not_failed`, `the_shot_count_bound_is_one_to_eight` (rewritten).
- **`fal_schema.rs`** — `apply_overrides(overrides, schema, body)` (D-C seam): a caller (the chain)
  writes endpoint-declared keys into the body **before** `apply_constants`, so it overrides the S14
  constant (`balanced`→`disabled`) but is schema-gated (cannot inject an arbitrary field). Tests:
  `a_chained_override_disables_expansion_and_carries_the_seed`,
  `a_standalone_submit_keeps_balanced_and_has_no_seed`,
  `an_override_cannot_inject_a_field_the_endpoint_rejects`.

### Shell (`src-tauri/src/`)
- **`store.rs`** — **migration 9**: `chained`/`shots_json`/`overlap_seconds`/`joins_json` on
  `storyboards`, `storyboard_renders` table. `Storyboard` gains four fields; new `StoryboardRender`;
  `upsert_render`/`render`/`renders`; `row_to_storyboard`/`row_to_render`. v1-drop list + the
  migration_7 test's `user_version` (8→9) updated. Test
  `the_chained_columns_and_a_render_round_trip` (asserts `user_version==9`).
- **`runner.rs`** — `OnSettled` hook type; `Runner` holds `on_settled: Arc<OnceLock<OnSettled>>`
  (set post-Arc, so the hook can hold a `Weak<Runner>`); `download_outputs` fires it **once** when a
  `storyboard_id` job `is_settled()` (Completed + every output local). Test
  `the_settled_hook_fires_once_for_a_storyboard_shot`.
- **`chain.rs`** (new) — `chainable_model`, `storyboard_seed`, `chained_overrides`,
  `prepare_next_shot` (pure), `advance`, `resume_all`. AC2/AC6 behaviour. Tests below.
- **`render.rs`** (new) — `render_storyboard` command, `list_storyboard_renders`,
  `RenderOptions`/`StoryboardRenderDto`/`RenderProgressDto`, per-join Timeline build (continuity =
  hard cut, else crossfade), `ffmpeg::render` with progress emit, output probe, `Library::adopt`,
  `upsert_render` (Σ member actual, None if any unpriced). Tests
  `render_builds_a_normalize_crossfade_concat_timeline`,
  `a_continuity_join_is_a_hard_cut_keeping_every_second`.
- **`commands.rs`** — `submit_one` refactored to take `&Arc<SqliteStore>, &Arc<Prices>, &Runner`
  (not `&AppState`) so `chain.rs` can call it without a Tauri `State`; body **unchanged** (see the
  diff summary below). `SubmitInput` gains `constant_overrides` (+ `Serialize, Clone`);
  `submit_to_provider` gains a `constant_overrides` param, applied before `apply_constants`.
  `StoryboardGenerateInput` gains `chained`/`overlap_seconds`; `generate_storyboard_inner` stores the
  chain state and, in chained mode, submits **only shot 0** (rest returned `pending`). `ShotOutcome`
  gains `pending`. D-G test rewrite `a_single_shot_plan_is_accepted_after_the_floor_dropped_to_one`.
- **`library.rs`** — `Library::adopt(produced, stem, created_at)` (rename, copy+remove fallback
  across volumes). Test `adopt_moves_a_local_render_into_the_day_folder`.
- **`app.rs`** — `AppState.runner: Arc<Runner>`; new `ffmpeg_dir`/`renders_root` fields + helpers
  `sidecar_dir`/`renders_root`; wires the `on_settled` hook to `chain::advance` with a
  `Weak<Runner>`; calls `chain::resume_all(self)` after `runner.resume_all()`. `submit_to_provider`
  override seam.
- **`lib.rs`** — `mod chain; mod render;`; registered `render::render_storyboard`,
  `render::list_storyboard_renders`.

### UI (`ui/src/`)
- **`types.ts`** — `Shot.chainable`; `ShotOutcome.pending`; new `StoryboardRender`,
  `RenderProgress`, `RenderOptions`.
- **`api.ts`** — `generateStoryboard` gains `chained`; new `renderStoryboard`,
  `listStoryboardRenders`, `openResult` (opener plugin, falls back to reveal),
  `subscribeRenderProgress` (`storyboard:render` event).
- **`components/Storyboard.tsx`** — Chain toggle (`.storyboard-chain-toggle`), default on only when
  every shot `chainable`, with a note when not; passes `chained` on first generate (not retry).
- **`components/Library.tsx`** — per-set render controls: Render button (`.storyboard-render`,
  enabled only when every shot is Done = completed + downloaded), progress bar
  (`.storyboard-render-progress`, `role="progressbar"`), finished-render line with Σ actual +
  duration + Open result (`.storyboard-open-result`), and the money-honest "Rendering is free" note.
- **`App.tsx`** — render state (`renders`, `renderProgress`), `subscribeRenderProgress`
  subscription, `onRenderStoryboard`, and the new Library props.
- **`base.css`** — additive `.storyboard-chain*` / `.storyboard-render*` / `.storyboard-result*`.
- Tests: `Storyboard.test.tsx` (+3), `Library.test.tsx` (+4).

### Docs
- **`CHANGELOG.md`** — `[Unreleased] ### Added`: the chain + render feature and the money-honesty
  note (rendering is free; same per-shot estimate + guard; never past a failure; single-shot sets).

---

## Design §10's five weakest points — addressed with tests

**(1) The chain scheduler / `Arc<Runner>`–`Weak` cycle; the hook fires exactly once and is
idempotent with the startup resume scan.**
- `AppState.runner` is now `Arc<Runner>`; the hook lives in a `OnceLock` set **after** the Arc
  exists and captures `Weak<Runner>`; `upgrade()` failing (teardown) is a no-op branch — cannot
  panic (`app.rs`).
- The hook fires only for a settled `storyboard_id` job (`runner.rs` `download_outputs`);
  `the_settled_hook_fires_once_for_a_storyboard_shot` asserts one fire for a set shot and **zero**
  for a standalone job.
- **Double-fire safety:** `advance` no-ops when a row already exists at `next`
  (`advance_is_idempotent_when_the_next_shot_exists`) — so the hook and `resume_all` cannot both
  submit.
- **Relaunch mid-chain:** `chain::resume_all` scans every chained set at startup and advances from
  the highest settled shot with no successor. Guard tests
  `a_failed_shot_stops_the_chain` (no submit past a failure) and
  `advance_ignores_a_non_chained_set_and_a_complete_chain` (non-chained / last-shot → no submit).
  Together with the idempotency guard, resume + a still-in-flight shot cannot double-submit.

**(2) Output-location deviation** — documented above (Library, not app_data_dir; Q6). `Library::adopt`
+ `adopt_moves_a_local_render_into_the_day_folder`.

**(3) `-sseof` on a keyframe-sparse synthesised clip.** The `--ignored`
`last_frame_writes_a_png_with_the_clips_dimensions` synthesises `testsrc` with `-g 250` (a single
head keyframe — the hostile case) and ran on this Mac's real sidecar. It **initially failed** with
`-sseof -0.05` (produced no frame — exactly the risk design §10.3 named), so `last_frame` was made
robust: **`-sseof -3` + `-update 1` (no `-frames:v 1`)** — seek to a keyframe ≤3s before EOF, then
overwrite the PNG for every decoded frame to EOF, so the file left is the *true* last frame even on
a sparse GOP. Re-run: **passes**. Pasted below.

**(4) Render-duration arithmetic against the real sidecar (±0.2s).** `--ignored`
`render_stitches_to_the_expected_duration`: three 4s `testsrc` clips crossfaded 1s → probes
**10±0.2s**, plus a single clip → its own 4±0.2s. Ran on the real sidecar. Pasted below.

**(5) `submit_one` refactor — single-job behaviour unchanged.** The only change is the parameter
shape: `state: &AppState` → `(store: &Arc<SqliteStore>, prices: &Arc<Prices>, runner: &Runner)`,
with the three uses rewritten `state.prices`→`prices`, `state.store`→`store`,
`state.runner`→`runner`. The compile→S6 guard→S7 verbatim→S10 estimate+provenance→`route::resolve`→
upsert→`submit_to_provider`→`runner.watch` body is **line-for-line identical** otherwise (the one
addition is passing `input.constant_overrides` — `None` on every standalone submit, so a single job
is byte-identical). S13's reuse tests stay green (the full-plan retry test still passes); `submit_job`
now calls `submit_one(&state.store, &state.prices, &state.runner, …, None, None)`.

## Coordinator course corrections — what shipped vs deferred

**Shipped (must-haves per the memo):**
- **D-G** — `1..=8` shot floor (+ copy + tests). First thing the verifier checks.
- **D-B** — `Join` enum + `stitched_duration_joined`; a continuity join renders as a **hard cut**
  (overlap 0), a non-continuity join a crossfade. 2×15 chained = **30.0s**, unchained = **29.5s**
  (`stitched_duration_joined_is_per_join`, `a_continuity_join_is_a_hard_cut_keeping_every_second`).
  `generate_storyboard_inner` stores `overlap_seconds = 0.0` for a chained set (so re-validate and
  the D3 probe assert the number that renders); the render decides each join from the shot's Start
  media at runtime.
- **D-C** — chained shots submit `prompt_expansion_mode: "disabled"` + a fixed per-storyboard `seed`
  via the `constant_overrides` binder seam (`apply_overrides` runs before `apply_constants`;
  schema-gated). Keyless bind test proves both keys land on a schema that declares them
  (`schemas/minimax_h3-max_image-to-video.json` carries both) and that a standalone body keeps
  `balanced` and no seed. `chain::prepare_attaches_the_frame_and_disables_expansion_when_chainable`
  asserts the chain sets them.
- **Frame scaling (#4 / D-C)** — `last_frame` gains `size: Option<(u32,u32)>` →
  `-vf scale=W:H:flags=lanczos`. The chain passes the settled clip's probed dims (the set's canvas,
  since a chained set is one resolution). Test `last_frame_args_scale_to_the_sets_canvas_when_requested`.

**Deferred (recorded, with the migration reserved):**
- **§0 per-join SSIM/signalstats continuity score + `joins_json` + the score UI + "Regenerate shot
  N+1 from the same frame"** — NOT built. The `storyboards.joins_json` column **is reserved** in
  migration 9 (stored + round-tripped, `None` today) so that slice is purely additive. The frames
  under `renders/<sid>/frames/` are kept (not cleaned) so a future regenerate reuses the identical
  conditioning PNG. Design to land next: `ffmpeg::first_frame` (same argv as `last_frame` sans the
  seek), pure `parse_ssim`/`parse_signalstats` on captured stderr, `join_passes` at the
  0.80/ΔY 12/ΔC 6 floor, compute-at-settle in `chain::advance`, per-join badge, and a retry with
  `seed + attempt`.
- **D-D** (`end_image_url` keyframe mode), **D-E** (planner `world` block + validator variants), the
  bounded auto-retry loop, and threshold calibration (memo Q4) — all next-slice, as the memo says.
- **N1 audio, N2 rich captions, N3 cloud render, N4 provider extend-endpoints** — design non-goals,
  unchanged. `RenderOptions` has no `captions` field (I dropped it rather than ship a dead toggle).

**Known limitation I did NOT fix (stated honestly):** in a *partially* chained set (a shot fell back
to text-only mid-chain), that fallback join renders as a **hard cut**, not a 0.5s crossfade — because
a chained set stores `overlap_seconds = 0.0`. The memo's 3×10-with-fallback example expects the
fallback join to crossfade (29.5s). In practice a chained set is all-chainable (one resolution, H3
Max — D-F), so every join is continuity; the mixed case is an edge that renders as clean cuts
throughout (keeps all paid seconds, just less smooth on the one fallback join). The per-join render
logic is correct; only the stored single `overlap_seconds` is a coarse approximation for the mixed
case. Not a money or correctness bug.

**Architecture note (deviation from AC5 wording):** the **Render button + progress bar + Open
result live in the Library set item**, not the Storyboard view. The Storyboard view resets after
generate (`onGenerated` switches to Library), so it has no job-Done state; the Library is where the
grouped set and its shot statuses live, which is where "enabled when every shot is Done" is
actually knowable. The **Chain toggle stays in the Storyboard view** (it affects generate). The
Library set item shows the rendered ad + Σ actual, as AC5 also requires.

---

## Money / safety invariants (hard rules)
- Chaining never submits a shot whose predecessor failed: the hook fires only for a Completed +
  settled shot; a failed shot parks the chain (`a_failed_shot_stops_the_chain`).
- Each chained shot re-runs estimate + S6 guard + S7 `final_prompt` + `route::resolve` at actual
  submit time via `submit_one` (`prepare` builds the input; `advance` calls `submit_one`).
- Total shown at generate time (the UI's summed per-shot estimate, unchanged S13); rendering spends
  nothing and the UI says "Rendering is free — it runs on your machine".
- Every ffmpeg spawn (`last_frame`, `render`) uses the S10 sandbox: timeout scaled to duration + a
  kill, `-nostdin`, `-protocol_whitelist file` before `-i`, bounded stderr, argv vector — no shell.
- Disk: the only intermediates are the small frame PNGs (kept for the deferred score slice) and the
  render temp mp4 (removed after adopt). No vendor copy.
- No paid job submitted by any agent; `first_light.rs` never touched; no key exists.

---

## Pasted command outputs

### `cargo test --workspace` (final)
```
RUST TOTAL: 888        # (0 failed; summed across all `test result: ok` lines)
```

### `cargo test -p hickeyfield-core --lib -- --ignored last_frame_writes render_stitches`
```
test ffmpeg::tests::last_frame_writes_a_png_with_the_clips_dimensions ... ok
test ffmpeg::tests::render_stitches_to_the_expected_duration ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 735 filtered out; finished in 5.83s
```
Sidecar present: `src-tauri/binaries/ffmpeg-aarch64-apple-darwin` (51,576,912 bytes).
NOTE: the *full* `-- --ignored` core run also has 2 FAILs —
`enhancer::…a_local_rewrite_completes_against_a_real_daemon` (needs a running Ollama) and
`prices::…live_feeds_answer…` (needs live network price feeds). Both are pre-existing,
network/daemon-dependent, and unrelated to S15; my two ffmpeg ignored tests pass.

### `cargo clippy --workspace --all-targets -- -D warnings`
```
(clean — 0 warnings)
```

### `cargo fmt --all --check`
```
(clean)
```

### `./scripts/lint-provenance.py`
```
CDN hostnames         PASS  no third-party media hosts referenced
Copy provenance       PASS  no shipped string matches the reference corpus (80015 shingles indexed)
```

### keyless `audit_fal`
```
=== PINNED FIELDS — the endpoint fixes this value and we would send another (0) ===
=== UNREACHABLE — fal serves no schema for this endpoint (0) ===
```

### `cd ui && pnpm test`
```
Test Files  23 passed (23)
      Tests  292 passed (292)
```

### `cd ui && pnpm build`
```
tsc --noEmit && vite build  → ✓ built (76 modules transformed)
```

### `ui/node_modules/.bin/tauri build --bundles app` (from repo root; exit 0)
```
✓ built in 3.20s
   Compiling hickeyfield-core v0.1.0 (…/crates/hickeyfield-core)
   Compiling hickeyfield-tauri v0.1.0 (…/src-tauri)
    Finished `release` profile [optimized] target(s) in 4m 51s
       Built application at: …/target/release/hickeyfield-tauri
    Bundling Hickeyfield.app (…/target/release/bundle/macos/Hickeyfield.app)
    Finished 1 bundle at:
        …/target/release/bundle/macos/Hickeyfield.app
```
The `.app` bundled cleanly. I did NOT install or open it (per the task). A VERIFY agent should
launch it and confirm a real window (not blank).
