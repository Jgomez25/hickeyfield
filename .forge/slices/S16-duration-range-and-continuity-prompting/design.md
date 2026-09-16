# S16-duration-range-and-continuity-prompting — DESIGN

Contract: `.forge/slices/S16-duration-range-and-continuity-prompting/slice.md` (AC0–AC10).
Upstream this design is built on and reuses wholesale:
`.forge/objective.md` (§Constraints stack rule, §A no-regression gates, §"no paid job"), the
**Fable memo** `.forge/slices/S15-storyboard-render/h3max-30s-memo.md` (§0 the two score tiers,
D-E world block + validator, D-F fill-to-ceiling + one resolution per set, §0.5/§7 the synthesised
tests, Q4 threshold calibration), and the shipped slices: S15 (`.forge/slices/S15-storyboard-render/
design.md` + implement/review — `chain.rs`, `render.rs`, `Join`, `joins_json` reserved in migration
9, the override seam, `last_frame(bin,clip,out,size)`, the `1..=8` floor), S14 (H3 Max i2v
constant fields, family-root suffix resolver), S13 (plan DTO, `storyboards` table, `submit_one`,
`generate_storyboard_inner`, retry-into-set), S10 (the sandboxed `ffmpeg -i` probe, `run_to_file`),
S9 (`Pinned`/`Refusal` advisory shape, `MediaRole::Start`→`image_url`). House style followed:
S15's `design.md` — grounded `file:line`, decisions with rationale + a worked example each,
per-AC named tests, exact gate commands, order of work, open questions with recorded defaults,
riskiest parts. Owner priority (memory): **continuity is the top product problem — 95% measured
floor, 99% ideal.** This slice ships the memo's **floor tier**: the score machinery, the flag, the
planner rules, one manual regenerate, and bounded auto-retry groundwork. The keyframe/`end_image_url`
mode (the 99% lever, memo §3/D-D) is **S17**, not here (§Non-goals).

---

## 0. Session constraint, stated before anything is claimed

Authored by reading the working tree at HEAD `801fb0f` this session; every `file:line` below is
first-hand. Written **without running** `cargo`, `pnpm`, or `tauri build` — a `tauri build` is
holding the working tree and the launching agent forbids touching `ui/`, `src-tauri/`, `crates/`
while it runs — so every count and every "still green" is a claim the TEST/VERIFY agents must
confirm, not a measured fact. This artifact writes **only** `design.md`; no source is modified.

**No money, no keys, never `first_light.rs`.** The two new sidecar behaviours (a first-frame
extract and a per-join SSIM/signalstats score) spawn only the bundled **ffmpeg sidecar** on **local
PNGs** — no network, no provider, no charge. Chained shots and every regenerate reuse S13's
`submit_one`, which agents never run against a real provider. Every score test uses **synthesised
lavfi clips** (`testsrc`, `eq`, `hue`), and the real-sidecar ones are `#[ignore]`d so the keyless
`cargo test --workspace` gate never spawns ffmpeg. The two threshold constants are **reasoned, not
measured** — Q6 records that they need one real paid 2×15 set to calibrate, which is human-run.

### 0.1 Ground truth, as read this session

| Fact | Read at |
|---|---|
| `fal_schema::parse` records `enums`/`pinned`/`required` but **ignores `minimum`/`maximum`**; the `anyOf` branch is explicitly *not descended into* (comment `:400-404`, test `a_nullable_integer_is_not_enumerated` `:981`) — so Wan 3.0 Prime's `duration anyOf[{integer 2..30},{null}]` is a bare free-form axis today | `fal_schema.rs:350-423`, `:981-991` |
| `EndpointSchema { accepted, required, enums, pinned }`; `#[serde(default)]` on `pinned` for old snapshots; `reconcile`/`Pinned{field,from,to}` is the advisory shape S9 uses | `fal_schema.rs:46-72`, `:158-163`, `:193-258` |
| `Axis { support, values, default, source, default_source }`; `is_free_form()` = `Yes && values.is_empty()`; `fal_axis` reads `schema.enums` only; `from_catalog`/`from_fal` build the three axes; **no `range` field** | `capability.rs:208-286`, `:603-619`, `:391-488` |
| `CapabilitiesDto { supports_duration, durations, default_duration, … }` built by `from_capability`; the `model_capabilities` command fetches the **live** schema (`fal_schema::for_endpoint`) | `commands.rs:295-351`, `:522-571` |
| `durationLadder(caps)`: enum → `caps.durations`; else if `supportsDuration` → hardcoded `[3,4,5,6,8,10,12]` (+default) — the owner's "capped at 12" bug; else `[]` | `ui/src/lib/variants.ts:45-53` |
| `resolveSettings` leaves a **free-form** duration untouched (`caps.durations.length===0`) | `variants.ts:131-136` |
| `ModelCapabilities` TS: `supportsDuration, durations, defaultDuration?, …` — no `durationRange` | `ui/src/types.ts:33-40` |
| `ChipRow` renders duration only when `supportsDuration && durations.length>0`, mapping each to a `{value,label}` `<select>` option | `ui/src/components/ChipRow.tsx:68-83` |
| Registry H3 Max / Turbo: `duration` is **`ValueSpec::Number`** (free-form), and the **5–15 range lives only in a constraint prose string** `"fal renders … clips of 5-15 seconds (default 5)"`; `resolution` is the `480P/768P/1080P` enum; `aspect_ratio` deliberately off-spec | `registry.rs:1849-1872`, `:1852` |
| Wan 3.0 Prime: `duration` `ValueSpec::Number`, range in prose `"…a plain integer from 2 to 30 seconds (default 5)…"` | `registry.rs:2167-2189` |
| `ValueSpec { Enum, Integer, Number, Boolean, Text, Media }` — a per-flag JSON-type tag matched in `media.rs`/`recipe.rs`/`audit_fal`; **no `Range`** | `catalog.rs:60-66` |
| Every other free-form-`duration` model (grep `("duration", false, ValueSpec::Number)`): `wan3_0_prime`, `wan2_5_video`, `minimax_h3`, plus the block at `registry.rs:1714/1731/1757/1777/1795/1809/1867/1885/2179` (H3 family, Kling t2v, Seedance family, Hailuo, Wan family). None publishes a fal enum for duration; a handful publish an `anyOf` range (Wan 3.0 Prime) and most publish nothing | `registry.rs` (grep) |
| `submit_one(store, prices, runner, model, input, sid, shot_idx)` is the whole money path: `compiled_for_submit` → `Billable::from(&input.settings)` → `route::resolve` → `estimate_with_provenance` → upsert Queued → `submit_to_provider(…, overrides)` → `runner.watch`; the job carries `advisories: Vec<String>` | `commands.rs:1035-1140`, `:1053` |
| `SubmitInput { model_id, route_id, prompt, settings, media, final_prompt, constant_overrides, … }`; `constant_overrides` is the S15 seam (`apply_overrides` before `apply_constants`) | `commands.rs:717-752`; `fal_schema.rs:309-322` |
| `storyboard::{plan, validate(shots,target,uc,overlap), clamp_shot}`; `validate` floor is `1..=8` (`:368`, S15 D-G); `plan` composes nothing today — the model's `prompt` is used verbatim; `Shot { title, prompt, seconds, camera, model_id, role, continuity, advisories, chainable }`; `clamp_shot` snaps an **enumerated** axis and leaves free-form as-is (`:476`) | `storyboard.rs:278-477`, `:64-96` |
| `PlanError { Backend, Refused, NotJson, ShotCount, DurationSum, UnknownModel, ModelNotRunnable, UnknownCamera }` — all pure | `storyboard.rs:120-142` |
| The storyboard system prompt `prompts/storyboard.v1.md` still says **"Emit between 3 and 8 shots"** (stale vs the `1..=8` floor) and names keys `title/prompt/seconds/camera/modelId/role` — **no `world`, no `action`, no hold rule** | `prompts/storyboard.v1.md:9`, corpus `include_str!` `corpus.rs:60` |
| `chain.rs`: `advance` (settle → extract `last_frame(size)` → `prepare_next_shot` → `submit_one`), `chainable_model(model) = use_case::supports(model, ImageToVideo)` (**coarse, model-level** — not the per-route bind check S15 D2.4 described), `storyboard_seed(id)` FNV-1a u32, `chained_overrides(seed)` = `{prompt_expansion_mode:"disabled", seed}` | `chain.rs:40-69`, `:79-240` |
| `render_storyboard`: per-join continuity derived from **`job.media` (a Start attachment present)**, not from `joins_json`; `crossfade_in(overlap)` on non-continuity joins | `render.rs:139-149` |
| Store: migration 9 already added `chained`/`shots_json`/`overlap_seconds`/**`joins_json TEXT` (reserved, always `None` today)** on `storyboards`, plus `storyboard_renders`; `user_version = 9`; `Storyboard.joins_json: Option<String>` round-trips | `store.rs:246-269`, `:25-46`, `:296-316`, `:460` |
| `ffmpeg.rs`: `last_frame_args(clip,out,scale)` (input opts before `-i`, `-sseof -3`, `-update 1`, optional `scale=W:H:flags=lanczos`), `run_to_file` (argv vector, drain+kill+deadline, asserts `status.success()`), `parse_probe`/`parse_dimensions`/`parse_timecode` (pure, on captured stderr, never 0 on failure), `FfmpegError::{NotFound,Spawn,Probe,Licence,Render}`, `probe_args` (`-protocol_whitelist file` before `-i`) | `ffmpeg.rs:704-940`, `:615-674`, `:81-105` |
| S13 retry-into-set guard: refuses resubmitting a shot whose row **is not `Failed`** at that index | `commands.rs:1466-1481` |
| `audit_fal` checks 1–6 (ignored media, missing required, dropped settings, wrong options, pinned, bad constants), pure helper fns each with a test; keyless (`fal_schema::for_endpoint` unauthenticated) | `examples/audit_fal.rs:39-343` |

**Corrections to the contract's Ground truth (re-verified this session):**
- The contract says migration 9 "reserved `joins_json`". Confirmed: the column and the
  `Storyboard.joins_json` field both exist and round-trip; AC9 fills it and **needs no new
  migration** for the score. (A migration 10 is discussed only for the AC0-H4 unique index, which I
  **do not** take — §0.2.)
- The contract says `chainable_model` is "a coarse check". Confirmed: it is
  `use_case::supports(model, ImageToVideo)` (`chain.rs:41`), model-level, not the per-route
  `resolve_endpoint + can_bind` the S15 design D2.4 specified. Folded as AC0 hygiene (§0.2 H2) since
  this slice's continuity work depends on "can this model actually take a start frame" being right.
- The contract lists `variants.ts durationLadder` as the owner-facing bug. Confirmed at
  `variants.ts:48` — the hardcoded ladder stops at 12; D3 replaces it with a range-built one.

### 0.2 AC0 — S15 review/test minors folded here as hygiene (each cheap, or a recorded "why not")

The launching agent asked me to fold the cheap S15 minors as AC0 or say why not:

- **H1 — partially-chained fallback join renders as the wrong kind.** `render.rs:145` derives a
  continuity join from `job.media` (Start present). This is a proxy; the authoritative record is
  `joins_json`. **Folded into D8**: render reads each join's `kind`/`applied` from `joins_json` when
  present, falling back to the media-presence heuristic only for a set with no scored joins. Fixes
  the case where a shot that fell back to text-only (no Start) but the *set* is chained rendered as
  a hard cut with a mismatched frame.
- **H2 — `chainable_model` is coarse.** Tighten it to the per-route authority S15 D2.4 named:
  a route whose Image endpoint resolves **and** binds `MediaRole::Start`
  (`media::resolve_endpoint(slug, InputMode::Image, produces_video).is_ok() && media::can_bind(&spec,
  Start, dialect_for(provider,slug), slug)`). `storyboard::plan` sets `shot.chainable` from the same
  predicate so the toggle default and the scheduler cannot drift. **Low risk, folded with a guard
  test**; MUST keep the H3 Max i2v path `true` (the S14 route). If the verifier finds it regresses
  any landed chainable model, revert to the coarse check and record — the continuity ACs do not
  strictly require the tightening.
- **H3 — vacuous phase-guard test.** `a_failed_shot_stops_the_chain` (`chain.rs:478`) uses a
  sidecar-less path where even a *completed* predecessor could not submit (no configured provider),
  so the assertion "no row at `next`" holds regardless of the `Phase::Completed` guard — vacuous.
  **Folded**: add a sibling test that drives `advance` on a **Completed** predecessor through a
  fake-provider `Runner` that would write a row, proving the guard (not the missing provider) is
  what stops a failed predecessor. Named `a_completed_predecessor_would_advance_so_the_failed_guard_bites`.
- **H4 — TOCTOU on the idempotency read-guard → a `UNIQUE(storyboard_id, shot_index)` index.**
  **Not taken; recorded why (Q7).** A unique index on `(storyboard_id, shot_index)` would make the
  S13 **retry-into-set** path illegal: a retry legitimately mints a *new* job id at an existing
  `shot_index` (`commands.rs:1466-1481` allows it when the old row is `Failed`), and AC9's
  regenerate does the same for a low-join Completed shot. The read-guard (`advance` no-ops if a row
  exists at `next`, `chain.rs:163-166`) is kept; the residual double-fire window is bounded because
  settles are dispatched from `download_outputs` on the runner and `chain::resume_all` runs **once**
  at startup after `runner.resume_all`. A first-class job DAG is the reversible future fix.

---

## 1. Decisions

### D1 — Schema learns integer/number ranges (AC1)

`EndpointSchema` gains one field, defaulted for old snapshots:

```rust
/// The [min, max] bound of a numeric field, where the endpoint declares one.
///
/// Recorded because a range is NOT an enum: turning `minimum:5 / maximum:15`
/// into `[5,6,…,15]` would invent members fal never published (the LTX-class
/// bug the enum reader already guards). fal states a duration bound two ways —
/// directly (`{type:"integer", minimum, maximum}`) and, for a nullable field,
/// inside `anyOf[{integer, minimum, maximum}, {null}]` (Wan 3.0 Prime) — and
/// both must be read or the range the UI shows is wrong for half the roster.
#[serde(default)]
pub ranges: BTreeMap<String, (f64, f64)>,
```

`parse` (`fal_schema.rs:364-405`) gains a per-property range read that runs **alongside** the enum/
const read (a `const`-pinned or fully-enumerated field has no range; a nullable range field is not
an enum, so the two never collide): for each `(name, prop)`, take `minimum`/`maximum` from `prop`
directly, else scan `prop["anyOf"]` / `["oneOf"]` members for the first object carrying both.
Record `(min, max)` only when both are present and `min <= max`; a field with neither stays absent.
This is the **one** place `anyOf` is now descended — for a range, never for an enum — so the
`a_nullable_integer_is_not_enumerated` invariant (`:981`) still holds (it asserts `enums` is empty;
this slice adds an assertion that `ranges["duration"] == (2.0, 30.0)`).

**Worked example (AC1 tests, pure, on captured docs):**
- Wan 3.0 Prime `anyOf`: `duration:{"default":5,"anyOf":[{"type":"integer","minimum":2,"maximum":30},{"type":"null"}]}` → `ranges["duration"] == (2.0, 30.0)`, `enums` untouched, `pinned` empty.
- A direct integer with a max: `duration:{"type":"integer","minimum":5,"maximum":15}` (the H3 Max shape were fal to publish it) → `ranges["duration"] == (5.0, 15.0)`.
- A field with no range: `prompt:{"type":"string"}` → `ranges.get("prompt") == None`.
- A `const` field carries no range; a fully-enumerated field carries no range (assert both absent).

### D2 — Capability carries a range, sourced live-first (AC2)

`Axis` gains `range: Option<(f64, f64)>`. Its precedence is **live schema > registry-declared >
none**, exactly the source order the module already documents for values/defaults:

- **Registry representation — decision: a spec-level `duration_range: Option<(f64,f64)>` on
  `ModelSpec`, NOT a new `ValueSpec::Range{min,max}`.** Rationale: `ValueSpec` is a per-flag *JSON
  type* tag (`Integer`/`Number`/`Enum`/…) matched exhaustively in `media.rs`, `recipe.rs`,
  `catalog.rs` and `audit_fal`; adding a `Range` arm forces a churn of every one of those matches
  and, worse, conflates "the wire type" with "the numeric bound" — H3 Max's `duration` is genuinely
  a free-form `Number` (reconcile/coerce must keep treating it as one), it merely *also* has a
  documented bound. A spec-level `duration_range` is additive, matched by nothing, leaves
  `ValueSpec::Number` and the whole `reconcile` path untouched, and mirrors how the constraint prose
  already lives on the spec. It flows to capability through `spec.capabilities()` (add
  `duration_range` to `catalog::Capabilities`, populated from the spec) so **both** `from_catalog`
  and `from_fal` see it with no new plumbing on the call sites.
- `fal_axis(schema, field, catalog_default, registry_range)` sets `range =
  schema.ranges.get(field).copied().or(registry_range)` — the live bound wins, the registry bound is
  the offline/unpublished fallback (H3 Max, where fal publishes no min/max). `from_catalog`'s `axis`
  closure sets `range` from the registry range directly (catalogue-sourced, `Source::Catalog`).
- Set `duration_range = Some((5.0, 15.0))` on `minimax_h3_max` and `minimax_h3_max_turbo` in
  `registry.rs` (the number the prose already states); leave the prose constraint as the
  human-readable twin. Wan 3.0 Prime needs **no** registry range — fal publishes it, so D1 supplies
  `(2,30)` live and the offline fallback can stay `None` (or be set to `(2,30)` for the offline
  path; default: set it, so an offline draft clamps identically to an online one — recorded Q2).
- `CapabilitiesDto` gains `pub duration_range: Option<(f64, f64)>`; `from_capability` copies
  `c.duration.range`. TS `ModelCapabilities` gains `durationRange?: [number, number] | null`.

**Worked example (AC2 tests):** `from_fal` on an H3 Max i2v schema with **no** published range but a
registry `duration_range=(5,15)` → `cap.duration.range == Some((5.0,15.0))`, `source` still
`FalEndpoint` for values/support (there are none — it is free-form), `range` sourced from the
registry. `from_fal` on a Wan 3.0 Prime schema whose `ranges["duration"]==(2,30)` **and** a registry
`(2,30)` → the live `(2,30)` (precedence proven by feeding a *different* registry value and
asserting the schema wins). `from_catalog` (offline) → the registry range.

### D3 — UI duration ladder is built from the range (AC3)

`durationLadder(caps)` gains a middle branch, precedence enum → range → generic → none:

```
if caps.durations.length > 0            -> caps.durations              // enumerated axis (unchanged)
if caps.durationRange                    -> ladderFromRange(range, default)
if caps.supportsDuration                 -> [3,4,5,6,8,10,12] (+default) // no range known (unchanged)
otherwise                                -> []
```

`ladderFromRange([min,max], default)`: integers `ceil(min)..=floor(max)`, then thin to **≤12** chips
by an even stride while **always** keeping the default and the endpoints, and **never** emit a value
`< min` or `> max`. `resolveSettings` (the clamp that runs on model switch) also clamps a free-form
duration into `[min,max]` when a `durationRange` is present (today it leaves free-form alone,
`variants.ts:135`) so switching to H3 Max cannot leave 20s staged.

**Worked examples (AC3 vitest):**
- H3 Max caps `{supportsDuration:true, durations:[], durationRange:[5,15], defaultDuration:5}` →
  `[5,6,7,8,9,10,11,12,13,14,15]` (11 ≤ 12, includes 5 and 15) — **15 is offered, 12 is not the
  ceiling**, the owner's report closed.
- Wan `durationRange:[2,30]` default 5 → 29 integers thinned to ≤12, e.g. `[2,5,8,11,14,17,20,23,26,30]`
  (default 5 forced in, both endpoints kept, none outside `[2,30]`).
- No range (`durationRange` null, `supportsDuration:true`) → old `[3,4,5,6,8,10,12]` behaviour.
- Still-image model (`supportsDuration:false`) → `[]`, chip hidden (unchanged).

### D4 — Clamp-with-advisory at submit (AC4)

A duration outside the model's range is clamped **before** the estimate, with a `Pinned`-style
advisory, and the estimate prices the clamped value. In `submit_one`, after the route resolves and
**before** `Billable::from(&input.settings)`:

1. Read the model's duration range offline (`capability::for_route(&spec, provider, slug,
   mode, |_|None).duration.range`) — network-free on the hot money path; it is the registry range,
   which is what the UI ladder showed offline and never wider than the live one for our roster
   (the live-vs-registry gap is guarded by AC6/D10). Recorded Q3.
2. If `settings.duration` is `Some(d)` and `d < min` or `d > max`, clamp to the nearest bound
   `c`, set `settings.duration = Some(c)`, and push an advisory onto the job's `advisories`:
   `"{display_name} runs {min}–{max}s; {d}s clamped to {c}s"`. The clamped `settings` is what
   `Billable::from` and `estimate_with_provenance` then read, so the price on the row is for what
   will actually render.
3. Never *widen* and never clamp a value already inside the range (no advisory then).

**Worked example (AC4 test):** H3 Max, `duration=20` → clamped to `15`, one advisory carrying
`"5–15s"` and `"20s clamped to 15s"`, and `job.estimated_usd` equals the 15s estimate, not the 20s
one. A `duration=10` submit is unchanged and advisory-free.

### D5 — Storyboard clamp uses the range; fill-to-ceiling; one resolution per set (AC5, AC8/D-F)

`clamp_shot` (`storyboard.rs:432`) gains a range clamp for a **free-form** axis (today free-form is
left as-is, `:476`): if the axis has a `range` and the shot's seconds fall outside it, snap to the
nearest bound and record the same-shaped advisory (`"shot {n}: {model} runs {min}–{max}s, not {d}s
— clamped"`). An enumerated axis still snaps to the nearest offered value (unchanged).

- **12s on H3 Max stays 12** (5 ≤ 12 ≤ 15, no advisory). **20s → 15** with an advisory. A **single-
  shot 30s** is kept only if the model's range reaches 30 (Seedance/Wan 3.0 Prime); on H3 Max it
  clamps to 15, at which point the planner's fill-to-ceiling rule (below) has already split 30s into
  2×15 so a single 30s shot never reaches `clamp_shot` for a 15s-ceiling model.
- **Fill to the ceiling (planner rule, memo §5/D-F).** For a model with a hard per-call ceiling,
  `n_min = ceil(target / ceiling)`; the planner emits `n_min` shots of `target / n_min` integer
  seconds (remainder on the last) unless the brief has more beats — never more shots than beats.
  `build_user_message` (`storyboard.rs:481`) MUST hand the model each allowed model's ceiling
  (from `duration_range.max` when present, else the enum max). Enforcement is a **plan-time typed
  error** `PlanError::ShotOverCeiling { shot, model_id, seconds, ceiling }` (a `clamp_shot` cannot
  cap a free-form axis silently past its range without the user knowing the target was unmet), not
  a silent clamp. 30s → 2×15; 45s → 3×15; 3×10 only for a three-beat brief.
- **One resolution per set (memo §5).** The plan carries a set-wide resolution (default **768P** on
  H3 Max); `validate` gains a `MixedResolution` advisory (D6) when shots disagree. The frame
  dictates the canvas (S15 D2 §0.2), so a mixed-resolution set is a continuity hazard.

**Worked example (AC5 tests):** `clamp_shot` on H3 Max with `seconds=12` → `12`, no advisory;
`seconds=20` → `15`, one advisory naming `5–15s`. A Seedance `seconds=30` with `range=(4,30)` → `30`,
no advisory (single-shot 30s survives). A 30s brief on an H3-Max-only preference: the planner is
asked to fill to 2×15; a plan that instead proposes a single 30s H3 Max shot is rejected
`ShotOverCeiling { seconds:30, ceiling:15 }`.

### D6 — Continuity prompting: the `world` block + validator (AC8, memo D-E)

**Plan DTO (additive, serde-default so a stored S13 plan still parses):**
- `StoryboardPlan` gains `#[serde(default)] pub world: String`.
- `Shot` gains `#[serde(default)] pub action: Option<String>` (the model drafts `world` once and an
  `action` per shot; `prompt` stays for backward-compat / single-shot plans).
- In `plan()`, after parse and before validate: when `world` is non-empty and a shot has an
  `action`, compose `shot.prompt = format!("{world} {action}")` (verbatim, the exact string S7
  previews and `submit_one` sends). A legacy plan with only `prompt` composes nothing.

**System prompt (`prompts/storyboard.v2.md`, new version; bump `corpus::STORYBOARD_SYSTEM` to
`include_str!` v2 and keep v1 on disk for provenance).** The template is the memo D-E corpus:
require a top-level `world` (subject, wardrobe/materials, palette, lighting, lens, grade, aspect;
**no action verb**); every non-final shot's `action` **ends on a static hold** ("Ends on a static
hold of …"); every shot after the first in a continuous run **opens on that hold** ("Opens on the
same static hold of …, then"); no new subject after shot 1; no cut words; whole-second durations;
one resolution/aspect for the set. Also fixes the stale **"3 and 8 shots" → "1 to 8 shots"** copy
(`storyboard.v1.md:9`), the S15 D-G bound that never propagated to the prompt.

**Validator variants (new `PlanError`, all pure; `validate` gains the checks):**
- `MissingWorld` — a multi-shot plan whose `world` is `< 5` words.
- `PromptMissingWorld { shot }` — a composed `prompt` that does not start with `world` (catches a UI
  edit that broke the verbatim composition).
- `IdenticalShots { a, b }` — two shots' `action`s equal after trim + case-fold.
- `NoHoldEnding { shot }` — a non-final shot's `action` lacks "static hold".
- `CutWords { shot, word }` — an `action`/`prompt` containing a cut word (`cut to`, `smash cut`,
  `meanwhile`, `elsewhere`, `later`).
- `MixedResolution` — **advisory, not a rejection** (shots disagree on resolution; recorded on the
  first shot, since a set-wide default resolves it).
- The `1..=8` floor is **preserved** (`storyboard.rs:368`, unchanged); a one-shot plan needs no
  `world` (the world rules gate only multi-shot plans).

**Worked example (AC8 tests, FakeChat fixtures):** a two-shot plan sharing a `world` with distinct
hold-ended actions is accepted, and each shot's `prompt` starts with the `world` string. Byte-equal
actions → `IdenticalShots{0,1}`. A non-final action without "static hold" → `NoHoldEnding{0}`. A
`world` of three words → `MissingWorld`. An action containing "meanwhile" → `CutWords`. A UI edit
that prepends text to shot 1's prompt → `PromptMissingWorld{0}`.

### D7 — Per-join continuity SCORE (AC9, memo §0)

**The metric (bundled ffmpeg only).** For a join `i` (shot i-1 → shot i) in a chained set, on two
PNGs — `A` = shot i-1's continuity frame already on disk (`renders/<sid>/frames/shot{i-1}.png`,
kept by S15 D6) and `B` = shot i's **first** frame (a new `ffmpeg::first_frame`, D7.1) — compute
three numbers through the S10 sandbox:

| number | argv (input opts before `-i`; `-nostdin -hide_banner -protocol_whitelist file`; 20s timeout as the probe) | parse |
|---|---|---|
| `ssim` | `-i A.png -i B.png -lavfi "[0:v][1:v]scale2ref[a][b];[a][b]ssim" -f null -` | `All:(\d+\.\d+)` on stderr |
| `psnr` (informative) | same graph with `psnr` | `average:(\d+\.\d+)` |
| `dy,du,dv` | per PNG `-i X.png -vf signalstats,metadata=print:file=- -f null -` | `lavfi.signalstats.{YAVG,UAVG,VAVG}=`; deltas `|A−B|` in 8-bit units |

`scale2ref` normalises B to A's size before SSIM (a size mismatch is otherwise a hard error — the
mutation test in §4 relies on this). A join **passes** iff `ssim ≥ T_SSIM && dy ≤ T_Y && du ≤ T_C &&
dv ≤ T_C`. The score shown is `ssim`.

**Named constants (one place, `storyboard.rs`, beside `DEFAULT_CROSSFADE_SECONDS`):**

```rust
pub const CONTINUITY_FLOOR:  ContinuityThresholds = ContinuityThresholds { ssim: 0.80, y: 12.0, c: 6.0 };
pub const CONTINUITY_IDEAL:  ContinuityThresholds = ContinuityThresholds { ssim: 0.90, y: 6.0,  c: 3.0 };
```

The **floor** gates the flag (S16 tier 1); the **ideal** is *reported* on the badge ("continuous" vs
"in-camera") but **not gated** — Q6 records that both need one real paid set to calibrate.
`join_passes(scores, thresholds) -> bool` and `continuity_rate(joins) -> f64` are pure fns tested at
the four boundaries and over a fake store (19/20 ⇒ 0.95, memo §0.5).

**D7.1 — `ffmpeg::first_frame(bin, clip, out, size)`** — the same helper shape as `last_frame`
(`ffmpeg.rs:856`) but **without** `-sseof` (grab frame 0): `first_frame_args` = last_frame_args
minus the `-sseof -3` pair, `-frames:v 1` (the first decoded frame, no `-update`). Reuses
`run_to_file`; a missing output is `FfmpegError::Render`.

**D7.2 — `ffmpeg::continuity_score(bin, a, b) -> Result<JoinScore, FfmpegError>`** — spawns the two
argv above through a new `run_capture` (the `run_probe` skeleton — drain+kill+deadline+bounded — but
returns stderr **and** requires success for the score run; a filter-graph failure is a real error).
Pure `parse_ssim`/`parse_signalstats` on the captured text (unit-tested on strings, exactly like
`parse_probe`). Returns `JoinScore { ssim, psnr, dy, du, dv }`.

**D7.3 — When it is computed.** In the runner's settled hook (`app.rs`, beside `chain::advance`),
when a shot `i ≥ 1` in a chained set settles **and** continuity was applied for join `i`
(the join kind, from the shot's Start media / `joins_json`): a new `chain::score_join(state,
settled)` extracts `first_frame(shot_i.mp4)` → scores it against `frames/shot{i-1}.png` → **appends
/ replaces** the record for index `i` in the set's `joins_json`. It runs **before or beside**
`advance` (the two are independent: the score is for the join *into* the settled shot, advance
submits the *next* shot). A low join **never blocks** the chain — it flags. **A scoring failure
(sidecar missing, frame unreadable) sets `ssim: None`, never `0.0`** — a `0` would read as a failed
join and trigger a needless paid regenerate. `join_passes` treats `None` as "unscored, not failing".

**Store record (the reserved `joins_json` column, no migration):**
`[{ "index": i, "kind": "continuity"|"crossfade", "applied": bool, "ssim": f64|null, "psnr":
f64|null, "dy": f64|null, "du": f64|null, "dv": f64|null, "pass": bool|null, "attempt": u32 }]`.
The struct is `JoinRecord` in `store.rs`/a shared module; `Storyboard.joins_json` already round-trips.

**UI (Library set item + Storyboard shot card).** Each shot after the first shows a join badge
(`.storyboard-join`, `data-pass`): "join 0.93 — continuous" / "join 0.71 — low" / "join —" (unscored).
A low join shows **Regenerate from frame** (`.storyboard-regenerate-join`). The Library set item
shows the **min** join score beside Σ actual.

**Regenerate ("Regenerate shot N+1 from the same frame").** Calls S13's retry path
(`generate_storyboard`, `shot_indices=[i]`, same `storyboard_id`) with the **identical** conditioning
PNG (`frames/shot{i-1}.png`, still on disk) as `MediaRole::Start`, `seed = storyboard_seed(sid) +
attempt`, `prompt_expansion_mode:"disabled"` (the chained overrides). The S10 estimate and the money
gate re-run through `submit_one`; **the subset total is shown first** (the estimate for shot i is
rendered before the button spends). On settle the join is re-scored (attempt increments in
`joins_json`). The S13 retry guard (`commands.rs:1466-1481`, refuse-if-not-`Failed`) is relaxed to
**"failed OR the join at this index is below floor"**; the newest attempt (max `created_at`) is what
the Library and the badge display, and `joins_json[i]` holds the newest score. Which attempt "wins"
in a multi-attempt set is Q5 (default: latest by `created_at`).

**Worked example (AC9 tests):**
- Pure: `parse_ssim("… SSIM Y:0.98 U:0.99 V:0.99 All:0.985 (18.3)") == Some(0.985)`;
  `parse_signalstats` on a captured `lavfi.signalstats.YAVG=16.482` block; `join_passes` in at
  `(0.80,12,6)` and out one-over-each of the four; `continuity_rate([19×pass,1×fail]) == 0.95`.
- `--ignored` real sidecar (synth, no money, memo §0.5): `A = testsrc 4s` last frame, `B = A` ⇒
  `ssim ≈ 1.0, dy ≈ 0` (passes both tiers); `B = A` JPEG-reencoded `-q:v 5` ⇒ `ssim ≥ 0.9` (pass);
  `B = eq=brightness=0.1` ⇒ `dy > 12`, `ssim` still high (fails on the **colour** gate — proves it is
  non-vacuous); `B = hue=s=0` ⇒ `du/dv` fail; `B = testsrc` at t+1s ⇒ `ssim < 0.8` (fail). Mutation:
  drop `scale2ref` ⇒ the reencode case errors on size mismatch; parse `Y:` instead of `All:` ⇒ the
  desaturate case passes wrongly.

### D8 — Per-join overlap in render from `joins_json` (AC9 render half, folds AC0-H1)

`render_storyboard` (`render.rs:139-149`) reads each join's kind from the set's `joins_json` when
present: a `"continuity"` join → **hard cut** (no `crossfade_in`); a `"crossfade"` join →
`crossfade_in(overlap)`. Falls back to the current media-presence heuristic only when `joins_json`
is `None` (a set generated before this slice, or one never scored). This makes the render read the
recorded runtime fact rather than re-deriving it, fixing the partially-chained fallback join
(AC0-H1) and keeping the D3 duration assertion (`stitched_duration_joined`) honest.

**Worked example:** a 3×10 chained set where shot 2 fell back text-only (`joins_json[1].kind ==
"crossfade"`, `joins_json[2].kind == "continuity"`) renders join 1 as a 0.5s crossfade and join 2 as
a hard cut → `stitched_duration_joined([10,10,10], [Crossfade(0.5), Continuity]) == 29.5` (the exact
S15 test `stitched_duration_joined_is_per_join`, `storyboard.rs:726`).

### D9 — Bounded auto-retry groundwork (AC10, memo §0.4 tier-2)

An **optional** per-set toggle "auto-retry low joins" (default **off**). When on, a settled shot
whose join scored below floor triggers **at most 2** regenerations of that join through the same S13
retry path as D7's manual regenerate (`seed + attempt`, `prompt_expansion_mode:"disabled"`), and
**never spends without the total shown**: the cost is `estimate × (1 + retries)` surfaced before the
chain proceeds (memo §0.4). This matches the house "two repairs then escalate" rule (memory). The
`attempt` counter in `joins_json[i]` bounds the loop; at `attempt == 2` it stops and leaves the flag.
Kept as **groundwork** — the loop and the cap ship; a full 99% path (keyframe mode) is S17.

**Worked example (AC10 test, fake scorer, no money):** a `FakeScorer` returning `ssim 0.6` twice
then `0.95`: with auto-retry on, `advance`/the auto-loop attempts exactly 2 regenerates (attempt 1,
2), the third is not attempted (cap), and each attempt's estimate is recorded before submit; with
the toggle off, zero regenerates fire.

### D10 — Audit check: spec range ⊆ schema range (AC6)

`audit_fal` gains **check 7 — RANGE MISMATCH**: when fal publishes a range for a duration-class
field (`schema.ranges.get("duration")`) **and** the spec declares a `duration_range`, assert the
spec range lies within the schema range; report `"spec range (a,b) is not within the endpoint's
(x,y)"` otherwise. Pure `range_conflicts(spec, schema) -> Vec<String>` with a test (a spec `(5,20)`
against a schema `(5,15)` reports; `(5,15)` within `(2,30)` is clean; no schema range → skip). Run
recorded; **no new findings on landed models** (H3 Max publishes no schema range → skipped; Wan 3.0
Prime's `(2,30)` matches).

**Worked example:** `range_conflicts` on a fabricated spec `duration_range=(5,20)` + a schema
`ranges["duration"]=(5,15)` → one row; on `(5,15)` + `(5,15)` → none; on `(5,15)` + no schema range
→ none (unpublished, nothing to compare, exactly as the pin/enum checks skip when one side is
silent).

---

## 2. Files touched (complete; nothing else changes)

**Core (`crates/hickeyfield-core/src/`):**
- `fal_schema.rs` — `EndpointSchema.ranges` (serde-default); `parse` range read (direct + `anyOf`);
  fixture tests (D1).
- `catalog.rs` — `Capabilities.duration_range: Option<(f64,f64)>`; `ModelSpec` gains
  `duration_range` (or `spec()` builder passthrough); populated from the spec (D2).
- `capability.rs` — `Axis.range`; `fal_axis` takes/propagates a registry range with live-first
  precedence; `from_catalog`/`from_fal` pass `spec` range; `duration_range()` accessor; tests (D2).
- `registry.rs` — `duration_range=(5,15)` on `minimax_h3_max`/`_turbo`; `(2,30)` offline fallback
  on `wan3_0_prime`; tests.
- `storyboard.rs` — `world` on `StoryboardPlan`, `action` on `Shot`; prompt composition in `plan`;
  range clamp in `clamp_shot`; `ShotOverCeiling` + fill-to-ceiling ceiling in `build_user_message`;
  the new `PlanError` variants + validator checks; `ContinuityThresholds`/`CONTINUITY_FLOOR`/
  `CONTINUITY_IDEAL`/`JoinScore`/`join_passes`/`continuity_rate`; tests + FakeChat fixtures (D5/D6/D7).
- `ffmpeg.rs` — `first_frame` + `first_frame_args`; `continuity_score` + `run_capture` +
  `parse_ssim`/`parse_signalstats`; pure argv/parse tests + `--ignored` real-sidecar tests (D7).
- `corpus.rs` — `STORYBOARD_SYSTEM` → `prompts/storyboard.v2.md` (D6).

**Prompts:** `prompts/storyboard.v2.md` — the world-block corpus + the `1..=8` copy fix (D6).

**Shell (`src-tauri/src/`):**
- `commands.rs` — `CapabilitiesDto.duration_range`; clamp-with-advisory in `submit_one` (D4); relax
  the retry guard for a below-floor join (D7); `world`/`action` pass-through in the generate DTOs;
  the auto-retry toggle field on `StoryboardGenerateInput` (D9); tests.
- `chain.rs` — tighten `chainable_model` to the per-route bind check (AC0-H2); `score_join`; the
  auto-retry loop (D9); the non-vacuous failed-guard test (AC0-H3); tests.
- `render.rs` — per-join kind from `joins_json` (D8); tests.
- `store.rs` — `JoinRecord` struct + `joins_json` read/write helpers on `Storyboard` (no migration;
  the column exists); tests.
- `app.rs` — wire `chain::score_join` into the settled hook beside `advance`.
- `lib.rs` — no new command unless the regenerate needs one (reuses `generate_storyboard`); confirm.

**UI (`ui/src/`):**
- `lib/variants.ts` — `durationLadder` range branch + `ladderFromRange`; `resolveSettings` range
  clamp (D3).
- `types.ts` — `durationRange` on `ModelCapabilities`; `world`/`action` on the plan/shot types;
  `JoinRecord`; the auto-retry toggle on the generate input (D3/D6/D7/D9).
- `components/Storyboard.tsx` — world-block display/edit, hold-ending hints, per-join badge +
  Regenerate action, the auto-retry toggle (D6/D7/D9).
- `components/Library.tsx` — per-join badge on the set item, min join score beside Σ actual (D7).
- `components/ChipRow.tsx` — unchanged (it already maps `durationLadder` output); confirm no change
  needed beyond the ladder source.
- `api.ts` — the regenerate call (reuses `generateStoryboard`); mocks.
- `base.css`/`rail.css` — `.storyboard-join`/`.storyboard-regenerate-join` styles (additive).
- Tests: `variants.test.ts`, `Storyboard.test.tsx`, `Library.test.tsx`, `api.test.ts`.

**Docs:** `CHANGELOG.md`, `README.md`, `.forge/slices/.../document.md`, `.forge/backlog.md` (§8).

**Not touched (recorded):** `reconcile`/`coerce`/`apply_constants`/`apply_overrides` — **N/A, reused
verbatim** (a range is not an enum; `ValueSpec::Number` is unchanged, so the wire coercion path is
byte-for-byte). The runner poll/download lifecycle (`apply_poll`) and the money path body of
`submit_one` — **N/A, reused** (D4 adds a clamp before the estimate; the submit body is unchanged).
`media::bind`/`resolve_endpoint` — **N/A, reused** (the regenerate attaches a `MediaRole::Start` the
binder already handles). `tauri.conf.json`/`capabilities/default.json` — **N/A**: the score spawns
the sidecar from Rust (`externalBin` already declared), no new permission. `first_light.rs` —
**never** (spends money).

---

## 3. Data flow

**Honest duration, end to end (AC1–AC5):** fal schema (`ranges`) → `EndpointSchema` → `capability::
fal_axis`/`from_catalog` (`Axis.range`, live > registry) → `CapabilitiesDto.duration_range` →
`model_capabilities` command → UI `ModelCapabilities.durationRange` → `durationLadder` (chips up to
the real ceiling) / `resolveSettings` (clamp on switch). At submit, `submit_one` clamps a
range-violating duration + advises + prices the clamped value. In the storyboard, `clamp_shot` +
the fill-to-ceiling planner keep a 30s brief reachable as 2×15 on H3 Max.

**Continuity, per join (AC8–AC10):** brief → `plan` (world block composed into each prompt, hold
rules validated) → chained generate submits shot 0 → runner settles shot i → **settled hook**:
`chain::score_join` extracts `first_frame(shot_i)`, scores it against `frames/shot{i-1}.png`, writes
`joins_json[i]` (never 0 on failure) **and** `chain::advance` submits shot i+1 → UI badge shows the
join; a low join offers **Regenerate from frame** (same PNG, `seed+attempt`, total shown) → re-score
on settle. Render reads `joins_json` for per-join hard-cut/crossfade (D8). Auto-retry (D9) fires ≤2
regenerates when the toggle is on, cost shown first.

---

## 4. Test plan — every AC to a named test

Rust tests in `#[cfg(test)] mod tests` beside the code; `--ignored` real-sidecar tests synthesise
inputs (`testsrc`/`eq`/`hue`). UI tests use the repo's `react-dom/client` + jsdom pattern.

| AC | test | file | asserts |
|---|---|---|---|
| AC1 | `a_nullable_integer_range_is_recorded_from_anyof` | `fal_schema.rs` | Wan 3.0 Prime doc → `ranges["duration"]==(2,30)`, `enums` empty, `pinned` empty |
| AC1 | `a_direct_integer_min_max_is_recorded` | `fal_schema.rs` | `{integer,minimum:5,maximum:15}` → `(5,15)` |
| AC1 | `a_field_with_no_bound_has_no_range` | `fal_schema.rs` | `prompt`/a `const`/an enum field → `ranges.get==None` |
| AC2 | `an_axis_range_comes_from_the_live_schema_first` | `capability.rs` | schema `(2,30)` + registry `(9,9)` → axis range `(2,30)` |
| AC2 | `an_unpublished_range_falls_back_to_the_registry` | `capability.rs` | schema no range + registry `(5,15)` → `(5,15)`; `from_catalog` → `(5,15)` |
| AC2 | `the_dto_carries_the_duration_range` | `commands.rs` | `CapabilitiesDto.duration_range` = axis range |
| AC3 | `the_ladder_offers_fifteen_and_is_not_capped_at_twelve` | `variants.test.ts` | H3 Max caps → includes 15, ≤12 chips, 12 not the max |
| AC3 | `the_ladder_covers_the_full_range_thinned` | `variants.test.ts` | Wan `[2,30]` → ≤12 chips, endpoints + default kept, none outside |
| AC3 | `no_range_keeps_the_generic_ladder` | `variants.test.ts` | `durationRange` null + `supportsDuration` → old `[3,4,5,6,8,10,12]` |
| AC4 | `a_duration_outside_the_range_is_clamped_with_an_advisory` | `commands.rs` | H3 Max `20` → `15`, advisory `5–15s`, estimate = 15s price |
| AC4 | `a_duration_inside_the_range_is_untouched` | `commands.rs` | `10` → `10`, no advisory |
| AC5 | `clamp_shot_uses_the_range_for_a_free_form_model` | `storyboard.rs` | H3 Max `12`→`12` no advisory; `20`→`15` advisory |
| AC5 | `a_single_shot_thirty_survives_only_when_the_range_reaches_it` | `storyboard.rs` | Seedance `(4,30)` keeps 30; a 30s H3 Max shot → `ShotOverCeiling` |
| AC5 | `fill_to_ceiling_splits_thirty_into_two_fifteens` | `storyboard.rs` | ceiling handed to `build_user_message`; a single-30 H3 Max plan rejected |
| AC6 | `range_conflicts_flags_a_spec_range_outside_the_schema` | `examples/audit_fal.rs` | `(5,20)` vs `(5,15)` reports; `(5,15)` vs `(2,30)` clean; no schema range → skip |
| AC8 | `a_world_block_is_composed_verbatim_into_each_prompt` | `storyboard.rs` | each shot's `prompt` starts with `world` |
| AC8 | `identical_actions_are_rejected` | `storyboard.rs` | byte-equal actions → `IdenticalShots` |
| AC8 | `a_non_final_shot_without_a_hold_is_rejected` | `storyboard.rs` | `NoHoldEnding{0}` |
| AC8 | `a_short_world_is_rejected` / `a_cut_word_is_rejected` / `a_broken_composition_is_caught` | `storyboard.rs` | `MissingWorld` / `CutWords` / `PromptMissingWorld` |
| AC8 | `the_one_to_eight_bound_is_preserved` | `storyboard.rs` | reuse S15's `the_shot_count_bound_is_one_to_eight` still green |
| AC9 | `parse_ssim_and_signalstats_read_the_captured_lines` | `ffmpeg.rs` | `All:0.985`→`0.985`; `YAVG/UAVG/VAVG` deltas |
| AC9 | `join_passes_at_the_four_boundaries` | `storyboard.rs` | in at `(0.80,12,6)`; out one-over each; `None`→not failing |
| AC9 | `continuity_rate_over_a_fake_store` | `storyboard.rs` | 19/20 ⇒ 0.95 |
| AC9 | `a_scoring_failure_is_none_never_zero` | `chain.rs` | sidecar missing → `ssim: None`, join not flagged as failed, no auto-regenerate |
| AC9 | `identical_frames_score_near_one` (`--ignored`) | `ffmpeg.rs` | `B=A` → `ssim≈1.0`, `dy≈0` |
| AC9 | `a_brightness_shift_fails_the_colour_gate` (`--ignored`) | `ffmpeg.rs` | `eq=brightness=0.1` → `dy>12`, `ssim` high (non-vacuous colour gate) |
| AC9 | `a_different_source_falls_below_the_ssim_floor` (`--ignored`) | `ffmpeg.rs` | `testsrc t+1s` → `ssim<0.8` |
| AC9 | `render_reads_the_join_kind_from_joins_json` | `render.rs` | mixed set → one crossfade + one hard cut; duration `29.5` |
| AC9 | `regenerate_reuses_the_same_frame_and_offsets_the_seed` | `chain.rs`/`commands.rs` | Start = `frames/shot{i-1}.png`, `seed = base+attempt`, guard relaxed for a low join |
| AC10 | `auto_retry_stops_after_two_attempts` | `chain.rs` | FakeScorer 0.6,0.6,0.95 → exactly 2 regenerates, cost shown each; toggle off → 0 |
| AC0-H2 | `chainable_model_is_the_per_route_bind_check` | `chain.rs` | H3 Max i2v → true; a model with no start-frame route → false |
| AC0-H3 | `a_completed_predecessor_would_advance_so_the_failed_guard_bites` | `chain.rs` | Completed → a row appears via fake-provider runner; Failed → none (non-vacuous) |
| AC7 | the gates (§5) | — | 0 failed, baseline held; `--ignored` sidecar green; CHANGELOG; `.app` builds+launches; keyless audit clean |

**Non-vacuity / mutation checks (named so they are not invented late):** drop `scale2ref` from the
ssim argv ⇒ `identical_frames_score_near_one`'s reencode variant errors on size mismatch; parse `Y:`
instead of `All:` ⇒ `a_brightness_shift_fails_the_colour_gate` passes wrongly; make
`ladderFromRange` emit `min-1` ⇒ `the_ladder_covers_the_full_range_thinned` fails on "none outside";
make the D4 clamp price the pre-clamp value ⇒ `a_duration_outside_the_range_is_clamped…` fails on the
estimate; make `join_passes` treat `None` as `false` ⇒ `a_scoring_failure_is_none_never_zero` fires a
spurious regenerate; return the raw `minimum` without `maximum` in `parse` ⇒ AC1 range tests fail.

**AC7 screenshots:** as S13/S15 — no automated screenshot tool; the VERIFY agent launches the
`tauri build` `.app`, opens the H3 Max duration chip (15 offered), drafts a 2×15 chained set, and
captures/narrates the join badge in `test.md`.

---

## 5. Gate commands (AC7), exact

Node 24 at
`/private/tmp/claude-501/-Users-jorge-Documents-GitHub-hickeyfield/79be25e2-8f71-4192-9e6d-cd0e9d068f1c/scratchpad/node-v24.8.0-darwin-arm64/bin`
(prepend to `PATH`).

```
# dummy keys so shell tests that read a key env do not skip/panic:
export hickeyfield_FAL_KEY=x hickeyfield_HIGGSFIELD_KEY=x hickeyfield_HIGGSFIELD_SECRET=x
cargo test --workspace                                   # 0 failed, baseline (>=718) + the new tests
cargo test -p hickeyfield-core --ignored                 # the real-sidecar first_frame + continuity_score tests (Mac w/ fetched ffmpeg)
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo deny check                                         # no dependency/manifest change, MUST stay green
./scripts/lint-provenance.py                             # provenance clean (v2 prompt is new prose — confirm it passes)
env -u hickeyfield_FAL_KEY -u FAL_KEY cargo run -q -p hickeyfield-core --example audit_fal  # keyless; RANGE MISMATCH (0) + no regression in checks 1–6
export PATH="/private/tmp/claude-501/-Users-jorge-Documents-GitHub-hickeyfield/79be25e2-8f71-4192-9e6d-cd0e9d068f1c/scratchpad/node-v24.8.0-darwin-arm64/bin:$PATH"
cd ui && pnpm test && pnpm build                         # vitest 0 failed; tsc --noEmit + vite build
ui/node_modules/.bin/tauri build --bundles app           # from repo root; the .app launches (window, not blank)
```

**Never** `first_light` (spends money, no key). **Never** bare `cargo build` for a runnable app.

---

## 6. Order of work (safest seams first; each step leaves the workspace green)

1. **Schema range** (D1): `EndpointSchema.ranges` + `parse` + fixtures. Pure, no dependant yet.
2. **Capability range** (D2): `Axis.range`, `Capabilities.duration_range`, registry ranges,
   `CapabilitiesDto.duration_range`. Pure; keeps every `capability.rs` test green.
3. **Audit check 7** (D10): `range_conflicts` + test + a recorded keyless run — proves the registry
   ranges land clean before anything consumes them.
4. **UI ladder** (D3): `durationLadder` range branch + `resolveSettings` clamp + vitest. Closes the
   owner-facing bug first once the DTO carries the range.
5. **Submit clamp** (D4) + **storyboard clamp / fill-to-ceiling** (D5): the money-side safety net.
6. **World block + validator** (D6): plan DTO, prompt v2, `PlanError` variants, FakeChat fixtures.
7. **Score core** (D7.1/D7.2): `first_frame`, `continuity_score`, parsers, thresholds — pure argv +
   `--ignored` real-sidecar, the way S10 proved the probe, before any shell depends on it.
8. **Score wiring** (D7.3): `chain::score_join` in the settled hook + `joins_json` write + the
   render per-join read (D8) + the AC0 hygiene (H1/H2/H3).
9. **Regenerate + auto-retry** (D7 UI / D9): the retry-guard relaxation, the badge, the toggle, the
   bounded loop with the cost shown.
10. **UI** (badges, world editor, toggle), then **docs** (§7), then `tauri build` + screenshots.

This follows the contract's dependency order (schema → capability → UI → storyboard clamp; then the
continuity block) with the pure sidecar helpers pulled ahead of their consumers.

---

## 7. Documentation (the DOCUMENT step, after code lands)

- **`CHANGELOG.md`** — `### Added`: a model's duration control now offers the provider's real range
  (H3 Max 5–15s, Wan 3.0 Prime 2–30s) instead of a generic ladder capped at 12; a 30-second
  storyboard fills any model's ceiling (2×15 on H3 Max) with a shared world block for continuity;
  every chained join is scored (SSIM + colour drift) and flagged, with one-click "Regenerate from
  frame" and optional bounded auto-retry. Money honesty: scoring spends nothing (local ffmpeg on
  saved frames); a below-floor join flags but never blocks paid work; no regenerate spends without
  the total shown.
- **`README.md`** — extend the storyboard bullet with the world block, the honest duration range,
  and the join score; else **N/A**.
- **`docs/`** — a short note where the storyboard/continuity is described that continuity is now a
  measured per-join number with a floor tier; the keyframe 99% path is deferred to S17. The ffmpeg
  license release-blocker is unchanged (no redistribution here).
- **`.forge/backlog.md`** — the Non-goals (§8) as follow-ups.
- **`document.md`** — written after docs land, recording file + section for each change and the
  weakest parts for the verifier.

---

## 8. Non-goals (recorded, not skipped — each a backlog follow-up)

- **N1 — keyframe / `end_image_url` mode (memo §3/D-D, the 99% lever): S17.** This slice is start-
  frame conditioning + the floor-tier score only. No stills model, no keyframe cards, no in-shot
  end-adherence.
- **N2 — threshold calibration on real footage.** The two constants ship reasoned, not measured
  (Q6); the calibration commit is human-run against the first paid 2×15 set.
- **N3 — full 99% auto loop.** D9 ships the ≤2-retry groundwork; a keyframe-anchored auto loop that
  reaches 99% is S17.
- **N4 — sharpest-of-N / head-trim** (memo §6): deferred with the recorded shape (choose `t*` then
  trim shot 1; measure the opening-hold trim on the first paid set).
- **N5 — no change to the S13/S15 money path body, poll/download lifecycle, or the enhancer.** D4
  adds a clamp before the estimate; the submit body is otherwise byte-for-byte.
- **N6 — no new dependency, no ffmpeg redistribution** (the "educational use only" license blocker
  stands, objective §Hard external blockers).

---

## 9. Open questions (each with the default this build assumes)

- **Q1 — Range representation: `ValueSpec::Range` or a spec-level `duration_range`?** **Default:
  spec-level `duration_range`** (D2) — additive, leaves `ValueSpec::Number` and the whole
  reconcile/coerce path untouched, no exhaustive-match churn. Reversible.
- **Q2 — Wan 3.0 Prime offline range: set the registry fallback, or rely on the live schema only?**
  **Default: set `(2,30)`** so an offline draft clamps identically to an online one; the live schema
  still wins when reachable.
- **Q3 — Submit-time clamp source: registry (offline) or live schema (a blocking fetch)?**
  **Default: registry/offline** (D4) — network-free on the hot money path, matches the UI ladder,
  and AC6/D10 guards the registry range against the schema. A future tightening can read the live
  range where a fetch already happened.
- **Q4 — When is the join scored: at settle or at render?** **Default: at settle** (D7.3, memo Q3) —
  the flag precedes the render spend and the next paid shot; the render re-reads `joins_json`.
- **Q5 — Which regenerate attempt "wins" a multi-attempt join?** **Default: latest by `created_at`**;
  `joins_json[i]` holds the newest score, `attempt` increments. Older attempts stay as history rows
  the Library de-dupes to the latest per index.
- **Q6 — Threshold calibration (memo Q4).** **Default: floor `0.80/12/6`, ideal `0.90/6/3`**,
  re-read once against the first real paid 2×15 set and changed in one commit with the numbers
  recorded. **Human-run** — needs a real spend.
- **Q7 — Double-fire safety: read-guard or a `UNIQUE(storyboard_id, shot_index)` index?**
  **Default: the read-guard** (AC0-H4) — a unique index would make the S13 retry-into-set and the
  AC9 regenerate (a new job id at an existing index) illegal; the residual window is bounded by
  single-threaded settle dispatch + a once-at-startup resume.

---

## 10. Riskiest parts (where the verifier should start)

1. **The thresholds are reasoned, not measured (D7, memo §0.1/Q6). — Weakest link.** The `--ignored`
   synth cases prove the *machinery and the gates* (SSIM catches identity, the colour gate is
   non-vacuous), **not** that `0.80/12/6` is the right line for an H3 Max i2v first frame. The
   verifier should confirm the synth cases prove non-vacuity and that Q6's "calibrate on one real
   set" is recorded, not that the numbers are correct.
2. **`parse_ssim`/`parse_signalstats` against the real sidecar's exact text.** ffmpeg's SSIM line
   spelling and `metadata=print` format are what the parsers key off; a bundled-build variation
   would silently return `None` (never `0` — that part is safe) and every join would read as
   unscored. The verifier must run the `--ignored` tests, not trust the string fixtures alone.
3. **The clamp-at-submit source (D4/Q3).** Using the registry (offline) range on the hot path is a
   deliberate trade; if a live endpoint ever narrows its range below the registry's, a value the UI
   allowed would 422 downstream instead of being clamped. AC6/D10 is the guard; the verifier should
   confirm the audit run is clean and the clamp uses the same range the UI ladder showed.
4. **`chainable_model` tightening (AC0-H2).** Changing the predicate changes which models chain; the
   verifier must confirm the H3 Max i2v path stays `true` and no landed chainable model regresses —
   revert to the coarse check and record if it does.
5. **Regenerate + retry-guard relaxation (D7).** Relaxing "refuse-if-not-`Failed`" to allow a
   below-floor Completed shot is a money-adjacent change: it must not let a *passing* or
   *already-started* shot be resubmitted. The verifier should confirm the relaxation is gated on the
   `joins_json` floor check and that the subset total is shown before the spend.
6. **The `world`-block prompt v2 and the `1..=8` copy fix (D6).** A model that ignores the hold
   instruction is the memo's own caveat (§0.4); the validator catches a *missing* hold in the plan,
   not a model that drafts one and then does not render it. The verifier should confirm the
   validator variants are non-vacuous (each rejects a fixture that violates exactly that rule) and
   that provenance/`pnpm build` accept the new prompt file.
