# Slice backlog (PLAN)

Vertical slices, thinnest shippable first. Each slice is end-to-end (a user can
see/use the result) and passes all nine phases before the next begins.

**Global regression gate — must stay green after EVERY slice (objective §A):**
`cargo test --workspace` (0 failed, >= 718 passed) · `cargo fmt --all --check` ·
`cargo clippy --workspace --all-targets -- -D warnings` · `cargo deny check` ·
`./scripts/lint-provenance.py` · `pnpm build` · `pnpm test` · `tauri build` produces a
`Hickeyfield.app` that launches (not blank) · existing `submit_job` + use-case-tab tests pass.

**Thinnest-valuable-first slice: #1 `job-engine-reliability`** (the real-money timeout
loss is the highest-value, lowest-surface-area fix, and every other slice depends on the
generator being trustworthy).

| # | Slice id | Delivers (one sentence) | Status |
|---|----------|-------------------------|--------|
| 1 | job-engine-reliability | A generation run no longer silently loses paid output: the runner honors each provider's `TimeoutPolicy` and concurrency cap, keeps a timed-out job resumable, and never flips a completed-but-undownloaded job to Failed on relaunch. | released |
| 2 | honest-free-tier | The dead `z_image`/`Local` free tier shows an explicit unavailable state instead of a "$0.00 / available / Generate" tile that fails at submit. | released |
| E | prompt-enhancer-wired | The prompt enhancer actually rewrites prompts through the filmmaking corpus using an available LLM backend (hosted OpenAI/Anthropic key, or auto-detected local Ollama), with an honest note when none is configured — instead of silently sending the raw prompt. (NEW: inserted after a user hit the dead enhancer.) | in-progress |
| 3 | reachable-logs | A packaged `.app` writes rolling file logs plus a panic-hook crash report to the app log dir, and the user can reveal that path from inside the app. | pending |
| 4 | live-fal-e2e | One real generation completes green against fal.ai (submit -> poll -> download -> reattach with estimated-vs-actual cost) because family-root video routes now resolve to the correct input-mode suffix instead of 404-ing. | pending |
| 5 | library-browser | The user can see and open their generated media in an in-app Library view backed by the existing `library.rs`. | pending |
| 6 | timeline-trim-export | The user trims a clip on a timeline UI and exports a shorter video file through the bundled ffmpeg sidecar (`compositor.rs`/`ffmpeg.rs`) — the first editing tool surface. | pending |
| 7 | motion-camera-controls | The user picks a camera-move preset in the video-generation UI and it is applied to the generation (existing `camera::slugs()` / `list_presets` surfaced). | pending |
| 8 | video-edit-usecase | The user selects the Edit Video use case, attaches a clip, and generation routes end-to-end to an edit-capable model. | pending |
| 9 | provider-tool-seam | A stub adapter/edit-op registers through a clean provider/tool abstraction seam and dispatches without editing the core dispatch match (groundwork for a future Runway adapter — the seam, not the adapter). | pending |
| 10 | release-readiness-doc | A release-readiness doc records the personal/local-build state and the deferred public-release gates (ffmpeg educational-use license, missing GPL texts, macOS notarization, updater, Windows CI-only). | pending |
| S8 | fal-catalogue-sync | fal's current frontier models (11, each verified keyless against fal's live schema) are in the picker, incl. Seedance 2.5's native 30s clip, plus a reusable fal→registry diff tool and a refreshed snapshot. | released |

Statuses: pending -> in-progress -> released (or blocked).

Start each slice with `.forge/new-slice.sh <slug>`.

---

## Per-slice detail (goal · acceptance criteria · dependencies)

### 1. job-engine-reliability  — position 1 (THINNEST-VALUABLE FIRST)
Goal: the existing generator survives timeouts, concurrency, and relaunch without losing
paid output. All fixes localized to `src-tauri/src/runner.rs` (endorsed merge of the three
cohesive runner fixes; each keeps its own acceptance criterion).
Acceptance criteria (objective §B1–B4):
- [ ] Runner uses `provider.rs::features().timeout` / `engine.rs` `TimeoutPolicy` instead of the flat `DEFAULT_TIMEOUT` (`runner.rs:238`); a test asserts a long/4K `Billable` yields a larger budget than a short job.
- [ ] On timeout a job stays in `unfinished()` / resumable (not permanently Failed); runner test proves a late provider result can still be downloaded/reattached.
- [ ] Concurrency capped at `Concurrency::Provider(2)` (fal); queuing more than N spawns no more than N simultaneous poll loops and drops no job (replaces uncapped per-job spawn at `runner.rs:81`); runner test.
- [ ] A Completed-but-undownloaded job is not flipped to Failed on the relaunch re-poll (`runner.rs:250`); test resumes such a job and asserts the terminal-success record survives.
Dependencies: none.

### 2. honest-free-tier — position 2
Goal: `z_image` (`registry.rs:1334`) / `Local` (`provider.rs` `has_adapter==true`, `app.rs:34`
`client_for` returns `None`, `clients.rs:413` empty) stops presenting a false free tier.
Acceptance criteria (objective §B5):
- [ ] Route/availability path renders an explicit unavailable state via the existing `RouteDto` unavailable-reason pattern — no "$0.00 / available / Generate" that then fails with "add a key for Local"; Rust test on the availability/DTO path.
- [ ] `vitest` UI test: the tile shows the unavailable state, not a Generate affordance.
Dependencies: none (independent of #1).

### 3. reachable-logs — position 3
Goal: replace stdout-only logging (`lib.rs:16`) so a packaged `.app` produces reachable
diagnostics.
Acceptance criteria (objective §B6):
- [ ] File-based rolling tracing layer writes to the Tauri app log/data dir + a panic hook records a crash file there; test asserts both are installed.
- [ ] The log path is discoverable — documented and/or surfaced by an in-app "reveal logs" command.
Dependencies: none.

### 4. live-fal-e2e — position 4
Goal: prove the one live end-to-end generation `docs/FIRST-LIGHT.md` records as never achieved.
Acceptance criteria (objective §C1–C2):
- [x] Route-suffix resolver: a family-root slug (e.g. `fal-ai/kling-video/...`) gains the correct input mode before submit — `/image-to-video` when a still is attached, otherwise the text suffix — so the call no longer 404s; resolver unit test. The three non-fal slugs (`kling3_0_turbo`, `minimax_hailuo`, `wan2_6`) are excluded or re-routed. *(Done: `media::resolve_endpoint` + `media::FAL_ROUTE_MODES`, landed `ab06e31` 2026-08-05 and extended to Higgsfield's mirror in `8d8ecfb` 2026-08-28; the three slugs are pinned in `media::FAL_MISSING_ROUTES` rather than excluded, so their non-fal routes still run. Re-probed 2026-09-16 by S8 — still unserved.)*
- [ ] One live generation proven green with a real `FAL_KEY` (keychain/env, never in chat): submit -> poll -> download -> file on disk -> reattach, `estimated_usd` recorded alongside actual charge; evidence is a green `cargo run -p hickeyfield-core --example first_light` (or equivalent e2e harness) and updated `docs/FIRST-LIGHT.md` / `docs/PARITY.md §3.3` (human-run, spends real money).
Dependencies: #1 (reliability baseline must hold before spending real money).

### 5. library-browser — position 5
Goal: surface existing `src-tauri/src/library.rs` (no UI caller today) so users can see/open
generated media. (In scope per authoritative FINAL scope decision — option (a).)
Acceptance criteria:
- [ ] Library UI lists generated media and lets the user open/reveal an item; `vitest` component test mounts the view and a library command is invoked (mocked at the bridge).
- [ ] Global regression gate green.
Dependencies: none hard; ordered after the reliability + live-e2e block per reliability-first.

### 6. timeline-trim-export — position 6 (objective's required tool surface §D)
Goal: the first editing tool surface — trim + export through the bundled ffmpeg sidecar
(`compositor.rs` / `ffmpeg.rs`, already wired in `tauri.conf.json` `externalBin` +
`capabilities/default.json`, no UI caller today). (Option (b).)
Acceptance criteria (objective §D1–D2):
- [ ] A timeline/editing UI renders and exposes a trim (in/out) control; `vitest` test mounts it and a trim action invokes the ffmpeg/trim command (mocked at the bridge).
- [ ] Applying a trim produces an output video whose duration < input via the ffmpeg sidecar; `cargo test -p hickeyfield-core --ignored` with `hickeyfield_FFMPEG` set asserts the output exists and duration < input.
Dependencies: #5 (browse/open a clip to trim).

### 7. motion-camera-controls — position 7
Goal: surface the 28 existing camera-move templates (`camera::slugs()` / `camera::get`,
`commands.rs:467 list_presets`) as controls that append the camera clause (`commands.rs:712`)
to a video generation. (Option (c).)
Acceptance criteria:
- [ ] Camera-move presets render as selectable controls in the video-generation UI and the chosen preset is applied to the submitted generation; `vitest` test + a Rust test over the preset-clause path.
- [ ] Global regression gate green.
Dependencies: #4 (video routing / route resolver).

### 8. video-edit-usecase — position 8
Goal: wire the Edit Video use case (`use_case.rs::UseCase::EditVideo`) end-to-end in the UI.
(Option (d).)
Acceptance criteria:
- [ ] Selecting Edit Video, attaching the required Video clip, and pressing Generate routes to an edit-capable model (e.g. `grok_edit_video` / a Kling edit route); `vitest` UI test + a Rust routing test assert the flow.
- [ ] Global regression gate green.
Dependencies: #4 (video route resolver).

### 9. provider-tool-seam — position 9 (objective §E — seam only)
Goal: a clean abstraction seam so a future Runway adapter or new edit op drops in without
reworking core dispatch. No adapter, Act-One, or Gen-4 code.
Acceptance criteria (objective §E1):
- [ ] A stub adapter/edit-op registers through the seam and dispatches to it without editing the core dispatch match; compiling test proves registration + dispatch.
- [ ] Global regression gate green.
Dependencies: #6, #7, #8 (the seam is validated against the real tool surfaces it must generalize).

### 10. release-readiness-doc — position 10 (final; documentation only)
Goal: record the personal/local-build state and the deferred public-release gates
(authoritative FINAL scope decision + objective Q1 / external blockers). No engineering.
Acceptance criteria:
- [ ] Doc records: ffmpeg macOS sidecar "educational use only" license (`NOTICE`, `src-tauri/binaries/README.md`), missing `LICENSES/GPL-2.0.txt` / `GPL-3.0.txt`, macOS notarization, auto-updater, and Windows-is-CI-build-only (no hand-test) — each flagged as a deferred release gate, not built here.
- [ ] Global regression gate green (doc-only change does not regress build/tests).
Dependencies: all prior slices (captures the end-state).

---

## Criterion -> slice coverage map

| Objective criterion | Slice(s) |
|---------------------|----------|
| §A No-regression gates (all 9) | Cross-cutting — enforced on EVERY slice (#1–#10) |
| §B1 TimeoutPolicy replaces flat DEFAULT_TIMEOUT | #1 |
| §B2 Timed-out job stays resumable (not dropped) | #1 |
| §B3 Concurrency capped at Provider(2) | #1 |
| §B4 Completed-undownloaded not flipped to Failed on relaunch | #1 |
| §B5 Honest z_image/Local unavailable state | #2 |
| §B6 File logs + panic hook + reveal-logs | #3 |
| §C1 Video route-suffix resolver + 3 non-fal excluded | #4 |
| §C2 One live fal generation proven green | #4 |
| §D1 Timeline UI + trim control (vitest) | #6 |
| §D2 Trim -> shorter output via ffmpeg (--ignored) | #6 |
| §E1 Provider/tool abstraction seam (stub dispatch) | #9 |
| Added scope: Library browser UI (option a) | #5 |
| Added scope: Motion/camera controls (option c) | #7 |
| Added scope: Video-edit use-case wiring (option d) | #8 |
| Added scope + Q1/external blockers: release-readiness doc | #10 |

Uncovered objective criteria: NONE — every §A–§E criterion maps to at least one slice.

## Finding: slice count vs FORGE's 3–8 preference

This backlog is **10 slices**, above FORGE's preferred 3–8. That is an honest reflection of
the authoritative scope: reliability-first (§B split across the trust bugs) + one live proof
(§C) + the objective's single required tool surface (§D) + the seam (§E) + the three EXTRA
tool surfaces the FINAL scope decision put in scope (library browser, motion/camera,
video-edit) + a release-readiness doc. I applied the one endorsed merge (the three cohesive
`runner.rs` fixes -> #1) and declined further merges because combining unrelated work
(e.g. logs + free-tier, or camera-controls + edit-video) would produce two-outcome, non-demoable
"task-list" slices. If this run must fit 3–8, the deferrable candidates are the added tool
surfaces beyond the objective's required trim (#5, #7, #8) and #9/#10 — but the authoritative
FINAL scope decision keeps them in, so they are included and the overage is flagged here rather
than hidden.

---
## Follow-ups discovered during S1 (triage, not yet slices)

Non-blocking findings from S1 REVIEW/SECURITY. None Critical/High, so none forced a
mandatory slice; tracked here for prioritization.

- **F1 (Medium, S1-introduced) — bound the resumable timeout state.** A timed-out job is
  now non-terminal with no attempt/age cap; a provider that never settles a request can
  keep a row `InProgress` forever, re-attached each launch, slowly parking blocking-pool
  threads. Add a re-poll/age cap → a terminal-but-recoverable "stalled" state, and/or bound
  `resume_all` fan-out. (runner.rs)
- **F2 (Low, pre-existing) — dependency advisory bump.** `cargo deny check` fails on
  RUSTSEC-2026-0258 (h2 0.4.15 → >=0.4.16) and RUSTSEC-2026-0285 (rustls 0.23.43 → >=0.23.45).
  Not S1's regression; `cargo update -p h2 -p rustls`, then re-run the gate.
- **F3 (Minor) — clear the stale timeout advisory on completion.** `apply_poll` never clears
  the NOTE advisory, so a job that later completes still shows "no result yet." Cosmetic.

- **F4 (raised to HIGH)** — `retry_job` command + `stalled` UI surfacing. Named mitigation for the
  age-cap residual below; the shipped `stalled` advisory currently points at a non-existent
  "reopen" affordance. F4 must reset `created_at` (not only `poll_cycles`) or the age branch
  re-stalls immediately.
- **F5 (correctness) — fix the age-cap horizon.** `age_cap = clamp(budget*5, 1h, 7d)` never
  reaches 7d (max budget 6h → 30h); effective hosted horizon is ~1–5h, which can abandon a
  still-live paid job. Make the horizon match a defensible intent (e.g. a fixed 7d age backstop
  with poll_cycles as the active bound) and correct the comment. Ship with or before F4.

- **F6 (enhancer robustness)** — adaptive local-enhancer timeout and/or a lighter system
  prompt for small models under heavy machine load. S5 live test: recommended 3-4B models
  hit the fixed 120s wall when the GPU was saturated (load avg 20-30); gemma3:1b did 32.7s.
- **F7 (enhancer UX)** — probe-on-select to detect an unloadable Ollama model (a model
  that lists in /api/tags but errors on /api/chat with e.g. "unknown model architecture").
  Today it passes name-based tiering and only fails at enhance-time via the honest fallback.

- **F8 (enhancer UX)** — invalidate the prompt preview when the raw prompt / model / settings
  change after previewing (App.tsx useEffect). Today the stale preview panel persists; Generate
  still sends the shown text (safe), but the stored `original` can diverge from what was shown.
  Cosmetic provenance, non-blocking.

- **DONE (S9-fal-schema-const-and-int-enums)** — fal-schema-const-and-int-enums (from S8 REVIEW, blocking-class bug).
  `crates/hickeyfield-core/src/fal_schema.rs:216` keeps only STRING enum members and ignores
  `const`, so (a) mixed enums like LTX 2.5's duration {6,8,10,"auto"} record as ["auto"] and every
  submit is refused, and (b) const-pinned fields (Veo 3.1 extend: duration "7s", resolution
  "720p") are sent free-form and 422. `examples/audit_fal.rs:133` check 4 only fires when both
  sides enumerate, so the audit cannot see either. Fix: read `const` as a one-member enum and
  stringify integer/number enum members; make audit check 4 use them; then re-probe ALL existing
  fal routes (the same bug may hide already-broken models); then re-land `ltx_2_5`,
  `ltx_2_5_animate` (duration enum 6/8/10, default 6) and `veo3_1_extend` (fixed 7s/720p — drop
  those fields from body + cost). Tests: parser fixtures for const + int-enum; audit finding on a
  const mismatch.
  Landed in S9: `parse` records a `const` as a one-member enum plus a `pinned` set and
  stringifies non-string enum members; `reconcile` writes a pin and reports it (never over a
  prompt or a media key); `audit_fal` gained a fifth check and dropped its duplicate fetcher;
  all 62 pre-existing endpoints re-probed with zero new findings; `ltx_2_5`, `ltx_2_5_animate`,
  `veo3_1_extend` and Seedance 2.5's `InputMode::Image` re-landed (CHECKED 62 -> 66).
- **S8 minors (non-blocking):** Wan 3.0 Prime "no published ceiling" comment is false;
  `nano_banana_2_edit` defaults to 0.5K but is priced at 1K; `fal_diff --dump --offline` restamps
  stale data with a new date; snapshot `thumbnailUrl` values (418 storage.googleapis.com,
  1 pbs.twimg.com) are parsed but never rendered — must stay unrendered (hotlink/provenance).

- **pin-aware-cost-estimate** (from S9 SECURITY/REVIEW, non-blocking): `estimated_usd` (commands.rs ~:1013) is priced from settings BEFORE `reconcile` may coerce a pinned cost-bearing field (e.g. a per-second route whose duration fal pins). Unreachable for today's landed set (Veo Extend's 7s is mirrored in the spec). Fix in the pricing slice: re-price after reconcile or from the settled body. **DONE (S10):** each completed job now records an `actual_usd` priced from the *settled* output (probed duration/resolution × the route rate), and a card advisory names the settled-vs-requested difference — so the settled-body price is now the authority the pre-`reconcile` estimate is reconciled against.

- **price-audit-tiered-prose** (from S10 REVIEW/coordinator, non-blocking): the hermetic audit
  (`prices::audit`) hard-compares only the ~11 fal routes whose prose reduces to a single rate; the
  ~46 conditional rows stay `Prose`. A tier-by-tier parser for the *regular* resolution tables
  (`$0.05/s at 480p, $0.06/s at 768p, …`; `$0.12/s for 720p or $0.17/s for 1080p`) would let the
  audit verify wan/ltx/minimax/grok-v1.5 `PerSecondTiered` literals against fal's own table. **Not
  done in S10:** a robust implementation exceeds the ~150-line budget the coordinator set, because
  the prose is irregular ("per second at" / "per second for" / "at Xp," / "/sec") and several rows
  carry multiple figures near one resolution token — grok edit/extend states `at 480p, every second
  costs $0.06 ($0.05 for output and $0.01 for input)` (three figures), and veo3.1 states
  `$0.20 without audio or $0.40 with audio for 720p or 1080p` (audio- and resolution-conditional).
  An `AuditRow::Mismatch` **fails the build**, so a parser that mis-pairs a figure with a resolution
  would break CI on a correct price — the exact "take the wrong number from a rate table" hazard the
  strict `fal_catalogue::parse_pricing` was written to avoid. Do it as its own slice with a resolved
  scope (which prose shapes are in/out) and fixture tests per shape; keep `fal_catalogue::parse_pricing`
  and its 193-rate contract untouched. Until then these rows are covered by the human-run AC6 live
  seam (`tests/live_fal_pricing.rs`).

- **dependency-advisories-h2-rustls** (from S9 SECURITY/REVIEW, non-blocking): `cargo deny check advisories` fails on main: RUSTSEC-2026-0258 (h2 0.4.15 → >=0.4.16), RUSTSEC-2026-0285 (rustls 0.23.43 → >=0.23.45), plus a stale RUSTSEC-2024-0429 ignore at deny.toml:77 (`advisory-not-detected`). `cargo update -p h2 -p rustls`, drop the stale ignore, re-run the gate.

- **S12 minors (from REVIEW, non-blocking):** orphaned `.model-row*` rules in `ui/src/components/rail.css`
  (~:481-524) and the unmounted `ResultsFeed.tsx` — delete; `WorkspaceTabs.tsx` nav tabs keep
  `aria-controls="generator-panel"` while Library has no panel; `--text-tertiary` legal notice ≈3.5:1 on
  the card (was 3.1:1) — consider #7a7a7a; `<1200px` meta rail as an Overlay sheet instead of a stacked column.

- **S13 minors (non-blocking):** a retried storyboard shot leaves the original `Failed` row in the group
  (two tiles at one shot_index — cosmetic; mirrors Rerun); `generate_storyboard` does not dedupe
  within-batch `shot_indices` (local webview only); a retry's plan is not compared with the group's
  stored plan; storyboard JSON hit-rate measured only on llama3.2:3b. Fold into S15 render or a hygiene slice.

- **S14 minors (non-blocking):** `examples/audit_fal.rs` ~:230-260 doc comment now sits above the wrong
  function (`pin_conflicts` undocumented); H3 Max / Turbo routes are PROSE in `prices::audit` (hand-verified
  2026-09-16; promotional rates — re-check when fal ends the promotion). Constant-fields item CLOSED.

- **S15 follow-ups:** per-join continuity score (SSIM + Y/U/V deltas via the sidecar; `joins_json`
  reserved; floor 0.80/12/6, ideal 0.90/6/3; badge + "regenerate shot N+1 from the same frame") and the
  planner world block / hold endings / validator variants / fill-to-ceiling → S16; keyframe-anchored mode
  (`end_image_url`) → S17; partially-chained fallback join renders as a hard cut until joins are per-join;
  minors: render.rs:104 comment vs code; ffmpeg::render blocking progress read can outlive the deadline;
  chain.rs `chainable_model` uses coarse `use_case::supports`; `a_failed_shot_stops_the_chain` phase-guard
  assertion vacuous (clip guard + hook gate still stop it); chain::advance TOCTOU → UNIQUE(storyboard_id,
  shot_index); `apply_overrides` allow-list when no schema cached.

- **S17 keyframe-anchored mode** (the 99% continuity path; draft in `.forge/slices/S17-parked/`): stills →
  i2v with actual last frame as start + next still as `end_image_url` (H3 Max supports both); tier-2
  thresholds 0.90/6/3; bounded auto-retry. **S18 shot trim handles**: per-shot in/out before stitching,
  total recomputed against the ±10% tolerance, continuity frame from the trimmed out-point.
- **S16 follow-ups:** threshold calibration on ONE real paid 2×15 set (human-run, owner's key; then adjust
  the named constants); world-block text editor + per-shot Regenerate button on the Storyboard card (UI
  polish; the action lives on the Library set item); world rules are opt-in on a non-empty world block.

- **S18 live-spend gate (NEXT, from the incident review):** `HICKEYFIELD_ALLOW_SPEND=1` required by every
  provider client / submit path (the packaged app sets it at startup); `first_light.rs` reads raw `FAL_KEY`
  and bypasses the vault — must refuse without the flag and lose the "copy your keychain key" docstring;
  repo-wide guard test that no credential is read outside vault.rs and no test strips `hickeyfield_*_KEY`;
  fake-provider-client seam so the live still/shot submit loops can be unit-tested offline (S17 M4).
- **S19 shot trim handles** (owner): per-shot in/out before stitching; total recomputed vs ±10%.
- **S17 minors:** validator's resolvable-route line is redundant with `use_case::supports` (keep or fold);
  `imageModelAvailable` default true; threshold calibration on one real paid 2×15 keyframe set (human-run).
- **PRICING (real data, 2026-09-16 17:35):** a Seedance 2.5 job estimated $1.95 settled at $6.18 actual
  (3.2× under-quote). The route is priced per 1k output tokens (`per_token(21.40)` note: "$0.0214 per 1k
  tokens at 480/720p, ~$0.0234 at 1080p"); the estimator's token count for the requested duration/
  resolution is far below what fal billed. Re-derive Seedance 2.5's token-per-second model from fal's
  prose and this settled job (and any others), add a hermetic test against the settled ratio, and show
  the estimate-vs-actual delta prominently in the picker for per-token routes. Owner-visible money bug.

- **S18 follow-ups (non-blocking):** move the spend gate into a non-overridable `ProviderClient::submit`
  default (delegating to `submit_inner`) or add a lint rule so a NEW provider cannot omit it; scope the
  entrypoint-enable test to the `run()` body (currently greps the whole file); hosted gate keys off
  `base_url.is_none()` — revisit if a configurable production base_url is ever added; `lint-spend.py`
  scans `.rs` only (add shell/CI/.env scanning); uploads are ungated (free today); daily budget cap.

### S20 follow-ups (2026-09-19)
- **OWNER 2026-09-19: Grok paid API is NEVER used** (MiniMax H3 Max Turbo is the only paid video route). Sweep Grok out of defaults/lead mocks alongside Seedance; keep the S20 `grok_video_v15` settle test as a regression fixture only; no Grok owner-run paid check, ever.
- Remove pre-existing Seedance entries (e.g. `seedance_2_0`) from `ui/src/mock.ts` and sweep defaults/samples for Seedance (owner "never" rule).
- Mock uses a flat 0.02/s for the tiered workhorse route (demo-only, not the real 480/768/1080 tiers).
- `parse_iso_date` accepts day 1–31 without per-month validation (panic-free; registry literals only).
- `promo_recheck` is not surfaced in the app UI (example printer only); live price re-verification belongs with S22.
- HANDOFF pointer to a settled Seedance job did not resolve in the app DB (moot after the "Seedance never" decision).
- Optional owner-run: one paid workhorse clip (~$0.13, 768p, 6 s) to live-confirm the output-seconds settle (agents never spend).

- **PAUSED by owner after S18 (2026-09-16).** Next when resumed: S19 shot trim handles, S20 per-token
  pricing accuracy (Seedance 2.5 3.2× under-quote on the owner's real job), continuity threshold
  calibration (human-run on one paid 2×15 set).

---

## Extension R queue (PLAN, 2026-09-19)

**OWNER AMENDMENT 2026-09-19 (S20 start): Seedance is NEVER used.** S20 drops its per-token half
(table + $1.95→$6.18 case); it is now workhorse output-seconds settle + promo flag + card delta, with
`grok_video_v15` (5 s → 5.04 s = $0.7056) as the second measured fixture. Objective R1.1 amended to match.
The S20 row/detail below is superseded where it mentions per-token; see `slices/S20-*/slice.md`.

**WORKHORSE (owner, 2026-09-19): `minimax_h3_max_turbo` (fal `minimax/h3-max-turbo/{text,image}-to-video`,
~$0.02/s at 768P, native audio + lipsync) is the everyday model.** Seedance 2.5 is NOT in use (too
expensive for the owner right now). Every default, sample, fixture, mock, mockup board, planner default,
money-accuracy target and owner-run paid check in this queue is H3 Max Turbo first; Seedance and other
premium routes are secondary and must never be the default or the first thing a slice optimises.

Merges objective §"Extension R — Creative Machine" (R0–R6, R-BRAND, R-FIELD, R-UX, R-RULES,
Q-R*) and the three companion inputs (`cost-model-v2-spec`, `character-library-data-model`,
`HARNESS-realism-extension`) into thin vertical slices. Numbering continues the live count
(18 released → next `new-slice.sh` id is **S19**). Owner reservations honored: **S19 = shot
trim handles**, **S20 = per-token pricing accuracy**; everything else numbered from **S21**.
All §A no-regression gates and the R-RULES (dummy keys + `HICKEYFIELD_NO_KEYCHAIN=1` SET,
never `HICKEYFIELD_ALLOW_SPEND`, never run `first_light`, hermetic via fixtures + the S18
fake-provider seam, commit at RELEASE only, max two repair rounds then escalate) apply to
EVERY slice below and are not repeated per row.

**Order = analyst proposal (owner decides at PLAN — MASTER-PROMPT: "order is a proposal").**
Money trust first ("money you can trust") → mockup (so later UI builds against an approved
design) → brand-pack boundary (R2/R4/R5 depend on it) → characters → finishing → ad studio →
multi-ad. Continuity priority preserved (Q-R6): **S19 trim handles and the human-run
continuity-threshold calibration are NOT displaced** and may be pulled forward at the owner's
word; they are listed in the finishing block by topic but carry their reserved ids.

### SCOPE-CHANGING decision points carried to the owner (defaults assumed; none block PLAN)
- **Q-R2 — billed-truth source.** DEFAULT (assumed by this plan): run on `Probe`/`OutputCount`
  + calibration priors and SAY SO; the ADMIN usage key is OPTIONAL; `BalanceDelta` is a
  lower-trust fallback only when exactly one job is in flight. Making `Billed` a hard
  requirement adds an admin-key ingestion subsystem — decided at S22 DESIGN.
- **Q-R3 — contract breadth.** DEFAULT: contract all of R1–R5 as the bounded extension, but
  sequence money (R1) + characters (R2) FIRST. Confirming R4/R5 (ad studio, multi-ad) as
  later-only would shrink the contract to S19–S34 and defer S35+ — an explicit owner call.
- **Q-R1 (non-scope-changing) — STT for word-timed captions:** DEFAULT local sidecar (no
  per-use spend), licence recorded in DOCUMENT — settled at S35 DESIGN.
- **Q-R4 (non-scope-changing) — brand-pack root:** DEFAULT `~/Documents/hickeyfield-brand-packs/<brand>/`,
  configurable — settled at S26.
- **Q-R5 (non-scope-changing) — budget-cap semantics:** DEFAULT per-day AND per-project USD
  caps vs `spent_today + estimate.high`, refuse before any socket — settled at S25.

### Ordered table

| # | Slice id | Slug | Delivers (one sentence) | Size | Owns (R-ids) |
|---|----------|------|-------------------------|------|--------------|
| S20 | workhorse-billing-accuracy | `workhorse-billing-accuracy` | **H3 Max Turbo first:** estimates bill on OUTPUT seconds (6 s asked → 6.592 s billed = $0.13 at 768P), the per-second tiers carry a dated "promotional rate" flag with a re-check, and the est-vs-billed delta shows on the card; THEN the per-token table (the recorded $1.95 → $6.18 Seedance case) is re-derived hermetically from that settled job — no new Seedance spend. | S | R1.1 |
| S21 | image-tier-pricing | `image-tier-pricing` | Size×quality image routes (gpt-image-2.5, nano-banana, seedream) quote a real tiered price instead of `Unknown`/flat, within 10% of each seeded row. | S | R1.2 |
| S22 | billed-truth | `billed-truth` | The app distinguishes true BILLED cost from its own re-priced formula, attributing a usage line to a job only when exactly one job settled in the bucket window. | M | R1.3 |
| S23 | calibration-and-drift | `calibration-and-drift` | A per-route calibration store learns billed/est ratios and raises a drift alarm (job card + picker + audit row) when a settled job exceeds tolerance. | M | R1.4, R1.6 |
| S24 | estimate-ranges-hint | `estimate-ranges-hint` | Every estimate becomes a `{low,expected,high,confidence}` range in core and picker, with a dated "free in the provider web sandbox" hint that never alters the billed number. | M | R1.5, R1.7 |
| S25 | budget-cap | `budget-cap` | Per-day and per-project USD caps refuse in the S18 gate path before any socket when `spent_today + estimate.high > cap`, with the gate moved into a non-overridable trait default. | M | R1.8 |
| S26 | brand-pack-boundary | `brand-pack-boundary` | Brand/client packs load at runtime from a private folder outside the repo against a documented pack schema; only a neutral sample ships, and a repo lint fails on any private/brand material. | M | R-BRAND.1–4 |
| S27 | creative-machine-mockup | `creative-machine-mockup` | One owner-approved design direction (extending S11) mocks Ad Studio/Storyboard, Library sets, Characters, the variant matrix, and all money surfaces via the `/design` skill — no source change. | M | R-UX.1 |
| S28 | storyboard-css-groundwork | `storyboard-css-groundwork` | The ~450 flat storyboard lines are extracted to a component sheet with the orphan rule deleted and mock data added, so every screen previews under `pnpm dev` with all test-locked selectors intact. | S | R-UX.2 |
| S29 | character-core | `character-core` | The owner can promote a generated still into a saved, image-anchored Character with lineage, anchor-generation, and consent invariants enforced (no generation yet). | M | R2.1, R2.3, R2.4 |
| S30 | character-still | `character-still` | "New still" of a saved character generates only through a subject-preserving edit route carrying the anchor + identity KEEP block (text-to-image refused), through the existing estimate→gate→submit path. | M | R2.2, R2.8 |
| S31 | character-clip | `character-clip` | "New clip" animates a character via i2v with the voice card's speech rules (direction first, "say once then hold" before the quoted line last) and line-sized duration, through the existing gate path. | M | R2.5 |
| S32 | character-ui | `character-ui` | The Library gains a Characters tab (anchor card, reference grid with empty-cell gaps, voice card) built against the approved mockup. | M | R2.6 |
| S33 | storyboard-character-binding | `storyboard-character-binding` | A storyboard shot can bind a character and then routes through `new_clip`/`new_still` carrying the anchor instead of a free-text world-block subject. | S | R2.7 |
| S34 | ledger-view | `ledger-view` | A ledger reports per-project and per-character spend totals (kept vs discarded), with per-character spend equal to the sum of that character's jobs' actuals. | S | R1.9 |
| S19 | shot-trim-handles | `shot-trim-handles` | Per-shot in/out trim before stitch, with total recomputed against ±10% and the continuity frame taken from the trimmed out-point. | S | R3.6 |
| S35 | captions-and-stt | `captions-and-stt` | Renders can burn word-timed captions: `compositor::Caption` is wired into `render.rs` + UI, fed by an STT decision (Q-R1 default local sidecar, licence recorded). | M | R3.1, R3.3 |
| S36 | audio-bed-ducking | `audio-bed-ducking` | An audio bed with ducking is wired from `compositor::AudioTrack` into `render.rs` and exposed in the UI. | S | R3.2 |
| S37 | post-looks-presets | `post-looks-presets` | Named post-look filter presets (silhouette/luma-crush, grade, grain, handheld, vignette) apply in post via the sidecar — cheaper and more reliable than in-prompt. | S | R3.4 |
| S38 | logo-endcard-overlay | `logo-endcard-overlay` | Logo / end-card overlay compositing is available in `render.rs` + UI, with the asset supplied at runtime and never bundled. | S | R3.5 |
| S39 | aspect-variants | `aspect-variants` | A finished render can emit 9:16, 4:5, 1:1, and 16:9 aspect variants via `compositor::render_variants`. | S | R3.7 |
| S40 | ad-templates-data | `ad-templates-data` | Ad templates are validated DATA (beats → shot classes → route class), and a neutral sample template parses and expands. | S | R4.1 |
| S41 | brief-to-storyboard | `brief-to-storyboard` | A brief flows brief → script → editable shot list bound to route classes through the existing planner/enhancer. | M | R4.2 |
| S42 | prompt-style-per-model | `prompt-style-per-model` | Per-route prompt STYLE (verbose-cinematic / literal / knob / short) is selectable, and newer still models get no negative prompt and no CFG. | S | R4.3 |
| S43 | realism-cue-and-qc | `realism-cue-and-qc` | A realism-cue toggle injects positive texture cues and a post-generation QC checklist overlay renders pass/fail items (the "realism is a recipe" lesson). | S | R4.4 |
| S44 | per-shot-cost-suggestions | `per-shot-cost-suggestions` | Each shot shows "free elsewhere" and cheaper-route alternatives, each with its per-route cost, so a compare never hides a paid route. | S | R4.5 |
| S45 | variant-matrix | `variant-matrix` | A variant matrix over hook × line × character × aspect × length × caption style expands to a concrete, de-duplicated variant set. | M | R5.1 |
| S46 | reuse-first-planner | `reuse-first-planner` | Given a matrix delta, only changed shots regenerate, and each variant reports TOTAL and MARGINAL cost. | M | R5.2 |
| S47 | batch-queue-under-cap | `batch-queue-under-cap` | A batch queue runs the variant set under the R1.8 budget cap, refusing/pausing at the cap. | M | R5.3 |
| S48 | export-bundle | `export-bundle` | The batch exports a bundle with a manifest and deterministic naming. | S | R5.4 |
| S49 | results-import-csv | `results-import-csv` | Optional CSV results import tags winners on the matrix. | S | R5.5 |
| S50 | ux-polish-pass | `ux-polish-pass` | Staged polish of Studio/picker/Library plus the recorded S12 minors, against the approved direction. | M | R-UX.1 (polish stage) |
| — | LATER (not queued) | — | M6: R6.1 character-lora (trainer routes, always-range-estimated + budget-capped) and R6.2 headless CLI/MCP surface — recorded, no acceptance criteria, not started in Extension R. | — | R6.1, R6.2 (deferred) |

### Per-slice detail (depends-on · seams reused · owner-run/paid steps)

- **S20 per-token-accuracy** — deps: none (extends S10 settled-actual path). Seams: `cost.rs`
  (`PerToken`, `Billable`), `prices.rs` fal prose, S10 settled-output re-price. Owns R1.1 and the
  R-FIELD lesson *"bill on OUTPUT seconds, not requested"* (est uses probed output tokens/seconds).
  Order inside the slice: H3 Max Turbo output-seconds billing FIRST (the owner's workhorse), per-token
  second. Owner-run: **R1.1 live re-confirm** — one paid **H3 Max Turbo** clip (~$0.13) to confirm the
  output-seconds estimate; the per-token table is validated against the ALREADY-PAID Seedance job in the
  app DB (est. $1.95 → actual $6.18) — **no new Seedance purchase** (agents never spend).
- **S21 image-tier-pricing** — deps: S20. Seams: `cost.rs` (add `PerImageTiered` + `Billable.quality`),
  `prices.rs` (`ImageSizeTier`), `registry.rs` `priced_routes`. Owns R1.2. Hermetic (seeded rows
  0.027/0.054/0.17). Owner-run: one paid gen per new shape at S22/human-run (R1.2 rows already
  measured 2026-09-18 in the cost-v2 spec).
  **OWNER NOTE 2026-09-19 — right-size the still to the target video (add as an S21 AC).** i2v output
  resolution is the MODEL's, not the input's, so a still larger than the target frame is wasted spend.
  Vertical 9:16 targets: 720p mockup = 720×1280 → the 608×1088 quality-high still ($0.027 measured) fits;
  1080p final = 1080×1920 (2.07 MP) ≈ the 1152×2048 hero (2.36 MP), so the size saving on finals is only
  ~12% — the real lever there is quality max → high. Draft rule: ONE 2K/max anchor per character ($0.17,
  once; it is also the identity anchor for edit routes), cheap stills downstream. AC: when a still feeds
  a video shot, the picker shows the target frame size and hints the cheapest tier that covers it, with
  the per-tier price; hint only, never changes the chosen size or `usd`. UNMEASURED (owner is testing
  himself — agents do not spend, do not seed guesses): price of ~1080×1920 at quality high; whether the
  9.8 realism score (measured at 2K + max) holds at quality high. Seed those rows only from owner data.
- **S22 billed-truth** — deps: S21. Seams: `cost.rs` `ActualSource` (add `Billed`,`BalanceDelta`),
  `runner.rs` actual write, `store.rs`. Owns R1.3. **Decision point Q-R2 resolved here** (admin
  usage key OPTIONAL). Owner-run: **R1.9/AC9-style one paid gen per new billing shape** to confirm
  billed truth (optional; only if the admin usage key is enabled).
- **S23 calibration-and-drift** — deps: S22. Seams: new append-only `store.rs` migration (next
  after the current 11) for `price_observation`, `prices::audit` row, picker + job-card surfaces.
  Owns R1.4, R1.6. Hermetic (fixture at billed/est = 3.2 raises the alarm).
- **S24 estimate-ranges-hint** — deps: S23. Seams: `cost.rs` `Estimate` → range, picker UI, static
  `sandbox_free.json`. Owns R1.5, R1.7 and the R-FIELD lesson *"same model free in the web sandbox,
  ~7× dearer on the API — warn before the click"*. Hermetic.
- **S25 budget-cap** — deps: S24. Seams: `spend_gate.rs` / `ProviderClient::submit` (the S18 gate),
  **carries the S18 follow-up "gate in a non-overridable trait default (delegating to submit_inner)"**.
  Owns R1.8. **Decision point Q-R5** (per-day AND per-project). Hermetic (unroutable-proxy proof,
  same style as S18). No owner-run spend.
- **S26 brand-pack-boundary** — deps: none (placed EARLY because R2/R4/R5 load packs). Seams: new
  runtime loader, `lint-provenance.py`/`lint-spend.py` family. Owns R-BRAND.1–4. **Decision Q-R4**
  (root path). Hermetic + [lint]. No brand/private material ever enters the repo.
- **S27 creative-machine-mockup** — deps: S25 (money surfaces designed against real semantics);
  builds on the approved S11 direction. Seams: `/design` skill, S11 `tokens.css`/vermilion system,
  provenance lint. Owns R-UX.1 (boards). **Owner-run approval gate on the boards; NO source change.**
- **S28 storyboard-css-groundwork** — deps: S27 (approved direction). Seams: extract
  `ui/src/base.css:662-1114` → `ui/src/components/storyboard.css`, delete orphan
  `.storyboard-regenerate-join`, add storyboard sets/joins/keyframes/renders/render-progress mock
  data to `ui/src/mock.ts`. Owns R-UX.2. Keeps ALL test-locked selectors; every screen previews
  under `pnpm dev`.
- **S29 character-core** — deps: S26 (pack boundary), S25 (gate path exists). Seams: new `store.rs`
  migration (`character`,`character_asset`,`character_voice`), store.rs patterns. Owns R2.1
  (image-anchored `identity_spec` → the R-FIELD *"identity wording is reusable data"* lesson),
  R2.3 (lineage/anchor generations, I1/I2/I4), R2.4 (consent I3; R-RULES.5). Hermetic (metadata-only
  neutral synthetic-persona fixture; NO face media and no private names in repo). *Note: the one borderline-horizontal slice — kept
  as its own step because the consent/lineage invariants are safety-critical and must land + be
  tested before any character submit loop; its visible deliverable is a promoted anchor in the list.*
- **S30 character-still** — deps: S29, **S18 fake-provider seam (must be present before this new
  submit loop)**, S25 (gate). Seams: `capability.rs`/`fal_schema.rs` (`image_urls` edit routes),
  estimate→spend_gate→submit. Owns R2.2 + R2.8 (character ops use the EXISTING spend path).
  Owner-run: none (hermetic via fake-provider seam).
- **S31 character-clip** — deps: S30, S18 fake-provider seam. Seams: i2v route, voice card rules,
  duration clamp. Owns R2.5 + the R-FIELD speech lesson. **Owner-run: by-ear voice / lip-sync
  `identity_check` verdict** (R2.5 human match — judgement, not spend).
- **S32 character-ui** — deps: S27 (design), S31. Seams: Library Characters tab, reference grid.
  Owns R2.6 (empty angle/expression/lighting/wardrobe cells rendered). Hermetic vitest.
- **S33 storyboard-character-binding** — deps: S31, S13 storyboard (released). Seams: `storyboard.rs`
  shot binding, `chain.rs`/`new_clip`. Owns R2.7. Hermetic.
- **S34 ledger-view** — deps: S29 (character_id on jobs), S22 (billed/actual). Seams: `store.rs`
  job actuals, ledger UI. Owns R1.9. Hermetic. *(Placed after characters because per-character
  totals require the character schema — the only R1 slice sequenced after R2.)*
- **S19 shot-trim-handles** — deps: S15/S16/S17 chaining (released). Seams: `compositor.rs` trim,
  `render.rs` stitch, continuity-frame extract. Owns R3.6. **Owner-prioritized (continuity =
  money-in-the-bank); may be pulled forward.** Hermetic + `--ignored` ffmpeg test.
- **S35 captions-and-stt** — deps: S28 (UI groundwork). Seams: `compositor::Caption` → `render.rs`
  + UI; STT sidecar (Q-R1). Owns R3.1, R3.3. Licence recorded in DOCUMENT. Hermetic (cue output).
- **S36 audio-bed-ducking** — deps: S35 (render.rs finishing surface). Seams: `compositor::AudioTrack`.
  Owns R3.2. Hermetic.
- **S37 post-looks-presets** — deps: S36. Seams: sidecar filter presets. Owns R3.4 + the R-FIELD
  *"looks/silhouettes are cheaper and more reliable in post"* lesson. Hermetic + `--ignored`.
- **S38 logo-endcard-overlay** — deps: S37. Seams: `render.rs` overlay compositing. Owns R3.5.
  Asset supplied at runtime, never bundled. Hermetic.
- **S39 aspect-variants** — deps: S37. Seams: `compositor::render_variants`. Owns R3.7. Hermetic.
- **S40 ad-templates-data** — deps: S26 (pack format). Seams: template schema loader/validator,
  neutral sample template. Owns R4.1. Hermetic.
- **S41 brief-to-storyboard** — deps: S40, S13. Seams: `storyboard.rs` planner + enhancer/`prompts/`.
  Owns R4.2. Hermetic.
- **S42 prompt-style-per-model** — deps: S41. Seams: prompt corpus. Owns R4.3 + the R-FIELD
  *"newer still models take no negative prompt and no CFG"* lesson. Hermetic.
- **S43 realism-cue-and-qc** — deps: S41. Seams: cue injector (Rust) + QC overlay (vitest). Owns
  R4.4 + the R-FIELD *"realism is a recipe"* lesson. Hermetic.
- **S44 per-shot-cost-suggestions** — deps: S24 (free-tier hint + ranges), S21/S20 (per-route cost),
  S41. Seams: picker/compare per-route cost. Owns R4.5 + the R-FIELD *"multi-model compare always
  shows per-route cost"* lesson (with the R1.6 drift delta). Hermetic.
- **S45 variant-matrix** — deps: S33 (character axis), S39 (aspect axis), S41. Owns R5.1. Hermetic.
- **S46 reuse-first-planner** — deps: S45, S23 (marginal cost from calibrated estimates). Owns R5.2.
  Hermetic.
- **S47 batch-queue-under-cap** — deps: S46, S25 (cap), **S18 fake-provider seam (new submit loop)**.
  Owns R5.3. Hermetic (refuses/pauses at the cap; unroutable-proxy proof). No owner-run spend.
- **S48 export-bundle** — deps: S47. Owns R5.4. Hermetic (manifest + deterministic naming).
- **S49 results-import-csv** — deps: S45. Owns R5.5. Hermetic.
- **S50 ux-polish-pass** — deps: S27 + the shipped UI slices (S28/S32/S44). Seams: Studio/picker/Library
  sheets + recorded S12 minors. Owns R-UX.1 polish stage. [owner-run] board approval of the polish
  pass; provenance lint stays green.
- **LATER (R6, not queued):** R6.1 character-lora (`character_lora` table, trainer routes, always
  range-estimated + budget-capped) and R6.2 headless CLI/MCP surface — recorded per R0.2/R6, no
  acceptance criteria in this contract, not started.

### Extension-R criterion → slice coverage map

| Extension-R criterion | Slice(s) |
|-----------------------|----------|
| R0.1 (three generic jobs "with money you can trust") | Sum of R1–R5 = S19–S50 (framing; no direct test) |
| R0.2 (scope in/out framing) | Framing only; no test (bounds S19–S50; R6 = LATER) |
| R1.1 per-token table (Seedance ≤20%) | S20 |
| R1.2 PerImageTiered + Billable.quality | S21 |
| R1.3 ActualSource::{Billed,BalanceDelta} + attribution | S22 |
| R1.4 calibration store + applied ratio | S23 |
| R1.5 estimate RANGES | S24 |
| R1.6 drift alarm (card + picker + audit) | S23 |
| R1.7 dated free-web-sandbox hint | S24 |
| R1.8 per-day/per-project budget cap in gate | S25 |
| R1.9 per-project / per-character ledger | S34 |
| R2.1 character schema + image-anchored identity_spec | S29 |
| R2.2 new_still (edit-route-only, KEEP block) | S30 |
| R2.3 lineage / anchor generations (I1,I2,I4) | S29 |
| R2.4 consent (I3) + synthetic-disclosed | S29 |
| R2.5 voice card + speech rules (new_clip) | S31 |
| R2.6 reference_set_gaps grid | S32 |
| R2.7 storyboard shot ↔ character binding | S33 |
| R2.8 character ops through existing estimate→gate→submit | S30, S31 |
| R3.1 captions wired into render.rs + UI | S35 |
| R3.2 audio bed + ducking | S36 |
| R3.3 STT word-timed caption cues | S35 |
| R3.4 named post-look presets | S37 |
| R3.5 logo/end-card overlay | S38 |
| R3.6 per-shot trim handles | S19 |
| R3.7 aspect variants | S39 |
| R4.1 ad templates as data | S40 |
| R4.2 brief → script → storyboard | S41 |
| R4.3 per-model prompt style (no neg/CFG on newer still) | S42 |
| R4.4 realism-cue toggle + QC checklist | S43 |
| R4.5 per-shot free/cheaper-route + per-route cost | S44 |
| R5.1 variant matrix expansion | S45 |
| R5.2 reuse-first planner (total + marginal cost) | S46 |
| R5.3 batch queue under budget cap | S47 |
| R5.4 export bundle (manifest + naming) | S48 |
| R5.5 results import (CSV) tags winners | S49 |
| R6.1 character LoRA / R6.2 CLI-MCP | LATER (not queued; no ACs by design) |
| R-BRAND.1–4 (runtime pack, schema, neutral sample, lint) | S26 |
| R-UX.1 mockup boards (+ polish stage) | S27 (+ S50 polish) |
| R-UX.2 test-locked selectors + browser mock mode | S28 |
| R-RULES.1–5 (spend hygiene, purchase gate, FORGE loop, licence, persona) | Cross-cutting — enforced on EVERY slice |

### R-FIELD lesson → owning slice (each lesson owned by exactly ONE slice)

| Field lesson | Criterion | Owning slice |
|--------------|-----------|--------------|
| Realism is a recipe (real env + people, candid, texture cues) | R4.4 | S43 |
| Newer still models take no negative prompt / no CFG | R4.3 | S42 |
| Identity wording is reusable data on the character | R2.1 | S29 |
| Bill on OUTPUT seconds, not requested | R1 (probed output) | S20 |
| Speech: direction first, "say once then hold", quoted line last; voice not locked → reference clip + human match | R2.5 | S31 |
| Same model free in web sandbox, ~7× dearer on the API — warn first | R1.7 | S24 |
| Multi-model compare must always show per-route cost | R4.5 (+ R1.6) | S44 |
| Looks/silhouettes cheaper + more reliable in post than in prompt | R3.4 | S37 |

### Uncovered Extension-R criterion

NONE. Every testable Extension-R criterion (R1.1–R5.5, R-BRAND.1–4, R-UX.1/.2) maps to exactly
one owning slice; R0.1/R0.2 are framing (coverage = sum of R1–R5), R-RULES.1–5 are cross-cutting,
and R6.1/R6.2 are recorded LATER with no acceptance criteria by design.

### Finding: Extension R is ~5 milestones ≈ several forge runs (exceeds the 3–8 preference)

Extension R queues **32 slices (S19–S50)**, far above FORGE's 3–8 preference. That is honest:
the contract spans five milestones (M1 money, M2 characters, M3 finishing, M4 ad studio, M5
multi-ad) plus the brand-pack boundary and a two-stage UX redesign — effectively five forge
runs' worth. Per the MASTER-PROMPT ("order is a proposal; I decide") the full ordered queue is
presented for the owner to pick from ONE slice at a time; this is a planning artifact, not a
mega-backlog to run in one pass. Recommended cadence: run **M1 (S20–S25) first for money you
can trust**, then the mockup + brand pack (S26–S28), then characters (S29–S34); confirm Q-R3
before committing S35+ (finishing/ad-studio/multi-ad), which Q-R3's shrink option would defer.

### Slices I had to split (an L must be split)

- **Money "billed-truth + calibration + drift" (cost-v2 spec's single S22) → S22 + S23.**
  ActualSource/attribution (R1.3) is a distinct, independently-testable outcome from the
  `price_observation` store + drift alarm (R1.4/R1.6); together they were L.
- **Character generation → S30 character-still + S31 character-clip.** `new_still` (edit-route)
  and `new_clip` (i2v + speech) are two separate submit loops and two separate demoable outcomes;
  combined they were L and would hide the fake-provider-seam dependency behind one row.
- **Multi-ad "variant matrix + reuse-first planner" → S45 + S46.** Matrix expansion (R5.1) and
  the reuse-first marginal-cost planner (R5.2) are separable outcomes; combined they were L.
- **UX redesign → S27 mockup (owner-approval gate, no code) + S28 css groundwork (code) + S50
  polish (staged later).** Per the owner's explicit R-UX-1/R-UX-2/R-UX-3 staging; a single UI
  slice would mix an approval gate, a refactor, and polish into one non-atomic step.
