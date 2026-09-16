# S10-pricing-accuracy-and-cost-tracker — IMPLEMENT (claim)

Author: implementer agent. Built exactly the S10 slice (AC1–AC7). Every command below
was run in this session with the dummy keys the task specified; outputs are pasted verbatim.
No job was submitted, no money spent, `first_light` never run, and the live AC6 seam was
NOT executed (no real key exists).

---

## Files changed

### core (`crates/hickeyfield-core/`)
- `src/prices.rs` — **AC1**: added `AuditRow`, `AUDIT_TOLERANCE` (1.2), `comparable_literal`,
  `fal_rate_from_prose`, `compare`, `classify_endpoint`, and the public `audit()` function; a
  `BTreeSet` import; module doc paragraph on the audit; 4 new tests. Also `Origin` was already
  public (used by the shell provenance code).
- `examples/price_check.rs` — rewritten to print the hermetic audit as sectioned counts (the
  AC1 evidence artefact). Keyless, offline.
- `src/cost.rs` — **AC2**: `PerSecond` and `PerToken` return `None` on a missing billable
  quantity (was `unwrap_or(0.0)`); `estimate` doc-comment amended; new `per_second_rate` helper
  used to floor per-second routes; new `PriceProvenance` and `ActualSource` enums (kebab serde);
  2 new AC2 tests.
- `src/route.rs` — **AC2**: `Route` gains `min_billed_seconds: Option<f64>` (`#[serde(default,
  skip_serializing_if)]`) and a `Route::floored(..)` constructor.
- `src/registry.rs` — **AC2**: `Model::estimate` applies `with_minimum_seconds` iff the route
  declares a floor and the shape is per-second; 1 new floor test.
- `src/ffmpeg.rs` — **AC3**: `MediaDims`, `parse_probe` (pure stderr parser), `parse_dimensions`,
  `probe_file` (sidecar `ffmpeg -i`); 3 pure fixture tests + 1 `#[ignore]` real-sidecar test.
- `src/engine.rs` — **AC3**: `JobSet` gains `priced_from`/`priced_at`/`actual_source`
  (`#[serde(default)]`); `apply_poll` counts a cost-only `actual_usd` delta as a change; 1 new
  `apply_poll` test.
- `tests/live_fal_pricing.rs` — **AC6**: new `#[ignore]` human-run seam (key from env only,
  never logged, skips when absent, uses the GET price lookup, spends nothing).

### shell (`src-tauri/`)
- `src/pricing.rs` — **AC3**: `estimate_with_provenance` returns `(Estimate, PriceProvenance)`.
- `src/store.rs` — **AC3**: migration 7 adds `priced_from`/`priced_at`/`actual_source`;
  `row_to_job` reads them; upsert writes them with `priced_from`/`priced_at` **write-once**
  (absent from the `ON CONFLICT` set) and `actual_source` in the update set; v1-replay test drops
  the new columns; new `migration_7_adds_provenance_columns_and_roundtrips` test.
- `src/commands.rs` — **AC3**: `submit_job` prices once via `estimate_with_provenance` and records
  `estimated_usd`/`priced_from`/`priced_at`.
- `src/runner.rs` — **AC3/AC4**: `Runner` carries `Arc<Prices>`; `download_outputs` probes the
  settled file, prices the settled billable (`compute_actual`/`settled_billable`), writes
  `actual_usd`/`actual_source`, and pushes a `quantity_advisory` when settled ≠ requested; 3 new
  tests.
- `src/app.rs` — builds `Prices` before the `Runner` and passes it in.

### UI (`ui/`)
- `src/types.ts` — `PriceStatus` interface.
- `src/api.ts` — `priceStatus()` bridge + mock fallback.
- `src/lib/cost.ts` — `costDelta`, `formatDelta`, `priceStatusNote` (+ private `humanAge`).
- `src/components/MetaCard.tsx` — additive **Delta** row (only when both figures are real).
- `src/components/MetaRail.tsx` — fetches `priceStatus` on mount, renders a source note.
- `src/components/meta.css` — additive `.spend-meter-source`, `.meta-card-delta[data-tone]`.
- `src/components/MetaCard.test.tsx` (new) + `src/lib/__tests__/cost.test.ts` (extended).

### docs
- `CHANGELOG.md` — `[Unreleased]` Added (actual cost + audit) and Fixed (unknown≠$0.00, floor
  wired) bullets.
- `docs/FIRST-LIGHT.md` — new "Verifying prices against the live fal API" section; cost-accuracy
  item updated.
- `.forge/backlog.md` — `pin-aware-cost-estimate` marked DONE (S10).

---

## AC1 — hermetic price audit (this was the design's central UNVERIFIED claim)

The design (§0.2) hypothesised that `fal_catalogue::parse_pricing` reduces **zero** registry fal
routes to a comparable `Rate`, making the audit vacuous. **I ran it — the hypothesis was correct:**
the strict catalogue parser produced 0 OK/0 comparable rows, everything Prose/Unpublished.

Per the task ("an audit that checks nothing is not acceptable; extend parse_pricing where the
prose shape is regular"), I gave the audit teeth — see the **deviation** note below for *how*.
Final run:

```
$ env -u hickeyfield_FAL_KEY -u FAL_KEY cargo run -q -p hickeyfield-core --example price_check
hermetic fal price audit (tolerance 1.2x)
endpoints audited: 66
...
=== OK (literal within tolerance of fal) (11) ===
  fal-ai/kling-video/o3/pro/image-to-video   literal $0.1120 ~ fal $0.1120 per second of video
  fal-ai/kling-video/o3/pro/text-to-video    literal $0.1120 ~ fal $0.1120 per second of video
  fal-ai/kling-video/v2.6/pro/text-to-video  literal $0.0700 ~ fal $0.0700 per second of video
  fal-ai/kling-video/v3/pro/text-to-video    literal $0.1120 ~ fal $0.1120 per second of video
  fal-ai/kling-video/v3/standard/image-to-video  literal $0.0840 ~ fal $0.0840 per second of video
  fal-ai/kling-video/v3/standard/text-to-video   literal $0.0840 ~ fal $0.0840 per second of video
  fal-ai/nano-banana-2                       literal $0.0800 ~ fal $0.0800 per image
  fal-ai/nano-banana-2/edit                  literal $0.0800 ~ fal $0.0800 per image
  fal-ai/nano-banana-pro                     literal $0.1500 ~ fal $0.1500 per image
  fal-ai/nano-banana-pro/edit                literal $0.1500 ~ fal $0.1500 per image
  fal-ai/veo3.1/extend-video                 literal $0.2000 ~ fal $0.2000 per second of video

MISMATCH 0 / OK 11 / UNMODELLABLE 0 / PROSE 46 / UNPUBLISHED 8 / NO-SNAPSHOT 1
coverage: 66 endpoints checked / 57 with fal prose / 8 with null prose
```

**Result: MISMATCH 0** — the design's expectation confirmed by a run, now with 11 rows actually
compared against fal's own published numbers (not vacuous). The 8 `Unpublished` rows include the
two audio literals the design flagged (`bytedance/seed-audio-1.0`, `mirelo-ai/sfx1.6/text-to-audio`)
— fal publishes no price for them, so they are AC6 line items, not fixable in-slice; no literal was
found to be wrong, so none was changed.

**Coverage: 66 endpoints checked / 57 with fal prose / 8 with null prose / 1 not in snapshot.**

### 15s-floor evidence (D2, §2.2)

Grepped the vendored snapshot prose for `minimum` / `at least` / `billed`:

```
$ python3 (over vendor/fal-catalogue-snapshot.json) — models whose pricing prose names a floor:
fal-ai/flux-kontext-trainer, fal-ai/flux-lora-portrait-trainer, fal-ai/flux-pro/v1/erase,
fal-ai/heygen/v3/filler-word-removal (60s input min), fal-ai/krea-2-trainer,
fal-ai/pika/v2.2/pikaframes (5 billable-second min), fal-ai/qwen-image-2512-trainer,
fal-ai/qwen-image-edit-2509-trainer, fal-ai/smart-resize, fal-ai/trellis-2-lora-trainer,
fal-ai/wan-22-image-trainer, fal-ai/wan-22-trainer/{i2v,t2v}-a14b, fal-ai/z-image-trainer,
fal-ai/z-image-turbo-trainer-v2, minimax/h3-max/director, veed/subtitles
```

**None of these is one of the 66 registry fal endpoints.** So D2 holds on evidence: **zero**
current routes carry a fal-stated floor. I wired the mechanism data-driven (`Route::floored` +
`Model::estimate`) and applied it to no route, rather than hardcoding a 15s constant that would
over-quote every short clip.

AC1 tests: `the_audit_is_hermetic_and_finds_no_literal_beyond_tolerance`,
`the_audit_actually_compares_something` (non-vacuity), `a_wrong_literal_is_caught` (mutation guard),
`an_unmodellable_unit_is_reported_not_failed`.

### DEVIATION from the design/task wording (declared, with reason)

The task said "extend `parse_pricing`". There are two parsers; the audit reads
`fal_catalogue::parse_pricing`. I did **not** modify it. Reasons: (1) it is a whole-catalogue
function governing the UI price surface for all 1,491 models, with a hard test asserting exactly
193 parsed rates and a documented "73% refusal is the feature" S8/S9 contract; loosening it to take
the audio-off base as "the rate" is the exact "take the wrong number from a rate table" anti-pattern
it was written to prevent, and would show users an incomplete number on catalogue cards. (2) It
would break `the_bundle_prices_exactly_what_was_measured_and_refuses_the_rest` and risk the
"audit_fal unchanged" gate.

Instead I gave the audit teeth surgically: when the strict parser refuses a route's prose, the audit
recovers fal's base rate from the **same prose** via the already-public, already-tested feed parser
`prices::fal::parse_prose` (the audio-off/on template and the `$X per image`/`per second` base with
the illustration trimmed). Both numbers are fal's own; the strict catalogue surface is untouched;
the audit now compares 11 real rows. This honours the intent (non-vacuous audit) with far lower
blast radius. If the reviewer prefers the literal reading, moving this recovery into
`fal_catalogue::parse_pricing` is mechanical but would require updating the `193` constant and its
rationale.

---

## AC2 — estimator honesty

`unknown_duration_is_none_not_zero_per_second`, `unknown_duration_is_none_not_zero_per_token`,
`a_floored_route_bills_the_minimum` all pass (see workspace run). The floor is applied only to a
route carrying `min_billed_seconds` (none today), so no short clip is over-quoted.

---

## AC3 — actual cost per job

- No `ffprobe` is bundled — confirmed: `src-tauri/binaries/` holds only
  `ffmpeg-aarch64-apple-darwin`. The probe uses `ffmpeg -i` stderr (`parse_probe`).
- Pure fixture test built from captured real sidecar stderr; real-sidecar `#[ignore]` test run:

```
$ cargo test -q -p hickeyfield-core --lib ffmpeg::tests::probe_file_reads_a_real_sidecar_output -- --ignored
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 695 filtered out
```

- `PollResult.actual_usd` → `apply_poll` cost-only early-return fixed
  (`apply_poll_persists_a_cost_only_delta`). fal reports no cost, so the per-job actual is computed
  at download from the settled output (`compute_actual`), the only path that yields a fal actual.
- Migration 7 + write-once semantics: `migration_7_adds_provenance_columns_and_roundtrips` asserts
  `user_version==7`, round-trip, `priced_from`/`priced_at` frozen on re-upsert, `actual_source`
  updated.
- AC3 named cases: `actual_is_priced_from_the_settled_quantity` (10s == 2×5s, == estimator's own
  quote for the settled billable, source `Probe`) and `an_unmeasurable_output_gets_none_not_zero`
  (all-`None` dims → `actual_usd == None`, never 0).

## AC4 — advisory

`a_settled_quantity_that_differs_carries_an_advisory` (7s@1080p vs requested 5s@720p → one advisory
naming both; equal → none).

## AC5 — UI

`MetaCard.test.tsx` (5 tests): estimate-only shows no delta + "Not reported"; actual-known shows the
figure; both present shows a signed, toned delta (over/under). `cost.test.ts` extended with
`costDelta`/`priceStatusNote`. Test-locked class names preserved (`meta-card`, `meta-card-fields`,
`meta-card-advisories`, `spend-meter`, `spend-meter-value`, `spend-meter-note`, `data-meta-anchor`);
new elements only add classes. `price_status` now has a UI caller (MetaRail source note).

## AC6 — live seam

`tests/live_fal_pricing.rs`, `#[ignore]`, reads key from `hickeyfield_FAL_KEY`/`FAL_KEY` only, skips
(does not fail) when absent, never logs the key, uses the GET price lookup (no generation). Compiles
and is correctly skipped keyless. **NOT run** — no real key, and per instructions. Documented in
`docs/FIRST-LIGHT.md`.

---

## AC7 — the gate (all pasted, all green)

```
cargo test --workspace          → 816 passed, 0 failed, 11 ignored
  (core lib 687, tauri 124, core integration 3+2, live_fal 1 ignored)
cargo fmt --all --check         → FMT CLEAN
cargo clippy --workspace --all-targets -- -D warnings → Finished, no warnings
./scripts/lint-provenance.py    → PASS (exit 0)
env -u hickeyfield_FAL_KEY -u FAL_KEY cargo run -q -p hickeyfield-core --example audit_fal
  → PINNED FIELDS (0), UNREACHABLE (0), CHECKED (66)  [unchanged from S9]
env -u ... --example price_check → MISMATCH 0 / OK 11
cd ui && pnpm test              → 245 passed (15 files)
cd ui && pnpm build             → built in ~0.8s, tsc clean
CHANGELOG [Unreleased]          → Added + Fixed bullets, provenance lint passes
```

---

## Known limitations / honest gaps (not hidden)

1. **AC1 hard-fail surface is 11 rows, not all 66.** The other 46 fal endpoints are conditional
   rate tables neither parser reduces without risking the wrong number; they are `Prose`, verifiable
   only via the AC6 live seam (a human-run job), not hermetically. This is the design's §12 weakest
   link and is called out in the module doc and `price_check` output.
2. **`.app` build / launch not run.** The task's gate list did not include `scripts/build-macos.sh`;
   `cargo build --workspace` compiles and clippy is clean, but I did not build or launch the bundle.
   RELEASE/verifier should confirm the window opens.
3. **Actual cost depends on the sidecar being next to the running exe.** `probe_first_local` resolves
   `current_exe().parent()`; in a dev build with no fetched binary, `locate` fails and the actual is
   `None` (honest unknown), never 0 — but this means the actual-cost path is only exercised
   end-to-end in a packaged build (the real-sidecar behaviour is covered by the `#[ignore]` probe
   test against the repo binary).
4. **Two `Unpublished` audio literals** (`seed-audio`, `mirelo`) have per-second literals with no fal
   backing; fal publishes nothing, so they cannot be verified in-slice and are AC6 items.
5. **`actual_source` for a probed per-image route** is `Probe` when any dimension came off the file,
   else `OutputCount`; a per-megapixel image priced purely from probed dims reads as `Probe`, which
   is accurate.

---

## ADDENDUM — post-verification repairs (coordinator round 2)

Four in-slice repairs after TEST/REVIEW/SECURITY. Uncommitted; coordinator commits.

### 1. AC1 — the two genuinely-wrong fal literals the verifier flagged, now fixed to fal's prose
Both were pre-existing `Prose` rows the audit's parse-to-Rate mechanism structurally cannot flag,
but AC1 says every mismatch found is fixed in-slice. Corrected in `registry.rs::priced_routes`
against `vendor/fal-catalogue-snapshot.json`, quoting the prose in each route note:

- **`minimax/h3` (fal):** was `per_second(0.26)` flat (over-quoted 1.6x–5x). fal prose: *"Video costs
  $0.05 per second at 480p, $0.06 per second at 768p, $0.13 per second at 2K and $0.16 per second at
  4K."* → `PerSecondTiered { [(480,0.05),(768,0.06),(1440,0.13),(2160,0.16)] }` (the published table,
  no made-up average).
- **`grok_edit_video` / `grok_extend_video` (fal):** were `per_second(0.05)` (dropped the input
  surcharge, under-quoted 720p). fal prose: *"at 480p, every second costs $0.06 ($0.05 output +
  $0.01 input), and at 720p, every second costs $0.08 ($0.07 output + $0.01 input)."* An edit/extend
  always has a video input, so the billed rate is the total per second →
  `PerSecondTiered { [(480,0.06),(720,0.08)] }`.

Two dependent registry tests encoded the *old over-quote* and were corrected (with comments):
`gateway_routes_win_where_the_plan_says_they_do` asserted "fal H3 is 2× VAIG" — the corrected fal now
equals VAIG's $0.13/s at 2K and undercuts it at 720p; and
`the_cheapest_provider_by_price_is_the_arbitrage_one` now expects H3 cheapest on **Fal** at 8s/720p
($0.48 vs VAIG $1.04) — the price fix flipped the arbitrage. The VAIG H3 note ("exactly half fal")
was corrected too.

New `price_check` table (keyless): unchanged counts — the corrected routes stay `Prose` because fal
states them with resolution words the strict parser refuses:
```
MISMATCH 0 / OK 11 / UNMODELLABLE 0 / PROSE 46 / UNPUBLISHED 8 / NO-SNAPSHOT 1
coverage: 66 endpoints checked / 57 with fal prose / 8 with null prose
```

**Tiered-prose parser (reviewer's optional suggestion): NOT added, logged to backlog** as
`price-audit-tiered-prose`. A tier-by-tier prose parser would cover wan/ltx/minimax/grok-v1.5, but a
robust one exceeds the ~150-line budget the coordinator set: the prose is irregular and several rows
carry multiple figures near one resolution token (grok edit/extend's `$0.06 ($0.05 output + $0.01
input)`, veo3.1's audio+resolution pair), and an `AuditRow::Mismatch` **fails the build** — a
mis-pair would break CI on a correct price, the exact hazard `fal_catalogue::parse_pricing` avoids.
Scoped as its own slice with per-shape fixtures. Reasoning recorded in `.forge/backlog.md`.

### 2. Security M1 — the `ffmpeg -i` probe is now bounded and sandboxed
`crates/hickeyfield-core/src/ffmpeg.rs`: new `run_probe` replaces the plain `Command::output()` for
the untrusted-media probe. No new dependency (`wait-timeout` is not in `Cargo.lock`) — uses a
thread-drained pipe + `try_wait` deadline + `child.kill()`:
- **20s wall-clock timeout with kill** (`PROBE_TIMEOUT`); on overrun → `FfmpegError::Probe`, which
  `probe_first_local` maps to all-`None` dims → `actual_usd = None`, never 0.
- **`-nostdin`** and **`stdin(Stdio::null())`** (never blocks on stdin).
- **`-protocol_whitelist file` before `-i`** (an input cannot chain to `http`/`concat`/etc.).
- **stderr/stdout capture capped at 64 KiB** (`MAX_PROBE_CAPTURE`, `bounded_push`), drained on
  threads so a full pipe cannot deadlock the wait.

Tests: `probe_args_sandbox_the_untrusted_input` (argv: `-nostdin`, whitelist-before-`-i`, no `-y`),
`probe_capture_is_bounded` (cap enforced, chunk truncated not dropped). The real-sidecar
`#[ignore]` probe still passes through the hardened path. The **timeout+kill path is not
fixture-tested** — making the real sidecar hang deterministically without a network or a crafted
malicious file is not feasible in a keyless unit test, and a sleep-based fake binary would be a
platform-specific fixture; the deterministic pieces (argv, bounded capture) are unit-tested and the
timeout wiring runs end-to-end in the ignored probe. Recorded here per the task.

### 3. Security L1 — a rate-derived `$0` now reads as unknown, not free
`crates/hickeyfield-core/src/cost.rs`: `estimate` is now a guard over `estimate_inner` — a
non-positive or non-finite computed usd returns `None` (a poisoned/erroneous feed publishing
`unit_price = 0`, or a mistyped literal, must not render "$0.00 free"). **`Flat` is exempt**: there
IS a genuinely free route (`z_image`, open weights on a local ComfyUI endpoint,
`CostModel::Flat { usd: 0.0 }`) that must still price as $0.00, and the feed's own `cost_is_sane`
already refuses a non-positive `Flat`, so no bad data reaches that arm. Test
`a_zero_rate_is_unknown_not_free_but_flat_zero_stays_free` covers PerSecond/PerImage/PerSecondTiered
$0 → `None`, `Flat{0.0}` → `$0.00`, and a normal positive rate unaffected.

### 4. Review (c) — `batch²` double-count fixed
`src-tauri/src/runner.rs::settled_billable`: `Billable::from` sets both `images` and `batch` to the
requested batch, and per-image `estimate` multiplies by both. For an image-output job the settled
image count IS the whole quantity, so `batch` is held at 1 (video keeps the requested batch, since
`images` is ignored for a per-second shape). Test `a_batched_image_job_is_not_double_counted`: a
batch-of-3 image job at $0.15 → `$0.45`, not `$1.35`, source `OutputCount`.

### Gate (round 2, all green, dummy keys; live seam / first_light never run)
```
cargo test --workspace   → 820 passed, 0 failed, 11 ignored
  (core lib 690, tauri 125, integration 3+2, live_fal 1 ignored)
cargo fmt --all --check  → FMT OK
cargo clippy --workspace --all-targets -- -D warnings → Finished, no warnings
./scripts/lint-provenance.py → PROV PASS (0)
env -u hickeyfield_FAL_KEY -u FAL_KEY cargo run -q -p hickeyfield-core --example audit_fal
  → PINNED (0), UNREACHABLE (0), CHECKED (66)  [unchanged]
env -u ... --example price_check → MISMATCH 0 / OK 11
cd ui && pnpm test → 245 passed (15 files)
cd ui && pnpm build → built, tsc clean
```

New tests this round: `a_zero_rate_is_unknown_not_free_but_flat_zero_stays_free` (cost.rs),
`probe_args_sandbox_the_untrusted_input` + `probe_capture_is_bounded` (ffmpeg.rs),
`a_batched_image_job_is_not_double_counted` (runner.rs), plus two updated registry assertions.
