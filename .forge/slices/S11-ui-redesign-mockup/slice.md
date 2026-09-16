# S11-ui-redesign-mockup — SLICE (DISCOVER + PLAN)

## Delivers (one sentence, end-to-end)
A 2–3 artboard design canvas (mockup, not code) of the redesigned Hickeyfield — a modern
Higgsfield-like layout/feel built entirely from ORIGINAL copy and assets — approved by the owner,
plus the implementation contract for the follow-on code slice (S12) derived from it.

## Ground truth (verified by S10-era explore; see STATUS.md "S10 redesign")
- Token layer `ui/src/tokens.css` (141 props, dark-only) → fonts → `base.css` → component sheets
  (app/rail/feed/meta/picker/setup.css). Layout `app.css:51-58`: fixed 3-col grid 410px/1fr/390px
  (tokens.css:211-212), NO @media anywhere.
- Weak spots: `ModelPicker.tsx` text-only list (no thumbnails/price); `ResultsFeed` single column,
  no gallery/grid/lightbox/library; zero preview media (25 presets are CSS-transform fallbacks via
  `CameraPreview`/`placeholder.ts`); flat 10-item left rail (`SettingsRail.tsx:206-253`); type scale
  unused (only h-xs/h-sm).
- Bugs to fix first in S12: `App.tsx:582-591` ConfirmDelete rendered INSIDE the titlebar <button>
  (invalid HTML); dead `ModelBrowser.tsx` + `browser.css` (~600 lines, imported nowhere); dangling
  `--color-vermilion` (picker.css:259 → use --accent) and `.btn-quiet` (ResultsFeed.tsx:53).
- Test-locked class names to KEEP: .model-item .model-item-name .model-item-main .chip-route
  .enhancer-picker .enhancer-model .enhancer-backend .prompt-preview-text .prompt-preview-retry
  .generate-button + data-unrunnable/data-unavailable/data-tier. Legal notice at
  `SettingsRail.tsx:249-252` must stay visible.
- Provenance: `./scripts/lint-provenance.py` bans Higgsfield copy and media hosts. "Higgsfield-like"
  means layout/feel ONLY (dense dark studio, hero gallery, tile grid pickers with thumbnails + price,
  sticky generate bar, left nav of use cases). No Higgsfield strings, logos, screenshots, or
  hotlinked media; all mockup imagery is original placeholder art (gradients, generated shapes, own
  renders) — the snapshot's `thumbnailUrl` values must NOT be rendered.
- Safest implementation seams (for S12), in order: tokens.css re-valuation → base.css primitives →
  per-sheet restyle (feed.css gallery grid, picker.css list→tile grid keeping li/button skeleton)
  → additive breakpoints. Do NOT rewrite the 3-col grid first (scrollIntoView pairing App.tsx:544-551).

## Acceptance criteria (testable)
- [ ] AC1 — Design canvas published (Artifact) with 3 artboards at desktop width: (a) Home/Generate
  — left use-case nav, prompt + enhancer preview, settings chips, sticky Generate with price;
  (b) Model picker as a tile grid with thumbnail, name, provider chip, price/s or /image, tier and
  unrunnable/unavailable states; (c) Results/Library — gallery grid with lightbox, per-job MetaCard
  showing estimated vs actual cost and the running spend meter. Every visible string is original;
  no vendor names beyond model NAMES/providers we already show; no external image URLs.
- [ ] AC2 — Tokens sheet artboard or panel: the proposed palette, type scale, spacing and radius
  values that will re-value `tokens.css` (mapping old token → new value), so S12 is a re-valuation
  not a rewrite.
- [ ] AC3 — Every test-locked class name and the legal notice are accounted for on the mockup
  (annotated), and the fixed-grid → responsive plan is drawn (desktop + one narrow breakpoint).
- [ ] AC4 — Provenance: the canvas HTML passes a grep for the banned hosts/strings the lint uses
  (run the lint's corpus check against the artboard text); design doc lists the asset sources
  (all original).
- [ ] AC5 — Owner review: the owner sees the canvas link and approves or annotates; the DOCUMENT
  phase records the decisions; the S12 implementation contract (`.forge/slices/S12-*/slice.md`)
  is drafted from the approved boards.
- [ ] AC6 (regression) — no source change in this slice; `cargo test --workspace`, pnpm test/build
  untouched (verify by `git diff main --stat` showing only `.forge/` + the mockup file).

## Approach + why now
Owner order: looks after money. Mockup first (cheap, reversible, owner-approved) so the code
slice re-values tokens and restyles sheets against an agreed target instead of iterating in code.
