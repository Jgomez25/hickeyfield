# S12-ui-redesign-code — TEST (evidence)

Author: adversarial verifier (a different agent than the implementer). Every command below
was run by me in this session; all output is pasted verbatim, not copied from `implement.md`.

## Implementer's claim (verbatim, `implement.md`)

> Built exactly the S12 slice (AC1–AC8) to `design.md` (D1–D8). No Rust source changed. No
> new runtime dependency. … `cd ui && pnpm test` — 272 passed (baseline 245; +27), 0 failed
> … `cd ui && pnpm build` — tsc --noEmit clean + vite build ok … `./scripts/lint-provenance.py`
> — PASS (exit 0) … `cargo test --workspace` — 820 passed, 0 failed … `tauri build … Built
> application at: .../target/release/bundle/macos/Hickeyfield.app`.

Deviations the implementer recorded: `<1200px` breakpoint stacks meta as a column (not a
sheet); picker filters are three capability toggles (not "all providers/sort"); `ResultsFeed`
kept but unmounted; the fixed grid lives in `base.css`, not `app.css`.

---

## Commands (real output, this session)

### AC8 — `cd ui && pnpm test`
```
 Test Files  22 passed (22)
      Tests  272 passed (272)
   Duration  31.05s
```
272 passed, 0 failed — matches the claim and clears the ≥245 floor.

### AC8 — `cd ui && pnpm build`
```
✓ 75 modules transformed.
dist/assets/index-Cba-pUne.css   57.49 kB │ gzip:  9.92 kB
dist/assets/index-BUbwhEYL.js   290.95 kB │ gzip: 90.71 kB
✓ built in 3.26s
BUILD_EXIT=0
```
tsc + vite clean, exit 0.

### AC8 — `./scripts/lint-provenance.py`
```
CDN hostnames    PASS  no third-party media hosts referenced
Copy provenance  PASS  no shipped string matches the reference corpus (80015 shingles indexed)
PROV_EXIT=0
```

### AC8 — `cargo test --workspace`
```
CARGO_EXIT=0
TOTAL_PASSED= 820
```
(690 core + 3 audit_fal + 2 fal_diff + 125 tauri_lib = 820; ≥820 floor met; no Rust changed.)

### AC8 — `tauri build` bundle + launch
Bundle present and built by the implementer (tree is unchanged, `git status` clean, so no
rebuild): `target/release/bundle/macos/Hickeyfield.app`. Launched with `open`; process came
up and stayed alive (`pgrep`: `69364 …/Contents/MacOS/hickeyfield-tauri`) — it launched
without crashing (not a blank-window `cargo build`). Quit cleanly afterward.

**Screenshots could not be captured.** `screencapture -x` fails in this environment with
`could not create image from display` (the sandboxed shell has no Screen Recording / TCC
grant), and `osascript`/System Events is denied assistive access (`-1728`), so I cannot
script the Library toggle either. This is the design §5 "capture impossible" fallback. As
substitute evidence that the shipping frontend is the S12 redesign, the freshly-built
`ui/dist/assets/index-*.js` carries every new class: `view-switch`(3), `use-nav`(2),
`stage-hero`(14), `library-tile`(4), `model-item-composer`(1), `camera-strip`(6). The bundle
was produced from this same clean tree.

---

## Break attempts

### 1. Pinned classes / data-attrs still present (AC5/AC7)
`grep` over `ui/src`: `model-item`, `model-item-name`, `model-item-main`, `chip-route`,
`enhancer-picker`, `enhancer-model`, `enhancer-backend`, `prompt-preview-text`,
`prompt-preview-retry`, `generate-button` all present across 3–6 files; 17 hits for
`data-unrunnable|data-unavailable|data-tier`. `ModelPicker.test.tsx` (li/button/chip/data-*
pins) passes. **Not broken.**

### 2. AC1 fixes real, and the button-in-button test is non-vacuous
`ls` confirms `ModelBrowser.tsx` and `browser.css` are gone; `--color-vermilion` and
`btn-quiet` appear only inside `fixes.test.ts` (the guard), nowhere in shipped css/tsx.
Mutation (scratch copy **outside the repo**, `…/scratchpad/uicopy`): I re-wrapped
`<ConfirmDelete …>` in a `<button>` in `App.tsx`. React logged
`In HTML, <button> cannot be a descendant of <button>` and the test failed:
```
× opens the delete confirmation as a dialog with no button-in-button
  → AssertionError: expected 4 to be +0
  expect(document.querySelectorAll("button button").length).toBe(0);
```
The dialog is genuinely opened (the "meta rail on Library" sibling test still passed), so the
assertion is structural, not the at-rest `open=false→null` case. **Test bites.**

### 3. AC2 tokens re-valued per the board; hex-grep is non-vacuous
`tokens.css` has `--text-secondary:#9a9a9a`, `--text-tertiary:#6e6e6e`, `--rail-settings:400px`,
`--rail-meta:390px`, `--rail-nav:72px`, `--radius-card`, `--radius-panel`,
`--tile-thumb-ratio:16/9`, `--bloom-strong`. These match `S11/document.md:7-8` verbatim
(text-secondary #9a9a9a, text-tertiary #6e6e6e, radius-card 1rem, radius-panel 1.5rem,
rail-nav 72px, rail-settings 400px, tile-thumb-ratio 16/9) and the Tokens board's new column.
Mutation (scratch copy): appended `.x { color: #828282; }` to `feed.css` →
```
× component sheets use tokens, not covered hexes
  tokens.test.ts:60  ${file} must reference a token, not the literal #828282
```
**Test bites.**

### 4. AC3 shell wiring
`base.css` carries `.workspace[data-view="studio"]` (nav/settings/1fr — no meta) and
`[data-view="library"]` (nav/1fr/meta — no composer), plus one additive
`@media (max-width:1200px)` collapsing to `nav minmax(0,1fr)`. The ≥1200px templates are the
default and untouched by the query (additive confirmed). `App.test.tsx` asserts the meta rail
mounts on Library only and the view flips, and that the `App.tsx:612` pairing effect (`?.`)
throws on neither view — passes. Mutation (scratch): replaced the nav's
`onClick={() => onChange(tab.id)}` with a no-op →
```
× WorkspaceTabs.test.tsx:84  expected "spy" to be called with [ 'edit-video' ]  (0 calls)
```
**Nav→use-case wiring test bites.**

### 5. AC4/AC6 cost captions and guards
`lib/cost.ts stageCostCaption` implements the D4 table exactly: est+actual → `est. X → actual Y`;
est-only → `est. X` (no invented arrow); actual-only → `actual Y`; neither → `unpriced`
(tone `unpriced`). `cost.test.ts` (31) green. Extend pill guarded `{onExtend && saved ? …}` in
`Stage.tsx:247` and App passes `onExtend` only when an edit-video use case exists (`App.tsx:543`)
— hidden when no clip or no extend use case. Lightbox wired via `lightboxId` + `Overlay`
(`App.tsx:138,573`); `Library.test.tsx` asserts click opens the lightbox — green. Mutation
(scratch): renamed the recents tile `data-job-anchor` →
```
× Stage.test.tsx:83  expected 0 to be 1  (.stage-recent-tile[data-job-anchor])
```
**Stage anchor test bites.**

### 6. AC7 picker additive-only
`git diff main...HEAD -- ui/src/components/ModelPicker.test.tsx`: 0 removed assertion lines;
one new `it` for `.model-item-thumb` + `data-tier`-from-`isLaunch`. Pinned li/button/chip DOM
unchanged. **Additive confirmed.**

### 7. Provenance: no snapshot thumbnails rendered
`grep thumbnailUrl ui/src` → none. Model tiles (`ModelRow.tsx`, `ModelPicker.tsx`) use
`gradientDataUri(model.id)` (deterministic original SVG). Stage/Library thumbnails use
`posterFor(result)`/`gradientDataUri(job.id)` over the user's own `jobs` — the same source
`ResultCard` already renders, not a provider snapshot thumbnail. Lint PASS (above). **Not broken.**

### 8. Four-plus mutations proven to bite (§ above): App button-in-button, tokens hex-grep,
WorkspaceTabs nav wiring, Stage data-job-anchor. All four new tests are non-vacuous.

---

## Findings (not defects)

- **AC3 breakpoint deviates from the AC's parenthetical**: `<1200px` stacks meta as a column
  rather than an `Overlay` sheet (Q6 default). The AC's testable core — "one additive
  breakpoint <1200px" — is satisfied and verified; the ≥1200px (1440px mockup) layout is
  untouched. Recorded in `implement.md`, honest, non-material.
- Picker filters are three real capability toggles (Runnable/Audio/30s), not the mockup's
  aspirational "providers/sort"; the mockup is design material, AC7 requires only a filters
  row, and all default off so pinned tests still see every model. Recorded.
- `ResultsFeed.tsx` compiles but is unmounted (Q5 default); its error-banner markup lives in
  `Stage`. Recorded.

None of these breaks an acceptance criterion. All four automated gates pass at the required
floors, every pinned class/attribute survives, the four fixes-first landed with non-vacuous
tests, and provenance is clean. The only unmet item is the AC8 screenshot, which is blocked by
the environment's Screen Recording permission (documented §5 fallback), not by the code — the
built `.app` launched and stayed alive, and the shipping frontend carries the S12 markers.

VERDICT: PASS
