# S20b-never-used-routes — RELEASE
**Gate:** `python3 .forge/gate.py S20b-never-used-routes` exit 0. DESIGN failed its first gate (five in-crate Rust
tests would not compile after the signature change) → Revision 1 → PASS (sha256 237154c5…). TEST + REVIEW +
SECURITY = PASS. TWO repair rounds (the maximum): round 1 = v11→v12 data-intact upgrade proof, mock constant,
tab test; TEST then FAILED the repaired tab test by mutation (tautological fixtures) → round 2 made it
falsifiable → fresh TEST re-ran five mutations separately, all RED → PASS; REVIEW delta PASS.
**Shipped (owner rules 2026-09-19: Seedance never, Grok paid API never; workhorse `minimax_h3_max_turbo`
is the only paid video route):** the Studio opens on the workhorse, falls back to it on tab change, the
storyboard defaults to it, and it replaces the Seedance family in the launch set; the 11 registry models
prefixed `seedance`/`grok` are hidden from every picker by default (one core predicate `is_never_used`,
filtered server-side) behind a persisted Settings toggle "Show never-used routes (Seedance, Grok)"
(`app_pref`, store migration 12, default OFF); while OFF `submit_one` refuses such a model before the
enhancer, route resolution or any provider client, naming the toggle; a failed pref read fails CLOSED;
history still renders via unfiltered `model_names()`; mocks, samples and blurbs carry no Seedance/Grok and
lead with the workhorse. Nothing deleted: registry, prices, tests (incl. S20's grok settle fixture) stay.
**Regression:** cargo test --workspace 979 / 0 (+20 pre-existing gated ignores; none added); fmt · clippy
-D warnings · provenance · lint-spend clean; keyless audit_fal 70 endpoints; pnpm test 321 (25 files) /
build ok. Mutations caught: predicate off, submit guard off, destructive migration 12, and five
workhorse-preference mutations in App.tsx.
**Money:** nothing spent. No S17/S18 structure touched; no dependency change.
**Data:** first launch of the new build upgrades the owner's DB v11 → v12 (adds `app_pref` only; existing
rows proven byte-for-byte intact on a simulated v11 file).
**Process note:** a verifier once used `git checkout` on a tree holding uncommitted slice work (recovered
byte-identical); briefs now forbid checkout/restore/reset/stash/clean.
**Landing:** commit on forge/s20b-never-used-routes; ff to main (local fork only, never pushed), tauri build
from the repo root, ad-hoc codesign, reinstall to /Applications — on the owner's go.
