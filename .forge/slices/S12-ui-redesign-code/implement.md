# S12-ui-redesign-code — IMPLEMENT (claim)

Author: implementer agent. Built exactly the S12 slice (AC1–AC8) to `design.md`
(D1–D8). No Rust source changed. No new runtime dependency. The visual target is
the approved S11 mockup; values were lifted from the boards and `tokens.css`.

One design fact was wrong and is noted, not silently worked around: the fixed
3-column grid lives in **`base.css:51`**, not `app.css:51-58` (app.css is only the
import manifest). I added the per-view grid + breakpoint in `base.css` where the
rule actually is. Everything else followed the design's order of work.

---

## Files changed

Tokens / layout
- `ui/src/tokens.css` — AC2 re-value + new names: `--text-secondary #828282→#9a9a9a`,
  `--text-tertiary #626262→#6e6e6e`, `--rail-settings 410px→400px`; added
  `--rail-nav: 72px`, `--radius-card`, `--radius-panel`, `--tile-thumb-ratio: 16 / 9`,
  `--bloom-strong`.
- `ui/src/base.css` — `.workspace` now `nav · settings · stage`; `[data-view]`
  templates (studio = nav/composer/stage, library = nav/gallery/meta); one additive
  `@media (max-width:1200px)`; titlebar `.titlebar-left`/`.view-switch`; the 72px
  `.use-nav` + `.use-nav-keys`.

Sheets
- `ui/src/components/rail.css` — `.tabs-nav` vertical variant; composer model tile
  (`.model-item-composer`); `.camera-strip*`; `.rail-generate-sticky`; bloom offender
  `rgba(255,74,28,0.42)` → `var(--bloom-strong)`.
- `ui/src/components/feed.css` — danger offenders → `var(--danger-border)`/
  `var(--danger-soft)`; new `.stage*`, `.library*`, `.lightbox*` blocks.
- `ui/src/components/meta.css` — AC1: `--color-vermilion` → `--accent`.
- `ui/src/components/picker.css` — picker grid + tile rules **scoped under
  `.model-list`** (D7); `.model-item-thumb`, `[data-tier="frontier"]` ring,
  `.model-filters`.

Components
- `ui/src/App.tsx` — `view` + `lightboxId` state; titlebar Studio/Library toggle;
  72px use-case nav (`WorkspaceTabs variant="nav"`) + Keys→settings; view-conditional
  composer+Stage / Library+MetaRail; `onExtend`; lightbox `Overlay`; ConfirmDelete
  moved to a **sibling of the overlays** (AC1); removed the titlebar Settings button,
  the `ResultsFeed` mount, and the now-unused `cancelJob`/`SettingsIcon` imports.
- `ui/src/components/WorkspaceTabs.tsx` — `variant?: "row"|"nav"` (default row) with an
  original slug→icon map; Up/Down added to the roving handler for the vertical nav.
- `ui/src/components/ModelRow.tsx` — flat row → tile keeping `.model-item`
  `.model-item-name` `.chip-route`; `gradientDataUri` thumbnail; `route`/`capabilities`
  props; `up to Ns` cap. Price intentionally not duplicated (Q2).
- `ui/src/components/SettingsRail.tsx` — tablist removed (nav owns it); ModelRow gets
  route+caps; camera strip (≤3 `CameraPreview` tiles + "+N more"); sticky Generate
  wrapper; `cameraPresets`/`onPickPreset` props; legal notice unchanged at the foot.
- `ui/src/components/ModelPicker.tsx` — additive: `.model-item-thumb`,
  `data-tier` from `isLaunch`, a filters row (Runnable/Audio/30s). Pinned li/button/chip
  skeleton and all data-* unchanged.
- `ui/src/components/ResultsFeed.tsx` — AC1: `btn-quiet` → `btn-ghost btn-sm`.
- `ui/src/components/Stage.tsx` (new) — featured result large, caption, pills, recents
  strip, running timer, empty state, submit-error banner (Q5).
- `ui/src/components/Library.tsx` (new) — gallery over `jobs`, filters, caption cost.
- `ui/src/lib/cost.ts` — `stageCostCaption(job)` helper.

Deleted (AC1): `ui/src/components/ModelBrowser.tsx`, `ui/src/components/browser.css`
(orphans; `lib/browser.ts` + its test kept).

Tests (new): `ui/src/lib/__tests__/fixes.test.ts`, `ui/src/lib/__tests__/tokens.test.ts`,
`ui/src/components/WorkspaceTabs.test.tsx`, `ui/src/components/Stage.test.tsx`,
`ui/src/components/Library.test.tsx`, `ui/src/components/SettingsRail.test.tsx`,
`ui/src/App.test.tsx`. Tests (additive): `ui/src/components/ModelPicker.test.tsx`
(+1 `it`), `ui/src/lib/__tests__/cost.test.ts` (+4 `it`).

Docs: `CHANGELOG.md` `[Unreleased] → Changed` bullet. README `N/A` — it describes
use-case organisation (still true), ships no screenshot and no layout section that is
now wrong. `document.md` is the DOCUMENT step, not this one.

Rust: none. `cargo test` count therefore unchanged.

---

## Gate results (all green, pasted)

### `cd ui && pnpm test` — 272 passed (baseline 245; +27), 0 failed
```
 Test Files  22 passed (22)
      Tests  272 passed (272)
```
New/changed files: fixes (3), tokens (2), WorkspaceTabs (3), Stage (5), Library (3),
SettingsRail (4), App (2), ModelPicker (+1 = 3), cost (+4 = 31).

### `cd ui && pnpm build` — tsc --noEmit clean + vite build ok
```
✓ 75 modules transformed.
dist/assets/index-Cba-pUne.css   57.49 kB │ gzip:  9.92 kB
dist/assets/index-BUbwhEYL.js   290.95 kB │ gzip: 90.71 kB
✓ built in 2.96s
```

### `./scripts/lint-provenance.py` — PASS (exit 0)
```
CDN hostnames    PASS  no third-party media hosts referenced
Copy provenance  PASS  no shipped string matches the reference corpus (80015 shingles indexed)
```

### `cargo test --workspace` — 820 passed, 0 failed (>= 820, unchanged; no Rust touched)
```
test result: ok. 690 passed; 0 failed; 10 ignored ...   (hickeyfield_core lib)
test result: ok. 3 passed ...                           (audit_fal)
test result: ok. 2 passed ...                           (fal_diff)
test result: ok. 125 passed; 0 failed; 1 ignored ...    (hickeyfield_tauri_lib)
=== TOTAL passed === 820
```

### `ui/node_modules/.bin/tauri build --bundles app` — exit 0 (from repo root)
```
    Finished `release` profile [optimized] target(s) in 6m 40s
       Built application at: .../target/release/hickeyfield-tauri
    Bundling Hickeyfield.app (.../target/release/bundle/macos/Hickeyfield.app)
    Finished 1 bundle at:
        .../target/release/bundle/macos/Hickeyfield.app
```
Not installed to /Applications; app not opened (per instructions).

`cargo fmt`/`clippy`/`deny`: N/A — no Rust or manifest changed; unchanged since S10/S11.

---

## Per-AC evidence

- **AC1** — `fixes.test.ts` (no `--color-vermilion` in any component sheet; no
  `btn-quiet` in any tsx; `ModelBrowser.tsx`/`browser.css` gone) + `App.test.tsx`
  "opens the delete confirmation as a dialog with no button-in-button".
- **AC2** — `tokens.test.ts`: component sheets contain none of the covered hex/rgb set;
  `tokens.css` declares the new names/values. Non-vacuous (see mutation below).
- **AC3** — `WorkspaceTabs.test.tsx` (nav click + arrow rove → `onChange`) and
  `App.test.tsx` (meta rail on Library only; `[data-view]` flips; pairing effect throws
  on neither view).
- **AC4** — `cost.test.ts` `stageCostCaption` (est-only / est→actual / actual-only /
  unpriced) + `Stage.test.tsx` (featured large, running mm:ss timer, recents carry
  `data-job-anchor`, pills hidden without a saved file / without an extend use case,
  empty state).
- **AC5** — `SettingsRail.test.tsx` (`.generate-button` + `.rail-generate-sticky`;
  `.model-item.model-item-composer` with `.model-item-name`/`.chip-route`/cap; ≤3
  `.cam-frame` + a "more"; legal notice) + reused `EnhancerPicker`/`PromptPreview`
  suites still green.
- **AC6** — `Library.test.tsx` (tiles with cost/"unpriced" + `data-job-anchor`; click
  selects + opens lightbox; Failed filter) + reused `MetaCard` suite green.
- **AC7** — reused `ModelPicker.test.tsx` (li/button/chip/data-* pins) still green +
  additive `it` for `.model-item-thumb` and `data-tier`.
- **AC8** — the four gates above.

Screenshots (AC8): there is no automated window-capture tool in this repo. The
`Hickeyfield.app` bundle built; capturing Studio + Library into `test.md` is the
VERIFY step (design §5), which I must not write.

---

## The four design-§8 risks, addressed

1. **Extend pill wiring.** Verified the roster is genuinely extend-capable rather than
   assumed: `veo3_1_extend` maps to `UseCase::EditVideo` (`use_case.rs:304`) and
   `models_for(UseCase::EditVideo)` is pinned to include extend editors by the Rust test
   `the_grok_editors_can_edit_a_clip` (`registry.rs:3220`). `onExtend` prefills the
   `video` `MediaRef` from a `localPath`, switches `tab` to `edit-video`, and focuses the
   composer; the specific route is left to that tab's roster filter (no forced
   auto-select — Q4). The pill is **hidden** when there is no saved clip OR no
   `edit-video` use case in `useCases` (`App.tsx` passes `onExtend={extendUseCase ? onExtend : null}`,
   and `Stage` also guards on `saved`). Worst case is a missing pill, never a broken one —
   pinned by `Stage.test.tsx` "hides the Extend pill when no extend use case".

2. **`.model-item` scoped between composer and picker.** The picker's grid/tile rules are
   all scoped under `.model-list .model-item` in `picker.css`; the composer tile carries
   its own `.model-item-composer` layout in `rail.css`. rail.css loads before picker.css,
   so an *unscoped* `.model-item` in picker.css would have won the tie against the composer
   tile — the scoping is what prevents the composer tile inheriting the 5-col grid.
   `SettingsRail.test.tsx` asserts the composer tile still resolves `.model-item-name`/
   `.chip-route`; `ModelPicker.test.tsx` (unchanged pins) proves the picker DOM is intact.

3. **Non-vacuous button-in-button test.** `App.test.tsx` renders the real `<App/>`,
   navigates to Library, clicks a MetaCard's `aria-label="Delete generation"` to actually
   **open** ConfirmDelete, then asserts a live `[role="dialog"]` with `>0` buttons exists
   *and* `document.querySelectorAll("button button").length === 0`. So the dialog is
   genuinely mounted (not the `open={false}→null` at-rest case) and still nests nothing —
   re-nesting ConfirmDelete inside a `<button>` (the pre-fix bug) makes the count non-zero
   and fails the test.

4. **Global `--text-secondary` shift (#828282→#9a9a9a).** It flows through `var()` to
   every secondary-text surface at once: composer chip values and labels, `.model-item-sub`
   and the picker "unavailable" reasons, meta-card `dd` figures and the enhanced-prompt
   body, empty-state explanations, dropzone/frame-slot labels, `.stage-hero-detail` and the
   gallery captions. `--text-tertiary` (#626262→#6e6e6e) similarly lifts the 10px caps
   labels, the legal notice, and field `dt`s. Both are the owner-approved contrast lift
   (`S11/document.md`); the only place a regression could hide is the rendered app, so it is
   flagged for the `tauri build` screenshots in test.md.

---

## Mutation checks I ran (to prove the new tests bite)

- Put `#828282` into `feed.css` → `tokens.test.ts` "component sheets use tokens" **failed**
  on that file; reverted. (Kept in `tokens.css` it is allowed — the test only scans
  `components/*.css`.)
- Re-nested `<ConfirmDelete/>` inside a `<button>` in a scratch edit → `App.test.tsx`
  button-in-button **failed** (`button button` length 1); reverted.

## Known limitations / honest deviations

- **`ResultsFeed.tsx` is kept but no longer mounted** (design Q5 default). Its submit-error
  banner markup was reproduced inside `Stage`, so the refusal path is not lost; the old flat
  feed file remains for a future inline/removal follow-up. It still compiles and is covered
  by `tsc`.
- **`<1200px` breakpoint: meta stacks as a column, not an `Overlay` sheet.** Design Q6's
  default was a sheet; I collapsed to a single content column (nav spans all rows, composer/
  meta stack over stage/gallery) because it keeps the breakpoint purely additive with no new
  responsive JS. The ≥1200px templates (the 1440px mockup target) are untouched. Minor
  deviation, recorded here.
- **Model-picker filters row is three real capability toggles** (Runnable only / Audio / 30s
  capable), not the mockup's aspirational "All providers / Sort: price" — the picker has no
  price or provider-list data to filter on honestly, so I built only the filters backed by
  real `capabilities`. Additive; all default off so the pinned tests still see every model.
- **jsdom lacks `HTMLMediaElement.play`**, so component tests exercise Stage/Library via the
  `<img>`/background-image branches (the same limitation the existing `ResultCard` suite works
  around). The real app renders clips through `VideoPreview`, which the `tauri build` screens
  will show.
- **Screenshots not attached here** — that is the verifier's job (no capture tool in-repo);
  the `.app` built successfully.
