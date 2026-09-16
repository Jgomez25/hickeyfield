# S15-storyboard-render — SECURITY (evidence)

Read-only pass over the slice diff (`git diff main...HEAD`, HEAD e82d2cd). Scope: the new
ffmpeg spawns (last-frame extract + render runner), the chain scheduler and runner settled
hook, the constant-override seam, migration 9, and Library registration. Every finding
carries a concrete path, severity, `file:line`, and fix. Boundaries examined with nothing
found are stated as cleared.

## Boundaries examined — CLEARED (no finding)

### ffmpeg spawn surface — argv-only, sandboxed, bounded
- `crates/hickeyfield-core/src/ffmpeg.rs:836` (`last_frame_args`) and `:876` (`last_frame`):
  the argv is a `Vec<String>` passed to `Command::new(bin).args(...)` — no shell, no
  interpolation into a shell string. `-nostdin` is arg 0; `-protocol_whitelist file`
  precedes `-i` (unit-asserted at `:1440`); `-sseof`/`-update` are numeric literals, not
  caller-derived. `run_to_file` (`:894`) closes stdin, drains stderr on its own thread
  bounded by `MAX_PROBE_CAPTURE`, enforces the 20s `PROBE_TIMEOUT` with `child.kill()` on
  overrun, and treats a non-zero exit / missing output as `FfmpegError::Render` — never a
  silent skip (AC1). A downloaded (untrusted) clip is only ever an `-i` operand behind the
  `file`-only whitelist, so it cannot chain to `http`/`concat`/`subfile`.
- `crates/hickeyfield-core/src/ffmpeg.rs:963` (`render`): same skeleton with a live
  progress reader on stdout; deadline is polled between reads so a wedged encoder that
  stops emitting progress is killed (`:1005`), stderr on its own thread so a full pipe
  cannot deadlock the wait, non-zero exit → typed error.
- `crates/hickeyfield-core/src/compositor.rs:565`: the render argv gained
  `-protocol_whitelist file` before the `-i` inputs (asserted at `:1379`), matching the S10
  probe sandbox. The filtergraph strings are built only from probed durations and numeric
  overlaps formatted through `secs()` — no untrusted string reaches a filter expression.

### Path traversal into intermediates / Library — CLEARED
- Storyboard ids are minted as `sb-<ts>-<seq>` (`commands.rs:47`) — no path separators.
  `render.rs:161` (`renders_root.join(&storyboard_id)`) and `chain.rs` frames dir
  (`renders_root.join(&sid).join("frames")`, `format!("shot{cur_idx}.png")` with a `u32`
  index) therefore cannot traverse. `render_storyboard` takes `storyboard_id` from the
  frontend but gates on a parameterised `store.storyboard(&id)` lookup that 404s before any
  path is joined, so a `../` argument dead-ends at the DB read.
- `library.rs:206` (`adopt`) builds the destination as
  `unique_path(dir, &sanitize(stem), ext)`; the stem derives from the user's `brief`, and
  `sanitize` (`:58`) maps `/ \ : | ? * < >`, controls, and reserved names, trims dots, and
  bounds length (tests `:226`, `:234`, `:244`). The output stays under the Library root
  (test `adopt_moves_a_local_render_into_the_day_folder`).

### Filtergraph numeric safety — CLEARED
- User-supplied `RenderOptions.overlap_seconds` (`render.rs:26`) only becomes a
  `crossfade_in` when `overlap > 0.0` (`render.rs:145`), so `NaN` (comparison false) and
  negatives are dropped. An over-long or `Inf` overlap is caught by `Timeline::validate`
  (`compositor.rs:694`) and `render.rs:189` refuses to render on any validation problem.
  Durations come from the sidecar probe (finite) and a shot with no probeable duration is
  rejected (`render.rs:126`), never guessed.

### Chain scheduler money / exactly-once — CLEARED
- Every chained shot submits through `submit_one` (`commands.rs:1032`), which re-runs the
  S6 route guard, S10 estimate/route-price guard, and S7 verbatim prompt — the identical
  money-gated path a standalone submit uses. `chain.rs:advance` adds only "when + which
  start frame".
- A dependent shot is submitted only from `download_outputs` after the predecessor is
  `Completed` AND every output is local (`runner.rs:273`, gated on `is_settled()`); a failed
  predecessor never reaches the hook, so the chain parks (`chain.rs` phase check).
- Exactly-once: the settled hook fires once per shot (`runner.rs` test
  `the_settled_hook_fires_once_for_a_storyboard_shot`), and `advance` no-ops if a row already
  exists at `next` (`chain.rs` idempotency guard), so the hook and `resume_all` cannot
  double-submit an index. Relaunch resume (`app.rs:resume` → `chain::resume_all`) runs after
  the ordinary resume and only acts on a genuinely stalled chain. See INFO-1 for the residual
  TOCTOU.
- No submit without the total shown at generate time: `generate_storyboard_inner` submits
  only shot 0 in chained mode and marks the rest `pending` (`commands.rs:1531`); the plan the
  user priced is the whole set.

### Constant-override seam — CLEARED (see INFO-2 for the offline note)
- `fal_schema.rs:307` (`apply_overrides`) is gap-only (`body.contains_key` → skip) and
  schema-gated (`schema.is_none_or(|s| s.accepts(key))`), so with a cached schema it writes
  only endpoint-declared keys — a chained shot cannot inject an arbitrary fal field (test
  `an_override_cannot_inject_a_field_the_endpoint_rejects`). `chain.rs:chained_overrides`
  emits only a typed `seed` (number) and `prompt_expansion_mode` (string). It runs before
  `apply_constants` (`app.rs:398`) so it overrides rather than clobbers a user/binder value.

### Migration 9 / stored state — CLEARED
- `store.rs:55` migration is static DDL (`ALTER TABLE ... ADD COLUMN`, `CREATE TABLE IF NOT
  EXISTS`, `PRAGMA user_version = 9`) with no interpolation. All writes are parameterised
  (`upsert` `params![..]` `:100`, `upsert_render` `:145`, `render`/`renders` readers). No
  `format!` builds SQL. `joins_json` is written `None` everywhere in this slice — reserved,
  not reachable by untrusted input.

### Logging / secret handling — CLEARED
- New `tracing` calls (`chain.rs` x3) carry only the minted `sid`, `u32` shot index, and a
  submit error string — no prompts, no API keys. `render.rs` logs nothing. No new
  `println!`/`eprintln!`. The Library render row (`StoryboardRender`) stores an app-generated
  path, dimensions, duration, Σ actual cost, timestamp — no prompt, no key.

### Supply chain — CLEARED
- No change to any `Cargo.toml`, `Cargo.lock`, or `deny.toml`; no new dependency introduced.
  `cargo deny` posture unchanged.

## Findings (informational only)

### INFO-1 — chain `advance` read/submit is not atomic (residual TOCTOU)
- `src-tauri/src/chain.rs` (`advance`): the idempotency guard reads
  `jobs_in_storyboard` and checks for a row at `next`, then `submit_one` mints an id and
  writes it — under two separate `conn.lock()` acquisitions, not one transaction.
- Path: two `advance` calls for the *same* predecessor running concurrently (e.g. a
  re-polled settle racing `resume_all`) could both pass the "no row at next" check and both
  submit shot `next` = a double charge. In practice the architecture serialises this: a
  settled job is not re-polled (`runner.rs` test), and `resume_all` runs once at startup
  before watch threads re-attach, so no concurrent `advance` for one predecessor is reachable
  today. Severity: Informational (no reachable exploit on the current call graph).
- Fix (defensive, for a future slice if the runner ever gains parallel re-poll): make the
  "no successor row" check and the successor insert a single transaction, or add a UNIQUE
  index on `(storyboard_id, shot_index)` so a racing second submit fails the insert rather
  than double-billing.

### INFO-2 — override seam writes unknown keys when no schema is cached
- `crates/hickeyfield-core/src/fal_schema.rs:307` (`apply_overrides`): `is_none_or` means
  that with no cached endpoint schema, any override key is written to the fal body.
- Path: the caller-controlled `constant_overrides` field on the `submit_job` command
  (`commands.rs:744`) is deserialised from the frontend; offline (schema not yet fetched) a
  caller could place arbitrary keys into the outbound fal body. This is the trusted local UI,
  the values are gap-only, and fal ignores/422s unknown fields, so it is not an app-boundary
  breach — and it exactly mirrors the pre-existing `apply_constants` offline rule. Severity:
  Informational.
- Fix (optional hardening): restrict `apply_overrides` to a known allow-list
  (`seed`, `prompt_expansion_mode`) rather than "any key when schema absent", so the seam
  cannot carry an arbitrary field even offline.

## Verdict

No Critical or High findings; both notes are informational with no reachable exploit on the
current call graph. No new backlog slices required. The ffmpeg spawns are argv-only,
`-nostdin`, `file`-only-whitelisted before `-i`, timeout-and-kill bounded; the chain is
money-gated, failure-parking, and idempotent; the override seam is typed and schema-gated;
migration 9 is static DDL with parameterised writes; the Library path is minted/sanitised;
nothing new logs prompts or keys; no dependency changed.

VERDICT: PASS
