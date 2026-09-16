# S16-duration-range-and-continuity-prompting — RELEASE
**Gate:** `python3 .forge/gate.py S16-duration-range-and-continuity-prompting` exit 0. TEST + REVIEW +
SECURITY = PASS on HEAD c76ac43 and again on repair 48cb68d (Opus 4.8 verifiers; one repair round of two).
**Shipped:** provider duration ranges reach the chip row (the MiniMax 12-second cap is gone: H3 Max
5–15, Wan 2–30) with submit/storyboard clamps and a keyless range audit; shared-world continuity
prompting with a validator and fill-to-ceiling planning; measured per-join continuity (SSIM + colour
drift, 0.80 floor) with badge, per-join render overlaps, regenerate-from-frame through the money gate
and an opt-in bounded auto-retry; the dormant double-bill on regenerate closed.
**Regression:** cargo test --workspace 924 / 0 + 8 real-sidecar ignored tests; fmt · clippy -D warnings ·
provenance clean; pnpm test 302 / build ok; keyless audit RANGE MISMATCH 0, CHECKED 70; tauri build +
launch at install.
**Money:** nothing spent. Thresholds are reasoned; calibration on one real 2×15 set is yours to run.
**Owner-visible:** migration 10 (auto-retry column) runs on first launch; auto-retry is off by default.
**Landing:** commits on forge/s16-continuity, ff to main (local fork only, never upstream), `tauri build
--bundles app`, ad-hoc codesign, reinstalled to /Applications.
**Status:** released (local main).
