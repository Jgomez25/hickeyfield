# S12-ui-redesign-code — RELEASE
**Gate:** `python3 .forge/gate.py S12-ui-redesign-code` exit 0. TEST + REVIEW + SECURITY = PASS
(Opus 4.8 verifiers; zero repair rounds).
**Shipped:** the approved S11 redesign in code — stage-first Studio, 72px use-case nav, Library
gallery + lightbox + cost meta, composer model tile + sticky priced Generate, tile-grid picker, token
re-valuation, four fixes-first. UI only.
**Regression:** pnpm test 272 / build ok; provenance PASS; cargo 820 / 0; tauri build + launch at install.
**Gap:** AC8 screenshots not captured (no screen-recording permission in the session) — owner visual
review on the installed build stands in.
**Landing:** commits on forge/s12-redesign-code, ff to main (local fork only, never upstream),
`tauri build --bundles app`, ad-hoc codesign, reinstalled to /Applications.
**Status:** released (local main).

**HOTFIX (2026-09-16, owner report "overlapping text on Select model"):** the picker opened as the
520px sheet with five fixed columns → ~85px tiles, names wrapping one word per line, LAUNCH badge
over the name. Fix: overlay variant `full`, grid `repeat(auto-fill, minmax(220px, 1fr))`, badge moved
onto the thumbnail, name 16px block with wrap, subtitle 2-line clamp. Verified in the browser mock at
1440×900 (symmetric 5-across, legible); pnpm test 272/272, build ok, provenance PASS. Landed as
`forge/s12-hotfix-picker` → main, rebuilt + reinstalled.
