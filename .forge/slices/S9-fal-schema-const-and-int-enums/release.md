# S9-fal-schema-const-and-int-enums — RELEASE
**Gate:** `python3 .forge/gate.py S9-fal-schema-const-and-int-enums` exit 0. TEST + REVIEW + SECURITY
= PASS (verifiers on claude-opus-4-8 for TEST/REVIEW; SECURITY pass retained per owner).
**Shipped:** fal schema parser reads `const` + integer enums; reconcile coerces pinned settings
(advisory) and refuses pinned prompt/media; audit check 4 enriched + new check 5 PINNED FIELDS;
re-landed LTX 2.5 Pro, LTX 2.5 Pro Animate, Veo 3.1 Extend, Seedance 2.5 image-to-video.
**Regression:** cargo test --workspace 801 / 0; fmt · clippy -D warnings · provenance clean; pnpm
test 232 / build ok; keyless audit 11/2/21/9, PINNED 0, UNREACHABLE 0, CHECKED 66; tauri build +
launch at install.
**Money:** nothing spent; first_light not run.
**Follow-ups:** pin-aware-cost-estimate (Medium/Minor, pricing slice); dependency-advisories-h2-rustls
(pre-existing); Seedance offline t2v aspect collapses to auto (disclosed, accepted).
**Landing:** commits on forge/s9-fal-schema-const, ff to main (local fork only, never upstream),
`tauri build --bundles app`, ad-hoc codesign, reinstalled to /Applications.
**Status:** released (local main).
