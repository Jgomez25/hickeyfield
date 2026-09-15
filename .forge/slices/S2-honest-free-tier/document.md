# S2-honest-free-tier — DOCUMENT

Upstream read for this phase: `.forge/objective.md` (§B, and Q4 on the free-tier
honesty fix), `.forge/slices/S2-honest-free-tier/slice.md` (AC1–AC4), `design.md`,
`implement.md`, `test.md`, `README.md`, `docs/PARITY.md`, `docs/HARNESS-WIRING.md`,
`docs/BRIDGE.md`, the S1 `document.md` (for changelog/format precedent), and
`scripts/lint-provenance.py`. This phase adds documentation only; it changes no
production or test code. All prose below is my own original wording.

## What S2 shipped (recap, for the reader of this file)

One production edit, `crates/hickeyfield-core/src/provider.rs`: `has_adapter()` now
returns `true` only for the providers that have a real client (`Fal | Higgsfield`);
`ProviderId::Local` was dropped because no Local client exists (`app.rs::client_for`
already returns `None`, and `clients.rs` has no Local client). That single predicate
correction propagates to the three call sites that consumed the old lie —
`commands.rs::route_state` (the availability DTO), `route::resolve` (the submit
path), and `use_case::route_serves` (use-case tab filtering). The only affected model
is `z_image`, whose sole route is `local:z-image`. Consequences: `z_image` is filtered
out of the use-case tabs like the other three unroutable models (owner's HIDE
decision, design Q2), the availability layer reports it unavailable with the honest,
reused reason `"Hickeyfield has no client for Local yet"`, and its resolver path now
returns `NoAdapter` instead of a misleading "add a key for Local" error at submit.
Pricing is unchanged: `z_image` keeps `CostModel::Flat { usd: 0.0 }` — the bug was
availability, not price. No Local/ComfyUI/Ollama client was built; that remains future
work, and the fix is written so re-adding `Local` to `has_adapter()` re-enables the
model in one edit when a client lands.

## Files changed in the DOCUMENT phase

| File | Kind | Change |
|---|---|---|
| `CHANGELOG.md` (repo root) | edited | Added one bullet to the existing `[Unreleased] › Fixed` section: the local-only free-tier model that could never run is no longer presented as an available $0.00 Generate option. |
| `docs/PARITY.md` | **not touched** | N/A — see "PARITY decision" below. |
| `README.md` | **not touched** | N/A — see "README decision" below. |
| `.forge/slices/S2-honest-free-tier/document.md` | replaced stub | This evidence file. |

## Changelog entry added (quoted verbatim)

Appended as the last bullet under `[Unreleased] › Fixed`:

```markdown
- **A model that could never actually run is no longer shown as a free option.**
  The one model served solely by the built-in "Local" provider (`z_image`) used to
  appear priced at $0.00 with a Generate button, then fail the moment you pressed
  it, because the app has no local (ComfyUI or Ollama) client to run it yet. Local
  is now treated as having no runnable client, so that model is hidden alongside the
  other routes the app cannot execute and is reported as unavailable with a plain
  reason instead of a price it cannot honour. It returns on its own once a real
  local client ships.
```

This maps to the slice's ACs: the "$0.00 with a Generate button, then fail"
description is the exact behaviour AC1/AC2 correct; "hidden alongside the other
routes the app cannot execute" records the HIDE decision (design Q2, AC2); "reported
as unavailable with a plain reason" is the `route_state` honest-reason path (AC1). It
is deliberately **not** overstated — "the app has no local … client to run it yet"
and "returns on its own once a real local client ships" both say plainly that local
inference is still future work, not something this slice delivered.

## PARITY decision — N/A (recorded, not skipped)

**N/A — `docs/PARITY.md` does not document the `z_image`/Local dishonest-availability
state as a known gap, so there is nothing to annotate.** I searched the docs for the
gap and confirmed:

- §3.3 tracks the *live end-to-end* proof gap; §4 ("Model roster") tracks *pricing*
  holes — its "never as free" line is about GPT Image 2 (`CostModel::Unknown`), a
  different model and a different issue. Neither describes `z_image`/Local being shown
  as available-at-$0.00 while failing at submit.
- The only Local/ComfyUI/Ollama references in `docs/` are `docs/HARNESS-WIRING.md`
  (the prompt-rewrite LLM via Ollama — an explicitly separate, out-of-scope spec) and
  `docs/BRIDGE.md:183` ("Do we ship a local ComfyUI provider?" — an open architectural
  question). Neither documents this availability bug, so the task's "if PARITY doesn't
  mention it, N/A is fine" applies. I did not add a note claiming local inference now
  works, because it does not.

## README decision — N/A (recorded, not skipped)

**N/A — no README claim asserts a free/local tier that S2 changes.** I checked the
two candidate lines:

- The pricing pitch (lines 3–4, "no subscription, no credits, no plan gating") is
  about not paying *us* and bringing your own provider keys; it does not advertise a
  keyless local image-generation tier, so S2 leaves it exactly true.
- "either deterministically from a preset, or through a local model via Ollama"
  (line 70) is about **prompt expansion**, not image generation — it is the
  harness/prompt-rewrite path, which S2 does not touch. The `z_image`/Local image
  route is not mentioned in the README at all.

So the README is intentionally left unchanged.

## Provenance lint

**Constraint:** this DOCUMENT-phase environment exposes only file read/write/search
tools — there is no shell/execution tool — so I could **not** run
`./scripts/lint-provenance.py` myself here (same limitation the S1 DOCUMENT phase
recorded). What I can attest:

- The only prose I added is the single `CHANGELOG.md` bullet above; every word is my
  own, describing this app's internal behaviour (`z_image`, the built-in Local
  provider, ComfyUI/Ollama clients, the Generate affordance). None of it is lifted
  from third-party product copy.
- By the checker's own rules (`scripts/lint-provenance.py`): the copy check flags a
  line only when it shares **≥ 3** five-word shingles (`MATCH_THRESHOLD = 3`,
  `SHINGLE = 5`, `MIN_WORDS = 8`) with the Higgsfield i18n/product corpus. App-specific
  technical prose about an unrunnable local model is not marketing copy and is very
  unlikely to reach that threshold. The CDN-host check cannot trip: I introduced no
  URLs, and the changelog's only pre-existing links (`keepachangelog.com`,
  `semver.org`) are not in `FORBIDDEN_HOSTS`.
- The repo-root `CHANGELOG.md` is **scanned** (not in `SKIP_DIRS`), so my addition is
  subject to the full check, as intended. `implement.md` records the delivered tree at
  lint **exit 0** already; adding one factual bullet does not change the URL set and
  is not expected to alter that.

**Verifier action requested:** run `./scripts/lint-provenance.py` from the repo root
and confirm it still exits 0 with the edited `CHANGELOG.md` in the tree. This is the
weakest point of this phase precisely because I could not execute it here. If the new
bullet trips it, the fix is to reword that bullet (never to relax the threshold).

## Weakest parts (verifier, start here)

1. **Un-run provenance lint.** I could not execute `./scripts/lint-provenance.py` in
   this environment (no shell tool). The claim that the new changelog bullet passes is
   reasoned from the checker's rules, not observed — run it.
2. **Changelog scope / no-overstatement call.** Confirm the bullet reads as "a model
   that could never run is no longer offered as free," and that "once a real local
   client ships" is not misread as claiming local inference exists now. It does not.
3. **PARITY N/A judgement.** Confirm PARITY genuinely does not document this specific
   gap (as opposed to the pricing/live-proof gaps it does track), so leaving it
   untouched is correct rather than a missed annotation.
