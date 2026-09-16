# S17-keyframe-anchored-mode — DESIGN

Contract: `.forge/slices/S17-keyframe-anchored-mode/slice.md` (AC1–AC7; every `file:line` below
re-verified this session).
Upstream this design is built on and reuses wholesale:
`.forge/objective.md` (§Constraints stack rule, §A no-regression gates, §"no paid job by agents"),
the **Fable memo** `.forge/slices/S15-storyboard-render/h3max-30s-memo.md` (§3/D-D the `end_image_url`
99% path, §0 the two score tiers, §0.4 the bounded auto loop, D-E world block, D-F fill-to-ceiling),
and the shipped slices this depends on:
**S16** (`.forge/slices/S16-duration-range-and-continuity-prompting/{design,implement}.md` — the
per-join score in `chain::score_join`, `JoinRecord`/`joins_json`, `parse_joins`/`upsert_join`,
`prepare_next_shot`, the regenerate + in-flight guard `shot_in_flight`, `AUTO_RETRY_CAP = 2` +
`maybe_auto_retry`, the world block + validator, `CONTINUITY_FLOOR`/`CONTINUITY_IDEAL`/`JoinScore`,
`ffmpeg::first_frame`/`continuity_score`, migration 10 `auto_retry`),
**S15** (chain scheduler settled hook, `chain::advance`, `ffmpeg::last_frame(bin,clip,out,size)`, the
override seam `SubmitInput.constant_overrides`, per-join render, `chainable_model`),
**S14** (H3 Max i2v `image_url`/`end_image_url`, family-root suffix resolver, `NO_END_FRAME`,
`FAL_CONSTANT_FIELDS`, the vendored `schemas/minimax_h3-max_image-to-video.json`),
**S13** (plan DTO, `storyboards` table, `submit_one`, `generate_storyboard_inner`, retry-into-set),
**S10** (per-image pricing, the sandboxed ffmpeg probe). House style followed: S16's `design.md` —
grounded `file:line`, decisions with rationale + a worked example each, per-AC named tests, exact
gate, order of work, open questions with recorded defaults, riskiest parts first. Owner priority
(memory): **continuity toward the 99% ideal.** This slice ships the memo's **ideal tier** — the
keyframe/`end_image_url` mode S16 explicitly deferred here (S16 design §N1).

---

## 0. Session constraint, stated before anything is claimed

Authored by reading the working tree at HEAD `c0035f3` this session; every `file:line` below is
first-hand. Written **without running** `cargo`, `pnpm`, or `tauri build` — a `tauri build` is
holding the working tree and the launching agent forbids touching `ui/`, `src-tauri/`, `crates/`
files while it runs — so every count and every "still green" is a claim the TEST/VERIFY agents must
confirm, not a measured fact. This artifact writes **only** `design.md`; no source is modified.

**No money, no keys, never `first_light.rs`.** The two genuinely new provider actions are (a)
submitting N+1 **still** image jobs and (b) submitting the shots with a second bound frame — both
reuse S13's `submit_one`, which agents never run against a real provider. The end-frame-adherence
score spawns only the bundled **ffmpeg sidecar** on **local PNGs** (no network, no charge), reusing
S16's `continuity_score`. Every bind/plan test uses `FakeChat` fixtures and keyless schema fixtures;
every score test uses synthesised lavfi clips, `#[ignore]`d so the keyless `cargo test --workspace`
gate never spawns ffmpeg. The two threshold constants are **inherited from S16, still reasoned not
measured** — calibration on a real paid set is human-run (Non-goal N2).

### 0.1 Ground truth, as read this session

| Fact | Read at |
|---|---|
| `MediaRole::End` → `end_image_url` on every fal video endpoint bar Wan VACE (`last_frame_url`) and `kling-video/v2.5-turbo` (`tail_image_url`); `MediaRole::Start` → `image_url` | `media.rs:189-228` |
| `can_bind(spec, MediaRole::End, Dialect::Fal, slug)` returns `false` iff `takes_no_media(slug)` **or** `takes_no_end_frame(slug)`; else gated on the measured mode table (permissive for an unmeasured endpoint) | `media.rs:391-414` |
| `NO_END_FRAME: [&str; 8]` = grok-imagine, `wan-25-preview`, five Higgsfield mirrors, `veo3.1`/`.fast`; **H3 Max i2v is NOT in it** (declares `end_image_url`) — S14 confirmed | `media.rs:896-914` |
| The three TextToImage models runnable on a **priced fal** route: `gpt_image_2_5_flare`, `gpt_image_2_5_sunburst` (both `CostModel::Unknown` — `$30/M output tokens`, `quality` moves per-image cost >10×), `qwen_image_3` (`per_image(0.04)`, `$0.04/img@1K, $0.075@2K`) | `registry.rs:1316-1356`, `use_case.rs:306-310` |
| **None** of the three image models declare `aspect_ratio` or `resolution`; canvas is a free-form `image_size` (`ValueSpec::Text`); the edit-tier siblings (`nano_banana_*_edit`, `seedream_v5_pro_edit`) are `EditImage`, not TextToImage | `registry.rs:2282-2349` |
| `SubmitInput { model_id, route_id, prompt, preset_id, settings, media: Vec<MediaRef>, final_prompt, constant_overrides }`; `media` carries roles, `constant_overrides` is the S15 override seam | `commands.rs:725-758` |
| `MediaRef::url(role, s)` / `MediaRef::local(role, s)` (local uploaded at submit); a still already downloaded to the Library is a local PNG | `media.rs` (`MediaRef`), S13/S15 |
| `chain::advance`: on a settled chained shot `cur_idx`, extracts `last_frame(clip, size)` → `frames/shot{cur_idx}.png`, then `prepare_next_shot` attaches it as `MediaRole::Start` + `chained_overrides(seed)` and `submit_one`s shot `cur_idx+1`; idempotency guard on an existing row at `next` | `chain.rs:88-249` |
| `prepare_next_shot(model, input, frame, seed, frame_error) -> (input, applied, advisory)` — the sole place a chained shot's frame media is attached | `chain.rs:88-118` |
| `chain::score_join`: scores the join *into* the settled shot `i≥1` (SSIM `frames/shot{i-1}.png` vs a scratch `first{i}.png` from `first_frame(clip_i)`), writes `JoinRecord` into `joins_json`; unscored → `None` never `0.0`; scratch frame cleaned up | `chain.rs:263-361` |
| `chainable_model(model) = use_case::supports(model, ImageToVideo)` (the per-route `can_bind` tightening was **reverted** in S16 as too loose — H2) | `chain.rs:48-51` |
| `JoinRecord { index, kind, applied, ssim, psnr, dy, du, dv, pass, attempt }`; `parse_joins`/`upsert_join` (latest write wins, ordered) | `store.rs:22-55` |
| `Storyboard { id, brief, target_seconds, plan_json, created_at, chained, shots_json, overlap_seconds, joins_json, auto_retry }`; migrations are **append-only**, `user_version` is currently **10** (S16 `auto_retry`); the INSERT + `ON CONFLICT DO UPDATE` + `row_to_storyboard` + `in_memory()` round-trip pattern | `store.rs:65-91`, `:290-378`, `:456-522`, migration blocks `:160-328` |
| `generate_storyboard_inner`: guards (brief/plan/shot caps), re-validates the plan via `storyboard::validate`, pins each shot to its plan slot (model, duration, route), mints `sid`, stores `chained`+`shots_json`+`overlap_seconds`+`auto_retry`, then in chained mode **submits only shot 0** and returns the rest `pending`; retry-into-set relaxes the guard for a below-floor Completed shot (`shot_in_flight` blocks in-flight) | `commands.rs:1400-1695` |
| `submit_one(store, prices, runner, model, input, sid, shot_idx)` is the whole money path: `compiled_for_submit` → `clamp_duration_to_range` → `Billable::from` → `route::resolve` → `estimate_with_provenance` → upsert Queued → submit → `runner.watch` | `commands.rs:1091-1140` |
| `AUTO_RETRY_CAP = 2`; `should_auto_retry(rec, enabled) = enabled && rec.pass == Some(false) && rec.attempt < CAP`; `maybe_auto_retry` refuses when `shot_in_flight` and bumps `attempt` in `joins_json` | `chain.rs:365-408` |
| `storyboard_seed(id)` FNV-1a u32; `chained_overrides(seed) = {prompt_expansion_mode:"disabled", seed}` | `chain.rs:56-78` |
| `StoryboardPlan { world, brief, target_seconds, use_case, shots }`; `Shot { title, prompt, action, seconds, camera, model_id, role, continuity, advisories, chainable, resolution }` (all continuity fields serde-default); `plan()` composes `prompt="{world} {action}"` and validates | `storyboard.rs:64-140`, `:451-520` |
| `PlanError` variants (S16): `…, MissingWorld, PromptMissingWorld, IdenticalShots, NoHoldEnding, CutWords, ShotOverCeiling`; all pure, `Display`ed at the command boundary | `storyboard.rs:146-192` |
| H3 Max i2v schema: `image_url` + `end_image_url` both `anyOf[{string},{null}]` (either alone allowed, "end-only keyframe generation"), canvas follows the frame, `seed` honoured, `prompt_expansion_mode` free string | `.forge/slices/S14-minimax-h3-max-constant-fields/schemas/minimax_h3-max_image-to-video.json`; S14 §0.1 |
| The S16 gate baseline this slice must hold: `cargo test --workspace` **924**, ffmpeg `--ignored` **8** (7 core + `score_join_cleans_up_the_scratch_first_frame`), `pnpm test` **302** | S16 `implement.md` repair-round gate |

**Corrections to the contract's Ground truth (re-verified this session):**
- The contract's D1 says the still model is "the cheapest runnable text-to-image on fal with the
  set's aspect". Re-verified: the only image model with a **known** per-image price is `qwen_image_3`
  (`$0.04/img@1K`); the two GPT tiers are `CostModel::Unknown` (token-billed, `quality`-dependent).
  So the honest "cheapest **priced**" default is **`qwen_image_3`** (D1). "with the set's aspect" is
  **not** an enum on any image model — canvas is a free-form `image_size` string, so aspect/resolution
  are composed into `image_size`, not selected from a chip (D1, Q1).
- The contract references "migration 11: `keyframes_json`". Confirmed: `user_version` is **10** today
  (S16), so the keyframe store is migration **11** (D2), the next append-only step.
- The contract references "the S16 regenerate/in-flight guard". Confirmed at `commands.rs:1547-1585`
  (`below_floor` + `shot_in_flight`); D4 extends the same guard to stills with a **separate** bound.

---

## 1. Decisions

### D1 — The keyframe plan shape: stills derived, not model-drafted (AC1)

**A keyframe is a still the app derives from the already-validated plan, not a new thing the model
drafts.** Rationale: a shot's `action` already ends on "…static hold of X" (S16 `NoHoldEnding`
enforces it) and every shot opens on "the same static hold" — the exact describable frames a still
must render. Deriving the still prompt from the world block + those hold clauses (a) guarantees the
still and the video share the identical world string verbatim (the whole point — a second free-text
draft is drift by construction, memo §1), and (b) needs **no** second LLM round trip and **no** v3
JSON contract the model could break. The **prompt v3 touch** is minimal: `prompts/storyboard.v2.md`
→ `v3` adds one paragraph telling the model to make each hold clause a *self-contained describable
still* ("name what is centred, its pose, the light — a photographer could shoot this one frame"), so
the derived still prompt reads well. No new JSON key the model must emit.

**Plan DTO (additive, serde-default so a stored S13/S16 plan still parses):** `StoryboardPlan` gains

```rust
/// The N+1 still boundaries of a keyframe-anchored set (memo §3). Empty for a
/// non-keyframe plan (serde-default), so every stored plan still parses. Derived
/// by [`derive_keyframes`] from the world block + the shots' hold clauses — the
/// model does not draft this; it is composed after validation.
#[serde(default)]
pub keyframes: Vec<Keyframe>,
```

```rust
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Keyframe {
    /// Boundary index k in 0..=N. K0 is shot 0's opening (its `image_url`);
    /// K_{i+1} is shot i's planned ending (its `end_image_url`).
    pub index: usize,
    /// `"{world} {hold clause}"` verbatim — the same world string the shots carry.
    pub prompt: String,
    /// The registry image model that will render this still. Defaults to the
    /// set-wide still model (D1 default `qwen_image_3`); one model for the set.
    pub model_id: String,
    /// The canvas the still (and therefore the video, "canvas follows the frame")
    /// renders at: derived from the set's resolution × aspect. Written into the
    /// image job's free-form `image_size` (there is no aspect/resolution enum on
    /// the image models). e.g. 768P 16:9 → `{"width":1360,"height":768}`.
    pub image_size: ImageSize,
}
```

`ImageSize` is `{ width: u32, height: u32 }` (serde camelCase). `derive_keyframes(plan, still_model,
image_size) -> Vec<Keyframe>` is a pure fn: `K0.prompt = "{world} {shot0 opening hold}"`; for
`k in 1..=N`, `K_k.prompt = "{world} {shot_{k-1} ending hold clause}"`, where the hold clause is the
"Ends on a static hold of …" tail extracted from `shot.action` (reusing S16's `ends_on_hold`
whole-word matcher, `storyboard.rs:595`, to find the clause boundary). `N = plan.shots.len()`, so
`keyframes.len() == N + 1`.

**Still model selection (D1 default + owner override).** Default = **`qwen_image_3`** — the cheapest
*priced* fal TextToImage model (`per_image(0.04)`); the GPT tiers are `CostModel::Unknown` (S10
prices them as `None` → the total shows "unpriced", which the money gate honestly surfaces but the
owner may not want). The owner override is a `still_model_id` on the generate input (D2/D5); the
validator gates it to a runnable+priced-route TextToImage model.

**Canvas derivation.** The video set has one resolution (S16 D-F, `resolution_height` maps 480/768/
1080) and one aspect (16:9 / 9:16 from the shot's route). `image_size.width/height = round-to-even(
height × aspect_ratio, height)`. 768P 16:9 → `1360×768` (even). Because "the output canvas follows
this image" on H3 Max i2v, the still's dimensions become the video canvas, so they MUST equal the
set's target — this is the load-bearing consistency the validator checks.

**Validator variants (new `PlanError`, all pure; run in `plan()` only when `keyframes` non-empty):**
- `KeyframeCount { expected, got }` — `keyframes.len() != shots.len() + 1`.
- `KeyframeModelNotRunnable { index, model_id }` — the still model is not a registry model runnable
  for `UseCase::TextToImage` (`use_case::supports`) **and** with a priced fal route
  (`route::resolve(Cheapest).is_ok()` — same predicate the picker uses).
- `KeyframeCanvasMismatch { index }` — a keyframe's `image_size` aspect ratio does not match the
  set's aspect, or its height does not match the set's resolution height (a mixed canvas re-opens the
  continuity hazard S16 D-F warned of).
- The world/hold/cut-word rules S16 already enforces are reused unchanged (a keyframe plan is a
  continuous run, so it always has a world).

**Worked example (AC1 test, FakeChat fixture):** a 2×15 H3 Max 768P 16:9 plan with a shared world
and shots ending "…static hold of the can centred, label to camera" / "…the can filling the frame,
label sharp". `derive_keyframes(…, qwen_image_3, {1360,768})` → 3 keyframes: `K0 = "{world} the can
centred, label to camera, one drop about to fall"`, `K1 = "{world} the can centred, label to camera"`
(shot 0's ending), `K2 = "{world} the can filling the frame, label sharp"` (shot 1's ending). Each
`model_id == "qwen_image_3"`, `image_size == {1360,768}`. `keyframes.len() == 3 == shots.len()+1`.
A plan whose still model is `kling3_0_pro` (no TextToImage route) → `KeyframeModelNotRunnable`.

### D2 — Generation order, the dependency graph, and the money gate (AC2, AC6, migration 11)

**The dependency graph (memo §3):** stills K0..KN first (all N+1, parallel within the runner's
provider cap) → shot 0 (`image_url = K0`, `end_image_url = K1`) once **all** stills are ready → shot
i (i≥1) once clip i−1 settles (`image_url` = actual last frame of clip i−1, S15; `end_image_url =
K_{i+1}`). This is two barriers on top of S15's per-shot chain: a **stills-all-ready → shot 0**
barrier (new), then the existing settle-driven `advance` chain (extended by D3 to attach the end
frame).

**Migration 11 — `keyframes_json` on `storyboards`, write-once-ish (append-only).**

```sql
-- migration 11 (store.rs, after the version < 10 block; bump PRAGMA user_version = 11)
ALTER TABLE storyboards ADD COLUMN keyframes_json TEXT;   -- NULL for a non-keyframe set
```

`Storyboard` gains `#[serde(default)] keyframes_json: Option<String>` (read `unwrap_or(None)`, added
to the INSERT column list + `ON CONFLICT DO UPDATE SET` — it DOES update, because a still regenerate
rewrites its record). The v1-drop list gains the column. `keyframes_json` holds a serialised
`Vec<KeyframeRecord>`:

```rust
pub struct KeyframeRecord {
    pub index: usize,             // 0..=N
    pub job_set_id: Option<String>, // the still image job's id
    pub local_path: Option<String>, // the downloaded still PNG, once settled
    pub estimated_usd: Option<f64>, // priced at submit (may be None → unpriced)
    pub attempt: u32,             // still-regenerate bound (separate from a shot's)
}
```

**Why store this and not re-derive:** the shot payloads must bind a still by a concrete path/URL,
which does not exist until the still job settles and downloads. `keyframes_json` is the crash-safe
record of "which still job is which boundary, and where its PNG landed", exactly mirroring how
`shots_json` records the shots (S15). Deriving it from `jobs_in_storyboard` alone is impossible — a
still job carries no boundary index in the jobs table (D2's shot-index decision below).

**Still jobs are tagged to the set but occupy no shot index.** A still is submitted via `submit_one`
with `storyboard_id = Some(sid)` and **`shot_index = None`** so it never collides with the shots'
`0..N` index space that `advance`/`score_join`/the idempotency guard key off (`advance` early-returns
on `shot_index == None`, `chain.rs:150`). The mapping boundary↔job lives in `keyframes_json`. A still
job's `UseCase` is `TextToImage`, so it is a normal image generation the Library already renders.

**`generate_storyboard_inner` keyframe branch (new, alongside the S15 chained branch).** When
`input.keyframe_mode` and it is a first generate (no `storyboard_id`, not a retry subset, `shots.len()
> 1`):
1. Store the group with `chained = 1`, `shots_json`, `overlap_seconds = 0.0` (hard cuts), and
   `keyframes_json` = one `KeyframeRecord { index, job_set_id: None, local_path: None, estimated_usd,
   attempt: 0 }` per boundary — the estimate is computed here per still (via the still model's route,
   S10) so the **total shown before Generate-all** already includes the stills (the UI sums per-shot
   + per-still estimates, D5).
2. Submit **all N+1 still jobs** through `submit_one` (each priced, gated, guarded — no still bypasses
   the estimate), write each returned `job_set_id` back into its `KeyframeRecord`.
3. Submit **no shots yet**; return every shot `pending`. The stills-ready barrier (below) submits
   shot 0 when the last still settles.

**The stills-ready barrier — `chain::keyframes_advance(state, settled)`.** Wired into the settled
hook beside `advance`/`score_join`. When a settled job is a **still** (its `job_set_id` is in the
set's `keyframes_json`, `shot_index == None`): update that `KeyframeRecord.local_path` from the
downloaded output; if **every** `KeyframeRecord` now has a `local_path` **and** no shot 0 row exists
yet, build shot 0's `SubmitInput` from `shots_json[0]` with `image_url = K0` and `end_image_url = K1`
(D3) and `submit_one` it. Idempotent (guarded on "shot 0 row already exists"), so the hook and the
startup resume cannot both fire it.

**A failed still parks the set with a clear error (AC2).** If a still job settles **Failed** (or its
retry bound is exhausted, D4), `keyframes_advance` does **not** submit shot 0; it records a set-level
advisory on the storyboard (a new `Storyboard` field is unnecessary — reuse a pending shot 0's
`ShotOutcome.error` surfaced via the Library, and a KeyframeRecord left with `local_path: None`). The
UI shows "keyframe K{index} failed — the set is parked; regenerate the still to continue" (D5). No
shot is ever submitted against a missing anchor.

**Money gate (AC6):** total = Σ still estimates (N+1) + Σ shot estimates (N), rendered by the UI
before Generate-all is enabled (D5). Every still and every shot is priced by `submit_one`'s existing
`estimate_with_provenance` — nothing invents a total, nothing skips an estimate. A GPT still model
prices `None` → the total is marked "unpriced" (the S10/S16 rule: never summed as $0).

**Worked example (AC2 test):** a 2×15 keyframe set, still model `qwen_image_3`. `generate_storyboard_
inner` stores `keyframes_json` with 3 records (each `estimated_usd == 0.04`), submits 3 still jobs,
returns shots 0/1 `pending`. As each still settles the barrier writes its `local_path`; when the third
lands, shot 0 is submitted with `image_url = K0.png`, `end_image_url = K1.png`. If K1's job fails, the
barrier records the park and shot 0 is never submitted (asserted: no row at shot_index 0).

### D3 — Both-frame binding through the media attach + override seam (AC3)

**Shot 0** is submitted by the barrier (D2) with two media roles pushed onto its `SubmitInput.media`:
`MediaRef::local(MediaRole::Start, K0.png)` and `MediaRef::local(MediaRole::End, K1.png)`, plus
`constant_overrides = chained_overrides(seed)`. `bind` already maps `Start → image_url`, `End →
end_image_url` (media.rs:189-228), and H3 Max i2v is not in `NO_END_FRAME`, so both keys land.

**Shot i (i≥1)** flows through the existing `chain::advance` → `prepare_next_shot`. Extend
`prepare_next_shot` to also attach the end frame in keyframe mode: after it pushes the `Start` frame
(the actual last frame of clip i−1, unchanged from S15), if the set is a keyframe set it also pushes
`MediaRef::local(MediaRole::End, K_{i+1}.png)` — read from `keyframes_json[i+1].local_path`. The
`advance` caller passes the keyframe path (looked up from the set's `keyframes_json`) into
`prepare_next_shot`; a non-keyframe set passes `None` and behaves exactly as S15/S16.

**`NO_END_FRAME` fallback with a join advisory (AC3).** Before attaching the End frame,
`prepare_next_shot` checks `media::can_bind(&model.spec, MediaRole::End, media::dialect_for(provider,
slug), slug)`. If it is `false` (a model in `NO_END_FRAME`, or one that takes no media), it attaches
**start-only** and pushes an advisory onto the shot: `"end-frame anchoring unavailable: {model} has
no end-frame route — this join is start-anchored only"`. The set still runs; the join is scored (D4)
but its end-adherence is recorded absent, not zero. This is the exact `NO_END_FRAME`-class harm S9/S14
guarded — never silently drop a bound frame into a route that ignores it.

**Keyless bind test design (AC3).** A pure test builds shot i's `SubmitInput` through
`prepare_next_shot` with both a Start path and a K_{i+1} path against the **vendored** H3 Max i2v
schema (`schemas/minimax_h3-max_image-to-video.json`, S14) and asserts the compiled body carries
**both** `"image_url"` and `"end_image_url"` (non-null strings) — reusing S14's keyless body-assertion
pattern (`media.rs` bind tests, `:1291-1472`). A second case runs a `NO_END_FRAME` slug (e.g.
`veo3.1`) and asserts the body carries `image_url` only, and the shot carries the advisory. No network,
no key, no submit.

**Worked example:** shot 1 on H3 Max, `image_url = frames/shot0.png` (S15), `end_image_url = K2.png`
→ both keys in the body, `constant_overrides` disable expansion + carry the seed. The same shot on a
hypothetical `NO_END_FRAME` model → `image_url` only + the advisory; `score_join` records the join
score but marks end-adherence unscored.

### D4 — Tier-2 scoring: end-frame adherence + separate regenerate/auto-retry bounds (AC4)

**Two scores per join, both in `joins_json` (schema-versioned, additive).** S16 records the *join*
score (SSIM of clip i−1's last frame vs clip i's first frame — the cut). S17 adds **end-frame
adherence**: SSIM between the **planned still K_{i+1}** and clip i's **actual last frame**. Extend
`JoinRecord` with additive `Option` fields (old rows deserialise with `None`, so it is purely
additive — no migration, `joins_json` already round-trips):

```rust
// added to JoinRecord (all Option → absent on a pre-S17 row / an unmeasured end)
pub end_ssim: Option<f64>,
pub end_dy: Option<f64>,
pub end_du: Option<f64>,
pub end_dv: Option<f64>,
pub end_pass: Option<bool>,   // scored against CONTINUITY_IDEAL (0.90/6/3)
```

The join *into* shot i already lives at `JoinRecord.index == i`. Shot i's end-adherence (its actual
last frame vs K_{i+1}) is recorded on **that same record** as the end-* fields. Shot 0 has an end
(vs K1) but no join into it; in keyframe mode a `JoinRecord { index: 0, kind:"continuity", … }` is
allowed carrying **only** the end-* fields (the cut fields stay `None`) — a small, additive
relaxation of S16's "no join 0" (`store.rs:19`), documented at the field.

**Gated against the ideal, not the floor (AC4).** Keyframe-mode joins are the 99% path, so the *cut*
score is reported against `CONTINUITY_IDEAL` (0.90/6/3) on the badge (S16 already reports it), and
end-adherence `end_pass` is computed against `CONTINUITY_IDEAL`. A below-ideal join (cut OR
end-adherence) offers the S16 regenerate path and, when enabled, the bounded auto-retry. **Unknown →
`None`, never 0** — the S16 invariant, extended to the end fields (`join_passes`-style: unscored end
is not failing).

**Where it is computed.** `chain::score_join` (chain.rs:263) is extended: it already extracts clip
i's *first* frame for the cut score; it now also needs clip i's *last* frame to compare against
K_{i+1}. For i≥1 the barrier/advance already wrote `frames/shot{i}.png` (clip i's last frame) when it
submitted shot i+1 — but the **last shot** (i = N−1) has no successor, so its last frame is never
extracted by `advance`. Decision: `score_join` extracts clip i's last frame itself
(`ffmpeg::last_frame(clip_i, scratch, size)`) when the set is a keyframe set and K_{i+1} exists,
scores it against `keyframes_json[i+1].local_path` via the existing `continuity_score`, and cleans up
the scratch (reusing the S16 `m5` scratch-cleanup discipline, chain.rs:319). A missing still or a
scoring failure records the end-* fields `None`.

**Separate bounds for stills and shots (AC4/AC6).** S16's `AUTO_RETRY_CAP = 2` bounds a **shot** join's
auto-retries via `JoinRecord.attempt`. Stills get their **own** bound `KEYFRAME_RETRY_CAP = 2` via
`KeyframeRecord.attempt`. Auto-retry logic:
- A **shot** join below ideal (cut or end) → the S16 `maybe_auto_retry` path (`seed + attempt`, same
  frames, priced first, `shot_in_flight` guard), capped at 2. The end-adherence failure is a *shot*
  regenerate (re-run the i2v with the same both-frame anchors and a new seed).
- A **still** that fails or renders a poor anchor → a **still** regenerate through `submit_one`
  (`storyboard_id = Some(sid)`, `shot_index = None`, `seed + KeyframeRecord.attempt`), capped at 2,
  guarded by an in-flight check on the still job. Regenerating a still re-parks the barrier until the
  new still settles, then shot 0 (or the dependent shot) proceeds.

**Nothing spends without the total shown** — every still and shot regenerate goes through
`submit_one`, which writes `estimated_usd` before contacting the provider (S16 M1 invariant).

**Worked examples (AC4 tests):**
- Pure: `join_passes_end(rec, CONTINUITY_IDEAL)` — `end_ssim 0.95, end_dy 4` → pass; `end_ssim 0.85`
  → fail; all `None` → not failing. A `JoinRecord{index:0}` with only end-* set scores shot 0's end.
- `--ignored` real sidecar (synth, no money): K = `testsrc` last frame, clip's actual last frame =
  the same PNG re-encoded → `end_ssim ≥ 0.9` (adheres); a different `testsrc` frame → `end_ssim <
  0.8` (drift). Reuses S16's `continuity_score` machinery unchanged.
- `auto_retry_bounds_are_separate` (fake scorer + fake store): a below-ideal still retries ≤2 via
  the still bound; a below-ideal shot join retries ≤2 via the shot bound; neither consumes the
  other's budget; both off by default → 0 regenerates.

### D5 — UI: Keyframe toggle, still previews, priced total, two badges (AC5)

Additive on S16's `Storyboard.tsx`/`Library.tsx`; test-locked class names unchanged.

- **Keyframe toggle** (`.storyboard-keyframe-toggle`, default **off**). Enabled only when **every**
  shot's model has an end-frame-capable i2v route (`!NO_END_FRAME` and `can_bind(End)` — surfaced as
  a per-shot `endFrameCapable: boolean` computed server-side in `plan_storyboard`, mirroring how
  `chainable` is computed, so the toggle default cannot drift from the scheduler) **and** an image
  model is available (a runnable priced TextToImage model exists — always true today: `qwen_image_3`).
  Off/disabled with a one-line note when any shot's model cannot take an end frame.
- **Still previews per boundary** (`.storyboard-keyframe-card`, one per K0..KN): shows the derived
  still prompt, the still model, and — once the still job settles — the downloaded PNG thumbnail, with
  a **Regenerate still** button (`.storyboard-regenerate-still`) that calls the still-regenerate path
  (D4), showing the still's estimate first.
- **Priced total including stills** (D2/D6): the total the Generate-all button gates on is
  `Σ still estimates + Σ shot estimates`, marked "unpriced" if any is `None` (reusing S13's
  `totalCostCaption`). Generate-all stays disabled until the total renders.
- **Library set item shows two scores per join** (`.storyboard-join` extended): the cut score
  ("join 0.93 — in-camera") **and** the end-adherence score ("end 0.91 — anchored" / "end 0.7 —
  drift" / "end —" unscored), each `data-pass` against the ideal.
- **api.ts / types.ts:** `keyframeMode` + `stillModelId` on the generate input; `Keyframe`,
  `KeyframeRecord`, the extended `JoinRecord` (end-* fields), `endFrameCapable` on the shot type; the
  still-regenerate call reuses `generateStoryboard`. `!isDesktop()` mocks.
- **vitest list:** `the_keyframe_toggle_enables_only_when_every_shot_takes_an_end_frame_and_an_image_
  model_exists`; `the_total_includes_the_stills`; `a_failed_still_shows_the_parked_state_and_offers_
  regenerate`; `a_settled_still_shows_its_thumbnail_and_regenerate`; `the_library_item_shows_both_the_
  cut_and_the_end_adherence_score`; `an_unscored_end_shows_a_dash_not_zero`.

### D6 — Costs: what the owner sees (AC2/AC6)

- **Per-image pricing (S10):** `qwen_image_3` = `$0.04/image at 1K` (`per_image(0.04)`,
  `registry.rs:1355`); at 2K `$0.075`. A 2×15 set = 3 stills → `3 × $0.04 = $0.12` in stills.
- **Promotional H3 Max rates (S14):** 768P `$0.04/s`, 480P `$0.025/s`, 1080P `$0.08/s`. A 2×15 768P
  set = `30s × $0.04 = $1.20` in shots. **Total shown = $1.32** (stills + shots) before any submit.
- **GPT tiers unpriced:** if the owner overrides to `gpt_image_2_5_flare`/`sunburst`
  (`CostModel::Unknown`), each still prices `None` and the total is marked "unpriced" — honest, the
  owner sees they cannot know the still cost up front (token-billed, `quality`-dependent). Default
  stays `qwen_image_3` so the common path has a real number.

### D7 — Resume / relaunch semantics with stills (S1)

A keyframe set survives a window close at any point, reusing S15's `chain::resume_all` (called once
at startup after `runner.resume_all`), extended for the two keyframe barriers:
- **Stills mid-flight:** resume via the ordinary runner poll loop; each settle fires
  `keyframes_advance` normally.
- **Stills all settled but shot 0 not submitted** (a crash in the barrier window): `resume_all` scans
  each keyframe set, and if every `KeyframeRecord.local_path` is present and no shot 0 row exists,
  submits shot 0. Idempotent (the barrier's "shot 0 exists" guard).
- **A still Failed:** the set stays parked (D2); resume does not resubmit it automatically (a
  regenerate is user/auto-retry driven, D4). This matches S15's "a failed predecessor parks the
  chain".
- The mid-chain (shot i≥1) resume is unchanged from S15/S16; D3's end-frame attach reads
  `keyframes_json` which survives the relaunch.

**Worked example (test):** a set whose 3 stills all downloaded but shot 0 is absent →
`resume_all` submits shot 0 with both anchors; a set with one still still Queued → `resume_all`
leaves it (the runner poll will settle it and fire the barrier); a set with a Failed still →
`resume_all` leaves it parked (no submit).

---

## 2. Files touched (complete; nothing else changes)

**Core (`crates/hickeyfield-core/src/`):**
- `storyboard.rs` — `Keyframe`/`ImageSize` types; `StoryboardPlan.keyframes` (serde-default);
  `derive_keyframes`; `PlanError::{KeyframeCount, KeyframeModelNotRunnable, KeyframeCanvasMismatch}`
  + `Display` arms; the keyframe validator run in `plan()`; `join_passes_end` (ideal-tier end
  adherence); tests + FakeChat fixtures (D1/D4).
- `media.rs` — **no change** (`MediaRole::End`/`end_image_url`/`NO_END_FRAME`/`can_bind` reused
  verbatim); recorded N/A below.
- `corpus.rs` + `prompts/storyboard.v3.md` — `STORYBOARD_SYSTEM` → v3 (the one still-friendly-hold
  paragraph; keep v2 on disk for provenance) (D1).
- `ffmpeg.rs` — **no change** (`last_frame`/`first_frame`/`continuity_score` reused); N/A below.

**Shell (`src-tauri/src/`):**
- `store.rs` — migration **11** (`keyframes_json`); `Storyboard.keyframes_json`; `KeyframeRecord`
  struct + `parse_keyframes`/`upsert_keyframe` helpers; `JoinRecord` end-* fields (additive);
  INSERT/`ON CONFLICT`/row-map + v1-drop updates; round-trip + `user_version == 11` test.
- `commands.rs` — `StoryboardGenerateInput.{keyframe_mode, still_model_id}`; the keyframe branch of
  `generate_storyboard_inner` (store `keyframes_json`, submit the N+1 stills, return shots pending);
  the still-regenerate guard (separate bound); `endFrameCapable` on the shot DTO in `plan_storyboard`;
  the per-still estimate exposure; tests.
- `chain.rs` — `keyframes_advance` (the stills-ready → shot 0 barrier + failed-still park); extend
  `prepare_next_shot` + `advance` to attach `end_image_url` from `keyframes_json` with the
  `can_bind(End)` fallback (D3); extend `score_join` with end-adherence (extract clip i's last frame,
  score vs K_{i+1}) (D4); extend `maybe_auto_retry`/`resume_all` for stills + shot 0 barrier (D4/D7);
  `KEYFRAME_RETRY_CAP`; tests + the keyless bind test.
- `app.rs` — wire `chain::keyframes_advance` into the settled hook beside `advance`/`score_join`.
- `lib.rs` — no new command (reuses `generate_storyboard`); confirm registration unchanged.

**UI (`ui/src/`):**
- `types.ts` — `Keyframe`, `KeyframeRecord`, extended `JoinRecord`, `keyframeMode`/`stillModelId` on
  the generate input, `endFrameCapable` on the shot type.
- `components/Storyboard.tsx` — the Keyframe toggle, still-preview cards + Regenerate still, the
  stills-inclusive total (D5).
- `components/Library.tsx` — the two-score-per-join badge (cut + end adherence) (D5).
- `api.ts` — the still-regenerate call (reuses `generateStoryboard`); the keyframe generate payload;
  mocks.
- `base.css`/`rail.css` — `.storyboard-keyframe-*`/`.storyboard-regenerate-still` styles (additive).
- Tests: `Storyboard.test.tsx`, `Library.test.tsx`, `api.test.ts` (the D5 vitest list).

**Docs:** `CHANGELOG.md`, `README.md`, `.forge/slices/S17-keyframe-anchored-mode/document.md`,
`.forge/backlog.md`.

**Not touched (recorded, not skipped):**
- `media.rs` binder / `fal_keys` / `NO_END_FRAME` / `can_bind` — **N/A, reused verbatim**: H3 Max i2v
  already binds `Start→image_url` and `End→end_image_url`, and `NO_END_FRAME` is the fallback gate.
  This slice adds a *caller* that pushes an `End` role; the binding mechanism is S9/S14's.
- `ffmpeg.rs` — **N/A, reused**: `last_frame`/`first_frame`/`continuity_score` cover both the cut and
  the end-adherence score; no new argv.
- `submit_one` body / the poll-download lifecycle / `reconcile` / `apply_constants` — **N/A, reused**:
  stills and both-frame shots are ordinary submits; the money path is byte-for-byte.
- `tauri.conf.json` / `capabilities/default.json` — **N/A**: no new sidecar, no new permission (the
  score spawns ffmpeg from Rust; stills download through the existing Library path).
- `first_light.rs` — **never** (spends money).

---

## 3. Data flow

**Keyframe generate (AC1–AC3, AC6):** Storyboard.tsx (Keyframe on) → `plan` derives `keyframes[N+1]`
(world + holds, still model, canvas) + validates → UI shows still cards + the stills-inclusive total
→ `generateStoryboard({keyframeMode:true, stillModelId, …})` → `generate_storyboard_inner` stores
`keyframes_json` + `shots_json`, submits **N+1 still jobs** (priced, gated), returns shots pending →
runner settles each still → **`keyframes_advance`** writes each `local_path`; on the last still,
submits shot 0 (`image_url = K0`, `end_image_url = K1`) → runner settles shot 0 → **`advance`**
extracts clip 0's last frame, `prepare_next_shot` attaches it (Start) + K2 (End), submits shot 1 → …
→ shot N−1 settles, set complete.

**Scoring (AC4):** on each shot i≥1 settle → **`score_join`** computes the cut score (S16) **and** the
end-adherence (clip i's actual last frame vs K_{i+1}), writes both into `joins_json[i]` (end-* fields);
shot 0's end-adherence is recorded at `joins_json[0]`. A below-ideal cut or end offers Regenerate
(shot, `seed+attempt`, both anchors) or bounded auto-retry (separate still/shot bounds). Render reads
`joins_json` per join (S16 D8, unchanged — continuity joins are hard cuts).

**Failure/park (AC2):** a Failed still → `keyframes_advance` parks (no shot 0); the UI shows the
parked K + Regenerate still. **Resume (D7):** startup `resume_all` re-drives whichever barrier the
crash interrupted.

---

## 4. Test plan — every AC to a named test

Rust tests in `#[cfg(test)] mod tests` beside the code; `--ignored` real-sidecar tests synthesise
inputs (`testsrc`/`eq`). UI tests use the repo's `react-dom/client` + jsdom pattern.

| AC | test | file | asserts |
|---|---|---|---|
| AC1 | `derive_keyframes_builds_n_plus_one_stills_from_the_holds` | `storyboard.rs` | 2 shots → 3 keyframes; K0 = world+opening, K1/K2 = world+each ending hold; each `image_size` = the set canvas |
| AC1 | `a_keyframe_count_mismatch_is_rejected` | `storyboard.rs` | `keyframes.len() != shots+1` → `KeyframeCount` |
| AC1 | `a_non_text_to_image_still_model_is_rejected` | `storyboard.rs` | still model `kling3_0_pro` → `KeyframeModelNotRunnable`; `qwen_image_3` accepted |
| AC1 | `a_still_canvas_that_disagrees_with_the_set_is_rejected` | `storyboard.rs` | a keyframe `image_size` off the set aspect/height → `KeyframeCanvasMismatch` |
| AC2 | `migration_11_adds_keyframes_json_and_roundtrips` | `store.rs` | `user_version == 11`; `keyframes_json` + `KeyframeRecord` round-trip; a non-keyframe set loads `None` |
| AC2 | `keyframe_generate_submits_the_stills_first_and_holds_the_shots` | `commands.rs` | N+1 still jobs submitted (shot_index None), shots returned pending, `keyframes_json` estimates present |
| AC2 | `all_stills_ready_submits_shot_zero_with_both_anchors` | `chain.rs` | when the last still's `local_path` lands, shot 0 submits with Start=K0, End=K1; idempotent on a second call |
| AC2 | `a_failed_still_parks_the_set` | `chain.rs` | a Failed still → no shot 0 row; the KeyframeRecord stays `local_path:None`; a park advisory is recorded |
| AC3 | `both_frames_land_in_the_body_on_h3_max_i2v` (keyless) | `chain.rs`/`media.rs` | `prepare_next_shot` with Start+End against the vendored schema → body has non-null `image_url` **and** `end_image_url` |
| AC3 | `a_no_end_frame_model_falls_back_to_start_only_with_an_advisory` | `chain.rs` | `veo3.1` → body has `image_url` only + the end-anchor advisory on the shot |
| AC4 | `join_passes_end_at_the_ideal_boundaries` | `storyboard.rs` | end `(0.90,6,3)` in; one-over each out; all-`None` → not failing |
| AC4 | `score_join_records_end_adherence_against_the_next_still` | `chain.rs` | i≥1 keyframe set → `joins_json[i].end_ssim` set; shot 0's end at `joins_json[0]`; missing still → end-* `None` |
| AC4 | `end_adherence_reads_the_actual_last_frame` (`--ignored`) | `ffmpeg.rs`/`chain.rs` | K==clip last frame → `end_ssim ≥ 0.9`; a different frame → `< 0.8` |
| AC4 | `still_and_shot_auto_retry_bounds_are_separate` | `chain.rs` | still ≤2 via `KeyframeRecord.attempt`; shot ≤2 via `JoinRecord.attempt`; neither borrows the other; off → 0 |
| AC4 | `a_scoring_failure_leaves_end_none_never_zero` | `chain.rs` | sidecar missing → end-* `None`, no spurious regenerate |
| AC5 | `the_keyframe_toggle_enables_only_when_every_shot_takes_an_end_frame` | `Storyboard.test.tsx` | all end-capable + image model → enabled; one not → disabled + note |
| AC5 | `the_total_includes_the_stills` | `Storyboard.test.tsx` | total = Σ still + Σ shot; a GPT still (unpriced) → "unpriced" |
| AC5 | `a_settled_still_shows_a_thumbnail_and_regenerate` / `a_failed_still_shows_the_parked_state` | `Storyboard.test.tsx` | still card states |
| AC5 | `the_library_item_shows_both_scores_per_join` / `an_unscored_end_shows_a_dash_not_zero` | `Library.test.tsx` | cut + end badges, unscored → "—" |
| AC6 | `no_still_or_shot_submits_without_a_priced_row` | `commands.rs` | every still/shot regenerate writes `estimated_usd` before submit; the still-regenerate in-flight guard refuses a queued still |
| AC7 (regression) | the gates (§5) | — | 0 failed, ≥924 workspace, ≥8 ignored sidecar, ≥302 vitest; keyless audit clean; CHANGELOG; `.app` builds + launches |
| S1 (D7) | `resume_submits_shot_zero_when_all_stills_are_ready` | `chain.rs` | stills all downloaded + no shot 0 → resume submits it; a Queued still → left; a Failed still → left parked |

**Non-vacuity / mutation checks (named so they are not invented late):** make `derive_keyframes`
emit `N` (not `N+1`) ⇒ `derive_keyframes_builds_n_plus_one_stills` fails; skip the `can_bind(End)`
check in `prepare_next_shot` ⇒ `a_no_end_frame_model_falls_back_to_start_only` fails (the frame lands
on a route that drops it); score end-adherence with `Y:` instead of `All:` (reuse S16's) ⇒ the colour
gate reads wrongly; treat a `None` end score as `false` ⇒ `a_scoring_failure_leaves_end_none…` fires a
spurious regenerate; share one `attempt` counter across stills and shots ⇒
`still_and_shot_auto_retry_bounds_are_separate` fails; submit shot 0 before the last still's
`local_path` lands ⇒ `all_stills_ready_submits_shot_zero…` fails.

**AC7 screenshots:** as S13/S15/S16 — no automated screenshot tool; the VERIFY agent launches the
`tauri build` `.app`, drafts a 2×15 keyframe set (Keyframe toggle on), watches the still cards render,
and captures/narrates the two-score join badge in `test.md`.

---

## 5. Gate commands (AC7), exact

Node 24 at
`/private/tmp/claude-501/-Users-jorge-Documents-GitHub-hickeyfield/79be25e2-8f71-4192-9e6d-cd0e9d068f1c/scratchpad/node-v24.8.0-darwin-arm64/bin`
(prepend to `PATH`).

```
# dummy keys so shell tests that read a key env do not skip/panic:
export hickeyfield_FAL_KEY=x hickeyfield_HIGGSFIELD_KEY=x hickeyfield_HIGGSFIELD_SECRET=x
cargo test --workspace                                   # 0 failed, baseline (>=924) + the new tests
cargo test -p hickeyfield-core --lib ffmpeg -- --ignored # the real-sidecar score tests (>=8; Mac w/ fetched ffmpeg)
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo deny check                                         # no dependency/manifest change, MUST stay green
./scripts/lint-provenance.py                             # v3 prompt is new prose — confirm it passes
env -u hickeyfield_FAL_KEY -u FAL_KEY cargo run -q -p hickeyfield-core --example audit_fal  # keyless; checks 1–7 unchanged, RANGE MISMATCH 0
export PATH="/private/tmp/claude-501/-Users-jorge-Documents-GitHub-hickeyfield/79be25e2-8f71-4192-9e6d-cd0e9d068f1c/scratchpad/node-v24.8.0-darwin-arm64/bin:$PATH"
cd ui && pnpm test && pnpm build                         # vitest 0 failed (>=302); tsc --noEmit + vite build
ui/node_modules/.bin/tauri build --bundles app           # from repo root; the .app launches (window, not blank)
```

**Never** `first_light` (spends money, no key). **Never** bare `cargo build` for a runnable app.

---

## 6. Order of work (safest seams first; each step leaves the workspace green)

1. **Plan shape** (D1): `Keyframe`/`ImageSize`, `StoryboardPlan.keyframes`, `derive_keyframes`, the
   three `PlanError` variants + validator, v3 prompt, FakeChat fixtures. Pure, no dependant yet.
2. **Store migration 11** (D2): `keyframes_json` + `KeyframeRecord` + `parse_keyframes`/
   `upsert_keyframe`; the `JoinRecord` end-* additive fields; round-trip + `user_version == 11` test.
3. **Both-frame binding + keyless bind test** (D3): extend `prepare_next_shot` with the End attach +
   `can_bind(End)` fallback advisory; prove both keys land against the vendored schema. Pure.
4. **End-adherence score** (D4): extend `score_join` (extract clip i's last frame, score vs K_{i+1});
   `join_passes_end`; pure argv + `--ignored` real-sidecar, before any shell barrier depends on it.
5. **Keyframe generate + the stills-ready barrier** (D2): `generate_storyboard_inner` keyframe branch
   (store, submit stills, hold shots); `chain::keyframes_advance` + the shot-0 submit + the park;
   wire into `app.rs`. The behavioural heart of AC2/AC3.
6. **Regenerate + auto-retry bounds** (D4): the still-regenerate guard (separate bound) + the shot
   auto-retry extension for an end-adherence failure.
7. **Resume** (D7): `resume_all` extension for the stills-ready and shot-0 barriers.
8. **UI** (D5): toggle, still cards, stills-inclusive total, two-score badge, vitest.
9. **Docs** (§7), then `tauri build` + screenshots.

This follows the contract's dependency order (plan → store → binding → score → generate/barrier → UI)
with the pure helpers pulled ahead of their consumers, exactly as S15/S16 sequenced.

---

## 7. Documentation (the DOCUMENT step, after code lands)

- **`CHANGELOG.md`** — `### Added`: a storyboard can now be generated in **keyframe-anchored mode** —
  the app renders one still per shot boundary (a shared world block + the shots' hold descriptions),
  prices every still into the total shown before anything is spent, and anchors each shot on **both**
  ends (the previous clip's true last frame in, the next planned still out) so every join is pinned
  from both sides and scored against the ideal tier plus an end-frame-adherence score. Money honesty:
  the total includes the stills; a failed still parks the set with a clear error and never submits a
  shot against a missing anchor; stills and shots have separate ≤2 auto-retry bounds; scoring spends
  nothing (local ffmpeg on saved frames).
- **`README.md`** — extend the storyboard bullet with the keyframe mode, the still cost, and the
  two-score join; else **N/A**.
- **`docs/`** — a short note where continuity is described that the 99% keyframe path (memo §3/D-D) is
  now shipped as the ideal tier on top of S16's floor tier; the ffmpeg license release-blocker is
  unchanged (no redistribution here).
- **`.forge/backlog.md`** — the Non-goals (§8) as follow-ups; mark the S17 entry.
- **`document.md`** — written after docs land, recording file + section for each change and the
  weakest parts for the verifier.

---

## 8. Non-goals (recorded, not skipped — each a backlog follow-up)

- **N1 — threshold calibration on real footage.** The ideal tier `0.90/6/3` and end-adherence line
  ship **reasoned, not measured** (inherited from S16 Q6); the calibration commit is human-run against
  the first real paid keyframe set. This slice proves the machinery and non-vacuity, not the numbers.
- **N2 — music / captions / audio.** Unchanged from S15/S16.
- **N3 — extend-model chaining** (Veo/Grok extend endpoints): out of scope; keyframe mode is
  last-frame + planned-still i2v, not the provider's extend op.
- **N4 — sharpest-of-N / head-trim** (memo §6): deferred with the S15 recorded shape.
- **N5 — no change to the S13/S15/S16 money-path body, poll/download lifecycle, or the enhancer.**
  Stills and both-frame shots are ordinary submits.
- **N6 — no new dependency, no ffmpeg redistribution** (the "educational use only" license blocker
  stands, objective §Hard external blockers).
- **N7 — model-drafted keyframe prompts.** Keyframes are *derived* (D1), not a new JSON key the model
  must emit; a model-authored still contract is a possible future refinement, recorded.

---

## 9. Open questions (each with the default this build assumes)

- **Q1 — Still canvas: `image_size` as `{width,height}` or a named preset?** **Default: a
  `{width,height}` object** derived from the set's resolution height × aspect (D1), because the image
  models take a free-form `image_size` and the video canvas must equal the still's dimensions ("canvas
  follows the frame"). If a specific fal image model rejects a raw `{width,height}` in favour of a
  named size (e.g. `square_hd`), the derivation maps to the nearest named size at IMPLEMENT and
  records it. Reversible.
- **Q2 — Still model default: `qwen_image_3` (priced) or a GPT tier (higher quality, unpriced)?**
  **Default: `qwen_image_3`** — the only known-priced TextToImage route, so the total shows a real
  number; the owner may override to a GPT tier and accept an "unpriced" total (D1/D6).
- **Q3 — Keyframes derived or model-drafted?** **Default: derived** (D1) — the still shares the
  identical world string verbatim and needs no second LLM round trip or JSON contract. A model-drafted
  keyframe (N7) is a reversible future refinement.
- **Q4 — Where is end-adherence stored?** **Default: additive end-* fields on the existing
  `JoinRecord`**, with a `JoinRecord{index:0}` allowed in keyframe mode to carry shot 0's end
  (relaxing S16's "no join 0" additively) — no migration, `joins_json` already round-trips (D4).
- **Q5 — Still shot-index space.** **Default: `shot_index = None`** for still jobs, tracked by
  boundary in `keyframes_json` (D2), so the shots keep a clean `0..N` index space and `advance`/the
  idempotency guard are untouched. A first-class job DAG is the reversible future fix.
- **Q6 — Separate auto-retry bounds.** **Default: `KEYFRAME_RETRY_CAP = 2` on `KeyframeRecord.attempt`
  and the S16 `AUTO_RETRY_CAP = 2` on `JoinRecord.attempt`** (D4) — a still failure and a shot join
  failure are different work and must not share a budget; both matching the house "two repairs then
  escalate" rule.
- **Q7 — Gate the keyframe joins against floor or ideal?** **Default: ideal** (0.90/6/3) — keyframe
  mode is the 99% path, so its badge and auto-retry trigger read the ideal tier (D4); the floor stays
  the S16 flag for non-keyframe chained sets.

---

## 10. Riskiest parts (where the verifier should start)

1. **The stills-ready barrier and its idempotency with resume (D2/D7). — Weakest link.** Two barriers
   now gate the chain (stills → shot 0, then the S15 settle chain), and correctness depends on
   `keyframes_advance` firing shot 0 **exactly once** when the last still lands, and `resume_all` not
   double-firing it after a crash. The failure modes are the same S15 ones (double-submit or stall) but
   one layer earlier. `all_stills_ready_submits_shot_zero…` and `resume_submits_shot_zero…` are the
   guards; the verifier should confirm both are non-vacuous and that a still settling (shot_index None)
   never trips `advance`/`score_join`.
2. **Both-frame binding on the real wire (D3).** The keyless bind test asserts both keys land against
   the *vendored* schema; the verifier should confirm the H3 Max i2v path still binds `end_image_url`
   (it is not in `NO_END_FRAME`) and that a `NO_END_FRAME` model genuinely falls back start-only with
   the advisory — a silently dropped end frame is a full-price render the user did not ask for (the S9
   harm). This is the AC3 heart.
3. **The still canvas ↔ video canvas equality (D1/Q1).** If the derived `image_size` does not match
   the set's resolution×aspect, H3 Max rescales/crops the still before conditioning and the end anchor
   drifts — the exact continuity hazard S16 D-F named. `KeyframeCanvasMismatch` guards the plan; the
   verifier should confirm it is non-vacuous and that the `{width,height}` shape is accepted by the
   chosen image model (Q1) — the one thing this design could not run.
4. **End-adherence scoring reads the *actual* last frame (D4).** The last shot has no successor, so
   `score_join` must extract its own last frame; if it instead reused a stale `frames/shot{i}.png` (or
   scored the wrong PNG) the end score would be meaningless. `end_adherence_reads_the_actual_last_frame`
   (`--ignored`) is the check; the verifier must run it on the real sidecar, not trust the fixtures.
5. **Separate retry budgets and the total-shown invariant (D4/D6).** A still regenerate and a shot
   regenerate are money-adjacent; the verifier should confirm the still bound and the shot bound do
   not share a counter, that a still-regenerate is refused while a still job is in flight, and that
   every still/shot regenerate writes `estimated_usd` before it spends (the S16 M1 invariant, extended
   to stills).
6. **The thresholds are inherited, not measured (N1).** The ideal tier and the end-adherence line are
   reasoned; the `--ignored` synth cases prove the *machinery*, not that 0.90/6/3 is the right line for
   an H3 Max end frame. The verifier should confirm N1's "calibrate on one real keyframe set" is
   recorded, not that the numbers are correct.
