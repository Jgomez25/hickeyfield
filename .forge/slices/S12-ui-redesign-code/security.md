# S12-ui-redesign-code — SECURITY (evidence)

Read-only pass over the slice diff (`git diff main...HEAD`). Scope confirmed
**UI-only**: `--stat` shows no `.rs`, no `Cargo.*`, no `package.json`/lockfile,
no `tauri.conf`. No new IPC command, no new dependency, no supply-chain change.
`revealResult` and `estimateCost`/submit paths are pre-existing `api.ts` code
(file untouched by this slice). Findings are ranked; where a boundary was
examined and clean it is stated explicitly.

## Boundaries examined — clean

### Injection / rendering of provider & job data
- **`dangerouslySetInnerHTML`, `innerHTML`, `eval`, `new Function`**: grep across
  `ui/src` returns none. All new markup is JSX with text children (React
  auto-escapes). No HTML string is ever constructed from job/provider data.
- **Media `src` / `href`**: every rendered media URL flows through the existing
  guard. `Stage.tsx:228` and `Library`/lightbox (`App.tsx:892` `Lightbox`) use
  `posterFor(first)` → `displayUrl` (`lib/media.ts:25`), which prefers the
  local `convertFileSrc` path and only falls back to the provider URL — the same
  localPath-first rule ResultCard used before. Video goes through the unchanged
  `VideoPreview` + `isPlayableVideo`. No job/result URL is placed in an `href`.
- **CSS `background-image` from a URL** (`Stage.tsx:160`, `Library.tsx:123`,
  `ModelRow.tsx`, `ModelPicker.tsx`): interpolated as `url("${...}")` into an
  inline `style` object. Values are either `gradientDataUri(...)`
  (`encodeURIComponent`-escaped data URI, no bare `"`) or `posterFor` output.
  React assigns these via the CSSOM `element.style.backgroundImage` property; a
  crafted provider URL containing `")` cannot add new declarations or rules
  through a single-property assignment (an invalid value is dropped, not
  parsed as extra CSS). No CSS-injection or style-breakout path. Examined, clean.
- **Prompt / caption text** (`titleOf` in `Stage.tsx:33`, `Library.tsx:35`):
  rendered as JSX text nodes — escaped, not interpreted. No prompt-injection
  surface (no fetched content is fed to an LLM here).

### The `thumbnailUrl` rule (never render fal snapshot thumbnails)
- `grep thumbnailUrl ui/src` → **no matches anywhere**. Every model tile
  thumbnail is `gradientDataUri(model.id)` — original, deterministic, inline SVG
  from `lib/placeholder.ts` (`ModelRow.tsx`, `ModelPicker.tsx:104`). Stage/Library
  tile art is `posterFor` of the user's own render, else `gradientDataUri`.
  Rule holds. `lint-provenance.py` PASS (no third-party media hosts, no copy
  match).

### Money-safety: Extend / Retry pills bypass no estimate/confirm path
- **Retry** (`Stage.tsx:259` → `onRerun`, `App.tsx:504`) and **Extend**
  (`Stage.tsx:251` → `onExtend`, `App.tsx:545`) both only *prefill the composer*
  (setModelId/setPrompt/setMedia/setTab and focus the prompt). Neither calls
  `onSubmit`/`estimateCost` or invokes a job. Submission remains solely through
  `GenerateButton` in `SettingsRail` (`SettingsRail.tsx:280`, now inside
  `.rail-generate-sticky` — a wrapper `<div>`, no behavior change). No new
  paid-submit path; the priced/estimate gate is unchanged. Clean.

### Cost text: unknown never shown as $0.00 (S10 rule)
- `lib/cost.ts` `stageCostCaption` (used by Stage caption `Stage.tsx:203` and
  Library tile `Library.tsx:105`) returns `"unpriced"` when neither estimate nor
  actual is a usable non-negative finite number — never `"$0.00"`. `sessionSpend`
  / `spendQualifier` / `totalSpend` exclude unknowns from the sum and count them
  separately. `formatUsd` only emits `"$0.00"` for a genuine `0`, never for
  null/undefined/negative/NaN (those → `UNKNOWN_COST_LABEL`). Library header and
  Stage tile surface an explicit "unpriced" count. Rule holds.

### Deleted `ModelBrowser.tsx` / `browser.css`
- Both deleted in the diff. `grep` for `ModelBrowser` / `browser.css` finds only
  assertions in `lib/__tests__/fixes.test.ts` proving their absence — no live
  import references them. Same for the dangling `--color-vermilion` / `.btn-quiet`
  (removed; `ResultsFeed.tsx:53` reclassed to `btn-ghost btn-sm`).

### Secrets / internal paths in new fixtures
- New test files (App/Library/ModelPicker/SettingsRail/Stage/WorkspaceTabs
  `.test.tsx`, `cost/fixes/tokens` tests) contain no keys, tokens, secrets, or
  real filesystem paths. The `sk-...` / `/Users/...` grep hits are all in
  *pre-existing* files this slice did not touch (`env-import.test.ts`,
  `providers.test.ts`, `media*.test.ts`) and are synthetic fixtures — one
  (`STRIPE_SECRET_KEY=sk_live_supersecret`) exists specifically to assert the
  value is never echoed. Not this slice's surface.

### Authz / egress / config
- No tenant/authz logic touched. No new network egress: the only outbound URL a
  provider can influence is the media fallback in `displayUrl`, which is the
  unchanged localPath-first behavior. `revealResult`/`reveal_result` is a
  pre-existing desktop-only IPC; the path handed to it is a backend-produced
  `result.localPath`, not free user input. No default-open config, no
  destructive op added (ConfirmDelete still gates deletion; it was only moved out
  of the titlebar `<button>` to fix invalid nesting — an HTML-correctness change,
  not a gate change).

## Findings
None at Critical, High, Medium, or Low severity. No new backlog slices
recommended.

VERDICT: PASS
