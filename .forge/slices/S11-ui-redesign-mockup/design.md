# S11-ui-redesign-mockup — DESIGN

Direction: one, "dense dark studio" — the app's existing token sheet (vermilion #ff4a1c brand,
cool neutrals #131416/#1c1e21/#2a2d32, Bebas Neue display, Space Grotesk 700 −4% names, Inter UI,
IBM Plex Mono numbers) kept and finally USED; the layout borrows only the generic shape of a modern
generation studio: 72px use-case nav · 400px composer (model tile → prompt → camera strip → chips →
sticky Generate with price) · a stage (latest result large, recents strip) · a Library gallery with
lightbox and a meta rail showing estimated vs actual cost and the running spend.

Artboards (`mockup/*.dc.html`, 1440×900 each): Main (Studio), ModelPicker (tile grid with
thumbnail/name/provider chip/price/tier + honest unrunnable and unavailable states), Library
(gallery + MetaCard with settled/estimated/actual/priced-from), Tokens (old→new token table, type
slots, desktop + narrow layout, pinned class names and the fixes-first list).

Grounding: every value lifted from `ui/src/tokens.css`, `base.css`, `app.css`, `components/*.css`
(52px titlebar, 24px card radius, 32px chips, 40px buttons, 16:9 tiles, bevel overlay, bloom on the
accent). Assets: CSS gradients only (original); fonts via Google Fonts in the canvas (the app
bundles the same faces locally). No external image URL; grep in implement.md.

S12 implementation seams (from the Tokens board): tokens.css re-valuation (text-secondary/tertiary
nudge, --radius-card/--radius-panel, --rail-nav 72px, --rail-settings 400px) → base.css → feed.css
gallery grid → picker.css list→tile grid keeping li/button skeleton → additive @media <1200px.
Fix first: ConfirmDelete out of the titlebar button, delete ModelBrowser.tsx + browser.css,
--color-vermilion → --accent, .btn-quiet.
