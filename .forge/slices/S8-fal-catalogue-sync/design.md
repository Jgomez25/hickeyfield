# S8-fal-catalogue-sync — DESIGN

Contract: `.forge/slices/S8-fal-catalogue-sync/slice.md` (AC1–AC7). Upstream: `.forge/objective.md`
(§C1 route-suffix resolver — landed), `.forge/backlog.md` #4. All `file:line` below re-verified
this session against the working tree.

Slice shape: **one tool + one data import + one doc correction**. No UI code, no new module, no
change to `bind`/`reconcile`/`route` logic — every new model reaches the user through tables that
already exist.

---

## 0. Ground the state (corrections to the contract's Ground truth, verified)

The contract says Seedance 2.5 "is absent" and the newest MiniMax is missing. More precisely:

- `seedance_2_5` **already exists** as a placeholder: spec `registry.rs:1397-1409` (prompt only,
  constraint "Announced 2026-08-01 … placeholder"), route `bytedance/seedance-2.5` with
  `CostModel::Unknown` and an "unverified slug" note (`registry.rs:719-728`), `JOB_TYPES` row
  (`registry.rs:306`), and a `FAL_ROUTE_MODES` entry `("bytedance/seedance-2.5", false,
  &[Text, Image])` (`media.rs:660-664`). **Three tests pin it as unpriced/unverified** —
  `registry.rs:2381`, `registry.rs:2491`, `registry.rs:2552`. Importing it for real MUST update
  those three, not delete them.
- `minimax_h3` exists on `minimax/h3` (`registry.rs:1017-1027`, spec `:1518-1530`). **H3 Max is a
  different model at a different price** — it is added alongside, never by re-pointing `minimax_h3`.
- Also already routed and only to be *verified*, not re-added: `flux_2` → `fal-ai/flux-2-pro`
  (`registry.rs:1100`), `gpt_image_2` → `openai/gpt-image-2` (`:1148`), `nano_banana_2` →
  `fal-ai/nano-banana-pro` (`:1195`), `nano_banana_flash`/`nano_banana_2_lite`,
  `seedream_v5_pro`/`_lite` (`:1263`, `:1281`), `grok_video_v15` → `xai/grok-imagine-video/v1.5`
  (`:1001`) and `grok_extend_video` → `xai/grok-imagine-video/extend-video` (`media.rs:611-615`),
  `veo3_1`/`_fast`/`_lite` (`media.rs:635-637`), `wan2_7` → `fal-ai/wan/v2.7` (`registry.rs:775`),
  `kling3_0` → `fal-ai/kling-video/v3/standard` (`:575`), `kling2_6` → `…/v2.6/pro` (`:598`).

This shrinks AC3 from "import 13 families" to "import ~6 genuinely new families + promote
`seedance_2_5` from placeholder to real + verify the rest still resolve". The verification half is
not optional: AC3 says *each* model in the minimum set passes `audit_fal`.

---

## 1. AC1 — the diff tool: `crates/hickeyfield-core/examples/fal_diff.rs`

### 1.1 Decision: one example, two modes — `fal_diff` subsumes `dump_fal_catalogue`

`fal_catalogue.rs:43` documents `cargo run -p hickeyfield-core --example dump_fal_catalogue`, which
**does not exist** (verified: the only two hits for that string in the repo are that doc comment and
the slice file). Two candidate fixes:

- (a) restore `dump_fal_catalogue.rs` *and* add `fal_diff.rs` — two examples, two copies of "fetch,
  shape a `SnapshotFile`, print";
- (b) **one example with a `--dump` flag. RECOMMENDED and specified below.**

Rationale: both modes need the identical live fetch and the identical row shape, and the dump is
one `println!` off the same `Catalogue`. A second example is a second thing to keep in step with
`SnapshotFile` (`fal_catalogue.rs:713-717`). The module doc at `fal_catalogue.rs:40-50` MUST be
updated to the new command in the same commit; leaving it pointing at a non-existent example is the
exact defect AC1 exists to close.

### 1.2 Interface

```
cargo run -p hickeyfield-core --example fal_diff               # diff, human-readable, stdout
cargo run -p hickeyfield-core --example fal_diff -- --dump     # snapshot JSON, stdout
cargo run -p hickeyfield-core --example fal_diff -- --offline  # diff against the bundled snapshot
```

- Arguments MUST be parsed with `std::env::args()` (no new dependency; the crate has no `clap`).
- `--dump` MUST write to stdout only. The tool MUST NOT write `vendor/fal-catalogue-snapshot.json`
  itself; the refresh is the shell redirect already documented at `fal_catalogue.rs:42-46`. A tool
  that overwrites a vendored file in place is a tool that can silently destroy the offline fallback
  on a partial fetch.
- Exit non-zero on `CatalogueError` (`fal_catalogue.rs:723-771`), printing the error's own
  `Display` — it already distinguishes `BotCheckpoint` / `Http` / `Transport` / `Malformed`, and a
  Vercel challenge must not read as "fal deleted 1,400 models".

### 1.3 Data flow

1. **Fetch** — `fal_catalogue::fetch()` (`:790`). Blocking, paged, guarded by `MAX_PAGES = 60`
   (`:69`), `FETCH_TIMEOUT = 30s` (`:73`) and `MAX_UNREADABLE_SHARE = 0.10` (`:82`). Unauthenticated;
   no key is read and none MUST be. `--offline` uses `Catalogue::bundled()` (`:579`) instead.
2. **Dump mode** — serialize `{"snapshot_date": "<UTC YYYY-MM-DD>", "items": <catalogue.all()>}`.
   `Model` is `Serialize` (`:387`) and `SnapshotFile` is private + `Deserialize`-only (`:713`), so
   the example MUST build the object with `serde_json::json!`, not construct `SnapshotFile`.
   Caveat, recorded rather than hidden: `Catalogue::all()` (`:600`) is post-filter — retired,
   id-less and duplicate rows are already dropped (`:524-544`). fal carried **zero** `deprecated`/
   `removed` rows at capture, so the dump is lossless today. The tool MUST therefore assert
   `catalogue.retired() == 0` (`:667`) before dumping and **fail loudly** if not, rather than
   silently baking fal's retirements out of the vendored file.
3. **Registry side** — `registry::registry()` (`:88`) → for each `Model`, each `Route` with
   `provider == ProviderId::Fal`, take `route.slug` (this is exactly what `examples/dump_fal_slugs.rs`
   does, lines 1-10).
4. **Expand registry slugs to endpoints.** A registry slug is often a *family root*
   (`media.rs:531-537`), so a raw slug-set difference would report `fal-ai/kling-video/v3/standard`
   as "missing from fal". The tool MUST expand each slug through the same resolver the submit path
   uses — `media::resolve_endpoint(slug, mode, produces_video)` (`media.rs:865-902`), for
   `mode ∈ {Text, Image, Video}` and `produces_video = (model.modality == Video)` — collecting every
   `Ok(_)` into a `BTreeSet<String>` of **covered endpoint ids**. `Err(UnsupportedMode)` is skipped
   (that mode is measured not to exist). This is the identical construction `audit_fal.rs:48-59`
   uses, so the two tools cannot disagree about what the registry covers.
5. **Diff** — `fal_minus_registry = { m ∈ catalogue.all() : m.id ∉ covered }`, matched by **exact
   endpoint id**. Exact, never prefix: `fal_catalogue.rs:621-628` spells out why (`fal-ai/flux` and
   `fal-ai/flux/dev` are different endpoints at different prices). Vendor-namespaced ids
   (`minimax/…`, `bytedance/…`, `openai/…`, `alibaba/…`, `xai/…`) need no special case *because*
   the comparison is exact-string on the full id.

   Worked example. Registry holds `kling3_0` (video) on `fal-ai/kling-video/v3/standard`. Expansion
   gives `{…/standard/text-to-video, …/standard/image-to-video}` (Video mode errors: the table entry
   `media.rs:695-699` lists Text+Image only). fal's index holds all three of
   `fal-ai/kling-video/v3/standard/text-to-video`, `…/image-to-video`, `…/pro/text-to-video`. Result:
   the two standard rows are covered and suppressed; `…/pro/text-to-video` is reported as new —
   which is the true answer, and the one a naïve slug diff would have missed in both directions.
6. **Bucket by use case.** Map fal's `Category` (`fal_catalogue.rs:99-135`) onto the app's 5
   `UseCase` values (`use_case.rs:54-60`):

   | fal `Category` | `UseCase` |
   |---|---|
   | `TextToVideo` | `TextToVideo` |
   | `ImageToVideo` | `ImageToVideo` |
   | `VideoToVideo` | `EditVideo` |
   | `TextToImage` | `TextToImage` |
   | `ImageToImage` | `EditImage` |

   Every other category (`Training`, `Llm`, `TextToAudio`, `Vision`, …, `Other(_)`, `Unknown`) is
   **out of scope for this slice** and MUST be dropped from the report with a single trailing count
   line ("N models in 21 other categories, not shown"), never silently. `Category::Other` and
   `Unknown` are load-bearing on purpose (`fal_catalogue.rs:98,130`) and must not be coerced into a
   use case.
7. **Print.** One section per `UseCase` in `UseCase::ALL` order, rows sorted by `Model::id`:
   `  <id>  |  <title>  |  <modelFamily or "-">  |  <pricing>`, where pricing is
   `Model::pricing()` (`fal_catalogue.rs:424`) rendered as its rate when it reduces to one and
   `(prose)` / `(none)` otherwise — the module is explicit that it is not a price feed
   (`:18-26`), so the tool MUST NOT present a parsed rate as authoritative. Header line records
   `catalogue.captured()` (`:662`) and `catalogue.len()`, so a report can never be mistaken for a
   fresher one than it is. Footer: per-section counts + total covered/uncovered.

### 1.4 Testability and the named unit test

The diff logic MUST be a pure function in the example, not inline in `main`:

```rust
/// `covered` is the endpoint set expanded from the registry; returns the fal rows
/// no registry route reaches, bucketed by use case.
fn fal_minus_registry(
    catalogue: &Catalogue,
    covered: &BTreeSet<String>,
) -> BTreeMap<UseCase, Vec<&Model>>
```

Test (in a `#[cfg(test)] mod tests` inside `examples/fal_diff.rs`; `cargo test --workspace` compiles
and runs example tests):

- **`fal_minus_registry_reports_only_endpoints_no_route_reaches`** — builds a fixture
  `Catalogue::new(vec![…], Captured::Snapshot{date})` (`:524`) of ~6 hand-written `Model` rows
  covering: a covered family-root expansion, an uncovered vendor-namespaced id, a row in an
  out-of-scope category, and a retired row. Asserts the retired row never appears (dropped at
  construction), the out-of-scope row is not bucketed, the covered row is suppressed, the new one is
  reported under the right `UseCase`. **No network**: the fixture is constructed in-process, and
  `covered` is passed in as a literal set. MAY be split into a second test
  `the_category_map_only_claims_the_five_use_cases` if the first grows past ~40 lines.

---

## 2. AC2 — snapshot refresh

`cargo run -p hickeyfield-core --example fal_diff -- --dump > crates/hickeyfield-core/vendor/fal-catalogue-snapshot.json`
then `cargo test -p hickeyfield-core fal_catalogue`. The snapshot tests assert the counts measured
at capture (`fal_catalogue.rs:47-50`); the refresh WILL fail them and the expected numbers MUST be
updated **in the same commit**, which is the documented design. `snapshot_date` moves from
`2026-08-05` (verified in the vendored file, line 2) to the refresh date.

DESIGN-only boundary: this design performs no fetch and no refresh. Every "resolve from the
refreshed snapshot" below is an instruction to the implementer, executed after AC2 lands.

---

## 3. AC3 — the curated import

### 3.1 The per-model edit list (six edits, same six every time)

For a new model id `X` with fal slug `S`:

1. **Spec** — `picker_only_specs()` (`registry.rs:1394`), mirroring `minimax_h3` (`:1518-1530`) or
   `seedance_pro` (`:1421-1437`). Built by the `spec()` helper (`:1786-1809`), which sets
   `alias: None`, `default: None`, `arity: One`. Where the parameter surface is not fully known,
   wrap in `ModelSpec { constraints: vec![…], ..spec(…) }` as `:1397` and `:1455` do.
2. **Route + price** — a `priced_routes` arm (`:568`): `Route::noted(ProviderId::Fal, S, "<note>")`
   paired with a `CostModel`. Video: `per_second(usd)` (`:489`) or `per_second_audio(usd, mult)`
   (`:496`); Seedance-family token billing: `per_token` (`:506`) — the note at `:614-620` explains
   why per-second arithmetic is wrong there. **Price provenance rule (MUST):** the number comes
   from the refreshed snapshot's `pricingInfoOverride` via `Model::pricing()`; if it does not reduce
   to a rate, the route is `CostModel::Unknown` with a note saying so. Never a plausible-looking
   guess — `registry.rs:2374` pins that rule.
3. **`JOB_TYPES`** — add the id to the `"video"` group (`registry.rs:286-318`) or the right image
   group (`:375-400`). Omitting it trips the `debug_assert` at `:464-468`.
4. **`FAL_ROUTE_MODES`** — `media.rs:599`, a fixed-size array; the length literal `38` MUST be
   bumped by the number of entries added. Entry = `(S, Exact, &[InputMode…])`. See §3.2 for how
   `Exact` is chosen.
5. **`NO_END_FRAME`** (`media.rs:786`, len `8`) — add `S` only if the probed schema has a start-frame
   key and **no** end-frame key. **`FAL_NO_MEDIA`** (`media.rs:751`, len `10`) — add only if the
   schema declares no media key at all. **`NO_MEDIA_MODELS`** (`use_case.rs:36`, len `10`, keyed by
   *model id* not slug) — same condition, model-side, so the picker hides it from the media tabs.
   All three lengths are literals and MUST be bumped when touched.
6. **`LAUNCH_FAMILIES`** (`registry.rs:204`, len `12`) — **default: do not touch.** It is the M1
   onboarding set and `registry.rs:2265` asserts every launch model is runnable. Adding a frontier
   model here is an editorial call outside AC3. Recorded as Open question Q2.

### 3.2 The `Exact` rule (this is the part that decides whether a slug 404s)

`Exact` and the mode list are independent facts (`media.rs:806-813`). The rule for the new
vendor-namespaced families:

> If fal serves the family's endpoints under a common prefix **and** the suffixes are exactly the
> strings `InputMode::suffix` produces (`media.rs:571-580`: `/text-to-video`, `/image-to-video`,
> `/video-to-video`, `/text-to-image`, `/image-to-image`), the registry MUST store the **family
> root** with `Exact = false` and the measured mode list. Otherwise the registry MUST store the
> **complete endpoint id** with `Exact = true`.

Worked example — MiniMax H3 Max. fal serves `minimax/h3-max/text-to-video`,
`…/image-to-video`, `…/reference-to-video`, `…/camera-controls`, `…/director`. The first two *are*
`InputMode::suffix` outputs, so one route `minimax/h3-max` with
`("minimax/h3-max", false, &[InputMode::Text, InputMode::Image])` covers both modes from one
registry entry, exactly as `minimax/h3` does today (`media.rs:711`). The other three are **not**
reachable by any `InputMode` — `resolve_endpoint` can never produce `/reference-to-video` — so each
is either its own model id with `Exact = true`, or it is not imported. Default: **not imported this
slice** (Non-goal N3); `reference-to-video` is the specific shape flagged in §7 as riskiest.

Counter-example — `bytedance/seedance-2.5/text-to-video` taken as a route slug directly: it must be
`Exact = true`, because `resolve_endpoint`'s `MODE_TAILS` guard (`media.rs:889-898`) would otherwise
be the only thing stopping a double suffix, and relying on a fallback guard for a slug we know the
shape of is how `…/edit-video/video-to-video` shipped. Prefer the family root; where the root is not
served, use the exact endpoint with `Exact = true`.

### 3.3 The minimum set, model by model

"Resolve at implement time" is a recorded answer, not a gap: guessing a slug here is precisely the
failure `docs/FIRST-LIGHT.md` records, and §4's audit gate cannot rescue a slug invented in a design.

| # | Model | Action | Slug | Exact / modes | Notes |
|---|---|---|---|---|---|
| 1 | MiniMax H3 Max | **new** `minimax_h3_max` | `minimax/h3-max` | `false`, `[Text, Image]` | Spec mirrors `minimax_h3` (`registry.rs:1518`); 5–15s, 2K. Price from snapshot. `JOB_TYPES` → `video`. |
| 2 | MiniMax H3 Max Turbo | **new** `minimax_h3_max_turbo` | `minimax/h3-max-turbo` | `false`, `[Text, Image]` | Same spec shape, own price. |
| 3 | Seedance 2.5 | **promote** existing `seedance_2_5` | `bytedance/seedance-2.5` (already routed, `registry.rs:722`) | entry already exists, `media.rs:660-664` | Replace the placeholder spec (`:1397-1409`) with real flags incl. duration (§5) and audio; replace the `CostModel::Unknown` + "unverified slug" note with a sourced price; **update the 3 tests in §6**. |
| 4 | Kling v3 pro | **new** `kling3_0_pro` | `fal-ai/kling-video/v3/pro` | `false`, `[Text, Image]` (mirrors `/v3/standard`, `media.rs:695-699`) | Price already recorded in the standard route's note: `$0.112/s` audio-off, `$0.168` audio-on (`registry.rs:576-577`) → `per_second_audio(0.112, 1.5)`. Keep `kling2_6`/`kling3_0` untouched; `kling3_0_turbo` stays in `FAL_MISSING_ROUTES` unless the refreshed snapshot carries `fal-ai/kling-video/v3/turbo`. |
| 5 | Veo 3.1 (+fast) | **verify only** | `fal-ai/veo3.1`, `/fast`, `/lite` (`media.rs:635-637`, `Exact`, no media) | unchanged | Must still pass §4. |
| 6 | Veo 3.1 extend-video | **new** `veo3_1_extend`, *if served* | resolve from refreshed snapshot | `true`, `[Video]` | Mirrors `grok_extend_video` (`media.rs:611-615`); `JOB_TYPES` → `animate` (`registry.rs:272-280`), which is where clip-editing belongs. |
| 7 | Wan 2.7 | **verify only** | `fal-ai/wan/v2.7` (`registry.rs:779`) | unchanged | |
| 8 | Wan 3.0 prime | **new** `wan3_0_prime`, *if served* | resolve from refreshed snapshot (do **not** guess `alibaba/…` vs `fal-ai/wan/…`) | resolve | |
| 9 | LTX-2.5 (+extend) | **new** `ltx_2_5` (+ `_extend`), *if served* | resolve from refreshed snapshot | resolve | No LTX model exists in the registry today. |
| 10 | Grok video 1.5 + extend | **verify only** | `xai/grok-imagine-video/v1.5` (`registry.rs:1001`), `xai/grok-imagine-video/extend-video` | unchanged | `v1.5` is already in `NO_END_FRAME` (`media.rs:788`) — keep. |
| 11 | FLUX.2 pro | **verify only** | `fal-ai/flux-2-pro` (`registry.rs:1100`) | unchanged | |
| 12 | GPT Image 2.5 (flare/sunburst, t2i + edit) | **new** id(s) | resolve from refreshed snapshot — the tier names are fal's and MUST be read from the index, not from this design | resolve; an `/edit` path is `Exact = true` | `gpt_image_2` stays as-is (`registry.rs:1148`). Price: token-billed → `CostModel::Unknown` unless the snapshot states a rate, matching the reasoning at `:1133-1137`. |
| 13 | Nano Banana 2 / Pro (+edit) | **verify** the three existing ids; **new** edit id(s) *if served* | existing `fal-ai/nano-banana-pro` (`:1195`), `fal-ai/nano-banana-2`, `google/nano-banana-2-lite`; edit path resolve | edit path `true`, `[Image]` | The three existing ids are in `FAL_NO_MEDIA` (`media.rs:755-760`) and `NO_MEDIA_MODELS`; an edit endpoint that *does* take media is a **different id**, and must not be conflated. |
| 14 | Seedream 4 edit / 5 | **verify** `seedream_v5_pro`/`_lite`; **new** edit id *if served* | resolve | resolve | Note `seedream_v5_pro` is in `FAL_NO_MEDIA` (`media.rs:752`). |
| 15 | Qwen Image 3 | **new** `qwen_image_3`, *if served* | resolve from refreshed snapshot | resolve | No Qwen model exists in the registry today. |

**`minimax_hailuo` (the dead pin).** Today: route `fal-ai/minimax/hailuo-2.3` with
`CostModel::Flat{0.49}` plus a Higgsfield route (`registry.rs:1032-1046`); the fal slug is pinned in
`FAL_MISSING_ROUTES` (`media.rs:730-734`, len `3`). Decision procedure at implement time, in order:

1. Search the refreshed catalogue for a Hailuo 2.3 endpoint —
   `fal_catalogue::search("hailuo")` (`:963`) plus an exact `get()` on the pinned slug (`:957`).
2. **If a live endpoint exists**: re-point the fal route to it, remove `"fal-ai/minimax/hailuo-2.3"`
   from `FAL_MISSING_ROUTES` and bump the length literal to `2`, add a `FAL_ROUTE_MODES` entry per
   §3.2, and run the §4 gate on it. Keep the Higgsfield route.
3. **Else leave it exactly as it is** — Higgsfield-only in practice — and add nothing. The pin is
   load-bearing: `use_case::route_serves` (`use_case.rs:213-215`) returns false on a missing route,
   which is what keeps a dead tile out of the picker.

Recorded now so the implementer does not have to re-derive it: option 3 is the default.

---

## 4. AC3/AC7 — the verification gate

**Command:** `cargo run -p hickeyfield-core --example audit_fal` (no key; fal's schema endpoint is
unauthenticated — `examples/audit_fal.rs:18-19`). Run **after** each batch of registry edits and
**before** landing them.

What it does per fal route (`audit_fal.rs:36-153`): resolves each `InputMode` through
`media::resolve_endpoint`, fetches `fal_schema::for_endpoint` (`:61`), and reports four buckets —
silently-ignored media, missing required fields, dropped settings, rejected options.

**Pass, for a newly imported slug, means all of:**

- it appears in the run's `checked` count (i.e. at least one mode resolved *and* fal returned a
  schema), and
- it appears in **none** of the four printed sections, and
- it appears in **none** of the `unreachable` entries.

**Rule (MUST): a slug that fails any of the above is NOT landed.** No "unverified" variant is
added under the owner's AC3 escape hatch without the owner saying so in writing on this slice.

**Defect to fix as part of this slice (AC3 depends on it).** `audit_fal.rs:33` declares
`unreachable`, `:62` pushes `"{model_id} -> {endpoint}"` into it when `fal_schema::for_endpoint`
returns `None` — and **nothing ever prints it**: the four `section(...)` calls at `:156-171` do not
include it. Verified by reading the whole file. So today the single most important signal for this
slice ("fal does not serve the slug you just added") is computed and thrown away. The implementer
MUST add a fifth `section("UNREACHABLE — fal serves no schema for this endpoint", &unreachable)`
before the summary line at `:173`. This is ~3 lines and is the difference between an evidence-grade
gate and a green-looking no-op. The pre-existing unreachable entries (the three
`FAL_MISSING_ROUTES` models and any Higgsfield-only route) will appear for the first time — they are
expected output and MUST be recorded in the evidence as the known baseline, not "fixed".

**Evidence (AC7):** the full `audit_fal` stdout, plus a one-line pass/fail per newly imported slug,
recorded by the implement/test agents. Full regression set per AC7: `cargo test --workspace`,
`cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
`cargo deny check`, `./scripts/lint-provenance.py`, and in `ui/`: `pnpm test`, `pnpm build`.

---

## 5. AC4 — the duration axis

Mechanism, verified: `ModelSpec::capabilities()` (`catalog.rs:242-268`) reads the `duration` flag —
`supports_duration = flag.is_some()`, `durations = numeric_options(f)` which is **non-empty only for
`ValueSpec::Enum`** (`catalog.rs:199-204`), `default_duration` from `f.default` (always `None` for
`spec()`-built flags, `registry.rs:1802`). `ModelCapability::from_catalog` (`capability.rs:428-432`)
turns that into an `Axis`; `from_fal` (`:470-474`) overrides it from fal's own schema when a schema
is available, keeping only the catalogue's *default*. `Axis::is_free_form` (`:283-285`) =
`support == Yes && values.is_empty()`, which the UI renders as a number input rather than a chip row
(`capability.rs:196-198`). `duration_seconds()` (`:538-544`) is the estimator's view and returns
`None` if any offered value fails `parse_seconds`.

**Decision — Seedance 2.5 declares duration free-form, not an enum.** `("duration", false,
ValueSpec::Number)` on the spec, plus a verbatim constraint string on the `ModelSpec`
(`"fal serves 4-30s; 30s is the native maximum"`), which flows to `Capabilities::constraints`
(`catalog.rs:266`) and out to the UI. Reasons, in order:

1. It lets the UI offer 30s. `Number` → `numeric_options` empty → `supports_duration = true` with no
   values → `is_free_form` → number input → the user types `30`. An enum would also work only if it
   listed every value fal accepts, and fal's own schema is the only source for that list.
2. It cannot advertise a duration fal rejects (AC4's actual requirement). `audit_fal` check 4
   (`audit_fal.rs:133-152`) flags exactly the case where our enum offers values the endpoint
   rejects; a free-form axis is unflaggable there because it claims nothing.
3. **Exception (MUST):** if the refreshed probe shows fal *enumerates* `duration` for
   `bytedance/seedance-2.5/*`, mirror that enum exactly as `ValueSpec::Enum([...])`. An enum copied
   from fal is strictly better than free-form; an enum invented here is strictly worse.

**Known caps are preserved by not touching them.** Veo (8s), Hailuo (10s), Wan 2.6 (15s) and every
other capped model take their duration values from the vendored `MODELS.md` parse
(`catalog.rs` catalogue) or from fal's schema, neither of which this slice edits. The only spec
duration flags this slice authors are on new models and on `seedance_2_5`.

**Non-regression (MUST NOT "fix"):** 28 of 32 video models declare `duration` as a plain integer and
therefore have no in-app ceiling (`capability.rs:196-198`); fal enforces. This slice neither adds a
ceiling nor removes one. New free-form models simply join that set.

---

## 6. AC5 — tests

**Updated, never deleted:**

- `registry.rs:1855` `the_twelve_picker_only_models_are_all_present` — the id list grows by each new
  hand-authored spec; the test name and its `!catalog::catalogue().contains_key(id)` assertion stay.
  The name is now a count-lie; renaming to `the_picker_only_models_are_all_present` is acceptable and
  SHOULD be done, with the doc comment at `registry.rs:1378-1383` updated to match.
- `registry.rs:1847-1851` `registry_covers_the_catalogue_minus_its_exclusions` — the `+ 17` literal
  MUST be bumped by the number of new ids. Easy to miss; it fails loudly.
- `registry.rs:2381` `models_the_corpus_could_not_price_stay_unknown` — remove `"seedance_2_5"`
  **iff** it gets a sourced price; keep the other three.
- `registry.rs:2491` `unverified_slugs_are_flagged_in_the_note` — remove `"seedance_2_5"` once its
  slug is verified by §4; `"outpaint"` stays, so the test keeps its teeth.
- `registry.rs:2552` (inside `hand_authored_specs_carry_their_parameters`) — the
  `!reg["seedance_2_5"].spec.constraints.is_empty()` assertion still holds with the new 4–30s
  constraint string; no change needed, confirm rather than edit.
- `registry.rs:1993` `there_are_twelve_launch_families` — unchanged under the §3.1(6) default of not
  touching `LAUNCH_FAMILIES`. If Q2 is answered "yes", both this and `registry.rs:2047` change.
- `registry.rs:2281` `the_unroutable_models_are_a_known_short_list` — **unchanged**, because every
  imported model gets a `ProviderId::Fal` route and Fal `has_adapter()`. If it fails, a model was
  landed with no executable route; fix the model, not the test.
- `media.rs:1620` `every_exact_endpoint_is_a_route_the_registry_actually_has` — iterates
  `FAL_ROUTE_MODES` and asserts each slug is a live fal route. Automatically covers the new entries;
  **this is why a `FAL_ROUTE_MODES` entry MUST NOT be added before its route exists.**
- `catalog.rs:437` — untouched. `MODELS.md` is vendored and this slice does not re-vendor it. If
  that test moves, something went wrong.

**New (AC5 + AC3 + AC4):**

- `every_imported_model_resolves_to_an_exact_fal_endpoint` (registry or media tests) — for each new
  id, its Fal route resolves through `media::resolve_endpoint` for at least one `InputMode` and the
  result is **not** equal to a bare family root that `FAL_ROUTE_MODES` marks non-exact without modes.
- `every_imported_model_has_a_fal_route_with_a_real_adapter` — for each new id,
  `routes.iter().any(|r| r.provider == ProviderId::Fal && r.provider.has_adapter())`. Guards against
  importing a frontier model that only reaches Higgsfield.
- `seedance_2_5_offers_a_thirty_second_clip` — via `ModelSpec::capabilities()`: either
  `durations` contains `30.0`, or the axis is free-form (`supports_duration && durations.is_empty()`)
  so the UI's number input can reach 30. Written to accept both so it survives §5's exception branch.
- `fal_minus_registry_reports_only_endpoints_no_route_reaches` — §1.4, in `examples/fal_diff.rs`.

**UI:** no `ui/` source change is expected — the picker derives its tabs from `use_case::supports`
/ `route_serves` (`use_case.rs:166`, `:207`) over whatever the registry holds. `pnpm test` and
`pnpm build` still run as AC7 gates. If a `vitest` snapshot pins a model count, it is updated, not
deleted.

---

## 7. AC6 — stale docs, and the document step

- **`docs/FIRST-LIGHT.md:32-95`** (§"What broke: the input-mode suffix was never implemented"). The
  heading and the narrative are historically accurate for 2026-08-05 and MUST be preserved as
  history. Edit: retitle to `## What broke: the input-mode suffix (resolved 2026-08-28)` and insert a
  short **Resolved** note immediately under it, before the `The first real submit failed:` block —
  the resolver is `media::resolve_endpoint` (`media.rs:865`) with the measured mode table
  (`media.rs:599`), wired at `app.rs:276-296`, landed in commits `3d238b8` / `0291e23` / `d075307`
  on 2026-08-28. The "Consequences" item 1 (`:95-97`) gets a `(done)` marker; item 2 gets a pointer
  to `media::FAL_MISSING_ROUTES` as the mechanism actually used instead of `EXCLUSIONS`; item 3
  (cost accuracy still unverified) **stays open** — this slice does not spend money.
- **`.forge/backlog.md` #4 `live-fal-e2e`** (`:65-70`): bullet 1 (`:68`, route-suffix resolver) is
  marked `[x]` with the same commit citation. Bullet 2 (`:69`, the live proven-green run) **remains
  open** — S8 imports models, it does not run one. Row `:22` and the traceability line `:135` are
  left alone; the slice stays `pending` because its second bullet is its point.
- **`CHANGELOG.md`** — one bullet under `## [Unreleased]` → `### Added` (`:19`), in the existing
  user-facing voice, original wording (`scripts/lint-provenance.py` is an AC7 gate; model names and
  slugs are fine per AC6): *"**The newest fal models are available.** …"* naming what the user can
  now pick and that a 30-second Seedance 2.5 clip is generatable. No `### Fixed` entry — nothing
  user-visible was broken.
- **`fal_catalogue.rs:40-50`** — the "Refreshing the snapshot" block MUST name `fal_diff --dump`
  (§1.1). This is a doc fix inside source; it is part of AC1, not optional polish.
- **README** — checked at implement time for a fal-example list; if one exists, `fal_diff` is added
  beside `audit_fal`/`first_light`. Otherwise N/A.
- **`document.md`** — written after the docs land, recording what changed and where (file + section),
  per the FORGE artifact set.

---

## 8. Non-goals

- **N1.** Bulk-mirroring fal's 1,418 models into the registry. Owner decision (slice.md:12): curated
  and verified, model by model.
- **N2.** Wiring `fal_catalogue` into the app UI (a fal model browser). The module is still
  "WIRED TO NOTHING" after this slice except through an example binary, and that is deliberate — the
  registry remains the single source of what is routable (`fal_catalogue.rs:28-30`).
- **N3.** Importing the non-`InputMode` endpoints of a family (`/reference-to-video`,
  `/camera-controls`, `/director`). They need their own model ids, their own media roles and their
  own `bind` verification; that is a slice, not a table row. See §7 risk.
- **N4.** A live paid generation. Objective §C2 / backlog #4 bullet 2 stay open.
- **N5.** Changing `bind`, `reconcile`, `can_bind`, `route::resolve`, the estimator, or any UI
  component.
- **N6.** Re-vendoring Higgsfield's `MODELS.md`.

## 9. Open questions (each with the default this build assumes)

- **Q1 — one example or two?** **Default: one (`fal_diff` with `--dump`), and fix the doc comment.**
  §1.1. Reversible in ten minutes if the owner prefers two named examples.
- **Q2 — do any imported models join `LAUNCH_FAMILIES`?** **Default: no.** §3.1(6). Keeps the M1
  onboarding set and its two tests (`registry.rs:1993`, `:2047`) stable; adding one is editorial and
  can be a one-line follow-up.
- **Q3 — what if a frontier slug resolves but `audit_fal` reports dropped settings?** **Default:
  land it, and remove the offending flag from the hand-authored spec** so the UI stops offering a
  control the endpoint ignores. A *rejected option* or *silently ignored media* finding is a
  hard fail; a dropped setting is a spec we wrote too optimistically.
- **Q4 — does the `--dump` output preserve fal's retired rows?** **Default: no, and assert
  `retired() == 0` before writing** (§1.3 step 2). If fal ever retires a row, the tool fails loudly
  and someone decides deliberately.
- **Q5 — `minimax_hailuo`.** **Default: leave Higgsfield-only** unless the refreshed snapshot shows
  a live Hailuo 2.3 endpoint (§3.3).
- **Q6 — Seedance 2.5 duration representation.** **Default: free-form `ValueSpec::Number` + a 4–30s
  constraint string**, upgraded to a mirrored enum only if fal's schema enumerates (§5).

## 10. The riskiest part

**Importing a slug that resolves in fal's *index* but whose *OpenAPI* uses media keys the app does
not bind.** The index answers "does this endpoint exist"; it says nothing about field names.
`reference-to-video`-shaped endpoints are the concrete case: they take a list of reference stills
under a key that is neither `image_url` nor `video_url`, so `media::bind`/`reconcile` either refuses
the attachment at submit (a dead tile the picker offered) or — the expensive direction — fal silently
drops the key and bills for an unrelated generation, which is the exact failure mode
`audit_fal.rs:9-12` and `use_case.rs:28-31` were both written after.

Mitigation, in layers: (a) §3.2's `Exact` rule keeps non-`InputMode` endpoints out of family-root
routes entirely; (b) N3 defers them; (c) the §4 audit gate — **with the `unreachable` section
actually printed** — is what makes "the schema differs" visible before landing rather than after a
charge; (d) `FAL_NO_MEDIA` / `NO_END_FRAME` / `NO_MEDIA_MODELS` entries are derived from the same
probe, so the picker and `can_bind` agree.

Second-riskiest, and worth the verifier's attention: **the array-length literals**. `FAL_ROUTE_MODES`
(`media.rs:599`, `38`), `FAL_MISSING_ROUTES` (`:730`, `3`), `FAL_NO_MEDIA` (`:751`, `10`),
`NO_END_FRAME` (`:786`, `8`), `NO_MEDIA_MODELS` (`use_case.rs:36`, `10`), `LAUNCH_FAMILIES`
(`registry.rs:204`, `12`), and the `+ 17` at `registry.rs:1849`. Each is a compile error or a test
failure when wrong, so none can ship silently — but they are the highest-frequency edit in this
slice and the easiest to under-count.
