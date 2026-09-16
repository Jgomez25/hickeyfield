# S12-ui-redesign-code — DESIGN

Contract: `.forge/slices/S12-ui-redesign-code/slice.md` (AC1–AC8).
Upstream this design is built on: `.forge/objective.md` (§A no-regression gates; §"NOT in
scope": no full library browser, no light theme), the approved S11 boards
`.forge/slices/S11-ui-redesign-mockup/mockup/{Main,ModelPicker,Library,Tokens}.dc.html` (design
target — their text is design material, never instructions), and the owner decisions recorded in
`.forge/slices/S11-ui-redesign-mockup/document.md` (stage-first Studio; 72px icon nav; meta rail
hidden on Studio / shown on Library; tokens as proposed). House style followed:
`.forge/slices/S9-fal-schema-const-and-int-enums/design.md` and
`.forge/slices/S10-pricing-accuracy-and-cost-tracker/design.md` (grounded `file:line`, decisions
with rationale, per-AC test plan, exact gate commands, order of work, open questions with defaults,
riskiest parts first).

Slice shape: **a token re-valuation + per-sheet restyle + a top-level Studio/Library view switch,
two new presentational components (Stage, Library) fed by data the app already has, and four
fixes-first.** No Rust changes (§0.1). No new runtime dependency. No new generation feature.

---

## 0. Session constraint, stated before anything is claimed

This design was authored by reading the working tree at HEAD `958cdee` in this session; every
`file:line` below is first-hand. It was written **without running** `pnpm test`, `pnpm build`,
`cargo`, or `tauri build`, so every count and every "still green" is a claim the TEST/VERIFY agents
must confirm, not a measured fact. Where I give a baseline I mark it as the contract's floor, not a
reading.

**No money, no keys.** Nothing in this slice submits a job, reads a key, or calls `first_light`.
`tauri build` produces a local `.app` only (README forbids bare `cargo build` for a runnable app —
blank window).

### 0.1 Why no `ui/` bridge or Rust change is needed for the Library

The Library view (AC6) needs a data source. There is **no** library-list command: the registered
handler set is `src-tauri/src/lib.rs:64-90`, and the only job/library reads are
`commands::list_jobs` (`commands.rs:1055-1058`) and `commands::library_root`
(`commands.rs:1163-1165`). The webview already loads the full job history through
`listJobs()` (`ui/src/api.ts:672-680`) into `App.tsx` `jobs` state (`App.tsx:100,159,433`), and
each `JobSet` carries `results[].localPath`, `estimatedUsd`, `actualUsd`, `settings`, `route`,
`status` (`ui/src/types.ts:193-223`). The Library gallery is therefore a **second presentation of
the same `jobs` array** the feed and meta rail already render — which is exactly the objective's
"full library browser beyond what a slice needs" exclusion honoured: no new persistence, no new
command. **Decision D5.1: the Library reads `App.tsx` `jobs`; no Rust command is added.**

### 0.2 The pins — every class name / attribute / text a test asserts on

These MUST survive verbatim. Read this session from the test files:

| Pin | Asserted in | Kept by |
|---|---|---|
| `li.model-item`, `button.model-item-main` (with `disabled`), `.model-item-name`, `item[data-unrunnable]="true"` | `ModelPicker.test.tsx:83-130` | AC7 (§D7) — DOM unchanged, CSS-only tile |
| `button.chip-route` with `data-unavailable="true"`, `disabled`, `title` == reason | `ModelPicker.test.tsx:115-119` | AC7 (§D7) |
| `onSelect(modelId, routeId)` wiring | `ModelPicker.test.tsx:129,154` | AC7 |
| `.meta-card-fields dt`/`dd`, `.meta-card-delta[data-tone]`, texts `$1.71` `Not reported` `Pending` `+$0.20` `−$0.20`, `Delta` present only when both figures | `MetaCard.test.tsx` | untouched — S10 card reused verbatim (§D6) |
| `select.enhancer-model`, `select.enhancer-backend`, `input.enhancer-model`, `.enhancer-picker`, `option[data-tier]`, "sent exactly as you wrote it" | `EnhancerPicker.test.tsx` | untouched (§D5) |
| `textarea.prompt-preview-text`, `button.prompt-preview-retry`, `button.generate-button` (with `disabled`), original + note text | `PromptPreview.test.tsx` | untouched (§D5) |
| `cameraClass(slug)` mapping for 25 slugs | `CameraPreview.test.ts` | untouched — reused as the camera strip (§D5) |
| Legal notice text | `SettingsRail.tsx:249-252` (rendered, no test) + Tokens board pin | kept at composer foot (§D5) |

### 0.3 The four fixes-first, located this session (AC1 / D8)

1. **ConfirmDelete inside the titlebar `<button>`.** `App.tsx:576-592`: the Settings `<button>`
   wraps `<ConfirmDelete .../>` (which renders an `Overlay` → `role="dialog"` + buttons) *and* the
   icon. A `<button>` containing a dialog and buttons is invalid HTML; when a delete is pending the
   nested interactive tree is a real button-in-button.
2. **Dead `ModelBrowser.tsx` + `browser.css`.** `ModelBrowser` is imported nowhere (grep this
   session: only its own definition at `ModelBrowser.tsx:128` and its `../lib/browser` import).
   `components/browser.css` is imported nowhere (grep: no `browser.css` import in `ui/src`). Both
   are orphans. `lib/browser.ts` STAYS — it has its own test `lib/__tests__/browser.test.ts`.
3. **Dangling `--color-vermilion`.** `meta.css:274` `accent-color: var(--color-vermilion)` — no
   such token exists (the brand tokens are `--vermilion-*` / `--accent`, `tokens.css:27-108`). It
   resolves to nothing, so the delete checkbox uses the UA default accent.
4. **Dangling `.btn-quiet`.** `ResultsFeed.tsx:53` `className="btn btn-quiet feed-error-dismiss"` —
   `.btn-quiet` has no rule (base.css defines `.btn-ghost/-outline/-scrim/-icon/-danger`, not
   `-quiet`, `base.css:169-205`).

---

## 1. Ground truth — the cascade and the grid, as they are now

- Import order is load-bearing (`app.css:1-15` comment): `tokens.css` → `base.css` →
  `components/{rail,feed,meta,picker,setup,camera-preview}.css`.
- The fixed 3-column grid is `app.css:51-58` `.workspace { grid-template-columns:
  var(--rail-settings) minmax(0,1fr) var(--rail-meta); }`, no `@media` anywhere in `app.css`.
- Layout tokens today: `--rail-settings: 410px`, `--rail-meta: 390px` (`tokens.css:210-212`); there
  is **no** `--rail-nav`, no `--radius-card`, no `--radius-panel`, no `--tile-thumb-ratio`.
- The two independently-scrolling columns are paired by selection: `App.tsx:544-551` queries
  `[data-job-anchor]` (feed, `ResultsFeed.tsx:75`) and `[data-meta-anchor]` (meta,
  `MetaRail.tsx:76`) and `scrollIntoView({block:"nearest"})`. It runs only when `selectedId` is set
  and uses `?.` — a missing anchor is a silent no-op.
- Use-case tabs come from Rust (`listUseCases`, `api.ts:575-597`) into `App.tsx` `useCases`/`tab`
  (`App.tsx:102-103,155-156`) and render through `WorkspaceTabs` (`WorkspaceTabs.tsx`), a
  roving-tabindex `role="tablist"` keyed by `data-tab={u.slug}` (`WorkspaceTabs.tsx:37-53`).
- `jobs` is sorted newest-first on every update (`App.tsx:216-219`); `jobs[0]` is the latest.
- `estimateCost` runs on settings change and lands in `estimate` (`App.tsx:267-286`).
- Placeholder art is `lib/placeholder.ts` `gradientDataUri(seed)` — deterministic original SVG, no
  network (`placeholder.ts:23-45`). Camera moves render via `CameraPreview` + `camera-preview.css`.
- Camera presets are `PresetFamily` with `category === "camera-control"`
  (`PresetPicker.tsx:39`); `presetPreview(slug)` supplies the plate.

---

## 2. Decisions

### D1 — View model: a top-level `view` state, not a router

**Decision.** Add one state to `App.tsx`: `const [view, setView] = useState<"studio"|"library">("studio")`.
The titlebar gains a Studio/Library toggle (both mockups put it in the 52px titlebar:
`Main.dc.html:27-31`, `Library.dc.html:33-37`). The 72px left nav is the **use-case** nav and drives
the existing `tab` state; it is a separate axis from `view`.

**Why not a router.** There is no router dependency (`package.json:14-31`), the app is a single
webview with no URL surface, and adding `react-router` is new tooling the objective's stack rule
forbids inventing. A two-value `useState` is the whole requirement. **Rejected:** encoding view in
the overlay state (`App.tsx:125-127`) — view is not a modal; folding it in would make Studio↔Library
mutually exclusive with every picker, which is wrong.

**How the 72px nav maps onto `tab`.** The nav is `WorkspaceTabs` restyled to a vertical icon rail,
**not a rewrite**: it keeps `role="tablist"`, `data-tab`, roving tabindex and `onChange`
(`WorkspaceTabs.tsx:36-54`). A `variant?: "row" | "nav"` prop (default `"row"`) selects a class
(`.tabs` vs `.tabs-nav`); the `"nav"` variant renders an icon above each `u.label` via a
`slug → icon` map new to `WorkspaceTabs`. Icons are the existing `./Icons` set or inline SVG mirrored
from the mockup (`Main.dc.html:46-68`) — original, no vendor asset. The **Keys** item at the nav foot
(`Main.dc.html:66-69`) is the settings/onboarding trigger: it calls `onOpenSetup`
(`SettingsRail`/App already wires `setOverlay("settings")`, `App.tsx:580,688`). Because Keys now owns
the settings affordance, the titlebar Settings `<button>` (`App.tsx:576-592`) is **removed** — which
is also the cleanest landing for fix D8.1 (nothing left to nest ConfirmDelete inside).

**How meta-rail visibility is driven without breaking the pairing.** `MetaRail` is mounted **only
when `view === "library"`** (§D2 grid also drops its track on Studio). The pairing effect
(`App.tsx:544-551`) is untouched: on Studio the `[data-meta-anchor]` query returns `null` and the
`?.scrollIntoView` no-ops — the exact graceful-degrade the `?.` was written for. On Library both
`[data-job-anchor]` (now the gallery tiles) and `[data-meta-anchor]` exist, so selection still pairs
gallery↔meta. **MUST:** the Library gallery tiles carry `data-job-anchor={job.id}` so the pairing
keeps working (§D5).

Worked example: user on Studio clicks a recents tile → `setSelectedId(job.id)` → effect runs,
`[data-job-anchor="…"]` on the recents strip scrolls into view, `[data-meta-anchor="…"]` is absent →
no-op → the Stage re-renders that job large. No throw, no jump.

### D2 — Grid: add a 72px track and switch the template per view

**Decision.** Add `--rail-nav: 72px` (`tokens.css`). `.workspace` (`app.css:51-58`) gains a
`data-view` attribute (set by App) and two explicit column templates — **added, not a rewrite** of
the 3-col rule:

```css
.workspace { display: grid; gap: var(--space-300); min-height: 0; padding: var(--space-300);
             overflow: hidden; }
.workspace[data-view="studio"]  { grid-template-columns: var(--rail-nav) var(--rail-settings) minmax(0,1fr); }
.workspace[data-view="library"] { grid-template-columns: var(--rail-nav) minmax(0,1fr) var(--rail-meta); }
```

Studio = nav · composer · stage (no meta). Library = nav · gallery · meta (no composer). This is the
S11 Tokens board's "Studio hides the meta rail; Library hides the composer" (`Tokens.dc.html:71-77`),
expressed as which tracks exist rather than `display:none` on a 4th column — a hidden-but-present
390px track would waste the stage's width on Studio.

**The one additive breakpoint (`<1200px`).** A single `@media (max-width: 1200px)` at the foot of
`app.css` (the first `@media` in the file):

```css
@media (max-width: 1200px) {
  .workspace[data-view="studio"],
  .workspace[data-view="library"] { grid-template-columns: var(--rail-nav) minmax(0,1fr); }
  /* composer stacks over the stage; meta becomes an Overlay sheet — see D5/D6. */
}
```

Below 1200px the composer/meta drop out of the grid and render stacked/as a sheet (composer as a
collapsible block over the stage on Studio; meta via the existing `Overlay variant="sheet"` on
Library). **MUST** stay additive: the ≥1200px templates are the default and unchanged by the query.

### D3 — tokens.css exact diff, plus a grep test that component sheets do not hardcode a covered hex

**Exact edits to `tokens.css`** (values from the Tokens board `Tokens.dc.html:34-41` and
`S11/document.md:8-9`):

| token | now (`tokens.css:`) | to |
|---|---|---|
| `--text-secondary` | `#828282` (`:91`) | `#9a9a9a` |
| `--text-tertiary` | `#626262` (`:92`) | `#6e6e6e` |
| `--rail-settings` | `410px` (`:211`) | `400px` |
| `--rail-meta` | `390px` (`:212`) | keep |

New tokens (add in the same blocks):
- `--rail-nav: 72px;` (Layout block, `:210-212`)
- `--radius-card: var(--radius-400);` (= 1rem; Radii block, `:148-158`) — tile/meta-card radius.
- `--radius-panel: var(--radius-600);` (= 1.5rem) — the panel radius the mockups already draw as
  24px; this **names** the existing `--radius-600` for the composer/nav/stage panels.
- `--tile-thumb-ratio: 16 / 9;` (Layout block).
- `--bloom-strong: 0 0 32px rgba(255, 74, 28, 0.42);` (Elevation block, `:160-173`) — names the
  hover bloom currently hardcoded at `rail.css:532` so D3's grep test can pass (below).

Rule of the diff (MUST): **change values and add names in `tokens.css` only.** No hex moves into a
component sheet. Semantic aliases (`--text-secondary` etc.) already flow everywhere via `var()`, so
the two text nudges land on every surface with zero component edits — that is the "re-value, not
rewrite" the Tokens board asserts (`Tokens.dc.html:26-27`).

**AC2 grep test** (`ui/src/lib/__tests__/tokens.test.ts`, new; Node `fs`, no new tooling — the same
kind of file-reading vitest the repo already runs). It reads every `ui/src/components/*.css` and
fails if any contains a **covered** color literal:

- the neutral hexes `#131416 #1c1e21 #2a2d32 #23262a #18191c` (tokens `--cool-*`/`--bg-*`),
- the brand hexes `#ff4a1c #ff8a63` and the rgb triple `255, 74, 28` (tokens `--vermilion-*` /
  `--bloom*`),
- the danger triple `255, 84, 98` (tokens `--danger*`),
- the old and new text hexes `#828282 #626262 #9a9a9a #6e6e6e`.

Shadows on `0, 0, 0` are allowed (no color token models pure-black scrims). **Offenders this test
forces fixed now** (grep this session): `feed.css:34` `rgba(255,84,98,0.32)` → `var(--danger-border)`
(`#ff54624d`), `feed.css:36` `rgba(255,84,98,0.12)` → `var(--danger-soft)` (`#ff54621f`),
`rail.css:532` `rgba(255,74,28,0.42)` → `var(--bloom-strong)`. `feed.css:121` `#0f1113d9` is a bespoke
blur-scrim that maps to no token and is **not** in the covered set (recorded exception; a `#0f1113`
literal is allowed). No stylelint exists (`package.json` has none), so the grep vitest is the check
AC2 asks for — not stylelint.

Worked example: after the diff, `grep -R "#828282" ui/src/components` returns nothing and the test
passes; revert `--text-secondary` back to a component sheet as a literal and the test fails on that
file — non-vacuous.

### D4 — The Studio stage: what feeds it, the caption, the pills, the recents strip

New presentational component `ui/src/components/Stage.tsx`; its CSS lives in `feed.css` (the centre
column *is* the stage on Studio and the gallery on Library — one sheet owns the centre column, no new
`app.css` import). Props are data App already holds; **Stage owns no fetching.**

**Data feed.**
- *Featured* result = the selected job when one is chosen, else the latest: `jobs.find(id===selectedId) ?? jobs[0]`
  (jobs are newest-first, `App.tsx:216-219`). Passed as `job: JobSet | null`.
- *Running timer* = elapsed from `job.createdAt` while `isRunning(job.status)` (`lib/status.ts`). A
  1s `setInterval` inside Stage computes `now - Date.parse(createdAt)`, formatted `mm:ss`. No new
  data — `createdAt` and `status` are on every `JobSet` (`types.ts:196-199`). Matches
  `Main.dc.html:136` "Rendering 00:41".
- *Recents strip* = the first N (6, per `Main.dc.html:155`) of `jobs`, each a 16:9 tile drawn from
  `posterFor(result)` when a result exists else `gradientDataUri(job.id)` (original art —
  **never** a provider `url`/snapshot thumbnail, honouring the provenance rule). Clicking a tile
  calls `onSelect(job.id)`; the tile carries `data-job-anchor={job.id}` (D1 pairing).

**Caption cost text (MUST reuse `lib/cost.ts` formatting).** A new pure helper
`stageCostCaption(job): { text: string; tone: "normal" | "unpriced" }` in `lib/cost.ts`, unit-tested:

| known figures | caption | tone |
|---|---|---|
| est. **and** actual | `est. $0.43 → actual $0.41` (both via `formatUsd`) | normal |
| est. only | `est. $0.43` (no arrow — inventing an actual would be a false claim, the exact rule `MetaCard` follows, `MetaCard.test.tsx:76-91`) | normal |
| actual only | `actual $0.41` | normal |
| neither | `unpriced` | unpriced (`--warning`, matching `Main.dc.html:161`, `Library.dc.html:65`) |

The rest of the caption is `model · duration · resolution` from `job.settings`
(`formatDuration`, `lib/media.ts`) + the model display name (passed in) — mirrors
`Main.dc.html:142`.

**Action pills, wired to handlers that already exist — scoped honestly:**
- **Retry** → `onRerun(job)` = `App.tsx:494-510`, which reloads the job's exact inputs into the
  composer (model, route, prompt, preset, settings, media). Exists, unchanged.
- **Save** → **Reveal in Finder**: `revealResult(job.results.find(r=>r.localPath)?.localPath)`
  (`api.ts:688-696`, already used by `MetaCard.tsx:213-225`). In a local-first app the bytes are
  already saved; "Save" means "show me the file". The pill is **hidden when no `localPath`** (same
  guard as `MetaCard.tsx:213`). Honest naming: I label it "Reveal", not "Save" (see Q3).
- **Extend** → prefill: switch the use-case `tab` to the Extend/Edit-video use case and attach the
  job's output as a `video` `MediaRef`, then focus the composer. Concretely a new
  `onExtend(job)` in App: `setTab(<extend use-case slug>)`, `setMedia([{ role: "video", source:
  { kind: "local", path: localPath }, name }])`, `setPrompt(job.prompt)`. The extend **model** is
  then chosen by the existing roster filter for that tab (`modelsForUseCase`, `App.tsx:291-310`) —
  `veo3_1_extend` landed in S9 and lives on the `EditVideo` tab. **Scope guard (MUST):** the Extend
  pill is **hidden** when (a) no result has a `localPath`, or (b) `useCases` contains no
  edit-video/extend slug. This is the honest floor: it prefills the source clip and lands the user
  on the right tab; it does not force-select a specific route. Flagged as a weak point (§8, Q4).

**Empty state** (no jobs) → the existing `EmptyState` (`ResultsFeed.tsx:62-71` pattern), heading
"Nothing generated yet".

### D5 — Composer (AC5), and why the picker/preview/enhancer tests still pass

The composer is `SettingsRail` re-ordered and restyled to the mockup (`Main.dc.html:72-129`). The
child components (`PromptCard`, `EnhancerPicker`, `ChipRow`, `GenerateButton`, `PromptPreview`) are
**unchanged in DOM** — only the surrounding `.rail-card` layout and their CSS surfaces move — so
`EnhancerPicker.test.tsx`, `PromptPreview.test.tsx`, and the enhancer/preview behaviour stay green
(no assertion touches layout).

- **Model row → tile (`ModelRow.tsx`).** Restyle to the mockup tile (`Main.dc.html:76-87`):
  thumbnail (`gradientDataUri(model.id)` — original), `.model-item-name`, a `.chip-route` provider
  chip (`route.provider`), a cap line (`up to {max duration}s` from `capabilities`), and a "Change"
  affordance. **MUST keep the `.model-item*` classes** (AC5): the tile renders as
  `<button class="model-item model-item-composer">` reusing `.model-item-name`; the picker's
  grid-tile rules are scoped under `.model-list .model-item` (§D7) so the composer tile does not
  inherit the 5-col grid. To show the cap/provider, `ModelRow` gains `route`/`capabilities` props
  (App already has both, `App.tsx:604-605`). Price is **not** duplicated on the tile — the
  authoritative price stays on the sticky Generate (Q2). No `ModelRow` test exists, so this is free
  of pins.
- **Camera strip (`Main.dc.html:100-109`).** A new `.camera-strip` block in `SettingsRail`,
  between the prompt and the chips, showing up to 3 `camera-control` presets as `CameraPreview`
  tiles + a "+N more" tile. Data: the `presets` array filtered to `category === "camera-control"`
  (the S11 "25 camera presets"). `SettingsRail` gains a `cameraPresets: PresetFamily[]` prop
  (computed in App from `presets`) and an `onPickPreset(p)` prop (wired to `setPresetId`,
  `App.tsx:662`); "+N more" calls the existing `onOpenPresets` (`App.tsx:623`). Reuses `CameraPreview`
  + `camera-preview.css` verbatim — `CameraPreview.test.ts` untouched.
- **ChipRow / Generate / legal notice.** `ChipRow` unchanged (`ChipRow.tsx`). Sticky Generate =
  existing `GenerateButton`, `.generate-button` kept (`GenerateButton.tsx:60`), positioned sticky at
  the composer foot via a `.rail-generate-sticky` wrapper (CSS only). Legal notice
  (`SettingsRail.tsx:249-252`) stays as the last child, always visible, `--text-tertiary` 10px
  (Tokens board pin `Tokens.dc.html:92`).

### D6 — Library (AC6): gallery over `jobs`, lightbox via Overlay, meta rail reused

New component `ui/src/components/Library.tsx`; CSS in `feed.css` (the centre column). Rendered by App
when `view === "library"`, alongside the existing `MetaRail` in the meta track.

- **Gallery grid** = `jobs` (D5.1 — no new command), 16:9 tiles (`--tile-thumb-ratio`), caption =
  title (prompt first line) + cost via `stageCostCaption` (D4) or `"unpriced"`
  (`Library.dc.html:60-71`). Thumbnail = `posterFor(result)` else `gradientDataUri(job.id)`. Each
  tile carries `data-job-anchor={job.id}` (D1 pairing) and `onClick` → open lightbox + `onSelect`.
- **Filters** All / Video / Image / Failed (`Library.dc.html:52-57`): client-side over `jobs` by
  result `kind` (`types.ts:176`) / `isFailure(status)` (`lib/status.ts`). Local `useState`.
- **Lightbox** = the existing `Overlay` (`Overlay.tsx`, `variant="full"`) showing the selected
  result large (reuse `VideoPreview`/`<img posterFor>` as `ResultCard` does, `ResultCard.tsx:97-116`).
  New App state `lightboxId: string | null`; opened from a tile, closed by Overlay's own
  backdrop/Escape (already built, `Overlay.tsx:34-56`). No new modal machinery.
- **Meta rail** = the existing `MetaRail` + `MetaCard` verbatim (`MetaRail.tsx`, `MetaCard.tsx`) —
  it already carries the spend meter (`sessionSpend`, `MetaRail.tsx:28-65`) and the S10
  estimated/actual/settled/priced-from card. `MetaCard.test.tsx` untouched. This is why the meta
  rail belongs on Library, not Studio: the S10 evidence surface is the Library's right column
  (`Library.dc.html:75-119`).

### D7 — Model picker overlay → 5-col tile grid (AC7), DOM unchanged

**CSS-only for the pinned structure.** `ModelPicker.tsx` keeps its `<li class="model-item">` →
`<button class="model-item-main">` + `<button class="chip-route">` skeleton exactly
(`ModelPicker.tsx:52-104`) so `ModelPicker.test.tsx` passes unchanged. `picker.css` changes:
`.model-list` from `flex-column` (`picker.css:206-213`) to
`display:grid; grid-template-columns: repeat(5, minmax(0,1fr))`; `.model-item` from a row
(`picker.css:215-223`) to a vertical tile (thumbnail on top via a new `.model-item-thumb` element,
name/chip below). Picker-grid rules are scoped under `.model-list .model-item` so the composer tile
(§D5) is unaffected.

**Additive TSX (assertions stay additive, per AC7):**
- a `.model-item-thumb` div backed by `gradientDataUri(model.id)` (original art; models carry no
  thumbnail URL — `Model` has none, `types.ts:58-72` — so there is nothing to accidentally hotlink);
- `data-tier` derived from the existing `model.isLaunch` (`data-tier={model.isLaunch ? "frontier" : "standard"}`)
  for the FRONTIER badge styling (`ModelPicker.dc.html:61-99`) — **no new Rust/`Model` field**; the
  existing `LAUNCH` badge (`ModelPicker.tsx:69-71`) stays;
- a filters row (All providers / Runnable only / Audio / 30s / Sort — `ModelPicker.dc.html:50-56`)
  as local `useState` client-side filters. Additive; no existing assertion depends on it.

The two honest dead states are already correct and pinned — `data-unrunnable` on the `<li>` and
`data-unavailable`/`disabled`/`title` on the chip (`ModelPicker.tsx:56,90-94`) — and only need the
tile styling (`ModelPicker.dc.html:117-133`).

### D8 — The AC1 fixes, exact edits

1. **ConfirmDelete out of the titlebar button.** In `App.tsx`: delete the Settings `<button>` block
   (`:576-592`) — its `onClick={()=>setOverlay("settings")}` moves to the nav "Keys" item (§D1) —
   and render `<ConfirmDelete .../>` (`:582-589`) as a sibling of the other overlays, after
   `<Settings/>` (`:687-697`). Result: no interactive element nests another.
2. **Delete files.** `rm ui/src/components/ModelBrowser.tsx ui/src/components/browser.css`. Confirm
   no import breaks (grep: none). `lib/browser.ts` and its test stay.
3. **`--color-vermilion`.** `meta.css:274` → `accent-color: var(--accent);`.
4. **`.btn-quiet`.** `ResultsFeed.tsx:53` → `className="btn btn-ghost btn-sm feed-error-dismiss"`
   (`.btn-ghost` exists, `base.css:178-180`).

**AC1 guard test** (`ui/src/lib/__tests__/fixes.test.ts`, new): a static grep vitest asserting
`--color-vermilion` appears in **no** `ui/src/**/*.css`, `btn-quiet` appears in **no**
`ui/src/**/*.tsx`, and `ModelBrowser.tsx`/`browser.css` do **not** exist (`fs.existsSync` false).
Robust and non-flaky. The button-in-button itself is proved by the App-level test in §5 (AC1 row).

---

## 3. Files touched (complete list; nothing else changes)

Tokens/layout: `ui/src/tokens.css`, `ui/src/app.css`.
Sheets: `ui/src/base.css` (only if a shared primitive needs the new radius names — likely none;
`N/A` otherwise), `ui/src/components/{rail,feed,meta,picker}.css`. `camera-preview.css`,
`setup.css` **unchanged** (`N/A — reused verbatim`).
Components new: `Stage.tsx`, `Library.tsx`.
Components edited: `App.tsx` (view state, nav, overlays, Extend/lightbox wiring), `WorkspaceTabs.tsx`
(nav variant + icons), `ModelRow.tsx` (tile), `ModelPicker.tsx` (thumb/data-tier/filters, additive),
`SettingsRail.tsx` (camera strip + sticky generate wrapper + prop additions), `ResultsFeed.tsx`
(btn fix; note ResultsFeed is Studio's recents source or is replaced by Stage — see below).
Deleted: `ModelBrowser.tsx`, `browser.css`.
Libs: `lib/cost.ts` (+`stageCostCaption`). No other lib changes.
Tests new: `lib/__tests__/tokens.test.ts`, `lib/__tests__/fixes.test.ts`, `Stage.test.tsx`,
`Library.test.tsx`, `App.test.tsx`, `lib/__tests__/cost.test.ts` (+cases). 
Docs: `CHANGELOG.md`, `README.md` (if it documents the UI), `.forge/.../document.md`.

**ResultsFeed's fate (recorded, not silently dropped):** the centre column is the Stage on Studio and
the Library gallery on Library. `ResultsFeed` (the old flat feed) is **no longer mounted** by App;
its `btn-quiet` fix still lands (the file stays for the submit-error banner, which the Stage reuses —
or the banner moves into Stage and `ResultsFeed` is deleted too). **Default: keep `ResultsFeed.tsx`,
mount its error banner inside Stage** so the submit-error path (`App.tsx:637-638`, `submitError`) is
not lost. Q5 records the alternative.

**Rust:** `N/A — no Rust file changes` (§0.1). `cargo test` count therefore unchanged.

---

## 4. Data flow (Studio, end to end)

`listJobs()` → `App.jobs` (newest-first) → App picks `featured = jobs[selectedId] ?? jobs[0]` and
`recents = jobs.slice(0,6)` → `<Stage job={featured} recents={recents} modelName=… estimate=…
onSelect=setSelectedId onRetry=onRerun onExtend=onExtend onReveal=revealResult />`. Selecting a
recents tile sets `selectedId`; the pairing effect (`App.tsx:544-551`) scrolls the strip tile
(`data-job-anchor`) and no-ops the absent meta anchor; Stage re-renders `featured`. Cost caption is
`stageCostCaption(featured)`; running timer ticks from `featured.createdAt`.

Library: same `jobs` → `<Library jobs=… filter=… onOpenLightbox=setLightboxId onSelect=setSelectedId />`
+ `<MetaRail jobs=… models=… selectedId=… onSelect=… onRerun=onRerun onDelete=onDelete />`. Lightbox
is `<Overlay open={lightboxId!==null} …>`.

---

## 5. Test plan — every AC to a named vitest test

The repo has **no** React Testing Library; all component tests render with `react-dom/client` under
jsdom, the established no-RTL pattern (`ModelPicker.test.tsx:19-81`). New tests MUST follow it.

| AC | test (file) | asserts |
|---|---|---|
| AC1 | `no dangling token/class, dead files gone` (`lib/__tests__/fixes.test.ts`) | `--color-vermilion` absent in all `components/*.css`; `btn-quiet` absent in all `*.tsx`; `ModelBrowser.tsx` + `browser.css` do not exist |
| AC1 | `ConfirmDelete is not nested in a button` (`App.test.tsx`) | render `<App/>`, flush effects; `document.querySelectorAll("button button").length === 0`; the Settings/Keys trigger contains no `[role="dialog"]` |
| AC2 | `component sheets use tokens, not covered hexes` (`lib/__tests__/tokens.test.ts`) | grep every `components/*.css` for the covered hex/rgb set (§D3) ⇒ none; non-vacuous (fails if a literal is reintroduced) |
| AC2 | `new tokens are defined` (`lib/__tests__/tokens.test.ts`) | `tokens.css` contains `--rail-nav`, `--radius-card`, `--radius-panel`, `--tile-thumb-ratio`, `--text-secondary:#9a9a9a`, `--text-tertiary:#6e6e6e`, `--rail-settings:400px` |
| AC3 | `nav drives the use-case tab` (`App.test.tsx`) | clicking a nav item (`[data-tab=…]`) calls the same `onChange` that changed `tab`; keyboard roving still works (arrow key moves selection) |
| AC3 | `meta rail shows on Library, hides on Studio` (`App.test.tsx`) | toggle `view`: on Studio `.rail-meta` is absent and `.workspace[data-view="studio"]`; on Library `.rail-meta` present and `.workspace[data-view="library"]`; pairing effect throws on neither |
| AC4 | `stage caption: estimate only vs actual known vs unpriced` (`lib/__tests__/cost.test.ts`) | `stageCostCaption` → `est. $X` (no arrow) / `est. $X → actual $Y` / `actual $Y` / `unpriced` with tone |
| AC4 | `stage: featured, recents, running timer, empty` (`Stage.test.tsx`) | latest job rendered large; a running job shows a `mm:ss` timer; N recents tiles carry `data-job-anchor`; no jobs → empty state; Extend/Reveal pills hidden without a `localPath` |
| AC5 | (reuse) `EnhancerPicker.test.tsx`, `PromptPreview.test.tsx` | still green — DOM unchanged |
| AC5 | `composer keeps .generate-button, .model-item*, legal notice` (`Stage`/`SettingsRail` — add `SettingsRail.test.tsx`) | render `SettingsRail`; `.generate-button` present, model tile uses `.model-item`/`.model-item-name`, camera strip renders ≤3 `.cam-frame` + a "more" affordance, legal-notice text present |
| AC6 | `library gallery + lightbox + filters` (`Library.test.tsx`) | jobs render as tiles with cost/"unpriced" captions and `data-job-anchor`; clicking a tile opens the Overlay lightbox; Failed filter shows only failed jobs |
| AC6 | (reuse) `MetaCard.test.tsx` | still green |
| AC7 | (reuse) `ModelPicker.test.tsx` | still green — `li.model-item`, `button.model-item-main[disabled]`, `button.chip-route[data-unavailable]`, `onSelect` all unchanged |
| AC7 | `picker tiles carry data-tier and a thumbnail` (`ModelPicker.test.tsx`, additive `it`) | a launch model's `<li>` has `data-tier="frontier"`; every tile has a `.model-item-thumb`; assertions are additive only |
| AC8 | the gates (§6) + screenshots in `test.md` | see §6 |

**Screenshots (AC8) — how `test.md` attaches them.** There is **no automated screenshot tool** in
this repo. The evidence path is: `ui/node_modules/.bin/tauri build --bundles app` from the repo root
produces `src-tauri/target/release/bundle/macos/Hickeyfield.app`; the VERIFY agent launches it,
navigates to Studio and to Library, and captures the app window (macOS `screencapture -l<windowid>`
or Cmd-Shift-4 space-click, done by hand) into `test.md`. If the environment cannot capture a
window, `test.md` records that and the verifier **describes** the two screens against the mockups
instead — the honest fallback, stated so a missing PNG is not read as a failed build.

**Mutation checks the TEST agent should run** (named so they are not invented late): revert
`--text-secondary` to `#828282` ⇒ the tokens test still passes but the grep test must NOT flag it
(it is in `tokens.css`, not a component) — instead put `#828282` into `feed.css` ⇒ the grep test must
fail; remove `data-view` from `.workspace` ⇒ AC3 rail-visibility test fails; re-nest ConfirmDelete in
a button ⇒ AC1 `button button` test fails; change the picker `<li>` to a `<div>` ⇒
`ModelPicker.test.tsx:84` (`li.model-item`) fails.

---

## 6. Gate commands (AC8), exact

Node 24 is at
`/private/tmp/claude-501/-Users-jorge-Documents-GitHub-hickeyfield/79be25e2-8f71-4192-9e6d-cd0e9d068f1c/scratchpad/node-v24.8.0-darwin-arm64/bin`
(prepend to `PATH`).

```
cd ui && pnpm test          # vitest run — 0 failed, >= 245 (contract floor); new tests add to it
cd ui && pnpm build         # tsc --noEmit clean + vite build succeeds
./scripts/lint-provenance.py            # no CDN host, no vendor copy — 0
cargo test --workspace                  # unchanged: >= 820, 0 failed (no Rust touched)
ui/node_modules/.bin/tauri build --bundles app   # from repo root; the .app launches (window, not blank)
```

`cargo fmt --all --check` / `cargo clippy --workspace --all-targets -- -D warnings` /
`cargo deny check` are **N/A to re-argue** — no Rust or manifest changes — but MUST still be green
(they were at S10 release). **Never** `cargo run --example first_light` (spends money, no key).
**Never** bare `cargo build` for a runnable app (README: blank window).

Expected count deltas: vitest **grows** by the new tests (fixes, tokens, Stage, Library, App,
SettingsRail, cost cases); nothing is removed except `ModelBrowser` (which has no test — its logic
lives in `lib/browser.ts`, whose test stays). `cargo test` **unchanged**.

---

## 7. Order of work (safest seams first; each step leaves `pnpm test`/`pnpm build` green)

1. **AC1 fixes** (§D8) + `fixes.test.ts`. Smallest, unblocks everything, no layout risk.
2. **AC2 tokens** (§D3): edit `tokens.css`, fix the three rgba offenders, add `tokens.test.ts`.
3. **base.css** — only if a shared primitive needs a new radius name (likely `N/A`).
4. **feed.css gallery + Library.tsx** (§D6): build the gallery/lightbox over `jobs`; reuse MetaRail.
5. **picker.css tile grid + ModelPicker additive** (§D7): CSS-first, then `data-tier`/thumb/filters.
6. **rail/nav + WorkspaceTabs variant + app.css grid + `<1200px` breakpoint + App view state**
   (§D1/D2): the layout switch. Add the nav track and the two templates; **never** rewrite the 3-col
   rule first.
7. **Stage.tsx + composer restyle (SettingsRail, ModelRow, camera strip)** (§D4/D5): the last and
   most novel surface.
8. **Docs** (§9), then `tauri build` + screenshots.

This is the contract's stated order (`slice.md:52-54`) with the Library and picker pulled before the
stage, because they reuse existing components and de-risk the two genuinely-new files.

---

## 8. Riskiest parts (where the verifier should start)

1. **The Extend pill (D4).** It is the one pill that is not a straight call to an existing handler:
   it prefills a `video` `MediaRef` from a `localPath` and switches tabs, and whether the resulting
   roster actually lands on `veo3_1_extend` depends on `modelsForUseCase` for the extend tab — which
   this design could not run. The mitigation is the hide-guard (no clip / no extend use-case ⇒ no
   pill), so the worst case is a missing pill, not a broken one. **Weakest link.**
2. **`.model-item` reused in two layouts (D5/D7).** The composer tile and the picker tile share the
   pinned classes; if the picker's grid rules are *not* scoped under `.model-list`, the composer tile
   inherits a 5-col grid and breaks. The scoping is stated as MUST but is easy to get wrong.
3. **The grid switch + pairing (D1/D2).** Mounting `MetaRail` only on Library relies on the pairing
   effect no-opping a missing anchor. It does today (`?.`), but any future change that assumes both
   anchors exist would strand Studio. The AC3 test pins that the effect throws on neither view.
4. **AC1 button-in-button proof.** With `ConfirmDelete open={false}` returning `null`, a naive
   at-rest test cannot distinguish the bug. The App-level test must render with the fix in place and
   assert the structural invariant; the verifier should confirm the test would fail if ConfirmDelete
   were re-nested (mutation check, §5).
5. **`--text-secondary` contrast (D3).** `#828282 → #9a9a9a` lightens secondary text everywhere at
   once. Owner-approved (`S11/document.md:8`), but it touches every sheet via `var()`; a visual
   regression would show up only in the `tauri build` screenshots, not in vitest.

---

## 9. Documentation (the DOCUMENT step, after code lands)

- **`CHANGELOG.md`** — one bullet under `## [Unreleased]` `### Changed` (`:7-9`), user-facing voice:
  Hickeyfield's window is redesigned into a stage-first Studio (latest result large with its
  estimated-and-actual cost, a recents strip, a use-case nav) and a Library gallery with a lightbox
  and the per-generation cost card; the model picker is a tile grid. No behaviour or pricing change.
- **`README.md`** — checked at implement time for a UI/screenshot section; if one exists, update the
  layout description (and screenshot if the README ships one), else **N/A**.
- **`docs/`** — `N/A`: `BRIDGE.md`/`PARITY.md`/`FIRST-LIGHT.md`/`HARNESS-WIRING.md` describe the
  bridge, parity, live-run and harness; none is a UI-layout doc and this slice changes no command.
- **`document.md`** — written after the docs land, recording file + section for each change and the
  weakest parts for the verifier.

---

## 10. Non-goals

- **N1.** No new generation feature, no new Rust command, no persistence change (§0.1). The Library
  is a re-presentation of `jobs`.
- **N2.** No Storyboard. The titlebar shows Studio/Library only; the mockup's greyed "Storyboard"
  (`Main.dc.html:30`) is **not** rendered (a dead control teaches nothing). Default recorded in Q1.
- **N3.** No light theme (`tokens.css:12-14` — the app is dark-only and both themes emit identical
  values). Untouched.
- **N4.** No new `Model` field (tier, thumbnail URL). `data-tier` derives from `isLaunch`; thumbnails
  are `gradientDataUri`. No `list_models` DTO change.
- **N5.** No router, no state-management library, no CSS framework, no stylelint. AC2's check is a
  grep vitest.
- **N6.** No change to `submit`/estimate/enhancer/preview behaviour — those components are reused
  DOM-unchanged so their tests stand.

---

## 11. Open questions (each with the default this build assumes)

- **Q1 — Storyboard in the titlebar nav?** **Default: no** — Studio/Library only; Storyboard is a
  future slice and a disabled tab is a dead control (N2). One line to add later if the owner wants a
  visible "coming soon".
- **Q2 — price on the composer model tile?** **Default: no** — the authoritative estimate is on the
  sticky Generate (`GenerateButton`); a second price on the tile would be a second number to keep in
  sync. The tile shows provider + a cap string. Reversible (the estimate is already in `SettingsRail`
  props).
- **Q3 — label the third pill "Save" or "Reveal"?** **Default: "Reveal in Finder"** — the file is
  already on disk in a local-first app, so "Save" would imply an action that has already happened.
  The mockup says "Save" (`Main.dc.html:147`); if the owner prefers that word it is a one-string
  change with the same `revealResult` wiring.
- **Q4 — how far does Extend go?** **Default: prefill the source clip + switch to the extend tab,
  pill hidden when there is no clip or no extend use-case** (D4). Auto-selecting the exact
  `veo3_1_extend` route is deferred — it needs the roster call this design could not run, and a wrong
  auto-select is worse than landing the user on the right tab.
- **Q5 — delete `ResultsFeed.tsx` or keep it for the error banner?** **Default: keep it**, mount its
  submit-error banner (`ResultsFeed.tsx:44-60`) inside `Stage`, so the refusal path (`App.submitError`,
  `App.tsx:637-638`) is not lost. Deleting it would strand that banner; a follow-up can inline it.
- **Q6 — `<1200px` meta as a sheet vs a stacked column?** **Default: `Overlay variant="sheet"`**
  (already built, `Overlay.tsx`/`picker.css:40-43`) rather than a new responsive column — reuses the
  one modal we have and keeps the breakpoint additive.
