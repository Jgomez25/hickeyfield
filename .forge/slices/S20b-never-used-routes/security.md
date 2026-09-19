# S20b-never-used-routes — SECURITY (evidence)

Read-only money-safety pass over the uncommitted slice diff (`git diff` +
untracked) in this worktree. No source, docs, or config changed. Static
inspection only; the two static lints (`lint-spend.py`, `lint-provenance.py`)
ran clean. All under the R-RULES preamble (`HICKEYFIELD_NO_KEYCHAIN=1` + dummy
`hickeyfield_*_KEY=x`); `HICKEYFIELD_ALLOW_SPEND` never set; no `first_light`,
no build/test run (a TEST agent holds the worktree). The lens: this slice is
itself a money-safety control (hide + refuse the owner's never-used Seedance/Grok
routes), so the audit is "can an expensive never-used generation still fire, or
can the guard be bypassed / fail open."

## (a) Submit bypasses — the never-used guard is the single funnel

Every fresh, billable provider submission is minted in `submit_one`
(`commands.rs:1147`). The new refusal (`commands.rs:1162-1172`) sits at the very
top of that function — before `compiled_for_submit` (which can call the paid
hosted enhancer), before `route::resolve`, before pricing, before
`app::submit_to_provider`, before any client or socket. Verified funnel:

- `submit_to_provider` (the only call to a real `ProviderClient::submit`) has
  exactly one caller: `submit_one` (`commands.rs:1259`; `app.rs:260` def). Grep
  confirms no other production caller.
- Every `submit_one` caller resolves `model` from `registry()` by id and passes
  `&model`, so the guard's `model.id` is the canonical id that will be routed:
  standalone `submit_job` (`commands.rs:1080`), storyboard/keyframe paths
  (`:1906`, `:1978`, `:2146`), and all three chain sites (`chain.rs:293,643,793`).
  One check covers standalone, storyboard shot, keyframe regen, and chain
  auto-retry.
- `runner.resume_all` (`runner.rs:123`) → `watch` → `run_watched` only re-polls
  jobs that a prior `submit_one` already created at the provider (no new
  `.submit`, no new spend) — same finding as S18 §1. Not a fresh-submit path.
- The webview cannot reach a raw provider submit: it only invokes registered
  commands, and the only generation commands route through `submit_one`.

No path reaches a provider submit while skipping the never-used check. The
picker feeds (`list_models`/`models_for_use_case` via the pure
`models_matching(show, ..)`, `commands.rs:444,463,491`) also omit never-used ids
server-side when the pref is OFF, so the UI cannot even offer one; but the
authority is the server refusal, not the hidden button. Defense in depth is
correct.

## (b) Fail-closed on the money guard — confirmed

`get_bool_pref` (`store.rs:432-441`) is `default` on ANY failure: a missing
`app_pref` table (downgraded DB), a locked/absent row, or a non-`"1"` garbage
value all fold through `.ok()...unwrap_or(default)` / `v == "1"`. The two money
consumers both pass `default = false`:

- `submit_one` guard (`commands.rs:1163`) — error/garbage ⇒ `false` ⇒ never-used
  refused ⇒ **fail CLOSED**. No spend on a read error.
- picker filter (`commands.rs:445,465`) — error ⇒ hide ⇒ fail closed.

A poisoned mutex would panic (`.lock().unwrap()`), which refuses-by-crashing, not
spends — still money-safe. The api.ts `getShowNeverUsed` localStorage fallback
(`api.ts:534`) is client-side and only triggers when `invoke` throws (mock/browser
build, no real spend); it can never override the server refusal in the packaged
app. No fail-open on the guard.

## (c) S17/S18 structure intact

`vault.rs`, `spend_gate.rs`, `clients.rs` (`ProviderClient::submit` spend gate),
and `examples/first_light.rs` are absent from the slice diff — untouched. The new
refusal test `submit_one_refuses_a_hidden_model_while_the_toggle_is_off`
(`commands.rs:3244`) uses an in-memory store + the S18 offline `RecordingClient`
(no socket), a temp-dir `Library`, sets no `HICKEYFIELD_ALLOW_SPEND`, unsets no
key, and asserts the OFF path returns `Err` with zero clients reached (proven
no-spend). Diff grep for `ALLOW_SPEND` / `remove_var` / `first_light` /
`find-generic-password`: none. `Cargo.toml`, `Cargo.lock`, `ui/package.json`,
`pnpm-lock.yaml`, `deny.toml` unchanged — no new dependency or supply-chain
surface. `lint-spend` ok (49 files); `lint-provenance` PASS.

## Boundaries examined — explicit clearances

- **Webview trust boundary.** `set_show_never_used` (`commands.rs:492`) takes a
  plain `bool on`; no generic `set_pref(key, value)` is exposed — the webview
  cannot write an arbitrary key/value into `app_pref`. `model_names`
  (`commands.rs:471`) returns only `{id, display_name}` — no keys, paths, or
  route secrets. The three new commands are registered only in the single
  `generate_handler!` list (`lib.rs:88-90`); no Tauri capability/CSP file changed.
- **SQL / migration 12.** `store.rs:414-427` is append-only (`if version < 12`,
  `CREATE TABLE IF NOT EXISTS ... STRICT`, then `user_version = 12`), touches no
  existing table, and preserves every existing row (owner's v11 DB reads the OFF
  default until they opt in). `get_bool_pref`/`set_bool_pref` use parameterised
  SQL (`?1`/`?2`, upsert) — no interpolation, no injection surface. Idempotent
  re-open proven by the new roundtrip test.
- **Injection / prompt injection.** No new SQL string-building, no command/path/
  template execution, no new fetched-content sink. The prompt/enhancer path is
  unchanged and now runs strictly AFTER the never-used refusal. No path from
  fetched or user content to toggling the pref or to an expensive submit.
- **Misleading-money UI (A4).** History resolves a hidden model's name through
  the unfiltered `model_names` map first (`App.tsx:724`, `Storyboard.tsx:139`),
  falling back to `models` then raw id — a job on `seedance_2_0` still shows
  "Seedance 2.0" and its recorded USD; it is never silently re-pointed to another
  model. UI defaults prefer `WORKHORSE_MODEL_ID` by name with `isLaunch`/`m[0]`
  fallbacks (`App.tsx:207,352`, `Storyboard.tsx:216`); the workhorse is not
  never-used, so no default can fall back onto a hidden expensive route. Toggle
  copy states the cost risk plainly ("These bill the provider — hidden by
  default.", `SettingsRail.tsx:333`).
- **Secrets / privacy / supply chain.** No keys, tokens, private folder paths, or
  private brand/person names introduced. Blurb sweep (`onboarding.rs:43,53,83`,
  `providers.ts:44,84`, `browser.ts:547`) only removes Seedance/Grok advertising;
  the xAI provider row + `GROK_API_KEY`/`XAI_API_KEY` envNames correctly stay
  (provider, not a default). `mock.ts` grep for `/seedance|grok/i` = 0.

## Findings

None at any severity. No injection surface, no submit bypass, no fail-open, no
secret/dependency exposure. The one behavioural note below is by design, not a
finding (it has no attacker path in a single-owner local desktop app):

- The toggle reveals and enables all 11 never-used routes globally with one flip;
  this is the owner's explicit opt-in and the server refusal remains authoritative
  per-submit. Accepted.

No Critical/High findings ⇒ no new backlog slices recommended.

VERDICT: PASS
