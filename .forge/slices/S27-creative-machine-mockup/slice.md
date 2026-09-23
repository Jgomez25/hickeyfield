# S27-creative-machine-mockup — SLICE (DISCOVER + PLAN)

Owner go 2026-09-22 (two-lane recommendation: S26 + S27 in parallel). Owns objective **R-UX.1, stage 1**
("mockups precede code, STAGED: undesigned screens first, polish second"). Branch
`forge/s27-creative-machine-mockup`, own worktree. **NO source change** — boards + evidence only, like S11.
The storyboard / ad flow is the owner's stated pain point, so it comes first.

**WORKHORSE = `minimax_h3_max_turbo`** (fal `minimax/h3-max-turbo/{text,image}-to-video`, $0.0125/s 480p,
$0.02/s 768p, $0.04/s 1080p — promotional rate as of 2026-09-16; native audio + lipsync; bills on OUTPUT
seconds: 6 s asked → 6.592 s → $0.13184 at 768P). It leads every model mention, default and sample number
on every board. **Seedance and the Grok paid API are never used** — they appear nowhere on the boards
(they are hidden in the app behind the S20b toggle).

## Delivers (one sentence, end-to-end)
The owner reviews and approves one design canvas that shows, in the approved S11 direction, the screens
Extension R will build — Ad Studio / Storyboard, Characters, Library sets, Variant matrix, Finishing and every
money surface — before any of them is coded.

## Acceptance criteria (testable — the tester will run these)
- [ ] **A1 boards.** A published Design canvas with one board per surface, each captioned with the R-ids it
      serves and the future slice ids that will build it:
      1. **Ad Studio / Storyboard** (S40–S44, S19, S33): brief → script → editable shot list bound to route
         classes; per-shot model (workhorse default), per-shot cost, "cheaper route" suggestion, character
         binding, per-shot trim handles, continuity status, render.
      2. **Characters** (S29–S32): anchor card, reference grid with EMPTY-cell gaps, voice card, "New still" /
         "New clip" / "Promote to anchor", consent state (synthetic-disclosed vs real-consented).
      3. **Library sets** (R-UX.1): results grouped into sets (storyboard renders, variant batches, character
         assets) inside the S11/S12 Library.
      4. **Variant matrix** (S45–S48): hook × line × character × aspect × length × caption-style axes, the
         de-duplicated variant count, TOTAL and MARGINAL cost per variant, budget-cap status, export.
      5. **Finishing** (S35–S39): captions, audio bed + ducking, named post-look presets, logo / end-card,
         aspect variants 9:16 · 4:5 · 1:1 · 16:9.
      6. **Money surfaces** (S21–S25, S34): estimate RANGE `{low, expected, high, confidence}`, "re-priced" vs
         "billed" labels, drift alarm, per-day + per-project budget cap (refusal state), ledger (per project and
         per character, kept vs discarded), dated free-web-sandbox hint, promo badge, est-vs-settled delta.
- [ ] **A2 direction.** Extends the owner-approved S11 direction exactly: dark studio, vermilion accent,
      Bebas Neue / Space Grotesk / Inter / IBM Plex Mono, 72px icon rail, stage-first Studio, meta rail hidden
      on Studio. Token VALUES are taken from `ui/src/tokens.css` (the real shipped layer), not re-invented.
- [ ] **A3 honest sample data.** Every number on the boards is consistent with the real pricing above; ranges
      follow the spec priors (per-second ±10%); "Unknown" is never shown as $0; settled figures are labelled
      "re-priced" unless the source is billed; no invented overrun constant. The workhorse leads every list.
- [ ] **A4 neutral content (R-BRAND).** Invented neutral brand, product, persona and copy only — no real
      brand, person or private name from the owner's notes; no imitation of a real organisation. The
      mockup working files live in the repo, so `./scripts/lint-provenance.py` must pass and a grep for the
      private names (run against the owner's notes, never written into the repo) finds nothing.
- [ ] **A5 original assets.** No third-party product copy, media or hotlinks (Higgsfield or any other); no
      external image hosts; web fonts only from Google Fonts; images are CSS/SVG placeholders or original.
- [ ] **A6 open questions on the canvas.** Each board carries its 1–3 open design questions as notes for the
      owner (e.g. where the variant matrix lives; how a budget-cap refusal looks). Answers recorded in
      `document.md`. **Owner-run approval gate:** the slice releases only after the owner approves the boards
      or lists changes.
- [ ] **A7 no source change.** `git diff --stat` touches only `.forge/slices/S27-creative-machine-mockup/**`
      (+ the usual `.forge` bookkeeping at release).

## Out of scope
Stage 2 polish of the existing Studio / picker / Library (S50); any code (S28 does the storyboard CSS
groundwork from these boards); the S26 brand-pack Settings surface (S26 builds its own minimal one).

## Approach
DESIGN agent plans the boards (surface → content → sample data → open questions) grounded in the S11
boards (`.forge/slices/S11-ui-redesign-mockup/mockup/`), the shipped UI (`ui/src/**`) and the Extension R
criteria. IMPLEMENT writes the board files under `mockup/`; the orchestrator publishes them to a new Design
canvas (the Artifact tool is orchestrator-only). TEST/REVIEW/SECURITY verify the boards against A1–A7.
