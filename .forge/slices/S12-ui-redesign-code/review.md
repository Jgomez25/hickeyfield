# S12-ui-redesign-code — REVIEW (evidence)

Independent, read-only production-standards review of the slice diff (`git diff main...HEAD`,
HEAD 4f87976). I changed no source. The gate was re-run first-hand (Node 24), results below.

## Gate — re-run by me, not taken on trust

- `cd ui && pnpm test` → **272 passed / 0 failed**, 22 files (baseline floor 245). Green.
- `cd ui && pnpm build` → `tsc` clean + `vite build` ok, 75 modules, `index-*.css` 57.49 kB. Green.
- `./scripts/lint-provenance.py` → **PASS** (no third-party media host; no shipped string matches
  the 80 015-shingle reference corpus).
- `cargo test --workspace` (dummy `hickeyfield_*_KEY=x`) → 690 + 3 + 2 + 125 = **820 passed / 0
  failed** (floor 820, no Rust touched). Green.

The implementer's pasted counts (272 / build / lint / 820) are truthful.

## What I checked and found clean

- **AC1 fixes-first, non-vacuously.** ConfirmDelete is a top-level sibling of the overlays
  (`App.tsx:806-813`), no longer nested in the titlebar `<button>`; `App.test.tsx` opens the dialog
  (real `[role="dialog"]` with buttons) and asserts `button button` count is 0 — it bites.
  `ModelBrowser.tsx`/`browser.css` are deleted (confirmed absent). `--color-vermilion` is gone from
  every component sheet (`meta.css:274` now `var(--accent)`); no `.btn-quiet` in any TSX
  (`ResultsFeed.tsx` uses `btn-ghost btn-sm`). `fixes.test.ts` statically guards all four.
- **AC2 tokens.** `--text-secondary #828282→#9a9a9a`, `--text-tertiary #626262→#6e6e6e`,
  `--rail-settings 410→400`, new `--rail-nav`, `--radius-card/panel`, `--tile-thumb-ratio`,
  `--bloom-strong` all declared (`tokens.css`). The last raw brand literal
  (`rgba(255,74,28,0.42)` on the Generate hover) is now `var(--bloom-strong)` (`rail.css:728`).
  `tokens.test.ts` scans every `components/*.css` for the covered hex/rgb set incl. both old and new
  text values and passes — I re-ran it. **Every `var(--…)` referenced by the new/changed rules
  resolves** — I diffed the full referenced-token set against `tokens.css` declarations: no dangling
  custom property was introduced (i.e. the exact class of bug AC1 was cleaning up did not recur).
- **AC3 shell.** `WorkspaceTabs variant="nav"` is the *same* tablist (roles, `data-tab`, roving
  tabindex, `onChange`) restyled — Up/Down added to the roving handler for the vertical axis
  (`WorkspaceTabs.tsx:86-103`). The per-view grid lives in `base.css` (`.workspace[data-view=…]`),
  not `app.css` — the implementer's stated design-fact correction is accurate (`app.css` is only the
  import manifest). Meta rail mounts on Library only, stage on Studio only — pinned by `App.test.tsx`.
- **CSS cascade (the load-bearing part).** The picker's grid/tile rules are all scoped
  `.model-list .model-item…` (`picker.css:224+`); the composer tile carries `.model-item-composer`
  and lives in the rail, **outside** `.model-list`, so it cannot inherit the 5-col grid. rail.css
  loads before picker.css, but because the picker rules are descendant-scoped (specificity 0,2,0) and
  never match the composer subtree, there is no tie to flip. Verified no other sheet defines a bare
  `.model-item` layout rule. Correct.
- **AC4/AC5/AC6/AC7 behaviour is tested, not asserted.** `Stage.test.tsx` (featured large; `est. →
  actual` caption; running mm:ss timer; recents keep `data-job-anchor`; Extend hidden when
  `onExtend` null or no saved file; empty state), `SettingsRail.test.tsx` (sticky `.generate-button`;
  `.model-item.model-item-composer` keeping `.model-item-name`/`.chip-route`; ≤3 camera tiles + more;
  legal notice), `Library.test.tsx` (tiles + cost/unpriced + `data-job-anchor`; click selects + opens
  lightbox; Failed filter), `ModelPicker.test.tsx` (+1, additive: thumbnail is original
  `data:image/svg+xml` never a snapshot; `data-tier` from `isLaunch`), `cost.test.ts` (+4
  `stageCostCaption` branches). All assert observable behaviour.
- **No weakened tests.** `git diff main -- '*.test.*'` shows **zero removed assertion lines** — every
  test change is additive. Snapshot `thumbnailUrl`s are never rendered: tiles use `gradientDataUri`
  or `posterFor` (the job's own local render).
- **Contrast (c).** New `--text-secondary #9a9a9a` computes ≈ **6.6:1** on `#131416` and ≈ **6.3:1**
  on `#1c1e21` — comfortably AA, and a genuine lift over the old `#828282` (~4.6-4.9:1). No AA
  regression on secondary text.
- **AI-code tells.** None found. `Route.available`/`Model.capabilities.{audio,durations}` used by the
  new picker filters are real fields (`types.ts:18,71`). `veo3_1_extend → EditVideo` extend wiring is
  real (comments match `App.tsx:546-571`). Comments describe what the code does; no invented API, no
  copy-paste divergence, no stale count. CHANGELOG wording is original prose.

## Findings (all minor; none on a critical path)

1. **[minor / dead code] `rail.css:481-524`** — the entire `.model-row / .model-row-text /
   -label / -name / -route` block is now orphaned: `ModelRow.tsx` was rewritten to emit
   `.model-item-composer` and no TSX references `.model-row` any longer (grepped). *Standard:* dead
   code — the same debt class AC1 removed for `ModelBrowser`. *Fix:* delete the block in a follow-up
   (do not ship a rule set nothing selects).

2. **[minor / dead code] `ResultsFeed.tsx`** — kept in the tree but mounted nowhere (its error-banner
   markup was reproduced in `Stage`). It compiles under `tsc` so it is not caught. This is a
   *documented* deviation (implement.md, design Q5), but it is a dead component. *Fix:* a follow-up
   backlog item to delete it or re-integrate an inline feed; recommend, do not apply.

3. **[minor / a11y+UX] use-case nav in Library.** The nav `role="tab"` items always carry
   `aria-controls="generator-panel"` (`WorkspaceTabs.tsx:123`), but that tabpanel (owned by
   `SettingsRail`) is unmounted in Library view — so the control target does not exist there, and
   clicking a use-case while in Library mutates `tab` with no visible effect until the user returns to
   Studio. *Standard:* ARIA `aria-controls` should reference a present element; a control should have
   a visible effect. *Fix:* either switch to Studio on a use-case click while in Library, or suppress
   `aria-controls`/disable the nav when the panel is absent.

4. **[minor / contrast, pre-existing] `rail.css:27`** — the legally-required disclaimer
   (`.rail-disclaimer`) uses `--text-tertiary #6e6e6e` at `--text-xxs` on `#1c1e21`, ≈ **3.5:1** —
   below AA 4.5:1 for normal-size text. This slice *improved* it from the old `#626262` (~3.1:1) and
   the value is the owner-approved Tokens board, so it is **not a regression this slice introduces**.
   Flagging because the notice must stay legible and this is the one place the token shift lands on
   legally-important small text. *Fix (optional):* confirm legibility on the `tauri build`
   screenshots; consider a dedicated slightly-lighter token for the notice if it reads faint.

## Notes for the verifier (not findings)

- The `<1200px` breakpoint collapses to a stacked single content column (documented deviation from
  the Q6 sheet default). It is purely additive to the ≥1200px (1440px mockup) target and has no
  visual test — the stacked layout should be eyeballed in a narrow-window screenshot.
- Lightbox reuses the existing `Overlay` (focus-in on open, focus-restore + Escape on close). It is
  not a full Tab-trap, but that matches every other overlay in the app; no new regression.

No unresolved blocker or major. Fidelity divergences (composer model tile keeps no price by design;
picker filters are three real capability toggles vs the mockup's aspirational sort/provider row;
meta stacks instead of a sheet under 1200px) are all honest, documented, and low-severity.

VERDICT: PASS
