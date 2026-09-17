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
