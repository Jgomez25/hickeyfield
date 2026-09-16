# S15-storyboard-render — TEST (evidence)

Author: adversarial verifier (a different agent than the implementer). Every command below
was run by me on this Mac; the pasted output is real, not copied from `implement.md`.

## Implementer's claim (verbatim excerpts from implement.md)

> - `cargo test --workspace`: **888 passed, 0 failed** (865 + 23 new).
> - `cargo test -p hickeyfield-core --lib -- --ignored last_frame_writes render_stitches`:
>   **2 passed, 0 failed** on this Mac with the real sidecar …
> - `cargo fmt --all --check`: clean. `cargo clippy … -D warnings`: clean (0 warnings).
> - `./scripts/lint-provenance.py`: PASS … keyless `audit_fal`: **PINNED 0 / UNREACHABLE 0**.
> - `cd ui && pnpm test`: **292 passed**, `pnpm build`: tsc + vite OK.
> - `last_frame` uses `-sseof -3` + `-update 1` (after `-sseof -0.05` failed on a keyframe-sparse clip).
> - a chained set stores `overlap_seconds = 0.0` so a continuity join renders as a hard cut;
>   the Render button/progress/Open-result live in the Library set item, the Chain toggle in Storyboard.
> - **D-G: shot floor `3..=8` → `1..=8`.**

## Commands (each run by me; real output + exit codes)

### AC7 — `cargo test --workspace` (dummy keys set)
```
$ cargo test --workspace
… (per-crate "test result: ok" lines summed)
SUM of "N passed"           = 888
SUM of "N failed"           = 0
```
Exit 0. Matches the claim (888 passed, 0 failed).

### AC1/AC3/AC7 — real-sidecar `--ignored` tests (I ran them; sidecar 51,576,912 bytes present)
```
$ cargo test -p hickeyfield-core --lib -- --ignored last_frame_writes render_stitches
test ffmpeg::tests::last_frame_writes_a_png_with_the_clips_dimensions ... ok
test ffmpeg::tests::render_stitches_to_the_expected_duration ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 735 filtered out; finished in 2.40s
```
The render test crossfades three 4s clips at 1s each and asserts the probed output is 10±0.2s,
plus a single clip → 4±0.2s. Both real on this Mac's VideoToolbox/libx264.

### AC7 — fmt / clippy / provenance / audit_fal
```
$ cargo fmt --all --check                       → exit 0 (clean)
$ cargo clippy --workspace --all-targets -- -D warnings → exit 0; 0 "warning:" lines
$ ./scripts/lint-provenance.py                  → CDN hostnames PASS; Copy provenance PASS (80015 shingles)
$ env -u hickeyfield_FAL_KEY -u FAL_KEY cargo run -q -p hickeyfield-core --example audit_fal
  === PINNED FIELDS (0) ===   === BAD ROUTE CONSTANTS (0) ===   === UNREACHABLE (0) ===
  (pre-existing SILENTLY-IGNORED 11 / MISSING 2 / DROPPED 21 / REJECTED 9 unchanged — no schema/route-constant logic touched)
```
PINNED 0 / BAD ROUTE CONSTANTS 0 / UNREACHABLE 0 — the categories D-C could have regressed are clean.

### AC5/AC7 — UI
```
$ cd ui && pnpm test   → Test Files 23 passed (23); Tests 292 passed (292); exit 0
$ pnpm build           → tsc --noEmit && vite build → ✓ built in 1.86s; exit 0
```

## Break attempts

### AC1 — last-frame extraction against the real sidecar (probed the argv directly)
- **Keyframe-sparse (`-g 250`, the hostile case):** the `--ignored` test passes → PNG 1280×720.
- **0.5s clip (`testsrc=duration=0.5 … -g 250`):** ran `last_frame`'s exact argv
  (`-nostdin -hide_banner -loglevel error -y -protocol_whitelist file -sseof -3 -i clip -update 1 out.png`);
  exit 0, PNG written, `ffprobe` → `320x240` (correct dims; `-sseof -3` on a sub-3s clip clamps to
  start and decodes to EOF, so the true last frame survives).
- **Corrupt file (2 KB of /dev/urandom):** exit 183, **no PNG produced**. `run_to_file` maps a
  non-zero exit to `FfmpegError::Render` — a typed error, never a silent skip. AC1 satisfied.
- **Scaling path (`size=Some`):** `-vf scale=1280:720:flags=lanczos` on the 320×240 clip → PNG
  `1280x720`. The set-canvas scaling works.

### AC2/AC6 — chain scheduler (read + mutation-tested in a git worktree OUTSIDE the repo)
- The runner `on_settled` hook fires only when `job.storyboard_id.is_some() && job.is_settled()`
  (Completed + all outputs local); `the_settled_hook_fires_once_for_a_storyboard_shot` asserts one
  fire for a set shot and zero for a standalone job.
- `advance` is idempotent (no-ops if a row exists at `next`), stops on a non-Completed predecessor
  and on a missing clip, and submits the next shot through `submit_one` (the same S6/S7/S10 money
  path). `submit_one` upserts the Queued row *before* the network call, so the guard tests are
  observable without a live provider.
- `prepare_next_shot` attaches the frame as `MediaRole::Start` + sets `constant_overrides`
  (expansion off + seed) only for an i2v model with a frame; else text-only + advisory.

### AC3/AC4/D-B — render + duration arithmetic
- `render.rs` decides each join per-shot: `continuity = sb.chained && shot has Start media` → hard
  cut; else crossfade when `overlap > 0`. A chained set stores `overlap_seconds = 0.0`, so its
  joins render as hard cuts (Σ, all paid seconds kept); a crossfaded set is Σ − Σ overlaps
  (proven 10s for 3×4−2×1 on the real sidecar). Output is adopted into the Library
  (`~/Movies/...`), a documented Q6 deviation from the contract's app_data_dir wording;
  intermediates cleaned, frames kept.
- Pure tests confirm 2×15 hard cut = 30.0s, crossfaded = 29.5s.

### Mutation testing (≥5) — non-vacuity proof, all in a throwaway worktree
| # | mutation | test that should catch it | result |
|---|---|---|---|
| 1 | `stitched_duration` ignores overlap (`d=0.0`) | `stitched_duration_subtracts_the_overlaps` | **FAILED** (caught) |
| 2 | shot floor `1..=8` → `3..=8` | `the_shot_count_bound_is_one_to_eight` | **FAILED** (caught) |
| 3 | drop `-protocol_whitelist file` from `last_frame_args` | `last_frame_args_sandbox_the_input_and_seek_from_eof` | **FAILED** (caught) |
| 4 | make `apply_overrides` ignore the schema gate | `an_override_cannot_inject_a_field_the_endpoint_rejects` | **FAILED** (caught) |
| 5 | remove `advance` idempotency guard | `advance_is_idempotent_when_the_next_shot_exists` | **FAILED** (caught — proves `advance` genuinely reaches `submit_one` and creates the next row) |
| 6 | remove `advance` failed-shot phase guard | `a_failed_shot_stops_the_chain` | **still ok** (see finding below) |

Baseline in the worktree: `chain::tests` 7 passed, 0 failed (after copying the untracked sidecar in).

### D-G — plan-count floor
`the_shot_count_bound_is_one_to_eight` and `a_single_shot_plan_is_accepted_after_the_floor_dropped_to_one`
both pass: 1-shot and 2-shot plans accepted, 0 and 9 rejected. Copy is "1 to 8 shots".

### D-C — chained overrides, keyless
`apply_overrides` runs before `apply_constants`, gap-only, schema-gated; wired in `app.rs`
`submit_to_provider` for Fal routes only. The real `minimax_h3-max_image-to-video.json` declares
both `seed` and `prompt_expansion_mode`. Tests prove a chained body carries `disabled` + `seed`, a
standalone keeps `balanced` and no seed, and an undeclared key is dropped (mutation #4 confirms the
gate is load-bearing).

### Sandbox on every spawn
Every `Command::new` added by the slice (`run_to_file`, `render` in ffmpeg.rs) uses an argv vector
(no shell), `-nostdin`, `-protocol_whitelist file` before `-i`, `Stdio::null` stdin, a wall-clock
deadline with `child.kill()` on overrun, and bounded stderr. The compositor `to_args` now also
emits `-protocol_whitelist file` before the inputs (asserted by
`the_render_argv_whitelists_only_the_file_protocol_before_the_inputs`).

## Findings (non-blocking)

1. **`a_failed_shot_stops_the_chain` is vacuous w.r.t. the phase guard (mutation #6).** Removing
   `if settled.status.phase() != Phase::Completed { return; }` does NOT make the test fail, because
   the test's failed shot also has `results.clear()`, so the independent clip guard
   (`let Some(clip) = settled.results.iter().find_map(local_path) else return`) stops the advance
   anyway. The *behaviour* AC6 requires is still enforced by three layers — (a) the runner only
   fires the hook for a settled `Completed` shot, (b) `resume_all` filters on
   `phase()==Completed && has-local-output`, and (c) the clip guard in `advance` — so a failed
   predecessor genuinely cannot submit its successor. This is a test-robustness nit, not an AC
   break: no path submits past a failure.

2. **Partially-chained sets render the fallback join as a hard cut, not a 0.5s crossfade** (the
   implementer states this honestly). Because a chained set stores `overlap_seconds = 0.0`, the
   text-only fallback join renders as a clean cut rather than the memo's 29.5s crossfade example.
   It keeps every paid second and is not a money or correctness error; the per-join render logic is
   correct, only the single stored overlap is coarse for the mixed case. Documented, acceptable for
   this slice.

## Conclusion
All seven acceptance criteria and the folded-in continuity rules (D-B hard-cut join, D-C
schema-gated expansion-off + seed, D-G 1..=8 floor, frame scaling) hold against real commands and
the real sidecar. Five of six mutations were caught (the sixth exposed a redundant-but-correct
guard, not a defect). The gate is green: 888 Rust tests + 2 real-sidecar ignored tests, fmt/clippy
clean, provenance PASS, audit_fal unchanged (PINNED/UNREACHABLE/BAD-ROUTE all 0), 292 UI tests +
build. I could not refute the claim.

VERDICT: PASS
