# S13-storyboard-plan — DESIGN

Contract: `.forge/slices/S13-storyboard-plan/slice.md` (AC1–AC7).
Upstream this design is built on: `.forge/objective.md` (§Constraints stack rule, §A no-regression
gates, §"NOT in scope": no motion/multi-op beyond the one surface, no Runway recreation), and the
shipped slices this reuses wholesale — S4/S5 enhancer backends (`enhancer.rs`), the S6 refusal guard
(`.forge/slices/S6-enhancer-output-validation/design.md`), S7 preview + verbatim-submit
(`.forge/slices/S7-enhancer-prompt-preview/design.md`), S9 capability axes (`capability.rs`), S10
cost/provenance (`.forge/slices/S10-pricing-accuracy-and-cost-tracker/design.md`), and the S12 view
switch (`.forge/slices/S12-ui-redesign-code/design.md`). House style followed: S10 and S12 designs
(grounded `file:line`, decisions with rationale, per-AC test plan with named tests, exact gate
commands, order of work, open questions with defaults, riskiest parts).

Slice shape: **one new core module (`storyboard.rs`: a plan DTO + a strict text→JSON validator +
model-aware duration clamp), one minimal chat seam reusing the existing enhancer HTTP plumbing
(no new client), two new Tauri commands (`plan_storyboard`, `generate_storyboard`) that reuse the
S6/S7 guard and the existing submit machinery per shot, one store migration (8) for a `storyboards`
grouping table + two columns on the jobs table, and one new UI view (Storyboard) built from
components the app already has.** S14 renders/stitches — nothing here spawns ffmpeg.

---

## 0. Session constraint, stated before anything is claimed

Authored by reading the working tree at HEAD `bddc549` this session; every `file:line` below is
first-hand. Written **without running** `cargo`, `pnpm`, or `tauri build`, so every count and every
"still green" is a claim the TEST/VERIFY agents must confirm, not a measured fact. Baselines are the
contract's floor (`cargo test --workspace` ≥ 820, `pnpm test` ≥ 272), not readings.

**No money, no keys, no `first_light`.** Drafting a plan makes exactly one enhancer-LLM call (free
local Ollama, or the user's own hosted key — §D7); every plan test uses a **fake chat backend** and
touches no network. Generating shots reuses the existing paid submit path, which agents never run
against a real provider. `tauri build` produces a local `.app` only (README forbids bare
`cargo build` — blank window).

### 0.1 Ground truth, as read this session

| Fact | Read at |
|---|---|
| The enhancer backends have **no JSON mode**: `openai_body`/`anthropic_body`/`ollama_body` send a plain `system`+`user` chat, no `response_format`/`tool` field | `enhancer.rs:1002-1012,1401-1410,1447-1456` |
| The raw wire round trip is already factored into private free fns taking `(model, system, user)` + text readers (`ollama_text`/`openai_text`/`anthropic_text`), sharing `client()`/`send_json()` | `enhancer.rs:986-1052,1361-1483,899-922` |
| `clean_reply` already strips a ```` ```json ```` fence and one matched quote pair, and refuses a sentinel — useful for JSON | `enhancer.rs:810-862` |
| The S6 guard lives in `enhance_or_original` via `refusal_reason` (anchored to reply start) | `enhancer.rs:516-555,609-668` |
| S7 `preview_prompt` takes **no `State`** (cannot bill), reuses `harness::compile`; `SubmitInput.final_prompt` sends verbatim, never re-compiled | `commands.rs:928-963,700-706,853-904` |
| Submit body: `compiled_for_submit` → `Billable::from(&settings)` → `route::resolve` → `estimate_with_provenance` → build `JobSet` → `store.upsert` → `submit_to_provider` → `runner.watch` | `commands.rs:966-1053` |
| `JobSet` is the **jobs** row; the SQLite table storing it is literally named `job_sets` | `engine.rs:19-105`, `store.rs:65` |
| Latest store migration is **7**; next is **8**. Added columns follow the `#[serde(default)]` + `ALTER TABLE … DEFAULT` + `unwrap_or_default` row-map pattern; write-once columns are absent from the `ON CONFLICT DO UPDATE SET` list (like `estimated_usd`/`priced_from`) | `store.rs:161-176,197-262,288-348` |
| Runnable-per-tab: `use_case::supports(&Model, UseCase)` and `use_case::models_for(uc)`; `UseCase::{from_slug,output,input_mode}` | `use_case.rs:166-265` |
| Duration axis (S9): `capability::for_route(spec, provider, slug, mode, \|_\|None)` → catalogue-sourced offline; `ModelCapability::{duration_seconds()->Option<Vec<f64>>, default_duration_seconds(), duration.is_free_form()}` | `capability.rs:538-549,283-286,581-595` |
| Camera slug existence: `camera::get(slug) -> Option<&CameraTemplate>` (25 slugs via `camera::slugs()`); `get` deliberately never falls back to a nearest move | `camera.rs:309-316` |
| Per-shot estimate: `estimate_cost` / `estimate_with(prices, model_id, route_id, &settings)`; unknown is `None`, never `$0` (S10) | `commands.rs:642-667` |
| Storyboard system prompt would live in `prompts/` and be `include_str!`'d, like the enhancer corpus | `corpus.rs:42-71` |
| UI view axis is `view: "studio"\|"library"` in `App.tsx`; the titlebar `view-switch` renders **only** Studio + Library (no Storyboard entry today) | `App.tsx:136,636-653` |
| UI bridge conventions: DTOs camelCase, commands wrapped `{ input }`, `!isDesktop()` returns a mock; `previewPrompt`, `submitJob`, `estimateCost`, `listJobs` established | `api.ts:503-523,621-680` |
| Reusable cost UI: `stageCostCaption(job)`, `formatUsd`, `usable`, `costDelta` in `lib/cost.ts` | `lib/cost.ts:11-209` |

**Correction to the contract's Ground truth (re-verified this session):** the contract says a
"Storyboard titlebar entry exists as a placeholder label only". It does **not** — `App.tsx:636-653`
renders Studio and Library only. AC3 therefore *adds* a third `view-switch` tab, it does not un-grey
an existing one (§D5). This is exactly the kind of stale `file:line` the contract warned to re-check.

---

## 1. Decisions

### D1 — The plan DTO, the chat seam, and the strict validator

**D1.1 — There is no JSON mode, so the plan is parsed from free text with a strict validator.**
`enhancer.rs` sends a plain `system`+`user` chat to all three backends (`:1002,1401,1447`) with no
`response_format`. Adding structured-output/tool-call plumbing is explicitly out of scope (slice
Non-goals). **Decision:** the storyboard system prompt (a new `prompts/storyboard.v1.md` corpus,
`include_str!`'d like the enhancer corpus, `corpus.rs:42`) instructs the model to emit **only** a
JSON object; the reply is run through the existing `clean_reply` (`enhancer.rs:810`, which already
unwraps a ```` ```json ```` fence and a quote pair) and then `serde_json::from_str` into
`StoryboardPlan`; anything that does not parse is a typed error (D1.4). Reusing `clean_reply` is
free and closes the most common real failure (a model wrapping JSON in a fence).

**D1.2 — The LLM call goes through the existing backend plumbing via a minimal `Chat` seam; no new
HTTP client.** The `Enhancer` trait (`enhancer.rs:486`) is the wrong shape to reuse directly: its
`rewrite` bakes `EnhanceRequest`→`user_message` framing (`enhancer.rs:684-714`), a baked
`self.system_prompt`, the notes-sentinel split, and the recompile step — none of which fit a
storyboard brief. **Decision:** add a small trait in `enhancer.rs`

```rust
/// One system+user chat round trip, returning the raw assistant text.
/// The seam the storyboard planner (and any future structured caller) uses
/// instead of the rewrite-shaped `Enhancer` trait.
pub trait Chat: Send + Sync {
    fn complete(&self, system: &str, user: &str) -> Result<String, JobError>;
}

/// A concrete backend selection with credentials, implementing `Chat` by
/// reusing the *existing* wire functions (`ollama_body`/`openai_body`/
/// `anthropic_body` + their text readers + `client`/`send_json`). No new HTTP.
pub enum ChatBackend {
    Ollama   { model: String, base_url: String },
    Hosted   { backend: HostedBackend, api_key: String, model: String, base_url: Option<String> },
}
impl Chat for ChatBackend { fn complete(&self, system: &str, user: &str) -> Result<String, JobError> { /* reuse */ } }
```

`ChatBackend::complete` MUST call the same private free fns the enhancer already uses (they live in
`enhancer.rs`, so no visibility change and no duplication of wire routes). It reuses
`with_base_url`-style overrides so a test stub can point either backend at a local server exactly as
`HostedEnhancer::with_base_url` (`enhancer.rs:1340`) and `LocalEnhancer::with_base_url`
(`enhancer.rs:953`) already allow. **A fake for tests** is any `struct FakeChat(Result<String, JobError>)`
implementing `Chat` — the storyboard planner depends on `&dyn Chat`, so plan tests run with a canned
reply and no network. **Rejected:** refactoring `HostedEnhancer::call`/`LocalEnhancer::rewrite` to
delegate through `ChatBackend` in this slice — a nice cleanup but it churns the S4/S5 enhancer tests
for no AC; recorded as Q1 (deferred, one-way door left open).

**D1.3 — `plan_storyboard` command + the plan DTO.** New core module
`crates/hickeyfield-core/src/storyboard.rs` owns the serde types and pure logic; the Tauri command
in `commands.rs` owns backend selection (impure) and mirrors `preview_prompt`'s no-money shape as
far as possible — but it needs the vault (a hosted key) so it reads it the same way
`probe_enhancer_env` (`commands.rs:824`) does. It never touches `state.store`/fal.

```rust
// storyboard.rs — serde camelCase for the bridge.
#[derive(Serialize, Deserialize, Clone, PartialEq)] #[serde(rename_all = "camelCase")]
pub struct Shot {
    pub title: String,
    pub prompt: String,
    pub seconds: f64,
    pub camera: Option<String>,          // a camera::slug, or None
    pub model_id: String,                // must exist + be runnable for the tab
    pub role: ShotRole,                  // start | middle | end
    #[serde(default)] pub continuity: Option<Continuity>, // recorded, not applied (D6)
    #[serde(default)] pub advisories: Vec<String>,        // clamp notes (D2)
}
#[derive(Serialize, Deserialize, Clone, PartialEq)] #[serde(rename_all = "camelCase")]
pub struct StoryboardPlan {
    pub brief: String,
    pub target_seconds: f64,
    pub use_case: String,                // the tab slug the plan is for
    pub shots: Vec<Shot>,
}
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq)] #[serde(rename_all = "snake_case")]
pub enum ShotRole { Start, Middle, End }
```

`StoryboardDto` returned to the UI is `StoryboardPlan` verbatim (already camelCase). The command:

```rust
#[tauri::command]
pub fn plan_storyboard(brief: String, target_seconds: f64, use_case: String,
                       model_prefs: Vec<String>, rewriter: Option<RewriterChoice>)
    -> Result<StoryboardPlan, String>
```

Backend selection reuses `probe_enhancer_env(true)` + a new `chat_backend_for(rewriter, key, up,
&models) -> Option<ChatBackend>` (the `ChatBackend` analogue of `select_rewriter`,
`commands.rs:739`). `None` (nothing reachable / `Rewriter::None`/`Unavailable`) ⇒ the command returns
the honest note as `Err` (AC1 "on an unavailable backend returns an honest error"). It then calls
`storyboard::plan(chat, &corpus::STORYBOARD_SYSTEM, &brief, target_seconds, uc, &model_prefs)`.

**D1.4 — The validator (`storyboard::validate`), and the typed errors.** Pure, in `storyboard.rs`,
run inside `plan` after `clean_reply`+parse. It reuses existing registry/use-case/camera primitives
so the picker, submit path, and validator cannot disagree:

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum PlanError {
    Backend(String),                              // unavailable / provider error (AC1)
    Refused(&'static str),                        // refusal_reason fired on the reply (S6)
    NotJson(String),                              // clean_reply/serde failed — carries a short cause
    ShotCount(usize),                             // not in 3..=8
    DurationSum { sum: f64, target: f64 },        // outside ±10%
    UnknownModel { shot: usize, model_id: String },
    ModelNotRunnable { shot: usize, model_id: String, use_case: String },
    UnknownCamera { shot: usize, slug: String },
}
```

Rules (MUST), in order:
1. **Refusal guard (S6).** Run `enhancer::refusal_reason(&raw)` on the assistant reply *before*
   parsing (`enhancer.rs:609`); `Some(reason) ⇒ PlanError::Refused` — a refusal is not garbage JSON,
   it is a decline, and the S6 detector already recognises it model-agnostically.
2. **Parse.** `clean_reply` then `serde_json::from_str::<StoryboardPlan>` ⇒ `NotJson(cause)` on
   failure. (Garbage / half-JSON lands here.)
3. **Shot count** `3..=8` else `ShotCount(n)`.
4. **Every model id exists and is runnable for the tab.** `registry::registry().get(&shot.model_id)`
   ⇒ `UnknownModel`; then `use_case::supports(model, UseCase::from_slug(&use_case)?)`
   (`use_case.rs:166`) ⇒ `ModelNotRunnable`. This is the *same* predicate the picker
   (`models_for_use_case`, `commands.rs:403`) and `route_serves` use, so a plan can never name a
   model the composer would refuse.
5. **Camera slug exists or is `None`.** `shot.camera.as_deref().map(camera::get)` — `Some(None)` ⇒
   `UnknownCamera` (`camera.rs:309`; it never guesses a nearest move — a wrong camera is worse than
   none).
6. **Duration sum within ±10% of target.** `let sum = Σ shot.seconds;` fail if
   `(sum - target).abs() > 0.10 * target`. Checked on the **LLM's proposed** seconds (the drafting
   quality gate), *before* the model-aware clamp (D2), which then adjusts individual shots and
   surfaces advisories rather than failing the plan. Ordering recorded in Q2.

Worked example (accept): brief "a 30-second ad for a cold-brew can", `target_seconds = 30`, tab
`text-to-video`, `model_prefs = ["seedance_2_5","ltx_2_5"]`. The model returns fenced JSON with 4
shots of `8+8+8+6 = 30`s, each `modelId` in the prefs and runnable for `text-to-video`, cameras
`push-in`/`none`/`orbit`/`static`. `refusal_reason` → `None`; `clean_reply` drops the fence; parse
ok; count 4 ∈ 3..=8; every model runnable; cameras exist; `|30-30| = 0 ≤ 3`. → `Ok(plan)`.

Worked example (reject, bad model): a shot names `"kling3_0_pro"` under tab `image-to-video`.
`registry.get` succeeds but `use_case::supports(kling3_0_pro, ImageToVideo)` is `false`
(`use_case.rs:323`) ⇒ `ModelNotRunnable { shot: 2, model_id: "kling3_0_pro", use_case: "image-to-video" }`.
The command maps `PlanError` to a human `Err(String)` at the boundary (Tauri commands return
`Result<_, String>`, as `submit_job`/`preview_prompt` do) via `impl Display for PlanError`.

### D2 — Model-aware duration clamp (AC2)

After validation passes, each shot's `seconds` is clamped to the chosen model's duration axis, reusing
S9 capability data offline (catalogue-sourced — no network, no key): for `model_prefs`/`shot.model_id`
resolve `cap = capability::for_route(&model.spec, route.provider, &route.slug, uc.input_mode(),
|_| None)` (`capability.rs:581`; `|_|None` ⇒ catalogue fallback, the same call `list_models` makes to
avoid 68 HTTP requests). Then, in `storyboard::clamp_shot`:

- **Enumerated axis** (`cap.duration_seconds() == Some(vals)` non-empty): snap to the **nearest**
  member; if it changed, push `"shot N: {model} runs {picked}s, not {asked}s — clamped"` onto
  `shot.advisories`.
- **Free-form axis** (`cap.duration.is_free_form()`, e.g. Seedance 2.5 native 4–30s): leave as-is —
  the provider validates the exact number. This is what lets a **single-shot 30s plan** on Seedance
  keep its 30s (slice AC2, "single-shot native 30s").
- **No duration control** (`cap.duration.support` is `No`, e.g. a still model): N/A — image shots
  carry no seconds that bill; leave `seconds` untouched and add no advisory.
- **Unknown axis**: leave as-is, no advisory (inventing a clamp from an unknown axis is the S9 harm).

Worked example (AC2's named case): a 12s shot on `ltx_2_5` (LTX enumerates 6/8/10s, `slice.md:19`).
`duration_seconds() == Some([6,8,10])`; nearest to 12 is 10 ⇒ `seconds = 10.0`, advisory
`"shot 3: LTX 2.5 runs 10s, not 12s — clamped"`. The clamp is **advisory + applied**, matching the
S9 rule that a duration we cannot honour is never silently priced.

### D3 — Storage: migration 8, and a name collision the contract missed

**The contract asks for a `job_sets` table — but that name is already the jobs table** (`store.rs:65`;
the `JobSet` row store). Reusing it would collide. **Decision:** the new grouping table is
**`storyboards`**, and the two columns land on the existing `job_sets` (jobs) table:

```sql
-- migration 8 (store.rs, after the `version < 7` block at :161-176)
CREATE TABLE IF NOT EXISTS storyboards (
    id             TEXT PRIMARY KEY,
    brief          TEXT NOT NULL,
    target_seconds REAL NOT NULL,
    plan_json      TEXT NOT NULL,       -- StoryboardPlan, serde round-trip (AC5)
    created_at     INTEGER NOT NULL
);
ALTER TABLE job_sets ADD COLUMN storyboard_id TEXT;       -- NULL for standalone jobs
ALTER TABLE job_sets ADD COLUMN shot_index    INTEGER;    -- position within the set
PRAGMA user_version = 8;
```

- `JobSet` (`engine.rs:19`) gains `#[serde(default)] pub storyboard_id: Option<String>` and
  `#[serde(default)] pub shot_index: Option<u32>` — `default` so pre-v8 rows deserialise (the exact
  S6→S10 pattern). **Write-once**: added to the INSERT column list + params in `store.rs:288-348` but
  **absent** from the `ON CONFLICT DO UPDATE SET` (a job's set membership never changes after submit,
  mirroring `estimated_usd`/`priced_from`). `row_to_job` (`store.rs:203`) reads both with
  `.ok().flatten()`/`unwrap_or(None)`.
- `SqliteStore` gains `upsert_storyboard(&Storyboard)`, `storyboard(&str) -> Option<Storyboard>`, and
  `jobs_in_storyboard(&str) -> Vec<JobSet>` (ordered by `shot_index`). `Storyboard` is a small shell
  struct `{ id, brief, target_seconds, plan_json, created_at }`; `plan_json` is
  `serde_json::to_string(&StoryboardPlan)`.
- **Library grouping (AC4):** `list_jobs` already returns every `JobSet` with the new
  `storyboardId`/`shotIndex` fields, so the UI Library groups client-side by `storyboardId` (no new
  read command needed — same principle as S12 D5.1: the Library is a re-presentation of `jobs`). A
  `list_storyboards` command is added only to fetch the brief/plan for the group header (§D5).

Worked example (round-trip, AC5): a `StoryboardPlan` with a `continuity` field on shot 2 is
`to_string`'d into `storyboards.plan_json`; `upsert_storyboard` then `storyboard(id)` then
`serde_json::from_str` returns an equal `StoryboardPlan` — the migration-8 test asserts equality and
that `PRAGMA user_version == 8`, and that a standalone job (no storyboard) loads `storyboard_id: None`.

### D4 — Generate all: reuse the submit path per shot, never bypass the money gate (AC4/AC6)

**Decision:** factor the body of `submit_job` (`commands.rs:966-1053`) into a private
`submit_one(state, model, input, storyboard_id: Option<&str>, shot_index: Option<u32>) -> Result<String, String>`
that does compile→estimate→route→upsert→submit→watch exactly as today, tagging the `JobSet` with the
two new fields. `submit_job` becomes `submit_one(.., input, None, None)` — so **single-job submit is
byte-for-byte unchanged** (its existing tests, `commands.rs:1640+`, stand; AC4 "submit path unchanged").

New command:

```rust
#[tauri::command]
pub fn generate_storyboard(state: State<'_, AppState>, input: StoryboardGenerateInput)
    -> Result<StoryboardResult, String>
```

where `StoryboardGenerateInput { brief, target_seconds, shots: Vec<SubmitInput> }` — each shot is a
plain `SubmitInput` (reused verbatim: it already carries `model_id`, `route_id`, `prompt`,
`preset_id` for the camera, `settings` for the seconds, `final_prompt` for a previewed/edited shot,
`media`). The command:

1. Mint a `storyboard_id`; `store.upsert_storyboard(&Storyboard{..})` with the plan JSON **first**, so
   the group survives even if a shot fails.
2. For each shot `i`, call `submit_one(state, model_i, &shots[i], Some(&sid), Some(i))`. Each shot
   runs the **full existing gate**: `compiled_for_submit` (S6 guard + S7 verbatim `final_prompt`),
   `Billable::from` + `estimate_with_provenance` (S10 pricing), `route::resolve` (adapter/route
   check), `submit_to_provider`. No shot bypasses estimate or the guard.
3. **Failure handling (decision):** a shot whose compile/guard fails or whose submit errors is
   recorded as a Failed `JobSet` row (the existing `Err` arm of `submit_job`, `commands.rs:1045-1051`)
   tagged to the storyboard, and the batch **continues**. The command returns
   `StoryboardResult { storyboard_id, shots: Vec<ShotOutcome> }`, `ShotOutcome = { shot_index,
   job_set_id: Option<String>, error: Option<String> }`. **Rationale:** a refusal on shot 3 must not
   discard shots 1–2 the user has already paid to start — the same "a failed optional step never
   blocks/erases a paid generation" invariant the enhancer funnel holds (`enhancer.rs:11-18`).
   All-or-nothing recorded as Q3.

**Money honesty (AC6):** `generate_storyboard` prices each shot (via `submit_one`'s existing
`estimate_with_provenance`) exactly as a single submit does — it invents no total and skips no
estimate. The **total shown before submit** is the UI's job (§D5): Generate-all is disabled until the
summed estimate is rendered. No shot reaches a model S9 marks unrunnable, because the validator
(D1.4 rule 4) and `route::resolve` both gate on the same runnable predicate.

### D5 — UI: the Storyboard view (AC3)

**A third top-level view.** `App.tsx:136` `view` becomes `"studio" | "library" | "storyboard"`; the
titlebar `view-switch` (`App.tsx:636-653`) gains a third `<button>` (Storyboard), and
`.workspace[data-view="storyboard"]` gets a grid template in `app.css` (nav · storyboard-panel,
following the S12 per-view-template pattern, `.forge/slices/S12/design.md` §D2). New component
`ui/src/components/Storyboard.tsx`; its CSS joins an existing sheet (no new `app.css` import).

- **Header:** a brief `<textarea>`, a target-length input (seconds), and a **Draft** button →
  `planStoryboard({ brief, targetSeconds, useCase: tab, modelPrefs, rewriter })`. `useCase` is the
  current use-case `tab` (so the plan is validated against the tab the user is on). `modelPrefs` are
  the runnable models for that tab (from the existing `modelsForUseCase(tab)` state, `App.tsx`).
- **Shot cards** (one per `plan.shots[i]`, local `useState<Shot[]>`): each editable —
  `prompt` (textarea), `seconds` (number), `camera` (reuse the S12 **camera strip** / preset picker,
  `SettingsRail` camera block), `model` (reuse the existing **`ModelPicker`** overlay, unchanged DOM —
  its `onSelect(modelId, routeId)` pins the shot). **Reorder** (up/down buttons updating array order,
  which also renumbers `role`/continuity per D6), **add** (clone-with-defaults), **delete**.
- **Per-shot Preview** → the S7 `previewPrompt({ modelId, prompt, settings, presetId: camera, … })`
  (`api.ts:652`) rendered inline in the card, reusing the `PromptPreview` component
  (`PromptPreview.test.tsx`-pinned classes untouched). Editing the previewed text sets that shot's
  `finalPrompt` (S7 verbatim path).
- **Total estimate** = Σ per-shot `estimateCost({ modelId, routeId, settings })` (`api.ts:503`),
  formatted with `formatUsd`; any shot returning `null` marks the total **"unpriced"** via a new pure
  helper `totalCostCaption(estimates)` in `lib/cost.ts` (built on the existing `usable`/`formatUsd`,
  `lib/cost.ts:20,185`) — reusing the S10/S12 honesty rule that an unknown price is never summed as
  `$0`. **Generate all** shows the total and calls `generateStoryboard`, then switches to the Library
  view (grouped by `storyboardId`) so the user watches the set run.
- **Library grouping (AC4):** `Library.tsx` (S12) renders a storyboard's jobs as one group when
  `job.storyboardId` is set, headed by the brief from `listStoryboards()`. Additive; standalone jobs
  render exactly as today.

**New api.ts functions** (camelCase DTOs, `{ … }` args, `!isDesktop()` mock, per `api.ts` house
rules): `planStoryboard`, `generateStoryboard`, `listStoryboards`. **New types.ts:** `Shot`,
`StoryboardPlan`, `StoryboardResult`, `ShotOutcome`, `Storyboard`; `storyboardId?/shotIndex?` on
`JobSet`.

**Test-locked classes reused (MUST NOT rename):** `ModelPicker`'s `li.model-item` /
`button.model-item-main` / `button.chip-route` / `onSelect` (`ModelPicker.test.tsx`), `PromptPreview`'s
`textarea.prompt-preview-text` / `button.prompt-preview-retry` / `button.generate-button`
(`PromptPreview.test.tsx`), the camera strip's `CameraPreview` (`CameraPreview.test.ts`). New
Storyboard markup adds classes; it renames none.

### D6 — Continuity recorded, not applied (AC5)

Each shot carries `role: start|middle|end` and, when a previous shot exists, a `continuity` plan
field. **Decision:** `continuity` is a recorded intent, not an operation:

```rust
#[derive(Serialize, Deserialize, Clone, PartialEq)] #[serde(rename_all = "camelCase")]
pub struct Continuity { pub last_frame_of: usize }   // "chain from shot N's last frame"
```

The planner sets `continuity = Some(Continuity { last_frame_of: i-1 })` for every shot after the
first (default; reorder/add/delete in the UI renumbers it). It is stored in `plan_json` and **never
touched by `generate_storyboard`** — no last-frame extraction, no `end_image_url` binding, no media
attach. S14 reads it to chain clips. AC5's requirement is only that `role` + `continuity` **round-trip
through the store** (the migration-8 plan-JSON test, §D3). Rationale: applying continuity needs the
last-frame-extract + render pipeline that is S14's whole point; recording it now is the thin,
honest seam and adds no false capability. Recorded as N-goal N2 and Q4.

### D7 — Cost of drafting (AC6, documented)

Drafting a plan is **one enhancer-LLM call** (D1.2). On **local Ollama** it is free and keyless. On a
**hosted backend** (the user's own OpenAI/Anthropic key) it costs whatever that one short completion
costs on the user's own account — the same call the enhancer already makes on every previewed submit,
no different in kind. **Decision:** document this in `README`/`docs` and surface a one-line note in the
Storyboard header ("Drafting uses your enhancer — free on local Ollama; a hosted key bills one short
completion"). No new billing surface, no fal spend at draft time (the command has no `State`/fal).

---

## 2. Files touched (complete; nothing else changes)

**Core (`crates/hickeyfield-core/src/`):**
- `enhancer.rs` — add `Chat` trait + `ChatBackend` (reusing existing wire fns) + tests. `Enhancer`
  and its tests untouched.
- `storyboard.rs` — **new**: `StoryboardPlan`/`Shot`/`ShotRole`/`Continuity`/`PlanError`,
  `plan(&dyn Chat, …)`, `validate`, `clamp_shot`, `Display for PlanError`, tests (with `FakeChat`).
- `corpus.rs` — add `STORYBOARD_SYSTEM` (`include_str!("../../../prompts/storyboard.v1.md")`) + a
  token-budget test (mirrors `corpus.rs:88`).
- `lib.rs` (core) — `pub mod storyboard;` and re-exports.
- `engine.rs` — two `#[serde(default)]` fields on `JobSet`.

**Prompts:** `prompts/storyboard.v1.md` — **new** corpus (system prompt: role, the exact JSON schema,
3–8 shots, sum-to-target, camera-slug list, model-id list guidance).

**Shell (`src-tauri/src/`):**
- `commands.rs` — `plan_storyboard`, `generate_storyboard`, `list_storyboards` commands;
  `chat_backend_for` helper; factor `submit_one` out of `submit_job`; `StoryboardGenerateInput` /
  `StoryboardResult` / `ShotOutcome` DTOs; tests.
- `store.rs` — migration 8; `Storyboard` struct; `upsert_storyboard`/`storyboard`/
  `jobs_in_storyboard`; INSERT columns + params for `storyboard_id`/`shot_index`; `row_to_job`; tests.
- `lib.rs` (shell) — register the three new commands (`:64-90`).

**UI (`ui/src/`):**
- `App.tsx` — `view` third value + titlebar tab + storyboard state/handlers.
- `app.css` — `.workspace[data-view="storyboard"]` template (additive).
- `components/Storyboard.tsx` — **new**; CSS in an existing sheet.
- `components/Library.tsx` — additive storyboard grouping.
- `api.ts` — `planStoryboard`/`generateStoryboard`/`listStoryboards` + mocks.
- `types.ts` — new interfaces; `storyboardId?`/`shotIndex?` on `JobSet`.
- `lib/cost.ts` — `totalCostCaption` helper.
- Tests: `storyboard` core tests, `store` migration-8 test, `commands` plan/generate tests,
  `Storyboard.test.tsx`, `cost.test.ts` (+`totalCostCaption`), api mapper tests.

**Docs:** `CHANGELOG.md`, `README.md`, `.forge/slices/S13-storyboard-plan/document.md`.

**Not touched (recorded):** `engine.rs`/`runner.rs` job lifecycle logic — **N/A, nothing here changes
polling/download/reattach** (only the two `JobSet` fields, serialized). `compositor.rs`/`ffmpeg.rs` —
**N/A, no render in S13.** `harness::compile` — **N/A, reused verbatim** by `submit_one` and
`preview_prompt`.

---

## 3. Data flow (brief → clips on disk)

`Storyboard.tsx` brief + target + `tab` → `planStoryboard` → `plan_storyboard` command →
`chat_backend_for(select…)` → `storyboard::plan(&dyn Chat, STORYBOARD_SYSTEM, …)` →
`clean_reply`→parse→`validate`→`clamp_shot` → `StoryboardPlan` → UI renders shot cards. User edits
(prompt/seconds/camera/model), optionally per-shot `previewPrompt` (S7) → sets `finalPrompt`. UI sums
`estimateCost` per shot → total. **Generate all** → `generateStoryboard({brief,targetSeconds,shots})`
→ `upsert_storyboard` then `submit_one` per shot (compile+guard+estimate+route+submit+watch, tagged
`storyboard_id`/`shot_index`) → each clip downloads to disk via the **unchanged** runner → Library
groups the set by `storyboardId`. S14 later reads `plan_json.continuity` to chain/stitch.

---

## 4. Test plan — every AC to a named test

Rust tests live in `#[cfg(test)] mod tests` beside the code; UI tests use the repo's no-RTL
`react-dom/client` + jsdom pattern (`ModelPicker.test.tsx`).

| AC | test | file | asserts |
|---|---|---|---|
| AC1 | `a_valid_plan_is_accepted` | `storyboard.rs` | `FakeChat` returns good fenced JSON (4 shots, sum=target, runnable models, real cameras) ⇒ `Ok`; no network |
| AC1 | `a_bad_model_id_is_rejected_with_a_reason` | `storyboard.rs` | shot names a model unrunnable for the tab ⇒ `ModelNotRunnable{shot,model_id,use_case}` |
| AC1 | `a_sum_outside_ten_percent_is_rejected` | `storyboard.rs` | shots sum 20s for a 30s target ⇒ `DurationSum{sum:20,target:30}` |
| AC1 | `a_refusal_is_a_typed_error_not_garbage` | `storyboard.rs` | `FakeChat` returns "I can't help with that." ⇒ `Refused(..)` (via S6 `refusal_reason`) |
| AC1 | `garbage_that_is_not_json_is_rejected` | `storyboard.rs` | non-JSON prose ⇒ `NotJson(..)` |
| AC1 | `an_unknown_camera_is_rejected` | `storyboard.rs` | a shot with `camera:"zoomy"` ⇒ `UnknownCamera` |
| AC1 | `an_unavailable_backend_is_an_honest_error` | `commands.rs` | `plan_storyboard` with no reachable backend ⇒ `Err` carrying the NOTE_* note |
| AC2 | `a_twelve_second_ltx_shot_clamps_to_ten_with_an_advisory` | `storyboard.rs` | `clamp_shot` on `ltx_2_5` @12s ⇒ `seconds==10`, advisory names both |
| AC2 | `a_single_shot_seedance_thirty_survives_free_form` | `storyboard.rs` | Seedance 2.5 free-form axis ⇒ 30s kept, no advisory |
| AC3 | `draft_renders_cards_and_edits_update_the_total` | `Storyboard.test.tsx` | plan → N cards; editing a shot's seconds re-sums the total |
| AC3 | `an_unpriced_shot_marks_the_total` | `Storyboard.test.tsx` | one shot's `estimateCost`→null ⇒ total shows "unpriced" tone |
| AC4 | `migration_8_adds_storyboards_and_columns_and_roundtrips` | `store.rs` | `user_version==8`; `Storyboard` + `plan_json` round-trip; a job tagged `storyboard_id/shot_index` round-trips; write-once |
| AC4 | `jobs_in_a_storyboard_are_ordered_by_shot_index` | `store.rs` | three tagged jobs return in `shot_index` order |
| AC4 | `single_job_submit_is_unchanged` (reuse) | `commands.rs` | existing submit tests still green; `submit_one(..,None,None)` path identical |
| AC5 | `a_plan_with_continuity_and_roles_round_trips` | `store.rs`/`storyboard.rs` | `role` + `continuity.lastFrameOf` survive serde + store |
| AC6 | `no_shot_is_submitted_to_an_unrunnable_model` | `storyboard.rs`/`commands.rs` | validator + `route::resolve` both reject; drafting reads no fal/State |
| AC6 | `generate_records_the_set_even_if_a_shot_fails` | `commands.rs` | one bad shot ⇒ its outcome carries `error`, others carry `job_set_id`, storyboard row persists |
| AC7 | the gates (§5) | — | 0 failed, `cargo` ≥ 820, `pnpm` ≥ 272 |

**Non-vacuity / mutation checks** (named so they are not invented late): flip the `3..=8` bound to
`0..=8` ⇒ `ShotCount` test fails to fire; loosen ±10% to ±100% ⇒ `DurationSum` test fails; drop the
`refusal_reason` pre-check ⇒ `a_refusal_is_a_typed_error` becomes `NotJson` (proves the S6 wiring);
remove `storyboard_id` from the INSERT list ⇒ the round-trip test fails; add `storyboard_id` to the
`ON CONFLICT UPDATE SET` ⇒ a write-once assertion fails.

**AC7 screenshots:** as S12 — no automated screenshot tool exists; the VERIFY agent launches the
`tauri build` `.app`, navigates to the Storyboard view, and captures/describes it in `test.md`.

---

## 5. Gate commands (AC7), exact

Node 24 at
`/private/tmp/claude-501/-Users-jorge-Documents-GitHub-hickeyfield/79be25e2-8f71-4192-9e6d-cd0e9d068f1c/scratchpad/node-v24.8.0-darwin-arm64/bin`
(prepend to `PATH`).

```
# dummy keys so shell tests that read a key env do not skip/panic:
export hickeyfield_FAL_KEY=x hickeyfield_HIGGSFIELD_KEY=x hickeyfield_HIGGSFIELD_SECRET=x
cargo test --workspace                                   # 0 failed, >= 820 + new tests
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
./scripts/lint-provenance.py                             # storyboard.v1.md is original prose — 0
env -u hickeyfield_FAL_KEY -u FAL_KEY cargo run -q -p hickeyfield-core --example audit_fal  # PINNED 0 / UNREACHABLE 0 unchanged (no schema/mode logic touched)
export PATH="/private/tmp/claude-501/-Users-jorge-Documents-GitHub-hickeyfield/79be25e2-8f71-4192-9e6d-cd0e9d068f1c/scratchpad/node-v24.8.0-darwin-arm64/bin:$PATH"
cd ui && pnpm test && pnpm build                         # vitest 0 failed >= 272; tsc + vite build
ui/node_modules/.bin/tauri build --bundles app           # from repo root; the .app launches (window, not blank)
```

`cargo deny check` is **N/A to re-argue** — no dependency/manifest change (the plan uses `serde_json`,
already a dep) — but MUST stay green. **Never** `first_light` (spends money, no key). **Never** bare
`cargo build` for a runnable app.

---

## 6. Order of work (safest seams first; each step leaves the workspace green)

1. **Core validator, offline** (§D1.3/D1.4/D2): `storyboard.rs` types + `validate` + `clamp_shot` +
   `plan(&dyn Chat)` + `FakeChat` tests. Pure, no shell, no UI — the whole AC1/AC2 surface provable
   here first.
2. **`Chat`/`ChatBackend` in `enhancer.rs`** (§D1.2) reusing the wire fns; `storyboard.v1.md` corpus +
   budget test.
3. **Store migration 8** (§D3): `storyboards` table + `JobSet` fields + round-trip test.
4. **Commands** (§D1.3/D4): `plan_storyboard`, factor `submit_one`, `generate_storyboard`,
   `list_storyboards`; register in `lib.rs`. Prove single-job submit unchanged.
5. **UI** (§D5): api.ts + types.ts + `lib/cost.ts` helper, then `Storyboard.tsx`, then App wiring +
   the `data-view="storyboard"` template + Library grouping.
6. **Docs** (§7), then `tauri build` + screenshots.

This is the contract's order (`slice.md:54-57`) with the pure core validator pulled first, because it
de-risks the two genuinely novel pieces (the JSON contract and the clamp) before any shell/UI exists.

---

## 7. Documentation (the DOCUMENT step, after code lands)

- **`CHANGELOG.md`** — under `## [Unreleased]` `### Added` (`:28`): a Storyboard view drafts a
  model-aware shot list from one brief via the local/hosted enhancer, previews and edits each shot's
  prompt, shows the summed estimate, and generates the shots as one linked set. Note (money honesty):
  drafting is free on local Ollama and costs one short completion on a hosted key; no clip is
  submitted without the total shown.
- **`README.md`** — if it documents the enhancer/use-cases or ships a feature list, add the Storyboard
  surface and the drafting-cost note (§D7); else **N/A**.
- **`docs/`** — `N/A` for BRIDGE/PARITY/FIRST-LIGHT (bridge/parity/live-run docs, unchanged); a short
  note may be added where the enhancer is described, recording that a second `Chat` caller now shares
  the backend plumbing.
- **`document.md`** — written after docs land, recording file + section for each change and the
  weakest parts for the verifier.

---

## 8. Non-goals (recorded, not skipped)

- **N1.** No rendering, stitching, last-frame extraction, or crossfade — S14. `compositor.rs`/
  `ffmpeg.rs` untouched.
- **N2.** Continuity is **recorded, not applied** (§D6). The plan carries `role`/`continuity`; nothing
  binds an `end_image_url` or extracts a frame.
- **N3.** No hosted-LLM JSON/structured-output mode — the plan is parsed from text (§D1.1). No new
  request field on any backend.
- **N4.** No new HTTP client — `ChatBackend` reuses the existing wire fns (§D1.2).
- **N5.** No change to the job lifecycle (poll/download/reattach), the enhancer rewrite path, or
  single-job submit behaviour (`submit_one(..,None,None)` is the old body).
- **N6.** No new dependency, router, or state library. The plan validator is plain serde + registry
  lookups.

---

## 9. Open questions (each with the default this build assumes)

- **Q1 — Refactor `HostedEnhancer::call`/`LocalEnhancer::rewrite` to delegate through `ChatBackend`?**
  **Default: no** — a clean dedup, but it churns the S4/S5 enhancer tests for no AC this slice. `Chat`
  is added beside them; the enhancers are untouched. One-way door left open for a later slice.
- **Q2 — Sum-check before or after the model-aware clamp?** **Default: before** (§D1.4 rule 6): the
  ±10% gate judges the LLM's plan intent; the clamp then adjusts individual shots and surfaces
  advisories rather than failing a plan the LLM got right but a model cannot honour exactly. Reversible.
- **Q3 — On a shot failure during generate-all: continue or all-or-nothing?** **Default: continue,
  best-effort** (§D4) — a refusal on shot 3 must not erase shots 1–2 the user paid for; each failure
  is a Failed row in the set. If the owner wants atomicity, a pre-flight "compile+guard every shot,
  then submit" pass is a follow-up.
- **Q4 — How far does continuity go here?** **Default: recorded only** (§D6). Applying it needs S14's
  last-frame + render pipeline; recording the intent is the honest thin seam.
- **Q5 — The grouping table name.** **Default: `storyboards`** (not the contract's `job_sets`, which
  already names the jobs table, §D3). Columns `storyboard_id`/`shot_index` on `job_sets`.
- **Q6 — What is `model_prefs` at draft time?** **Default: the runnable models for the current tab**
  (`modelsForUseCase(tab)`), so the plan can only propose models the composer would accept. The system
  prompt is handed this list; the validator enforces it regardless.

---

## 10. Riskiest parts (where the verifier should start)

1. **The JSON contract is an LLM's free text (§D1.1).** A real model may emit prose around the JSON,
   trailing commas, or extra keys. `clean_reply` + strict serde + the `NotJson`/`Refused` split is the
   mitigation, but the corpus must be firm and the parse forgiving only in the ways `clean_reply`
   already is. **Weakest link** — the verifier should confirm the `garbage`/`refusal` tests are
   non-vacuous and that `NotJson` carries a usable cause. A local run against a real Ollama (not a
   gate) is the honest confidence check.
2. **`submit_one` factoring (§D4).** If the extraction drifts from the current `submit_job` body, every
   single-job submit changes silently. The "single-job submit unchanged" reuse-test is the guard; the
   verifier should diff `submit_one(..,None,None)` against the old `submit_job` line-for-line.
3. **Write-once columns + the `job_sets`/`storyboards` name split (§D3).** Getting `storyboard_id`
   into the `ON CONFLICT UPDATE SET` would let a rerun re-tag a job; omitting it from the INSERT list
   would drop it. The migration-8 round-trip + write-once test pins both.
4. **The duration clamp reads catalogue-sourced axes offline (§D2).** `capability::for_route(.., |_|None)`
   returns the *catalogue's* durations, which can differ from fal's authoritative enum. The clamp is
   therefore a best-effort honesty nudge, not a guarantee; a wrong catalogue enum would clamp to the
   wrong neighbour. Mitigation: it only ever *narrows toward* an offered value and always advises;
   the provider still validates the exact number at submit. Recorded, not hidden.
5. **The total-before-submit gate lives in the UI (§D5).** `generate_storyboard` itself does not
   enforce "total shown first" — it prices per shot but a caller could invoke it without rendering a
   total. The mitigation is that the only caller is the Storyboard view, which disables Generate-all
   until the sum renders; AC6's money-honesty test asserts the per-shot estimate path, and the UI test
   asserts the total gates the button.
