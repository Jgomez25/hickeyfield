# S10-pricing-accuracy-and-cost-tracker — REVIEW (evidence)

Independent, read-only production-standards review of the slice diff (`git diff main...HEAD`).
I changed no source. Gate re-run by me, keyless, on HEAD `462120e`.

## Gate re-run (my own, keyless — dummy-key env, never the live seam / first_light)

- `cargo test --workspace` → **816 passed, 0 failed** (core lib 687, core integ 3+2, tauri 124; 11 ignored). Matches implement.md.
- `cargo fmt --all --check` → clean (exit 0).
- `cargo clippy --workspace --all-targets -- -D warnings` → 0 warnings/errors.
- `scripts/lint-provenance.py` → PASS (exit 0).
- `--example audit_fal` (keyless) → PINNED 0 / UNREACHABLE 0 / CHECKED 66 — unchanged from S9.
- `--example price_check` (keyless) → MISMATCH 0 / OK 11 / UNMODELLABLE 0 / PROSE 46 / UNPUBLISHED 8 / NO-SNAPSHOT 1. Byte-identical to `price-audit-2026-09-16.log`.
- `ui: pnpm test` → 245 passed (15 files); `pnpm build` → tsc clean, built.

Every count the implementer claimed reproduces. No fabricated evidence found.

## What I checked and found clean

- **AC2 estimator honesty** (cost.rs:190 `PerSecond`, cost.rs:216 `PerToken`): `seconds.unwrap_or(0.0)` is gone; both now `?`-return `None` on a missing billable quantity. `PerToken` correctly requires all of seconds+width+height (tokens are h·w·fps·s) while defaulting only `fps`. Tests `unknown_duration_is_none_not_zero_per_{second,token}` cover both. No path turns unknown into `$0.00`.
- **AC2 floor** (route.rs:26/57 `min_billed_seconds`+`floored`, registry.rs:73 `Model::estimate`, cost.rs:274 `per_second_rate`): data-driven, applied only when a route declares a floor (none does today) and the shape is per-second; `per_second_rate` folds the audio multiplier / tier so `with_minimum_seconds` doesn't double-apply it. Test `a_floored_route_bills_the_minimum` asserts $0.525 floored vs $0.175 linear. Sound — no short clip is over-quoted.
- **AC3 ffmpeg probe** (ffmpeg.rs:602 `parse_probe`, :623 `parse_dimensions`, :660 `probe_file`): pure stderr parser. Robustness verified — the codec tag `0x31637661` is rejected (left digit is `0`, guarded by `w>0`), `SAR`/`DAR` (`:`, no `x`) are ignored, `Duration: N/A` is not read as a number, fractional/`HH:MM:SS.ss` durations via `parse_timecode`, first `Video:` stream wins, audio-only and image files (no `Duration`) degrade the missing field to `None`. `run()` (ffmpeg.rs) ignores the non-zero exit of `ffmpeg -i` and reads stderr, only erroring on empty output / spawn failure. Every failure yields `None`, never `0`.
- **AC3 apply_poll early-return fix** (engine.rs:442): the `changed` gate gains an *additional* OR term (`poll.actual_usd.is_some() && job.actual_usd != poll.actual_usd`); it can only make `changed` true more often, so it cannot drop a previously-persisted transition — no S1 resumable-semantics regression. Body writes `actual_usd` (engine.rs:471). Test `apply_poll_persists_a_cost_only_delta` covers the cost-only delta and the no-op re-poll.
- **AC3 migration 7** (store.rs:160): additive `ALTER TABLE ADD COLUMN … ; PRAGMA user_version = 7`, all nullable; `version < 7` guard is idempotent on existing DBs. `row_to_job` degrades an unparseable enum to `None` (not a dropped row), pre-v7 rows load as `None`. Write-once is real: `priced_from`/`priced_at` are in the INSERT VALUES but **absent** from the `ON CONFLICT DO UPDATE SET`, mirroring `estimated_usd`; `actual_source` is in the update set beside `actual_usd`. `migration_7_adds_provenance_columns_and_roundtrips` asserts `user_version==7`, the frozen `priced_from/at`, and the updated `actual_source`. The v1-replay test drops the three new columns. Correct.
- **AC3 actual-cost path** (runner.rs:213 download_outputs, :274 compute_actual, :313 settled_billable): only computes when `actual_usd.is_none()` (no re-probe on re-poll), quantity is the *settled* probe (seconds/res/mp) with request-choice modifiers (audio/batch/fps) kept from settings, unmeasurable → `None` never `0`. Tests confirm 10s == 2×5s, actual == estimator's own quote for the settled billable, and all-`None` dims → `None`. Sound for per-second / per-token / per-megapixel / flat.
- **AC6 live seam** (tests/live_fal_pricing.rs): `#[ignore]`, reads key from `hickeyfield_FAL_KEY`/`FAL_KEY` only, skips (does not fail) when absent, key travels only in the `Authorization` header and is never printed (only endpoint/figures/ratio), uses `GET /v1/models/pricing` (a lookup, bills nothing), never touches `first_light`. Docs (FIRST-LIGHT.md) accurately describe it. Confirmed compiles + is skipped keyless (1 ignored in the integration bucket). I did not run it.
- **AC5 UI parity** (pricing.rs:146 `PriceStatus` `#[serde(rename_all="camelCase")]` → `routes`/`ageSeconds`/`live`; types.ts:101 + api.ts `priceStatus()`): DTO/type parity exact; `invoke("price_status")` matches the command registered at lib.rs:80. `costDelta` returns `null` unless both figures are real (no invented delta), `formatDelta` neutralises sub-0.00005 float noise. Test-locked class names preserved and the Delta row is asserted absent for estimate-only and toned `over`/`under` for real deltas (MetaCard.test.tsx). Additive CSS only.
- **AC1 tolerance is a hard failure**: `compare` (prices.rs) emits `Mismatch` when `ratio > AUDIT_TOLERANCE` (1.2), and `the_audit_is_hermetic_and_finds_no_literal_beyond_tolerance` asserts `mismatches.is_empty()`. `the_audit_actually_compares_something` guards non-vacuity (`compared >= 5`). So "0 mismatches" cannot mean "0 checked", and a wrong literal fails the build (`a_wrong_literal_is_caught`). Yes, 1.2× is enforced as a real test failure.

## Findings

All ranked minor. No blocker or major on a critical path.

1. **[minor] AC1's teeth are our-parser-vs-our-literal, not the design's independent index compare.** `crates/hickeyfield-core/src/prices.rs:1550` (`classify_endpoint`) — the strict `fal_catalogue::parse_pricing` (`Pricing::Rate`) reduces **zero** of the registry fal routes (implement.md, confirmed by the 0 rows), so all 11 hard-compared rows arrive via the lenient `fal_rate_from_prose` → `fal::parse_prose`. That compares a number our parser extracts from fal's prose string against a literal a human transcribed from the *same* prose. It genuinely catches a transcription slip (the `0.084`/`0.84` decimal-slip case is real), but it is not fully independent: a shared misreading of an ambiguous sentence would agree while both are wrong. This is honestly disclosed in the module doc and `price_check` output. Standard: tests must exercise the real acceptance path — here they do, but the coverage is narrower than AC1's literal wording. **Fix (follow-up, not in-slice):** the 46 PROSE rows include very regular resolution tables (`$0.12/s 720p or $0.17/s 1080p`; wan/ltx/minimax/veo/xai). A tiered-prose parser that reduces those to `PerSecondTiered` tiers and hard-compares each tier against the corresponding `CostModel` tier would non-circularly extend the teeth to ~15–20 more routes without loosening the 193-rate `parse_pricing` contract. Recommend as a backlog item.

2. **[minor] Latent `batch²` on per-image / per-megapixel actual (and estimate).** `src-tauri/src/runner.rs:313` (`settled_billable`) starts from `Billable::from(&SettingsDto)`, which at `src-tauri/src/commands.rs:631,634` sets **both** `images: Some(s.batch)` and `batch: s.batch`. For `PerImage`/`PerMegapixel`, `estimate` then computes `count(=batch) × usd × batch_factor(=batch)` = `batch²·usd` — an overcharge that would also flow into `actual_usd`. It is **dormant today**: the UI never sends `batch` (no frontend writer; `SettingsDto.batch` defaults to `one_u32`), so `images`/`batch` are both `1`, and the settled path overwrites `images` with the real output count while `batch` stays `1` (correct: fal bills per image). It is also pre-existing in `Billable::from` (untouched by this slice) and identical between estimate and actual. Standard: money-correctness on unit×quantity. Reachable only if a future setting sends `batch>1`. **Fix (follow-up):** for image shapes, drive `images` from the settled/requested count and hold `batch` at 1 (or drop the double-source), and add a per-image batch>1 test.

3. **[minor] `estimated_usd` is still priced before `reconcile`.** `src-tauri/src/commands.rs:986` prices via `estimate_with_provenance` from `Billable::from(&input.settings)`, while fal's field coercion runs later in `submit_to_provider` → `fal_schema::reconcile` (`src-tauri/src/app.rs:441`). The `pin-aware-cost-estimate` backlog item is marked DONE by adding a *settled* actual cost + advisory rather than re-pricing the estimate post-reconcile — defensible under the backlog's "or from the settled body" wording, and unreachable for today's landed set (Veo Extend's 7s mirrored in the spec). But the pre-submit number a user sees can still be stale if fal pins a cost-bearing field; only the post-completion actual/advisory corrects it. Acceptable; note for the record.

4. **[minor / dead-ish] `ActualSource::Provider` is never produced.** `crates/hickeyfield-core/src/cost.rs:346` defines `Provider`, and `apply_poll` writes `actual_usd` from a poll but never sets `actual_source` (engine.rs:471) — so a provider that ever reports a charge would land an `actual_usd` with a `None` source. Inert for fal (which reports no cost; actual is computed at download). Documented as forward-looking, so acceptable, but strictly it is an unreachable variant + an unset field on that path today.

5. **[minor / UI edge] `priceStatusNote` "live" labelling.** `pricing.rs:135` sets `live = feed.problems().is_empty()`; a bundled snapshot that carries a timestamp and has no problems before the first live fetch could momentarily read `prices N old · live` rather than `· snapshot`. `start_refresh` runs at launch and the age-less bundled case reads `prices: bundled`, so the window is small. Cosmetic.

## Verdict

Slice delivers AC1–AC7 against the diff: estimator-honesty fixes are correct and tested, the floor is data-driven and dormant-by-design, the ffmpeg probe is robust and fails safe to `None`, the actual-cost path prices the settled quantity (unit × measured) and never zero, migration 7 is additive with genuine write-once semantics and no S1 regression, the live seam is key-safe and money-safe, and the UI is parity-correct with locked test classes. The full keyless gate reproduces green (816/0, clippy/fmt/provenance clean, audit_fal unchanged, MISMATCH 0, UI 245/build). All findings are minor and either dormant, pre-existing, or honestly disclosed; none is a blocker or a major on a critical path.

VERDICT: PASS

---

## RE-REVIEW (HEAD 63e6fc2)

Read-only re-review of the follow-up delta only (`git show 63e6fc2`). No key, no money, live seam never run.

### Gate re-run (mine, keyless, HEAD 63e6fc2)

- `cargo test --workspace` → **820 passed, 0 failed** (core lib 690, integ 3+2, tauri 125; 11 ignored). Matches the coordinator's 820/0.
- `cargo fmt --all --check` clean; `cargo clippy --workspace --all-targets -- -D warnings` → 0; `lint-provenance.py` → PASS.
- `--example price_check` (keyless) → **MISMATCH 0 / OK 11 / PROSE 46 / UNPUBLISHED 8 / NO-SNAPSHOT 1** — unchanged. The two corrected literals are `PerSecondTiered`, which the audit's `comparable_literal` leaves as PROSE, so there is no audit-count regression.

### Judged against the snapshot prose (the audit log carries fal's own sentences)

- **`minimax/h3`** — prose: *"Video costs $0.05 per second at 480p, $0.06 per second at 768p, $0.13 per second at 2K and $0.16 per second at 4K."* Literal `[(480,0.05),(768,0.06),(1440,0.13),(2160,0.16)]` — 2K→1440, 4K→2160, each rate exact. The smallest-first `height <= max_h` lookup maps 480/768/2K/4K correctly and rounds an unlabelled 1080p up into the 2K tier (reasonable). The old flat `$0.26/s` over-quoted 1.6×–5× per tier; the fix is faithful. Dependent tests updated honestly (2K/5s = $0.65 = 0.13×5; fal now undercuts VAIG at 720p, flipping the cheapest-provider assertion — the old "fal is exactly 2× VAIG" encoded the over-quote). **Correct.**
- **`grok-imagine-video/edit-video` & `/extend-video`** — prose: *"…at 480p, every second costs $0.06 ($0.05 output + $0.01 input), and at 720p, every second costs $0.08 ($0.07 output + $0.01 input)."* Literal `[(480,0.06),(720,0.08)]` — the output+input **total** per second. Since an edit/extend always carries a video input, billing the total (not the bare $0.05 output rate the old literal used) is the correct read. **Correct.**

### Security L1 — rate-derived $0 → None (cost.rs `estimate`/`estimate_inner`)

`estimate` now wraps `estimate_inner` and returns `None` unless `est.usd.is_finite() && est.usd > 0.0`, with `Flat` exempt so an authored free route (`z_image` local) still prices $0.00. A poisoned feed rate of 0 for a per-second/image/megapixel/tiered shape now reads unknown, not free. Test `a_zero_rate_is_unknown_not_free_but_flat_zero_stays_free` covers all arms. The wrapper does not affect the audit (which compares literals via `comparable_literal`, not `estimate`) nor the floor path. **Sound.**

### Batch fix — confirmed no under-count (runner.rs `settled_billable`)

For an image-output job (`images>0 && videos==0`), `batch` is held at `1` so per-image `estimate` no longer squares the count. The billed quantity is the **actual number of image outputs** fal returned (`b.images = images`), which is exactly fal's per-image basis — a 3-image job prices 3×$0.15 = $0.45 (`a_batched_image_job_is_not_double_counted`), not 1×rate and not 9×rate. **No under-count of multi-image jobs.** Video keeps the requested `batch` (its `images` is ignored by the per-second shape), which is right. Benign edge: a mixed image+video result set (`videos>0`) would retain the requested batch, but per-image models do not emit video outputs, so it is unreachable.

### Security M1 — ffmpeg probe timeout/kill (ffmpeg.rs `run_probe`)

Checked for thread leaks and zombies:
- **Normal exit:** `try_wait` → `Some` → both drain threads reach EOF when the child's pipes close → `join()` returns; `try_wait` reaps the child. No leak, no zombie.
- **Timeout:** `child.kill()` then `child.wait()` reaps the process; the closed pipes EOF the drain threads → both `join()`. No leak, no zombie.
- **Deadlock safety:** stdout and stderr are drained on separate threads and the drain loop keeps reading (discarding past the cap) so a full pipe cannot block `try_wait`; `bounded_push` caps memory at 64 KiB and truncates a straddling chunk rather than dropping it. `-nostdin` + `Stdio::null()` stdin means no stdin block. `-protocol_whitelist file` precedes `-i` (asserted by `probe_args_sandbox_the_untrusted_input`), refusing http/concat chaining. **Sound.**

### Findings (this delta)

1. **[minor] `run_probe` error path leaks on the near-unreachable `try_wait` Err.** `crates/hickeyfield-core/src/ffmpeg.rs:775` (the `Err(e) => return …` arm inside the wait loop) returns without `join`-ing the two drain threads or reaping the child. `try_wait` erroring is effectively unreachable (it would need an OS-level `waitpid` failure), and the drain threads will still terminate when the child eventually exits, but strictly this one path skips the kill/reap the rest of the function guarantees. Cosmetic; recommend a `child.kill()`+`wait()` and thread `join` before the early return.

2. **[minor, carried] `price-audit-tiered-prose` deferred, not built.** My prior finding (b) — extending the hermetic teeth to the regular resolution tables — is logged to `.forge/backlog.md` with a reason rather than implemented. The two most-wrong literals this delta corrects (`minimax/h3`, grok edit/extend) were caught by the human verifier reading prose, not by a machine check, which is exactly the gap that backlog item names. Acceptable to defer; the coverage limitation remains honestly disclosed.

The two prior minor findings this delta targeted — the per-image `batch²` (my (c)) and, from SECURITY, the probe hardening and rate-derived $0 — are resolved. No new blocker or major on a critical path; the corrected literals are faithful to fal's prose and reduce real over-quotes.

VERDICT: PASS
