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

---

## RE-TEST (reduced 11-model set, 2026-09-16, HEAD 58e981c)

Everything above this line is the earlier PASS over the 14-model set and is kept as history.
It is **stale**: three of those fourteen (`ltx_2_5`, `ltx_2_5_animate`, `veo3_1_extend`) were
withdrawn after REVIEW. This section re-runs every acceptance criterion from scratch against
the reduced set and does not reuse a single figure from it or from `implement.md`.

### 0. The claim under test

From `implement.md`, ORCHESTRATOR ADDENDUM (verbatim):

> ## ORCHESTRATOR ADDENDUM (post-REVIEW repair, 2026-09-16)
> REVIEW (FAIL) proved 3 landed models cannot generate: `ltx_2_5`, `ltx_2_5_animate` (fal duration
> enum {6,8,10,"auto"}; `fal_schema::parse` keeps only string members -> ["auto"] -> every submit
> refused) and `veo3_1_extend` (fal pins duration/resolution by `const`; parser ignores const -> 422).
> Root cause is the schema parser + audit blind spot, logged as its own slice
> ("fal-schema-const-and-int-enums", backlog). Per the slice's own AC3 rule ("a failing slug is NOT
> landed"), the orchestrator REMOVED these 3 from the registry/JOB_TYPES/FAL_ROUTE_MODES (51->48)/
> use_case tests/IMPORTED_2026_09 (14->11) and the CHANGELOG. 11 verified models remain. No agent
> was spawned for this repair (owner constraint); a fresh REVIEW+TEST pass over the reduced set is
> required before gate/merge.

and from `implement.md` §4:

> **Every landed slug: CHECKED, zero findings, zero UNREACHABLE. Gate passed, no exceptions
> taken.**

and from `CHANGELOG.md` (the user-facing claim):

> Each was checked against the provider's published schema before being offered, so a model that
> appears under a tab is one that can actually run there.

The eleven under test, from `registry.rs:2810` (`IMPORTED_2026_09`), read out of the file:

```
$ sed -n '2810,2827p' crates/hickeyfield-core/src/registry.rs
    const IMPORTED_2026_09: [&str; 11] = [
        "gpt_image_2_5_flare",
        "gpt_image_2_5_flare_edit",
        "gpt_image_2_5_sunburst",
        "gpt_image_2_5_sunburst_edit",
        "kling3_0_pro",
        "nano_banana_2_edit",
        "nano_banana_pro_edit",
        "qwen_image_3",
        "seedance_2_5",
        "seedream_v5_pro_edit",
        "wan3_0_prime",
    ];
```

---

## Commands

Environment for every Rust command below:
`source "$HOME/.cargo/env"` + the eight dummy `hickeyfield_*_KEY=x` exports. `audit_fal` was run
with `env -u hickeyfield_FAL_KEY -u FAL_KEY` on top of that. `first_light.rs` was never run and
nothing in this section submits a paid job.

### AC7 — regression gate

```
$ cargo test --workspace
test result: ok. 657 passed; 0 failed; 9 ignored; 0 measured; 0 filtered out; finished in 0.51s
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 120 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 15.13s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
TEST_EXIT=0
```
Summed: **779 passed, 0 failed** — matches the expected figure.

```
$ cargo fmt --all --check
FMT_EXIT=0                      (no output)

$ cargo clippy --workspace --all-targets -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.93s
CLIPPY_EXIT=0

$ ./scripts/lint-provenance.py
CDN hostnames
  PASS  no third-party media hosts referenced
Copy provenance
  PASS  no shipped string matches the reference corpus (80015 shingles indexed)
PROV_EXIT=0
```

UI (Node 24 from the scratchpad toolchain, `cd ui`):

```
$ pnpm test
 Test Files  14 passed (14)
      Tests  232 passed (232)
UITEST_EXIT=0

$ pnpm build
✓ built in 881ms
UIBUILD_EXIT=0
```

`tauri build` / app launch not exercised (release phase), same exception the earlier section took.

### AC1 — the diff tool, offline

```
$ cargo run -q -p hickeyfield-core --example fal_diff -- --offline | head -1
fal index: 1491 model(s), bundled snapshot of 2026-09-16. Registry reaches 63 fal endpoint(s).
...
1080 of 1138 generation model(s) are outside the registry; 353 model(s) in other categories (training, audio, vision, …) are not shown.
DIFF_EXIT=0
```
63, not the 66 `implement.md` §3 recorded — exactly the −3 the withdrawal should produce. The
example's two fixture tests are the `2 passed` line in the workspace run above.

### AC3 / AC7 — `audit_fal`, keyless

```
$ env -u hickeyfield_FAL_KEY -u FAL_KEY cargo run -q -p hickeyfield-core --example audit_fal
=== SILENTLY IGNORED MEDIA — bills for the wrong generation (11) ===
=== MISSING REQUIRED FIELDS — 422 after a round trip (2) ===
=== DROPPED SETTINGS — the control has no effect (21) ===
=== REJECTED OPTIONS — the chip row offers invalid values (9) ===
=== UNREACHABLE — fal serves no schema for this endpoint (0) ===
=== CHECKED — fal answered with a schema (63) ===
AUDIT_EXIT=0
```

Per-imported-id tally, computed by splitting the log at the CHECKED header so a CHECKED line
cannot be miscounted as a finding:

```
gpt_image_2_5_flare         findings=0 checked=1
gpt_image_2_5_flare_edit    findings=0 checked=1
gpt_image_2_5_sunburst      findings=0 checked=1
gpt_image_2_5_sunburst_edit findings=0 checked=1
kling3_0_pro                findings=0 checked=1
nano_banana_2_edit          findings=0 checked=1
nano_banana_pro_edit        findings=0 checked=1
qwen_image_3                findings=0 checked=1
seedance_2_5                findings=0 checked=2
seedream_v5_pro_edit        findings=0 checked=1
wan3_0_prime                findings=0 checked=1

$ grep -c "ltx_2_5\|veo3_1_extend" audit.log
0
```
UNREACHABLE 0, CHECKED 63, none of the eleven in any finding section, the three withdrawn ids
absent entirely. **The gate as written passes.** §"Break attempts" B2 shows why that is not the
same as the models being runnable.

---

## Break attempts

### B1 — is the withdrawal complete?

```
$ grep -rniE 'ltx_2_5|ltx-2\.5|veo3_1_extend|veo3\.1/extend|extend-video' \
    crates/ src-tauri/ ui/ docs/ CHANGELOG.md .forge/backlog.md | grep -v fal-catalogue-snapshot.json
crates/hickeyfield-core/src/registry.rs:928:  "xai/grok-imagine-video/extend-video",
crates/hickeyfield-core/src/registry.rs:2229:  // place, not added). Three probed-but-unrunnable imports (`ltx_2_5`,
crates/hickeyfield-core/src/registry.rs:2230:  // `ltx_2_5_animate`, `veo3_1_extend`) were removed again before
crates/hickeyfield-core/src/media.rs:612:     "xai/grok-imagine-video/extend-video",
crates/hickeyfield-core/src/media.rs:630:  // a verb (`/extend-video`) — so the mode list is the one mode it serves and
crates/hickeyfield-core/src/media.rs:1614:   "xai/grok-imagine-video/extend-video",
.forge/backlog.md:206-207: the follow-up slice naming the three ids
```
No live code, route, mode entry, JOB_TYPES row, use_case test or CHANGELOG bullet survives for
the three. The Grok hits are the pre-existing, unrelated `xai/grok-imagine-video/extend-video`.
The snapshot JSON still lists the LTX/Veo rows, which is correct — it is fal's index, not ours.
**Not broken**, with one cosmetic exception (F2 below).

`git status --porcelain` before I wrote this file: only the untracked
`audit_fal-2026-09-16-reduced.log` that was already there. I mutated nothing in the repo; all
mutation work below ran in an rsync copy under the scratchpad.

### B2 — the defect class that killed the other three, re-run against all 11 (**this breaks it**)

I fetched fal's live OpenAPI keylessly for every endpoint the eleven resolve to and looked for
exactly what killed `veo3_1_extend`: a field the app sends that fal pins with `const`, or a
mixed/integer enum `fal_schema::parse` collapses.

```
$ for s in openai/gpt-image-2.5/flare/text-to-image openai/gpt-image-2.5/flare/edit \
  openai/gpt-image-2.5/sunburst/text-to-image openai/gpt-image-2.5/sunburst/edit \
  fal-ai/kling-video/v3/pro/text-to-video fal-ai/nano-banana-2/edit fal-ai/nano-banana-pro/edit \
  alibaba/qwen-image-3/text-to-image bytedance/seedance-2.5/text-to-video \
  bytedance/seedance-2.5/image-to-video bytedance/seedream/v5/pro/edit \
  alibaba/wan-3.0-prime/text-to-video ; do
    curl -s -o "$f.json" -w '%{http_code} ' "https://fal.ai/api/openapi/queue/openapi.json?endpoint_id=$s"; echo "$s"; done
200 openai/gpt-image-2.5/flare/text-to-image
200 openai/gpt-image-2.5/flare/edit
200 openai/gpt-image-2.5/sunburst/text-to-image
200 openai/gpt-image-2.5/sunburst/edit
200 fal-ai/kling-video/v3/pro/text-to-video
200 fal-ai/nano-banana-2/edit
200 fal-ai/nano-banana-pro/edit
200 alibaba/qwen-image-3/text-to-image
200 bytedance/seedance-2.5/text-to-video
200 bytedance/seedance-2.5/image-to-video
200 bytedance/seedream/v5/pro/edit
200 alibaba/wan-3.0-prime/text-to-video
```

Scanning every input property of all twelve documents for `const`, exactly one hit:

```
$ python3 -c "import json;print(json.dumps(json.load(open('bytedance_seedance-2.5_image-to-video.json'))\
  ['components']['schemas']['Seedance25ImageToVideoInput']['properties']['aspect_ratio'],indent=1))"
{
 "default": "auto",
 "type": "string",
 "title": "Aspect Ratio",
 "description": "The aspect ratio of the generated video. Always \"auto\" for image-to-video",
 "const": "auto"
}
```

The same field on the *text*-to-video sibling is a seven-member string enum:

```
aspect_ratio  req=False default='auto' types=['string'] ENUM=[('string', ['auto','21:9','16:9','4:3','1:1','3:4','9:16'])]
```

The registry declares one `ModelSpec` for `seedance_2_5` covering both, with
`aspect_ratio = en(&["auto","21:9","16:9","4:3","1:1","3:4","9:16"])` (registry.rs ~:1572), and
`media::FAL_ROUTE_MODES` gives `bytedance/seedance-2.5` `&[InputMode::Text, InputMode::Image]`
(media.rs:690), so the model is offered on the Animate tab:

```
$ cargo run -q -p hickeyfield-core --example probe_uc      # scratch copy, uses use_case::supports
seedance_2_5 -> text-to-video
seedance_2_5 -> image-to-video
```

I then ran the app's own submit-time aspect decision, using the real
`capability::for_route` / `fal_schema::for_endpoint` / `AspectPlan::decide` from this HEAD (an
example added only to the scratch copy; the repo was not touched), with `requested = "16:9"`
because that is literally `DEFAULT_SETTINGS.aspect` in `ui/src/App.tsx:69`:

```
$ cargo run -q -p hickeyfield-core --example probe_aspect
--- mode Text -> bytedance/seedance-2.5/text-to-video
  aspect: support=Yes values=["auto", "21:9", "16:9", "4:3", "1:1", "3:4", "9:16"] default=None source=FalEndpoint
  schema.accepts(aspect_ratio)=true
  schema.enums[aspect_ratio]=Some(["auto", "21:9", "16:9", "4:3", "1:1", "3:4", "9:16"])
  AspectPlan::decide -> Sent("16:9") wire=Some("16:9")
--- mode Image -> bytedance/seedance-2.5/image-to-video
  aspect: support=Yes values=[] default=None source=FalEndpoint
  schema.accepts(aspect_ratio)=true
  schema.enums[aspect_ratio]=None
  AspectPlan::decide -> Sent("16:9") wire=Some("16:9")
```

**The app decides to put `aspect_ratio: "16:9"` on the wire to an endpoint whose schema says that
field must be the literal string `"auto"`.** fal generates these documents from pydantic
`Literal[...]`, so a non-matching value is a 422 validation error — the identical failure the
orchestrator withdrew `veo3_1_extend` for. Nothing downstream rescues it, and I checked each
link by reading this HEAD:

- `fal_schema::parse` (fal_schema.rs:213-226) builds `enums` from `prop["enum"]` only; `const`
  is never read, so `schema.enums["aspect_ratio"]` is `None` on the i2v endpoint.
- `capability::fal_axis` (capability.rs:603-619) therefore returns `values = []` with
  `support = Yes` — "free-form axis". `ui/src/components/ChipRow.tsx:96` renders the aspect chip
  only when `capabilities.aspects.length > 0`, so **the user is never shown the control and
  never told it is locked.**
- `ui/src/lib/variants.ts:145-147` (`resolveSettings`) leaves `settings.aspect` untouched when
  `caps.aspects` is empty and `defaultAspect` is null — so the stale `"16:9"` survives the
  switch to image mode instead of being clamped.
- `spec()` in registry.rs sets `default: None` on every flag, so
  `capabilities().default_aspect == None` and there is no `"auto"` fallback to save it.
- `src-tauri/src/app.rs:329-369` asks `schema.accepts("aspect_ratio")` (true — `const` fields are
  still properties, fal_schema.rs:45-47), gets `AspectPlan::Sent("16:9")` and inserts it.
- The later sweep at app.rs:402-432 only drops keys the schema does **not** accept, and
  `fal_schema::reconcile` (fal_schema.rs:146) only rewrites *enumerated* fields. A `const`
  mismatch passes through both untouched.
- `ui/src/App.tsx:332-351` makes it worse, not better: attaching an image auto-snaps the aspect
  to the nearest of `capabilities.aspects`, and `nearestAspect` (ui/src/lib/media.ts:138) skips
  any option that is not `w:h`, so `"auto"` can never be the snapped result.

So the only way `"auto"` reaches the wire is if the user first selects `"auto"` on the
text-to-video chip row and then attaches an image. The default path — open the Animate tab, pick
Seedance 2.5, attach a photo, hit Generate — sends `"16:9"` and 422s.

`audit_fal` reports `seedance_2_5 findings=0 checked=2` because check 4 compares the spec's
option set against `schema.enums`, which is empty here. That is the same audit blind spot the
backlog slice names; it means the clean audit above is **not** evidence that `seedance_2_5` can
run in image mode.

This is not a pre-existing defect the slice merely failed to fix. Before S8 (`b319c5d^`)
`seedance_2_5` carried `CostModel::Unknown` and the route note *"unverified slug — announced
2026-08-01 and not yet carried by any aggregator"*, plus the constraint *"this spec is a
placeholder"*. S8 deleted both, added the seven-member `aspect_ratio` enum, priced it, and
changed `unverified_slugs_are_flagged_in_the_note_the_ui_shows` to assert it does **not** claim
to be unverified. The slice is what turns a tile the UI labelled unverified into one the
CHANGELOG promises "can actually run there".

### B3 — every other imported endpoint, same test

No other `const` anywhere in the twelve documents, and every enum the app mirrors is a
string enum whose members match the spec verbatim:

| id → endpoint | duration | resolution | aspect_ratio | verdict |
|---|---|---|---|---|
| `kling3_0_pro` → `…/v3/pro/text-to-video` | enum `'3'…'15'` = spec | field absent → dropped with advisory | enum `16:9,9:16,1:1` = spec | clean |
| `wan3_0_prime` → `alibaba/wan-3.0-prime/text-to-video` | `integer`, default 5 → spec free-form `Number` | enum `480p,720p,1080p` = spec | enum 6 members = spec | clean |
| `seedance_2_5` → `…/text-to-video` | enum `auto,4…30` = spec | enum = spec | enum 7 = spec | clean |
| `seedance_2_5` → `…/image-to-video` | enum `auto,4…30` = spec | enum = spec | **`const: "auto"`** | **broken, B2** |
| `qwen_image_3`, `seedream_v5_pro_edit`, all four `gpt_image_2_5_*` | n/a | n/a | **no `aspect_ratio` field** → `AspectPlan` removes the key | clean |
| `nano_banana_pro_edit` | n/a | enum `1K,2K,4K` = spec | enum incl. `16:9` | clean |
| `nano_banana_2_edit` | n/a | enum `0.5K,1K,2K,4K` = spec | enum incl. `16:9` | clean |

No integer or mixed enum anywhere in the eleven — the `ltx_2_5` failure mode does not recur.
Required-with-no-default fields are `prompt` everywhere plus `image_urls` on the three edit
endpoints and `image_url` on seedance i2v, all of which the specs declare. So B2 is the only
endpoint-level break, but it is sufficient.

### B4 — AC4 duration honesty

`seedance_2_5`'s spec list is fal's list character for character on **both** endpoints
(`auto,4,5,…,30`), so 30s is real and no offered length is one fal rejects. `wan3_0_prime` is a
bare integer on fal and free-form `ValueSpec::Number` in the spec — uncapped in-app, as AC4
requires. Mutation M3 below proves the 30s test has teeth. **AC4 not broken.**

### B5 — AC5 picker / use-case coverage, tests updated not deleted

`use_case::tests::the_models_imported_from_fal_land_on_the_right_tab` exists and asserts both
directions — the positive tab for each id, and `!supports(kling3_0_pro, ImageToVideo)`,
`!supports(wan3_0_prime, ImageToVideo)`, `!supports(qwen_image_3, EditImage)`. The array-length
guards are present and intact, not deleted: `FAL_ROUTE_MODES: [...; 48]` (media.rs:599),
`LAUNCH_FAMILIES: [...; 12]` (registry.rs:204) with `there_are_twelve_launch_families`
(registry.rs:2390) and `the_unroutable_models_are_a_known_short_list` (registry.rs:2679), all
green in the 779. **AC5 structurally not broken** — but the tab list it asserts includes
`seedance_2_5` on image-to-video, which B2 shows is a tab the model cannot run on, so AC5's
substance ("a model that appears under a tab is one that can actually run there") fails with B2.

### B6 — mutation sanity (scratch copy only; repo untouched)

| # | Mutation | Result |
|---|---|---|
| M1 | delete the `alibaba/wan-3.0-prime` row from `FAL_ROUTE_MODES` | `error[E0308]: mismatched types` — compile-time, cannot be skipped |
| M2 | typo `alibaba/qwen-image-3/text-to-image` → `…image-33/…` | 3 failures: `every_imported_model_resolves_to_an_exact_fal_endpoint`, `every_exact_endpoint_is_a_route_the_registry_actually_has`, `the_models_imported_from_fal_land_on_the_right_tab` |
| M3 | drop `"30"` from seedance's duration enum | `seedance_2_5_offers_a_thirty_second_clip` FAILED |
| M4 | registry-size literal `+ 27` → `+ 28` | `registry_covers_the_catalogue_minus_its_exclusions` FAILED |
| — | restore | `test result: ok. 657 passed; 0 failed` |

The new tests are non-vacuous. They simply do not test the thing B2 breaks.

### B7 — idempotency / re-run / cwd

`fal_diff --offline` and `audit_fal` were each run twice from the repo root with identical
output and exit 0; neither writes to the tree. `git status --porcelain` is unchanged apart from
the pre-existing untracked audit log and this file.

---

## Findings

**F1 (blocking) — `seedance_2_5` cannot generate in image-to-video mode.**
`bytedance/seedance-2.5/image-to-video` pins `aspect_ratio` to `const: "auto"`; the registry
declares a seven-member aspect enum for the single `seedance_2_5` spec; `fal_schema::parse`
never reads `const`, so `capability::fal_axis` reports the axis as free-form, the chip row
disappears, `resolveSettings` does not clear the stale value, and
`AspectPlan::decide(true, Some("16:9"), None, true)` returns `Sent("16:9")`, which
`src-tauri/src/app.rs:351-360` writes into the request body. Evidence: the raw fal schema, the
`probe_aspect` run above, and the `probe_uc` run showing the model is offered on the
image-to-video tab. This is the identical `const`-pin defect class that caused
`veo3_1_extend` to be withdrawn — **the withdrawal was incomplete**, and it lands on the model
the slice and the CHANGELOG lead with.

**F2 (minor, documentation) — stale claim left behind by the withdrawal.**
`crates/hickeyfield-core/src/registry.rs:1536`, the doc comment on `picker_only_specs`, still
reads *"…Kling 3.0 Pro, Wan 3.0 Prime, **LTX 2.5 Pro and its animate half, Veo 3.1 Extend**, GPT
Image 2.5…"*. Those three specs no longer exist. AC6 asks for docs that match reality.

---

## Root-cause diagnosis (for the repair loop)

`fal_schema::parse` (crates/hickeyfield-core/src/fal_schema.rs:213-226) extracts only
`properties[*].enum` and ignores `properties[*].const`. Every consumer therefore treats a
`const`-pinned field as an unconstrained one: `capability::fal_axis`
(crates/hickeyfield-core/src/capability.rs:603-619) reports `support=Yes, values=[]`,
`EndpointSchema::accepts` returns true, `fal_schema::reconcile`
(crates/hickeyfield-core/src/fal_schema.rs:146) skips it, and `AspectPlan::decide`
(crates/hickeyfield-core/src/aspect.rs:43-73) sends whatever the UI staged. `audit_fal`'s
"REJECTED OPTIONS" check is blind for the same reason, which is why all 11 read clean.

The one landed model this currently breaks is `seedance_2_5` on
`bytedance/seedance-2.5/image-to-video` (`aspect_ratio` `const "auto"`, verified live above).
Any of these closes it, in increasing order of goodness:

1. Narrowest, S8-scoped: withdraw `InputMode::Image` from the `bytedance/seedance-2.5` row in
   `media::FAL_ROUTE_MODES` (48 → the array shrinks, `use_case` test updated), making
   `seedance_2_5` text-to-video only — the same treatment `kling3_0_pro` and `wan3_0_prime`
   already got for a comparable binder limitation, with the reason recorded. AC3's minimum set
   still names Seedance 2.5 t2v+i2v, so this is a declared scope refusal, not a silent one.
2. Teach `fal_schema::parse` to read `const` as a one-member enum. That makes `fal_axis` return
   `values=["auto"]`, the chip row shows a single locked option, `reconcile` coerces, and
   `audit_fal` starts reporting the whole class. This is the backlog slice
   ("fal-schema-const-and-int-enums") and would also let the three withdrawn models return.
3. Either way: re-run `audit_fal` and re-check the remaining ten, and fix F2.

Until one of them lands, `CHANGELOG.md`'s "a model that appears under a tab is one that can
actually run there" is false for the Animate tab, and `implement.md` §4's "Every landed slug:
CHECKED, zero findings" is true of the audit but not of the app.

AC1, AC2, AC4, AC6 and the mechanical half of AC7 are independently verified green above; AC3
and AC5 fail on F1.

VERDICT: FAIL

---

## RE-TEST 2 (HEAD 9e53dfd)

Scope: the repair delta only (`git show 9e53dfd`), plus a full gate re-run. Everything above is
kept as history. RE-TEST 1's F1 was the blocking finding; this section re-runs it.

The delta I reviewed, from `git show 9e53dfd -- crates/ CHANGELOG.md`:

```
-    (
-        "bytedance/seedance-2.5",
-        false,
-        &[InputMode::Text, InputMode::Image],
-    ),
+    // fal serves `/seedance-2.5/image-to-video` too, and it is deliberately
+    // not claimed here: that endpoint pins `aspect_ratio` to `"auto"` by
+    // `const` ... Re-claim Image once the parser reads `const` (backlog:
+    // fal-schema-const-and-int-enums).
+    ("bytedance/seedance-2.5", false, &[InputMode::Text]),
```
plus the `use_case.rs` list losing `("seedance_2_5", UseCase::ImageToVideo)`, a second
constraint string on the seedance spec, the CHANGELOG qualifier, the `picker_only_specs` doc
comment (my F2), a stale `/extend-video` mention in `media.rs`, and the Wan 3.0 Prime duration
note.

### Commands — gate re-run at 9e53dfd

```
$ cargo test --workspace
test result: ok. 657 passed; 0 failed; 9 ignored; ... finished in 1.03s
test result: ok.   2 passed; 0 failed; 0 ignored; ... finished in 0.00s
test result: ok. 120 passed; 0 failed; 1 ignored; ... finished in 15.27s
test result: ok.   0 passed; 0 failed; ... (x3 bin/doc targets)
TEST_EXIT=0
```
Summed: **779 passed, 0 failed.**

```
$ cargo fmt --all --check                                   FMT_EXIT=0   (no output)
$ cargo clippy --workspace --all-targets -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.04s
                                                            CLIPPY_EXIT=0
$ ./scripts/lint-provenance.py
CDN hostnames      PASS  no third-party media hosts referenced
Copy provenance    PASS  no shipped string matches the reference corpus (80015 shingles indexed)
                                                            PROV_EXIT=0
$ cargo run -q -p hickeyfield-core --example fal_diff -- --offline | head -1
fal index: 1491 model(s), bundled snapshot of 2026-09-16. Registry reaches 62 fal endpoint(s).
                                                            DIFF_EXIT=0
$ cd ui && pnpm test                                        (node v24.8.0)
 Test Files  14 passed (14)
      Tests  232 passed (232)                               UITEST_EXIT=0
$ cd ui && pnpm build
✓ built in 894ms                                            UIBUILD_EXIT=0
```
63 → 62 fal endpoints reached is the one endpoint the repair gives up. Correct.

### F1 re-test — is `bytedance/seedance-2.5/image-to-video` still reachable?

fal still pins it, re-fetched live and keylessly just now:

```
$ curl -s "https://fal.ai/api/openapi/queue/openapi.json?endpoint_id=bytedance/seedance-2.5/image-to-video" | ...
{ "default": "auto", "type": "string", "title": "Aspect Ratio",
  "description": "The aspect ratio of the generated video. Always \"auto\" for image-to-video",
  "const": "auto" }
```

The same probe I used to break RE-TEST 1, re-run unchanged against a scratch copy of 9e53dfd:

```
$ cargo run -q -p hickeyfield-core --example probe_aspect
slug = bytedance/seedance-2.5
-- resolve_endpoint --
  Text -> Ok("bytedance/seedance-2.5/text-to-video")
  Image -> Err(UnsupportedMode { wanted: Image, supported: [Text] })
  Video -> Err(UnsupportedMode { wanted: Video, supported: [Text] })
-- use_case::supports (picker tabs) --
  text-to-video -> true
  image-to-video -> false
  edit-video -> false
  text-to-image -> false
  edit-image -> false
-- submit-time aspect decision (requested = DEFAULT_SETTINGS.aspect) --
  Text -> bytedance/seedance-2.5/text-to-video
     aspect values=["auto","21:9","16:9","4:3","1:1","3:4","9:16"] accepts=true plan=Sent("16:9") wire=Some("16:9")
  Image: no endpoint (this model takes a prompt on its own, not a still image) -- unreachable, nothing is sent
```

Three independent layers now agree, and I checked the third by reading this HEAD:
`media::resolve_endpoint` errors for Image; `use_case::supports` is false for the Animate tab so
`models_for_use_case` never lists it there; and `src-tauri/src/app.rs:277-286` maps a
`resolve_endpoint` error to `JobError::Permanent` **before** the provider is called, so even a
hand-constructed attachment is refused without a round trip and without spend. The `"16:9"` that
still goes on the wire now goes to the text-to-video endpoint, which enumerates `16:9`.

**F1 is genuinely fixed.**

### 30s still offered on text-to-video

```
$ cargo test -p hickeyfield-core seedance_2_5_offers_a_thirty_second_clip
test registry::tests::seedance_2_5_offers_a_thirty_second_clip ... ok
```
and independently, against fal's live schema rather than the test's own fixture:

```
-- 30s still on text-to-video --
  durations = [4.0, 5.0, ..., 29.0, 30.0]
  contains 30.0 = true
  live fal axis for bytedance/seedance-2.5/text-to-video:
    ["auto","4","5",...,"29","30"]
```
The spec's list and fal's list are identical, 30 is on both. **AC4's headline holds.**

### Wan 3.0 Prime constraint note

The old note claimed "no published ceiling". fal, fetched live:

```
"duration": { "default": 5, "anyOf": [ { "type": "integer", "maximum": 30, "minimum": 2 },
                                       { "type": "null" } ] }
```
The new note ("a plain integer from 2 to 30 seconds (default 5) … the endpoint enforces the
range") is accurate. The app still offers it free-form, which is what AC4 asks for
("free-form-integer models remain uncapped in-app, fal enforces"). **Correct.**

### F2 re-test

`crates/hickeyfield-core/src/registry.rs:1536` no longer claims LTX 2.5 / Veo 3.1 Extend were
imported; it now records them as probed-and-withdrawn. The stale `/extend-video` clause in the
`media.rs:626` comment is gone. `grep -rniE 'ltx_2_5|ltx-2\.5|veo3_1_extend|veo3\.1/extend'`
over `crates/ src-tauri/ ui/ docs/ CHANGELOG.md` still returns only the deliberate withdrawal
comments, the backlog entry and the vendored snapshot rows. **F2 fixed.**

---

## Break attempts (RE-TEST 2)

### B8 — mutation: revert the one-line repair (**not caught**)

In a scratch copy of 9e53dfd, outside the repo, I restored exactly what the repair removed:

```
697:    ("bytedance/seedance-2.5", false, &[InputMode::Text, InputMode::Image]),

$ cargo test -q --workspace
test result: ok. 657 passed; 0 failed; 9 ignored; ... finished in 0.77s
test result: ok.   2 passed; 0 failed; ...
test result: ok. 120 passed; 0 failed; 1 ignored; ... finished in 144.16s
```

**779 green with the defect fully restored.** Nothing guards the repair. The array-length
assertion does not fire (the length is still 48 — only the mode list changed), and the
`use_case` test lost its `seedance_2_5` / ImageToVideo line by **deletion**, where the same test
block two lines below already uses the negative form for precisely this situation:

```
crates/hickeyfield-core/src/use_case.rs:315  assert!(!supports(&reg["kling3_0_pro"], UseCase::ImageToVideo));
crates/hickeyfield-core/src/use_case.rs:316  assert!(!supports(&reg["wan3_0_prime"], UseCase::ImageToVideo));
crates/hickeyfield-core/src/use_case.rs:317  assert!(!supports(&reg["qwen_image_3"], UseCase::EditImage));
```

This is a defect that escaped one REVIEW, one TEST and a second REVIEW, and was found only by
probing fal's live schema. Leaving it unguarded by a one-line assertion that the file already
demonstrates the pattern for is not acceptable at a gate whose own AC5 says these guards are to
be "updated, not deleted".

### B9 — `audit_fal` after the repair (**a new finding the repair introduced**)

```
$ env -u hickeyfield_FAL_KEY -u FAL_KEY cargo run -q -p hickeyfield-core --example audit_fal
=== SILENTLY IGNORED MEDIA — bills for the wrong generation (12) ===
  ...
  seedance_2_5               bytedance/seedance-2.5/text-to-video
      catalogue offers [start_image, end_image] but the endpoint takes none
  ...
=== UNREACHABLE — fal serves no schema for this endpoint (0) ===
=== CHECKED — fal answered with a schema (62) ===
AUDIT_EXIT=0
```

Pre-repair (the committed `audit_fal-2026-09-16-reduced.log`) that section had **11** entries and
`seedance_2_5` was not among them; I verified `findings=0` for all eleven in RE-TEST 1. The
repair took the model down to one endpoint but left `("start_image", false, ValueSpec::Media)`
and `("end_image", false, ValueSpec::Media)` on the spec (`registry.rs:1564-1565`), describing
an endpoint `seedance_2_5` no longer serves.

Consequences, each checked rather than assumed:

- `implement.md` §4's pasted claim — *"Every landed slug: CHECKED, zero findings, zero
  UNREACHABLE. Gate passed, no exceptions taken."* — is now **false**, on the one model the
  slice and the CHANGELOG lead with. AC3's text ("each PASSING `audit_fal`") is the rule the
  orchestrator invoked four separate times to withdraw models; it is not being applied here.
- **No money is at risk and the user cannot reach it.** `UseCase::TextToVideo => slots() = &[]`
  (`use_case.rs:131`), so the New Video tab draws no attachment box at all, and if one were
  reached, `app.rs:277-286` refuses before the provider call. The severity is honesty, not spend.
- It is the same *class* as 11 pre-existing entries in that section (`veo3_1`, `flux_2`,
  `gpt_image_2` …), which the project already carries as its known baseline — so this is a
  regression on the slice's own verification metric, not a new kind of wrongness.

### B10 — the other ten, re-checked

The delta touches no other model's routes, slugs, modes or flags. `resolve_endpoint`,
`FAL_ROUTE_MODES` length (48), `LAUNCH_FAMILIES` (12), `there_are_twelve_launch_families`,
`the_unroutable_models_are_a_known_short_list`, `registry_covers_the_catalogue_minus_its_exclusions`
(`+27`) and `every_imported_model_resolves_to_an_exact_fal_endpoint` are all unchanged and green
in the 779. `audit_fal` shows `findings=0` for the other ten and `UNREACHABLE 0`. **Not broken.**

### B11 — repo state

`git status --porcelain` before writing this file showed only `.forge/.../review.md`, modified
by the concurrent REVIEW agent, not by me. All mutation work ran in
`scratchpad/copy2`, an rsync copy outside the repo. The only file I wrote is this one.

---

## Findings (RE-TEST 2)

- **F1 — FIXED.** Verified three ways above, including against fal's live schema. No spend path.
- **F2 — FIXED.**
- **F3 (new, introduced by the repair) — `seedance_2_5` now carries an `audit_fal` finding.**
  Its spec still declares `start_image` / `end_image` (`registry.rs:1564-1565`) for an endpoint
  it no longer serves. Unreachable from the UI and cannot bill, but it falsifies
  `implement.md` §4 and AC3's literal rule.
- **F4 (new) — the repair has no regression test.** Restoring one line re-creates the 422 defect
  with all 779 tests green (B8). The deleted positive assertion was not replaced by the negative
  one the same test block already uses three times.

## Root-cause diagnosis (for the repair loop)

Both remaining findings are in the same two files and are two lines of work.

1. **F3** — delete `("start_image", false, ValueSpec::Media)` and
   `("end_image", false, ValueSpec::Media)` from the `seedance_2_5` spec at
   `crates/hickeyfield-core/src/registry.rs:1564-1565`. The model is text-to-video only now;
   those flags describe `…/image-to-video`, which `FAL_ROUTE_MODES` no longer claims. Removing
   them restores `SILENTLY IGNORED MEDIA` to 11, makes `media::can_bind` /
   `CapabilitiesDto::unsupported_roles` honest, and makes `implement.md` §4's "zero findings"
   sentence true again. Re-add them in the same commit that re-claims `InputMode::Image` once
   the parser reads `const`. (The existing constraint note already explains the restriction to
   the user, so nothing user-facing is lost.)
2. **F4** — add one line to `the_models_imported_from_fal_land_on_the_right_tab`
   (`crates/hickeyfield-core/src/use_case.rs`, beside lines 315-317):
   `assert!(!supports(&reg["seedance_2_5"], UseCase::ImageToVideo));`
   Confirm it is non-vacuous by re-running B8: with `InputMode::Image` restored in a scratch
   copy, that test must fail.

Then re-run `audit_fal` (expect `SILENTLY IGNORED MEDIA (11)`, `CHECKED 62`, `UNREACHABLE 0`,
`seedance_2_5 findings=0`) and `cargo test --workspace` (expect 779, or 779 with the added
assertion folded into the existing test).

AC1, AC2, AC4, AC5's named guards, AC6 and AC7's mechanical gate are all independently verified
green at 9e53dfd. AC3 fails on F3; the durability of the F1 repair fails on F4.

VERDICT: FAIL

---

## RE-TEST 3 (HEAD 55aa32f)

Scope: the `55aa32f` delta only, plus a full gate re-run. RE-TEST 2's F3 and F4 were the
blocking findings; this section tries to break their repair. Everything above is history.

The source delta, from `git show 55aa32f -- crates/ src-tauri/ ui/ docs/ CHANGELOG.md` — three
hunks, nothing else:

```
 use_case.rs   +assert!(!supports(&reg["seedance_2_5"], UseCase::ImageToVideo));   (line 317)
 registry.rs   -("start_image", false, ValueSpec::Media),
               -("end_image",   false, ValueSpec::Media),
               +// `start_image` / `end_image` return with `InputMode::Image`
               +// once the parser reads fal's `const` (see `FAL_ROUTE_MODES`).
 media.rs      comment only: the withdrawn `…/text-to-video/pro` example replaced
```

## Commands

```
$ cargo test --workspace
test result: ok. 657 passed; 0 failed; 9 ignored; ... finished in 0.63s
test result: ok.   2 passed; 0 failed; 0 ignored; ... finished in 0.00s
test result: ok. 120 passed; 0 failed; 1 ignored; ... finished in 15.13s
test result: ok.   0 passed; 0 failed; ... (x3 bin/doc targets)
TEST_EXIT=0
```
Summed: **779 passed, 0 failed.**

```
$ cargo fmt --all --check                                   FMT_EXIT=0   (no output)
$ cargo clippy --workspace --all-targets -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.08s
                                                            CLIPPY_EXIT=0
$ ./scripts/lint-provenance.py
CDN hostnames      PASS  no third-party media hosts referenced
Copy provenance    PASS  no shipped string matches the reference corpus (80015 shingles indexed)
                                                            PROV_EXIT=0
$ cargo run -q -p hickeyfield-core --example fal_diff -- --offline | head -1
fal index: 1491 model(s), bundled snapshot of 2026-09-16. Registry reaches 62 fal endpoint(s).
                                                            DIFF_EXIT=0
$ cd ui && pnpm test                                        (node v24.8.0)
 Test Files  14 passed (14)
      Tests  232 passed (232)                               UITEST_EXIT=0
$ cd ui && pnpm build
✓ built in 878ms                                            UIBUILD_EXIT=0
```

### F3 re-test — `audit_fal`, keyless, my own run

```
$ env -u hickeyfield_FAL_KEY -u FAL_KEY cargo run -q -p hickeyfield-core --example audit_fal
=== SILENTLY IGNORED MEDIA — bills for the wrong generation (11) ===
=== MISSING REQUIRED FIELDS — 422 after a round trip (2) ===
=== DROPPED SETTINGS — the control has no effect (21) ===
=== REJECTED OPTIONS — the chip row offers invalid values (9) ===
=== UNREACHABLE — fal serves no schema for this endpoint (0) ===
=== CHECKED — fal answered with a schema (62) ===
AUDIT_EXIT=0
```
Back to **11**, from the 12 that RE-TEST 2 measured. The only `seedance_2_5` line in the whole
log is at line 144, inside the CHECKED section (which starts at line 99). Per-id tally, computed
by splitting the log at the CHECKED header:

```
gpt_image_2_5_flare         findings=0 checked=1
gpt_image_2_5_flare_edit    findings=0 checked=1
gpt_image_2_5_sunburst      findings=0 checked=1
gpt_image_2_5_sunburst_edit findings=0 checked=1
kling3_0_pro                findings=0 checked=1
nano_banana_2_edit          findings=0 checked=1
nano_banana_pro_edit        findings=0 checked=1
qwen_image_3                findings=0 checked=1
seedance_2_5                findings=0 checked=1
seedream_v5_pro_edit        findings=0 checked=1
wan3_0_prime                findings=0 checked=1
```

I diffed my fresh run against the log committed at
`.forge/slices/S8-fal-catalogue-sync/audit_fal-2026-09-16-reduced.log`:

```
$ diff <(grep -v 'Compiling\|Finished\|Running' audit_fal-2026-09-16-reduced.log) my-audit.log
IDENTICAL to the committed log
```
The evidence file is not a summary of a run I have to take on trust; it reproduces byte for byte.
**F3 fixed.** `implement.md`'s "every landed slug: CHECKED, zero findings, zero UNREACHABLE" is
true again, and I have now measured it rather than read it.

Behaviour behind the flag removal, probed rather than assumed (scratch copy of 55aa32f):

```
$ cargo run -q -p hickeyfield-core --example probe_roles
  Start          can_bind=false
  End            can_bind=false
  Reference      can_bind=false
  Video          can_bind=false
  VideoReference can_bind=false
  Audio          can_bind=false
  AudioReference can_bind=false
media_flags  = []
takes_prompt = true
required     = ["prompt"]
```
A prompt-only text-to-video model that binds no media role — which is exactly what
`FAL_ROUTE_MODES` now claims, so `media::can_bind`, `CapabilitiesDto::unsupported_roles`,
`use_case::route_serves` and `audit_fal` all tell the same story. Nothing user-facing is lost:
`UseCase::TextToVideo => slots() = &[]` (use_case.rs:131), so the New Video tab never drew an
attachment box for it in the first place.

### F4 re-test — B8 mutation, both directions (**now caught**)

Run in `scratchpad/copy3`, an rsync copy of 55aa32f outside the repo.

**B8a — restore only the mode** (literally the line the previous repair removed):

```
697:    ("bytedance/seedance-2.5", false, &[InputMode::Text, InputMode::Image]),

$ cargo test -q -p hickeyfield-core
---- use_case::tests::the_models_imported_from_fal_land_on_the_right_tab stdout ----
thread '...' panicked at crates/hickeyfield-core/src/use_case.rs:317:9:
assertion failed: !supports(&reg["seedance_2_5"], UseCase::ImageToVideo)
test result: FAILED. 656 passed; 1 failed; 9 ignored; 0 measured; 0 filtered out
```

**B8b — restore the mode *and* both media flags** (the complete revert to the state that
produced the original 422 defect). I ran this separately because the new assertion goes through
`supports` → `route_serves`, which is guarded by both the mode list *and* `can_bind`; a guard
that only fired when both were reverted would be weaker than it looks:

```
$ cargo test -q --workspace
---- use_case::tests::the_models_imported_from_fal_land_on_the_right_tab stdout ----
test result: FAILED. 656 passed; 1 failed; 9 ignored; 0 measured; 0 filtered out
```

**Both directions fail the build.** The guard is non-vacuous against a partial revert and
against a full one. Restoring the pristine files returns the suite to green:

```
$ cargo test -q -p hickeyfield-core
test result: ok. 657 passed; 0 failed; 9 ignored; ... finished in 0.43s
test result: ok.   2 passed; 0 failed; ...
```
**F4 fixed.**

### F1 / AC4 — still fixed, re-measured not re-read

```
$ cargo test -p hickeyfield-core seedance_2_5_offers_a_thirty_second_clip
test registry::tests::seedance_2_5_offers_a_thirty_second_clip ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 665 filtered out

$ cargo run -q -p hickeyfield-core --example probe_aspect
-- resolve_endpoint --
  Text  -> Ok("bytedance/seedance-2.5/text-to-video")
  Image -> Err(UnsupportedMode { wanted: Image, supported: [Text] })
  Video -> Err(UnsupportedMode { wanted: Video, supported: [Text] })
-- use_case::supports (picker tabs) --
  text-to-video -> true      image-to-video -> false   edit-video -> false
  text-to-image -> false     edit-image -> false
-- submit-time aspect decision (requested = DEFAULT_SETTINGS.aspect) --
  Text -> bytedance/seedance-2.5/text-to-video
     aspect values=["auto","21:9","16:9","4:3","1:1","3:4","9:16"] accepts=true
     plan=Sent("16:9") wire=Some("16:9")
  Image: no endpoint -- unreachable, nothing is sent
-- 30s still on text-to-video --
  durations = [4.0, 5.0, ..., 29.0, 30.0]   contains 30.0 = true
  live fal axis: ["auto","4",...,"29","30"]
```
The `const`-pinned endpoint is unreachable at three independent layers (resolver, picker,
and `src-tauri/src/app.rs:277-286`, which converts a `resolve_endpoint` error into
`JobError::Permanent` before the provider is called — no round trip, no spend). The `"16:9"`
that still goes on the wire goes to the endpoint that enumerates `16:9`. The 30s option is
present and matches fal's live enum exactly. **AC4's headline holds.**

### B12 — did the flag removal break anything else?

`cargo test --workspace` is 779/0 including `the_unroutable_models_are_a_known_short_list`,
`editing_a_clip_offers_only_models_that_take_one`, `every_imported_model_resolves_to_an_exact_fal_endpoint`,
`registry_covers_the_catalogue_minus_its_exclusions` (`+27`), `there_are_twelve_launch_families`,
and the compile-time `FAL_ROUTE_MODES: [...; 48]`. `seedance_2_5` is not in `NO_MEDIA_MODELS`,
which is harmless: `supports` returns false for all four media tabs anyway via `can_bind`
(probed above), so the two paths agree. `fal_diff --offline` and `audit_fal` were each run twice
with identical output and exit 0; neither writes to the tree. **Not broken.**

### B13 — repo state

`git status --porcelain` before writing this file showed only
`.forge/slices/S8-fal-catalogue-sync/review.md`, modified by the concurrent REVIEW agent, not by
me. All mutation work ran in `scratchpad/copy3`. The only file I wrote is this one.

---

## Verdict on the acceptance criteria (reduced 11-model set, HEAD 55aa32f)

| AC | Verdict | Evidence in this file |
|---|---|---|
| AC1 — reusable fal→registry diff tool, no key, fixture tests | **PASS** | `fal_diff --offline` runs (62 endpoints reached), 2 example tests inside the workspace gate |
| AC2 — snapshot refreshed and committed | **PASS** | 1,491 rows, `snapshot_date` 2026-09-16, `fal_catalogue` constants green in the 779 |
| AC3 — curated import, routed + priced + `audit_fal`-clean before landing | **PASS** | 11/11 `findings=0 checked=1`, `UNREACHABLE 0`, my run byte-identical to the committed log |
| AC4 — duration honest; native 30s | **PASS** | spec list == fal's live enum for seedance t2v incl. 30; Wan note now matches fal's `2..30` |
| AC5 — picker tabs correct; array-length guards updated not deleted | **PASS** | `the_models_imported_from_fal_land_on_the_right_tab` with 3+1 negative assertions, all non-vacuous (B8a/B8b, M2); `[...; 48]`, `[...; 12]` intact |
| AC6 — stale docs corrected, CHANGELOG, provenance lint | **PASS** | FIRST-LIGHT/backlog updated, CHANGELOG qualified for the i2v restriction, provenance PASS |
| AC7 — full regression gate | **PASS** | 779/0, fmt, clippy `-D warnings`, provenance, UI 232/232 + build |

### What this PASS does and does not say

It says: at 55aa32f, all eleven landed models are routed, priced, probed against fal's live
schema, offered only on tabs they can actually run on, and the gate is green — and I verified
each of those by running it, not by reading `implement.md`. The three findings I raised
(F1 const-pinned aspect, F3 phantom media flags, F4 unguarded repair) are each closed and each
re-broken-and-re-caught above.

It does not say the slice delivered its full headline scope. Four declared refusals stand, all
of them under AC3's own rule ("a slug that fails the audit is NOT added"), all disclosed in
code comments, `implement.md` §8 + addenda, the backlog and the CHANGELOG:

- MiniMax H3 Max / H3 Max Turbo — never imported (required `prompt_expansion_mode` with no
  default; needs a constant-fields mechanism). The slice's own one-line summary names H3 Max
  first, so this is a real scope shortfall.
- `ltx_2_5`, `ltx_2_5_animate`, `veo3_1_extend` — withdrawn (integer/mixed enums and `const`).
- Seedance 2.5 image-to-video — withdrawn (`const`), the model still ships text-to-video with
  the native 30s that is the slice's other headline.
- Kling 3.0 Pro / Wan 3.0 Prime — text-to-video only (`start_image_url` the binder cannot write).

All four converge on one root cause that is already a separate backlog slice
(`fal-schema-const-and-int-enums`): `fal_schema::parse` reads only `properties[*].enum`, so
`const` and integer enum members are invisible to the capability layer, to `reconcile` and to
`audit_fal`'s own checks. Whether S8 releases with that much of its minimum set deferred is an
owner decision above this gate, and it is recorded here so it is made knowingly rather than by
default.

One non-blocking observation, comment-only and pre-existing in kind: the `FAL_ROUTE_MODES`
block comment at `media.rs:627-631` still generalises these rows as "split by something other
than an input mode", which is loose for `alibaba/qwen-image-3/text-to-image`,
`fal-ai/nano-banana-*/edit` and `bytedance/seedream/v5/pro/edit` — those are plain, already
applied mode suffixes. The actionable half of the comment ("the mode list is the one mode it
serves and `Exact` is true. Suffixing any of them 404s") is correct for every row in the block,
and `55aa32f` only replaced a dead example. Worth a word at DOCUMENT; not a defect.

I tried to break this three times and succeeded twice. The third attempt found nothing that
changes what the app does, what it bills, or what it tells the user.

VERDICT: PASS
