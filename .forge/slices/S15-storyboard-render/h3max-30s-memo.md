# S15 memo — a 30-second ad on MiniMax H3 Max (15s per call), with measured continuity

Owner question: split one prompt into two 15s shots chained with continuity (a), or generate two 15s
videos and edit them together (b)? Owner bar: continuity between consecutive shots at 99% ideal,
95% realistic floor. Grounded in `.forge/slices/S15-storyboard-render/design.md` (D1–D4, D6),
`.forge/slices/S13-storyboard-plan/design.md` (D1, D6), `crates/hickeyfield-core/src/{storyboard,
media,compositor,ffmpeg,registry}.rs`, and S14's vendored `schemas/minimax_h3-max_image-to-video.json`.
Schema facts used without hedging (verified keyless by the coordinator): `prompt_expansion_mode` ∈
{`disabled`, `balanced` (~1s rewrite), `quality` (~30s)}; `image_url` = first frame, `end_image_url` =
last frame, either alone is allowed; "the output canvas follows this image"; `seed` is honoured;
`duration` integer 5..15; `resolution` 480P/768P/1080P. Read-only session; nothing was run.

## Decisions

| # | Decision |
|---|---|
| D-0 | **Continuity is a per-join score computed by the bundled ffmpeg, stored, shown, and gated.** Floor tier (S15 ships): every join scored, threshold flags a low join, one-click "Regenerate shot N+1 from the same frame". Ideal tier (later): keyframe-anchored mode + bounded auto-regenerate. §0. |
| D-A | **(a).** One world, two beats, shot 2 image-to-video from shot 1's true last frame. (b) is the recorded fallback (design D2 step 6), never the default. §1 |
| D-B | **A continuity join is a hard cut (`overlap = 0`)**; crossfade only where continuity was not applied. Per-join arithmetic. §2 |
| D-C | **Chained shots submit `prompt_expansion_mode: "disabled"`, a fixed per-storyboard `seed`, and a continuity frame extracted at the set's exact size.** Standalone submits keep S14's `"balanced"` constant. §0.2 |
| D-D | **`image_url` only in S15.** `end_image_url` keyframe mode is the 99% path, its own slice. §3 |
| D-E | **Plan-level `world` block** composed verbatim into every prompt; shots differ only in `action`; non-final shots end on a static hold; later shots open on that hold. Validator enforces. §4 |
| D-F | **Fill to the ceiling:** 30s on H3 Max = exactly 2×15, integer seconds, one resolution for the set, **768P default**. §5 |
| D-G | **S13's `3..=8` floor (`storyboard.rs:308`) blocks the 2×15 plan.** MUST become `1..=8`. First thing the verifier checks. §5 |
| D-H | **True last frame, hard cut, planner hold.** Sharpest-of-N (with trim) and shot-2 head-trim deferred with recorded shape. §6 |

## 0. Continuity as a number: the two tiers

### 0.1 The metric (bundled ffmpeg only, no network, no money)

A join `i` (shot i-1 → shot i) is scored on two PNGs: `A` = shot i-1's extracted last frame (the
exact `image_url` we sent, already on disk at `renders/<sid>/frames/shot{i-1}.png`, design D1) and
`B` = shot i's **first** frame (a new `ffmpeg::first_frame`: same argv as `last_frame_args`,
`ffmpeg.rs:813`, without `-sseof`). Three numbers, each parsed from ffmpeg's text output the way
S10's `parse_probe` reads the probe (pure `parse_*` fns, unit-tested on captured strings):

| number | ffmpeg argv (input options before `-i`, `-protocol_whitelist file`, `-nostdin`, 20s timeout as the probe) | parse |
|---|---|---|
| `ssim` | `-i A.png -i B.png -lavfi "[0:v][1:v]scale2ref[a][b];[a][b]ssim" -f null -` | `All:(\d\.\d+)` on stderr |
| `psnr` | same with `psnr` | `average:(\d+\.\d+)` (informative only) |
| `dy,du,dv` | per PNG: `-i X.png -vf signalstats,metadata=print:file=- -f null -` | `lavfi.signalstats.{YAVG,UAVG,VAVG}=`; deltas `|A−B|` in 8-bit units |

`ssim` measures "same picture" (identity, composition, sharpness); the `YAVG/UAVG/VAVG` deltas
catch a grade/exposure/white-balance drift that SSIM under-weights. A join **passes** when
`ssim ≥ T_ssim && dy ≤ T_y && du ≤ T_c && dv ≤ T_c`. The score shown is `ssim`.

**Thresholds (constants in one place, `storyboard.rs`, next to `DEFAULT_CROSSFADE_SECONDS`):**

| tier | `T_ssim` | `T_y` | `T_c` | meaning |
|---|---|---|---|---|
| Floor (S15 gate) | 0.80 | 12 | 6 | a viewer sees one continuous picture across a hard cut |
| Ideal (later gate) | 0.90 | 6 | 3 | indistinguishable from an in-camera cut |

These are initial values reasoned from what an i2v first frame is (a VAE round-trip of the
conditioning image, then H.264): a faithful reproduction lands ~0.9+; a re-composed subject lands
below 0.7. They MUST be re-read against the scores of the first paid 2×15 set and adjusted once;
the metric machinery, not the two numbers, is what S15 proves.

**"95%" as a test, not a feeling.** `continuity_rate = passing joins / scored joins` over the
store (a pure fn over the stored join rows). Floor tier: `≥ 0.95` over the joins of the last 20
storyboards **after at most one user regenerate per join**. Ideal tier: `≥ 0.99` after the bounded
auto loop (§0.4). For one 2×15 ad the statement is simply "the join passes".

### 0.2 Where S15 computes, stores, shows and acts on it

- **Compute at settle, not at render.** In `chain::advance` (design D2.3), when shot `i ≥ 1` settles
  and `continuity_applied[i]` is true: `first_frame(shot_i.mp4)` → score against
  `frames/shot{i-1}.png` → store. The user sees the flag before paying for a render, and before the
  next shot is submitted (the chain still advances; a low join never blocks paid work, it flags it).
- **Store (migration 9, one more column):** `storyboards.joins_json TEXT` = `[{ "index": i, "kind":
  "continuity"|"crossfade", "applied": bool, "ssim": f64, "psnr": f64, "dy": f64, "du": f64,
  "dv": f64, "pass": bool, "attempt": u32 }]`. `continuity_applied` (§2) lives here too. Frames under
  `renders/<sid>/frames/` MUST persist (amend design D6 cleanup: keep `frames/`, they are small
  PNGs) so a regenerate reuses the *identical* conditioning frame.
- **UI:** each shot card after the first shows a join badge (`.storyboard-join`, `data-pass`):
  "join 0.93 — continuous" or "join 0.71 — low". A low join shows **Regenerate from frame**
  (`.storyboard-regenerate-join`), which calls S13's retry path (`shot_indices=[i]`, same
  `storyboard_id`, `commands.rs:1425` refuse-if-not-failed guard relaxed to "failed **or** low-join")
  with the same `image_url` PNG, `seed = storyboard_seed + attempt`, the S10 estimate shown, and the
  money gate re-run through `submit_one`. On settle the join is re-scored and `attempt` increments.
  The Library set item shows the min join score next to Σ actual.
- **Submit body for a chained shot (D-C):** `prompt_expansion_mode: "disabled"` (fal's rewrite is a
  second enhancer on top of S4–S7's guarded one; two different rewrites of the same world block is
  drift by construction), `seed: storyboard_seed` (minted once per storyboard at generate, stored),
  `image_url` = the frame at the set's size. Mechanism: `FAL_CONSTANT_FIELDS` "only fills a gap; a
  value the binder wrote always wins" (`fal_schema.rs:269`), so the chain writes the field
  explicitly and standalone submits keep `"balanced"`. `seed` and `prompt_expansion_mode` are
  declared by fal, so the keyless `audit_fal` stays clean. **Keyless check:** a bind test asserts
  a chained `SubmitInput` body carries `"prompt_expansion_mode":"disabled"` and `"seed":<n>`, a
  standalone body carries `"balanced"` and no seed.
- **Frame at the set's size:** "the canvas follows this image", so the PNG dictates shot 2's
  resolution and aspect. `last_frame` gains `size: Option<(u32,u32)>` → `-vf
  scale=W:H:flags=lanczos` before the output; the chain passes the set's resolution (`resolution_height`,
  `commands.rs:613`, × the 16:9/9:16 aspect of shot 1's probe). Equal size = no-op.

### 0.3 Levers, ranked by expected effect on the join score

1. **`image_url` = the exact last frame, at the exact canvas size.** The pixel anchor; everything
   else is second order. Wrong size = fal rescales/crops before conditioning.
2. **`prompt_expansion_mode: "disabled"` on chained shots.** Removes a per-shot server rewrite
   of the world block. Cheap, certain.
3. **World block verbatim + hold ending + "opens on the same hold".** Governs how *sharp* the
   anchor frame is (a hold) and how far shot 2 drifts from it in its first second.
4. **Camera continuity.** A camera move across the cut is the main cause of a blurred anchor and of
   a visible direction change; non-final shots end static, the next shot's camera starts from static.
5. **`end_image_url` keyframes (§3).** Large effect on *in-shot* drift and endings — beyond what the
   join score measures — at the price of a stills step. The 99% lever.
6. **Fixed `seed`.** Small effect on the join (conditioning dominates the first frame); its value is
   reproducibility: a regenerate with a tweaked prompt changes one thing. On a retry the seed MUST
   change (`seed + attempt`), otherwise identical inputs reproduce the identical bad output.

### 0.4 What 95% and 99% honestly require

- **Floor 95% (S15):** items 1–4 + 6 above, the score, the flag, one manual regenerate. If the
  first-take pass rate is `p`, one retry gives `1 − (1−p)²`: `p = 0.8` → 96%, `p = 0.9` → 99%.
  95% after one retry needs `p ≥ 0.78` — plausible for a hold-to-hold i2v join, and measurable
  from `joins_json` on the first sets.
- **Ideal 99% (a later slice, "S16-keyframe-continuity"):** keyframe-anchored mode (§3), a
  **bounded auto-regenerate loop** (max 2 retries per join, matching the house "two repairs then
  escalate" rule, total cost shown as `estimate × (1 + retries)` before the chain starts), expansion
  off, fixed seed with per-attempt offset. With `p = 0.8` and two retries: 99.2%.
- **What no setting can guarantee on a generative model:** the score measures the *join*; the
  subject may morph *inside* a 15s shot (label text, proportions) with a perfect join and no
  reference frame to measure against; the model may ignore the hold instruction; end-frame
  adherence is a target it approaches, not a constraint; the safety checker may reject a frame.
  99% is a rate over joins after retries, not a first-take promise, and the memo says so in UI copy.

### 0.5 Verifier tests for the score (synthesised, no money)

Pure: `parse_ssim`/`parse_signalstats` on captured stderr strings; `join_passes` at the four
boundaries (0.80/12/6 in, one over each ⇒ out); `continuity_rate` over a fake store (19/20 ⇒ 0.95).
Real sidecar (`--ignored`): `A = testsrc 4s` last frame; `B = A` ⇒ ssim 1.0, dy 0 (pass both tiers);
`B = A` re-encoded through `-q:v 5` JPEG ⇒ ssim ≥ 0.9 (pass); `B = eq=brightness=0.1` ⇒ dy > 12
(fail on `dy` with ssim still high — proves the colour gate is non-vacuous); `B = hue=s=0` ⇒
du/dv fail; `B = testsrc` at t+1s ⇒ ssim < 0.8 (fail). Mutation: drop `scale2ref` ⇒ the
re-encode case errors on size mismatch; swap `All:` for `Y:` ⇒ the desaturate case passes wrongly.

## 1. (a) vs (b)

Two independent text-to-video calls are two *samples* of the prompt: label art, palette, lens and
light are re-drawn each time; a fixed seed does not carry a subject across two different prompts.
Cut together, that is two ads for two similar products. (a) fixes identity with pixels: shot 2's
first frame *is* shot 1's last frame, the prompt carries only the next action. (b) is right only
for a join the planner marks as a scene change (montage), which D-B treats as a crossfade join.

## 2. The join: hard cut, per-join arithmetic

With `F` = shot 1's last frame ≈ shot 2's frame 0, a 0.5s `xfade` superimposes `A(F−0.25s)` on
`B(F+0.25s)` at its midpoint: two instants of one motion at 50% each — a ghost — and since i2v
clips commonly open on a near-still hold of `F`, the motion appears to stall then double. A hard
cut is a match cut: invisible when the frames match, one frame wide when they do not. It also
keeps all 30 paid seconds (crossfade renders 2×15 as 29.5).

**Rule (MUST):** `join(i) = Continuity` iff the set is chained **and** shot i was submitted with the
Start frame (no fallback advisory, no `last_frame` failure); else `Crossfade(overlap_seconds)`.
Persist `continuity_applied[i]` (in `joins_json`, §0.2) — it is a runtime fact, not derivable later.

**Render (D3 step 3):** `.crossfade_in(overlap)` only on `Crossfade` joins. The compositor already
mixes cuts and fades (`crossfades_and_hard_cuts_can_be_mixed`, `compositor.rs:994`) and degrades
`0` to `concat` (`:1034`). **Arithmetic (D4):** add `pub enum Join { Continuity, Crossfade(f64) }`
and `stitched_duration_joined(shots, joins) -> f64` (same clamp as `storyboard.rs:207`);
`stitched_duration(shots, o)` stays as "all joins `Crossfade(o)`". Worked: 2×15 chained →
**30.0**; 2×15 unchained → **29.5**; 3×10 with shot 2 fallen back → `[Crossfade(0.5),
Continuity]` → **29.5**; 8/8/8/6 all faded → 28.5 (design D4's example). Store `overlap_seconds =
0.0` for a set whose shots are all chainable so `generate_storyboard_inner`'s re-validate and D3's
`≈ total_duration() ± 0.2s` probe assert the number that renders.

## 3. `end_image_url`: the 99% path, not S15

Binding is possible today (`MediaRole::End → end_image_url`, `media.rs:200-215`; H3 is not in
`NO_END_FRAME`, `:896`). What S15 lacks is the *frame*: shot 2's end does not exist until shot 2 is
generated. **Keyframe mode (recorded shape, own slice):** the planner emits `K0..Kn` still prompts
sharing the world block; an image model generates the stills (each estimated + guarded like a
shot); shot `i` = i2v with `image_url` = shot i-1's **actual** last frame (keeps §2's match cut;
start conditioning is tight, end conditioning approximate, so cutting `K_i`→shot i would
reintroduce a mismatch) and `end_image_url = K_{i+1}`; shot 0 uses `image_url = K0`. Every beat's
ending and the closing pack shot become fixed pictures; in-shot drift is bounded at both ends. Cost:
`n+1` stills, a second model in the plan, keyframe cards in the UI, and the auto-regenerate loop
(§0.4). **S15 = start frame only.**

## 4. Prompt template and validator (S13 planner)

Plan JSON (additive, serde-default): `{"world": "...", "shots": [{"title","action","seconds",
"camera","modelId","role"}]}`. `plan()` composes `shot.prompt = "{world} {action}"` after parse
(`storyboard.rs:250` region); `world`/`action` stay on the DTO for separate editing; the composed
`prompt` is what S7 previews and `submit_one` sends.

```
world:  "A matte-black cold-brew can with a copper rim on wet slate; studio product photography;
         single hard key top-left, soft fill; deep teal-and-copper palette; 50mm, shallow depth of
         field; clean high-contrast grade; 16:9."
shot 1: action = "Condensation beads and runs down the can as a drip lands on the slate. Ends on a
         static hold of the can centred, label to camera, one drop about to fall."          camera: push-in
shot 2: action = "Opens on the same static hold of the can centred, label to camera; then the drop
         falls and the can tilts toward camera as the copper rim catches the key. Ends on the can
         filling the frame, label sharp."                                                    camera: null
```

Corpus rules (MUST): the `world` names subject, wardrobe/materials, palette, lighting, lens, grade
and aspect, and contains no action verb; every non-final shot's `action` ends with "Ends on a
static hold of …"; every shot after the first in a continuous run begins "Opens on the same static
hold of …, then"; no new subject is introduced after shot 1 of a run; no cut words ("cut to",
"smash cut", "meanwhile", "elsewhere", "later"); seconds are whole numbers; all shots share the
set's resolution and aspect.

Validator (new `PlanError` variants, all pure): `MissingWorld` (multi-shot plan, `world` < 5
words); `PromptMissingWorld{shot}` (composed prompt does not start with `world` — catches a UI edit);
`IdenticalShots{a,b}` (`action`s equal after trim+case-fold); `EmptyAction{shot}`;
`NoHoldEnding{shot}` (non-final action lacks "static hold"); `CutWords{shot, word}`;
`MixedResolution` advisory. Acceptance: byte-equal prompts rejected; shared world + distinct
actions accepted; a one-shot plan needs no world; a non-final action without a hold rejected.

## 5. H3 Max specifics

**Duration policy (MUST, planner):** for a model with a hard ceiling, `n_min = ceil(target /
ceiling)`; emit `n_min` shots of `target / n_min` integer seconds (remainder on the last) unless the
brief has more beats; never more shots than beats — every join is identity risk and, if faded, paid
footage lost. 30s → **2×15**; 3×10 only for a three-beat brief; 45s → 3×15. `build_user_message`
(`storyboard.rs:421`) MUST hand the model each allowed model's ceiling; `clamp_shot` cannot cap a
free-form axis (`registry.rs:1867` is `ValueSpec::Number`, the 5–15 range is a constraint string
at `:1852`), so a plan-time `seconds > ceiling` check with a typed error is the enforcement; the
ceiling is a per-model constant citing `maximum: 15` until `capability` learns integer ranges.

**D-G:** `validate` rejects `< 3` shots (`storyboard.rs:308`), so 2×15 and S15's own single-shot
Seedance case (`slice.md:23`) are unreachable. Floor → `1..=8`; UI copy at `storyboard.rs:152`
follows.

**Resolution:** 480P $0.025/s, 768P $0.04/s, 1080P $0.08/s (`registry.rs:3014`); 30s = $1.20 vs
$2.40. 1080P is "latent refinement from a native 768P source" — upsampled 768P. **Default 768P**
for every draft/iteration (takes are where money goes); a "final export at 1080P" set-wide choice
MAY exist with a one-line note. One resolution per set (the frame dictates the canvas, §0.2).

## 6. Risk: the anchor frame is blurred or mid-action

Chosen for S15: the true last frame (`-sseof -0.05`, `ffmpeg.rs:822`) + the planner hold (§4). Why
not sharpest-of-N now: any frame but the last breaks the hard cut (shot 2 continues from `t*`
while shot 1 plays to EOF — a visible skip). The correct deferred form is *choose `t*`, then trim
shot 1 to `t*`* (`Clip::new(path, 0.0, t*)`); it needs a sharpness measure (Laplacian variance:
new dependency, or an ffmpeg filter unverified in the bundled build). Also deferred: trimming shot
2's opening hold (`Clip::new(path, k/fps, dur)`) — measure `k` on the first paid set by SSIM of
frame 0 vs frames 3–6, decide then.

## 7. Verifier checks for the join rule and arithmetic (no money)

Pure: `join_rule_is_per_join` (three cases of §2); `stitched_duration_joined` (30.0/29.5/29.5/
28.5); `a_chained_render_builds_concat_not_xfade` (graph has `concat=n=2`, no `xfade`; mixed has
one `xfade` at the summed-cut offset); `a_two_shot_plan_is_accepted` and 1-shot accepted, 0 and 9
rejected; `a_shot_over_the_ceiling_is_rejected` (20s on `minimax_h3_max`); §4's four validator
cases; `chained_body_disables_expansion_and_carries_the_seed`; `continuity_applied_survives_restart`.
Real sidecar (`--ignored`, lavfi): two `testsrc 4s` clips hard-cut → **8.0 ± 0.2s**, faded 1.0s →
**7.0 ± 0.2s** (both, so the join kind is load-bearing); **match-cut proof**: `last_frame(A)` → `F.png`,
`B = -loop 1 -t 1 F.png`, render `[A,B]` hard cut, frames at `4.0−1/fps` and `4.0` of the output
score ssim ≥ 0.98; **ghost demo** (informative): same pair faded 0.5s, midpoint frame ssim < 0.9
against both neighbours. Plus §0.5.

## Open questions (default recorded)

- **Q1 — Seed source:** `storyboard_seed` minted at generate, u32 from the id hash; retry uses
  `seed + attempt`. Default: yes; the seed is shown on the set item for reproducibility.
- **Q2 — Ceiling source:** per-model constant citing the schema, until the capability axis carries a
  max. Default: constant.
- **Q3 — Score at settle vs at render:** settle (§0.2) so the flag precedes the render spend. Default:
  settle, and the render re-reads `joins_json` rather than recomputing.
- **Q4 — Threshold calibration:** the two constants are re-read once against the first paid set's
  scores and changed in one commit with the numbers recorded. Default: 0.80/12/6 and 0.90/6/3.

## Weakest parts (start here)

1. The thresholds in §0.1 are reasoned from what an i2v first frame is, not measured on an H3 Max
   clip; §0.5's synth cases prove the machinery and the gates, not the numbers.
2. §0.2's "chain writes `prompt_expansion_mode`/`seed` explicitly and the constant defers" rests on
   `fal_schema.rs:269`; the implementer must find the seam that lets a `SubmitInput` carry those two
   keys (spec fields or a settings passthrough) and prove it with the bind test, keyless.
3. D-G changes an S13 validator bound; confirm no S13 test or UI copy depends on the old floor.
4. §2's ghost analysis is reasoned; the `--ignored` ghost demo and the first paid set are the checks.
