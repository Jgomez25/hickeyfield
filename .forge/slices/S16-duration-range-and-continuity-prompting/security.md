# S16-duration-range-and-continuity-prompting — SECURITY (evidence)

Read-only pass over the slice diff (`git diff main...HEAD`). Scope: the new ffmpeg
score spawns, the money path (submit-time clamp, regenerate-guard relaxation,
auto-retry), the LLM-authored planner text flowing to fal, `joins_json` storage, UI
rendering, logging, and dependency posture. No source, docs, or backlog changed.

## Findings

No Critical, High, Medium, or Low findings. Boundaries examined and cleared below.

## Boundaries examined (silence is not clearance — stated explicitly)

### 1. FFmpeg score spawns — CLEAR
`crates/hickeyfield-core/src/ffmpeg.rs`
- `first_frame_args:885`, `ssim_psnr_args:950`, `signalstats_args:973` build **argv
  vectors of constant strings**; the only interpolation is `format!("scale={w}:{h}...")`
  over `u32` values (`last_frame_args:840`, `first_frame_args`) — no untrusted text
  reaches a flag. Filtergraphs (`scale2ref…ssim;…psnr`, `signalstats,metadata=print`)
  are fixed literals; no clip/frame content or LLM text is ever concatenated into them.
- Every builder starts `-nostdin` and places `-protocol_whitelist file` **before every
  `-i`** (`ssim_psnr_args:954` before both inputs; asserted by
  `the_score_argv_normalises_size_and_sandboxes_the_inputs:1805` and
  `first_frame_args_grab_frame_zero…:1829`). A crafted provider clip that is really a
  playlist/concat referencing `http`/`tcp` cannot trigger egress: the file protocol is
  the only one allowed. This is the SSRF-via-media boundary and it holds.
- `run_capture:1077` mirrors the `run_probe` hardening: `Stdio::null()` stdin, stderr
  drained on a thread with `bounded_push` capped at `MAX_PROBE_CAPTURE` (64 KiB,
  `:697`), a wall-clock `deadline` with `child.kill()` on overrun (`:1103-1109`).
  Unbounded/wedged sidecar output cannot exhaust memory or wedge the thread.
- Inputs are app-controlled paths (`renders/<sid>/frames/shot{i}.png`,
  provider-downloaded clips) passed as separate argv elements, never a shell string —
  `Command::new(bin).args(...)`, no `sh -c`. Command injection is not reachable.

### 2. Money path — CLEAR
`src-tauri/src/commands.rs`, `src-tauri/src/chain.rs`
- **Submit-time clamp** (`clamp_duration_to_range:1071`) runs in `submit_one` *before*
  `Billable::from(&settings)` (`:1109`), on a `settings` clone that is also what is
  persisted (`:1158`). The estimate therefore prices the clamped value, and the
  advisory names the range ("`{name} runs {min}–{max}s; {d}s clamped to {c}s`"). Range
  is read offline from `model.spec.duration_range`, never widened, network-free — no
  cost surprise. Verified by `a_duration_outside_the_range_is_clamped_with_an_advisory`.
- **Regenerate-guard relaxation** (`generate_storyboard_inner:1535-1553`): the
  below-floor set is computed **only** from `sb.joins_json` read from the store, filtered
  to `pass == Some(false)`. Client cannot forge this: `StoryboardGenerateInput` has no
  `joins_json` field; the only `pub joins_json` in commands.rs is on the *output*
  `StoryboardDto:1680`; new sets are minted with `joins_json: None` and the retry branch
  reads the existing server value. The sole writers are server-side scorers
  (`chain.rs::score_join:296`, `maybe_auto_retry:424`). A passing or still-running shot
  is still refused (`a_below_floor_join_may_be_regenerated_but_a_passing_one_may_not`).
  So a client cannot craft a score to unlock a paid resubmit of a Completed shot.
- **Auto-retry** (`should_auto_retry:` cap `AUTO_RETRY_CAP=2`, `maybe_auto_retry:`):
  off by default (`auto_retry` serde-default false, migration-10 column default 0),
  bumps and persists `attempt` *before* submitting so the bound survives re-entrant
  settles, unscored (`pass==None`) never retries, and every attempt goes through
  `submit_one` which prices the row before contacting the provider. Nothing spends
  without the total shown. `auto_retry_stops_after_two_attempts` pins the ≤2 bound.
- **Seed** is offset by `new_attempt` so a retry is not a paid no-op reproducing the
  same output.

### 3. Planner text → fal prompts — CLEAR
`crates/hickeyfield-core/src/storyboard.rs`, `prompts/storyboard.v2.md`
- The `world` block and hold rules are LLM-authored text composed as `"{world}
  {action}"` (`plan:229-236`) into the `prompt` string. This is generation *data* sent
  to fal, never interpreted as code, a shell, or a template; it is the same trust class
  as every existing prompt and reaches no injection sink (it does NOT flow into any
  ffmpeg filtergraph — see #1). No prompt-injection escalation path.
- Validator bounds: `MIN_WORLD_WORDS` floor on a multi-shot world, `CUT_WORDS` reject
  list, hold/identical-action/composition checks, and the preserved `1..=8` shot bound.
  Overall size is bounded upstream by the existing "brief is too large" / "plan is too
  large" caps at submit (unchanged, still tested). No unbounded field is newly exposed.
- `prompts/storyboard.v2.md` reviewed end-to-end: instructions only, no vendor copy, no
  keys or secrets; provenance lint passes per implement.md.

### 4. `joins_json` storage — CLEAR
`src-tauri/src/store.rs`
- Written only by the app (server-side scorers), never from a command input. All SQL is
  parameterised (`params![…]`, `?1..?10`); no string-built SQL. `parse_joins:37` fails
  soft to an empty vec on malformed JSON (no panic). Migration 10 is a single additive
  nullable column with a safe default (off); `user_version` asserted at 10. Size is
  bounded by the same plan/brief caps. Versioning/round-trip test present.

### 5. UI rendering — CLEAR
`ui/src/components/Library.tsx`, `Storyboard.tsx`
- Badges/percentages render as JSX text children (`min.toFixed(2)`, `data-pass`
  attribute) — React auto-escapes; no `dangerouslySetInnerHTML`/`innerHTML`. No XSS
  from a stored score or set title reaches the DOM as markup.

### 6. Logging — CLEAR
- New `tracing::warn!/info!` in `chain.rs` log `sid`, join index, and error strings
  (ffmpeg local file paths). No prompts, media bytes, or API keys are logged. No
  `println!`/`dbg!` added on any secret-bearing path.

### 7. Dependency / supply-chain — CLEAR
- `git diff main...HEAD --stat` shows no `Cargo.toml`, `Cargo.lock`, `deny.toml`,
  `package.json`, or `pnpm-lock` changes. `cargo deny` posture unchanged; no new crates
  or npm packages introduced by the slice.

## Recommendation to the orchestrator
No Critical/High findings — no new security slices required.

VERDICT: PASS

## RE-CHECK (HEAD 48cb68d)

Re-reviewed `git show 48cb68d -- src-tauri/src/commands.rs src-tauri/src/chain.rs`
(repair round 1). Money focus confirmed clear.

### `shot_in_flight(jobs, idx)` — the shared double-submit gate — CLEAR
`src-tauri/src/commands.rs:1086`
- Reads **only server-side job rows**: it iterates `&[JobSet]` and inspects `j.shot_index`
  and `j.status` — both store-owned fields — matching `!j.status.is_terminal()`.
  `is_terminal` (`crates/hickeyfield-core/src/job.rs:143`) is `phase ∈ {Completed, Failed,
  Canceled}`; Queued/InProgress and every intermediate stage are non-terminal, so the gate
  is conservative (in-flight ⇒ refuse). No client-supplied field is consulted: the caller
  passes `existing_jobs`/`store.jobs_in_storyboard(&sid)`, never anything from
  `StoryboardGenerateInput`. Cannot be bypassed by a crafted request field.
- **Manual regenerate** (`generate_storyboard_inner:1558-1585`): the `shot_in_flight`
  refusal returns `Err("shot N is still in flight …")` inside the per-index loop, i.e.
  before the branch returns the `existing` sid and before the batch reaches any
  `submit_one` or store write. The below-floor relaxation is only reached for a **settled
  (terminal)** low-join shot. Verified by `a_below_floor_join_that_is_still_in_flight_is_refused`
  (Queued/InProgress refuse; Completed eligible) and `shot_in_flight_reads_only_non_terminal_rows`.
- **`maybe_auto_retry`** (`chain.rs:399-409`): the same `shot_in_flight` check runs
  immediately after `let i = record.index;` and returns **before** the frame check, the
  `shots_json` parse, the attempt bump/persist, and `submit_one`. So a prior auto-retry
  attempt still queued/running cannot spawn a duplicate in-flight job; the persisted
  `attempt` still bounds the loop at ≤2. Refusal precedes any spend or store write.

### Scratch-frame cleanup — CLEAR
`src-tauri/src/chain.rs:314-318`
- `std::fs::remove_file(&first)` targets only
  `renders_root/<sid>/frames/first{i}.png`, built from the app-owned `renders_root`, the
  stored `sid` (from `settled.storyboard_id`, a persisted server value that must match an
  existing storyboard row), and the numeric shot index `i`. No component comes from
  untrusted/client input, and the path pattern is unchanged from the S15 `shot{i}.png`
  derivation. It deletes the scratch score input only; `shot{i-1}.png` (the continuity
  frame the chain and render still read) is untouched — confirmed by
  `score_join_cleans_up_the_scratch_first_frame`. Cleanup runs unconditionally after the
  score attempt (success or `Err`), so a failed score leaves no cruft either. No
  path-traversal or arbitrary-delete surface introduced.

All prior boundaries (sections 1–7) remain valid — the repair only added the in-flight
gate and the scratch cleanup, both money/disk-hygiene hardening. No Critical/High
findings; no new security slices required.

VERDICT: PASS
