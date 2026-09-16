# S8-fal-catalogue-sync — ADVERSARIAL TEST

Verifier did not write the code. Every number below was re-measured in this session; nothing
in `implement.md` was taken on trust. Working tree at verification: branch
`forge/s8-fal-catalogue-sync`, uncommitted. Tree left as found (one scratch copy of the tree
was mutated outside the repo and deleted; `git status` re-checked at the end).

Environment: `source "$HOME/.cargo/env"`; Node from the session scratchpad; dummy key
overrides exported. **No real fal key exists anywhere in this session** — `audit_fal` was run
with `FAL_KEY`/`hickeyfield_FAL_KEY` explicitly unset via `env -u`. `first_light.rs` was never
run. Nothing here spent money.

---

## 1. The claim, quoted verbatim

From `implement.md` §4:

> **Every landed slug: CHECKED, zero findings, zero UNREACHABLE. Gate passed, no exceptions
> taken.** The 43 pre-existing findings are the known baseline (they are all on models this
> slice did not touch); `UNREACHABLE` is 0 because `resolve_endpoint` returns `Err` for the
> three `FAL_MISSING_ROUTES` slugs, so they are skipped before the probe rather than reported.

From `implement.md` §7:

> 779 passing, 0 failed — above the 773 baseline.

From `implement.md` §8.1:

> **MiniMax H3 Max and H3 Max Turbo are NOT imported.** This is the biggest gap against the
> slice's minimum set and it is deliberate. fal's schema for all four endpoints
> (`minimax/h3-max{,-turbo}/{text,image}-to-video`) declares
> `required: ["prompt", "prompt_expansion_mode"]`, and `prompt_expansion_mode` has **no
> default**

---

## Commands

### 1.1 Scope — `git diff --stat`

```
$ git diff --stat
 .forge/backlog.md                                  |     2 +-
 .forge/state.json                                  |     8 +-
 CHANGELOG.md                                       |    11 +
 crates/hickeyfield-core/Cargo.toml                 |     8 +
 crates/hickeyfield-core/examples/audit_fal.rs      |    20 +-
 crates/hickeyfield-core/src/fal_catalogue.rs       |    71 +-
 crates/hickeyfield-core/src/media.rs               |    51 +-
 crates/hickeyfield-core/src/registry.rs            |   670 +-
 crates/hickeyfield-core/src/use_case.rs            |    39 +
 .../vendor/fal-catalogue-snapshot.json             | 17825 +++++++++++++++++--
 docs/FIRST-LIGHT.md                                |    23 +-
 11 files changed, 17242 insertions(+), 1486 deletions(-)
$ git status --porcelain | grep '^??'
?? .forge/slices/S8-fal-catalogue-sync/
?? crates/hickeyfield-core/examples/fal_diff.rs
```

Matches `implement.md` §1 exactly. **No `Cargo.lock` change. No dependency change** — the only
`Cargo.toml` edit is an `[[example]] name = "fal_diff", test = true` target (read in full; no
`[dependencies]` line touched). **No provider, vault, `bind`, `reconcile`, `route::resolve`,
`enhance` or `ui/` source change.** `MODELS.md` untouched.

Two out-of-scope files are dirty and are *not* the implementer's: `.forge/state.json`
(`currentSlice` bump + an S8 row) and `.forge/STATUS.md` (appeared dirty only after this
verification session opened — forge orchestration, not slice source). Neither is an AC.

### 1.2 AC1 — the diff tool

```
$ cargo run -q -p hickeyfield-core --example fal_diff -- --offline | head -1
fal index: 1491 model(s), bundled snapshot of 2026-09-16. Registry reaches 66 fal endpoint(s).
... (tail)
1077 of 1138 generation model(s) are outside the registry; 353 model(s) in other categories
(training, audio, vision, …) are not shown.
$ echo exit=$?
exit=0
```

Fixture tests, run as their own target:

```
$ cargo test -q -p hickeyfield-core --example fal_diff
running 2 tests
..
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

and inside the workspace gate (§1.9 below shows `Running unittests examples/fal_diff.rs …
2 passed`), which is what the `test = true` target buys.

**Exact-slug + family-root-expansion check (I crafted this, the implementer did not).** I
grepped the report for slugs whose presence/absence proves the expansion is real rather than
asserted:

```
covered/not-reported  : bytedance/seedance-2.5/text-to-video      <- family root expanded
covered/not-reported  : bytedance/seedance-2.5/image-to-video     <- family root expanded
MISSING-FROM-REGISTRY : bytedance/seedance-2.5/reference-to-video <- not an InputMode suffix
MISSING-FROM-REGISTRY : minimax/h3-max/text-to-video
MISSING-FROM-REGISTRY : minimax/h3-max/image-to-video
covered/not-reported  : fal-ai/kling-video/v3/pro/text-to-video   <- root, [Text] only
MISSING-FROM-REGISTRY : fal-ai/kling-video/v3/pro/image-to-video  <- mode deliberately unclaimed
covered/not-reported  : alibaba/wan-3.0-prime/text-to-video
MISSING-FROM-REGISTRY : alibaba/wan-3.0-prime/image-to-video
covered/not-reported  : fal-ai/kling-video/v3/standard/text-to-video
covered/not-reported  : lightricks/ltx-2.5/text-to-video/pro      <- Exact=true endpoint
covered/not-reported  : fal-ai/veo3.1/extend-video
MISSING-FROM-REGISTRY : alibaba/qwen-image-3/edit                 <- honest gap #4
```

This is exactly the behaviour AC1 demands: a family root is expanded through
`media::resolve_endpoint` and is therefore **not** falsely reported missing, while a sibling
endpoint no `InputMode` can reach (`/pro/image-to-video`, `/reference-to-video`) still shows.
Confirmed in source: `fal_diff.rs::covered_endpoints()` iterates
`{Text, Image, Video} × resolve_endpoint`, and `fal_minus_registry` compares
`covered.contains(&m.id)` — full-string equality, never a prefix.

`--dump` exists (`fal_diff.rs:41,65-99`) and the `fal_catalogue.rs` module doc now reads
`cargo run -p hickeyfield-core --example fal_diff -- --dump > …`. `grep -rn dump_fal_catalogue`
over the source tree returns nothing outside the slice's own markdown. The non-existent-example
doc defect is closed.

### 1.3 AC2 — snapshot

```
$ python3 …  vendor/fal-catalogue-snapshot.json
date 2026-09-16 items 1491
deprecated 0 removed 0
OK   bytedance/seedance-2.5/text-to-video
OK   minimax/h3-max/text-to-video
(+18 further landed/named slugs, all present)
```

I did not stop at the file. I re-ran the live dump myself and byte-compared the id sets:

```
$ ./target/debug/examples/fal_diff --dump > scratch/dumplive.json
LIVE snapshot_date= 2026-09-16 items= 1491
vendored items= 1491
live-only= 0   vendor-only= 0
```

The vendored snapshot is a genuine live capture of fal's index, reproducible today, with zero
drift. AC2 holds.

### 1.4 AC3 — the audit gate, re-run keyless by me

```
$ env -u hickeyfield_FAL_KEY -u FAL_KEY cargo run -q -p hickeyfield-core --example audit_fal
exit=0
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

All **14 landed ids appear under CHECKED** (16 endpoint rows — `seedance_2_5` twice,
`grok_video_v15`-style dual modes). **UNREACHABLE = 0.** I read all four finding sections in
full (reproduced in §2.1 below): **not one of the 14 landed ids appears in any of them.** The
43 findings are entirely on `flux_2`, `gemini_omni`, `gpt_image_2`, `grok_image`,
`nano_banana_2`, `nano_banana_2_lite`, `nano_banana_flash`, `seedream_v5_pro`, `veo3_1{,_fast,
_lite}`, `inworld_text_to_speech`, `mirelo_text_to_audio`, `kling*`, `recraft_v4_1`,
`seedance_2_0*`, `wan2_7` — all pre-existing, none touched by this slice.

The previously-silent `unreachable` bucket now prints: `audit_fal.rs` diff shows the new
`section("UNREACHABLE — …", &unreachable)` call and `checked` promoted from `usize` to
`Vec<String>`. That was a real defect (the signal was computed at `:62` and discarded) and it
is closed.

**MiniMax H3 Max is genuinely absent, not half-landed.** `grep -rn h3_max crates/ src-tauri/`
finds nothing; the registry, `JOB_TYPES`, `FAL_ROUTE_MODES` and `priced_routes` carry no
`minimax_h3_max*` id; and the diff tool reports `minimax/h3-max/{text,image}-to-video` as
missing from the registry (§1.2). No dangling half-import.

### 1.5 AC4 — duration

```
seedance_2_5 durations=[4,5,6,…,29,30]  supports_duration=true
veo3_1       durations=[4, 6, 8]
minimax_hailuo durations=[6, 10]
wan2_6       durations=[5, 10, 15]
```
(measured through `ModelSpec::capabilities()` from an out-of-tree probe crate)

`git diff registry.rs | grep '^-.*"duration"'` → **no existing duration flag was removed or
edited.** `wan3_0_prime` / `veo3_1_extend` / `ltx_2_5*` declare `ValueSpec::Number`
(free-form), joining the uncapped set; no in-app ceiling added or removed.

### 1.6 AC5 — tests updated, not deleted

Each run individually, all green:

```
there_are_twelve_launch_families                          1 passed   (unchanged)
the_unroutable_models_are_a_known_short_list              1 passed   (unchanged)
every_exact_endpoint_is_a_route_the_registry_actually_has 1 passed   (FAL_ROUTE_MODES iteration)
the_picker_only_models_are_all_present                    1 passed   (renamed, 13 ids added)
registry_covers_the_catalogue_minus_its_exclusions        1 passed   (+17 -> +30)
models_the_corpus_could_not_price_stay_unknown            1 passed   (lost seedance_2_5, gained 4)
unverified_slugs_are_flagged_in_the_note_the_ui_shows     1 passed   (gained a negative assertion)
every_imported_model_resolves_to_an_exact_fal_endpoint    1 passed   (new)
every_imported_model_has_a_fal_route_with_a_real_adapter  1 passed   (new)
seedance_2_5_offers_a_thirty_second_clip                  1 passed   (new)
the_models_imported_from_fal_land_on_the_right_tab        1 passed   (new, use_case)
$ git diff registry.rs | grep -E '^-.*fn (there_are_twelve|the_unroutable|every_exact_endpoint)'
(none deleted)
```

### 1.7 AC6 — docs

```
$ git log --format='%h %ad %s' --date=short -S resolve_endpoint --all -- '*media.rs'
8d8ecfb 2026-08-28 Reach fal-mirrored models on the user's own Higgsfield key
ab06e31 2026-08-05 WIP snapshot before machine wipe (2026-08-05)
$ git grep -l "fn resolve_endpoint" ab06e31    -> ab06e31:crates/halation-core/src/media.rs
$ git grep -l "fn resolve_endpoint" ab06e31^   -> (absent in parent)
$ git grep -l "FAL_ROUTE_MODES"    ab06e31^   -> (absent in parent)
$ git log -1 --date=short --format='%h %ad %s' 3d238b8
3d238b8 2026-08-28 Refuse an end frame no endpoint can hold, and grey the slot before it is filled
$ git log -1 --date=short --format='%h %ad %s' 0291e23
0291e23 2026-08-28 Send the end frame under the key fal declares, and never drop a file quietly
$ git log -1 --date=short --format='%h %ad %s' d075307
d075307 2026-08-28 Post to the host Higgsfield documents, and stop guessing why they refuse
```

The implementer's correction is **confirmed**: `ab06e31` (2026-08-05) introduced both
`resolve_endpoint` and `FAL_ROUTE_MODES` (under the pre-rename crate path, which is why a
path-scoped `git log -S` on `crates/hickeyfield-core/src/media.rs` alone misses it);
`8d8ecfb` extended them. The design's three cited commits are a different fix. The docs cite
what git confirms.

`docs/FIRST-LIGHT.md` §"input-mode suffix" retitled `(resolved)` with a Resolved note citing
`ab06e31`/`8d8ecfb`; Consequence 1 `(done)`, Consequence 2 answered via `FAL_MISSING_ROUTES`
+ a 2026-09-16 re-probe, Consequence 3 left open. `.forge/backlog.md` #4 bullet 1 is `[x]`,
bullet 2 still `[ ]`. `CHANGELOG.md` has one `[Unreleased] › Added` bullet, and — checked
against what actually landed — it does **not** claim MiniMax H3 Max. No user-facing overclaim.

```
$ ./scripts/lint-provenance.py
CDN hostnames      PASS  no third-party media hosts referenced
Copy provenance    PASS  no shipped string matches the reference corpus (80015 shingles indexed)
prov_exit=0
```

### 1.8 AC7 — regression gate

```
$ cargo test --workspace
     Running unittests src/lib.rs (hickeyfield_core)
running 666 tests
test result: ok. 657 passed; 0 failed; 9 ignored
     Running unittests examples/fal_diff.rs
running 2 tests
test result: ok. 2 passed; 0 failed
     Running unittests src/lib.rs (hickeyfield_tauri_lib)
running 121 tests
test result: ok. 120 passed; 0 failed; 1 ignored
     Running unittests src/main.rs      -> 0 passed
   Doc-tests hickeyfield_core            -> 0 passed
   Doc-tests hickeyfield_tauri_lib       -> 0 passed
```
**779 passed, 0 failed** (657 + 2 + 120). Baseline 773. Claim confirmed to the test.

```
$ cargo fmt --all --check           -> FMT_OK, exit 0
$ cargo clippy --workspace --all-targets -- -D warnings
    Finished `dev` profile … in 1.10s        (no warnings, exit 0)
$ ./scripts/lint-provenance.py      -> both PASS, exit 0
$ cd ui && pnpm test
 Test Files  14 passed (14)
      Tests  232 passed (232)
$ cd ui && pnpm build
dist/assets/index-IklELJ0D.js   281.43 kB │ gzip: 88.29 kB
✓ built in 916ms
```

`cargo test --workspace` compiles the `hickeyfield-tauri` binary target, so the app builds. I
did **not** run `tauri build` or launch the packaged `.app` (release-phase, and not something
this verifier can assert from pasted output). `cargo deny check` not run — no dependency
changed.

---

## 2. Break attempts

Each is an attempt to refute the claim, with the result.

### 2.1 "Are the landed slugs really clean, or did the CHECKED list just hide findings?"

Read all 43 findings in full and cross-referenced against the 14 landed ids. **Not refuted.**
Verbatim finding sections:

```
SILENTLY IGNORED MEDIA (11): flux_2, gemini_omni, gpt_image_2, grok_image, nano_banana_2,
  nano_banana_2_lite, nano_banana_flash, seedream_v5_pro, veo3_1, veo3_1_fast, veo3_1_lite
MISSING REQUIRED FIELDS (2): inworld_text_to_speech, mirelo_text_to_audio
DROPPED SETTINGS (21): flux_2 x2, gemini_omni, gpt_image_2 x2, kling-omni-flf,
  kling-v2-5-turbo x3, kling2_6, kling3_0 x3, kling_o3_flf, minimax_h3, nano_banana_2_lite,
  recraft_v4_1 x2, seedream_v5_lite, seedream_v5_pro, wan2_7
REJECTED OPTIONS (9): grok_image, nano_banana_2, nano_banana_flash, seedance_2_0_fast x2,
  veo3_1, veo3_1_fast, veo3_1_lite x2
```
Zero overlap with `{seedance_2_5, kling3_0_pro, wan3_0_prime, ltx_2_5, ltx_2_5_animate,
veo3_1_extend, gpt_image_2_5_flare, gpt_image_2_5_sunburst, gpt_image_2_5_flare_edit,
gpt_image_2_5_sunburst_edit, qwen_image_3, nano_banana_pro_edit, nano_banana_2_edit,
seedream_v5_pro_edit}`.

### 2.2 "Is the H3 Max refusal a real schema fact or a convenient excuse?"

Probed fal's OpenAPI myself, keyless. **Not refuted — the refusal is factually correct.**

```
$ curl -s "https://fal.ai/api/openapi/queue/openapi.json?endpoint_id=minimax/h3-max/text-to-video"
 H3MaxTextToVideoInput   required= ['prompt', 'prompt_expansion_mode']
   prompt_expansion_mode -> {"title":"Prompt Expansion Mode","examples":["disabled","balanced",
     "quality"],"type":"string", …}       <- examples, NOT default. No default key.
 H3MaxImageToVideoInput      required= ['prompt', 'prompt_expansion_mode']
 H3MaxTurboTextToVideoInput  required= ['prompt', 'prompt_expansion_mode']
```
`spec()` sets `default: None` and `Inputs::to_provider` forwards only user-set settings, and
the capability layer surfaces only duration/resolution/aspect/audio — so there is no path by
which the app could send `prompt_expansion_mode`. Had H3 Max landed, `audit_fal` check 2 would
have reported "requires [prompt_expansion_mode] which nothing in the app supplies" and AC3's
own rule ("Any slug that fails audit_fal is NOT added") forbids landing it. **Recorded as a
scope shortfall against the slice's *headline*, not an AC3 violation** — see §4.

### 2.3 "Kling 3.0 Pro / Wan 3.0 Prime t2v-only — real constraint or laziness?"

Probed the i2v endpoints and read `media::fal_keys`. **Not refuted.**

```
fal-ai/kling-video/v3/pro/image-to-video  media keys ['start_image_url','end_image_url'] req ['start_image_url']
alibaba/wan-3.0-prime/image-to-video      media keys ['start_image_url','end_image_url'] req ['start_image_url']
media.rs:189  fal_keys(MediaRole::Start, non-vace) -> &["image_url"]
```
The binder cannot write `start_image_url`. Restricting both to `[Text]` is correct and the
`the_models_imported_from_fal_land_on_the_right_tab` test pins it negatively.

By contrast, I verified the modes that *were* claimed do bind:

```
lightricks/ltx-2.5/image-to-video/pro  ['image_url','end_image_url']  req ['prompt','image_url']  OK
bytedance/seedance-2.5/image-to-video  ['image_url','end_image_url']  req ['prompt','image_url']  OK
fal-ai/nano-banana-pro/edit            ['image_urls']  req ['prompt','image_urls']                OK (Reference -> image_urls)
fal-ai/nano-banana-2/edit              ['image_urls', …]                                          OK
bytedance/seedream/v5/pro/edit         ['image_urls']  req ['prompt','image_urls']                OK
openai/gpt-image-2.5/{flare,sunburst}/edit ['image_urls','mask_url'] req ['prompt','image_urls']  OK
fal-ai/veo3.1/extend-video             ['video_url']   req ['prompt','video_url']                 OK (Video -> video_url)
```
Every landed media-taking model binds on a key the app actually writes. This check is
*stronger* than `audit_fal`, which has a blind spot here (see §4, finding F3).

### 2.4 "Is Seedance 2.5's 30s claim real, or an invented enum?"

```
$ curl … endpoint_id=bytedance/seedance-2.5/text-to-video
 Seedance25TextToVideoInput required=['prompt']
   duration -> {"default":"auto","enum":["auto","4","5",…,"29","30"],"type":"string", …}
   resolution -> {"default":"720p","enum":["480p","720p","1080p"]}
   aspect_ratio -> {"default":"auto","enum":["auto","21:9","16:9","4:3","1:1","3:4","9:16"]}
```
**Not refuted.** The registry's enum is fal's enum value-for-value; `capabilities().durations`
reaches 30.0; `audit_fal` check 4 (rejected options) is silent for it, which it would not be
if the enum had been invented.

### 2.5 "Are the new tests vacuous?" — mutation testing

Copied the working tree into the scratchpad (never the repo), mutated, ran, restored, deleted.
**Every mutation was caught:**

| Mutation | Test | Result |
|---|---|---|
| Drop `"30"` from the Seedance duration enum | `seedance_2_5_offers_a_thirty_second_clip` | **FAILED** (registry.rs:2996) |
| Re-point `ltx_2_5` at `…/text-to-video/ultra` (+ matching `FAL_ROUTE_MODES` entry) | `every_imported_model_resolves_to_an_exact_fal_endpoint` | **FAILED** (registry.rs:2961 — "not in fal's index") |
| Add `InputMode::Image` to `fal-ai/kling-video/v3/pro` | `the_models_imported_from_fal_land_on_the_right_tab` | **FAILED** (`assertion failed: !supports(&reg["kling3_0_pro"], UseCase::ImageToVideo)`) |
| Put the word `unverified` back in `seedance_2_5`'s note | `unverified_slugs_are_flagged_in_the_note_the_ui_shows` | **FAILED** (registry.rs:3112) |
| control: unmutated copy | `every_imported_model_resolves_to_an_exact_fal_endpoint` | ok, 1 passed |

Residual weakness recorded honestly: `seedance_2_5_offers_a_thirty_second_clip` accepts
`durations.is_empty()` as a pass, so replacing the enum with `ValueSpec::Number` would pass
vacuously. That is the design's explicit §6 instruction ("written to accept both"), not drift.

### 2.6 CLI edge cases on `fal_diff`

| Attempt | Result |
|---|---|
| `fal_diff --bogus` | `unknown argument --bogus; expected --dump and/or --offline`, **exit 2** |
| `fal_diff --help` | same message, exit 2 — **no `--help`**; usage lives only in the module doc-comment. Minor. |
| run from `/` (wrong cwd), `--offline` | works — snapshot is `include_str!`'d. exit 0 |
| `--offline` twice, byte-compare | **IDENTICAL** (deterministic despite the `HashMap` bucket store; order comes from `UseCase::ALL`) |
| `--dump` live, compare id-set to the vendored file | 1491 vs 1491, **0 live-only, 0 vendor-only** |
| `--dump --offline` together | **Accepted.** Re-serialises the *bundled* snapshot stamped with *today's* date. Harmless today (today == snapshot date) but a future footgun: it can forge freshness onto a stale file. Finding F4. |
| piping into `head` | broken-pipe panic message on stderr. Cosmetic, stock Rust behaviour, pre-existing style across the repo's examples. |
| key leakage | `fal_diff` imports no vault/key symbol; `audit_fal` was run with `FAL_KEY` and `hickeyfield_FAL_KEY` unset and still returned 66 schemas. Both are genuinely unauthenticated. |

### 2.7 The specific thing the implementer seemed least sure of — **prices**

The implementer flagged `per_token(21.40)` for `seedance_2_5` with a 9% caveat. I attacked it
two ways: against fal's live model page, and against the app's own estimator through an
out-of-tree probe crate.

---

## 3. Price table (AC3)

`fal stated` is (a) the snapshot's `pricingInfoOverride` **and** (b) `curl https://fal.ai/models/<slug>`
live this session. The two agreed on every row, so the snapshot is not stale.

| Model | App `CostModel` | fal stated price | Verdict |
|---|---|---|---|
| `seedance_2_5` | `PerToken{21.40/M, fps 24}` = $0.0214/1k | *"You are charged **$0.0214 per 1000 tokens** at both 480p and 720p"*; *"roughly $0.0234 per 1000 tokens for 1080p"*; *"tokens = (output_height × output_width × duration_seconds × 24) / 1024"* | **MATCH** at 480p/720p. The app's `cost.rs:207` formula `(h*w*f*secs)/1024` is fal's formula character-for-character, and 21.40/M *is* $0.0214/1k. **MISMATCH ~8.5% low at 1080p** (fal's higher token tier is inexpressible), disclosed verbatim in the user-visible route note. Sanity: app $2.3112 for 5s@720p vs fal's own "$0.4730/s" → $2.365 (−2.3%). The per-token figure is **real**, and `Billable` **does** compute tokens for a video job. |
| `kling3_0_pro` | `PerSecond{0.112, audio ×1.5}` | $0.112 audio-off / $0.168 audio-on / $0.196 voice-control | **MATCH** (0.112×1.5 = 0.168 exactly). Voice-control tier not modelled; disclosed in the spec constraint. |
| `wan3_0_prime` | `PerSecondTiered{480:0.068, 720:0.14, 1080:0.28}` | $0.068 @480p, $0.14 @720p, $0.28 @1080p | **MATCH**, all three tiers |
| `ltx_2_5` | `PerSecondTiered{720:0.12, 1080:0.17}` | $0.12/s 720p, $0.17/s 1080p | **MATCH**. fal also lists a 4K tier ($0.30/s), which the spec's `resolution` enum `["720p","1080p"]` makes unselectable — so no wrong number is reachable. |
| `ltx_2_5_animate` | same tiers | same | **MATCH** |
| `veo3_1_extend` | `PerSecond{0.20, audio ×2.0}` | $0.20 audio-off / $0.40 audio-on | **MATCH** (0.20×2 = 0.40 exactly) |
| `gpt_image_2_5_flare` | `Unknown` | token-billed: $8/$30 per M image tokens, *"Changing the **quality** parameter significantly affects cost"* | **MATCH** (honest refusal). Verified `estimate(...) == None` — the UI renders "price unavailable", never $0.00. |
| `gpt_image_2_5_flare_edit` | `Unknown` | same | **MATCH**, `estimate == None` |
| `gpt_image_2_5_sunburst` | `Unknown` | same | **MATCH**, `estimate == None` |
| `gpt_image_2_5_sunburst_edit` | `Unknown` | same | **MATCH**, `estimate == None` |
| `qwen_image_3` | `PerImage{0.04}` | $0.04 @1K, $0.075 @2K | **MATCH at every reachable setting.** fal's schema for this endpoint has **no `resolution` field** (shape is `image_size`, default `square_hd` = 1024²), so the capability layer exposes no resolution axis and 2K is not selectable in-app. |
| `nano_banana_pro_edit` | `PerImage{0.15}` | $0.15/image; *"4K outputs will be charged at double the standard rate"*; +$0.015 web search | **MATCH at 1K/2K; MISMATCH ×2 at 4K** — the spec exposes `resolution ["1K","2K","4K"]`, so a user can pick 4K and see $0.15 where fal bills $0.30. See finding F1. |
| `nano_banana_2_edit` | `PerImage{0.08}` | $0.08/image; 2K ×1.5, 4K ×2, 0.5K ×0.75 | **MATCH at 1K; MISMATCH at 0.5K (−25% over-quote), 2K (−33% under), 4K (−50% under)** — same exposed resolution axis. See finding F1. |
| `seedream_v5_pro_edit` | `PerImage{0.0675, +0.0045/extra input}` | $0.0675 ≤1536², $0.135 above; +$0.0045 per extra input image | **MATCH at every reachable setting.** `image_size` is free text with no resolution axis; the >1536² tier is not selectable. Extra-input surcharge modelled exactly. |

Two verified negatives worth stating: the app's per-image base rates are **fal's own published
base rates, copied verbatim**, per design §3.1(2); and none of the four `Unknown` GPT Image 2.5
routes can render $0.00 (`estimate()` returns `None`, pinned by
`models_the_corpus_could_not_price_stay_unknown`).

---

## 4. Findings

**F1 — `nano_banana_pro_edit` / `nano_banana_2_edit` under-quote at non-default resolutions.**
Severity: real, user-reachable wrong USD on the Generate button (up to 2× low at 4K).
*Why this is a recorded note and not a FAIL:* it is the **pre-existing, already-shipped house
treatment of the identical sibling endpoints** — `nano_banana_2` → `fal-ai/nano-banana-pro`
`per_image(0.15)` noted *"4K doubles"*, and `nano_banana_flash` → `fal-ai/nano-banana-2`
`per_image(0.08)` noted *"2K is 1.5x, 4K is 2x, 0.5K is 0.75x"* (registry.rs, untouched by this
slice). `CostModel` has `PerSecondTiered` but **no per-image tiered variant**, so expressing it
requires an estimator change, which design N5 puts explicitly out of scope for S8. The gap is
disclosed twice at the UI layer (route note + spec constraint string). Failing S8 for it would
require failing every already-released model on the same code path. **→ backlog: add
`CostModel::PerImageTiered` and re-price the six affected nano-banana routes.**
Credit where due: the new `_edit` specs use fal's exact casing `["1K","2K","4K"]`, so they
avoid the `REJECTED OPTIONS` bug (`offers [1k,2k,4k] … it takes [1K,2K,4K]`) that the two
pre-existing siblings still carry.

**F2 — `PerToken` renders `$0.00` rather than "unavailable" when width is unknown.**
`cost.rs:199-221` uses `b.width.unwrap_or(0)`, so `tokens = 0` and the estimate is
`Some(0.0000)` — unlike `PerSecondTiered`, which correctly returns `None`. Measured:
`seedance_2_5` + `Billable{seconds: 5, width: None}` → `$0.0000 "0x720 @24fps for 5s = 0 tokens"`.
Reachable because fal's `aspect_ratio` enum for this endpoint includes `"auto"`, which
`commands.rs::width_for` cannot parse. **Pre-existing and systemic**, not S8's doing: I
confirmed `bytedance/seedance-2.0/text-to-video` (already shipped on `per_token(14.00)`)
declares the identical `"auto"` aspect *as its default*. The UI's `DEFAULT_SETTINGS`
(`App.tsx:66-73`) hard-codes `aspect: "16:9"`, `resolution: "1080p"`, so the default path is
priced; only a deliberate "auto" selection hits it. S8 does move `seedance_2_5` from a safe
`Unknown` into this hazard class. **→ backlog: `PerToken` should return `None` on unknown
dimensions, as `PerSecondTiered` already does.**

**F3 — `audit_fal` cannot see role-key mismatches.** The gate's check 1 only fires when the
endpoint takes *no* media key at all; it does not compare the app's `fal_keys` spelling against
the endpoint's. That is exactly why `kling3_0` (`fal-ai/kling-video/v3/standard`) sits under
CHECKED with only three `DROPPED SETTINGS` rows while its **Animate-Image path is dead today**:
```
fal-ai/kling-video/v3/standard/image-to-video  media keys ['start_image_url','end_image_url']  req ['start_image_url']
media.rs:189  fal_keys(Start, non-vace) -> ["image_url"]
```
**Confirmed real, confirmed pre-existing, confirmed NOT fixed here** (correct — `fal_keys` is
inside `bind`, design N5). The implementer reported it unprompted in `implement.md` §8.2, which
is the honest behaviour. **→ backlog: (a) fix `kling3_0`'s dead i2v path; (b) add a fifth
`audit_fal` check comparing `fal_keys(role, slug)` against the endpoint's declared media keys —
without it, "CHECKED with zero findings" does not prove bindability, and this slice's safety
rested on the implementer doing that comparison by hand.** I re-did that comparison
independently for all 14 landed models (§2.3) and every one binds.

**F4 — `fal_diff --dump --offline` re-stamps the bundled snapshot with today's date** without
fetching, which can forge freshness onto a stale file. Harmless today; the argument parser
documents "and/or", so it is a designed-in combination rather than a slip. **→ backlog: make
`--dump` reject `--offline`.**

**F5 — `fal_diff` has no `--help`.** `--help` exits 2 with the usage string, which is
serviceable but not conventional. Cosmetic.

**F6 — `seedance_2_5` is ~8.5% low at 1080p**, which is the UI's *default* resolution. Real,
disclosed in the route note the UI shows, and within fal's own "roughly" language; the chosen
`PerToken` shape is the design's explicit §3.1(2) instruction for the Seedance family and is
*more* accurate than a per-second rate for non-16:9 aspects. Note, not a fail.

---

## 5. Per-AC verdict

| AC | Verdict | Evidence |
|---|---|---|
| AC1 — reusable diff tool, live + fixture-tested, expands family roots, exact-slug diff, `--dump` fixes the dead doc | **PASS** | §1.2, §2.6. Family-root expansion proved by a check I constructed, not by the implementer's assertion. |
| AC2 — snapshot refreshed & committed | **PASS** | §1.3. Live re-dump reproduces the vendored file with **zero** id drift. |
| AC3 — curated import, each routed + priced + input-moded + passing `audit_fal` | **PASS with a recorded scope shortfall** | §1.4, §2.1-2.3, §3. All 14 landed ids CHECKED, 0 findings, UNREACHABLE=0. MiniMax H3 Max (4 endpoints) refused under AC3's *own* rule, with the schema fact independently verified. |
| AC4 — Seedance 4–30s; Veo 8s / Hailuo 10s / Wan 2.6 15s caps intact; free-form unchanged | **PASS** | §1.5, §2.4. Enum is fal's, verbatim. |
| AC5 — picker tabs correct; count/array-length/pinned tests updated not deleted | **PASS** | §1.6, §2.5. All four mutations caught. |
| AC6 — stale docs corrected with correct commit citations; CHANGELOG; provenance lint | **PASS** | §1.7. Citation independently re-derived from `git grep` on `ab06e31^`. |
| AC7 — full regression gate | **PASS** | §1.8. 779/0, fmt, clippy, provenance, 232 UI tests, UI build. `tauri build`/launch not exercised. |

## 6. Why this is a PASS I can defend

The claim's three load-bearing assertions — UNREACHABLE=0 with all 14 landed slugs CHECKED and
clean, 779 tests green, and H3 Max refused for a real schema reason — were each re-derived from
scratch against fal's live API and my own test runs, not read from `implement.md`. The diff
tool's central mechanism (family-root expansion, exact-slug comparison) was proved by a probe I
wrote. The new tests are non-vacuous under four independent mutations. Every landed model's
media binding was verified against fal's OpenAPI by hand, catching a blind spot in the gate
itself (F3) that the implementer had already declared.

The one place the claim is weaker than its own framing: the slice's **headline deliverable
names MiniMax H3 Max first**, and it did not ship. That is a genuine scope shortfall. It is not
an AC3 violation, because AC3's text explicitly subordinates the minimum set to the audit rule
("Any slug that fails audit_fal is NOT added"), the failure mode is verified real, the
CHANGELOG does not claim it, and the refusal is recorded prominently with a concrete follow-up
design. **The owner should decide whether S8 releases without H3 Max or whether a constant-fields
follow-up is required first** — that is an editorial call above this gate, not a defect.

Findings F1/F2/F3 are each real wrongness, each pre-existing in kind, each outside design N5's
scope, and each disclosed to the user at the route-note layer. None of them is introduced by
this slice's mechanism, and none makes a landed model unrunnable.

Backlog items to carry forward: F1 (`PerImageTiered`), F2 (`PerToken` → `None` on unknown
dimensions), F3 (`kling3_0` dead Animate-Image path + a fifth `audit_fal` media-key check),
F4 (`--dump --offline`), plus the four gaps `implement.md` §8 already declares.

VERDICT: PASS
