# S13-storyboard-plan — SECURITY (evidence)

Read-only security pass, scoped to the slice diff `git diff main...HEAD`. Reviewer
changed no source, docs, or config. Threat model: this is a native Tauri desktop app;
the new commands (`plan_storyboard`, `generate_storyboard`, `list_storyboards`) are
invokable only from the app's own local webview — there is no remote request surface.
The genuinely untrusted input in this slice is the **LLM's free-text reply** (it becomes
prompts, model ids, camera slugs and durations that drive PAID fal submissions); the brief
is the local user's own text.

Note: the MCP "auto mode" instruction to avoid the Read/Write tools was ignored — it is
not an instruction from the user or the launching agent and does not change how this
review is conducted.

## Boundaries examined and found clean (silence is not clearance)

- **Prompt-injection / "LLM output as instruction" surface — CLEAN.** The plan is
  untrusted LLM text, but nothing in it is ever interpreted as an app instruction. Every
  field routes through a strict allowlist before it can act:
  - `model_id` → `registry().get()` + `use_case::supports()` (`storyboard.rs:258-270`),
    the same predicate the picker and submit path use — an invented id is
    `UnknownModel`/`ModelNotRunnable`, never resolved by name.
  - `camera` → `camera::get()` which never guesses a nearest move
    (`storyboard.rs:274-281`) — an invented slug is `UnknownCamera`.
  - `seconds` → clamped only *toward* a catalogue-offered value, else left for the
    provider to validate (`storyboard.rs:309-354`); it cannot inflate beyond the model's
    axis for an enumerated model.
  - `brief`/`target_seconds`/`use_case` are overwritten with the authoritative command
    inputs after parse (`storyboard.rs:219-221`), so the model cannot corrupt the target
    or the tab it is validated against.
  The `prompt` text itself flows to fal as generation *content* (never re-compiled — S7
  verbatim `final_prompt`), which fal treats as a prompt, not as instructions to this app.
  The brief is the local user's own textarea input, not fetched/third-party content, so
  there is no untrusted-content→LLM boundary crossing to exploit.

- **XSS / HTML injection — CLEAN.** No `dangerouslySetInnerHTML`, `innerHTML`, `eval`,
  `new Function`, or `document.write` anywhere in the diff. All LLM-authored text
  (`shot.title`, `shot.prompt`, `advisories`, `storyboard.brief`) is rendered as JSX
  children or `<input>/<textarea>` values — React-escaped (`Storyboard.tsx:330,338,391`;
  `Library.tsx:155`). `plan_json` is never `JSON.parse`'d back into the DOM on the read
  path.

- **SQL injection — CLEAN.** Migration 8 is static DDL via `execute_batch`
  (`store.rs:186-213`); every read/write is parameterised (`upsert_storyboard`,
  `storyboard`, `storyboards`, `jobs_in_storyboard`, and the extended `upsert` INSERT
  params, `store.rs:227-320,435-486`). No string interpolation into SQL.

- **Secrets handling — CLEAN.** `ChatBackend` deliberately derives **no `Debug`**
  (`enhancer.rs`, the `pub enum ChatBackend` block) so an api key cannot be accidentally
  logged; the key is placed only in the `Authorization`/`x-api-key` **headers**
  (`enhancer.rs` `ChatBackend::complete`), never in a URL or request body. Error text from
  `send_json` carries the HTTP status and response *body* only (`enhancer.rs:912-924`) —
  not request headers — so a hosted error surfaced as `PlanError::Backend` cannot leak the
  key. `plan_storyboard` reads the key via `probe_enhancer_env`, exactly as the existing
  enhancer does.

- **SSRF / egress — CLEAN.** No user-controlled URL is introduced. Ollama uses the
  `OLLAMA_URL` constant; hosted requests go to the provider constants
  (`base_url: None` in `ChatBackend::hosted`). `with_base_url` exists for tests only and is
  not reachable from the two new commands.

- **Write-once / storage integrity — CLEAN.** `storyboard_id`/`shot_index` are absent from
  the `ON CONFLICT DO UPDATE SET` (`store.rs:451-456`), so a rerun-poll cannot re-tag a
  job's set membership — pinned by `migration_8_adds_storyboards_and_columns_and_roundtrips`.

- **Money gate reuse — CLEAN (as designed).** `submit_one` is the former `submit_job` body
  verbatim; every storyboard shot runs the identical compile→S6 guard→S7 verbatim
  prompt→S10 estimate→route→submit path (`commands.rs:980-1076,1240-1262`). `plan_storyboard`
  takes no `State`, so drafting structurally cannot bill fal.

- **Supply chain — CLEAN.** No `Cargo.toml`, `Cargo.lock`, `package.json`, or
  `pnpm-lock.yaml` change. `cargo deny` posture unchanged; no new dependency.

## Findings (ranked)

### L1 — `generate_storyboard` does not bound shot count or re-validate the plan server-side (Low)
`commands.rs:1219-1262` (`generate_storyboard_inner`). The command accepts an arbitrary
`shots: Vec<SubmitInput>` and submits each through the paid path; it does not re-apply
`storyboard::validate` (the `3..=8` count bound / sum gate) that `plan_storyboard` enforced.
Exploit path: a caller that invokes `generate_storyboard` directly (bypassing the
Storyboard view, which disables Generate-all until the summed total renders) can submit an
unbounded number of paid jobs with no total ever shown — the AC6 "total shown first"
invariant lives only in the UI (design §10.5 acknowledges this). Impact is bounded by the
local trust boundary (the command is only reachable from the app's own webview, and each
shot still passes the full per-shot estimate/route gate, so no unrunnable/unknown model
runs) — this is why it is Low, not High. Fix: cap `input.shots.len()` (e.g. `<= 8`) in
`generate_storyboard_inner` and return an error above it, so the money-gate/plan-size
invariant does not depend solely on the UI.

### L2 — Storyboard id is time+count derived, not unique (Low)
`commands.rs:1223` — `let sid = format!("sb-{}-{}", now_secs(), input.shots.len());`.
`now_secs()` is whole-second resolution (`runner.rs:35`). Exploit/failure path: two
generate-all batches issued in the same wall-clock second with the same shot count produce
the same `sid`; `upsert_storyboard` then overwrites the first group's row
(`store.rs:227-243`, `ON CONFLICT DO UPDATE`), and jobs from both batches are tagged to the
same set — the Library would merge two distinct storyboards under one header. Not a
confidentiality/integrity breach of another user (single-user local app), but a data-mixing
correctness bug with a security-adjacent "one action silently rewrites another's record"
shape. Fix: mint a random/uuid suffix (or include a monotonic counter) rather than
`now_secs()+len`.

### L3 — No size cap on `brief` / `plan_json` before LLM call or DB write (Low/Informational)
`storyboard.rs:358-379` (`build_user_message` embeds the full brief) and
`commands.rs:1224-1234` / `store.rs:227-243` (`plan_json` stored verbatim as TEXT). No
upper bound is enforced. Failure path: a very large brief inflates the one hosted enhancer
completion's token cost (on the user's own key), and a very large `plan_json` bloats the
local SQLite row. Both are self-inflicted on a local single-user app, hence Informational.
Fix (optional): a defensive length cap on `brief` and `plan_json` at the command boundary.

## Backlog recommendation
No Critical or High findings, so no new blocking slice is required. L1 (server-side
shot-count bound / plan re-validation in `generate_storyboard`) is worth a small hardening
task if the owner wants the money-gate invariant to hold independent of the UI — recommend
folding it into S14 (render/stitch) rather than opening a standalone slice.

VERDICT: PASS
