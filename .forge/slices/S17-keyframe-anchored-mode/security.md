# S17-keyframe-anchored-mode — SECURITY (evidence)

Read-only pass over `git diff main...HEAD`, with the 2026-09-16 real-spend
incident as the primary lens. No source, docs, or config changed.

## Primary lens: the keychain hotfix (INCIDENT 2026-09-16)

The incident: a unit test stripped the dummy `hickeyfield_*_KEY` env, `vault::get`
fell through to the owner's real macOS keychain key, and `submit_one` billed 9
paid generations from `cargo test`. Fix = `vault::keychain_blocked()`.

**The hotfix is correctly placed and complete for the vault.** Verified:

- `src-tauri/src/vault.rs:60` — `get()` is the SINGLE credential read. It checks
  `env_override` first, then returns `None` when `keychain_blocked()` — before
  ever calling `keyring`. Both halves route through it: key (`secret_half=false`)
  and the Higgsfield secret (`secret_half=true`, `vault.rs:53-54`,
  `app.rs:53-54,81-82`).
- Grep for every credential reader across `src-tauri` and `crates`
  (`get_password` / `keyring` / `vault::get`): the ONLY `keyring::Entry` /
  `get_password` call sites are inside `vault.rs`. Every other consumer
  (`app.rs:48-90`, `commands.rs:110-111,885`) goes through `vault::get`, so all
  inherit the guard. No credential read bypasses it. **Nothing found at this
  boundary that escapes the guard.**
- `keychain_blocked()` (`vault.rs:40`): `cfg!(test)` OR a truthy
  `HICKEYFIELD_NO_KEYCHAIN` (correctly treats `0`/`false`/empty as off). The two
  guard tests (`vault.rs:161-177`) pin both the stripped-env case and the
  env-override-still-wins case.

**`cfg!(test)` coverage — stated plainly:**

- `cfg!(test)` is true for the vault's own unit tests (they live in `vault.rs`,
  compiled into the `src-tauri` lib with `--test`). It is NOT set when the lib is
  built as a dependency of a separate integration-test crate, nor for
  `examples`/binaries.
- `src-tauri` (the crate that owns the vault) has NO `tests/` integration
  directory and NO `[[example]]` targets. So the only non-`cfg(test)` build of
  `vault.rs` is the production Tauri app (`src-tauri/src/main.rs`) — where the
  keychain is *supposed* to work. There is therefore no integration-test or
  example build of the vault that silently loses the guard. Good.

## Findings (ranked)

### MEDIUM — live-spend example `first_light` is guarded by neither the hotfix nor `HICKEYFIELD_NO_KEYCHAIN` (pre-existing; NOT introduced by this slice)

`crates/hickeyfield-core/examples/first_light.rs:30-46` reads the raw env var
`FAL_KEY` (line 31), builds a real `FalClient`, and submits a real generation
(step 3, "A real generation, cheapest available"). It does NOT use the vault at
all — the vault lives in `src-tauri`, not `hickeyfield-core`. Consequently:

- `vault::keychain_blocked()` and `cfg!(test)` are irrelevant to it.
- `HICKEYFIELD_NO_KEYCHAIN=1` does NOT guard it either — the task framed this env
  var as "the only guard for agent-run `cargo run --example`", but that is not
  accurate for `first_light`: nothing in that binary consults the keychain or
  `HICKEYFIELD_NO_KEYCHAIN`. Its only guard is that plain `FAL_KEY` happens to be
  unset.

Exploitation path: an agent follows `first_light`'s own docstring
(`first_light.rs:20` — `FAL_KEY=$(security find-generic-password -s
ai.hickeyfield.keys -a fal -w) cargo run ... --example first_light`), which
copies the owner's real keychain key into `FAL_KEY`; the example then submits a
real paid generation. The standing agent rules export `hickeyfield_FAL_KEY=x`
(dummy) and `HICKEYFIELD_NO_KEYCHAIN=1` but NOT `FAL_KEY`, so under the rules as
written `cargo run --example first_light` exits at line 34 without spending. The
residual path requires an agent to deviate from the rules (set `FAL_KEY`) — hence
MEDIUM, not High, and out of this slice's scope.

Fix (recommend as its own backlog slice): gate every live-spend example behind an
explicit `HICKEYFIELD_ALLOW_SPEND=1` and have it refuse a key otherwise, so no
example can spend under `cargo run --example` unless a human opts in. Applies to
`first_light.rs` (the live generation) and any future spend-capable example.

`crates/hickeyfield-core/tests/live_fal_pricing.rs` was reviewed alongside: it is
`#[ignore]`, reads `hickeyfield_FAL_KEY`/`FAL_KEY`, hits only the read-only GET
`models/pricing` endpoint (`:31`, "starts no generation"), never logs the key,
and is not run by default `cargo test`. No spend path. Not in the slice diff.

## Boundaries examined — clear

**Test suite / no remaining env-stripping (incident recurrence).** Grep for
`remove_var` / `set_var` / `strip_provider_env` across `src-tauri` + `crates`:
the only `remove_var`/`set_var` on provider keys are inside `vault.rs`'s own
guard tests (each pairs set with remove, scoped). `commands.rs:3468` documents
that `strip_provider_env` was deliberately NOT reintroduced. The two keyframe
tests that could reach a submit are safe by construction: the barrier/chain tests
pin `route_id: Some("unavailable:no-such-route")`
(`chain.rs:1426-1436`), so any accidental `submit_one` fails at `route::resolve`
with `UnknownPin` BEFORE a Queued row or any HTTP; and
`keyframe_generate_holds_the_shots_and_stores_the_boundaries`
(`commands.rs:3260-3350`) runs only the offline derive+store half and asserts
`jobs_in_storyboard` is empty — it never enters the submit loop. No test
constructs a real provider client with a resolvable route + a key.

**Slice money paths.** Each still is priced up front
(`commands.rs:1674-1690`, `still_estimate` `:1291`) into the stored
`KeyframeRecord.estimated_usd`, feeding the total shown before Generate; an
unpriced model yields `None` and is never summed as $0 (`store.rs:52-56` doc,
`:1289`). Still regenerate is gated: in-flight guard refuses while the boundary's
still is non-terminal (`commands.rs:2019-2027`), and the auto path is bounded by
`KEYFRAME_RETRY_CAP = 2` (`commands.rs:2031`, `chain.rs:639,647`), distinct from
the shot join's `AUTO_RETRY_CAP`. A failed still parks the set — no shot 0 is
submitted against a missing anchor (`chain.rs:526-547`, barrier returns while any
`local_path` is `None`). `submit_shot_zero` is idempotent
(`stills_ready_for_shot_zero` `:557` refuses if a shot-0 row exists). N+1 stills
are bounded by `MAX_SHOTS = 8` (`commands.rs:1509,1536`).

**Both-frame binding.** The end still is attached only when `end_frame_capable`
(route accepts `MediaRole::End`) is true (`chain.rs:92-98,146-160`); otherwise the
shot falls back start-only with an advisory and is scored end-absent — never
billed for a frame a route silently drops (the S9 full-price harm). Both start
and end frames are `MediaRef::local(...)` from downloaded still PNGs
(`chain.rs:120-124,148-152`), and the end score checks `Path::is_file`
(`chain.rs:416`) — app-local files through the existing media/download path, no
untrusted-URL injection. `image_size` rides as a schema-gated constant override
(`commands.rs:1257-1261`, `still_submit_input` doc `:1248`), server-computed from
the set canvas, not client-supplied width/height.

**Storage / migration 11.** `store.rs:395-408` — a single static
`ALTER TABLE storyboards ADD COLUMN keyframes_json TEXT; PRAGMA user_version=11`.
Additive, nullable; pre-v11 rows read `None` (`store.rs:603-605`). `keyframes_json`
is server-written (`serde_json::to_string` of a `Vec<KeyframeRecord>` the backend
derives), size-bounded by `MAX_SHOTS`. All writes use parameterized
`params![...]` (`store.rs:445-458`) — no SQL injection. The end-adherence numbers
are additive `#[serde(default)] Option` fields on `JoinRecord` (`store.rs:38-56`),
so no separate migration and pre-S17 rows deserialize cleanly.

**Injection surfaces.** The derived still prompt is sent verbatim to the provider
as generation text (`still_submit_input` `final_prompt`, `commands.rs:1281`) —
that is ordinary content-to-model, not injection into our own control plane. The
untrusted `plan_json` is size-capped (`MAX_PLAN_JSON_BYTES = 64 KiB`,
`commands.rs:1505,1526`) and `brief` at 4 KiB (`:1503,1519`); both are parsed with
`serde_json::from_str` into typed structs. No command injection (the only new
`Command::new` is an `#[ignore]`d test spawning the bundled ffmpeg on synth PNGs,
`commands.rs:3326,3357`). No path traversal introduced — still paths originate
from job downloads via the existing Library media path.

**Least privilege / secrets / egress.** Single-user desktop Tauri app: no
tenant/authz boundary in scope. Secrets never cross into the webview — the vault's
invariant is unchanged and the slice adds no key-returning command. No new hosted
egress in the slice's production code paths; live submits remain the shared,
audited `submit_one`.

**Logging.** Grep of added `+` lines for `tracing|println|eprintln` near
`key|secret|token|password`: none. The one new `tracing::warn!`
(`commands.rs:1889`) logs a storyboard id and error string, no credential.

**Dependencies / supply-chain.** `Cargo.toml`, `Cargo.lock`, and `deny.toml` are
NOT in the slice diff. No new crates; `cargo deny` posture unchanged.

## Recommendation to the orchestrator

Open ONE follow-up backlog slice (not blocking S17): "Gate live-spend examples
behind `HICKEYFIELD_ALLOW_SPEND=1`" — `crates/hickeyfield-core/examples/first_light.rs`
must refuse to run (and refuse a key) unless that env var is explicitly set, and
its docstring should stop instructing a keychain-to-`FAL_KEY` copy. This closes
the one credential-read path in the repo that neither the keychain hotfix nor
`HICKEYFIELD_NO_KEYCHAIN=1` protects.

No Critical or High finding blocks this slice: the vault hotfix — the slice's
central security change — is correct and complete, and every money path the slice
adds is priced, gated, bounded, and parks on failure. The one MEDIUM is a
pre-existing, out-of-slice example not reachable under the standing agent rules.

VERDICT: PASS
