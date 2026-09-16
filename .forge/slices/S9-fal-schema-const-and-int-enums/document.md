# S9-fal-schema-const-and-int-enums — DOCUMENT

## What shipped
- `fal_schema::parse` now records a scalar `const` as a one-member enum plus a `pinned` set, and
  stringifies integer/number enum members; `anyOf`/nullable integers stay free-form (Wan 3.0 Prime
  regression-tested). `reconcile` returns `Result<Vec<Pinned>, Refusal>`: a pinned settings field
  is coerced to the pin with a job advisory; a pinned `prompt` or media key is refused.
- `examples/audit_fal`: check 4 uses the enriched enums; new check 5 "PINNED FIELDS" reports a
  value the app would send that fal pins differently; the example is now a tested target.
- Re-landed, each clean on the fixed keyless audit: `ltx_2_5`, `ltx_2_5_animate` (duration 6/8/10/auto,
  720p/1080p, $0.12/$0.17 per s), `veo3_1_extend` (fixed 7s/720p, $0.20/s, ×2 with audio), and
  Seedance 2.5 image-to-video (start + end frame; aspect pinned to auto on that endpoint).
  Counts: FAL_ROUTE_MODES 48→51, IMPORTED_2026_09 11→14, registry size +27→+30.
- Deviation from design §6.4, accepted by TEST and REVIEW: Seedance 2.5's spec aspect is `["auto"]`
  (valid on every endpoint it serves); online t2v still offers fal's 7 ratios from the live schema,
  offline t2v collapses to auto.

## Docs changed
- `CHANGELOG.md` — `[Unreleased] › Fixed` bullet (const/int-enum handling + the four paths);
  S8's "text-to-video for now" qualifier removed.
- `.forge/backlog.md` — slice entry marked done; new follow-ups: `pin-aware-cost-estimate`
  (estimated_usd is priced before reconcile may coerce a pinned duration — unreachable for the
  landed set, fix in the pricing slice) and `dependency-advisories-h2-rustls` (pre-existing
  `cargo deny` failures RUSTSEC-2026-0258/-0285 + stale deny.toml:77 ignore).
- README / docs: N/A.

## Evidence
- TEST / REVIEW / SECURITY = PASS (all three verifiers re-ran the gate independently).
- cargo test --workspace 801 pass / 0 fail; fmt · clippy -D warnings · provenance clean;
  pnpm test 232 / build ok; keyless audit_fal 11/2/21/9, PINNED 0, UNREACHABLE 0, CHECKED 66;
  fal_diff --offline reaches 66. Live schemas in `schemas/`, audit logs before/after in this dir.
- Nothing spent; no key in the session.
