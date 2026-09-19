# S20b-never-used-routes — SLICE (DISCOVER + PLAN)

Owner pick 2026-09-19: "do option 1, sweep seedance and grok out" → depth chosen by the owner:
**Demote + hide**. Branch `forge/s20b-never-used-routes`. S20 follow-up; serves objective **R0.3**
(workhorse leads every default/mock/fixture). Id is `S20b` so backlog S21 (image-tier-pricing) keeps its id.

**WORKHORSE = `minimax_h3_max_turbo`** (fal `minimax/h3-max-turbo/{text,image}-to-video`, $0.02/s at
768P, native audio + lipsync, bills on OUTPUT seconds). It is the ONLY paid video route the owner uses.
**Owner rules (2026-09-19, firm): Seedance is NEVER used. Grok paid API is NEVER used.** The app bills the
API, so inside this app every Seedance and every Grok route (video AND image) is a never-used route.

## Finding that motivates the slice (verified 2026-09-19)
`registry.rs` `LAUNCH_FAMILIES` (~:214) has a "Seedance 2.0" family and NO MiniMax H3 entry; the UI opens
on `m.find(x => x.isLaunch && x.modality === "video") ?? m[0]` (`ui/src/App.tsx:194`, `:336`) and the
storyboard falls back to `models[0]` (`Storyboard.tsx:210`). So the app does not start on the workhorse,
and an expensive never-used route is one click away.

## Delivers (one sentence, end-to-end)
The app opens on the workhorse, Seedance and Grok are gone from the launch set, mocks, samples and
blurbs, and they are hidden from every picker unless the owner flips a "show never-used routes" toggle.

## Acceptance criteria (testable — the tester will run these)
- [ ] **A1 startup default.** With the real model list, the Studio's initial model AND the fallback after
      a use-case/tab change is `minimax_h3_max_turbo` whenever it is in the list; the storyboard's
      new-shot default is the workhorse (not `models[0]`). One named constant for the workhorse id per
      language (Rust + TS), not scattered literals. [vitest + Rust test]
- [ ] **A2 launch set.** `LAUNCH_FAMILIES` contains the workhorse and contains NO Seedance and NO Grok
      id; a Rust test asserts both (by family-prefix, so a future `seedance_*`/`grok_*` cannot slip in).
- [ ] **A3 hidden by default.** A single core predicate (e.g. `registry::is_never_used(model_id)`)
      defines the never-used set = every Seedance + every Grok model id in the registry (test enumerates
      the registry and proves none is missed). `list_models` / `models_for_use_case` omit them by default
      and include them only when the owner's persisted "show never-used routes" setting is ON (default
      OFF). Hidden ≠ deleted: registry, prices, capability tables and existing regression tests
      (incl. S20's `grok_video_v15` settle test) stay and stay green.
- [ ] **A4 no dead ends.** History still renders: an existing job/storyboard shot whose `modelId` is a
      hidden model shows its display name and cost and can be viewed; it is never silently re-pointed to
      another model. Re-running such a shot while hidden is refused with a plain reason naming the
      toggle (no spend path). [test]
- [ ] **A5 the toggle.** Settings shows one labelled control "Show never-used routes (Seedance, Grok)"
      with a one-line cost warning; flipping it re-lists the picker. Test-locked selectors intact
      (`.forge/STATUS.md`). [vitest]
- [ ] **A6 mocks / samples / blurbs.** `ui/src/mock.ts` has no Seedance and no Grok (models, preset
      `models:` lists, rates, sample jobs) and its sample job + preset lists lead with the workhorse;
      `ui/src/lib/browser.ts` sample entry, `onboarding.rs` + `ui/src/lib/providers.ts` blurbs no longer
      advertise Seedance or Grok (the xAI key row itself may stay — it is a provider, not a default).
      A vitest greps the mock module's exported data for /seedance|grok/i = 0 hits.
- [ ] **A7 no collateral.** No `CostModel` value changes; no route/price/capability row deleted; no new
      dependency; no migration unless the settings store genuinely needs one (design must justify).
- [ ] **G1** `cargo test --workspace`, `cargo fmt --all --check`, `cargo clippy --workspace
      --all-targets -- -D warnings`, `./scripts/lint-provenance.py`, `./scripts/lint-spend.py`, keyless
      `audit_fal`, `cd ui && pnpm test && pnpm build` all pass. Baseline 971 Rust / 313 UI.
- [ ] **G2 spend hygiene.** Every command: `HICKEYFIELD_NO_KEYCHAIN=1` + dummy `hickeyfield_*_KEY=x` SET;
      never `HICKEYFIELD_ALLOW_SPEND`; never `first_light`; hermetic tests only.

## Out of scope
Deleting models/routes/prices; Higgsfield preset glob tables in `preset.rs` (vendor data, not defaults);
code comments and historical docs that merely mention the names; vendored snapshots; S21+ work.

## Approach + why this slice now
Smallest change that makes the owner's two "never" rules structural instead of a habit: one predicate in
core, filtered at the command boundary (the same place use-case filtering already lives,
`commands.rs` `models_matching`), one persisted setting, workhorse-first defaults. No spend, no purchase.
