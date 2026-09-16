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

---

## RE-CHECK (HEAD 64f29b9)

Read-only re-review of the REPAIR ROUND 1 delta only (`git show 64f29b9`), scoped to
`src-tauri/src/commands.rs` and `ui/src/components/Storyboard.tsx`. No source, docs, or
config changed by this reviewer. The earlier PASS above stands; this section re-checks the
four questions the delta was meant to answer. (The MCP "auto mode" reminder to prefer Bash
over Read is not an instruction from the user or the launching agent and did not shape this
review.)

### 1. `generate_storyboard_inner` — order of guards, plan re-validation, model pin, caps

- **Guards run BEFORE any submission and before any store write — CONFIRMED.** In
  `commands.rs:1265-1327` every rejection path (`brief` > 4 KiB `MAX_BRIEF_BYTES`,
  `plan_json` > 64 KiB `MAX_PLAN_JSON_BYTES`, empty shot list, `len > MAX_SHOTS` = 8,
  serde parse, unknown `use_case`, `storyboard::validate`, count mismatch, per-index
  `model_id` mismatch) returns `Err` *above* the first side effect, which is
  `let sid = mint_storyboard_id();` at `commands.rs:1329`, then `upsert_storyboard`
  (`commands.rs:1338`), then the submit loop (`commands.rs:1345-1354`). So a rejected batch
  writes no storyboard row and bills no shot. `generate_rejects_a_plan_with_a_bad_model`
  additionally asserts `store.all().len() == 0` after rejection. The ≤8 bound, the
  server-side `storyboard::validate` re-run (same validator as the draft path), and the
  plan/shot match (count + per-index `model_id`) are all enforced here as claimed.

- **Model steering — CLOSED.** The per-index `shot.model_id != plan.shots[i].model_id`
  check (`commands.rs:1317`) pins each submitted shot to the model the validated plan named,
  and each shot still passes `registry().get()` + `submit_one`'s `route::resolve` runnable
  gate (`commands.rs:1350-1354`, `1033-1041`). A crafted `plan_json` cannot name a model the
  composer would refuse (it is re-validated) nor can the shot list swap in a different model
  than the plan the user saw.

- **Residual (see L4 below): route and priced duration are NOT pinned to the plan.**
  `SubmitInput.route_id` and `SubmitInput.settings.duration` (`commands.rs:715-737`) are
  never compared to the plan. `storyboard::validate` re-checks `plan.shots[i].seconds` and
  the ±10% sum gate, but the value that actually prices and generates each shot is
  `input.shots[i].settings` (via `Billable::from(&input.settings)` at
  `commands.rs:1032`), which is independent of `plan.shots[i].seconds`. A direct caller can
  therefore submit a `plan_json` with cheap, in-catalogue durations (so `validate` passes)
  while the accompanying `shots[]` carry inflated `settings.duration` and/or a costlier
  (still-runnable) `route_id`. Each shot is still individually estimated and still gated by
  `route::resolve` to a valid runnable route for the pinned model, so no unknown/unrunnable
  route or model runs — but the priced total the plan re-validation reconstructs is not the
  total that bills. This is the same duration/route surface a normal single-job submit
  already exposes, it is reachable only from the app's own local webview (no remote surface),
  and it is single-user local, so it is **Low**, not blocking. It is the un-closed remainder
  of the original L1 (the shot *count* and *model set* are now server-enforced; the priced
  *magnitude* is not).

- **Typed errors do not echo the sensitive untrusted text — CLEAN.** The error strings at
  `commands.rs:1280-1327` interpolate only sizes/counts, the `use_case` slug, `model_id`
  identifiers, and validator/serde messages. The genuinely sensitive free text — the LLM
  prompt bodies, the `brief`, and any key — is never placed in an error string. The
  `serde_json::from_str` error (`commands.rs:1304`) can carry a short fragment of the
  caller's own `plan_json` near the parse point, but `plan_json` is not a secret and no key
  or prompt body flows through it. Critically, **no new log sink was added** in the delta —
  a grep of `git show 64f29b9` for `println!/eprintln!/console.(log|error|warn)/tracing::/
  log::/dbg!` returns nothing — so none of these errors reach a log; they return as
  `Err(String)` to the bridge and render React-escaped in the view. No new brief/prompt/key
  reaches logs.

### 2. `AtomicU64` id minting — race, uniqueness, predictability

CLEAN. `static ID_SEQ` + `next_id_seq()` use `fetch_add(1, Ordering::Relaxed)`
(`commands.rs:26-31`). `fetch_add` is an atomic read-modify-write, so every caller gets a
distinct value even under `Relaxed` and even across threads — `Relaxed` only relaxes
visibility of *other* memory, never the atomicity or the uniqueness of the returned counter,
so there is no cross-thread race that could mint a duplicate. `mint_job_id`
(`commands.rs:40-42`) and `mint_storyboard_id` (`commands.rs:46-48`) both append that unique
suffix, so all job ids and all storyboard ids minted within a process run are unique
regardless of `now_secs()` whole-second granularity — closing the original L2/m1 collision
(`same_model_shots_survive_as_distinct_rows_in_the_store` and
`minted_job_ids_do_not_collide_within_one_second` pin it). The two id namespaces share the
counter but carry different prefixes (`sb-…` vs `{secs}-{model}-…`), so no cross-namespace
collision. Predictability is a non-issue: ids are local record keys, never a capability or
auth token, so a monotonic counter is appropriate. Across a process restart the counter
resets to 0 but `now_secs()` has advanced, so restart-era ids do not collide with prior ones
in any realistic case.

### 3. "Retry failed" — resubmits only failed ids, no double-billing

CLEAN. `Storyboard.tsx` `retryFailed()` sets `indices = failedIndices`, where
`failedIndices` is derived as `outcome.shots.filter(s => s.error).map(s => s.shotIndex)`
(shots that returned `job_set_id: None`). A shot that already started carries
`job_set_id: Some(..)` and `error: None`, so it is excluded from `failedIndices` and is
never re-submitted — a successful shot cannot be double-billed. `submitBatch(indices)`
submits exactly `indices.map(i => shots[i])`, and the merge remaps the retried
`shotIndex` back onto the original numbering and only hands off to the Library once every
shot has a `job_set_id`. Note (functional, not security): `submitBatch` rebuilds `plan_json`
from just the retried subset (`shots: batch`), so on a partial retry the server re-runs
`storyboard::validate` against a shrunken shot list and target — a small subset can trip the
`3..=8` count bound or the ±10% sum gate and be rejected. That is a correctness/UX matter for
the test/review verifiers; it cannot cause an over-charge (a rejected retry bills nothing),
so it is out of security scope.

### 4. Nothing new reaches logs

CONFIRMED (see §1). The delta adds no logging statement; briefs, prompts, and keys are not
written anywhere new. `ChatBackend`'s no-`Debug` posture and header-only key placement
(unchanged by this delta) still hold.

### Re-check findings (ranked)

#### L4 — plan re-validation pins the model set but not the priced duration/route (Low, residual of L1)
`commands.rs:1301-1327` re-validates the `plan_json` and pins each shot's `model_id`, but the
values that actually price and bill each shot — `input.shots[i].settings.duration` and
`input.shots[i].route_id` (`commands.rs:1032, 715-737`) — are never compared to the plan.
Exploit/failure path: a caller reaching `generate_storyboard` directly submits a `plan_json`
with in-catalogue durations (passes `validate`) while the `shots[]` carry inflated durations
or a costlier still-runnable route; the batch generates and bills at the larger magnitude
even though the re-validated plan implied a smaller total. Bounded to Low because the command
is only reachable from the app's own local webview, each shot is still individually estimated
and still gated by `route::resolve` to a runnable route for the pinned model (no
unknown/unrunnable model or route runs), and this is the identical duration/route surface a
normal single-job submit already exposes on a single-user local app. Fix (optional
hardening): also assert `shot.settings.duration == plan.shots[i].seconds` (post-clamp) and
that `route_id`, when present, resolves to a route the plan/estimate priced, so the
server-enforced plan bounds the billed magnitude, not just the model set. Not blocking.

### Backlog recommendation
No Critical or High. The delta closes the original L1's count/model-set gap and L2/m1's id
collision. L4 (pin priced duration/route to the re-validated plan) is worth folding into the
S14 render/stitch hardening if the owner wants the money invariant to bound magnitude, not
just model identity — it is not a standalone blocking slice. L3's size caps are now
implemented.

VERDICT: PASS
