# Objective (DISCOVER)

## Outcome

Hickeyfield's existing generator becomes **trustworthy with real money** and gains its
**first editing tool surface**, without regressing the current build.

Concretely, at the end:

1. The trust/money bugs in the generator are fixed and proven by tests: paid provider output
   is no longer silently lost to a flat timeout; jobs respect the provider's concurrency limit
   instead of self-inflicting 429s; a completed-but-undownloaded job is never flipped to Failed
   on relaunch; the dead free tier (`z_image` / `Local`) shows an honest state instead of
   "$0.00, available" that fails at submit; and a packaged `.app` writes reachable logs and a
   crash report.
2. **One live end-to-end generation is proven green** against real fal.ai (submit → poll →
   download → reattach, with the estimated cost compared to the actual charge) — the thing
   `docs/FIRST-LIGHT.md` records as never yet achieved, which requires the video input-mode
   route-suffix resolver so routes stop 404-ing.
3. **One vertical editing tool surface ships**: a timeline UI that applies a trim to a video
   via the bundled ffmpeg sidecar (`compositor.rs` / `ffmpeg.rs`), which currently have no UI
   caller.
4. A **provider/tool abstraction seam** exists so a direct Runway API adapter (or a new edit
   op) can be added later without reworking core dispatch — the seam, not the adapter.

This is scoped to a small set of vertical slices. See **Constraints** for what "production
ready" and "the runway tools" are explicitly *not* taken to mean here.

## Users

- **Primary — the solo creator (bring-your-own-key).** Runs a local `.app` on macOS
  (Apple Silicon), stores a fal.ai key in the OS keychain, picks a use case, attaches media,
  presses Generate, and expects to (a) never be billed for output the app then throws away,
  and (b) trim/edit a clip they already have. This is the repo owner's own live-key workflow.
- **Contributor / maintainer.** Runs the CI gates locally (`cargo test`, `clippy`, `fmt`,
  `deny`, provenance, `pnpm build`/`test`) and needs each fix pinned by a named test so the
  regression baseline is defended per slice.
- **Out of band — the future Runway-adapter author.** Does not touch this contract's code, but
  the abstraction seam must let them add a direct adapter without editing the dispatch core.

## Constraints

**Stack (do not invent tooling outside this):**
- Tauri v2. Rust core `crates/hickeyfield-core/` (no Tauri dep), shell `src-tauri/`, React 19 +
  Vite UI in `ui/`. Workspace edition 2021, `rust-version = 1.85`, license AGPL-3.0-or-later.
- Build/test commands are exactly those the README documents:
  `cargo test --workspace`, `cargo fmt --all --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`, `cargo deny check`,
  `./scripts/lint-provenance.py`, `pnpm build` (`tsc --noEmit && vite build`), `pnpm test`
  (`vitest run`), `tauri build` / `scripts/build-macos.sh`, `scripts/verify-macos.sh`.
  ffmpeg-real tests run via `cargo test -p hickeyfield-core --ignored` with the
  `hickeyfield_FFMPEG` env var. Live provider smoke via
  `cargo run -p hickeyfield-core --example first_light` (reads `FAL_KEY`).
- **Never build with bare `cargo build`** for a runnable app — the README states it produces a
  blank window because the frontend is not bundled. Use `tauri build`.

**Baked-in scope decisions (agreed with the user):**
1. **"Runway tools" = build UI tool surfaces** (editing timeline, later motion/camera controls,
   video-edit) that route to models the app already reaches via fal. **Act-One and Gen-4 are
   Runway proprietary models and are OUT OF SCOPE to recreate.** Groundwork only: design the
   provider/tool abstraction as a clean seam so a direct Runway adapter can be added later —
   the seam, not the adapter.
2. **Reliability first, then tool surfaces.** Fix the trust/money bugs in the existing
   generator before building new surfaces.
3. **"Make sure this app still works" = no regressions.** The app must still build, launch, and
   run the existing generator after every slice. The protected baseline this session:
   `cargo test --workspace` = **718 passed / 9 ignored / 0 failed**; the release `.app` builds
   and launches.

**Live-key constraint:** the user will provide a real fal.ai key via in-app keychain or the
`FAL_KEY` env var — **never pasted into chat**. At least one live end-to-end generation must be
provable. No key is ever read across the webview bridge (existing house rule).

**Hard external blockers (record, do not assume solvable in-repo):**
- **ffmpeg macOS sidecar license.** Per `NOTICE` and `src-tauri/binaries/README.md`, the
  current macOS static build is sourced from osxexperts.net, labelled "for educational purposes
  only" — legally incompatible with **public redistribution**. In-scope only to document/flag
  as a release blocker; re-sourcing/self-building is out of scope unless a slice explicitly
  targets it. Also noted there: `LICENSES/GPL-2.0.txt` and `LICENSES/GPL-3.0.txt` do not exist
  yet (obligation for any public release).
- **Windows cannot be hand-tested** on the user's Apple-Silicon Mac. Windows verification is
  **CI-build-only**; any Windows success criterion is limited to "CI produces a build", not
  hand-run behaviour.

**Explicitly NOT in scope for this contract** (would be several separate projects):
- A distributable/notarized public release, App Store, or auto-update.
- Direct adapters for the 7 providers that currently have no client (only Fal + Higgsfield are
  real); the fal path is the live target.
- The full studio suite (audio, additional studios), a full library browser beyond what a slice
  needs, prompt-rewrite LLM wiring (`docs/HARNESS-WIRING.md` is a separate spec), or motion /
  camera-control / multi-op editing beyond the single trim surface.
- Recreating any Runway proprietary model.

## Success criteria (testable)

**A. No-regression gates (must stay green after every slice):**
- [ ] `cargo test --workspace` exits 0 with **0 failed** and **>= 718 passed** (baseline held).
- [ ] `cargo fmt --all --check` exits 0.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` exits 0.
- [ ] `cargo deny check` exits 0.
- [ ] `./scripts/lint-provenance.py` exits 0.
- [ ] `pnpm build` (in `ui/`) exits 0 (`tsc --noEmit` clean + `vite build` succeeds).
- [ ] `pnpm test` (in `ui/`, `vitest run`) exits 0.
- [ ] `tauri build` (or `scripts/build-macos.sh`) produces a `Hickeyfield.app` under
      `src-tauri/target/release/bundle/` that launches (window opens, not blank).
- [ ] The existing generator path still works: a `commands.rs::submit_job` test with a stubbed
      provider client still submits, and the use-case tabs still render (existing UI tests pass).

**B. Reliability / money-trust fixes (each pinned by a named test):**
- [ ] The runner uses the provider's `TimeoutPolicy` (via `provider.rs::features().timeout`
      / `engine.rs` `TimeoutPolicy::timeout(work)`), **not** the flat `DEFAULT_TIMEOUT` at
      `src-tauri/src/runner.rs:238`. Proven by a test asserting a long/4K `Billable` yields a
      larger budget than a short job (mirrors `engine.rs` timeout tests).
- [ ] On timeout, a job is **not** permanently marked Failed and dropped from `unfinished()`:
      it remains resumable so a late provider result can still be downloaded/reattached. Proven
      by a runner test.
- [ ] Concurrency is capped at the provider's `Concurrency::Provider(2)` (fal) from
      `provider.rs::features()`: queuing more than the limit does not spawn more than N
      simultaneous poll loops and drops no job. Proven by a runner test (replaces the uncapped
      per-job spawn at `runner.rs:81`).
- [ ] A Completed-but-undownloaded job is **not** flipped to Failed on relaunch when the
      provider URL has expired (the `runner.rs:250` re-poll path). Proven by a test that
      resumes such a job and asserts its terminal-success record survives.
- [ ] The dead free tier is honest: selecting `z_image` (`registry.rs:1334`) / `Local`
      (`provider.rs` `has_adapter` currently `true`, `app.rs:34` `client_for` returns `None`)
      no longer presents "$0.00 / available / Generate" that then fails with a misleading
      "add a key for Local". It renders an explicit unavailable state (following the existing
      `RouteDto` unavailable-reason pattern). Proven by a Rust test on the availability/DTO path
      and a UI test that the tile shows the unavailable state, not a Generate affordance.
- [ ] A packaged `.app` writes logs to a file in the app log/data dir (not stdout only, as at
      `src-tauri/src/lib.rs:16`) and installs a panic hook that records a crash report to that
      dir. Proven by a test asserting the file-based tracing layer + panic hook are installed,
      and the log path is discoverable (documented or surfaced by a "reveal logs" affordance).

**C. Live end-to-end proof (real fal key):**
- [ ] The video input-mode route-suffix resolver exists: a family-root route (e.g.
      `fal-ai/kling-video/...`) is resolved to the correct `/text-to-video` /`/image-to-video`
      suffix from attached media before submit, so it no longer 404s (the failure in
      `docs/FIRST-LIGHT.md`). Proven by a unit test over the resolver, and the 3 non-fal models
      (`kling3_0_turbo`, `minimax_hailuo`, `wan2_6`) are excluded or re-routed.
- [ ] One live generation is proven green with a real `FAL_KEY`: submit → poll → download →
      local file on disk → reattach, with `estimated_usd` recorded alongside the actual charge.
      Evidence: a green run of `cargo run -p hickeyfield-core --example first_light` (or the
      equivalent e2e harness) reaching a completed, downloaded result — updating
      `docs/FIRST-LIGHT.md` / `docs/PARITY.md §3.3` which currently state no generation has ever
      completed. (Human-run because it spends real money; criterion is the recorded green run.)

**D. First editing tool surface:**
- [ ] A timeline/editing UI component renders in the app and exposes a trim (in/out) control.
      Proven by a `vitest` component test that the component mounts and a trim action invokes
      the ffmpeg/trim command (mocked at the bridge).
- [ ] Applying a trim produces an output video file whose duration is shorter than the input,
      via the bundled ffmpeg sidecar through `compositor.rs` / `ffmpeg.rs`. Proven by a Rust
      `--ignored` ffmpeg test (`cargo test -p hickeyfield-core --ignored` with
      `hickeyfield_FFMPEG` set) asserting the output exists and its duration < input duration.
      (ffmpeg is already wired in `tauri.conf.json` `externalBin` and
      `capabilities/default.json` `shell:allow-execute`; this closes the missing UI caller.)

**E. Extensibility groundwork (seam only):**
- [ ] A provider/tool abstraction seam exists such that a new adapter (a stub standing in for a
      future Runway adapter) or a new edit op can be registered **without editing the core
      dispatch match**. Proven by a compiling test that registers a stub through the seam and
      dispatches to it. No Runway adapter, Act-One, or Gen-4 code is added.

## Open questions

Every question has a DEFAULT the build will assume if unanswered, so **none block PLAN**.
Two are **scope-changing** (flagged).

- **Q1 — SCOPE-CHANGING. Does "production ready" require a distributable public release**
  (notarized `.app` + `.dmg`, ffmpeg re-sourced under a redistributable license, GPL license
  texts added)? **DEFAULT: No.** This contract targets a locally-built, hand-runnable
  personal/dev build; the ffmpeg "educational use only" license and missing `LICENSES/GPL-*.txt`
  are documented as release blockers, **not resolved here**. Re-sourcing ffmpeg + notarization
  is a separate follow-on project.
- **Q2 — SCOPE-CHANGING. Which runway tool surfaces must ship in THIS contract** vs. later?
  **DEFAULT: exactly one** — the editing timeline with a video trim (through ffmpeg/compositor)
  as the proof of the tool-surface pattern. Motion/camera controls and additional edit ops are
  **deferred** to follow-on contracts; the abstraction seam (E) must accommodate them. Shipping
  "all of them" at once would be several projects and an unbounded contract.
- **Q3 — Which fal route is the canonical live smoke test, and is the user OK spending the
  (small) real USD?** DEFAULT: the cheapest available fal image route (e.g. a Seedream image
  model or `z-image` once it has a real path) to minimize cost; the user supplies `FAL_KEY` via
  keychain/env; one paid generation is acceptable.
- **Q4 — For the `z_image`/`Local` honesty fix, remove from the list or keep visible as
  unavailable?** DEFAULT: keep visible but render an explicit unavailable state (no "$0.00 /
  Generate"), matching the existing `RouteDto` unavailable-reason pattern.
- **Q5 — Where do logs and crash reports go in a packaged `.app`?** DEFAULT: the Tauri app
  log/data directory, as a rolling file plus a panic-hook crash file, with the path documented
  and/or surfaced by a "reveal logs" affordance.
- **Q6 — On timeout, is reconciling est-vs-actual billing required, or only preserving the
  resumable/reattachable record?** DEFAULT: preserve the resumable record and record actual
  charge when the result later completes; no separate billing-reconciliation subsystem.

---

# Extension R — Creative Machine (DISCOVER, appended 2026-09-19)

This section EXTENDS the contract above; it does not replace it. The original objective scopes
"the full studio suite (audio, additional studios) … prompt-rewrite LLM wiring … motion /
camera-control / multi-op editing beyond the single trim surface" **OUT** (§Constraints
"Explicitly NOT in scope", ~:89). Extension R deliberately re-opens a **bounded** part of that:
money-trust, a character library, finishing (captions/audio/looks/aspects), a generic ad studio,
and multi-ad batching — built on the already shipped S8–S18 foundations by reusing existing seams.
Everything not listed here stays OUT. All §A no-regression gates still apply to every slice, plus
the R-RULES below.

Verification tag on each criterion: **[test]** hermetic Rust/vitest (fake-provider seam, no
network), **[lint]** repo guard script, **[audit]** keyless (`env -u` fal audit / `prices::audit`),
**[owner-run]** human paid check (one per new billing shape; agents never spend).

## R0 — Mission + scope change
- **R0.1** Extension R serves three generic jobs on the existing engine: (a) a single piece of
  content, (b) a finished ad, (c) a multi-ad/variant batch — "with money you can trust". This is
  the outcome the R-criteria below make mechanically checkable. [coverage = sum of R1–R5]
- **R0.2** Newly IN scope (extends §:89): learning cost model v2, character library, finishing
  wiring, generic ad studio, multi-ad batch. Newly named-OUT and NOT contracted here: ffmpeg
  sidecar redistribution (stays the documented release blocker); recreating any proprietary studio
  or model; every M6 "LATER" item (R6). R0.2 frames R1–R6; no test asserts it directly.
- **R0.3 WORKHORSE (owner, 2026-09-19).** The everyday video model is `minimax_h3_max_turbo`
  (fal `minimax/h3-max-turbo/{text,image}-to-video`, ~$0.02/s at 768P, native audio + lipsync).
  Seedance 2.5 and other premium routes are NOT in everyday use. Therefore: the storyboard/ad planner
  defaults to the workhorse; mock data, fixtures, mockup boards and doc examples lead with it; money
  accuracy (R1) is proven on its billing shape FIRST (per-second tiers billed on OUTPUT seconds,
  6 s asked → 6.592 s billed, with a dated promotional-rate flag); no criterion may require a new
  premium-route purchase — per-token accuracy is validated against jobs already paid. [test] planner
  default + mock/fixture lead model; [owner-run] one workhorse clip per new billing shape.

## R1 — MONEY TRUST (folds cost-model-v2-spec; supersedes backlog S20 note)
- **R1.1** `PerToken` carries an explicit per-route tokens-per-second(resolution) table (not one
  derived constant); the recorded per-token video case (est **$1.95** vs billed **$6.18**, 3.2×)
  estimates within 20% of $6.18 from the owner's job parameters. [test] + [owner-run] live re-confirm.
- **R1.2** `PerImageTiered { tiers: [ImageTier{max_megapixels, quality, usd}] }` + `Billable.quality`
  exist; the GPT-image-class size×quality route is no longer `Unknown`, and its range's `expected`
  is within 10% of each seeded row ($0.027 / $0.054 / $0.17). [test]
- **R1.3** BILLED truth is distinct from re-applied formula: `ActualSource::{Billed, BalanceDelta}`
  added; `Probe`/`OutputCount` are relabelled "re-priced" in the UI and never shown as "actual
  billed". Usage-API attribution rule: exactly one job in the bucket window → `Billed`; two or more
  → endpoint×window aggregate and neither job claims the line. [test] on fixture JSON.
- **R1.4** Calibration store: table `price_observation(route_id, billable_json, est_usd, billed_usd,
  source, observed_at, feed_version)` via a new append-only `store.rs` migration (next after the
  current 11); estimator applies per-route `ratio = median(billed/est, N≤20)`. [test]
- **R1.5** Estimates are RANGES `{low, expected, high, confidence}`; with 0/2/5 observations the
  range widens/narrows per spec priors (PerSecond/PerImage ±10%, Tiered ±25%, PerToken ×0.8–3.5,
  Unknown = no number, never $0). [test]
- **R1.6** Drift alarm: when `|billed/est − 1| > AUDIT_TOLERANCE−1` the delta shows on the job card,
  in the picker for that route, and adds an `audit()` row. [test] + [audit]
- **R1.7** Dated free-web-sandbox hint from a static `sandbox_free.json` {route_id, free_per_day,
  as_of, source}: renders "Free in the provider web sandbox (N/day, as of DATE) — this app bills the
  API: $X", never alters `usd`, degrades silently when stale (>30 d). [test]
- **R1.8** Per-day and per-project USD budget caps enforced in the S18 `ProviderClient::submit` /
  `spend_gate.rs` path: refuses when `spent_today + estimate.high > cap` BEFORE any socket (same
  unroutable-proxy proof style as S18). [test]
- **R1.9** A ledger view reports per-project and per-character totals (kept vs discarded); per-
  character spend equals the sum of that character's jobs' actuals. [test]

## R2 — CHARACTERS (folds character-library-data-model)
- **R2.1** Character schema + invariants via a new `store.rs` migration: `character`,
  `character_asset`, `character_voice` (LoRA table deferred to R6). Identity is IMAGE-ANCHORED —
  one anchor asset, never a prompt/seed; `identity_spec` is reusable data on the character. [test]
- **R2.2** `new_still` picks an EDIT-capable route only (route accepts `image_urls` per
  `fal_schema.rs`/`capability.rs` AND preserves subject) and attaches the anchor + a KEEP block from
  `identity_spec`; a text-to-image-only route is refused with the "no seed → re-roll gives a new
  face" reason. [test]
- **R2.3** Lineage/anchor generations enforced: a `reference`/`clip` must chain via `parent_asset_id`
  to an anchor of the same generation (I2); promoting a new anchor bumps `anchor_generation` and
  retires the prior generation for NEW work but never deletes (I1, I4). [test]
- **R2.4** `real_consented` without `consent_ref` fails validation (I3); synthetic personas are
  stored `synthetic_disclosed`. [test]
- **R2.5** Voice card + speech rules stored as reusable data and applied by `new_clip`: with a line,
  the composed prompt places direction first, "say it once then hold" BEFORE the quote, the quoted
  line LAST; duration = clamp(model range, words×0.45 + 1 s); voice is not locked between runs so the
  card records a reference clip + a human `identity_check`/voice-match verdict. [test] + [owner-run]
  for the by-ear voice/lip-sync judgement.
- **R2.6** `reference_set_gaps` reports empty angle/expression/lighting/wardrobe cells against the
  target grid (≥12 assets, ≥3 angles, ≥3 expressions, ≥2 lighting, ≥2 wardrobe) for a seeded
  metadata-only fixture. [test]
- **R2.7** A storyboard shot can bind `character_id`; a bound shot routes through
  `new_clip`/`new_still` (anchor carried), replacing the free-text world-block subject, not a raw
  prompt. [test]
- **R2.8** Every character operation returns a `JobRequest` that goes through the EXISTING
  estimate → spend gate → submit path (no new spend path). [test]

## R3 — FINISHING (wire what already exists in `compositor.rs`)
- **R3.1** `compositor::Caption` is wired into `render.rs` and exposed in the UI; a render with
  captions burns them and a vitest mounts the caption control. [test]
- **R3.2** `compositor::AudioTrack` audio bed + ducking is wired into `render.rs` + UI. [test]
- **R3.3** Speech-to-text for word-timed captions: a documented decision (local sidecar vs priced
  provider route) with the licence checked either way; whichever is chosen produces word-timed
  caption cues. DEFAULT is Q-R1. [test] for cue output; licence recorded in DOCUMENT.
- **R3.4** Post looks ship as NAMED filter presets (e.g. silhouette/luma-crush, grade, grain,
  handheld, vignette) applied in post via the sidecar. [test]
- **R3.5** Logo / end-card overlay compositing available in `render.rs` + UI (asset supplied at
  runtime, never bundled). [test]
- **R3.6** Per-shot trim handles land as the existing S19 slice (in/out before stitch, total
  recomputed vs ±10%, continuity frame taken from the trimmed out-point). [test]
- **R3.7** Aspect variants 9:16, 4:5, 1:1, 16:9 via `compositor::render_variants`. [test]

## R4 — AD STUDIO (generic engine only)
- **R4.1** Ad TEMPLATES are DATA (not code): beats → shot classes → route class (talking / b-roll
  people:none / insert / end card), loaded and validated as a schema; a neutral sample template
  parses and expands. [test]
- **R4.2** brief → script → storyboard flows through the EXISTING planner (`storyboard.rs` +
  enhancer/`prompts/`); a brief produces an editable shot list bound to route classes. [test]
- **R4.3** Per-model prompt STYLE lives in the prompt corpus: styles verbose-cinematic / literal /
  knob / short selectable per route; newer still models get NO negative prompt and NO CFG (steer via
  positive texture cues). [test]
- **R4.4** A realism-cue toggle injects positive texture cues; a post-generation QC checklist overlay
  renders the pass/fail items. [test] (Rust for the injector, vitest for the overlay.)
- **R4.5** Per-shot suggestions: "free elsewhere" (from R1.7) and cheaper-route alternatives, each
  showing per-route cost. [test]

## R5 — MULTI-AD
- **R5.1** Variant matrix over hook × line × character × aspect × length × caption style expands to a
  concrete, de-duplicated variant set. [test]
- **R5.2** REUSE-FIRST planner: given a matrix delta, only changed shots are (re)generated; each
  variant reports TOTAL and MARGINAL cost. [test]
- **R5.3** Batch queue runs variants under the R1.8 budget cap (refuses/pauses at the cap). [test]
- **R5.4** Export bundle writes a manifest + deterministic naming for the batch. [test]
- **R5.5** Optional results import (CSV) tags winners on the matrix. [test]

## R6 — LATER (recorded, NOT contracted now)
- **R6.1** Character LoRA training via catalogue trainer routes (`character_lora` table + always-
  range-estimated, budget-capped). Not built in Extension R; recorded for a future contract.
- **R6.2** Headless CLI / MCP surface over owned code. Not built here.
- These carry no acceptance criteria in this contract by design.

## R-BRAND — brand-pack hard boundary
- **R-BRAND.1** Brand/client material loads at RUNTIME from a private folder OUTSIDE the repo
  (default `~/Documents/hickeyfield-brand-packs/<brand>/`, path configurable; see Q-R4). [test]
- **R-BRAND.2** A documented pack format exists: manifest + templates + characters + assets + claims
  rules; a schema test validates the format. [test]
- **R-BRAND.3** Only a NEUTRAL sample pack ships in-repo. [test]
- **R-BRAND.4** A lint/test FAILS if brand-pack content or named private material (any brand, client,
  person, character name, script line, hook, or claim from the owner's private notes) appears in the
  repo, tests, or fixtures. [lint] in the `lint-provenance.py` / `lint-spend.py` family.

## R-FIELD — field-lesson → criterion map (every master-prompt lesson is an AC)
| Field lesson (restated generically) | Criterion |
|---|---|
| Realism is a recipe (real environment + people, candid/phone-native, texture cues; a lone locked talking head fails) | R4.4 (+ QC checklist) |
| Newer still models take no negative prompt and no CFG; steer with positive texture language | R4.3 |
| Identity wording is reusable data stored on the character | R2.1 / R2.2 (`identity_spec`) |
| Bill on OUTPUT seconds, not requested (e.g. 6 s asked → 6.592 s delivered) | R1 (actual uses probed output duration) |
| Speech: direction first, "say once then hold" before the quote, quoted line last, ≤ ~6 s lines; voice not locked between runs → reference clip + human match | R2.5 |
| Same model can be free in the provider web sandbox and ~7× dearer on the API; warn before the click | R1.7 |
| Multi-model compare silently includes paid routes → always show per-route cost | R4.5 (+ R1.6 delta) |
| Looks/silhouettes are cheaper and more reliable in post than in the prompt | R3.4 |

## R-UX — mockup-first redesign, pass 2
- **R-UX.1** The owner-approved S11 direction (dark studio, vermilion accent, Bebas Neue / Space
  Grotesk / Inter / IBM Plex Mono, original assets, provenance lint) is the baseline; a mockup-first
  pass 2 covers the Ad Studio / Storyboard flow, Library sets, Characters, Variant matrix, and money
  surfaces. Mockups precede code, STAGED: undesigned screens first, polish second. [owner-run]
  approval of boards + [test] that provenance lint stays green.
- **R-UX.2** Test-locked selectors are preserved; every screen is previewable in the browser mock
  mode. [test] (vitest asserts locked selectors present; mock mode renders).

## R-RULES — non-negotiable (contract restatement)
- **R-RULES.1** Spend hygiene on EVERY agent command: `HICKEYFIELD_NO_KEYCHAIN=1` + dummy
  `hickeyfield_*_KEY=x` SET (never stripped); NEVER set `HICKEYFIELD_ALLOW_SPEND`; never run
  first_light; tests hermetic via fixtures + the S18 fake-provider seam. [lint] + [test]
- **R-RULES.2** No purchase of any kind without first stating what it is, the cost, and the
  alternatives, and getting the owner's explicit go; paid confirmations are owner-run only, one per
  new billing shape. [owner-run]
- **R-RULES.3** FORGE loop (DESIGN→IMPLEMENT→TEST→REVIEW→SECURITY→DOCUMENT→RELEASE), `gate.py`
  exit 0, distinct agents, never hand-edit a verdict, max two repair rounds then escalate; reuse
  before building; commit at RELEASE only; never bare `cargo build`. [gate]
- **R-RULES.4** Licence: repo is AGPL; copy nothing in from proprietary repos and nothing out; new
  deps go through `deny.toml`; the ffmpeg macOS sidecar stays a documented, non-redistributable
  release blocker (out of scope here). [audit] `cargo deny` + provenance lint.
- **R-RULES.5** Persona safety: real people require a `consent_ref` (R2.4); synthetic personas are
  disclosed-synthetic. [test]

## R — Open questions (each has a DEFAULT; scope-changing ones flagged)
- **Q-R1 — Speech-to-text: local sidecar or priced provider route?** DEFAULT: local sidecar for
  word-timed captions (no per-use spend, no new provider dependency), licence recorded in DOCUMENT;
  a priced route is a later option. Non-scope-changing.
- **Q-R2 — SCOPE-CHANGING. Billed-truth source, given fal exposes no per-request cost** (usage API
  has no request_id; per memory `fal-cost-apis`). DEFAULT: run on `Probe`/`OutputCount` +
  calibration priors and SAY SO; treat the ADMIN usage key as OPTIONAL, and `BalanceDelta` as a
  lower-trust fallback only when exactly one job is in flight. Making `Billed` truth a hard
  requirement adds an admin-key ingestion subsystem (scope-changing).
- **Q-R3 — SCOPE-CHANGING. Is the full R3 finishing set + R4/R5 studios in THIS contract, or only
  R1 (money) + R2 (characters) first?** DEFAULT: contract all of R1–R5 as the bounded scope
  extension, but PLAN sequences money + characters first per owner priority; R6 stays out.
  Confirming R4/R5 as later-only would shrink the contract.
- **Q-R4 — Brand-pack root path.** DEFAULT: `~/Documents/hickeyfield-brand-packs/<brand>/`,
  configurable in settings. Non-scope-changing.
- **Q-R5 — Budget-cap semantics.** DEFAULT: per-day AND per-project USD caps, evaluated against
  `spent_today + estimate.high` (billed where known, else expected), refusing before any socket in
  the S18 gate path. Non-scope-changing.
- **Q-R6 — Continuity priority ordering.** DEFAULT: the owner's standing "continuity is the money
  problem" priority is preserved — S19 trim handles and threshold calibration are NOT displaced by
  Extension R; the owner orders the queue at PLAN. Non-scope-changing.
