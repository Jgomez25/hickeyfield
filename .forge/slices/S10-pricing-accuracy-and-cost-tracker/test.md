# S10-pricing-accuracy-and-cost-tracker — TEST (evidence)

Author: adversarial verifier (not the implementer). Every command below was run by me in
this session with the dummy keys the task specified; the `#[ignore]` live seam, `first_light`,
and any authenticated fal call were NOT run. Output pasted is real.

## Implementer's claim (verbatim excerpts)

> "Built exactly the S10 slice (AC1–AC7)."
> AC1: "MISMATCH 0 — ... now with 11 rows actually compared ... The 8 `Unpublished` rows ...
> no literal was found to be wrong, so none was changed."
> AC7: "cargo test --workspace → 816 passed, 0 failed, 11 ignored ... cargo fmt --all --check →
> FMT CLEAN ... clippy ... no warnings ... lint-provenance.py → PASS ... audit_fal → PINNED
> FIELDS (0), UNREACHABLE (0), CHECKED (66) ... price_check → MISMATCH 0 / OK 11 ... pnpm test →
> 245 passed (15 files) ... pnpm build → tsc clean."
> Deviation: `fal_catalogue::parse_pricing` NOT loosened; the audit recovers the base rate via
> `prices::fal::parse_prose`. Floor applied to zero routes. ffmpeg `-i` stderr probe. Migration 7
> write-once. Live seam `#[ignore]`, not run.
> Known limitation #1: "AC1 hard-fail surface is 11 rows, not all 66 ... the other 46 fal
> endpoints are ... `Prose`, verifiable only via the AC6 live seam ... not hermetically."

## Commands (all real, this session)

### AC7 gate
- `cargo test --workspace` → **exit 0**. core lib `687 passed; 0 failed; 10 ignored`; tauri lib
  `124 passed; 0 failed; 1 ignored`; integration crates green; total matches the claimed 816/0/11.
- `cargo fmt --all --check` → **FMT_EXIT=0** (clean).
- `cargo clippy --workspace --all-targets -- -D warnings` → **exit 0**, no warnings.
- `./scripts/lint-provenance.py` → **PROV_EXIT=0**; "no third-party media hosts", "no shipped
  string matches the reference corpus (80015 shingles indexed)".
- `env -u hickeyfield_FAL_KEY -u FAL_KEY cargo run -q -p hickeyfield-core --example audit_fal` →
  `PINNED FIELDS (0)`, `UNREACHABLE (0)`, `CHECKED (66)`. Unchanged from S9.
- `cd ui && pnpm test` → **15 files, 245 passed** (incl. `MetaCard.test.tsx` 5, `cost.test.ts` 27).
- `cd ui && pnpm build` → **built in 1.08s**, tsc/vite clean.

### AC1 — hermetic price audit (keyless)
`env -u hickeyfield_FAL_KEY -u FAL_KEY cargo run -q -p hickeyfield-core --example price_check`
reproduced verbatim:
```
MISMATCH 0 / OK 11 / UNMODELLABLE 0 / PROSE 46 / UNPUBLISHED 8 / NO-SNAPSHOT 1
coverage: 66 endpoints checked / 57 with fal prose / 8 with null prose
```
The 11 OK rows each print `literal $X ~ fal $X per <unit>` (kling silent bases $0.112/$0.070/$0.084,
nano-banana $0.08/$0.15, veo3.1/extend $0.20) — genuinely compared, not vacuous. 46 Prose rows are
printed with their fal prose for a human.

### AC3 — ffmpeg probe against the real sidecar
- `cargo test -p hickeyfield-core --lib ffmpeg::tests -- --ignored` →
  `probe_file_reads_a_real_sidecar_output ... 1 passed` (synthesises a 2s 1280x720 clip with the
  bundled `ffmpeg-aarch64-apple-darwin` and probes it back).
- Ran the sidecar `-hide_banner -i` directly on **corrupt random bytes** and a **text file named
  `.mp4`**: output has no `Duration:`/`Video:` line → `parse_probe` returns all-`None` (never 0).
- **Audio-only** m4a: `Duration: 00:00:03.00`, an `Audio:` stream, no `Video:` → seconds set,
  width/height `None`; the `44100 Hz` on the Audio line is not misread as a resolution (probe only
  reads dims off a `Video:` line). Confirms "None never rendered as 0" holds for real inputs.

### AC6 — live seam
`crates/hickeyfield-core/tests/live_fal_pricing.rs` read line-by-line: `#[ignore]`; `fal_key()`
reads only `hickeyfield_FAL_KEY`/`FAL_KEY` and returns early with a "not a failure" note when
absent; the key travels only in the `Authorization` header and is never in any `println!`
(only endpoint + figures are printed); uses `GET models/pricing` (a lookup, starts no generation).
Counted among the 11 ignored tests in the keyless workspace run — never executed.

## Break attempts

All mutations were performed in a **copy of the repo outside the tree**
(`.../scratchpad/mutrepo`), never in the repo. Baseline in the copy: core `687 passed`,
tauri `124 passed`.

1. **AC2 regression guard** — reverted `cost.rs` `let secs = b.seconds?` → `unwrap_or(0.0)`:
   `unknown_duration_is_none_not_zero_per_second` → **FAILED** (caught). Restored.
2. **AC1 teeth** — mutated an OK-row literal `nano-banana-2` `per_image(0.08)` → `per_image(0.20)`:
   `the_audit_is_hermetic_and_finds_no_literal_beyond_tolerance` → **FAILED** with a
   `Mismatch { ratio: 2.5 }`. Proves the 11 OK rows are really compared, not decorative. Restored.
3. **AC3 apply_poll** — dropped the `poll.actual_usd.is_some() && …` term from the `changed` gate
   (`engine.rs`): `apply_poll_persists_a_cost_only_delta` → **FAILED** (caught). Restored.
4. **AC3 probe** — corrupted `parse_probe`'s `find("Duration:")`:
   `parse_probe_reads_duration_and_resolution` → **FAILED** (caught). Restored.
5. **AC3 migration write-once** — added `priced_from = excluded.priced_from` to the upsert UPDATE
   set (`store.rs`): `migration_7_adds_provenance_columns_and_roundtrips` → **FAILED**
   (`left: Some(Literal), right: Some(Feed)` — write-once asserted). Restored.
6. **Idempotency / re-poll** — `runner` guards the actual write behind `job.actual_usd.is_none()`
   and de-dupes the advisory (`!job.advisories.contains(&adv)`); `an_unmeasurable_output_gets_none_
   not_zero`, `actual_is_priced_from_the_settled_quantity` (10s == 2×5s, source `Probe`) and
   `a_settled_quantity_that_differs_carries_an_advisory` all pass in the scratch baseline.
7. **AC2 `unwrap_or(0.0)` sweep** — grep of `cost.rs`/`pricing.rs`/`commands.rs`: the only live
   `unwrap_or(0.0)` left is inside `with_minimum_seconds` (reached only after `estimate` already
   returned `Some`, i.e. seconds present) and two are comments. The two bug sites are gone.
8. **UI honesty** — `formatCost` returns `UNKNOWN_COST_LABEL` for null/NaN; `formatActual` returns
   "Not reported" for null/undefined; a real `None` is never rendered as `$0.00`. Test-locked
   classes (`meta-card`, `meta-card-fields`, `meta-card-advisories`, `spend-meter`,
   `spend-meter-value`, `spend-meter-note`, `data-meta-anchor`) all still present.

### AC1 teeth — the owner's "many prices are wrong" probe (findings, not AC failures)

I hand-checked >8 of the 46 Prose routes against the vendored snapshot prose. Most single-rate and
all tiered literals track fal within 1.2× (ltx-2.5 `[(720,0.12),(1080,0.17)]`, wan2.2/wan-vace
`[(480,0.04),(580,0.06),(720,0.08)]`, veo3.1 `per_second_audio(0.20,2.0)` = $1.00/$2.00 for 5s,
grok_video_v15 `[(480,0.08),(720,0.14)]`, gemini_omni fal $0.125 = fal's 720p figure — all match).
Two genuine residual wrong prices remain, **both pre-existing literals untouched by S10 and both
`Prose` rows AC1's parse-to-Rate mechanism structurally cannot flag**:
- `minimax/h3` (fal) = `per_second(0.26)` flat, but the snapshot prose is
  `$0.05/480p, $0.06/768p, $0.13/2K, $0.16/4K` — the literal over-quotes by 1.6×–5× depending on
  resolution. (Mitigated in practice: the cheaper VAIG route `$0.13` is preferred by the resolver.)
- grok edit/extend `per_second(0.05)` under-quotes 720p (fal $0.07–$0.08/s), the design Q3 item.

These do **not** falsify the claim or AC1: the implementer's claim is narrowly "MISMATCH 0 among
the 11 comparable rows" plus an explicit written disclosure (limitation #1) that the 46 Prose routes
are not hermetically verified. AC1's *testable* wording in slice.md scopes hard-fail to "every route
whose row parses to a Rate" and requires only that prose be printed for the rest — both satisfied.
The design (§0.2, §12.1, Q1), committed before implementation, deliberately declined to parse
conditional prose. I record these as caveats the RELEASE owner should see: the hermetic audit's
teeth cover 11 of 66 routes, and "every fal price verified" is only fully achievable via the
human-run AC6 seam.

## Root cause / disposition

No AC is refuted. Every acceptance criterion reproduces on my own runs; all five new-test mutation
guards fail when the fix is reverted; the keyless gates (audit_fal PINNED 0/UNREACHABLE 0, price
audit MISMATCH 0, provenance PASS) hold; the live seam cannot spend money or leak the key. The
minimax/h3 and grok residual over/under-quotes are real but are pre-existing, out-of-scope-by-design
`Prose` routes the slice honestly declares it cannot verify hermetically — a disclosed limitation,
not a broken claim.

VERDICT: PASS

## RE-TEST (HEAD 63e6fc2)

Coordinator applied my caveats in commit `63e6fc2`. I re-tested only that delta
(`git show 63e6fc2`), keyless, no live seam, repo read-only apart from this file.

### Gate at HEAD 63e6fc2 (re-run by me)
- `cargo test --workspace` → **exit 0**, **820 passed / 0 failed / 11 ignored** (core lib 690:
  +3 new; tauri 125: +1 new). Matches the coordinator's 820/0.
- `cargo fmt --all --check` → FMT=0. `cargo clippy --workspace --all-targets -- -D warnings` →
  exit 0, no warnings. `./scripts/lint-provenance.py` → PROV=0.
- keyless `price_check` → `MISMATCH 0 / OK 11 / UNMODELLABLE 0 / PROSE 46 / UNPUBLISHED 8 /
  NO-SNAPSHOT 1`; coverage unchanged. `minimax/h3` and grok edit/extend remain PROSE rows (a
  tiered literal is still not hard-compared to multi-figure prose) — MISMATCH stays 0.
- keyless `audit_fal` → PINNED 0 / UNREACHABLE 0 / CHECKED 66 (unchanged).
- `cargo test -p hickeyfield-core --lib ffmpeg::tests -- --ignored` → real-sidecar probe
  **1 passed** with the new hardened argv.

### The two corrected literals, re-derived by me from the snapshot prose
- `minimax/h3` snapshot prose: *"$0.05/s 480p, $0.06/s 768p, $0.13/s 2K, $0.16/s 4K"* →
  literal `PerSecondTiered [(480,0.05),(768,0.06),(1440,0.13),(2160,0.16)]`. The four figures
  are **exactly** fal's; tier heights map 2K→1440, 4K→2160 (a 1080p request lands in the 2K tier
  at $0.13, the nearest published rate — a defensible round-up, not a fabricated number).
  Estimates: 5s@2K = 5×$0.13 = **$0.65** (= VAIG); 5s@720p = 5×$0.06 = **$0.30**; 5s@480p =
  **$0.25**. All within 1.2× of fal. The old flat $0.26/s (1.6×–5× over) is gone.
- grok edit/extend prose: *"480p every second $0.06 ($0.05 out + $0.01 in), 720p $0.08 ($0.07 out
  + $0.01 in)"* → literal `PerSecondTiered [(480,0.06),(720,0.08)]` (output+input totals, correct
  because an edit/extend always has a video input). 5s@720p = **$0.40** (was $0.25, a 1.6× under);
  5s@480p = **$0.30**. Both now equal fal.

### Other delta pieces, verified
- `cost::estimate` now returns `None` on a rate-derived `$0`/non-finite, with `Flat` exempt so a
  deliberate free route (z-image `Flat{0.0}`) still prices `$0.00`. Logic read; test present.
- `settled_billable` batch² double-count fixed: a batch of N images no longer multiplies count×batch
  (N=3 @ $0.15 → $0.45, not $1.35); video keeps requested batch since `images` is ignored per-second.
- ffmpeg probe argv is `-nostdin -hide_banner -protocol_whitelist file -i <file>` (whitelist before
  `-i`, no `-y`), draining both pipes on threads, 20s deadline + kill, capture bounded to 64 KiB;
  a timeout returns `FfmpegError::Probe` → runner's `probe_file(...).unwrap_or(none)` → all-`None`
  dims → unknown cost, never 0.

### Mutation checks (fresh scratch copy at HEAD, outside the repo)
Baseline: all five target tests green. Each mutation caught:
1. `minimax/h3` tier `(1440,0.13)`→`(1440,0.26)` ⇒ `gateway_routes_win_where_the_plan_says_they_do`
   **FAILED** (2K estimate no longer $0.65).
2. Removed the rate-derived-`$0` guard in `cost.rs` ⇒
   `a_zero_rate_is_unknown_not_free_but_flat_zero_stays_free` **FAILED**.
3. Dropped `-nostdin` from `probe_args` ⇒ `probe_args_sandbox_the_untrusted_input` **FAILED**.
4. Made `bounded_push` extend the whole chunk (ignore cap) ⇒ `probe_capture_is_bounded` **FAILED**.
5. Reverted the `if videos == 0 { b.batch = 1; }` fix in `runner::settled_billable` ⇒
   `a_batched_image_job_is_not_double_counted` **FAILED** ($1.35 instead of $0.45).
All source restored after each; the timeout-kill path is not fixture-tested (acknowledged in a
code comment) but is deterministically bounded and exercised by the real-sidecar probe.

### Disposition
The delta resolves both residual wrong prices I flagged (minimax/h3, grok edit/extend) with literals
that match fal's published prose exactly, hardens the untrusted-media probe, and fixes a real
batch² over-count — each covered by a test that fails when the fix is reverted. Full gate green at
820/0; keyless audits unchanged. No AC is refuted; no regression found in the delta.

VERDICT: PASS
