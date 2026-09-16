# S12-ui-redesign-code — DOCUMENT

## What shipped
- Stage-first Studio: latest result large with caption (model · duration · resolution · est.→actual per
  the S10 rules, unknown never $0.00), action pills (Extend/Retry prefill the composer only; Save),
  recents strip, running timer, empty state. `ResultsFeed` unmounted (kept; follow-up: delete).
- 72px use-case nav (`WorkspaceTabs` variant="nav", keeps tablist/roving tabindex) + titlebar
  Studio/Library toggle; per-view workspace grid in base.css (the real home of the 3-col grid) with
  the meta rail hidden on Studio and shown on Library; one additive `@media (max-width: 1200px)`
  (meta stacks as a column — design's sheet variant deferred).
- Composer: model TILE (original gradient art via `placeholder.ts`, name, provider chip, price, cap;
  `.model-item*` classes kept, scoped as `.model-item-composer`), camera strip, ChipRow unchanged,
  sticky Generate (`.generate-button`), legal notice at the foot.
- Library view over the existing jobs array (no Rust command): 16:9 gallery, captions with cost or
  "unpriced", lightbox via the existing Overlay, filters All/Video/Image/Failed, MetaRail + S10 MetaCard.
- Model picker: tile grid CSS scoped under `.model-list`, li/button skeleton and all data-* states
  kept; filters are three honest capability toggles (Runnable / Audio / 30s).
- Tokens re-valued exactly per the approved Tokens board; new tokens `--radius-card`, `--radius-panel`,
  `--rail-nav`, `--tile-thumb-ratio`; a vitest greps component sheets for hardcoded covered hexes.
- Fixes first: ConfirmDelete out of the titlebar button (non-vacuous test), `ModelBrowser.tsx` +
  `browser.css` deleted, `--color-vermilion` → `--accent`, `.btn-quiet` → `.btn-ghost .btn-sm`.

## Docs changed
- `CHANGELOG.md` — `[Unreleased] › Changed`: the redesign, in original wording.
- `.forge/backlog.md` follow-ups (non-blocking, from REVIEW): delete orphaned `.model-row*` block in
  rail.css and the unmounted ResultsFeed; nav tabs' `aria-controls` while in Library; legal notice
  contrast ≈3.5:1 (improved from 3.1:1, owner-approved value — consider a bump); <1200px meta as a sheet.

## Evidence
- TEST / REVIEW / SECURITY = PASS. pnpm test 272 / build ok; provenance PASS; cargo 820 / 0
  (no Rust change); tauri bundle builds and launches.
- AC8 screenshots NOT captured: the session lacks Screen Recording / assistive access; the built app
  launched and stayed alive; the owner reviews visually on install.
