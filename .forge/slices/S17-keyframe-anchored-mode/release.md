# S17-keyframe-anchored-mode — RELEASE
**Gate:** `python3 .forge/gate.py S17-keyframe-anchored-mode` exit 0. TEST + SECURITY PASS on b3939cd,
REVIEW FAIL → repair 5b353c0 → TEST + REVIEW + SECURITY PASS (Opus 4.8; one repair round of two).
**Shipped:** keyframe-anchored storyboards (stills first, both-frame binding, end-frame adherence score,
ideal-threshold gating, still cards + priced total incl. stills) — the path from the 95% floor toward
the owner's 99% continuity ideal. Carries the keychain hotfix: `vault::get` never reaches the keychain
under cfg(test) / `HICKEYFIELD_NO_KEYCHAIN=1`; env-stripping helper deleted; keyframe tests pin an
unavailable route; the TEST verifier ran the whole workspace behind an unroutable proxy — zero
provider reach.
**Regression:** cargo test --workspace 950 / 0 + 9 real-sidecar ignored tests; fmt · clippy -D warnings ·
provenance clean; pnpm test 312 / build ok; keyless audit unchanged; tauri build + launch at install.
**Money:** $0.36 spent by the STOPPED first implementer's test before the hotfix (9 qwen stills,
2026-09-16 15:39); nothing since; nothing spendable from tests any more. Thresholds uncalibrated
(human-run). Migration 11 runs on first launch.
**Landing:** commits on forge/s17-keyframe, ff to main (local fork only, never upstream), `tauri build
--bundles app`, ad-hoc codesign, reinstalled to /Applications.
**Status:** released (local main).
