# S8-fal-catalogue-sync — RELEASE
**Gate:** `python3 .forge/gate.py S8-fal-catalogue-sync` exit 0. TEST + REVIEW + SECURITY = PASS
(test.md RE-TEST 3, review.md RE-REVIEW 3 over HEAD 55aa32f; security.md PASS).
**Shipped:** `examples/fal_diff.rs` (keyless fal→registry diff + snapshot refresh), snapshot
2026-09-16 (1,491 models), 11 audit-verified fal frontier models incl. Seedance 2.5 with a native
30s text-to-video clip. Withdrawn as unrunnable: LTX 2.5 ×2, Veo 3.1 Extend, Seedance 2.5 i2v mode
(→ backlog `fal-schema-const-and-int-enums`). H3 Max not landed (constant-fields follow-up).
**Regression:** cargo test --workspace 779 pass / 0 fail; fmt · clippy -D warnings · provenance
clean; pnpm test 232 pass; pnpm build ok; keyless audit_fal UNREACHABLE 0 / CHECKED 62 / no finding
on any landed slug; tauri build + launch at install.
**Money:** nothing spent — every probe keyless; first_light not run.
**Scope note (owner-ratified 2026-09-16 handoff: "keep the 11 verified"):** the AC3 minimum set is
partly deferred; every refusal is disclosed and traced to one backlog slice.
**Landing:** commits on forge/s8-fal-catalogue-sync (b319c5d..HEAD), ff to main (local fork only,
never upstream), `tauri build --bundles app`, codesign ad-hoc, reinstalled to /Applications.
**Status:** released (local main).
