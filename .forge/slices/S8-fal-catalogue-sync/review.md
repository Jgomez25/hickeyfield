# S8-fal-catalogue-sync — REVIEW (evidence)

Reviewer: independent (did not implement). Read-only: no project file was changed by
this pass; the only artefacts written are this file and `security.md`.

Scope: the working-tree diff on `forge/s8-fal-catalogue-sync` —
`examples/fal_diff.rs` (new), `examples/audit_fal.rs`, `Cargo.toml`,
`vendor/fal-catalogue-snapshot.json`, `src/fal_catalogue.rs`, `src/registry.rs`,
`src/media.rs`, `src/use_case.rs`, `docs/FIRST-LIGHT.md`, `CHANGELOG.md`,
`.forge/backlog.md`, `.forge/state.json`.

---

## 1. Gate, re-run independently (not taken on trust)

| Gate | Result |
|---|---|
| `cargo test --workspace` | **779 passed, 0 failed** (657 core + 2 `examples/fal_diff` + 120 tauri-lib); matches the claim |
| `cargo fmt --all --check` | clean |
| `cargo clippy --workspace --all-targets -- -D warnings` | clean |
| `./scripts/lint-provenance.py` | CDN hostnames PASS, copy provenance PASS |
| `cd ui && pnpm test` | 14 files / 232 tests passed |
| `cd ui && pnpm build` | built |
| `cargo run --example audit_fal` (live, keyless) | reproduced exactly: 11 / 2 / 21 / 9 findings, **UNREACHABLE 0**, **CHECKED 66**; none of the 14 imported ids appears in any finding section |
| `cargo run --example fal_diff -- --offline` | `1491 model(s) … Registry reaches 66 fal endpoint(s)`, `1077 of 1138` outside the registry; `--bogus` exits 2 with a usable message |

The `[[example]] test = true` wiring is real: `Running unittests examples/fal_diff.rs …
2 passed` appears in the workspace run, so the diff logic is genuinely under test rather
than merely compiled.

---

## 2. BLOCKERS

### B1 — LTX 2.5 Pro and LTX 2.5 Pro Animate cannot generate at all

`crates/hickeyfield-core/src/registry.rs:2067` (`ltx_flags`, shared by `ltx_2_5` at
`:2075` and `ltx_2_5_animate` at `:2087`):

```rust
("duration", false, ValueSpec::Number),
```

fal declares LTX 2.5's duration as a **closed** enum, `{6, 8, 10, "auto"}`
(`lightricks/ltx-2.5/{text,image}-to-video/pro`, probed 2026-09-16). Two consequences
compound:

1. `ValueSpec::Number` ⇒ `capabilities().durations` is empty ⇒ `ui/src/lib/variants.ts:133`
   leaves the app-wide default `duration: 5` (`ui/src/App.tsx:67`) unclamped, and
   `ChipRow` offers the invented ladder `[3,4,5,6,8,10,12]`.
2. `fal_schema::parse` (`crates/hickeyfield-core/src/fal_schema.rs:216-226`) keeps only the
   **string** members of an enum, so this endpoint's recorded enum is `["auto"]` alone.
   `fal_schema::reconcile` then refuses anything else.

Measured, against the live schema, with the body the submit path builds from the app's
default settings:

```
ltx_2_5          lightricks/ltx-2.5/text-to-video/pro   FAIL this endpoint takes duration of auto, not 5
ltx_2_5_animate  lightricks/ltx-2.5/image-to-video/pro  FAIL this endpoint takes duration of auto, not 5
```

`src-tauri/src/app.rs:438` turns that `Refusal` into `JobError::Permanent`, so **every**
submit of these two tiles fails before the request is sent. No duration the UI can produce
passes: `6`/`8`/`10` are refused too, because `parse` dropped the integer members. Standard
violated: AC3 ("any slug that fails the gate is NOT added"), AC4 ("no model advertises a
duration fal will reject"), AC5/CHANGELOG ("a model that appears under a tab is one that
can actually run there"). Severity blocker — two of the thirteen imported models are dead
tiles on the app's critical path.

Fix (pick one, do not paper over):
- withdraw `ltx_2_5` / `ltx_2_5_animate` from this slice, per AC3's own rule; or
- teach `fal_schema::parse` to keep non-string enum members and `reconcile` to write the
  member's own JSON type, then declare the spec flag as `en(&["6","8","10","auto"])` — a
  mechanism change, i.e. its own slice.

### B2 — Veo 3.1 Extend sends two values the endpoint fixes by `const`

`crates/hickeyfield-core/src/registry.rs:2107-2108`:

```rust
("duration",   false, ValueSpec::Number),
("resolution", false, ValueSpec::Text),
```

fal's `fal-ai/veo3.1/extend-video` schema declares
`duration: {"type":"string","default":"7s","const":"7s"}` and
`resolution: {"type":"string","default":"720p","const":"720p"}`. Both keys are *accepted*
(so `app.rs`'s unknown-key sweep keeps them) and neither is an `enum` (so `reconcile`
passes them through untouched). The body actually built from default settings is:

```
veo3_1_extend  fal-ai/veo3.1/extend-video  OK  {"duration":5.0,"prompt":"…","resolution":"1080p"}
```

i.e. a JSON number where fal wants the literal string `"7s"`, and `"1080p"` where fal
wants the literal `"720p"` — a 422 on every submit, with no local guard left to catch it.
Two further consequences: the constraint string at `:2096` ("`duration` is the length to
add, not the total") documents a control that does not exist, and the cost model
`per_second_audio(0.20, 2.0)` at `:945` scales the Generate-button price by a duration the
provider ignores (always 7s), so the quoted figure is wrong whenever the chip is not 7 —
and 7 is not even on the ladder.

Fix: drop the `duration` and `resolution` flags from the `veo3_1_extend` spec so neither is
offered nor sent (fal then applies its own consts), and state the fixed 7s/720p in
`constraints`; price it as `Flat { usd: 7.0 * 0.20 }`-equivalent or leave the per-second
model with a constraint that says the length is fixed.

### B3 (root cause of B1/B2) — the verification gate is blind to `const` and to numeric enums

`crates/hickeyfield-core/examples/audit_fal.rs:133-150` (check 4) only compares option sets
"where both sides enumerate": it requires our flag to be `ValueSpec::Enum` *and*
`schema.enums` to hold the field. `fal_schema::parse:216` never records `const`, and drops
non-string enum members. So an endpoint that pins a value (`veo3.1/extend-video`) or
enumerates integers (`ltx-2.5`) is reported as CHECKED-and-clean while being unusable.

The slice treated a clean audit as proof ("Every landed slug: CHECKED, zero findings …
Gate passed, no exceptions taken", implement.md §4). That inference is not sound for this
tool as written. Standard violated: evidence must support the claim it is offered for.

Fix: add a fifth audit check — "we offer a field the endpoint pins (`const`) or whose enum
we cannot represent" — and record `const` in `EndpointSchema`. Until then, no import may
cite audit_fal alone as the duration/resolution clearance.

---

## 3. Major / minor findings

**M1 (minor) — a constraint string that contradicts the schema.**
`registry.rs:2037`: "fal declares `duration` as a plain integer with **no published
ceiling**, so the app offers it free-form and lets the endpoint judge." fal's
`alibaba/wan-3.0-prime/*` publishes `{"anyOf":[{"type":"integer","minimum":2,"maximum":30},
{"type":"null"}]}` — a published floor and ceiling. The offered ladder (3–12s) is all
valid, so this is not a blocker, but the sentence is shipped to users as a constraint and
is false, and the free numeric box can still send 60. Fix: state 2–30s and clamp.

**M2 (minor) — imported specs carry no defaults, and one of them costs money.**
`registry.rs:2221` declares `en(&["0.5K","1K","2K","4K"])` for `nano_banana_2_edit` with no
default, so `resolveSettings` (`ui/src/lib/variants.ts:143`) falls to
`caps.resolutions[0]` = **0.5K**, while fal's published default is `1K` and the cost model
at `:1381-1388` quotes the **1K** rate ($0.08). Measured body:
`{"prompt":…,"resolution":"0.5K"}`. The user silently gets the 0.75x tier and an
over-quote. The `spec()` helper has no default parameter at all — a pre-existing
limitation, but this import is the first place where the first enum member is also the
wrong price tier. Fix: order the enum with fal's default first, or extend `spec()` to carry
fal's `default`.

**M3 (minor) — `--dump --offline` re-stamps stale data with today's date.**
`examples/fal_diff.rs:65-90`: `dump_snapshot` always writes `today_utc()` as
`snapshot_date`, regardless of `catalogue.captured()`, and the usage line at `:47`
advertises "`--dump` and/or `--offline`" as a valid combination. Reproduced: `--dump
--offline` exits 0 and emits the bundled rows under today's date. Today the dates coincide
so nothing is visibly wrong; next month it would mark a month-old snapshot fresh, defeating
the drift signal the whole module exists for. Fix: refuse `--dump` when the catalogue is
`Captured::Snapshot`, or carry the snapshot's own date through.

**M4 (minor) — a comment claiming protection the design does not provide.**
`examples/fal_diff.rs:27-29`: "`--dump` writes to stdout only: a tool that overwrites the
vendored snapshot in place is a tool that can destroy the offline fallback on a partial
fetch." The documented invocation is `--dump > crates/…/vendor/fal-catalogue-snapshot.json`
(`fal_catalogue.rs:45`), and the shell truncates that file *before* the process starts, so a
failed fetch leaves it empty — exactly the failure the comment says is avoided. It is loud
rather than silent (`include_str!` then fails the build) and git restores it, hence minor,
but the comment overstates. Fix: write to a temp file and rename, or reword.

**M5 (minor) — stringly-typed modality.**
`examples/fal_diff.rs:129`: `format!("{}", m.modality) == "video"`. `Modality` is public,
`PartialEq`, and the registry's own new test uses `m.modality == Modality::Video`
(`registry.rs:2951`). Copied deliberately from `audit_fal.rs:46` so the two agree; the fix
is to compare the enum in both places, not to keep the string in one.

**M6 (minor, debt) — `covered_endpoints()` is duplicated.**
`examples/fal_diff.rs:126-139` reimplements `audit_fal.rs`'s registry→endpoint expansion,
with a comment saying the two "must not be able to disagree". Duplication is how they will.
Fix: one `pub fn covered_fal_endpoints()` in `registry` or `media`, called by both.

**M7 (minor) — untested hand-rolled calendar.**
`examples/fal_diff.rs:102-120` (`today_utc`) stamps the snapshot's provenance and has no
test. I verified it independently against 40,000 consecutive days (1970-01-01 … 2079):
zero mismatches. Clean today; a two-line unit test would keep it that way.

---

## 4. Checked and found clean (silence is not clearance)

- **Prices, all 14, against the refreshed snapshot's own prose** — `kling3_0_pro`
  0.112/0.168 (×1.5) ✓; `wan3_0_prime` tiers 480/720/1080 = 0.068/0.14/0.28 ✓; `ltx_2_5*`
  0.12/0.17 ✓; `veo3_1_extend` 0.20/0.40 (×2.0) ✓; `qwen_image_3` $0.04 ✓;
  `nano_banana_pro_edit` $0.15 ✓; `nano_banana_2_edit` $0.08 ✓; `seedream_v5_pro_edit`
  `PerImage{0.0675, +0.0045/extra input}` ✓; the four GPT Image 2.5 ids left `Unknown` with
  fal's token table as the reason ✓ and a test that asserts they stay `None`
  (`registry.rs:2896-2912`).
- **`seedance_2_5 = per_token(21.40)` on a video model is coherent, not a category error.**
  `CostModel::PerToken` (`cost.rs:199-221`) derives tokens from `h × w × fps × seconds`,
  all of which `Billable::video` supplies; `Billable` carries no token count and needs
  none. fal publishes "`$0.0214` per 1000 tokens … tokens ≈ (h × w × (in+out duration) ×
  24) / 1024", which is the same formula at the same scale: 1280×720×24×5s ⇒ 108,000
  tokens ⇒ $2.31 for a 5s clip = $0.462/s against fal's own quoted $0.4730/s at 720p. The
  route note's "1080p is a ~9% under-estimate" checks out ((0.0234−0.0214)/0.0234 = 8.5%).
- **Every enumerated option set is fal's, verbatim.** I re-probed all six video/image
  families: `seedance-2.5` duration `auto,4…30`, resolution, aspect ✓; `kling/v3/pro`
  duration `'3'…'15'`, aspect `16:9,9:16,1:1` ✓; `wan-3.0-prime` resolution/aspect ✓;
  `ltx-2.5` resolution `720p,1080p` ✓; `veo3.1/extend-video` aspect ✓; `qwen-image-3` and
  `gpt-image-2.5` `image_size` present and neither `aspect_ratio` nor `resolution` invented
  ✓; nano-banana edits' resolution ladders ✓. No invented field name was found anywhere in
  the import.
- **`nano_banana_2_edit`'s optional `image_references` is not a copy-paste divergence** —
  fal's `fal-ai/nano-banana-2/edit` really does require only `["prompt"]`, whereas
  `nano-banana-pro/edit`, `gpt-image-2.5/*/edit` and `seedream/v5/pro/edit` require
  `["prompt","image_urls"]`, and the four specs mirror that split exactly.
- **`FAL_ROUTE_MODES`** (`media.rs:599-751`): lookup is exact-slug (`media.rs:865`), so the
  new `…/edit` and `…/text-to-image` rows cannot collide with their family roots; the
  `Exact = true` rule is applied correctly to the vendor-namespaced already-suffixed
  endpoints, and is *load-bearing* for `lightricks/ltx-2.5/text-to-video/pro` (which ends
  in `/pro`, so the `MODE_TAILS` fallback would have double-suffixed it); the two
  `Exact = false` roots (`kling-video/v3/pro`, `wan-3.0-prime`) restrict to `[Text]` with
  the `start_image_url` reason recorded, and `use_case.rs:283-318` asserts the withheld
  modes stay hidden. Array length `38 → 51` matches the 13 added rows and compiles.
- **Registry wiring**: each of the 13 new ids has a fal `Route` *and* a `CostModel` in the
  same `priced_routes` arm; `JOB_TYPES` placement is defensible and enhancer-aware
  (`veo3_1_extend` under `animate` = enhancer off for an instruction-following extend;
  `gpt_image_2_5_*` under `image-gpt-image-2`; `qwen_image_3` under the generic
  `image-styled` bucket, with `alibaba/qwen-image-3/edit` explicitly *not* imported for the
  reason recorded in implement.md §8.4). Size literal `+17 → +30` is consistent with 13
  additions plus the `seedance_2_5` promotion.
- **Tests updated, not deleted, and not tautological.** `registry_covers_the_catalogue_…`,
  `the_picker_only_models_are_all_present` (renamed, 13 ids added),
  `models_the_corpus_could_not_price_stay_unknown` (lost `seedance_2_5`, gained four GPT
  ids — net stricter), `unverified_slugs_are_flagged…` (keeps `outpaint` and adds a
  negative assertion for `seedance_2_5`). `every_imported_model_resolves_to_an_exact_fal_endpoint`
  (`registry.rs:2938`) has real teeth: it asserts the *resolved* endpoint is present in
  fal's own index, which is the check a slug-level diff cannot do. `there_are_twelve_launch_families`
  and `the_unroutable_models_are_a_known_short_list` are untouched and green.
- **`fal_diff` diff logic**: comparison is exact-id (`fal_minus_registry:152-160`), roots
  are expanded through the production resolver first (`:126-139`) so family roots are not
  false-missing — the fixture test covers both halves, including the retired-row drop and
  the out-of-scope category. `use_case_of` refuses to coerce `Training`/`Llm`/`Other`, and
  the second test pins "exactly five categories map". Unknown args exit 2. `--dump` honours
  the existing guards by construction (it calls `fal_catalogue::fetch`, which keeps
  `MAX_PAGES = 60`, `FETCH_TIMEOUT` and `MAX_UNREADABLE_SHARE` — none of them touched by
  this slice) and additionally refuses to write when `retired() > 0`.
- **Snapshot refresh is lossless and self-consistent.** `--dump --offline` round-trips the
  vendored file byte-for-byte at the JSON level (`items` compare equal, same key set), and
  the frozen constants add up: 761 unpublished + 193 rates + 537 prose = 1491 = the row
  count; per-category counts and `TextTo3d 11→12` all assert in `fal_catalogue.rs`.
- **`audit_fal`'s `unreachable` fix is correct** — the bucket collected at `:64` is now
  printed at `:174-178`, and the `CHECKED` section prints `m.id -> endpoint` rows so per-slug
  evidence exists. Confirmed live: `UNREACHABLE (0)`, `CHECKED (66)`.
- **Doc corrections are factually right.** `git log -S "fn resolve_endpoint"` returns
  exactly `ab06e31` (2026-08-05, as `crates/halation-core/src/media.rs`), and `8d8ecfb`
  (2026-08-28) is "Reach fal-mirrored models on the user's own Higgsfield key" — so
  `docs/FIRST-LIGHT.md:32-44` and `.forge/backlog.md:68` cite what git confirms, and the
  design's suggested commit ids were correctly rejected. `Cargo.lock` is unchanged; the only
  manifest edit is the `[[example]]` target.
- **No dead code** in the new example (every helper is reachable from `main` or a test), and
  no invented API: every `hickeyfield_core` item `fal_diff` imports exists and is public.

---

## 5. Blocking findings, restated

- **B1** `registry.rs:2067` (+`:2075`, `:2087`) — `ltx_2_5` / `ltx_2_5_animate` are refused
  locally on every submit; two imported models cannot generate.
- **B2** `registry.rs:2107-2108` — `veo3_1_extend` sends `duration: 5.0` / `resolution:
  "1080p"` to an endpoint that pins `"7s"` / `"720p"` by `const`; 422 on every submit, plus
  a cost estimate that scales with a duration the provider ignores.
- **B3** `examples/audit_fal.rs:133` + `src/fal_schema.rs:216` — the gate that cleared all
  14 slugs cannot see `const` or integer enums, so its silence does not support the claim
  made for it.

Everything else in this slice is well above the bar — the pricing, the enum transcription,
the resolver wiring, the tests and the documentation corrections were all independently
verified and stand. B1 and B2 are narrow: withdrawing three model ids (or fixing the two
specs) resolves them without touching anything else that landed.

VERDICT: FAIL

---

## RE-REVIEW (reduced 11-model set, 2026-09-16, HEAD 58e981c)

Reviewer: independent, read-only. Nothing in the project was modified by this pass; the only
file written is this one. Scope: `git diff main...HEAD` — `examples/fal_diff.rs` (new),
`examples/audit_fal.rs`, `Cargo.toml`, `vendor/fal-catalogue-snapshot.json`,
`src/fal_catalogue.rs`, `src/registry.rs`, `src/media.rs`, `src/use_case.rs`,
`docs/FIRST-LIGHT.md`, `CHANGELOG.md`. The withdrawn `ltx_2_5` / `ltx_2_5_animate` /
`veo3_1_extend` are not re-reviewed; they are verified gone (§3 below). The parser blind spot
(prior B3) is out of scope — it is its own backlog slice.

### 1. Gate, re-run here (real output)

```
$ cargo test --workspace            TEST exit=0
test result: ok. 657 passed; 0 failed; 9 ignored   (hickeyfield-core lib)
test result: ok.   2 passed; 0 failed              (examples/fal_diff)
test result: ok. 120 passed; 0 failed; 1 ignored   (hickeyfield-tauri-lib)
test result: ok.   0 passed …  (bin + 3 empty targets)          -> 779 passing, 0 failed
$ cargo fmt --all --check           FMT exit=0   (no output)
$ cargo clippy --workspace --all-targets -- -D warnings
    Finished `dev` profile … in 0.96s             CLIPPY exit=0 (no warnings)
$ ./scripts/lint-provenance.py      PROV exit=0
CDN hostnames      PASS  no third-party media hosts referenced
Copy provenance    PASS  no shipped string matches the reference corpus (80015 shingles indexed)
$ env -u hickeyfield_FAL_KEY -u FAL_KEY cargo run -q -p hickeyfield-core --example audit_fal
=== SILENTLY IGNORED MEDIA (11) ===  === MISSING REQUIRED FIELDS (2) ===
=== DROPPED SETTINGS (21) ===        === REJECTED OPTIONS (9) ===
=== UNREACHABLE — fal serves no schema for this endpoint (0) ===
=== CHECKED — fal answered with a schema (63) ===
$ cargo run -q -p hickeyfield-core --example fal_diff -- --offline | head -1
fal index: 1491 model(s), bundled snapshot of 2026-09-16. Registry reaches 63 fal endpoint(s).
$ cd ui && pnpm test     Test Files 14 passed (14)   Tests 232 passed (232)
$ cd ui && pnpm build    ✓ built in 897ms
```

`UNREACHABLE 0`, `CHECKED 63` — exactly the expected 66 − 3, matching
`audit_fal-2026-09-16-reduced.log`. All 11 ids appear under `CHECKED` and in **no** finding
section (counted mechanically over lines 1–98 of the run: every id `findings=0`). `first_light.rs`
was not run; nothing in this pass was authenticated and nothing spent money.

### 2. BLOCKER

#### B4 — Seedance 2.5 on the Animate Image tab sends a value fal pins by `const`

This is the *same defect class* as the withdrawn B2, on the slice's headline model, and it is
still landed.

`crates/hickeyfield-core/src/registry.rs:1572-1576`:

```rust
(
    "aspect_ratio",
    false,
    en(&["auto", "21:9", "16:9", "4:3", "1:1", "3:4", "9:16"]),
),
```

That enum is transcribed from `bytedance/seedance-2.5/**text**-to-video`. The comment above it
(`registry.rs:1544`) claims it is "Transcribed from fal's own schema for
`bytedance/seedance-2.5/*`" — the `*` is the error: the two endpoints do **not** agree. Fetched
keyless this pass:

```
$ curl -s 'https://fal.ai/api/openapi/queue/openapi.json?endpoint_id=bytedance/seedance-2.5/image-to-video'
Seedance25ImageToVideoInput.properties.aspect_ratio =
{ "default": "auto", "type": "string", "title": "Aspect Ratio",
  "description": "The aspect ratio of the generated video. Always \"auto\" for image-to-video",
  "const": "auto" }
```

and the model is offered on that tab: `media.rs:689-693` lists `bytedance/seedance-2.5` as
`(false, [Text, Image])`, `use_case.rs:293` asserts the `ImageToVideo` tab, and
`registry.rs:2830` asserts both endpoints resolve.

Measured against the live schema through the shipped library (no key, no money):

```
spec aspects   = ["auto", "21:9", "16:9", "4:3", "1:1", "3:4", "9:16"]
default_aspect = None
schema.accepts(aspect_ratio) = true
schema.enums[aspect_ratio]   = None          <- `const` is not an `enum`, so nothing is recorded
capability aspect axis        = Axis { support: Yes, values: [], default: None, … }
requested  16:9 -> plan Sent("16:9") wire=Some("16:9")
requested  9:16 -> plan Sent("9:16") wire=Some("9:16")
```

The chain, each link read in the code:

1. `fal_schema::parse` (`src/fal_schema.rs:214-226`) records only `enum`, never `const`, so the
   pin is invisible to everything downstream (unchanged by this slice — 0 lines in the diff).
2. `src-tauri/src/app.rs:329-332` sets `aspect_key = Some("aspect_ratio")` because the endpoint
   *declares* the field; `:336-351` then always sends an explicit value —
   `AspectPlan::decide(true, requested, …)` returns `Sent(requested)` (`aspect.rs:57`).
3. `requested` is never `"auto"` in practice: the app-wide default is `aspect: "16:9"`
   (`ui/src/App.tsx:69`), and on attaching an image `App.tsx:342` *snaps* it to the nearest
   `W:H` chip — `"auto"` is not a ratio and cannot be the snap target.
4. `resolveSettings` (`ui/src/lib/variants.ts:143-147`) cannot clamp it back: the capability axis
   for this endpoint has `values: []` (measured above), so the branch falls through to
   `settings.aspect` unchanged. The UI also renders an aspect control with support `Yes` and zero
   options.
5. The unknown-key sweep in `app.rs` keeps the key (the endpoint declares it), and
   `fal_schema::reconcile` has no enum to check it against.

So every Animate-Image submit on Seedance 2.5 puts `"aspect_ratio": "16:9"` (or `"9:16"`, …) on
the wire to an endpoint whose schema pins the literal `"auto"` — a 422 after the media upload,
exactly the failure withdrawn `veo3_1_extend` was withdrawn for. As with B2 I cannot execute the
authenticated request (no key, no money), so the 422 is an inference from `const` on a
Pydantic-generated schema — the *same* inference the orchestrator already accepted as blocking
for `veo3_1_extend`.

**Why this is the slice's finding and not pre-existing noise.** The wire behaviour does predate
the branch (the `[Text, Image]` row is on `main` at `media.rs:661`), but on `main`
`seedance_2_5` was an explicitly hedged placeholder: prompt-only spec, no price, and a route note
the UI shows containing "unverified" (`main:registry.rs:2489`). This slice removes the hedge
(implement.md §6: `unverified_slugs_are_flagged_in_the_note_the_ui_shows` now asserts it does
*not* claim to be unverified), prices it (`registry.rs:763-771`), asserts the Animate Image tab
(`use_case.rs:293`), and ships the claim "Each was checked against the provider's published
schema before being offered, **so a model that appears under a tab is one that can actually run
there**" (`CHANGELOG.md`, Added bullet). That sentence is false for this tab.

Standards violated: AC3 ("any slug that fails the gate is NOT added" — the rule the orchestrator
just applied to three other models), AC4 ("no model advertises a [setting] fal will reject"), AC5
(a tile under a tab must run there), and the CHANGELOG claim as written.

Fix (pick one, all inside this slice):
- **Narrowest, matches what the slice already did twice**: change `media.rs:689-693` to
  `("bytedance/seedance-2.5", false, &[InputMode::Text])` and drop the
  `("seedance_2_5", UseCase::ImageToVideo)` row from `use_case.rs:293`, exactly as
  `kling3_0_pro` (`media.rs`) and `wan3_0_prime` were restricted for the `start_image_url`
  mismatch, with the reason recorded in the comment. Text-to-video (including the 30s headline)
  is unaffected and stays.
- Or pin the value: make `app.rs` send `"auto"` when the endpoint `const`s the field — which
  requires recording `const` in `EndpointSchema`, i.e. the already-logged
  "fal-schema-const-and-int-enums" slice. Land that first, then re-land the Image mode.
- Do **not** fix by reordering the spec enum so `"auto"` is first: `App.tsx:342` overrides it on
  attach, so the tile would still fail.

### 3. Withdrawal of the three models — verified complete

`grep -rn -e ltx_2_5 -e 'ltx-2\.5' -e veo3_1_extend -e 'veo3\.1/extend' -e extend-video -e 'LTX 2\.5'`
over `crates/ src-tauri/ ui/ docs/ CHANGELOG.md` (excluding the vendored snapshot, where the fal
index rows legitimately remain) returns **no live code, route, job type, test or changelog
reference**. Only:

- `registry.rs:928`, `media.rs:612`, `media.rs:1614` — `xai/grok-imagine-video/extend-video`, a
  different, pre-existing model. Correct.
- `registry.rs:2229-2232` — the test comment that explains the `+27`. Correct and accurate.
- `.forge/backlog.md:201-207` — the follow-up slice. Out of scope, correct.
- Two stale comments, N1/N2 below.

Counts all re-derived and consistent: `FAL_ROUTE_MODES: [...; 48]` (`media.rs:599`) = 38 + 10 new
rows, and `every_exact_endpoint_is_a_route_the_registry_actually_has` (`media.rs:1658`) would fail
on an orphan row — it passes. `IMPORTED_2026_09: [&str; 11]` (`registry.rs:2813`) lists exactly 11
ids. `cat.len() - EXCLUSIONS.len() + 27` (`registry.rs:2233-2237`) = 11 imported − 1 promoted =
+10 on the previous +17. `audit_fal` CHECKED 63 and `fal_diff` "Registry reaches 63" both fell out
of the code, not out of a literal — no hard-coded 66 survives anywhere.

### 4. The other 10 models re-probed for the B1/B2 defect class — clean

I fetched fal's OpenAPI keyless for all 12 endpoints the 11 models resolve to and scanned every
input property for `const` and for non-string enum members (the two things
`fal_schema.rs:216-219` cannot represent):

| endpoint | `const` | non-string enum |
|---|---|---|
| `bytedance/seedance-2.5/text-to-video` | none | none (duration `'auto','4'…'30'`, all strings) |
| `bytedance/seedance-2.5/image-to-video` | **`aspect_ratio: "auto"`** → B4 | none |
| `fal-ai/kling-video/v3/pro/text-to-video` | none | none (duration `'3'…'15'`, strings) |
| `alibaba/wan-3.0-prime/text-to-video` | none | none |
| `openai/gpt-image-2.5/{flare,sunburst}/{text-to-image,edit}` | none | none |
| `alibaba/qwen-image-3/text-to-image` | none | none |
| `fal-ai/nano-banana-{pro,2}/edit` | none | none (`safety_tolerance '1'…'6'`, strings) |
| `bytedance/seedream/v5/pro/edit` | none | none |

So B4 is the only instance. Every enum the specs do declare I compared member-for-member against
fal's list and they match verbatim (`kling3_0_pro` duration/aspect, `wan3_0_prime`
resolution/aspect, `nano_banana_*_edit` resolution, `seedance_2_5` duration/resolution).

### 5. Checked and found clean (silence is not clearance)

- **The 11 remaining specs' wiring**: each has a fal `Route` *and* a `CostModel` in one
  `priced_routes` arm, a `JOB_TYPES` placement, and a `FAL_ROUTE_MODES` row; `Exact = true` on the
  eight already-suffixed vendor-namespaced endpoints, `Exact = false` on the two family roots.
  Prices unchanged by the withdrawal and re-checked against the refreshed snapshot's prose.
- **`kling3_0_pro` / `wan3_0_prime` restricted to `[Text]`** with the `start_image_url` reason
  recorded (`media.rs:726-736`) and negative assertions in `use_case.rs:313-315` — the right call,
  and the precedent B4's fix should follow.
- **`seedance_2_5` text-to-video** is fully sound: duration enum mirrors fal exactly including
  `"30"`, `seedance_2_5_offers_a_thirty_second_clip` (`registry.rs:2889`) passes, the per-token
  cost model and its 1080p under-estimate note are unchanged from the prior review's verification.
- **Tests were updated, not deleted, by the withdrawal**: `IMPORTED_2026_09` shrank to 11 and the
  three driven tests (`every_imported_model_resolves_to_an_exact_fal_endpoint`,
  `every_imported_model_has_a_fal_route_with_a_real_adapter`,
  `the_models_imported_from_fal_land_on_the_right_tab`) still iterate it; the size assertion was
  corrected rather than relaxed; no `#[ignore]` was added anywhere in the diff.
- **`fal_diff`, `audit_fal`, `fal_catalogue` and the snapshot** are untouched by the repair and
  their prior clearance stands (re-confirmed by the green run above).
- **Docs**: `docs/FIRST-LIGHT.md` and the CHANGELOG bullet contain no reference to a withdrawn
  model; provenance lint passes.

### 6. Minor findings (new this pass — not blocking)

- **N1 — stale doc comment naming withdrawn models.** `crates/hickeyfield-core/src/registry.rs:1536`:
  "…imported 2026-09-16 …: Kling 3.0 Pro, Wan 3.0 Prime, **LTX 2.5 Pro and its animate half, Veo
  3.1 Extend**, GPT Image 2.5 …". Those three no longer exist in the registry. A comment that
  contradicts the code is how the next reader re-adds them. Fix: delete the two names from the
  list.
- **N2 — stale examples in the `FAL_ROUTE_MODES` block comment.** `crates/hickeyfield-core/src/media.rs:629-630`
  describes the block's rows as split by "a tier (`…/text-to-video/pro`), a tiered name
  (`…/flare/edit`) or a verb (`/extend-video`)" — but no `…/text-to-video/pro` row exists anywhere
  in the file any more, and the only `/extend-video` row is the pre-existing xai one in a
  different block. Fix: drop the two dead examples.
- **N3 — prior M1 still open and still false.** `crates/hickeyfield-core/src/registry.rs:1999`
  ships "fal declares `duration` as a plain integer with **no published ceiling**" to the user for
  `wan3_0_prime`; fal publishes `{"anyOf":[{"type":"integer","minimum":2,"maximum":30},{"type":"null"}]}`
  (re-fetched this pass). Fix: say 2–30s.
- **N4 — "every enumerated option set is copied verbatim" is broader than the code.**
  `registry.rs:2044`, `:2066` and the qwen/seedream specs declare `quality`, `background`,
  `image_size`, `output_format` as free-form `ValueSpec::Text` although fal publishes closed enums
  for all four. Harmless today — the UI surfaces none of them, so nothing is sent — but
  implement.md §4's claim does not hold literally. Fix: transcribe the enums or narrow the claim.

Prior review's M2–M7 stand as recorded and need no action in this slice.

### 7. Scope note (not a finding, for the owner)

After the withdrawal, AC3's named minimum set is only partly delivered: MiniMax H3 Max / H3 Max
Turbo (deliberately refused, implement.md §8.1), LTX-2.5 and Veo 3.1 extend-video (withdrawn) are
all absent, and Kling v3 Pro / Wan 3.0 Prime ship text-to-video only. Each omission has a recorded,
defensible reason and AC3's own "a failing slug is not added" rule takes precedence — but the
contract's minimum set is not met, and that is an owner decision to ratify, not something a
reviewer can wave through silently.

### 8. Blocking findings, restated

- **B4** `crates/hickeyfield-core/src/registry.rs:1572-1576` (+ `media.rs:689-693`,
  `use_case.rs:293`) — `seedance_2_5` on the Animate Image tab sends `aspect_ratio` to
  `bytedance/seedance-2.5/image-to-video`, which pins it by `const: "auto"`; 422 on every submit
  of the slice's headline model in one of its two advertised modes, while the slice removed the
  "unverified" hedge and published the claim that every tab shown can run.

Everything else re-verified above is at or above the bar, and the repair itself was executed
cleanly: no dangling references, no stale counts, no relaxed tests.

VERDICT: FAIL
