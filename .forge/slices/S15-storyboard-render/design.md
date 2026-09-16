# S15-storyboard-render — DESIGN

Contract: `.forge/slices/S15-storyboard-render/slice.md` (AC1–AC7).
Upstream this design is built on and reuses wholesale:
`.forge/objective.md` (§Constraints stack rule, §A no-regression gates, §D first editing tool
surface — the ffmpeg/compositor seam this completes, §"NOT in scope"), and the shipped slices:
S13 (`.forge/slices/S13-storyboard-plan/design.md` + `implement.md`: the plan DTO, `storyboards`
table, migration 8, `submit_one`, `generate_storyboard_inner`, per-shot outcomes, retry-into-set,
continuity **recorded not applied**), S10 (`.forge/slices/S10-pricing-accuracy-and-cost-tracker/
design.md`: the sandboxed `ffmpeg -i` probe in `ffmpeg.rs`, `settled_billable`, `compute_actual`,
`ActualSource`), S9 (`MediaRole::Start`→`image_url`, `MediaRole::End`→`end_image_url`,
`NO_END_FRAME`), S14 (`.forge/slices/S14-minimax-h3-max-constant-fields/design.md`: H3 Max i2v
`image_url`/`end_image_url`, the family-root suffix resolver). House style followed: S13's and
S14's `design.md` — grounded `file:line`, decisions with rationale, per-AC named tests, exact gate
commands, order of work, open questions with recorded defaults, riskiest parts first.

Slice shape: **the render/chain half of the storyboard feature, entirely on top of S13's plan/
generate.** Concretely — two new core `ffmpeg.rs` spawn helpers (`last_frame`, `render`) reusing
the S10 sandbox; one small compositor argv-safety addition; one store migration (9: `chained`/
`shots_json`/`overlap_seconds` on `storyboards`, a `storyboard_renders` table); one shell-side
chain scheduler (`chain.rs`) driven by a new runner "settled" hook + a startup reconciliation; one
shell render command (`render.rs` + a `render_storyboard` command) that builds a compositor
`Timeline` and streams progress; a `storyboard::stitched_duration` + overlap-aware plan check
(AC4); and additive UI (Chain toggle, Render button, progress bar, Open result, Library set item).
**Nothing here re-opens S13's money path** — chained shots and the render all reuse `submit_one`
and the S10 pricing verbatim; the render itself spends nothing on any provider.

---

## 0. Session constraint, stated before anything is claimed

Authored by reading the working tree at HEAD `1147b5e` this session; every `file:line` below is
first-hand. Written **without running** `cargo`, `pnpm`, or `tauri build` (a `tauri build` holds
the working tree and the launching agent forbids touching `ui/`, `src-tauri/`, `crates/` while it
runs), so every count and every "still green" is a claim the TEST/VERIFY agents must confirm, not a
measured fact. Baselines are the contract's floor (`cargo test --workspace` ≥ 865, `pnpm test`
≥ 285), not readings.

**No money, no keys, no `first_light`.** The render and the last-frame extraction spawn only the
bundled **ffmpeg sidecar** on **local files** — no network, no provider, no charge. Chained shots
reuse S13's `submit_one`, which agents never run against a real provider (a submit needs a paid
POST). Every render/extract test uses **synthesised lavfi clips** (`testsrc`), and the real-sidecar
ones are `#[ignore]`d (§D7) so the keyless `cargo test --workspace` gate never spawns ffmpeg.
`tauri build` produces a local `.app` only (README forbids bare `cargo build` — blank window).

### 0.1 Ground truth, as read this session

| Fact | Read at |
|---|---|
| `ffmpeg.rs` already has the S10 sandbox: `run_probe` spawns with `-nostdin`, `-protocol_whitelist file` **before** `-i`, a 20s wall-clock timeout + `kill`, drain threads, `bounded_push` cap; `probe_file`/`parse_probe` return `MediaDims{seconds,width,height}`, each field `Option`, **never 0 on failure** | `ffmpeg.rs:697-791`, `:591-637` |
| `ffmpeg.rs` has a `ProgressReader` for `-progress pipe:1` (handles `out_time_us`, split lines, `progress=end`) and `Progress::percent(total)` clamped 0..1; **nothing spawns a render yet** | `ffmpeg.rs:477-569`, `:493-501` |
| `ffmpeg::locate(dir)` finds `ffmpeg-<triple>` or bare `ffmpeg`; the runner locates it next to `current_exe()` (`probe_first_local`) | `ffmpeg.rs:64-79`; `runner.rs:247-275` |
| `FfmpegError::{NotFound,Spawn,Probe,Licence}` — no "render/extract failed" variant yet | `ffmpeg.rs:81-105` |
| `compositor::Timeline` builds a complete `-filter_complex` (normalize → xfade/concat → captions → `format=yuv420p`) and `to_args()` (input `-ss/-t/-i` per clip, maps, encoder, `-movflags +faststart`, `-progress pipe:1`, `-nostdin`, `-y`); `total_duration()` = Σ clips − Σ overlaps; a single clip terminates at `[vout]` with no concat; `render_variants` exists; **`to_args` has no `-protocol_whitelist`** | `compositor.rs:335-618`, `:392-394`, `:926-931` |
| `Clip::new(path, trim_start, trim_end)` + `.crossfade_in(seconds)`; `OutputSpec::new(w,h,fps,path)` (default `Fill`, `Libx264`, `Crf(20)`) | `compositor.rs:59-73`, `:220-231` |
| `ffmpeg::select_encoder(&available)` / `probe_encoders(bin)` pick VideoToolbox on macOS, else libx264 | `ffmpeg.rs:294-342` |
| The runner downloads outputs (`download_outputs`), sets `local_path`, computes `actual_usd` via `compute_actual`+`settled_billable`+`probe_first_local`, and calls `on_update(&job)` **once** with the settled row; a completed-but-undownloaded job resumes and skips the poll | `runner.rs:183-240`, `:576-621`, `:496-498` |
| `Runner::new(store, clients, on_update, library, prices)`; `watch(job)` is idempotent; `resume_all` re-attaches everything in `unfinished()`; the shell wires `on_update` to `handle.emit("job:update", job)` | `runner.rs:74-92`, `:110-145`, `:96-103`; `app.rs:112-133` |
| `AppState { store: Arc<SqliteStore>, runner: Runner, library: Arc<Library>, prices: Arc<Prices> }` — **`runner` is held by value** | `app.rs` (init `:107-141`) |
| `submit_one(state, model, &SubmitInput, storyboard_id, shot_index)` is the whole money path (compile→S6 guard→S7 verbatim→S10 estimate+provenance→`route::resolve`→upsert→`submit_to_provider`→`runner.watch`); tags the two `JobSet` set-membership fields; `mint_job_id`/`mint_storyboard_id` are collision-proof | `commands.rs:1018-1111`, `:40-46` |
| `generate_storyboard_inner` writes the group row first, re-validates `plan_json` via `storyboard::validate`, pins each shot's model/duration/route to the plan, supports a retry subset (`shot_indices`, `storyboard_id`), then loops `submit_one` best-effort | `commands.rs:1284-1484` |
| `store::Storyboard{id,brief,target_seconds,plan_json,created_at}`; `upsert_storyboard`/`storyboard`/`storyboards`/`jobs_in_storyboard` (ordered by `shot_index`); migration is **8**, next is **9**; the v1-drop list + `PRAGMA user_version` bump pattern | `store.rs:21-31`, `:229-314`; migration flow (S13 impl §store) |
| `JobSet` carries `storyboard_id: Option<String>`, `shot_index: Option<u32>` (write-once), `results: Vec<Output{url,kind,local_path}>`, `advisories`, `actual_usd`, `status` | `engine.rs` (JobSet); `runner.rs:792-822` (literal) |
| `media::can_bind(spec, MediaRole::Start, Dialect::Fal, slug)`, `resolve_endpoint(slug, Image, true)`, `MediaRole::Start`→`image_url`; `MediaRef::local(role, path)` → `MediaSource::Local` (uploaded at submit) | `media.rs:391-414`, `:189-228`, `:268-270` |
| `storyboard::plan/validate/clamp_shot`; `validate(shots, target, uc)` checks `3..=8`, runnable model, camera slug, **raw** Σ seconds within ±10%; `Continuity{last_frame_of}` recorded on every shot after the first | `storyboard.rs:194-354`, `:247-295` |
| `SettingsDto{duration:Option<f64>, resolution, aspect, …}`; `resolution_height(label)` | `commands.rs:586-605`, `:613` |
| `Library{root}` writes into `~/Movies/Hickeyfield/day-<n>/`, `fetch` downloads a provider URL; there is **no** "register a locally-produced file" method | `library.rs:31-47`, `:124-188` |
| `app_data_dir()` is where the sqlite lives (`db_path`); the sidecar is an `externalBin` (`tauri.conf.json:63-65`); the shell plugin's `shell:allow-execute` is for the **webview**, not the Rust runner (the runner spawns `ffmpeg` directly via `std::process::Command`) | `app.rs:100-105`; `capabilities/default.json:20-29`; `runner.rs`/`ffmpeg.rs` (direct `Command`) |
| The UI receives job changes via `listen("job:update")`; `opener:allow-open-path`/`reveal-item-in-dir` already permit "Open result" | `app.rs:115`; `capabilities/default.json:17-18` |
| Storyboard view exists (S13): brief/target/Draft, editable shot cards, per-shot preview, summed estimate gate, partial-failure retry; Library groups by `storyboardId` | S13 implement §UI |

**Corrections to the contract's Ground truth (re-verified this session):**
- The contract lists `ffmpeg::last_frame` as ~15 lines "missing" — correct, but it is **not** a
  copy of the probe: the probe tolerates ffmpeg's exit 1 (no output file), whereas `last_frame`
  **writes** a PNG and must assert success. A new spawn helper, not a `run_probe` call (§D1).
- The contract says register the render "under the app data dir". `library.rs:24-30` deliberately
  keeps user media in `~/Movies/Hickeyfield` (visible, survives uninstall) and refuses
  `Application Support`. I split the two (§D3): **intermediates** (extracted frames, temp
  normalises) go under `app_data_dir()/renders/<sid>/`; the **final ad** goes into the Library so
  it is a real, findable item — honouring both the contract's "registered as a Library item" and
  the library module's own rule. Recorded as Q6.
- `AppState.runner` is a **by-value `Runner`**, so a chain scheduler that must call `runner.watch`
  from a runner callback is an Arc cycle. §D2 changes `AppState.runner` to `Arc<Runner>` and the
  hook holds a `Weak<Runner>`. This is the single most invasive change in the slice.

---

## 1. Decisions

### D1 — Last-frame extraction: `ffmpeg::last_frame` (AC1)

A new **spawn helper in `crates/hickeyfield-core/src/ffmpeg.rs`**, beside `run_probe`, reusing its
hardening but asserting success and returning the written path:

```rust
/// Extract the final frame of `clip` as a PNG at `out`, via the sidecar.
///
/// Sandboxed exactly like the probe: stdin closed, `-protocol_whitelist file`
/// before `-i`, a wall-clock timeout with kill, bounded stderr. Unlike the
/// probe this WRITES a file, so a non-zero exit is a real failure (there is an
/// output). Returns `out` on success.
pub fn last_frame(bin: &Path, clip: &Path, out: &Path) -> Result<PathBuf, FfmpegError>;
```

**Exact argv** (a `last_frame_args(clip, out)` pure fn, unit-tested without spawning, mirroring
`probe_args`):

```
-nostdin  -hide_banner  -loglevel error  -y
-protocol_whitelist file        # BEFORE -i: refuse http/concat an input might chain to
-sseof -0.05                    # input seek: 0.05s before EOF (the last frame)
-i <clip>
-frames:v 1  -update 1
<out.png>
```

`-sseof`/`-protocol_whitelist` are **input** options and MUST precede `-i` (a test asserts the
ordering, like `probe_args_sandbox_the_untrusted_input`, `ffmpeg.rs:1130-1142`). `-sseof -0.05`
lands the demuxer just before EOF; `-frames:v 1 -update 1` writes exactly one image. The 0.05s
window is small enough to be the genuine last frame and large enough to survive a keyframe-sparse
stream (a `-sseof` too close to EOF can seek past the last keyframe and produce nothing).

**Error typing (AC1: "failure → typed error, never a silent skip").** Add
`FfmpegError::Render(String)` (the existing four are `NotFound`/`Spawn`/`Probe`/`Licence`; extract
and render share this new variant). A non-zero exit, a timeout, or a missing output file → `Render`
carrying the bounded stderr tail. The run helper (a `run_to_file` beside `run_probe`) is the same
timeout/kill/drain loop but checks `status.success()` **and** `out.is_file()`.

**Worked example (AC1 test, `--ignored`):** synthesise `testsrc=duration=2:size=1280x720:rate=10`
(the exact fixture the existing ignored probe uses, `ffmpeg.rs:1170-1194`), call
`last_frame(bin, clip, tmp.png)`, then `probe_file(bin, tmp.png)` → `MediaDims{width:Some(1280),
height:Some(720), ..}` and the PNG exists. A pure test asserts `last_frame_args` sandboxing.

**Output location.** `last_frame` takes an explicit `out` — the caller (the chain scheduler)
writes to `app_data_dir()/renders/<sid>/frames/shot<N>.png` (§D3 intermediates).

### D2 — Continuity chaining: a shell scheduler on a runner "settled" hook (AC2, AC6)

This is the riskiest piece; §10 flags it first. The requirement: in **chained** mode, shot N+1 is
submitted **only after** shot N completes and downloads, with shot N's last frame attached as
`MediaRole::Start`, through the S13 money-gated path; a failed shot stops the chain; it survives a
window close (S1 resume).

**D2.1 — Where the state lives (migration 9 on `storyboards`).** A chained storyboard stores
enough to advance itself after a crash: the full ordered shot payloads and a chained flag.

```sql
-- migration 9 (store.rs, after the version < 8 block; bump PRAGMA user_version = 9)
ALTER TABLE storyboards ADD COLUMN chained         INTEGER NOT NULL DEFAULT 0;  -- 0=all-at-once (S13), 1=chained
ALTER TABLE storyboards ADD COLUMN shots_json      TEXT;    -- Vec<SubmitInput> for the WHOLE plan, in order
ALTER TABLE storyboards ADD COLUMN overlap_seconds REAL;    -- the crossfade the plan was reconciled to (D4)
CREATE TABLE IF NOT EXISTS storyboard_renders (
    storyboard_id TEXT PRIMARY KEY,          -- one finished ad per set (re-render overwrites)
    output_path   TEXT NOT NULL,
    width         INTEGER, height INTEGER, seconds REAL,
    actual_usd    REAL,                       -- Σ member-shot actual at render time; NULL if any shot unpriced
    created_at    INTEGER NOT NULL
);
```

- `Storyboard` (`store.rs:25`) gains `chained: bool`, `shots_json: Option<String>`,
  `overlap_seconds: Option<f64>` — read with `unwrap_or_default`, added to the INSERT/`ON CONFLICT`
  set (these DO update: a re-generate of the set may re-store the plan). New store methods:
  `upsert_render(&StoryboardRender)`, `render(&str) -> Option<StoryboardRender>`,
  `renders() -> Vec<StoryboardRender>`. The v1-drop list gains the three columns + `DROP TABLE
  storyboard_renders`.
- **Why store `Vec<SubmitInput>` and not just re-derive from `plan_json`.** The plan `Shot`
  carries `prompt/seconds/camera/model_id` only; the user's per-shot resolution/aspect edits and a
  previewed `final_prompt` (S7 verbatim) live on the `SubmitInput`. Re-deriving would lose them and
  silently change what the user priced. Storing the exact payloads the UI would have submitted is
  the faithful, replayable record. Capped at `MAX_PLAN_JSON_BYTES`-class bound (§D6).
- **Not chosen: a `jobs.chain_after`/`depends_on` column + pre-created blocked rows.** Pre-creating
  provider job rows that are never watched invents phantom `Queued` jobs the resume path and the
  Library would both have to special-case. Deriving "the next shot" from `jobs_in_storyboard`
  (the highest present `shot_index`) is crash-safe and needs no cursor column. Recorded as Q1.

**D2.2 — The runner gains an `on_settled` hook.** `Runner::new` and `download_outputs` already know
the exact moment a job's bytes are local and the row is final (`runner.rs:235-239`,
`on_update(&job)`). Add an optional hook fired **once** there, only when the settled row carries a
`storyboard_id`:

```rust
// runner.rs
pub type OnSettled = Arc<dyn Fn(&JobSet) + Send + Sync>;
// Runner gains `on_settled: Option<OnSettled>`; download_outputs, after the final upsert+on_update,
// calls it iff job.storyboard_id.is_some() && job.is_settled() (Completed + every output local).
```

**D2.3 — The scheduler: `src-tauri/src/chain.rs`.** The hook is wired in `app.rs` to
`chain::advance(store, prices, library, ffmpeg_dir, runner_weak, &settled_job)`. To break the
Arc cycle (Runner holds the hook → the hook needs Runner to `watch` the next shot), **`AppState`
holds `runner: Arc<Runner>`** and the hook captures a `Weak<Runner>` (`upgrade()` at fire time;
`None` only during teardown, harmless). `advance`:

1. Load the storyboard; if `!chained`, return (S13 all-at-once sets never chain).
2. If the settled shot **failed** (`status != Completed` or no local output), **stop**: record a
   per-shot advisory on the *would-be-next* nothing (there is nothing to submit), leave the chain
   parked. No next submit — "chaining never submits a shot whose predecessor failed" (AC6).
3. Compute `next = settled.shot_index + 1`. If `next >= shots.len()` the chain is complete —
   nothing to submit (render is a separate, user-driven step, §D3).
4. Guard against a double-fire: if a row already exists at `shot_index == next`
   (`jobs_in_storyboard`), return (idempotent — the hook and the startup reconciliation must not
   both submit).
5. Extract the settled shot's **last frame**: `ffmpeg::last_frame(bin, &clip,
   &frames_dir.join(format!("shot{idx}.png")))`. On error, proceed **text-only** with an advisory
   (a missing continuity frame must not strand a paid set).
6. Build shot `next`'s `SubmitInput` from `shots_json[next]`; **if** its model serves i2v
   (`chainable_model(model)`, §D2.4) attach `MediaRef::local(MediaRole::Start, frame.png)` (the
   uploader turns it into `image_url` at submit, S9/S14); **else** leave it text-only and push
   `"continuity unavailable: <model> has no image-to-video route — generated from the prompt
   alone"` onto the shot's advisories.
7. Submit via the **existing money path**: look up the model, call `submit_one(state-pieces, model,
   &input, Some(&sid), Some(next))`, then `runner.watch`. This re-runs the S6 guard, the S7
   verbatim prompt, the **S10 estimate + `route::resolve` guard** on the shot as actually submitted
   — no chained shot bypasses the estimate or the guard (AC6). (`submit_one` today takes
   `&AppState`; §D2.6 records the small refactor to a pieces-struct so `chain.rs` can call it
   without a Tauri `State`.)

**D2.4 — i2v availability / fallback.** `chainable_model(model) -> bool` reuses existing
predicates: the model has a route whose Image endpoint binds a start frame —
`model.routes.iter().any(|r| media::resolve_endpoint(&r.slug, InputMode::Image, produces_video)
.is_ok() && media::can_bind(&model.spec, MediaRole::Start, media::dialect_for(r.provider, &r.slug),
&r.slug))`, i.e. exactly the same "start-frame binds on the i2v endpoint" test S14 relied on for H3
Max. `use_case::supports(model, UseCase::ImageToVideo)` is the coarser tab-level check the UI uses
for the Chain toggle default (§D5); `chainable_model` is the per-model authority the scheduler
gates on. When it is false the fallback (step 6) is text-only + advisory — never a silent
start-frame dropped into a route that ignores it (the exact `NO_END_FRAME`-class harm S9 guarded).

**D2.5 — Startup resume (S1).** A crash *between* shot N downloading and shot N+1 being submitted
leaves shot N settled → **not** in `unfinished()` → the hook never fires → the chain stalls. Add
`chain::resume_all(state)` called once at startup **after** `runner.resume_all()` (which re-attaches
still-running shots so their eventual settle fires the hook normally): scan every `chained`
storyboard, find its highest settled shot with no failed predecessor and no successor row, and
`advance` from it. So a chained set survives a window close exactly like a single job. A shot that
was mid-flight resumes via the ordinary poll loop; a shot that finished-but-never-advanced is picked
up here. Idempotency (D2.3 step 4) makes the two paths safe to overlap.

**D2.6 — `submit_one` reuse without a Tauri `State`.** `submit_one` currently takes
`state: &AppState` and touches `state.prices`, `state.store`, `state.runner`. `chain.rs` runs
outside a command, from the runner thread, so it has the Arcs but not a `State`. Refactor
`submit_one` to take the three Arcs it uses (`store: &Arc<SqliteStore>`, `prices: &Arc<Prices>`,
`runner: &Runner`) — a mechanical change; `submit_job`/`generate_storyboard_inner` pass
`&state.store, &state.prices, &state.runner`. The compile→guard→estimate→route→submit→watch body is
**unchanged** (S13's "single-job submit byte-for-byte" guarantee is preserved; the reuse-test still
guards it). Recorded as Q2 (alternative: a thin `submit_from_chain` duplicate — rejected, it would
drift from `submit_one`).

**D2.7 — Money gate under a moving price feed.** The **total** shown at generate time is the UI's
sum of per-shot `estimateCost` (S13, §D5). Each chained shot is *re-estimated at submit* by
`submit_one` from `state.prices` as it stands then; if the live feed drifted between generate and
the shot's turn, the shot's `estimated_usd` reflects the new price and the S10 estimated-vs-actual
meta rail surfaces the drift. **Decision: proceed, surface, do not block.** The user authorised the
set at the shown total; interrupting a chain mid-run for a re-confirm would strand paid predecessors
(the same invariant S13's best-effort-continue rests on). A per-shot price that has become *unknown*
(feed returns `None`) still submits — S10 already prices it as unknown, never $0 — and the advisory
says so. Recorded as Q3.

**D2.8 — Interaction with S13 retry.** S13's retry resubmits failed shots into the existing group
(`shot_indices` + `storyboard_id`). For a chained set, retrying a failed shot re-submits it; when it
settles the hook advances the chain from it as normal (D2.3). The retry path already refuses to
resubmit a non-failed shot (`commands.rs:1425-1434`), so it cannot double-run a chained shot that
already started. No new retry logic — the hook does the continuation. Recorded as Q4.

**D2.9 — Chained vs all-at-once at generate.** `StoryboardGenerateInput` gains
`#[serde(default)] chained: bool`. When `false` (default, S13 behaviour) `generate_storyboard_inner`
loops every shot immediately, unchanged. When `true`: store `chained=1` + `shots_json` +
`overlap_seconds`, submit **only shot 0** via `submit_one`, and return a `StoryboardResult` whose
later shots are marked "pending" (a new `ShotOutcome` variant field `pending: bool`, or absence from
`shots` — I use an explicit `pending` outcome so the UI can render the chain state). The hook and
resume drive the rest.

### D3 — Render: `render_storyboard` + `render.rs` + `ffmpeg::render` (AC3)

**D3.1 — The core spawn helper (`ffmpeg.rs`).**

```rust
/// Run a compositor argv to completion, streaming progress. Spawns the sidecar
/// with the argv (which already carries `-progress pipe:1`), feeds stdout to a
/// `ProgressReader`, and calls `on_progress` per completed block. Timeout scaled
/// to the render's own length (D6). A non-zero exit is `FfmpegError::Render`.
pub fn render(bin: &Path, args: &[String], timeout: Duration,
              mut on_progress: impl FnMut(Progress)) -> Result<(), FfmpegError>;
```

Reuses the `run_probe` skeleton (spawn, drain threads, deadline+kill, bounded **stderr** for the
error message) but reads **stdout** through `ProgressReader::push` and invokes `on_progress`, and
asserts `status.success()`. The argv comes from `compositor::Timeline::to_args` verbatim.

**D3.2 — The shell orchestration (`src-tauri/src/render.rs` + a `render_storyboard` command).**

```rust
#[tauri::command]
pub fn render_storyboard(app: AppHandle, state: State<'_, AppState>,
                         storyboard_id: String, options: RenderOptions)
    -> Result<StoryboardRenderDto, String>
```

`RenderOptions { overlap_seconds: Option<f64>, captions: bool }` (defaults from the stored
`overlap_seconds`/0.5, no captions). Steps:

1. `jobs_in_storyboard(&sid)` in `shot_index` order; require **every** shot `Completed` with a
   local video output (the Render button already gates this, but re-check server-side, mirroring
   S13's `generate_storyboard_inner` guards).
2. For each clip: `ffmpeg::probe_file(bin, &local_path)` → `MediaDims`. The **source duration** is
   needed for the crossfade offsets the compositor computes (`compositor.rs:345-388` documents why
   `joins()` needs the accumulated length). A clip whose duration cannot be probed → typed `Err`
   (never render a set whose timeline maths we cannot compute).
3. Build the `Timeline`: pick the output size from the clips (the max width×height across shots,
   even-rounded — reuse `compositor::even` semantics via `AspectRatio::dimensions` or a small
   helper) and fps 30; `Clip::new(path, 0.0, dur)` per shot; `.crossfade_in(overlap)` on every clip
   after the first (overlap default 0.5s, clamped by the compositor to `min(prev_acc, this_dur)`,
   `compositor.rs:364-372`); `OutputSpec::new(w,h,30, output_path)` with
   `encoder = ffmpeg::probe_encoders(bin)?` (VideoToolbox on this Mac); optional single plain-title
   caption when `options.captions` (a `Caption` built from the brief). **Single-shot case:** one
   `Clip`, no `crossfade_in` — `Timeline` emits `[c0]format=yuv420p[vout]` (copy/normalize only,
   `compositor.rs:926-931`), exactly what AC3's "single-shot storyboard needs no stitch" wants.
4. `t.validate()` — if non-empty, typed `Err` (the compositor's own timeline problems).
5. `ffmpeg::render(bin, &t.to_args(), timeout, on_progress)` where `on_progress` emits
   `app.emit("storyboard:render", RenderProgressDto{ storyboardId, percent, done })` — `percent =
   Progress::percent(t.total_duration())`, the one honest percentage in the app
   (`ffmpeg.rs:474-476`). This mirrors the `job:update` emit convention (`app.rs:115`).
6. Probe the output; **assert** `probed.seconds ≈ t.total_duration()` within ±0.2s (the AC3
   duration test does this on synth clips) and record `width/height/seconds`.
7. Register in the Library: write the output into `library.root()`'s day folder (a new
   `Library::adopt(local_produced_path, stem, created_at) -> PathBuf` that *moves/links* an already
   local file into the library, since `fetch` only downloads a URL). `Σ actual = sum of member
   `actual_usd`` (S10) — `None` if any shot is unpriced (never summed as $0, the S10/S13 rule).
   `upsert_render(StoryboardRender{ sid, output_path, w,h,seconds, actual_usd, created_at })`.
8. Return `StoryboardRenderDto` (camelCase) and emit a final `storyboard:render` `done:true`.

**Output naming:** `<brief-slug>-storyboard.mp4`, uniquified by `library::unique_path`. Intermediate
normalises ffmpeg does in-graph (no temp files); the only intermediates on disk are the extracted
frames (§D1/D3 under `app_data_dir()/renders/<sid>/frames/`), cleaned after a successful render
(§D6).

**D3.3 — Duration arithmetic test (AC3).** `--ignored`, synth three `testsrc` clips of 4/4/4s at a
common size; render with overlap 1.0s; probe → `12 − 2×1 = 10s` within ±0.2s; and the single-shot
case → the clip's own duration. Pure tests assert `Timeline::total_duration()` already (compositor
has them, `compositor.rs:957,984`); the ignored test proves the sidecar produces a file matching the
maths.

### D4 — Target-length arithmetic at plan time (AC4)

The stitched result is `Σ seconds − (N−1)·overlap`, not `Σ seconds`. S13's `validate`
(`storyboard.rs:247`) checks the raw sum. **Decision:** add a pure
`storyboard::stitched_duration(shots, overlap) -> f64` and make the plan-time ±10% gate use the
**stitched** duration.

- `DEFAULT_CROSSFADE_SECONDS: f64 = 0.5` (a new const in `storyboard.rs`, the single source both
  plan-time and render-time read; `RenderOptions.overlap_seconds` defaults from it).
- `validate` gains an `overlap: f64` parameter and checks `(stitched_duration(shots, overlap) −
  target).abs() ≤ 0.10·target`. `plan` passes `DEFAULT_CROSSFADE_SECONDS`; the shell
  `generate_storyboard_inner` re-validation passes the stored `overlap_seconds` (or the default).
  **Backward-compat check:** S13's existing tests still pass — `a_valid_plan_is_accepted`
  (8+8+8+6=30, stitched 30−1.5=28.5 ∈ [27,33] ✓) and `a_sum_outside_ten_percent_is_rejected`
  (4×5=20, stitched 20−1.5=18.5, still ≪27, still rejected ✓). Call sites that pass `0.0` (a hard
  cut) get the raw sum, so the parameter is a strict generalisation.
- **Clamp interaction (AC4 "clamp interactions").** `plan` validates the *proposed* seconds
  (drafting gate), then `clamp_shot` narrows each shot toward the model's axis. After the clamp,
  recompute the stitched duration; if it now drifts beyond ±10%, push a **plan-level advisory**
  onto the first shot ("clamped shots run ~{x}s; target was {t}s") rather than failing — the clamp
  is a model constraint the user cannot override, so failing would strand a well-drafted plan. The
  AC4 test asserts a normal plan's stitched (post-clamp) lands within ±10%.

**Worked example (AC4):** target 30, overlap 0.5, four shots 8/8/8/6 → stitched 30−1.5 = 28.5;
`|28.5−30| = 1.5 ≤ 3` → accepted, and the rendered file (D3) probes to 28.5±0.2s.

### D5 — UI (AC5)

Additive on S13's `Storyboard.tsx` and `Library.tsx`; test-locked class names from S13
(`ModelPicker`/`PromptPreview`/library grouping) unchanged.

- **Chain toggle** (`.storyboard-chain-toggle`, a checkbox). Default **on** when every shot's model
  has an i2v route — the UI already holds each model's capabilities; the DTO gains a per-shot
  `chainable: boolean` computed server-side in `plan_storyboard` (from `chainable_model`, §D2.4) so
  the toggle default cannot drift from what the scheduler will actually do. When any shot is not
  chainable the toggle defaults **off** with a one-line note ("some shots' models can't continue
  from a frame — they'll be generated from the prompt"). `generateStoryboard` passes `chained`.
- **Render button** (`.storyboard-render`, reuse S13's generate-button styling). Enabled only when
  every member job is `Done` (Completed + downloaded) — derived from the grouped jobs the Library
  already has. Calls `renderStoryboard({ storyboardId, options })`.
- **Progress bar** (`.storyboard-render-progress`, `role="progressbar"`). Subscribes to
  `listen("storyboard:render")`; `percent` drives the width; `done` hides it and reveals **Open
  result**.
- **Open result** (`.storyboard-open-result`): `openPath(render.outputPath)` via the existing
  opener permission (no capability change).
- **Library set item** (`Library.tsx`, additive): when a storyboard has a `render` (from a new
  `listStoryboardRenders()` / folded into `listStoryboards()`), the group header shows the rendered
  ad tile with **Σ actual** (`totalCostCaption` of the member `actualUsd`, reusing S13's helper) and
  its duration. A set with no render yet renders exactly as S13.
- **New `api.ts`**: `renderStoryboard`, `listStoryboardRenders` (or extend `listStoryboards`);
  `!isDesktop()` mocks. **New `types.ts`**: `StoryboardRender`, `RenderProgress`, `RenderOptions`;
  `chained` on the generate input; `chainable` on `Shot`.
- **vitest (AC5 states):** Render disabled until all shots Done → enabled; a `storyboard:render`
  event advances the progress bar; `done` shows Open result; the Chain toggle defaults on when all
  chainable and off (with the note) when not; the Library set item shows Σ actual (and "unpriced"
  when a member is unpriced). No RTL — the repo's `react-dom/client` + jsdom pattern.

### D6 — Safety (D6)

- **No shell, ever.** The render and extract spawn `ffmpeg` via `std::process::Command` with an
  **argv vector** (`Timeline::to_args` / `last_frame_args`), never a shell string — the same as the
  S10 probe. The webview never invokes ffmpeg (the `shell:allow-execute` capability is not used by
  this path).
- **Protocol whitelist.** `last_frame` already whitelists `file`. `Timeline::to_args` does **not**
  today; add `-protocol_whitelist file` to the compositor's global input options
  (`compositor.rs:558-569`) so a downloaded clip cannot chain to `http`/`concat`. This is a small
  compositor change; the existing `contains`-style argv tests are unaffected (they assert presence
  of specific flags, not exclusivity). Flagged in §10.
- **Timeouts scaled to total duration.** The render timeout is `base + k·total_seconds` (e.g.
  `Duration::from_secs(60) + total·30` — a 30s ad at ~0.5–2× realtime on VideoToolbox finishes far
  inside this; a wedged encode is killed). `last_frame` reuses the probe's fixed 20s ceiling (a
  single-frame decode).
- **Disk usage bound.** Refuse a render whose summed input clip bytes exceed a cap
  (`MAX_RENDER_INPUT_BYTES`, e.g. 4 GiB) — a storyboard is short clips, so this only catches a
  pathological set. The output is a single H.264 file bounded by CRF.
- **Cleanup.** The `app_data_dir()/renders/<sid>/frames/` intermediates are removed after a
  successful render (and best-effort on failure); the final ad is the only lasting artefact, in the
  Library.

### D7 — Real-sidecar `--ignored` tests (D7)

The pure work is exhaustively unit-testable off captured strings / argv (as S10/compositor already
are). The three genuinely end-to-end facts — `last_frame` writes a correct PNG, `render` produces a
file, and the stitched duration equals the maths — need the real sidecar, so they are `#[ignore]`d
in `crates/hickeyfield-core/src/ffmpeg.rs` (and `tests/` if a render fixture is cleaner), each
**synthesising** its inputs with `testsrc`/`lavfi` (no downloaded media, no network), locating the
sidecar exactly as the existing ignored probe does (`CARGO_MANIFEST_DIR/../../src-tauri/binaries`,
`ffmpeg.rs:1174-1175`). **The verifier runs them with:**

```
cargo test -p hickeyfield-core --ignored           # spawns the bundled sidecar (fetch-ffmpeg.sh)
```

They skip cleanly if the sidecar is absent (the `locate` error is asserted-on in the keyless suite,
`ffmpeg.rs:1073-1078`, and the ignored tests `.expect(...)` the binary — run only under `--ignored`
on a Mac that has fetched it, as AC7 requires).

---

## 2. Files touched (complete; nothing else changes)

**Core (`crates/hickeyfield-core/src/`):**
- `ffmpeg.rs` — `FfmpegError::Render`; `last_frame` + `last_frame_args` + `run_to_file`; `render`
  (progress-streaming spawn); tests (pure argv/sandbox + `--ignored` real-sidecar).
- `compositor.rs` — add `-protocol_whitelist file` to `to_args` global input options (§D6). Nothing
  else; the graph builder is reused as-is.
- `storyboard.rs` — `DEFAULT_CROSSFADE_SECONDS`; `stitched_duration`; `validate` gains an `overlap`
  param; `plan` passes the default and adds the post-clamp drift advisory; tests.

**Shell (`src-tauri/src/`):**
- `store.rs` — migration 9 (`chained`/`shots_json`/`overlap_seconds` on `storyboards`,
  `storyboard_renders` table); `StoryboardRender` struct; `Storyboard` gains three fields;
  `upsert_render`/`render`/`renders`; INSERT/`ON CONFLICT`/row-map + v1-drop updates; tests.
- `runner.rs` — `OnSettled` hook type; `Runner` holds `on_settled`; `download_outputs` fires it on a
  settled storyboard shot; a runner test that the hook fires once with the settled row.
- `chain.rs` — **new**: `advance(...)`, `resume_all(state)`, `chainable_model(model)`; tests with a
  fake store/runner.
- `render.rs` — **new**: `render_storyboard` command, `RenderOptions`/`RenderProgressDto`/
  `StoryboardRenderDto`, the Timeline build + Library adopt + render-row write; tests (pure Timeline
  build; the sidecar run is the core `--ignored` test).
- `commands.rs` — `submit_one` takes Arc pieces not `&AppState` (§D2.6); `StoryboardGenerateInput`
  gains `chained`; `generate_storyboard_inner` chained branch (submit shot 0, store chain state);
  `ShotOutcome` gains `pending`; `list_storyboard_renders` (or extend `list_storyboards`).
- `library.rs` — `Library::adopt` (move/link a locally-produced file into the library); test.
- `app.rs` — `AppState.runner: Arc<Runner>`; wire the `on_settled` hook to `chain::advance` with a
  `Weak<Runner>`; call `chain::resume_all` after `runner.resume_all`; `render`/`chain` modules
  declared.
- `lib.rs` — register `render_storyboard` (+ `list_storyboard_renders` if separate); `mod render;
  mod chain;`.

**UI (`ui/src/`):**
- `components/Storyboard.tsx` — Chain toggle, Render button, progress bar, Open result.
- `components/Library.tsx` — the rendered-ad set item + Σ actual.
- `App.tsx` — `storyboard:render` subscription; render/chain state; pass renders to Library.
- `api.ts` — `renderStoryboard`, `listStoryboardRenders`; `chained` on generate; mocks.
- `types.ts` — `StoryboardRender`, `RenderProgress`, `RenderOptions`; `chained`; `chainable`.
- `base.css` — `.storyboard-render*`/`.storyboard-chain-toggle`/set-item styles (additive).
- Tests: `ffmpeg`/`compositor`/`storyboard` core; `store`/`runner`/`chain`/`render`/`library`
  shell; `Storyboard.test.tsx`, `Library.test.tsx`, `api.test.ts`.

**Docs:** `CHANGELOG.md`, `README.md`, `.forge/slices/S15-storyboard-render/document.md`,
`.forge/backlog.md` (record the follow-ups in §8).

**Not touched (recorded):** `engine.rs` `apply_poll`/poll logic — **N/A, the render is not a
provider job**; the runner's poll loop is unchanged (only the settled hook is added). The enhancer /
S13 plan validator's model/camera rules — **N/A, reused verbatim** (only `validate`'s overlap arg is
new). `media::bind`/`fal_keys`/`resolve_endpoint` — **N/A, reused** (the chain attaches a
`MediaRef::Start` the existing binder already handles). `tauri.conf.json`/`capabilities/default.json`
— **N/A**: the render spawns the sidecar from Rust (`externalBin` already declared) and "Open
result" uses the already-granted opener permission; no fs-scope or sidecar-permission change
(confirm at IMPLEMENT — if the asset protocol must show the file, that is a `scope` addition, noted
in Q5).

---

## 3. Data flow (a plan → one finished ad on disk)

**Chained generate:** Storyboard.tsx (Chain on) → `generateStoryboard({..., chained:true})` →
`generate_storyboard_inner` stores `chained=1`+`shots_json`+`overlap`, mints the group, submits
**shot 0** via `submit_one` (full money gate) → the runner polls + downloads shot 0 → the
`on_settled` hook fires → `chain::advance` extracts shot 0's last frame
(`ffmpeg::last_frame`), attaches it as `MediaRole::Start` to shot 1 (i2v) or falls back text-only +
advisory, submits shot 1 via `submit_one` → … → shot N settles, chain complete. A window close mid-
chain resumes via `runner.resume_all` (in-flight shot) + `chain::resume_all` (settled-but-not-
advanced).

**Render:** every shot Done → Render button → `renderStoryboard({storyboardId})` →
`render.rs`: probe each clip → build `compositor::Timeline` (normalize → N−1 crossfades → concat/
`[vout]`) → `ffmpeg::render` streams `Progress` → `app.emit("storyboard:render", …)` →
Storyboard.tsx progress bar → output probed (duration ≈ Σ − Σ overlaps) → `Library::adopt` into
`~/Movies/Hickeyfield` → `upsert_render` (Σ actual from member `actual_usd`) → Library set item
shows the finished ad + Σ actual; Open result opens it.

---

## 4. Test plan — every AC to a named test

Rust tests live in `#[cfg(test)] mod tests` beside the code; `--ignored` real-sidecar tests
synthesise their inputs. UI tests use the repo's `react-dom/client` + jsdom pattern.

| AC | test | file | asserts |
|---|---|---|---|
| AC1 | `last_frame_args_sandbox_the_input_and_seek_from_eof` | `ffmpeg.rs` | `-protocol_whitelist file` before `-i`; `-sseof -0.05` before `-i`; `-frames:v 1 -update 1`; `-nostdin`; output path last |
| AC1 | `last_frame_writes_a_png_with_the_clips_dimensions` (`--ignored`) | `ffmpeg.rs` | synth `testsrc 1280x720`; `last_frame` → PNG exists; `probe_file` → 1280×720 |
| AC1 | `last_frame_failure_is_a_typed_render_error` | `ffmpeg.rs` | a missing input / non-zero exit → `FfmpegError::Render`, never `Ok` with no file |
| AC2 | `advance_submits_the_next_shot_after_a_settled_one` | `chain.rs` | a fake settled shot 0 (with a local clip) → `advance` extracts a frame, attaches Start, submits shot 1 tagged `shot_index 1` |
| AC2 | `a_failed_shot_stops_the_chain` | `chain.rs` | settled shot with `status=Failed`/no output → no next submit; no row at `next` |
| AC2 | `a_model_without_i2v_falls_back_text_only_with_an_advisory` | `chain.rs` | `chainable_model==false` → next shot submitted with no Start media + a continuity advisory |
| AC2 | `advance_is_idempotent_when_the_next_shot_exists` | `chain.rs` | a row already at `next` → `advance` no-ops (hook + resume cannot double-submit) |
| AC2 | `chain_resume_advances_a_settled_but_unadvanced_set` | `chain.rs` | shot 0 settled, no shot 1 → `resume_all` advances; a fully-submitted chain is left alone |
| AC2 | `the_settled_hook_fires_once_for_a_storyboard_shot` | `runner.rs` | `download_outputs` on a settled `storyboard_id` job calls `on_settled` once; a standalone job never does |
| AC3 | `render_builds_a_normalize_crossfade_concat_timeline` | `render.rs` | N probed clips → a `Timeline` with N `[c]` normalises, N−1 xfades, `[vout]`; single clip → no xfade/concat |
| AC3 | `render_stitches_to_the_expected_duration` (`--ignored`) | `ffmpeg.rs`/`render` | 3× `testsrc 4s`, overlap 1s → output probes 10±0.2s; dims match; single-shot → clip duration |
| AC3 | `render_registers_a_library_item_with_sum_actual` | `render.rs`/`store.rs` | after a (mocked) render, `upsert_render` stores the path + Σ member `actual_usd`; an unpriced member → `actual_usd None` not 0 |
| AC4 | `stitched_duration_subtracts_the_overlaps` | `storyboard.rs` | `stitched_duration([8,8,8,6], 0.5) == 28.5` |
| AC4 | `plan_time_gate_uses_the_stitched_duration` | `storyboard.rs` | overlap-aware `validate` accepts 28.5-vs-30, rejects a stitched 18.5-vs-30; S13's two sum tests still pass |
| AC4 | `clamp_drift_is_advised_not_failed` | `storyboard.rs` | a plan whose clamp pushes stitched past ±10% is accepted with a plan-level advisory |
| AC5 | `render_button_enables_only_when_every_shot_is_done` | `Storyboard.test.tsx` | disabled with a pending shot; enabled when all Done |
| AC5 | `a_render_event_advances_the_progress_and_reveals_open_result` | `Storyboard.test.tsx` | `storyboard:render` percent → bar; `done` → Open result |
| AC5 | `the_chain_toggle_defaults_on_only_when_every_shot_is_chainable` | `Storyboard.test.tsx` | all chainable → on; one not → off + note |
| AC5 | `the_library_set_item_shows_the_sum_actual` | `Library.test.tsx` | a rendered set shows the ad + Σ actual; unpriced member → "unpriced" |
| AC6 | `no_chained_shot_bypasses_the_estimate_or_guard` | `chain.rs`/`commands.rs` | `advance` routes through `submit_one` (estimate+`route::resolve`); reuse `submit_one` unchanged reuse-test |
| AC6 | `rendering_costs_nothing` | `render.rs` | the render writes no `estimated_usd`/`actual_usd` of its own; the render row's `actual_usd` is the **sum of members**, and the render spawns only the sidecar |
| AC6 | `a_failed_predecessor_never_submits_its_successor` (reuse AC2 `a_failed_shot_stops_the_chain`) | `chain.rs` | — |
| AC7 | the gates (§5) | — | 0 failed, `cargo` ≥ 865, `pnpm` ≥ 285; `--ignored` ffmpeg tests pass on this Mac; CHANGELOG; `.app` builds + launches |

**Non-vacuity / mutation checks** (named so they are not invented late): drop
`-protocol_whitelist` from `last_frame_args` ⇒ the sandbox test fails; make `stitched_duration`
return the raw sum ⇒ `stitched_duration_subtracts_the_overlaps` fails; drop the hook's `next`-exists
guard ⇒ `advance_is_idempotent…` fails (a double submit); make `advance` submit on a Failed
predecessor ⇒ `a_failed_shot_stops_the_chain` fails; make the render probe assertion ±5s instead of
±0.2s ⇒ `render_stitches_to_the_expected_duration` stops catching an off-by-one-overlap.

**AC7 screenshots:** as S12/S13 — no automated screenshot tool; the VERIFY agent launches the
`tauri build` `.app`, drafts+chains+renders a set on synth-safe inputs (or narrates the render UI),
and captures/describes it in `test.md`.

---

## 5. Gate commands (AC7), exact

Node 24 at
`/private/tmp/claude-501/-Users-jorge-Documents-GitHub-hickeyfield/79be25e2-8f71-4192-9e6d-cd0e9d068f1c/scratchpad/node-v24.8.0-darwin-arm64/bin`
(prepend to `PATH`).

```
# dummy keys so shell tests that read a key env do not skip/panic:
export hickeyfield_FAL_KEY=x hickeyfield_HIGGSFIELD_KEY=x hickeyfield_HIGGSFIELD_SECRET=x
cargo test --workspace                                   # 0 failed, >= 865 + new tests
cargo test -p hickeyfield-core --ignored                 # the real-sidecar last_frame + render tests (Mac w/ fetched ffmpeg)
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
./scripts/lint-provenance.py                             # no new shipped prose — 0
env -u hickeyfield_FAL_KEY -u FAL_KEY cargo run -q -p hickeyfield-core --example audit_fal  # PINNED 0 / UNREACHABLE 0 unchanged (no schema/mode logic touched)
export PATH="/private/tmp/claude-501/-Users-jorge-Documents-GitHub-hickeyfield/79be25e2-8f71-4192-9e6d-cd0e9d068f1c/scratchpad/node-v24.8.0-darwin-arm64/bin:$PATH"
cd ui && pnpm test && pnpm build                         # vitest 0 failed >= 285; tsc + vite build
ui/node_modules/.bin/tauri build --bundles app           # from repo root; the .app launches (window, not blank)
```

`cargo deny check` — **N/A to re-argue** (no dependency/manifest change; the render reuses
`serde_json`/`reqwest`/std already present) but MUST stay green. **Never** `first_light` (spends
money, no key). **Never** bare `cargo build` for a runnable app.

---

## 6. Order of work (safest seams first; each step leaves the workspace green)

1. **Core `ffmpeg::last_frame` + `render` + `FfmpegError::Render`** (§D1/D3.1): pure argv/sandbox
   tests + the `--ignored` real-sidecar tests. No shell, no UI — the two novel spawns proven first,
   the same way S10 proved the probe.
2. **Compositor `-protocol_whitelist` addition** (§D6) + `storyboard::stitched_duration` +
   overlap-aware `validate` + clamp-drift advisory (§D4). Pure; keeps S13 tests green.
3. **Store migration 9** (§D2.1): `chained`/`shots_json`/`overlap_seconds` + `storyboard_renders`;
   round-trip + `user_version==9` test.
4. **Runner `on_settled` hook** (§D2.2) + `AppState.runner: Arc<Runner>` + `submit_one` pieces
   refactor (§D2.6). Prove the hook fires once and single-job submit is unchanged.
5. **`chain.rs`** (§D2.3–D2.5) + `generate_storyboard_inner` chained branch (§D2.9) + `app.rs`
   wiring + startup resume. The behavioural heart of AC2/AC6.
6. **`render.rs`** (§D3.2) + `Library::adopt` + the `render_storyboard` command + registration.
7. **UI** (§D5): api.ts + types.ts, then Storyboard/Library, then App wiring + the render event.
8. **Docs** (§7), then `tauri build` + screenshots.

This is the contract's order (`slice.md:56-57`: extract → render → chaining → UI) with the pure
core spawns pulled first, because they de-risk the two genuinely new sidecar behaviours before any
shell/UI depends on them.

---

## 7. Documentation (the DOCUMENT step, after code lands)

- **`CHANGELOG.md`** — under `## [Unreleased]` `### Added`: a storyboard can now be chained (each
  shot continues from the previous shot's last frame, when the model does image-to-video, else from
  the prompt with a note) and rendered into one finished ad — crossfaded and stitched through the
  bundled ffmpeg with a real progress bar — that opens from the Library with its total actual cost.
  Note (money honesty): rendering spends nothing; chaining runs the same per-shot estimate + guard
  as a single generation, and never submits a shot whose predecessor failed.
- **`README.md`** — if it lists the storyboard surface (S13 added a bullet), extend it with the
  chain + render step and the "rendering is free / no shot runs without the total shown" note; else
  **N/A**.
- **`docs/`** — a short note where the ffmpeg sidecar / editing surface is described that the
  compositor now has a live render caller (closing the objective §D "missing UI caller"); the
  ffmpeg **license** release-blocker (objective §Hard external blockers) is re-stated as still
  unresolved (this slice ships no redistribution). BRIDGE/PARITY/FIRST-LIGHT unchanged.
- **`.forge/backlog.md`** — record the Non-goals (§8) as follow-ups.
- **`document.md`** — written after docs land, recording file + section for each change and the
  weakest parts for the verifier.

---

## 8. Non-goals (recorded, not skipped — each a backlog follow-up)

- **N1.** Audio: no ducking, music bed, or narration. The compositor's audio graph
  (`compositor.rs:472-549`) is left unused by the render (generated clips are mute; `to_args` emits
  no audio flags when `audio` is empty). Follow-up.
- **N2.** Captions beyond a single optional plain title. Timed/multi captions are a later editing
  surface; the compositor already supports them, but the render surfaces only a title.
- **N3.** Cloud/remote rendering — the render is local-only, on the bundled sidecar.
- **N4.** Extend-model chaining (Veo `veo3_1_extend` fixed 7s/720p, `grok_extend_video`). Chaining
  here is **last-frame → start-frame i2v**, not the provider's own extend endpoint. Follow-up.
- **N5.** No change to the S13 plan/generate money path, the poll/download/reattach lifecycle
  (`apply_poll`), the enhancer, or single-job submit behaviour (`submit_one` body verbatim).
- **N6.** No new dependency, no redistribution of ffmpeg (the "educational use only" macOS build
  license blocker stands, objective §Hard external blockers — documented, not resolved).

---

## 9. Open questions (each with the default this build assumes)

- **Q1 — Dependency representation: `shots_json` on the storyboard, or `jobs.chain_after` + pre-
  created blocked rows?** **Default: `shots_json` + derive the next shot from `jobs_in_storyboard`**
  (§D2.1) — crash-safe, no phantom `Queued` rows for the resume path and Library to special-case.
  Reversible if a future feature needs a first-class job DAG.
- **Q2 — `submit_one` refactor to Arc pieces, or a `submit_from_chain` duplicate?** **Default:
  refactor `submit_one`** (§D2.6) so the chain and commands share one audited money path; a
  duplicate would drift. The single-job-unchanged reuse-test still guards the body.
- **Q3 — Price drift mid-chain: proceed, or re-confirm?** **Default: proceed, surface via the S10
  meta rail, do not block** (§D2.7) — the user authorised the shown total; blocking strands paid
  predecessors. A future "pause the chain if a shot's estimate jumps >X%" is a follow-up.
- **Q4 — Chained retry: does the hook resume the chain from a retried shot?** **Default: yes** (§
  D2.8) — retry re-submits the failed shot and the settled hook advances from it; the existing
  refuse-to-resubmit-a-started-shot guard prevents double runs.
- **Q5 — Does "Open result" need an asset-protocol fs-scope entry?** **Default: no** — the opener
  permission (`opener:allow-open-path`) already opens a Library file in the OS, and existing
  generation results open the same way. If the UI must *inline-preview* the rendered file via
  `asset:`, add the Library dir to `assetProtocol.scope` — a one-line capability change, noted for
  IMPLEMENT to verify against how the Library already previews clips.
- **Q6 — Final output under app_data_dir (contract) or the Library (~/Movies)?** **Default: the
  Library** (§D3, correcting the contract) — it is a user-facing finished ad and `library.rs`
  refuses `Application Support` on purpose; only ephemeral frames/temps go under `app_data_dir()`.
  If the owner wants renders quarantined from the Library, one path constant changes it.
- **Q7 — Output size/fps: max-of-clips@30, or a user choice / target-tier?** **Default: max clip
  dimensions (even-rounded) at 30fps** — normalises to the highest-fidelity source without
  upscaling smaller clips beyond it. A resolution/fps picker is a later render-options surface.

---

## 10. Riskiest parts (where the verifier should start)

1. **The chain scheduler and the Arc cycle (§D2).** This is the behavioural heart and the most
   invasive change: `AppState.runner` becomes `Arc<Runner>`, a `Weak<Runner>` lives inside a runner
   callback, and correctness depends on the hook firing **exactly once** per settled shot **and**
   being idempotent with the startup reconciliation. Two ways it breaks silently: a chain that
   double-submits (hook + resume both fire → two paid rows at one index) or a chain that stalls (a
   settled-but-unadvanced set the resume scan misses). `advance_is_idempotent…` and
   `chain_resume_advances…` are the guards; the verifier should confirm both are non-vacuous and
   that `Weak::upgrade` failing (teardown) cannot panic. **Weakest link.**
2. **`submit_one` pieces refactor (§D2.6).** If the extraction drifts from the current body, every
   single-job submit changes silently. The S13 "single-job submit unchanged" reuse-test is the
   guard; the verifier should diff the new `submit_one` against the old body and confirm only the
   parameter shape changed.
3. **Last-frame `-sseof` reliability (§D1).** `-sseof -0.05` can, on a keyframe-sparse clip, seek
   past the last keyframe and emit nothing (a `Render` error) or a near-last frame. The generated
   clips this chains are short and dense, so it is sound in practice, but the verifier should run
   the `--ignored` `last_frame` test on the real sidecar and confirm the PNG is genuinely the tail
   frame (dimensions match, not black). A too-small window is the classic failure.
4. **The render timeline maths against the real sidecar (§D3).** The pure `total_duration` is
   already proven by the compositor's own tests; the risk is the *sidecar's* output drifting from
   it (an off-by-one crossfade eats a shot, `compositor.rs:961-985` documents exactly this class).
   The `--ignored` ±0.2s duration assertion is the only thing that catches it; the verifier must run
   it, not trust the pure test.
5. **The render button / all-shots-Done gate lives in the UI (§D5), re-checked server-side (§D3).**
   `render_storyboard` re-verifies every shot is Completed+local, so a direct caller cannot render a
   half-finished set — but the verifier should confirm the server guard exists and is not only the
   UI's `disabled`. Same shape as S13's server-side generate guards.
6. **`Library::adopt` moving a local file (§D3).** `fetch` only downloads a URL; adopting a locally-
   produced render is new. A cross-device rename (app_data_dir vs ~/Movies on different volumes)
   would fail an `fs::rename` — the helper must fall back to copy+remove. The verifier should check
   the fallback exists.
