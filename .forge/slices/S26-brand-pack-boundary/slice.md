# S26-brand-pack-boundary — SLICE (DISCOVER + PLAN)

Owner go 2026-09-22 ("continue" on the two-lane recommendation: S26 + S27 in parallel). Owns objective
**R-BRAND.1–4** and settles **Q-R4** (default root `~/Documents/hickeyfield-brand-packs/<brand>/`,
configurable). Branch `forge/s26-brand-pack-boundary`. Parallel lane with S27 (mockup, no source change).

**WORKHORSE = `minimax_h3_max_turbo`** (fal `minimax/h3-max-turbo/{text,image}-to-video`, $0.02/s at
768P, native audio + lipsync, bills on OUTPUT seconds). Any sample template, sample shot or example in
this slice that names a video model names the workhorse. Seedance and the Grok paid API are never used.

## Why this slice
The repo is an AGPL generic engine. Everything brand- or client-specific (hooks, scripts, ad structures,
claims rules, characters, logos) must live OUTSIDE the repo in private brand packs loaded at runtime.
R2 (characters), R4 (ad studio) and R5 (multi-ad) all load packs, so the boundary lands first.

## Delivers (one sentence, end-to-end)
The app discovers and validates brand packs from a private folder outside the repo (path changeable in
Settings), ships only a neutral sample pack, and a repo lint fails if any private pack's material appears in
tracked files.

## Acceptance criteria (testable — the tester will run these)
- [ ] **A1 runtime root (R-BRAND.1, Q-R4).** Core resolves the brand-pack root: persisted setting if set,
      else `~/Documents/hickeyfield-brand-packs/`. Reuse the S20b `app_pref` store for the setting (no new
      migration unless the design proves it necessary). A missing root is a normal empty state, never an
      error. Nothing under the root is ever copied into the repo or the app bundle. [test]
- [ ] **A2 pack format (R-BRAND.2).** A documented, VERSIONED pack format: `manifest` (id, display name,
      format version) + sections for templates, characters, assets, claims rules. Sections that later slices
      own (templates → S40, characters → S29) are defined minimally now and extend additively. A schema
      validator returns structured, per-file errors (never panics) for: bad/missing manifest, unknown
      format version, malformed section files, and asset paths that escape the pack folder (`..`, absolute
      paths, symlinks pointing outside). Documented in a repo doc. [test]
- [ ] **A3 neutral sample only (R-BRAND.3).** Exactly one pack ships in-repo, a neutral invented sample
      (no real brand, person, product, claim or place). It validates clean. Any sample ad/shot in it uses the
      workhorse. A test asserts it is the only in-repo pack and that it validates. [test]
- [ ] **A4 private-material lint (R-BRAND.4).** A new `scripts/lint-brand.py` (in the `lint-provenance` /
      `lint-spend` family) scans tracked files (`git ls-files`) and FAILS on any private material. It must
      NEVER write or embed the private names themselves in the repo: its deny-set is derived at run time from
      the packs under the brand-pack root (manifest names, character names, claim/hook/script lines) plus an
      optional private deny-list file kept under that root. With no private packs present it passes and says
      so plainly. The sample pack is exempt by path. Proven by a hermetic test that builds a throwaway pack
      with a synthetic nonsense token in a temp dir, plants the token in a temp copy of a tracked-style file,
      and sees the lint fail, then pass once removed. [lint + test]
- [ ] **A5 Settings surface.** Settings shows the brand-pack root (editable, with a reset to default) and
      the packs discovered there, each with its validation status and first error. Test-locked selectors in
      `.forge/STATUS.md` stay intact. Browser mock mode renders it. [vitest]
- [ ] **A6 no spend, no network.** Pack loading is file-only; no provider call, no upload, no new spend
      path. [test / review]
- [ ] **G1** `cargo test --workspace`, `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --
      -D warnings`, `./scripts/lint-provenance.py`, `./scripts/lint-spend.py`, `./scripts/lint-brand.py`,
      keyless `audit_fal`, `cd ui && pnpm test && pnpm build` all pass. Baseline 979 Rust / 321 UI.
- [ ] **G2 spend hygiene.** Every command ran with `HICKEYFIELD_NO_KEYCHAIN=1` + dummy `hickeyfield_*_KEY=x`
      SET; never `HICKEYFIELD_ALLOW_SPEND`; never `first_light`; hermetic tests only (temp dirs; never
      read or write the owner's real `~/Documents/hickeyfield-brand-packs/` or app DB in a test).

## Owner-run (optional, after release)
- Create a private pack for a real brand under the root and see it validate in Settings.
- Optionally seed the private deny-list file under the root with the private names from the owner's notes,
  so the lint has teeth before the first real pack exists. Agents do this only on the owner's explicit go,
  and only OUTSIDE the repo.

## Out of scope
Character data model + operations (S29+), ad-template semantics (S40), using packs in generation, pack
editing UI, encryption of packs, syncing packs anywhere.

## Approach
New core module for the pack format, validator and discovery (no Tauri dependency); thin Tauri commands; a
Settings section; a Python lint beside the existing two, wired into the documented gate list. Reuse over
new abstractions; no new dependency unless the design justifies it against `deny.toml`.
