# S8-fal-catalogue-sync — IMPLEMENT claim

Branch: `forge/s8-fal-catalogue-sync`. No merge, no PR. `.forge/state.json` and
`.forge/STATUS.md` untouched by me (state.json was already dirty when the session opened).

---

## 1. Files changed, and which AC each serves

| File | Change | AC |
|---|---|---|
| `crates/hickeyfield-core/examples/audit_fal.rs` | **Prerequisite fix**: the `unreachable` bucket, collected at `:62` and never printed, now has its own `section(...)`. Also replaced the bare `checked` counter with a `CHECKED` section listing every `model -> endpoint` fal answered for — without it there is no per-slug pass evidence. | AC3, AC7 |
| `crates/hickeyfield-core/examples/fal_diff.rs` | **New.** Live fal index → registry diff, bucketed by the 5 use cases; `--dump` refreshes the snapshot to stdout; `--offline` diffs the bundle. Two fixture unit tests, no network. | AC1 |
| `crates/hickeyfield-core/Cargo.toml` | `[[example]] name = "fal_diff", test = true` — without it `cargo test --workspace` compiles the example's tests and never runs them. | AC1, AC7 |
| `crates/hickeyfield-core/vendor/fal-catalogue-snapshot.json` | Regenerated from live fal. `snapshot_date` `2026-08-05` → `2026-09-16`; 1,418 → 1,491 rows (100 added, 27 retired). | AC2 |
| `crates/hickeyfield-core/src/fal_catalogue.rs` | Snapshot test constants + category counts updated to the refresh (same commit, as designed). Module doc: refresh command now names `fal_diff --dump` (the documented `dump_fal_catalogue` never existed); measured counts refreshed. | AC1, AC2 |
| `crates/hickeyfield-core/src/registry.rs` | 13 new models (spec + priced route + `JOB_TYPES`), `seedance_2_5` promoted from placeholder to a real probed+priced model, `+17` → `+30` registry-size literal, test updates + 3 new tests. | AC3, AC4, AC5 |
| `crates/hickeyfield-core/src/media.rs` | 13 new `FAL_ROUTE_MODES` entries, array length `38` → `51`. | AC3, AC5 |
| `crates/hickeyfield-core/src/use_case.rs` | New test `the_models_imported_from_fal_land_on_the_right_tab`. | AC5 |
| `docs/FIRST-LIGHT.md` | §"input-mode suffix" retitled `(resolved)` with a Resolved note; Consequence 1 marked done, Consequence 2 answered with `FAL_MISSING_ROUTES` + a 2026-09-16 re-probe; Consequence 3 (cost accuracy) left open. | AC6 |
| `.forge/backlog.md` | #4 bullet 1 `[x]` with the git-confirmed commits; bullet 2 (live proven-green run) left open. | AC6 |
| `CHANGELOG.md` | One `[Unreleased] › Added` bullet, original wording; provenance lint passes. | AC6 |

Not touched: `LAUNCH_FAMILIES` (design §3.1(6) default), `FAL_MISSING_ROUTES`, `FAL_NO_MEDIA`,
`NO_END_FRAME`, `NO_MEDIA_MODELS`, `bind`/`reconcile`/`route::resolve`, any `ui/` source,
`MODELS.md`.

---

## 2. AC1 — the diff tool

```
$ cargo test -p hickeyfield-core --example fal_diff
running 2 tests
test tests::the_category_map_only_claims_the_five_use_cases ... ok
test tests::fal_minus_registry_reports_only_endpoints_no_route_reaches ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

and it runs inside the workspace gate (see §7: `Running unittests examples/fal_diff.rs …
2 passed`).

Live run, no key:

```
$ cargo run -q -p hickeyfield-core --example fal_diff -- --offline | head -2
fal index: 1491 model(s), bundled snapshot of 2026-09-16. Registry reaches 66 fal endpoint(s).
```

Design deviations, both deliberate and both visible in the code:

- the bucket map is `HashMap<UseCase, …>`, not `BTreeMap` — `UseCase` derives `Hash`/`Eq` but
  not `Ord`, and adding `Ord` to a public enum to satisfy an example is the wrong direction.
  Output order still comes from `UseCase::ALL`, so the report is deterministic.
- `--dump` refuses to write when `catalogue.retired() > 0` (design Q4), verified only by
  reading the code — fal published zero retired rows at capture, so the branch did not fire.

## 3. AC2 — snapshot refreshed

```
$ cargo run -q -p hickeyfield-core --example fal_diff -- --dump > crates/hickeyfield-core/vendor/fal-catalogue-snapshot.json
$ python3 -c "..."   # compared old vs new
date 2026-09-16 items 1491
old date 2026-08-05 old items 1418
added 100 removed 27
$ cargo test -p hickeyfield-core fal_catalogue
test result: ok. 50 passed; 0 failed; 0 ignored; 0 measured; 612 filtered out
```

Constants updated in the same commit: `SNAPSHOT_MODELS` 1418→1491, `SNAPSHOT_DATE`
2026-08-05→2026-09-16, `SNAPSHOT_PARSED_RATES` 184→193, `SNAPSHOT_UNPUBLISHED` 765→761,
`SNAPSHOT_UNPARSED` 469→537, the nine per-category counts, and `TextTo3d` 11→12. The refusal
rate held at 73%, so the parser did not get looser.

### What the diff reported as missing (pre-import excerpt, the families the slice names)

```
alibaba/wan-3.0-prime/text-to-video          | Wan 3.0 Prime               | $0.068/480p … $0.28/1080p
alibaba/wan-3.0-prime/image-to-video         | Wan 3.0 Prime               | …
fal-ai/kling-video/v3/pro/text-to-video      | Kling Video v3 [Pro]        | $0.112 audio-off / $0.168 audio-on
fal-ai/kling-video/v3/pro/image-to-video     | Kling Video v3 [Pro]        | …
lightricks/ltx-2.5/text-to-video/pro         | Ltx 2.5 Text to Video       | $0.12/s 720p, $0.17/s 1080p
lightricks/ltx-2.5/image-to-video/pro        | LTX 2.5 Image to Video Pro  | …
minimax/h3-max/text-to-video                 | MiniMax H3 Max              | $0.025/480p, $0.04/768p, $0.08/1080p
minimax/h3-max/image-to-video                | H3 Max Image to Video       | …
minimax/h3-max-turbo/{text,image}-to-video   | H3 Max Turbo                | half the H3 Max rate
fal-ai/veo3.1/extend-video                   | Veo 3.1                     | $0.20 audio-off / $0.40 audio-on
openai/gpt-image-2.5/{flare,sunburst}/text-to-image and /edit | GPT Image 2.5 | token-billed
alibaba/qwen-image-3/text-to-image, /edit    | Qwen Image 3                | $0.04 at 1K, $0.075 at 2K
fal-ai/nano-banana-pro/edit                  | Nano Banana Pro             | $0.15 per image
fal-ai/nano-banana-2/edit                    | Nano Banana 2               | $0.08 per image
bytedance/seedream/v5/pro/edit               | Seedream 5.0 Pro            | $0.0675 + $0.0045/extra input
```

`bytedance/seedance-2.5/{text,image}-to-video` did **not** appear as missing — the registry
already carried the root and the resolver expanded it, which is exactly the family-root
false-positive the design's §1.3 step 4 exists to prevent. Only
`…/seedance-2.5/reference-to-video` showed, and that is Non-goal N3.

After the import: registry coverage 53 → 66 fal endpoints, uncovered generation models
1090 → 1077.

---

## 4. AC3 / AC7 — the verification gate

`cargo run -p hickeyfield-core --example audit_fal` (no key, no money). Final run:

```
=== SILENTLY IGNORED MEDIA — bills for the wrong generation (11) ===
=== MISSING REQUIRED FIELDS — 422 after a round trip (2) ===
=== DROPPED SETTINGS — the control has no effect (21) ===
=== REJECTED OPTIONS — the chip row offers invalid values (9) ===
=== UNREACHABLE — fal serves no schema for this endpoint (0) ===
  none
=== CHECKED — fal answered with a schema (66) ===
  …
  gpt_image_2_5_flare          openai/gpt-image-2.5/flare/text-to-image
  gpt_image_2_5_flare_edit     openai/gpt-image-2.5/flare/edit
  gpt_image_2_5_sunburst       openai/gpt-image-2.5/sunburst/text-to-image
  gpt_image_2_5_sunburst_edit  openai/gpt-image-2.5/sunburst/edit
  kling3_0_pro                 fal-ai/kling-video/v3/pro/text-to-video
  ltx_2_5                      lightricks/ltx-2.5/text-to-video/pro
  ltx_2_5_animate              lightricks/ltx-2.5/image-to-video/pro
  nano_banana_2_edit           fal-ai/nano-banana-2/edit
  nano_banana_pro_edit         fal-ai/nano-banana-pro/edit
  qwen_image_3                 alibaba/qwen-image-3/text-to-image
  seedance_2_5                 bytedance/seedance-2.5/text-to-video
  seedance_2_5                 bytedance/seedance-2.5/image-to-video
  seedream_v5_pro_edit         bytedance/seedream/v5/pro/edit
  veo3_1_extend                fal-ai/veo3.1/extend-video
  wan3_0_prime                 alibaba/wan-3.0-prime/text-to-video

66 endpoint(s) audited against fal's own schema. A slug listed under CHECKED and under
nothing else is one this registry can drive.
```

Mechanical check that no imported id appears in any finding section:

```
$ for id in gpt_image_2_5_flare … wan3_0_prime; do echo "$id findings=$(grep -c "^  $id " audit.txt)"; done
gpt_image_2_5_flare findings=0        ltx_2_5_animate findings=0      seedance_2_5 findings=0
gpt_image_2_5_flare_edit findings=0   nano_banana_2_edit findings=0   seedream_v5_pro_edit findings=0
gpt_image_2_5_sunburst findings=0     nano_banana_pro_edit findings=0 veo3_1_extend findings=0
gpt_image_2_5_sunburst_edit findings=0 qwen_image_3 findings=0        wan3_0_prime findings=0
kling3_0_pro findings=0               ltx_2_5 findings=0
```

**Every landed slug: CHECKED, zero findings, zero UNREACHABLE. Gate passed, no exceptions
taken.** The 43 pre-existing findings are the known baseline (they are all on models this
slice did not touch); `UNREACHABLE` is 0 because `resolve_endpoint` returns `Err` for the
three `FAL_MISSING_ROUTES` slugs, so they are skipped before the probe rather than reported.

### The imported models

| Model id | fal slug | Price (sourced from the refreshed index) | `FAL_ROUTE_MODES` | Audit |
|---|---|---|---|---|
| `seedance_2_5` *(promoted)* | `bytedance/seedance-2.5` | `per_token(21.40)` — $0.0214/1k tokens at 480p/720p | root, `[Text, Image]` *(pre-existing)* | CHECKED ×2, clean |
| `kling3_0_pro` | `fal-ai/kling-video/v3/pro` | `per_second_audio(0.112, 1.5)` | root, `[Text]` | CHECKED, clean |
| `wan3_0_prime` | `alibaba/wan-3.0-prime` | tiered 480p $0.068 / 720p $0.14 / 1080p $0.28 | root, `[Text]` | CHECKED, clean |
| `ltx_2_5` | `lightricks/ltx-2.5/text-to-video/pro` | tiered 720p $0.12 / 1080p $0.17 | **Exact**, `[Text]` | CHECKED, clean |
| `ltx_2_5_animate` | `lightricks/ltx-2.5/image-to-video/pro` | same tiers | **Exact**, `[Image]` | CHECKED, clean |
| `veo3_1_extend` | `fal-ai/veo3.1/extend-video` | `per_second_audio(0.20, 2.0)` | **Exact**, `[Video]` | CHECKED, clean |
| `gpt_image_2_5_flare` | `openai/gpt-image-2.5/flare/text-to-image` | `Unknown` (token-billed, `quality` swings it) | **Exact**, `[Text]` | CHECKED, clean |
| `gpt_image_2_5_flare_edit` | `openai/gpt-image-2.5/flare/edit` | `Unknown` | **Exact**, `[Image]` | CHECKED, clean |
| `gpt_image_2_5_sunburst` | `openai/gpt-image-2.5/sunburst/text-to-image` | `Unknown` | **Exact**, `[Text]` | CHECKED, clean |
| `gpt_image_2_5_sunburst_edit` | `openai/gpt-image-2.5/sunburst/edit` | `Unknown` | **Exact**, `[Image]` | CHECKED, clean |
| `qwen_image_3` | `alibaba/qwen-image-3/text-to-image` | `per_image(0.04)` | **Exact**, `[Text]` | CHECKED, clean |
| `nano_banana_pro_edit` | `fal-ai/nano-banana-pro/edit` | `per_image(0.15)` | **Exact**, `[Image]` | CHECKED, clean |
| `nano_banana_2_edit` | `fal-ai/nano-banana-2/edit` | `per_image(0.08)` | **Exact**, `[Image]` | CHECKED, clean |
| `seedream_v5_pro_edit` | `bytedance/seedream/v5/pro/edit` | `PerImage{0.0675, +0.0045/extra input}` | **Exact**, `[Image]` | CHECKED, clean |

Every enumerated flag on every new spec is copied verbatim from that endpoint's OpenAPI,
probed 2026-09-16. `Exact = true` on the vendor-namespaced already-suffixed endpoints, per
the design's rule; the two family roots are the ones whose sub-paths are literally
`InputMode::suffix` output.

## 5. AC4 — duration

fal **enumerates** `duration` for `bytedance/seedance-2.5/*` as
`['auto','4','5',…,'30']`, so design §5's *exception* branch applies and the enum is mirrored
exactly rather than left free-form. `capabilities().durations` therefore contains `30.0` and
the option set cannot be one fal rejects (audit check 4 is clean). New models whose schema
declares `duration` as a bare integer (`wan3_0_prime`, `veo3_1_extend`) are free-form
`ValueSpec::Number`, joining the 28 uncapped models; no existing cap was changed.

```
$ cargo test -p hickeyfield-core seedance_2_5_offers_a_thirty_second_clip
test registry::tests::seedance_2_5_offers_a_thirty_second_clip ... ok
```

## 6. AC5 — tests

Updated, not deleted: `registry_covers_the_catalogue_minus_its_exclusions` (`+17` → `+30`),
`the_twelve_picker_only_models_are_all_present` → `the_picker_only_models_are_all_present`
(renamed per design, 13 ids added, doc comment at `picker_only_specs` rewritten),
`models_the_corpus_could_not_price_stay_unknown` (`seedance_2_5` removed — it has a sourced
price now — and the four GPT Image 2.5 ids added, so the test gained teeth rather than lost
them), `unverified_slugs_are_flagged_in_the_note_the_ui_shows` (`seedance_2_5` removed, plus
a new assertion that it does *not* claim to be unverified). `there_are_twelve_launch_families`
and `the_unroutable_models_are_a_known_short_list` unchanged and still green.

New: `every_imported_model_resolves_to_an_exact_fal_endpoint` (also asserts the resolved
endpoint is one fal's index actually lists), `every_imported_model_has_a_fal_route_with_a_real_adapter`,
`seedance_2_5_offers_a_thirty_second_clip`, `the_models_imported_from_fal_land_on_the_right_tab`
(use_case), `fal_minus_registry_reports_only_endpoints_no_route_reaches` +
`the_category_map_only_claims_the_five_use_cases` (fal_diff).

## 7. AC7 — regression gate (all run, all pasted)

```
$ cargo test --workspace
test result: ok. 657 passed; 0 failed; 9 ignored   (hickeyfield-core lib)
test result: ok.   2 passed; 0 failed              (examples/fal_diff)
test result: ok. 120 passed; 0 failed; 1 ignored   (hickeyfield-tauri-lib)
test result: ok.   0 passed …  (bin + 2 doc-test targets)
```
779 passing, 0 failed — above the 773 baseline.

```
$ cargo fmt --all --check      → clean (FMT_OK)
$ cargo clippy --workspace --all-targets -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 6.56s   (no warnings)
$ ./scripts/lint-provenance.py
CDN hostnames      PASS  no third-party media hosts referenced
Copy provenance    PASS  no shipped string matches the reference corpus (80015 shingles indexed)
$ cd ui && pnpm test
Test Files  14 passed (14)   Tests  232 passed (232)
$ cd ui && pnpm build
✓ built in 941ms
```

`tauri build` not run (release phase). `first_light.rs` not run — nothing here spent money;
`audit_fal` and `fal_diff` are unauthenticated.

---

## 8. Known limitations — what I did **not** land, and why

These are refusals under AC3's own rule ("a slug that fails the audit is not added"), not
oversights.

1. **MiniMax H3 Max and H3 Max Turbo are NOT imported.** This is the biggest gap against the
   slice's minimum set and it is deliberate. fal's schema for all four endpoints
   (`minimax/h3-max{,-turbo}/{text,image}-to-video`) declares
   `required: ["prompt", "prompt_expansion_mode"]`, and `prompt_expansion_mode` has **no
   default** — it is a required enum-ish string (`disabled|balanced|quality`). The app has no
   mechanism to send a constant field: `Inputs::to_provider` only forwards settings the user
   set, and the capability layer surfaces just duration/resolution/aspect/audio. Landing it
   would ship a tile that 422s on every submit, which is precisely what audit check 2 names.
   **Proposed change (not implemented):** a per-route "constant fields" map applied in
   `to_provider`, then H3 Max lands in one follow-up commit. That is a new mechanism, so it
   belongs to a slice, not to a table row.
2. **Kling 3.0 Pro and Wan 3.0 Prime are text-to-video only**, though fal serves both in
   image-to-video. Their i2v endpoints name the start frame `start_image_url`, and
   `media::fal_keys` maps `MediaRole::Start` to `image_url` for every non-VACE slug. The
   submit gate would refuse the attachment ("its endpoint has no `image_url` field"), so
   claiming the mode would hand the user a tile that dies after they chose it. Registering
   `[Text]` keeps the honest half. **Pre-existing defect found in passing:** `kling3_0`
   (`fal-ai/kling-video/v3/standard`) already claims `[Text, Image]` and has the same key
   mismatch — its Animate Image path is dead today. I did not fix it: `fal_keys` is inside
   `bind`, which design N5 puts out of scope.
3. **`minimax_hailuo` left Higgsfield-only; `FAL_MISSING_ROUTES` unchanged at 3.** fal does
   now serve Hailuo 2.3 — but at `fal-ai/minimax/hailuo-2.3/{pro,standard}/{text,image}-to-video`,
   not at the pinned slug `fal-ai/minimax/hailuo-2.3`, which is still unserved (so the pin is
   still factually correct). Re-pointing to `…/hailuo-2.3/pro` was probed and rejected: that
   endpoint's whole input surface is `prompt`, `image_url`, `prompt_optimizer` — no
   `duration`, no `resolution`, no end frame — while the vendored Higgsfield spec offers
   `--duration`, `--resolution` and `--end-image`. It would have landed three DROPPED SETTINGS
   findings and a refused end-frame attach. Fixing it means a hand-authored fal-side spec
   (i.e. a new model id), which is an import, not a re-point. Same reasoning applies to
   `fal-ai/kling-video/v3/turbo`: fal serves `…/v3/turbo/pro/…` and `…/v3/turbo/standard/…`,
   which are different endpoints, so the pin stands.
4. **`alibaba/qwen-image-3/edit` is not imported.** The endpoint is fine (probed, clean), but
   every image job type that defaults the enhancer **off** is named for a specific product
   family (`image-nano-banana*`, `image-gpt-image-2`, `image-seedream`). Filing a Qwen edit
   under any of them is a lie, and filing it under `image-styled` would default prompt
   enhancement **on** for an instruction-following edit — the exact thing the `animate`
   comment in `JOB_TYPES` warns against. It needs a `JobType` variant, which touches
   `enhance.rs` and the bridge. Deferred with the reason recorded.
5. Not imported, and out of scope by design N3: the `reference-to-video`, `camera-controls`,
   `director`, `motion-control` and `/lora` endpoints of every family; LTX 2.5's `fast` tier;
   Seedance 2.5's `reference-to-video`. 1,077 fal generation endpoints remain outside the
   registry — the diff tool now says so out loud, which is the point of AC1.
6. **Design citation corrected.** The design asked me to cite commits `3d238b8` / `0291e23` /
   `d075307` (2026-08-28) as the resolver's provenance. `git log -S` says otherwise: those
   three are the end-frame-key and Higgsfield-host fixes. `resolve_endpoint` and
   `FAL_ROUTE_MODES` were both introduced in **`ab06e31` (2026-08-05)** and extended to
   Higgsfield's mirror in **`8d8ecfb` (2026-08-28)**. The docs cite what git confirms.
7. **`cargo deny check` not run** — it is in the design's §4 list but not in this slice's
   run instruction, and no dependency was added or changed (the only `Cargo.toml` edit is an
   `[[example]]` target).
8. The `--dump` retired-row guard and the `CatalogueError` exit path are code-reviewed, not
   exercised: fal returned a clean 1,491-row index with zero retired rows and no WAF
   challenge on the day.


---
## ORCHESTRATOR ADDENDUM (post-REVIEW repair, 2026-09-16)
REVIEW (FAIL) proved 3 landed models cannot generate: `ltx_2_5`, `ltx_2_5_animate` (fal duration
enum {6,8,10,"auto"}; `fal_schema::parse` keeps only string members -> ["auto"] -> every submit
refused) and `veo3_1_extend` (fal pins duration/resolution by `const`; parser ignores const -> 422).
Root cause is the schema parser + audit blind spot, logged as its own slice
("fal-schema-const-and-int-enums", backlog). Per the slice's own AC3 rule ("a failing slug is NOT
landed"), the orchestrator REMOVED these 3 from the registry/JOB_TYPES/FAL_ROUTE_MODES (51->48)/
use_case tests/IMPORTED_2026_09 (14->11) and the CHANGELOG. 11 verified models remain. No agent
was spawned for this repair (owner constraint); a fresh REVIEW+TEST pass over the reduced set is
required before gate/merge.
