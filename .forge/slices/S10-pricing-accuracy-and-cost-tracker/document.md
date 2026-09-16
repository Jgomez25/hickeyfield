# S10-pricing-accuracy-and-cost-tracker — DOCUMENT

## What shipped
- **Hermetic price audit** `prices::audit()` (+ `examples/price_check`): every fal route literal is
  compared against fal's own `pricingInfoOverride` prose in the vendored index; a unit test fails
  the build on any mismatch beyond 1.2×. Result: MISMATCH 0 / OK 11 / PROSE 46 / UNPUBLISHED 8 /
  NO-SNAPSHOT 1 over 66 endpoints. Two genuinely wrong literals found by the adversarial verifier
  and fixed: MiniMax H3 (flat $0.26/s over-quoted 1.6–5× → fal's per-resolution tiers) and Grok
  edit/extend (dropped the input surcharge → output+input tiers). 15s floor: mechanism wired,
  data-driven; zero registry routes carry a fal-stated floor (prose grep recorded).
- **Estimator honesty**: `seconds.unwrap_or(0.0)` gone (unknown → None); a rate-derived $0 is
  None (Flat exempt so the free route stays free); batch double-count in settled billing fixed.
- **Actual cost per job**: settled output probed via the bundled ffmpeg sidecar (`ffmpeg -i`
  stderr; no ffprobe bundled) → `actual_usd` = rate × measured quantity, plumbed through
  `PollResult` → `apply_poll` (cost-only delta no longer dropped) → store; migration 7 adds
  write-once `priced_from` / `priced_at` and `actual_source`. Probe is sandboxed: 20s timeout +
  kill, `-nostdin`, `-protocol_whitelist file`, 64 KiB bounded stderr; failure → None, never 0.
- **Advisory** when the settled quantity differs from the requested one.
- **UI**: MetaCard estimated vs actual with delta row; MetaRail spend meter labelled actual/estimated
  with a price-source note (`price_status` now has a caller). Test-locked class names kept.
- **Live seam** (human-run, `#[ignore]`): diffs every fal route literal against fal's pricing API
  using the key from env/keychain only; never logged; reads prices, spends nothing. Not run here.
- Deviation (declared, accepted by REVIEW/TEST): `fal_catalogue::parse_pricing` not loosened
  (193-rate contract); the audit recovers the base rate via `prices::fal::parse_prose`.

## Docs changed
- `CHANGELOG.md` — `[Unreleased]` Fixed/Added bullets (audit, corrected prices, actual cost, UI).
- `docs/FIRST-LIGHT.md` — new "Verifying prices against the live fal API" section (how to run the
  ignored seam with your own key) + cost-accuracy notes.
- `.forge/backlog.md` — follow-ups: `price-audit-tiered-prose` (extend the hermetic audit to the
  46 conditional/tiered prose rows), `dependency-advisories-h2-rustls` (pre-existing),
  `pin-aware-cost-estimate` closed via settled actual cost (estimate still priced before reconcile;
  unreachable today), minor: `try_wait` Err arm in the probe skips join/reap (cosmetic).

## Evidence
- TEST / REVIEW / SECURITY = PASS on HEAD 63e6fc2 (each re-ran the gate; one repair round).
- cargo test --workspace 820 / 0; fmt · clippy -D warnings · provenance clean; pnpm test 245 /
  build ok; keyless audit_fal PINNED 0 / UNREACHABLE 0 / CHECKED 66; price audit log in this dir.
- Scope honesty: the hermetic audit has teeth on 11 of 66 fal routes; the other 46 are verifiable
  only via the human-run live seam until `price-audit-tiered-prose` lands.
- Nothing spent; no key in the session.
