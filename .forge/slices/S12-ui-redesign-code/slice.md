# S12-ui-redesign-code — SLICE (DISCOVER + PLAN)

## Delivers (one sentence, end-to-end)
The running app looks like the approved S11 canvas: a stage-first Studio with a 72px use-case nav,
a 400px composer with a model tile and sticky priced Generate, a Library gallery with lightbox and
the estimated-vs-actual meta rail, and a tile-grid model picker — implemented as a token
re-valuation plus per-sheet restyles, with every test-locked class name and the legal notice intact.

## Ground truth (approved S11 boards; explore facts in S11/slice.md + STATUS)
- Canvas https://claude.ai/artifact/2des9awBapmx8rSZN8wvAe; owner decisions in S11/document.md:
  stage-first Studio, 72px icon nav, meta rail hidden on Studio / shown on Library, tokens as proposed.
- Cascade: `ui/src/tokens.css` → `base.css` → `components/{rail,feed,meta,picker,setup,camera-preview}.css`
  (app.css import order is load-bearing). Fixed grid `app.css` `.workspace` 410px/1fr/390px, no @media.
- Test-locked class names to KEEP: .model-item .model-item-name .model-item-main .chip-route
  .enhancer-picker .enhancer-model .enhancer-backend .prompt-preview-text .prompt-preview-retry
  .generate-button + data-unrunnable/data-unavailable/data-tier. Legal notice `SettingsRail.tsx:249-252`
  stays visible. scrollIntoView pairing `App.tsx:544-551` must keep working.
- Bugs to fix first: ConfirmDelete inside the titlebar <button> (`App.tsx:582-591`); dead
  `ModelBrowser.tsx` + `browser.css`; dangling `--color-vermilion` (picker.css:259) and `.btn-quiet`.
- Provenance lint bans vendor copy/media; snapshot `thumbnailUrl`s must never be rendered — model
  tile thumbnails are ORIGINAL (generated placeholder art or the user's own renders from the library).
- 25 camera presets are CSS-transform previews (`CameraPreview`/`placeholder.ts`) — reuse as the
  camera strip tiles.

## Acceptance criteria (testable)
- [ ] AC1 — Fixes-first landed with tests/lint: ConfirmDelete rendered outside the titlebar button
  (valid HTML, vitest asserts no button-in-button), ModelBrowser.tsx + browser.css deleted, no
  dangling `--color-vermilion`/`.btn-quiet` (grep test or stylelint).
- [ ] AC2 — tokens.css re-valued per the Tokens board (values listed in S11/document.md); new tokens
  added; no component sheet hardcodes a hex that a token covers (grep test on components/*.css).
- [ ] AC3 — Shell: 72px use-case nav (Video/Animate/Image/Edit/Extend + Keys) drives the existing
  use-case tab state; workspace grid becomes nav/composer/stage(/meta) with the meta rail hidden on
  Studio and shown on Library; one additive breakpoint <1200px (composer over stage, meta as a sheet);
  vitest for nav→use-case wiring and rail visibility per view.
- [ ] AC4 — Studio stage: latest result large with caption (model · duration · resolution ·
  est.→actual) and action pills (Extend/Retry/Save), recents strip below; running job shows the
  timer; empty state when no results. vitest for caption cost text (estimate only / actual known).
- [ ] AC5 — Composer: model TILE (thumbnail placeholder, name, provider chip, price, cap) keeping
  .model-item* classes; camera strip (3 tiles + "more"); ChipRow unchanged; sticky Generate bar with
  price (.generate-button); legal notice at the foot. Existing enhancer/preview tests still pass.
- [ ] AC6 — Library view: gallery grid (16:9 tiles, caption with cost or "unpriced"), lightbox on
  click, filters (All/Video/Image/Failed), meta rail with spend meter (actual · est. · unpriced) and
  the S10 MetaCard (settled/estimated/actual/priced-from). vitest for grid render + lightbox open.
- [ ] AC7 — Model picker overlay as a 5-col tile grid keeping the li/button skeleton and all
  data-* states (frontier badge, unrunnable, unavailable), filters row; ModelPicker.test.tsx passes
  with additive assertions only.
- [ ] AC8 (regression) — `cargo test --workspace` unchanged (>= 820), `cd ui && pnpm test && pnpm
  build` (>= 245), `./scripts/lint-provenance.py`, clippy/fmt untouched; `tauri build` app launches
  and a screenshot of Studio + Library is attached in test.md; CHANGELOG entry.

## Approach + why now
Owner-approved mockup in hand. Order of work (safest seams): AC1 fixes → AC2 tokens → base.css →
feed.css gallery → picker.css tile grid → rail/nav + layout + breakpoint → stage. Never rewrite the
3-col grid first; add the 72px track and a view switch.
