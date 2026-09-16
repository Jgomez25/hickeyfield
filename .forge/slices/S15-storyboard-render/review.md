# S15-storyboard-render — REVIEW (evidence)

Independent, read-only production-standards review of the slice diff (`git diff main...HEAD`).
Scope: the render/chain half of the storyboard feature. I changed no source. Gate re-run below.

## Gate re-run (this machine, dummy-key env as in implement.md)
- `cargo test --workspace`: **888 passed, 0 failed** (summed across `test result: ok` lines).
- `cargo test -p hickeyfield-core --lib -- --ignored last_frame_writes render_stitches`:
  **2 passed, 0 failed** on the real bundled sidecar.
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `cargo fmt --all --check`: clean (exit 0).
- `cd ui && pnpm test`: **292 passed** (23 files).

Matches the implementer's pasted counts. No count inflation.

## What I checked and found clean

**(a) Scheduler correctness.** Exactly-once/idempotence/no-deadlock/failure-stop/cancellation all hold:
- The settled hook fires only from `download_outputs` after the final upsert, gated on
  `job.storyboard_id.is_some() && job.is_settled()` (runner.rs:263-273). A failed shot hits the
  early return at the top of `download_outputs` (phase != Completed || results empty) and never
  reaches the hook — the chain parks, verified by `a_failed_shot_stops_the_chain` and the
  `advance` guard `settled.status.phase() != Completed` (chain.rs:136-138).
- "Exactly once" is not defended by the hook itself (a re-poll could call `download_outputs` again);
  it is defended by `advance`'s idempotency guard — if a row already exists at `next`, it no-ops
  (chain.rs:170-174, `advance_is_idempotent_when_the_next_shot_exists`). This is the correct place
  for the invariant and it is real.
- Resume: `chain::resume_all` runs after `runner.resume_all()` (app.rs:222), scans chained sets,
  advances from the highest Completed shot with a local output; the idempotency guard makes overlap
  with an in-flight shot safe.
- No deadlock between the concurrency cap and chain waits: `advance` holds no store lock across
  `submit_one`; `runner.watch` only spawns a thread and returns (the permit is acquired *inside*
  the spawned `run_watched`, not synchronously), and a chained set has at most one shot in flight,
  so the cap is never contended by a waiting chain. The hook runs on the settling shot's own watch
  thread, whose permit is released when `run_watched` returns — after the hook has already handed
  off to a new thread.
- Cancellation: the abandoned-set check gates `download_outputs` (runner.rs:653-656), so a shot
  cancelled during its final poll neither downloads nor fires the hook.

**(b) `submit_one` unchanged for single jobs.** The refactor is a parameter-shape change only
(`&AppState` → `&Arc<SqliteStore>, &Arc<Prices>, &Runner`); the compile→guard→estimate+provenance→
`route::resolve`→upsert→`submit_to_provider`→`watch` body is line-for-line identical (commands.rs
:1035-1135). The one addition, `constant_overrides`, is `None` on every standalone submit
(`submit_job` passes `None`, commands.rs:1018-1026) and resolves to an empty map, so a single job is
byte-identical. `media: input.media.clone()` still persists (commands.rs:1101) — which the render's
per-join continuity detection depends on.

**(c) Money honesty.** Each dependent shot re-runs the estimate + S6/route guard at its own submit
via `submit_one` (chain.rs:214-222). The total is shown at generate time (S13 sum, unchanged);
drift between generate and a later shot's turn is re-priced by `submit_one` from the live feed and
surfaced on the S10 meta rail (design D2.7 — proceed/surface/never block, consistent with S13
best-effort-continue). Rendering spends nothing and says so ("Rendering is free" in Library.tsx).
Σ actual is `None` if any member is unpriced — never summed as $0 (render.rs:129-133, store test
`the_chained_columns_and_a_render_round_trip`, UI test `an_unpriced_member…never $0`).

**(d) Override seam.** With a cached schema, `apply_overrides` writes only keys the endpoint
`accepts`, so it cannot inject an undeclared field (`an_override_cannot_inject_a_field_the_endpoint_rejects`,
fal_schema.rs). It runs before `apply_constants`, so a chained shot's `prompt_expansion_mode:
"disabled"` beats the S14 `balanced` constant, and a standalone submit (no overrides) keeps
`balanced` and carries no seed (`a_standalone_submit_keeps_balanced_and_has_no_seed`). The only
override map ever constructed is `chained_overrides` — the two fixed, H3-Max-declared keys
(`prompt_expansion_mode`, `seed`) — so `audit_fal` stays clean (re-verified PINNED 0 / UNREACHABLE 0
by the implementer; no schema/mode logic touched). Offline (no cached schema) it writes
unconditionally, exactly the lesser-risk rule `apply_constants` already uses — noted, not a defect.

**(e) Render.** argv reuses the S10 sandbox (`-nostdin`, `-protocol_whitelist file` before `-i`,
timeout+kill, bounded stderr, argv vector); the compositor gained `-protocol_whitelist file` too
(compositor.rs). Progress parsing reuses the tested `ProgressReader`; percent is honest
(`Progress::percent(total)`). Temp mp4 written under `renders_root`, adopted into the Library
(cross-volume rename→copy+remove fallback, `adopt_moves_a_local_render_into_the_day_folder`), then
removed. Disk bound: only small frame PNGs (kept for the deferred score slice) + the render temp
(removed). Library registration: item is a `StoryboardRender` row, Σ member actual. Single-shot path
covered (`render_stitches_to_the_expected_duration` asserts 4±0.2s single, 10±0.2s three-clip).
Server-side re-gate refuses a set with any un-downloaded shot (render.rs:117-127).

**(f) Partial-chain overlap-0 limitation** — see the finding below; acceptable for this slice, with a
recommended follow-up.

**(g) Migration 9 / dead code / tests.** Additive columns + `storyboard_renders`, `user_version`
bumped to 9, pre-v9 rows default to all-at-once, round-trip tested, v1-drop list updated (drops the
whole `storyboards` table so per-column drops are unnecessary). `1..=8` floor + the "1 to 8 shots"
copy + the `//! 1..=8` doc all moved together (storyboard.rs). Stitched-duration arithmetic is
correct and clamped to the shorter neighbour. `store::render()` is `#[allow(dead_code)]` and honestly
labelled test-only. **Test quality:** the rewritten `a_single_shot_plan_is_accepted…` (was the M2
`sub-3-shot rejected` test) is a legitimate consequence of the D-G floor change, not a weakened
assertion — the full-plan retry contract is still guarded by the retained retry tests
(`shot_indices=[2]` + full plan). No assertion in `git diff main -- '*test*'` was loosened to pass.
CHANGELOG wording is accurate to the shipped behaviour.

## Findings (all minor; none blocking)

1. **minor — misleading comment vs. code in the render gate.** `src-tauri/src/render.rs:104-107`
   the comment says "every shot must be Completed with a local video output", but the loop
   (render.rs:117-127) only checks for a local video output, never `job.status == Completed`. In
   practice a `local_path` is only set for a Completed job (download_outputs requires it), so the
   effective gate is correct and safe; the comment overstates what the code asserts. Fix: either
   check `job.status.phase()` explicitly, or reword the comment to "must have a downloaded video
   output".

2. **minor — blocking read can defeat the render deadline.** `crates/hickeyfield-core/src/ffmpeg.rs`
   `render()` evaluates the deadline only *between* stdout reads; a wedged encoder that stops
   emitting `-progress` blocks on `out.read(&mut chunk)` and never reaches the deadline check, so the
   kill never fires. The doc-comment "a wedged encoder that stops emitting progress is killed here"
   overstates this. Low risk (local, trusted bundled sidecar, own already-downloaded clips), but the
   claimed guarantee is not delivered. Fix: bound the read (a watchdog thread that kills on the
   deadline, or a read timeout), or soften the comment.

3. **minor — swallowed render error / misleading comment.** `ui/src/App.tsx` `onRenderStoryboard`'s
   `.catch(() => …)` clears the progress entry and returns; the comment claims "the error surfaces
   via the standard toast path the invoke throws through", but the catch consumes the rejection and
   shows nothing — a failed render (missing sidecar, disk full, timeout) silently re-enables the
   Render button with no user-facing message. Fix: surface the caught error (toast/inline), or drop
   the comment's claim.

4. **minor / informational — `chainable_model` is the coarse check.** `src-tauri/src/chain.rs:39-41`
   collapses to `use_case::supports(model, ImageToVideo)`, whereas design D2.4 specified a stricter
   per-route "binds `MediaRole::Start` on an i2v endpoint" authority for the scheduler (with
   `use_case::supports` reserved for the UI toggle default). The two now share the coarse check. Not
   a correctness bug — the S9 binding path still governs whether the frame is actually attached at
   submit, and the text-only fallback + advisory is honest — but the design's stricter gate was not
   implemented. Worth a note; no change required for this slice.

## Recommended follow-up (non-blocking)

- **Per-join overlap for a partially-chained set.** A chained set stores one `overlap_seconds = 0.0`
  (commands.rs:1509-1518), so a mid-chain text-only fallback join renders as a hard cut by default
  rather than the 0.5s crossfade memo D-B calls for (the implementer documented this honestly). The
  render already decides continuity per-join from each shot's Start media (render.rs:150-158) and
  would crossfade a fallback join if a caller passed `overlap_seconds > 0`, so this is only the
  stored default. `joins_json` is reserved for exactly this. Acceptable for S15 (all-chainable H3 Max
  set is the real case; hard cuts keep every paid second); fold per-join persistence into the
  deferred continuity-score slice.

No unresolved blocker or major on a critical path. The money invariants, the exactly-once/idempotent
scheduler, and the no-charge render all hold and are backed by real tests.

VERDICT: PASS

## RE-REVIEW (HEAD 1139b13)

Re-reviewed the coordinator's fix for minor finding 3 (`git show 1139b13` — UI-only, 3 lines in
`ui/src/App.tsx` `onRenderStoryboard`).

**Delta confirmed correct, introduces nothing new.** The `.catch` now takes `(e: unknown)` and calls
`setSubmitError(e instanceof Error ? e.message : String(e))` before clearing the render progress; the
misleading "surfaces via the standard toast path" comment is gone. `setSubmitError` is the app's
shared error state (App.tsx:142), rendered at App.tsx:875 with a dismiss handler — the exact banner
every other refused action already uses (generate at :469/:511, chain/submit at :617/:723), and the
error-coercion expression is character-identical to those sites. No new state, no new surface, no new
control flow. Money/scheduler/render invariants are untouched (no Rust changed). Re-ran the UI gate at
HEAD 1139b13: **pnpm test 292/292 passed, pnpm build green.** Findings 1, 2, and 4 from the first pass
stand unchanged (all minor, non-blocking); the per-join overlap follow-up is unaffected.

**Is the banner the right surface for a Library-view action?** Yes — acceptable and, on balance, the
right call. The banner is rendered in the always-visible workspace chrome (not tab-scoped), so it is
seen from the Library, and using it keeps every refused action in one predictable place rather than
inventing a one-off error surface for renders. A Library-local inline message (near the Render button
of the set that failed) would be marginally better for locality — it ties the error to the specific
set — but it costs a new per-set error state and a new UI affordance for a single action, and it would
diverge from the app's established one-banner convention. Recommendation: keep the shared banner now;
consider a per-set inline error only if/when the Library grows other set-scoped failures worth
localising. Non-blocking.

VERDICT: PASS
