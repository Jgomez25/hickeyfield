# S14-minimax-h3-max-constant-fields — TEST (evidence)

Author: adversarial verifier (different agent than the implementer). Every command below
was run by me on `forge/s14-h3-max` (HEAD `e953d65`); all output is pasted from my own runs.

## Implementer's claim (verbatim, key lines from implement.md)

> `cargo test --workspace`: **865 passed; 0 failed; 12 ignored** (baseline 852 + 13 new tests).
> `cargo fmt --all --check`: clean (`FMT_OK`).
> `cargo clippy --workspace --all-targets -- -D warnings`: **Finished, 0 warnings**
> `./scripts/lint-provenance.py`: **PASS**
> keyless `audit_fal`: see `audit_fal-after.log` (deltas above).
> `price_check`: **MISMATCH 0** (H3 Max rows PROSE).
> `cd ui && pnpm test`: **285 passed (23 files)**. `pnpm build`: **built in ~0.9s, exit 0**.

> Four endpoints CHECKED, zero findings; ... `audit_fal-after.log` (CHECKED 70, +4; UNREACHABLE 0,
> PINNED 0, BAD ROUTE CONSTANTS 0, MISSING REQUIRED 2 — both pre-existing `inworld`/`mirelo`).

> **Deviation from the design ...** Design D3/Q2 put `aspect_ratio` on the shared spec ... I omitted it.

## Commands

### AC6 — cargo test --workspace
```
TOTAL passed=865, failed=0, ignored=12
```
(summed from every `test result:` line; matches the 865/0/12 claim.)

### AC6 — fmt, provenance
```
=== FMT ===
FMT_OK exit=0
=== PROVENANCE ===
CDN hostnames
  PASS  no third-party media hosts referenced
Copy provenance
  PASS  no shipped string matches the reference corpus (80015 shingles indexed)
provenance exit=0
```

### AC6 — clippy -D warnings
```
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.80s
[exited with code 0]
```

### AC5 — keyless audit_fal (live network, then diffed vs committed after-log)
Ran `env -u hickeyfield_FAL_KEY -u FAL_KEY cargo run -q -p hickeyfield-core --example audit_fal` (exit 0, empty stderr, network reached):
```
=== SILENTLY IGNORED MEDIA ... (11) ===
=== MISSING REQUIRED FIELDS — 422 after a round trip (2) ===
=== DROPPED SETTINGS ... (21) ===
=== REJECTED OPTIONS ... (9) ===
=== PINNED FIELDS ... (0) ===
=== BAD ROUTE CONSTANTS — the app injects a field the endpoint refuses (0) ===
=== UNREACHABLE — fal serves no schema for this endpoint (0) ===
=== CHECKED — fal answered with a schema (70) ===
```
`diff <live> audit_fal-after.log` → **IDENTICAL**. The four H3 Max endpoints appear ONLY under CHECKED:
```
[CHECKED] minimax_h3_max        minimax/h3-max/text-to-video
[CHECKED] minimax_h3_max        minimax/h3-max/image-to-video
[CHECKED] minimax_h3_max_turbo  minimax/h3-max-turbo/text-to-video
[CHECKED] minimax_h3_max_turbo  minimax/h3-max-turbo/image-to-video
```
MISSING REQUIRED (2) = `inworld_text_to_speech`, `mirelo_text_to_audio` — both pre-existing.
Before-log baseline: CHECKED 66, MISSING 2, DROPPED 21, PINNED 0, UNREACHABLE 0, no h3-max rows.
Delta is exactly CHECKED +4 with zero findings; nothing else moved. AC5 satisfied.

### AC3/AC4 (price) — price_check
```
MISMATCH 0 / OK 11 / UNMODELLABLE 0 / PROSE 50 / UNPUBLISHED 8 / NO-SNAPSHOT 1
```
All four H3 Max endpoints classify as **PROSE** (multi-figure/resolution-conditioned). Live prose
confirms the transcribed current promotional tiers: H3 Max `$0.025/$0.04/$0.08`, Turbo
`$0.0125/$0.02/$0.04`. MISMATCH 0 as claimed.

### AC6 — UI
```
Test Files  23 passed (23)
     Tests  285 passed (285)
✓ built in 815ms
```

## Break attempts

### Probe 1 — build the real request body via the shipped library (schema + no schema)
Wrote an integration test in a scratch copy OUTSIDE the repo that drives
`media::fal_constants` → `fal_schema::apply_constants` → accepts-sweep → `fal_schema::reconcile`
against the four vendored live schemas. Result: **4 passed; 0 failed.**
- t2v (both models), schema loaded: `prompt_expansion_mode` present **exactly once** = `"balanced"`;
  `resolution` = `"768P"` (uppercase member); `aspect_ratio` = `"16:9"` (six-member enum present);
  `duration` is a JSON integer.
- i2v (both models): pem once = `"balanced"`; **no `aspect_ratio`** on the wire; `image_url` bound;
  `resolution` = `"768P"`.
- t2v with `schema = None` (offline): pem still written once = `"balanced"`.
- Body pre-carrying `prompt_expansion_mode: "quality"`: value **unchanged** (constant never overrides).

### Probe 2 — six mutations in a scratch copy OUTSIDE the repo (all caught)
| # | mutation | test that failed |
|---|---|---|
| 1 | remove `body.contains_key` override guard in `apply_constants` | `a_constant_never_overrides_a_user_value` FAILED |
| 2 | force `accepted = true` (ignore schema `accepts`) | `a_constant_is_dropped_when_the_schema_rejects_the_key` FAILED |
| 3 | drop constant-subtraction term in `unsupplied_required` (check 2) | `a_required_constant_is_not_flagged_as_missing` FAILED |
| 4 | remove enum-out-of-range branch in `constant_issues` (check 6) | `a_constant_the_endpoint_rejects_is_reported` FAILED |
| 5 | revert `"768p" => Some(768)` in `resolution_height` | `the_768p_tier_is_reachable` FAILED |
| 6 | change resolution flag to `ValueSpec::Text` | `h3_max_offers_only_the_resolutions_fal_enumerates` FAILED |
Every new test is non-vacuous.

### Probe 3 — 768p tier / pricing
`resolution_height("768P")` → `Some(768)` (mutation 5 proves it load-bearing). Workspace test
`h3_max_is_priced_per_the_launch_tiers` asserts 5s @ 768 = `5×0.04 = $0.20` and turbo `5×0.02 = $0.10`,
plus 480/1080 tiers — all green in the 865-pass run.

### Probe 4 — counts / untouched model
`FAL_ROUTE_MODES: [...; 53]` compiles (length literal enforced). Registry size test `+32`,
`JOB_TYPES` +2, `IMPORTED_2026_09_H3: [&str; 2]` chained into both resolve/adapter loops, four
`use_case` positive rows — all pass. `the_h3_max_models_are_present` asserts `minimax_h3` still routes
to `minimax/h3`; the diff shows the `minimax_h3` arm and route unchanged.

### Probe 5 — the aspect deviation, judged
The implementer omitted `aspect_ratio` from the shared spec (design D3/Q2 default was to keep it).
I verified the online path is correct per endpoint: my Probe-1 body carries `aspect_ratio` on t2v and
none on i2v, and `h3_max_aspect_is_per_endpoint_not_per_spec` confirms t2v exposes the six ratios
(from `from_fal`) while i2v is `Support::No`. AC3's "aspect enum on t2v only" holds on the path the
picker actually renders. The deviation is a genuine, honestly-documented resolution of a hard
conflict: keeping aspect on the spec makes `supports_aspect` true, which the audit's DROPPED SETTINGS
check (23 vs 21) flags on the two i2v endpoints, breaking AC5's "4 endpoints CHECKED with zero
findings." AC5 is a hard requirement and AC3 does not require offline aspect. The sole cost — offline
text-to-video loses the aspect chip and falls back to fal's own 16:9 — is benign and disclosed.
**Judgment: acceptable; not a defect.**

### Probe 6 — idempotency / determinism
Re-ran the keyless audit; output byte-identical to the committed after-log. price_check MISMATCH 0
on repeat. No non-determinism observed.

## Root-cause diagnosis
None. Every AC (AC1–AC6) reproduced from live output; all six adversarial mutations were caught by
the shipped tests; the end-to-end request body is correct for all four endpoints online and offline.

VERDICT: PASS
