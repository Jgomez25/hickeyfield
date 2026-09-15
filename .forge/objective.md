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
