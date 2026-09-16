# S11-ui-redesign-mockup — TEST (evidence)

Adversarial verification by a different agent than the implementer. Read-only apart from this
file. The 2.5 MB seeded `hickeyfield-studio-redesign.html` was NOT opened (grep -c only); the
published Artifact URL was NOT opened. No cargo run — this slice touches no Rust.

## Implementer's claim (implement.md, verbatim)

> Deliverable: design canvas "Hickeyfield Studio Redesign", published as a private Artifact
> (https://claude.ai/artifact/2des9awBapmx8rSZN8wvAe, version 1), seeded from `mockup/` with the
> design skill's helper (`--check` ok: 4 artboards + canvas.json, 0 images, 2.53 MB).
>
> AC1 — 3 screens: Main (Studio), ModelPicker, Library. AC2 — Tokens board with old→new table.
> AC3 — pinned class names and the legal notice placed and annotated (Tokens board, right column;
> classes also literally present on Main/ModelPicker markup). AC4 — provenance: `grep -o
> 'https\?://…' *.dc.html` → only fonts.googleapis.com; no storage.googleapis / twimg / higgsfield
> hosts; the only vendor string is the app's own shipped legal notice. AC6 — `git diff main --stat`
> touches only `.forge/`. AC5 — awaiting owner review (two sticky notes on the canvas list the four
> questions).

Note (ambiguity, not a defect): implement.md writes `mockup/`, but the working files actually live at
`.forge/slices/S11-ui-redesign-mockup/mockup/`. That location is consistent with AC6 (everything
under `.forge/`). Hashes match review.md exactly (verified below).

## Commands

### Files + hashes (`shasum -a 256`)
```
98ee9e38...635f58  Library.dc.html
98f686e3...8022be  Main.dc.html
fc0499a7...426096  ModelPicker.dc.html
3d095dce...2e55b2  Tokens.dc.html
ad20d692...6edbef  canvas.json
```
All five match the hashes recorded in review.md. Sizes: Main 17916, ModelPicker 12992,
Library 11965, Tokens 8371, canvas.json 1092; seeded HTML 2534949 bytes (2.53 MB — matches claim).

### AC1 — three screens with the listed elements (read + grep)
- Main.dc.html (Studio): use-case icon nav `Video/Animate/Image/Edit/Extend` (:47-63); model tile with
  thumbnail `.thumb-b` + name `Seedance 2.5` + `.chip-route` `fal` + price `$0.021 / 1k tokens` (:76-87);
  prompt + enhancer preview `Enhance: on` / `.prompt-preview-retry` "Preview rewrite" (:92-96);
  settings chips `Duration/Resolution/Aspect/Audio on` (:113-116); sticky `.generate-button` with price
  `<span>Generate</span>` / `est. $0.43 · 10s` (:123-125). All confirmed by direct read.
- ModelPicker.dc.html: 5-col tile grid, each tile has media thumbnail (`.tile-media` gradient), name,
  provider chip `.chip-route prov`, price (`$0.14 / s`, `$0.021 / 1k tok`), tier badge
  `<span class="tier">FRONTIER</span>` + `data-tier`; unrunnable `data-unrunnable="true"` (:118) and
  unavailable `data-unavailable="true"` (:127) states present with honest reasons.
- Library.dc.html: 4-col gallery grid, 12 items, lightbox affordance ("click to open" on Main stage /
  captions); MetaCard shows `Estimated $0.43` (:95) vs `Actual $0.41 · −$0.02` (:96) and a running
  spend meter `Spend today $3.42 actual · est. $3.61` (:78-80).

URL grep across all four artboards + canvas.json:
```
Library/Tokens/Main/ModelPicker.dc.html:https://fonts.googleapis.com/css2?family=Bebas+Neue&family=Space+Grotesk...
exit=0
```
Only fonts.googleapis.com. No `<img>`, no `.jpg/.png/.webp/.mp4`, no `thumbnailUrl`, no `src=` other
than the local `./support.js` helper (canvas-tool provided, not an external host). No lorem / no
marketing boilerplate (grep for lorem|ipsum|revolution|unleash|next-gen|game-chang|cutting-edge →
no match, exit 1).

### AC2 — Tokens board with old→new mapping
Tokens.dc.html:28-42 has a three-column table `Token | Now | Proposed`: e.g. `--text-secondary #828282
→ #9a9a9a`, `--text-tertiary #626262 → #6e6e6e`, `--radius-card → 1rem`, `--radius-panel → 1.5rem`,
`--rail-settings 410px → 400px`, `--rail-nav → 72px`, `--tile-thumb-ratio → 16 / 9`, brand `--accent
#ff4a1c keep`. Matches the owner-accepted table in document.md.

### AC3 — pinned classes + legal notice + desktop/narrow layout
Literal counts across the four artboards:
```
model-item             40      generate-button        2
model-item-main        13      prompt-preview-retry    2
model-item-name        13      prompt-preview-text     1
chip-route             13      enhancer-picker         1
data-tier              10      enhancer-model          1
data-unrunnable         3      enhancer-backend        1
data-unavailable        3
```
Every pinned name is literally present (≥1). Legal notice byte-exact:
```
grep -c "Independent open-source project. Not affiliated with, endorsed by, or sponsored by Higgsfield, Inc."
Main.dc.html:1  (others:0)
```
Present verbatim at Main.dc.html:127. Tokens.dc.html draws both `Desktop ≥ 1200px` (72/400/1fr/390
grid, :70-77) and `Narrow < 1200px` additive breakpoint (:78-82).

### AC4 — provenance
```
./scripts/lint-provenance.py
CDN hostnames        PASS  no third-party media hosts referenced
Copy provenance      PASS  no shipped string matches the reference corpus (80015 shingles indexed)
lint-exit=0
```
Note: lint SKIP_DIRS includes `.forge`, so the script gates the shipped app, not the mockup — the
manual greps above cover the mockup itself. Banned-host grep (storage.googleapis|twimg|higgsfield.|
googleusercontent) over the four artboards + canvas.json → no match (exit 1). `Higgsfield` appears
exactly once, inside the required legal notice (Main.dc.html:127). Provider chips (`fal`, `vaig`) and
model names are the app's own. No emoji (perl codepoint scan 1F000–1FAFF/2600–27BF/2B00–2BFF/FE0F →
no match).

### AC5 — owner review recorded
document.md records owner decisions dated 2026-09-16 (stage-first Studio, 72px icon rail, meta rail
hidden on Studio/shown on Library, Tokens table accepted as proposed). S12 contract exists at
`.forge/slices/S12-ui-redesign-code/slice.md` (present in git diff below).

### AC6 — no source change; UI suite untouched
```
git diff main --stat
```
All 24 changed paths are under `.forge/` (slices S11/S12, STATUS.md, state.json). No `ui/`, `crates/`,
`src-tauri/`, README, or CHANGELOG source touched. No Rust change → cargo not run (correctly N/A).

UI test suite (Node v24.8.0, pnpm 10.15.0):
```
pnpm test  →  Test Files  15 passed (15) ; Tests  245 passed (245) ; pnpm-test-exit=0
```
245 tests pass, matching the pinned baseline.

## Break attempts
1. Hash tamper check — recomputed all five sha256 now; identical to review.md. No post-review edit.
2. Off-manifest URL / hotlinked media — full `https?://` grep + `<img>/thumbnailUrl/.jpg/.png/.mp4`
   grep: only fonts.googleapis.com and local `./support.js`. No storage.googleapis / twimg /
   googleusercontent / higgsfield host. FAILED to refute.
3. Legal notice drift — byte-exact grep of the full sentence: present, unmodified. FAILED to refute.
4. Pinned-class omission (the implementer's weakest-sounding point: "classes also literally present")
   — every one of the 13 pinned tokens including `prompt-preview-text`, `enhancer-*` present. Even
   the annotation-only ones appear literally. FAILED to refute.
5. Marketing/lorem copy or emoji slop — none found. FAILED to refute.
6. Seeding claim — grep -c of the 2.5 MB seeded file (not opened) shows `model-item-name`,
   `generate-button`, `data-unrunnable`, and the legal notice each present. Consistent with a seed
   from the artboards. FAILED to refute.
7. Regression re-run — `pnpm test` a second time exit 0, 245/245; lint re-run exit 0 (idempotent).
   FAILED to refute.
8. Path ambiguity — implement.md says `mockup/` while files live under the slice dir; harmless and
   consistent with AC6. Noted, not a defect.

No acceptance criterion could be broken. Provenance clean, all pinned classes and the verbatim legal
notice present, both layouts drawn, owner review recorded, no source change, 245/245 UI tests green.

VERDICT: PASS
