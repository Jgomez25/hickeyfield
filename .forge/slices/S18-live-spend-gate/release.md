# S18-live-spend-gate — RELEASE
**Gate:** `python3 .forge/gate.py S18-live-spend-gate` exit 0. TEST + REVIEW + SECURITY = PASS on HEAD
07da1b8 (Opus 4.8 verifiers; zero repair rounds). TEST proved, behind an unroutable proxy, that every
billable path returns SpendBlocked with no socket when the flag is absent, and reaches the network
only when `HICKEYFIELD_ALLOW_SPEND=1` is set.
**Shipped:** the live-spend gate (provider submits + hosted LLM paths; app entrypoint enables it;
everything else OFF by construction), `first_light` refuses without the flag, `scripts/lint-spend.py`
repo guard, fake-provider seam, docs for both safety flags. With S17's keychain block this closes the
2026-09-16 incident class structurally.
**Regression:** cargo test --workspace 961 / 0 + 9 real-sidecar ignored tests; fmt · clippy -D warnings ·
provenance · lint-spend clean; pnpm test 312 / build ok; keyless audit unchanged; tauri build + launch
at install.
**Money:** nothing spent. Owner check on first launch: a normal estimate appears and Generate works
(the packaged app enables the gate at startup) — the one proof only the owner can give.
**Landing:** commits on forge/s18-spend-gate, ff to main (local fork only, never upstream), `tauri build
--bundles app`, ad-hoc codesign, reinstalled to /Applications.
**Status:** released (local main). PROJECT PAUSED by owner after this slice.
