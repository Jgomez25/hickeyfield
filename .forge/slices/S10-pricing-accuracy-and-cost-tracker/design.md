# S10-pricing-accuracy-and-cost-tracker — DESIGN

Contract: `.forge/slices/S10-pricing-accuracy-and-cost-tracker/slice.md` (AC1–AC7).
Upstream this design is built on: `.forge/objective.md` (§Success B money-trust, §Constraints,
Q6), `.forge/slices/S9-fal-schema-const-and-int-enums/design.md` (house style; the pinned-field
work AC4 re-checks), `.forge/backlog.md:221` (`pin-aware-cost-estimate`, this slice's to fix).
S9's `design.md` is the house style followed here.

Slice shape: **one hermetic audit function + two one-line estimator-honesty fixes + one
per-job actual-cost path (probe → provenance columns → apply_poll fix) + a small UI
delta/provenance surface + one human-run live seam.** No new dependency. One store migration
(7). The UI already renders estimated-vs-actual and a fallback spend meter (§5), so `ui/`
changes are additive and small.

---

## 0. Session constraint, stated before anything is claimed

This design was authored **without shell access** — this environment exposes no Bash tool, so
**no `cargo`, no `curl`, no `git` ran.** Every `file:line` below was read from the working tree
at `584b547` this session and is first-hand. Every fal price figure below is quoted from the
**vendored** snapshot `crates/hickeyfield-core/vendor/fal-catalogue-snapshot.json`
(`"snapshot_date": "2026-09-16"`), read directly, and is cited to its line.

Because I could not execute the audit, the mismatch list in §1.4 is derived by **reading** each
registry literal against the snapshot's `pricingInfoOverride`. It is a hypothesis, not a run.

> **MUST, first task of IMPLEMENT (AC1 evidence):** build the audit, then run it keyless and
> paste the output into `implement.md`:
> ```
> cargo run -q -p hickeyfield-core --example price_check | tee \
>   .forge/slices/S10-pricing-accuracy-and-cost-tracker/price_check-<date>.log
> ```
> If the run disagrees with §1.4, **the run wins** and this design is amended in `implement.md`.
> No fal *network* call and no key is needed or permitted; the audit reads the vendored
> snapshot only. `first_light.rs` MUST NOT run — no key exists and it spends money.

### 0.1 Ground truth, as read this session

| Fact | Read at |
|---|---|
| `actual_usd` is hardcoded `None` in both provider clients | `clients.rs:240` (fal `poll`), `clients.rs:395` (Higgsfield `poll`) |
| `apply_poll`'s `changed` gate reads only status/results/fail_reason; a **cost-only** delta returns `false` and is dropped before the `actual_usd` write at `:450-452` | `engine.rs:422-427`, `:450-452` |
| `estimate` uses `b.seconds.unwrap_or(0.0)` in `PerSecond` and `PerToken` → a confident **$0.00** when duration is unknown | `cost.rs:182`, `cost.rs:203` |
| `with_minimum_seconds` (fal's floor) is defined and **never called** in `src/` (grep: only its own tests reference it) | `cost.rs:263-283` |
| `estimated_usd` is written **once** at insert — it is absent from the `ON CONFLICT DO UPDATE SET` list; `actual_usd` **is** in the update set | `store.rs:249-268` |
| Latest migration is **6**; next is **7** | `store.rs:145-155` |
| `Billable::from(&SettingsDto)` derives seconds/width/height/megapixels/images/audio/batch from the settings blob | `commands.rs:613-637` |
| `estimated_usd` is priced from the pre-`reconcile` settings at submit | `commands.rs:1013`, backlog `pin-aware-cost-estimate` |
| `PollResult{status,outputs,fail_reason,actual_usd}`; `Output{url,kind,local_path}` | `engine.rs:175-180`, `:94-102` |
| The bundled sidecar is **`ffmpeg` only** — `SIDECAR_STEM = "ffmpeg"`, resolved via `ffmpeg::locate(dir)` to `ffmpeg-aarch64-apple-darwin` or bare `ffmpeg`. **No `ffprobe` is bundled.** `ffmpeg.rs` has `run()` (private) and stderr/stdout parsers (`parse_encoders`, `parse_licence`) but **no input-file duration/resolution probe** | `ffmpeg.rs:28`, `:47-77`, `:446-463` |
| The UI already: shows Estimated + Actual per job (`MetaCard.tsx:141-154`), maps `actual_usd`→`actualUsd` (`api.ts:465-466`), and the spend meter already prefers actual, falls back to estimate, and labels it (`MetaRail.tsx:26-43`, `lib/cost.ts:87-142`) | as cited |
| `price_status` command is registered but **has zero UI callers** (only `estimate_cost` is invoked) | `lib.rs:80`, grep of `ui/` finds `estimateCost` only |
| fal's index snapshot is the AC1 source: `Catalogue::bundled()` → `Model::pricing()` → `Pricing::{Rate{usd,unit,prose},Unparsed{prose},Unpublished}`; `parse_pricing` refuses any prose with >1 figure, a resolution/conditional word, or an unmodelled unit | `fal_catalogue.rs:585`, `:430`, `:330-344`, `:1185-1250` |

### 0.2 The single most important finding, stated up front

`parse_pricing` is **deliberately strict**: it returns `Pricing::Rate` only for prose that names
exactly one unconditional dollar figure in a unit it models (`fal_catalogue.rs:1173-1184`). fal
writes almost every video/image rate as a **conditional table** ("$0.12/s for 720p or $0.17/s
for 1080p", "$0.15 per image … 4K outputs double") — which `parse_pricing` refuses.

Reading all ~40 registry fal endpoints against the snapshot (§1.4), **essentially none parse to a
`Rate` in a unit `CostModel` can express.** The one that parses cleanly — `fal-ai/inworld-tts`
→ `Rate{$0.01, ThousandCharacters}` — is priced `CostModel::Unknown` on purpose (`Billable` has
no character count, `registry.rs:1532-1539`).

**Consequence for D1:** with today's registry + snapshot the AC1 *hard-fail* surface is
**empty** — I found no wrong literal it would fail on. The audit's value is therefore (a) a
**regression guard** (a future literal that contradicts a clean fal `Rate` fails the build), and
(b) a **coverage report** that prints the ~40 conditional/unpublished routes for human eyes and
the live seam (AC6). The design must say this honestly rather than promise teeth it does not have,
and AC1's test must be **non-vacuous** (§8) so "0 mismatches" cannot mean "the audit checked
nothing".

---

## 1. AC1 — the hermetic price audit (D1)

### 1.1 Where it lives and its shape

Promote the ad-hoc comparison in `examples/price_check.rs` into a **pure, testable function in
core**, `crates/hickeyfield-core/src/prices.rs`:

```rust
/// The outcome of comparing one route's literal `CostModel` against fal's own
/// published rate for the endpoint(s) it resolves to.
#[derive(Debug, Clone, PartialEq)]
pub enum AuditRow {
    /// fal publishes a single modellable rate and the literal is within tolerance.
    Ok { endpoint: String, unit: PriceUnit, fal_usd: f64, literal_usd: f64 },
    /// fal publishes a single modellable rate the literal contradicts beyond tolerance.
    /// This is the only variant that fails the build.
    Mismatch { endpoint: String, unit: PriceUnit, fal_usd: f64, literal_usd: f64, ratio: f64 },
    /// fal publishes a rate in a unit `CostModel` does not express (tokens, chars,
    /// compute-second). Reported, never failed — `Unknown` is the honest literal.
    UnmodellableUnit { endpoint: String, unit: PriceUnit, fal_usd: f64 },
    /// fal's prose did not reduce to a rate (tiered/conditional/multi-figure).
    /// The prose is carried so a human can read it.
    Prose { endpoint: String, prose: String },
    /// fal published nothing for this endpoint; the literal has no fal backing.
    Unpublished { endpoint: String },
    /// The route resolved to an endpoint the snapshot does not contain.
    NoSnapshot { endpoint: String },
}

/// Compare every fal route's literal against the vendored fal index. Hermetic:
/// reads `Catalogue::bundled()` and `registry()`, no network, no key.
pub fn audit() -> Vec<AuditRow>;
```

`audit()` MUST:
1. Iterate `registry::registry()`; for each `Route` with `provider == ProviderId::Fal`, read the
   literal via `Model::cost_model(route)` (`registry.rs:65`).
2. Expand the family-root slug to concrete endpoint ids with
   `media::resolve_endpoint(&route.slug, mode, produces_video)` for `mode ∈ {Text, Image, Video}`
   (the exact expansion `fal_diff` and `audit_fal` already use — `fal_diff.rs:126-138` — so the
   three tools cannot disagree about what the registry covers).
3. Look each endpoint up in `Catalogue::bundled()` (`fal_catalogue.rs:585`, `get` at `:632`),
   read `Model::pricing()` (`:430`), and classify into an `AuditRow` per the table below.

### 1.2 What is compared, per unit

`Pricing::Rate { usd, unit, .. }` carries fal's rate and a `PriceUnit`
(`fal_catalogue.rs:269-297`). Map the literal `CostModel` to a comparable `(unit, usd)` and match
on unit:

| `PriceUnit` (fal) | comparable literal figure | source `CostModel` |
|---|---|---|
| `VideoSecond` / `AudioSecond` | `usd` (silent base) | `PerSecond.usd`; a `PerSecondTiered` compares each `(height, usd)` tier against fal only if fal is *also* a single rate (it never is — those are `Prose`) |
| `Image` | `usd` | `PerImage.usd` |
| `Megapixel` | `usd`, and `first_usd` when fal states a first-MP rate | `PerMegapixel` |
| `Generation` | `usd` | `Flat.usd` |
| `ComputeSecond`, `TrainingStep`, `ThousandCharacters`, `VideoMegapixel`, `InputAndOutputMegapixel`, `InputVideoSecond`, `Minute` | — | → `UnmodellableUnit` (report, never fail): `Billable`/`CostModel` has no term for these; `Unknown` or a per-second approximation is the honest literal |

**Audio multipliers:** fal writes the audio rate as a *second* figure, so any endpoint with an
audio surcharge is multi-figure → `Prose`, never `Rate`. The audit therefore cannot verify a
`PerSecond.audio_multiplier` automatically; that is a `Prose` row and an AC6 (live seam) job.
Recorded as a limitation, not hidden.

### 1.3 Tolerance and the pass/fail rule (D1 decision)

**A `Mismatch` (build-failing) is declared only when `ratio = max(fal/lit, lit/fal) > 1.2`** on a
`Rate` in a modellable unit.

- Why **1.2× (20%)**, not the existing 10× (`prices.rs:1483`): that test guards a *units bug*
  (per-image vs per-second) against the coarse `prices-snapshot.json` feed. This audit compares a
  hand-typed literal against fal's own published *number*; those should agree to the cent, so 20%
  is loose enough to absorb fal's "roughly"/rounding and a genuine price drift, tight enough to
  catch a transcription error (a fat-fingered `0.15` vs `0.015` is 10×; a `0.084` vs `0.84` is
  10×; even a decimal-place slip inside a unit shows > 20%).
- **Non-`Rate` rows never fail.** `Prose`, `Unpublished`, `UnmodellableUnit`, `NoSnapshot` are
  reported and counted, never asserted. This is the "prints the prose for rows that do not parse"
  half of AC1.

### 1.4 The mismatches I expect the implementer to find (read, not run)

Reading every registry fal literal against the 2026-09-16 snapshot, **I found no build-failing
`Rate` mismatch** — the literals track fal's current prose. The rows worth the implementer's
attention, each with the fal figure and its snapshot line, are:

| endpoint | literal | fal prose (snapshot line) | classify | note |
|---|---|---|---|---|
| `fal-ai/inworld-tts` | `Unknown` | `$0.01 per 1000 character` (`:6311`) | `UnmodellableUnit` (`ThousandCharacters`) | correct as-is; `Billable` has no char count |
| `bytedance/seed-audio-1.0` | `per_second(0.003125)` | `pricingInfoOverride: null` (`:800`) | `Unpublished` | literal has **no fal backing** — flag for AC6 |
| `mirelo-ai/sfx1.6/text-to-audio` | `per_second(0.002)` | `null` (`:15144`) | `Unpublished` | note already says "approximate"; flag for AC6 |
| `xai/grok-imagine-video/edit-video`, `/extend-video` | `per_second(0.05)` | 480p $0.06/s ($0.05 out+$0.01 in), 720p $0.08/s ($0.07 out) (`:16310`) | `Prose` | 480p-output approximation; **under-quotes 720p** — a per-tier fix is optional (Q3) |
| `bytedance/seedance-2.5` | `per_token(21.40)` | `$0.0214/1000 tok` 480/720p, `$0.0234` 1080p (`:943`) | `Prose` | ~9% under at 1080p, already noted in `registry.rs:772` |
| `fal-ai/veo3.1/lite` | `per_second_audio(0.03, ~1.667)` | 720p $0.03/$0.05, 1080p $0.05/$0.08 (`:12911`) | `Prose` | correct at 720p; under at 1080p |
| `lightricks/ltx-2.5/{text,image}-to-video/pro` | `PerSecondTiered[(720,0.12),(1080,0.17)]` | `$0.12/s 720p or $0.17/s 1080p` (`:14638`,`:14660`) | `Prose` | literal matches prose exactly |
| `fal-ai/veo3.1/extend-video` | `per_second_audio(0.20, 2.0)` | `$0.20 off / $0.40 on` (`:12823`) | `Prose` | matches; audio pair unverifiable by audit |
| `fal-ai/kling-video/v3/pro` | `per_second_audio(0.112, 1.5)` | `$0.112 off/$0.168 on/$0.196 voice` (`:6960`) | `Prose` | matches |
| all other fal video/image routes (kling std, wan\*, seedance\*, nano-banana\*, flux-2-pro, seedream v5, qwen-image-3, gpt-image\*) | as in `priced_routes` | multi-figure / resolution-conditioned | `Prose` | not machine-verifiable; matches prose on inspection |

**Recorded expectation for AC7:** the audit's `Mismatch` count is expected to be **0** against the
bundled snapshot. If the implementer's run produces any `Mismatch`, each MUST be fixed in
`priced_routes` in this slice with the fal figure cited (the AC1 obligation). The two
`Unpublished` audio literals (`seed-audio`, `mirelo`) are **not** fixable here (fal publishes
nothing) — they are recorded as AC6 line items, not build failures.

### 1.5 `price_check.rs` becomes the audit's printer

Rewrite `examples/price_check.rs` to call `prices::audit()` and print a section per `AuditRow`
variant with a trailing count line (`MISMATCH n / OK n / PROSE n / UNPUBLISHED n /
UNMODELLABLE n / NO-SNAPSHOT n`), in the shape of `audit_fal.rs`'s `section` helper. The example
stays keyless and offline. It is the AC1 evidence artefact (§0).

---

## 2. AC2 — estimator honesty (D2)

### 2.1 `unwrap_or(0.0)` → `None`

`cost.rs:182` (`PerSecond`) and `cost.rs:203` (`PerToken`) MUST return `None` when the billable
quantity they need is absent, matching the pattern `PerSecondTiered` already uses
(`cost.rs:161-163`: `let (Some(seconds), Some(height)) = … else { return None };`).

```rust
// PerSecond
let Some(secs) = b.seconds else { return None };
// PerToken — needs seconds, width, height (a token count of 0 is not a price)
let (Some(secs), Some(w), Some(h)) = (b.seconds, b.width, b.height) else { return None };
```

Worked example: a video route with `Billable{seconds:None,..}` today returns
`Some(Estimate{usd:0.0,..})` — a paid model quoted **free**. After the fix it returns `None`,
which `Prices::estimate` propagates and the UI renders as `UNKNOWN_COST_LABEL` ("Price
unavailable", `lib/cost.ts:11`). This is the invariant the whole cost surface rests on
(`cost.rs:340-343`, `pricing.rs:158-174`) applied to the two paths that violated it.

`PerToken`'s `fps` still defaults via `b.fps.unwrap_or(*fps)` (`cost.rs:206`) — fps has a real
default (24), duration and resolution do not.

### 2.2 Which routes get `with_minimum_seconds` (D2 decision)

`with_minimum_seconds` (`cost.rs:263`) MUST be applied **only to fal per-second routes whose fal
prose states a minimum** — "minimum", "billed at least", "at least Ns". Reading the snapshot,
**no registry fal route's `pricingInfoOverride` states a floor.** The Hummingbird 15s example the
doc-comment cites (`cost.rs:260-262`) is **not a registry route** (grep: no `hummingbird` in
`registry.rs`).

**Decision: apply the floor to zero routes in this slice, and wire the mechanism so it is
*ready* but *data-driven*, not hardcoded.** Concretely:

- Add an optional `min_billed_seconds: Option<f64>` to the `Route` construction path used by
  `priced_routes` (a `Route::floored(...)` helper beside `Route::noted`, `route.rs:18-45`), left
  `None` for every current route.
- `Model::estimate` (`registry.rs:72`) applies `CostModel::with_minimum_seconds` after `estimate`
  **iff** the route carries a floor and the model is `PerSecond`/`PerSecondTiered`.
- Add the **unit test** AC2 asks for over `with_minimum_seconds` directly (it already has two,
  `cost.rs:354-380`) plus one asserting `Model::estimate` honours a floored route.

Rationale: AC2 says "apply the 15s floor **where fal states it**"; fal states it **nowhere** in
the current roster, and inventing a floor is the same class of harm as inventing a price. Wiring
the mechanism (rather than calling it with a guessed constant) satisfies "the floor is applied
when a route declares one" and keeps `with_minimum_seconds` from being dead code, without
over-charging a route fal does not floor. **Open question Q2** records the alternative
(hardcode 15s on a named list) and why it is rejected.

**Rejected:** calling `with_minimum_seconds(…, 15.0, rate)` unconditionally on every fal
per-second route. That would over-quote every short clip on routes fal bills linearly (Kling,
Veo, Wan), i.e. break the estimate for the common case to fix a case no current route has.

---

## 3. AC3 — actual cost per job (D3)

### 3.1 Where the number comes from (decision)

fal's queue result body carries **no billing field** (slice §Ground truth; `outputs_from`,
`clients.rs:77-124`, reads only URLs). So per-job actual = **unit price × measured billable
quantity**, measured from the **settled output**:

- **Video (per-second / per-second-tiered / per-token):** duration and resolution of the
  downloaded file. **No `ffprobe` is bundled** (§0.1), so probe with the **`ffmpeg` sidecar**:
  run `ffmpeg -hide_banner -i <file>` and parse its **stderr** for the `Duration: HH:MM:SS.ss`
  line and the video stream's `WxH`. This reuses the sidecar the objective already ships
  (`tauri.conf.json` `externalBin`, `ffmpeg::locate`) — no new binary, no license change.
- **Image (per-image / per-megapixel):** the **count** of image outputs (length of the settled
  `results` of `OutputKind::Image`) and, for per-megapixel, the decoded `WxH`. fal's image result
  body carries `width`/`height` per image, but `outputs_from` discards them today; the cheapest
  correct source is the **downloaded file's header** via the same `ffmpeg -i` probe (it reports
  image dimensions too), avoiding a second parse path.

### 3.2 The probe: a pure parser in core

Add to `crates/hickeyfield-core/src/ffmpeg.rs` (beside `parse_encoders`/`parse_licence`, which are
already pure stderr parsers):

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MediaDims { pub seconds: Option<f64>, pub width: Option<u32>, pub height: Option<u32> }

/// Parse `ffmpeg -i <file>` stderr. Pure and unit-testable on a captured string.
pub fn parse_probe(stderr: &str) -> MediaDims;

/// Run the sidecar to probe a file. Reuses the private `run()` (extend it to
/// tolerate ffmpeg's exit code 1 on `-i` with no output file, which is normal).
pub fn probe_file(bin: &Path, file: &Path) -> Result<MediaDims, FfmpegError>;
```

`parse_probe` reads the `Duration:` line (reuse `parse_timecode`, `ffmpeg.rs:570`) and the first
`Video:` stream's `NNNxNNN`. It returns `None` fields it cannot read — never a zero — so an
unmeasurable output yields `actual_usd = None`, never `0` (the AC3 requirement).

### 3.3 Computing and writing the actual (decision on the write site)

The actual is computed **after download**, in the shell, because that is the only moment the bytes
are local (`runner::download_outputs`, `runner.rs:176-210`). Add there, after the download loop
sets `local_path`:

1. Build a `Billable` from the **probed** dims (video) or the **output count + dims** (image),
   *not* from the requested settings — the settled quantity is what fal billed.
2. Price it with the **same route rate** the estimate used: `state.prices` is shell-side, so
   `download_outputs` gains a `prices: Arc<Prices>` parameter and calls
   `prices.estimate(model, route, &settled_billable).map(|e| e.usd)` (`pricing.rs:95`). Reuse
   `registry()` to resolve the model/route from `job.model_id`/`job.route_id`.
3. Set `job.actual_usd` and `job.actual_source` directly and `store.upsert`.

**Why the write is here and not only through `PollResult.actual_usd`:** no fal/Higgsfield poll
ever carries a cost, so the poll channel would never fire for the live target. Computing at
download is the only path that produces a per-job actual for fal. `PollResult.actual_usd` is kept
as the channel for any *provider-reported* cost (future direct adapters), and its plumbing is
fixed (§3.5) so it is not silently dead.

### 3.4 Provenance columns — migration 7

Migration 7 (`store.rs`, after the `version < 6` block at `:145-155`) adds three columns:

```sql
ALTER TABLE job_sets ADD COLUMN priced_from    TEXT;    -- 'literal' | 'feed' | 'snapshot' | NULL
ALTER TABLE job_sets ADD COLUMN priced_at      INTEGER; -- unix seconds the estimate was priced
ALTER TABLE job_sets ADD COLUMN actual_source  TEXT;    -- 'probe' | 'output-count' | 'provider' | NULL
PRAGMA user_version = 7;
```

- `JobSet` (`engine.rs:20-92`) gains `#[serde(default)] priced_from: Option<PriceProvenance>`,
  `priced_at: Option<i64>`, `actual_source: Option<ActualSource>` — `#[serde(default)]` so rows
  written before this field deserialise (the pattern S9/S8 used for every added field).
- New enums in `cost.rs` (kebab serde): `PriceProvenance::{Literal,Feed,Snapshot}`,
  `ActualSource::{Probe,OutputCount,Provider}`.
- `priced_from`/`priced_at` are **write-once** (set at insert, like `estimated_usd`): add them to
  the INSERT column list + params in `store.rs:249-293` but **not** to the `ON CONFLICT DO UPDATE
  SET`. `actual_source` **is** in the update set beside `actual_usd` (`:264`).
- `priced_from` is set by `Prices::estimate` returning which source it used. Minimal change:
  `Prices::estimate` already tries feed then literal (`pricing.rs:95-100`); return the provenance
  alongside (a small struct or a second method) so `submit_job` (`commands.rs:1013`) can record it.
- Row mapping (`store.rs:197-209`) reads the three new columns with `unwrap_or_default`.

### 3.5 The `apply_poll` cost-only early-return fix

`engine.rs:422-424` MUST also treat an `actual_usd` change as a change:

```rust
let changed = job.status != poll.status
    || job.results != poll.outputs
    || job.fail_reason != poll.fail_reason
    || (poll.actual_usd.is_some() && job.actual_usd != poll.actual_usd);
```

Worked example: a provider that reports its charge on a poll whose status/outputs already match
the stored job would today hit `if !changed { return false; }` (`:425-427`) and drop the cost.
After the fix the row is persisted. (For fal this path is inert — fal reports no cost — but
leaving the bug means the `PollResult.actual_usd` channel is a trap for the first adapter that
uses it.) Covered by a new `engine.rs` test.

### 3.6 Worked example (AC3's named case)

A `veo3_1_extend` job (per-second-tiered-free, `per_second_audio(0.20,2.0)`) completes; download
saves a 7.0s 720p file. `parse_probe` → `MediaDims{seconds:Some(7.0),width:Some(1280),
height:Some(720)}`. `settled_billable = Billable{seconds:Some(7.0),height:Some(720),
audio:false,batch:1,..}`. `prices.estimate` → `$1.40`. `job.actual_usd = Some(1.40)`,
`actual_source = Probe`. A job whose file cannot be probed → `MediaDims` all-`None` →
`Billable.seconds = None` → `estimate` returns `None` (§2.1) → `actual_usd = None`, never `0`.

---

## 4. AC4 — estimate re-check at completion (D4)

When the **settled** quantity differs from the **requested** one (S9 pin coercion, a floor, or
provider rounding), the job carries an advisory. `download_outputs` already has both figures once
§3 runs: the requested `Billable` (from `job.settings`, the same `Billable::from` conversion the
timeout budget uses, `runner.rs:231-233`) and the probed one.

Rule (MUST): if the probed duration/resolution differs from the requested by more than a rounding
epsilon, push one sentence onto `job.advisories` (the existing channel, `engine.rs:52-59`), e.g.
`"billed for 7.0s at 720p — your 5s/1080p request was adjusted by the provider"`. This reuses the
advisory surface the UI already renders (`MetaCard.tsx:166-174`) and the voice S9 established.
No advisory when the two agree (the common case). This also closes the backlog
`pin-aware-cost-estimate` item (`backlog.md:221`): the settled-body price is now the authority the
pre-`reconcile` estimate is reconciled against.

---

## 5. AC5 — UI (D5)

The heavy lifting is **already done** (§0.1): `MetaCard` shows Estimated and Actual;
`MetaRail`/`lib/cost.ts` implement the actual-preferring, estimate-falling-back, labelled spend
meter. Remaining, additive work:

### 5.1 MetaCard: the delta

Add a **Delta** row to the `<dl className="meta-card-fields">` block (`MetaCard.tsx:136-164`),
rendered only when both `estimatedUsd` and `actualUsd` are non-null: show
`formatUsd(actual − estimated)` signed, e.g. `+$0.20` / `−$0.05`, with `data-tone` so an overrun
reads differently from a saving. A pure helper `costDelta(est, actual)` in `lib/cost.ts` returns
`{ usd, sign } | null` (null when either side is unusable, reusing the existing `usable()` guard,
`lib/cost.ts:126`). The three card states AC5 names — estimate-only, actual-known,
delta — fall out of: est present + actual null → no delta row; both present → delta row.

### 5.2 price_status wiring (feed age / source)

- **Bridge:** add `priceStatus(): Promise<PriceStatus>` to `ui/src/api.ts` beside `estimateCost`
  (`api.ts:502`), invoking `"price_status"`; DTO `{ routes: number; ageSeconds: number | null;
  live: boolean }` mapping the shell's `camelCase` `PriceStatus` (`pricing.rs:122-133`). Provide a
  `mock.ts` value for the non-desktop path (the pattern `estimateCost` uses at `api.ts:519`).
- **Surface:** render it in the `spend-meter` block (`MetaRail.tsx:35-44`) as a small note under
  the qualifier: `"prices {ageSeconds→"2 days"} old · snapshot"` when `!live`, `"live"` otherwise;
  `None` age → "prices: bundled". Fetched once on mount (or lifted to `App.tsx` beside the
  existing `estimateCost` effect at `App.tsx:271`). This is the "surfaced (feed age / source)"
  requirement; it must **never** imply "$0.00 / current" when the feed is the bundled snapshot.

### 5.3 vitest tests + test-locked class names

- **New** `ui/src/components/MetaCard.test.tsx`: mounts `MetaCard` and asserts the three states —
  (a) estimate only (`actualUsd: null`) shows "Not reported" and **no** delta row;
  (b) actual known shows `formatActual`; (c) both present shows a delta row with the signed value.
- **Extend** `ui/src/lib/__tests__/cost.test.ts` with `costDelta` cases (overrun, saving, either
  side null/unknown → null) and a `priceStatus`-mapping case if the mapper lives in a lib.
- **Test-locked class names that MUST be preserved** (existing tests/`App.tsx` anchors depend on
  them): `meta-card`, `meta-card-fields`, `meta-card-advisories`, `spend-meter`,
  `spend-meter-value`, `spend-meter-note`, and the `data-meta-anchor` attribute
  (`App.tsx:546`). New elements add classes; none of these are renamed.

---

## 6. AC6 — live reconciliation seam (D6)

A **`#[ignore]`d** integration test (Rust) — never an example that could be run by accident, and
never `first_light` — that, given the user's real key, calls fal's pricing API for every fal route
and diffs against the literal, so a human can run it to verify the ~40 `Prose`/`Unpublished` routes
the hermetic audit cannot.

- Location: `crates/hickeyfield-core/tests/live_fal_pricing.rs`, gated `#[ignore]` and behind an
  env read: it MUST read the key from `hickeyfield_FAL_KEY` **or** `FAL_KEY` env only (the same
  vars the objective names, `objective.md:53`), and **skip** (not fail) when absent — so the
  keyless `cargo test --workspace` gate never runs it.
- Endpoint: `GET https://api.fal.ai/v1/models/pricing?endpoint_id=<ep>` → `{unit_price, unit}`
  (slice §Ground truth). For each fal route's resolved endpoint, fetch, compare to the literal,
  print a diff table. **The key MUST NOT be logged** — assert only on the diff, print only
  endpoint/figure, never the `Authorization` header (mirrors `clients.rs`, which never logs auth).
- Run command, documented for the human: `hickeyfield_FAL_KEY=… cargo test -p hickeyfield-core
  --test live_fal_pricing -- --ignored --nocapture`.
- Docs: a new section in `docs/FIRST-LIGHT.md` ("Verifying prices against the live fal API"),
  because that is where the objective's cost-accuracy-unverified item lives (`objective.md:146`).

---

## 7. Non-goals (recorded, not skipped)

- **N1.** No usage-API reconciliation UI. `GET /v1/models/usage` is an ADMIN-key aggregate with
  no `request_id` (slice §Ground truth), so it cannot attribute cost per job. Aggregate
  reconciliation is a later slice. **N/A here.**
- **N2.** No Higgsfield/vaig actuals — their results bill in opaque credits (`registry.rs:558-560`)
  and `CostModel::Unknown`; §3 computes an actual only where a rate exists. A Higgsfield job keeps
  `actual_usd = None`.
- **N3.** No new `CostModel` variants (`PerImageTiered`, per-tier grok input surcharge). The
  grok edit/extend 720p under-quote (§1.4) stays a `Prose`/backlog item unless the audit run
  reclassifies it. `PerToken`/`PerImageTiered` returning `None` on unknown dims stays backlog
  (S8/S9 N3).
- **N4.** No change to `outputs_from`'s URL shapes, `media::bind`, `route::resolve`, the enhancer,
  or the S9 pinned-field logic (AC4 only *reads* its output).
- **N5.** No live paid generation, no `first_light`.

---

## 8. Test plan — every AC to a named test

Rust tests live in the `#[cfg(test)] mod tests` block beside the code.

| AC | test | file | asserts |
|---|---|---|---|
| AC1 | `the_audit_is_hermetic_and_finds_no_literal_beyond_tolerance` | `prices.rs` | `audit()` returns **0** `Mismatch`; runs with no network |
| AC1 | `the_audit_actually_compares_something` (non-vacuity) | `prices.rs` | `audit()` returns ≥1 `Ok`/`Prose`/`Unpublished` row and ≥1 fal route resolved — so "0 mismatches" cannot mean "0 checked" |
| AC1 | `a_wrong_literal_is_caught` (mutation guard) | `prices.rs` | a hand-built fixture: literal contradicting a `Rate` by >1.2× yields a `Mismatch`; within 1.2× yields `Ok` |
| AC1 | `an_unmodellable_unit_is_reported_not_failed` | `prices.rs` | `inworld-tts` (`ThousandCharacters`) → `UnmodellableUnit`, not `Mismatch` |
| AC2 | `unknown_duration_is_none_not_zero_per_second` | `cost.rs` | `PerSecond.estimate(Billable{seconds:None})` == `None` |
| AC2 | `unknown_duration_is_none_not_zero_per_token` | `cost.rs` | `PerToken` with no seconds/dims == `None` |
| AC2 | existing `unknown_price_is_none_never_zero` (`cost.rs:340`) | `cost.rs` | unchanged |
| AC2 | `a_floored_route_bills_the_minimum` | `registry.rs` | a test route carrying `min_billed_seconds:Some(15.0)` bills the floor on a 5s clip; `None` routes do not |
| AC2 | existing `provider_minimum_overrides_a_short_clip` (`cost.rs:354`) | `cost.rs` | unchanged |
| AC3 | `parse_probe_reads_duration_and_resolution` | `ffmpeg.rs` | a captured `ffmpeg -i` stderr string → `MediaDims{7.0,1280,720}`; a truncated one → `None` fields |
| AC3 | `actual_is_priced_from_the_settled_quantity` | `runner.rs` (or a shell test) | a completed 10s per-second job with probed dims gets `actual_usd == 10 × rate`, `actual_source == Probe` |
| AC3 | `an_unmeasurable_output_gets_none_not_zero` | `runner.rs` | probe returns all-`None` ⇒ `actual_usd == None` |
| AC3 | `apply_poll_persists_a_cost_only_delta` | `engine.rs` | same status/outputs but new `actual_usd` ⇒ `apply_poll` returns `true`, writes the cost |
| AC3 | `migration_7_adds_provenance_columns_and_roundtrips` | `store.rs` | migrate 6→7 idempotently; a `JobSet` with `priced_from/priced_at/actual_source` round-trips; `priced_from` is write-once, `actual_source` updates |
| AC4 | `a_settled_quantity_that_differs_carries_an_advisory` | `runner.rs` | probed 7s vs requested 5s ⇒ one advisory naming both; equal ⇒ none |
| AC5 | MetaCard three states | `ui/src/components/MetaCard.test.tsx` | estimate-only (no delta), actual-known, delta row present with signed value |
| AC5 | `costDelta` + `priceStatus` mapping | `ui/src/lib/__tests__/cost.test.ts` | overrun/saving/null cases; DTO mapping |
| AC6 | (human-run, not a gate) | `tests/live_fal_pricing.rs` | `#[ignore]`; skips without a key; never logs the key |
| AC7 | the gate (§9) | — | 0 failed, ≥ 801 passed |

**Mutation checks the TEST agent should run:** revert `cost.rs:182` to `unwrap_or(0.0)` ⇒
`unknown_duration_is_none_not_zero_per_second` fails; drop the `actual_usd` term from
`apply_poll`'s `changed` ⇒ `apply_poll_persists_a_cost_only_delta` fails; break `parse_probe`'s
duration read ⇒ AC3 probe + pricing tests fail.

---

## 9. Gate commands (AC7), exact

```
# dummy keys so shell tests that read a key env do not skip/panic:
export hickeyfield_FAL_KEY=x hickeyfield_HIGGSFIELD_KEY=x hickeyfield_HIGGSFIELD_SECRET=x
cargo test --workspace                                   # 0 failed, >= 801 passed + new tests
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo deny check                                         # no dependency change expected
./scripts/lint-provenance.py
env -u hickeyfield_FAL_KEY -u FAL_KEY cargo run -q -p hickeyfield-core --example price_check
env -u hickeyfield_FAL_KEY -u FAL_KEY cargo run -q -p hickeyfield-core --example audit_fal   # PINNED 0, UNREACHABLE 0 unchanged
# UI (Node 24):
export PATH="/private/tmp/claude-501/-Users-jorge-Documents-GitHub-hickeyfield/79be25e2-8f71-4192-9e6d-cd0e9d068f1c/scratchpad/node-v24.8.0-darwin-arm64/bin:$PATH"
cd ui && pnpm test && pnpm build
scripts/build-macos.sh    # or `tauri build`; the .app must launch (window, not blank)
```

`price_check` MUST print `MISMATCH 0`. The keyless `audit_fal` output (PINNED 0 / UNREACHABLE 0)
MUST be unchanged from S9 — this slice touches no schema/mode logic. **Never** run `first_light`
(spends money, no key). **Never** bare `cargo build` for a runnable app (README: blank window).

---

## 10. Order of work

1. AC1 audit (`prices::audit` + rewrite `price_check.rs`) — run it first; it finds any wrong
   literal for free before anything else changes (§0 evidence). 2. Fix any `Mismatch` in
   `priced_routes` with the fal figure cited. 3. AC2 estimator honesty (`cost.rs` two returns +
   the floor wiring). 4. AC3: `parse_probe`/`probe_file` in core, then migration 7 + `JobSet`
   fields + provenance enums, then the `download_outputs` compute-and-write + `apply_poll` fix.
   5. AC4 advisory (same site). 6. AC5 UI (delta, `priceStatus` wiring, vitest). 7. AC6 live
   seam + docs. 8. Docs (§13). Each step leaves `cargo test --workspace` green.

---

## 11. Open questions (each with the default this build assumes)

- **Q1 — Should AC1's audit try to parse fal's *conditional* prose (tiers, audio) to verify the
  video literals?** **Default: no.** `parse_pricing` refuses conditional prose on purpose
  (`fal_catalogue.rs:1180-1184`); re-implementing a looser parser here re-opens the "take the
  wrong number from a rate table" bug it was written to close. The `Prose` rows go to AC6's live
  seam instead. Reversible: a later slice can add structured tier parsing.
- **Q2 — AC2 floor: data-driven `None`, or hardcode 15s on a named list?** **Default: data-driven,
  applied to zero current routes** (§2.2), because fal states a floor on no registry route today.
  If the owner knows a route fal floors that the snapshot prose omits, add `min_billed_seconds` to
  that one route — a one-line change, not a code-path change.
- **Q3 — grok edit/extend: model the 720p tier + $0.01/s input surcharge, or keep the 480p
  approximation?** **Default: keep the approximation**, note it, defer the tier to a `PerSecondTiered`
  slice — under-quoting one edit route at 720p is a backlog item, not this slice's, and adding an
  input-surcharge term touches `Billable`/`CostModel` (N3).
- **Q4 — Compute the actual from the probe, or trust fal's image result-body dims?** **Default:
  probe the downloaded file** (one path for video and image), because the file is the settled
  truth and `outputs_from` already discards the body dims. If probing an image proves flaky, fall
  back to output count × rate (per-image) — recorded as `ActualSource::OutputCount`.
- **Q5 — `priced_from` for a feed-priced route: does `Prices::estimate` expose which source it
  used?** **Default: yes, via a small return-provenance variant** (§3.4). If threading it through
  is disproportionate, default `priced_from` to `Feed` when `feed.cost_model` was `Some` and
  `Literal` otherwise, computed at the `submit_job` call site.
- **Q6 — On timeout, reconcile est-vs-actual?** **Default: no** (objective Q6): preserve the
  resumable record; record the actual only when the result later completes and downloads.

---

## 12. The riskiest parts (where the verifier should start)

1. **AC1 has near-zero teeth against today's registry (§0.2).** The honest finding is that fal's
   strict prose parser leaves almost nothing to hard-compare, so the audit is mostly a regression
   guard + report. The danger is an implementer (or reviewer) reading "0 mismatches" as "prices
   verified" when it means "nothing was machine-checkable". The `the_audit_actually_compares_
   something` non-vacuity test and the `a_wrong_literal_is_caught` mutation test are the whole
   mitigation. **This is the weakest link; the verifier should confirm the audit is non-vacuous by
   running `price_check` and checking the `Prose`/`Unpublished`/`Ok` counts are non-trivial.**
2. **The mismatch list in §1.4 was read, not run (§0).** I have no shell; the first IMPLEMENT task
   MUST run `price_check` and reconcile. If a `Mismatch` appears I did not predict, it is fixed in
   this slice with the fal figure — do not wave it through.
3. **The ffmpeg-`-i` probe is stderr-shaped and version-sensitive.** `parse_probe` must tolerate
   the real sidecar's output (H:M:S.ss duration, `NNNxNNN [SAR …]` on the video stream) and
   `ffmpeg -i` exiting non-zero (it has no output file). The pure test uses a captured string; an
   `--ignored` ffmpeg test (like the objective's trim test) should confirm against the real
   sidecar. A wrong probe silently mis-prices every actual.
4. **The actual is priced from settled dims with the *same* rate as the estimate.** If the route
   resolution changed between estimate and completion, or the probe reads the wrong stream, the
   actual is wrong in a way that looks authoritative next to the estimate. §3.6 pins the happy and
   the unmeasurable path; the delta advisory (§4) is the honesty backstop.
5. **Migration 7 + write-once semantics.** `priced_from`/`priced_at` must be INSERT-only (mirroring
   `estimated_usd`, which is *absent* from the `ON CONFLICT` set at `store.rs:256-268`);
   `actual_source` must be in the update set beside `actual_usd`. Getting the two lists wrong makes
   provenance silently freeze or silently churn. The `migration_7` round-trip test guards it.

---

## 13. Documentation (the DOCUMENT step)

- **`CHANGELOG.md`** — under `## [Unreleased]`: a `### Fixed` bullet (`:46`) that unknown prices no
  longer read as $0.00 and the fal 15s-floor mechanism is wired; a `### Added` bullet (`:19`) that
  each finished job now records an actual cost from its settled output, shown as estimated-vs-actual
  with a running spend, plus a keyless price audit.
- **`docs/FIRST-LIGHT.md`** — new section for the AC6 live-pricing verification command; the
  "cost accuracy unverified" item is updated to point at it (still human-run).
- **`crates/hickeyfield-core/src/cost.rs:143-145`** — the `estimate` doc-comment says it returns
  `None` "only for `Unknown`"; amend to include the missing-quantity cases (§2.1).
- **`crates/hickeyfield-core/src/prices.rs`** — module doc gains the audit's purpose and its known
  limitation (conditional prose is `Prose`, verified live not hermetically).
- **`.forge/backlog.md:221`** — mark `pin-aware-cost-estimate` done (settled-body price is now the
  authority, §4); mark the S10 entry done.
- **README** — checked at implement time for a fal-examples list; `price_check` already exists, so
  if the list mentions it nothing changes, otherwise **N/A**.
- **`document.md`** — written after the docs land, recording file + section for each change.
