# S10-pricing-accuracy-and-cost-tracker — RELEASE
**Gate:** `python3 .forge/gate.py S10-pricing-accuracy-and-cost-tracker` exit 0. TEST + REVIEW +
SECURITY = PASS on HEAD 63e6fc2 (Opus 4.8 verifiers; one repair round of the allowed two).
**Shipped:** hermetic fal price audit (build-failing at 1.2×; 2 wrong literals fixed), estimator
honesty (unknown never $0, floor mechanism), settled-output actual cost + provenance (migration 7),
sandboxed ffmpeg probe, estimated-vs-actual UI + price source, human-run live price seam.
**Regression:** cargo test --workspace 820 / 0; fmt · clippy -D warnings · provenance clean; pnpm
test 245 / build ok; keyless audit_fal PINNED 0 / UNREACHABLE 0 / CHECKED 66; tauri build + launch
at install.
**Money:** nothing spent; live seam and first_light not run.
**Owner-visible scope:** audit hard-checks 11/66 fal routes; 46 prose rows need the live seam or
`price-audit-tiered-prose`. Migration 7 runs on first launch of this build.
**Landing:** commits on forge/s10-pricing, ff to main (local fork only, never upstream),
`tauri build --bundles app`, ad-hoc codesign, reinstalled to /Applications.
**Status:** released (local main).
