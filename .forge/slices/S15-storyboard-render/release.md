# S15-storyboard-render — RELEASE
**Gate:** `python3 .forge/gate.py S15-storyboard-render` exit 0. TEST + REVIEW + SECURITY = PASS on
HEAD e82d2cd; UI error-surfacing fix 1139b13 re-reviewed PASS (Opus 4.8 verifiers; zero repair rounds).
**Shipped:** one brief → chained shots with last-frame continuity → sidecar render → a 30-second ad in
the Library on ANY model with an image-to-video route (H3 Max 2×15, LTX 3×10, Seedance single-shot),
continuity levers (fal prompt expansion disabled on chained shots, fixed seed, frame at target size),
hard-cut continuity joins, progress, Σ actual cost on the set. Design memo + follow-ups (continuity
score, world block, keyframe mode) logged for S16/S17.
**Regression:** cargo test --workspace 888 / 0 + 2 real-sidecar ignored tests; fmt · clippy -D warnings
· provenance clean; pnpm test 292 / build ok; keyless audit unchanged; tauri build + launch at install.
**Money:** nothing spent; render is local and free; every chained shot re-runs the estimate + guard.
**Owner-visible:** final ad lands in ~/Movies/Hickeyfield (Library folder); migration 9 runs on first
launch; Render lives on the Library set item, Chain toggle in Storyboard.
**Landing:** commits on forge/s15-storyboard-render, ff to main (local fork only, never upstream),
`tauri build --bundles app`, ad-hoc codesign, reinstalled to /Applications.
**Status:** released (local main).
