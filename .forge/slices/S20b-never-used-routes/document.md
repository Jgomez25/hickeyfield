# S20b-never-used-routes — DOCUMENT

## What shipped (user-facing)
- The app opens on the everyday workhorse (MiniMax H3 Max Turbo, $0.02/s at 768p), keeps it
  across a use-case tab change wherever it fits, and defaults a new storyboard shot to it —
  instead of whatever model sorted first. The launch set that seeds these defaults swaps the
  old Seedance family out for the workhorse.
- Every Seedance and every Grok route (video AND image) is **hidden from every picker by
  default** — not deleted. A Settings control **"Show never-used routes (Seedance, Grok)"**
  (off by default) re-lists them.
- The refusal is **server-side**: with the toggle off, a hidden route is turned away in the
  core before any provider is contacted, with a message naming the toggle — covering re-runs,
  storyboard shots, keyframe regen and chained retries through the single `submit_one` funnel.
- History for a job already made on a hidden route still shows its real display name and cost;
  nothing is silently re-pointed.
- Demo data, samples and provider blurbs no longer lead with — or advertise — Seedance or Grok.

## Docs changed (file + heading)
- `CHANGELOG.md` › `[Unreleased]` › `### Added` — new lead entry: never-used routes hidden by
  default, the "Show never-used routes (Seedance, Grok)" toggle (off), the server-side refusal,
  migration 12, and the note that this is separate from and additional to the spend gate. Test
  counts 971→979 Rust, 313→321 UI.
- `CHANGELOG.md` › `[Unreleased]` › `### Changed` — new lead entry: startup / tab-fallback /
  storyboard-shot defaults are the workhorse; the launch set swaps the Seedance family for the
  workhorse; mocks/samples/blurbs no longer advertise Seedance or Grok.
- `README.md` › `### Never-used routes` (new subsection under `### Spending and credentials`) —
  what is hidden, why (owner cost rule), how to reveal them, that the refusal is server-side in
  the core, and that it is separate from and additional to the S18 spend gate.
- `docs/PARITY.md` › `## 4. Model roster` — a **dated note (2026-09-19)** recording the hide +
  workhorse-led launch set, rather than rewriting the 2026-08-04 measured roster/pricing.
- `.forge/backlog.md` — the two Seedance/Grok sweep lines under `### S20 follow-ups
  (2026-09-19)` marked DONE in S20b; a new `### S20b follow-ups (2026-09-19)` block appended.

## Owner decisions recorded this slice
- **Depth = Demote + hide** (owner pick 2026-09-19, "do option 1, sweep seedance and grok
  out"): Seedance and Grok are hidden behind a persisted, default-OFF toggle and refused
  server-side while hidden — never deleted (registry, prices, capability tables and S20's
  `grok_video_v15` settle test all stay).
- Design defaults O1–O4, as built:
  - **O1 — predicate shape.** Prefix-based `is_never_used` (`starts_with "seedance" | "grok"`)
    in `registry.rs`, so a future `seedance_*`/`grok_*` id is auto-hidden; an exhaustive
    set-equality test pins it to exactly the 11 registry ids today (Seedream and the workhorse
    on the KEPT side).
  - **O2 — toggle home.** The model **settings rail** (`SettingsRail.tsx`), adjacent to the
    picker so flipping re-lists immediately; an additive `.never-used-toggle` touches no
    test-locked selector.
  - **O3 — mock replacement model.** `veo_3_1_fast` for the removed `seedance_2_0` preset
    entries; the removed roster MODEL block became `wan_2_6` (the design's `veo_3_1_fast`
    default was already present in the mock roster, so reusing it would have duplicated an id —
    the run's finding, recorded in implement.md).
  - **O4 — refusal wording.** The full message: names Seedance/Grok, states the billing reason,
    and quotes the exact toggle label.

## Repair history (two lines)
- **Round 1** — added a real v11→v12 upgrade-with-data proof (`store.rs`, full-row equality on a
  genuine v11 file), routed `mock.ts` through the `WORKHORSE_MODEL_ID` constant, and reworked the
  A1 cross-tab UI test to actually click tabs.
- **Round 2** — after TEST caught the round-1 A1 test as a fixture-ordering tautology (stripping
  all workhorse-preference from `App.tsx` still left it green), the A1 test was rebuilt as a
  falsifiable file (`App.workhorse.test.tsx`, workhorse never at index 0) proven RED under
  mutation; no production file changed.

## Follow-ups
- `ui/src/components/Storyboard.tsx` ~:216–219 — the workhorse-first new-shot branch is
  unreachable via the DOM today (the "+ Add shot" control renders only when `shots.length > 0`,
  so `last?.modelId` always short-circuits it); it is a defensive default with no direct test.
- `get_bool_pref` treats any non-`"1"` value as false (the safe/fail-closed direction) rather
  than honouring the passed `default` for a corrupt row; consider honouring `default` for general
  reuse.
- Resume-drain of an already-submitted queued job does not re-check the toggle — no new spend
  (any re-run goes back through `submit_one`, which does check).
- The A2 launch-set test does not independently catch a broken `is_never_used` predicate (its
  `!is_never_used(id)` clause is trivially satisfied when the predicate returns false); the A3
  exhaustive and default-hidden tests do catch it.
- The v11→v12 upgrade test's temp-dir cleanup is not RAII-guarded, so a panicking assertion
  leaks the PID-scoped dir until the next same-PID run overwrites it (idempotent-safe).
- Process note: verifiers must **never** use `git checkout`/`restore` in a tree that holds
  uncommitted slice work (a tester did once during a mutation revert; the tree was recovered
  byte-identical from `pre.diff`, but the hazard is real — restore by `cp` from a scratch copy).
- Slice-id note: this slice is `S20b`, so backlog **S21** (image-tier-pricing) keeps its id; the
  NEXT `new-slice.sh` scaffold will come out as `S22` and must be renamed to `S21`.

## Evidence (verifier writes verdicts)
TEST / REVIEW / SECURITY verdicts and the two repair rounds live in this directory. Per
implement.md and the TEST re-verification: `cargo test --workspace` 979 passed / 0 failed / 20
ignored; fmt · clippy · provenance · lint-spend clean; keyless `audit_fal` (70 endpoints)
unchanged; `cd ui && pnpm test` 321 passed and `pnpm build` ok. Nothing spent
(`HICKEYFIELD_ALLOW_SPEND` never set; `first_light` never run).
