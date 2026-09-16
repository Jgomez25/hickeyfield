# S9-fal-schema-const-and-int-enums — IMPLEMENT (claim)

Branch `forge/s9-fal-schema-const` — working tree only, **not committed, not pushed, main untouched**.
Every command below was run in this session; every block is pasted output, not a summary.
`examples/first_light.rs` was never run. No key exists in this session and nothing was submitted.

---

## 0. The design's first task: the four endpoint documents, re-fetched keyless

Fetched from `https://fal.ai/api/openapi/queue/openapi.json?endpoint_id=…` and checked in raw at
`.forge/slices/S9-fal-schema-const-and-int-enums/schemas/` (4 files, 27 KB). Reproduce the
extraction with:

```
$ cd .forge/slices/S9-fal-schema-const-and-int-enums/schemas
$ for f in *.json; do python3 -c "
import json; d=json.load(open('$f')); s=next(v for k,v in d['components']['schemas'].items() if k.endswith('Input'))
print(json.dumps({'required': s.get('required'), 'properties': s['properties']}, indent=1))"; done
```

**Deviation from design §0, stated plainly:** the design says paste the four documents. They are
27 KB of mostly prose descriptions and example prompts. Pasted below are the *whole* of every
property this slice reasons about, verbatim; the untouched remainder (descriptions, `examples`,
`seed`, `negative_prompt`, `end_user_id`, `auto_fix`, `safety_tolerance`, `bitrate_mode`) is in the
checked-in raw JSON. Nothing below is from memory.

**`lightricks/ltx-2.5/text-to-video/pro`** — `required: ["prompt"]`
```json
"duration":      {"default":"auto","enum":[6,8,10,"auto"],"title":"Duration"}
"aspect_ratio":  {"type":"string","default":"16:9","enum":["16:9","9:16"]}
"fps":           {"type":"integer","default":25,"enum":[24,25,50]}
"resolution":    {"type":"string","default":"1080p","enum":["720p","1080p"]}
"generate_audio":{"type":"boolean","default":true}
"camera_motion": {"anyOf":[{"type":"string","enum":["dolly_in","dolly_out","dolly_left",
                  "dolly_right","jib_up","jib_down","static","focus_shift"]},{"type":"null"}]}
```

**`lightricks/ltx-2.5/image-to-video/pro`** — `required: ["prompt","image_url"]`
```json
"duration":      {"default":"auto","enum":[6,8,10,"auto"]}
"aspect_ratio":  {"type":"string","default":"auto","enum":["auto","16:9","9:16"]}
"fps":           {"type":"integer","default":25,"enum":[24,25,50]}
"resolution":    {"type":"string","default":"1080p","enum":["720p","1080p"]}
"image_url":     {"type":"string","title":"Start Image URL"}
"end_image_url": {"anyOf":[{"type":"string"},{"type":"null"}],"title":"End Image URL"}
"generate_audio":{"type":"boolean","default":true}
```

**`fal-ai/veo3.1/extend-video`** — `required: ["prompt","video_url"]`
```json
"duration":      {"const":"7s","type":"string","default":"7s"}
"resolution":    {"const":"720p","type":"string","default":"720p"}
"aspect_ratio":  {"type":"string","default":"auto","enum":["auto","16:9","9:16"]}
"generate_audio":{"type":"boolean","default":true}
"video_url":     {"type":"string","title":"Video URL"}
```

**`bytedance/seedance-2.5/image-to-video`** — `required: ["prompt","image_url"]`
```json
"aspect_ratio":  {"default":"auto","type":"string","const":"auto",
                  "description":"… Always \"auto\" for image-to-video"}
"duration":      {"default":"auto","enum":["auto","4",…,"30"],"type":"string"}
"resolution":    {"default":"720p","enum":["480p","720p","1080p"],"type":"string"}
"image_url":     {"type":"string"}
"end_image_url": {"anyOf":[{"type":"string"},{"type":"null"}]}
"generate_audio":{"type":"boolean","default":true}
```

### What the documents settled (the design's open questions)

| Q | Answer from the live document | What was built |
|---|---|---|
| **Q3** — does extend-video publish `generate_audio`? | **Yes**, boolean default true | Declared on the spec; priced `per_second_audio(0.20, 2.0)` ⇒ $1.40 / $2.80 (design's stated default) |
| **Q4** — do LTX's two endpoints agree? | **No.** t2v aspect `["16:9","9:16"]`, i2v aspect `["auto","16:9","9:16"]`. duration/fps/resolution identical | **Two specs, and neither declares an aspect flag** — the per-endpoint schema is the only thing that can be right about it. Pinned by `fal_schema::tests::the_two_ltx_endpoints_do_not_agree_on_aspect_ratio` |
| **Q5** — does extend-video need a `NO_END_FRAME` / `FAL_NO_MEDIA` row? | No — `video_url` is its only media key | Those lists unchanged (3 / 10 / 8 / 10) |
| **Q6** — LTX 4K | fal's enum is `["720p","1080p"]`; 4K is unselectable | Kept unselectable, recorded in the route note |
| **Q1/Q2** | — | Design defaults taken: `pinned: BTreeSet<String>` beside `enums`; a Class-A pin is coerced **and reported** |

The design's §1.1 quote of Seedance and §0.1's Veo/LTX/Wan fragments all match the live documents.
**No fragment in the design was contradicted**, so no design fact was amended.

---

## 1. Files changed (10)

| file | what |
|---|---|
| `crates/hickeyfield-core/src/fal_schema.rs` | `EndpointSchema.pinned`; `parse` reads `const` + stringifies non-string enum members; `Pinned` struct; `reconcile` returns `Result<Vec<Pinned>, Refusal>`; module doc gains bug #4; 14 new tests |
| `crates/hickeyfield-core/src/capability.rs` | **no production change** (that is the claim of design §4); `pinned: Default::default()` on the test helper, a `pinned_schema` helper built through `parse`, 3 new tests |
| `crates/hickeyfield-core/examples/audit_fal.rs` | check 4 now reads the already-fetched schema; the duplicate `schema_enum` fetcher **deleted**; new check 5 `pin_conflicts` + its 3 fixture tests; module doc "Four checks" → "Five checks" |
| `crates/hickeyfield-core/Cargo.toml` | `[[example]] name = "audit_fal" / test = true` so those tests run in `cargo test --workspace` |
| `crates/hickeyfield-core/src/registry.rs` | specs + priced routes for `ltx_2_5`, `ltx_2_5_animate`, `veo3_1_extend`; Seedance `start_image`/`end_image` restored and its withdrawal constraint replaced; `JOB_TYPES`; `IMPORTED_2026_09` 11→14; registry size `+27`→`+30`; `picker_only_specs` doc rewritten; 2 new tests |
| `crates/hickeyfield-core/src/media.rs` | `FAL_ROUTE_MODES` 48→**51** (3 rows) and `bytedance/seedance-2.5` back to `[Text, Image]`; both withdrawal comments rewritten |
| `crates/hickeyfield-core/src/use_case.rs` | 4 positive rows added (incl. the flipped `seedance_2_5` ImageToVideo); its negative assertion removed; comment rewritten |
| `src-tauri/src/app.rs` | the one `reconcile` call site now consumes `Vec<Pinned>` and pushes a job advisory per pin |
| `CHANGELOG.md` | new `### Fixed` bullet; the S8 `### Added` bullet's "text-to-video for now" qualifier removed (it would otherwise contradict it) |
| `.forge/backlog.md` | the slice's own entry marked DONE with what landed |

**No file under `ui/` was changed** — design §4's claim held. `pnpm test` / `pnpm build` are still run
as gates below.

---

## 2. AC-by-AC evidence

### AC1 — the parser

`parse` now: reads a scalar `const` as a one-member `enums` entry **and** adds the field to `pinned`
(a non-scalar `const` is ignored entirely); otherwise stringifies every enum member through the
**existing** `scalar_text` (so `6.0` and `6` render the same on both sides); all-or-nothing — one
unreadable member voids the entry; a `null` member is skipped without voiding it; `anyOf`/`oneOf`
are still not descended into.

```
$ cargo test -p hickeyfield-core --lib fal_schema
running 33 tests
test fal_schema::tests::a_const_is_recorded_as_a_one_member_enum ... ok
test fal_schema::tests::an_integer_enum_is_stringified_not_dropped ... ok
test fal_schema::tests::a_pure_integer_enum_is_kept ... ok
test fal_schema::tests::a_const_beside_an_enum_wins ... ok
test fal_schema::tests::a_nullable_integer_is_not_enumerated ... ok
test fal_schema::tests::a_null_member_is_skipped_without_voiding_the_enum ... ok
test fal_schema::tests::an_enum_we_cannot_fully_read_is_recorded_as_nothing ... ok
test fal_schema::tests::a_const_that_is_not_a_scalar_is_ignored_entirely ... ok
test fal_schema::tests::published_enums_are_captured ... ok
test fal_schema::tests::the_two_ltx_endpoints_do_not_agree_on_aspect_ratio ... ok
[…]
test result: ok. 33 passed; 0 failed; 0 ignored; 0 measured; 647 filtered out
```

The Wan 3.0 Prime regression fixture is its `anyOf` verbatim
(`{"default":5,"anyOf":[{"type":"integer","minimum":2,"maximum":30},{"type":"null"}]}`) and asserts
*no* enum and an empty `pinned`.

### AC2 — reconcile / coerce

Policy implemented is design D2: **Class A** (any field that is not `prompt` and not a media key) is
coerced to the pin and a `Pinned{field, from, to}` is returned; **Class B** (`prompt`, media keys) is
**refused**, body untouched; **Class C** (pinned key absent from the body) is left alone. The pinned
pass runs *before* the enum pass, so the type half of the contract still holds. `coerce` itself is
unchanged.

```
test fal_schema::tests::a_pinned_field_is_written_to_the_value_the_endpoint_fixes ... ok
test fal_schema::tests::a_pinned_duration_accepts_the_bare_number ... ok
test fal_schema::tests::a_pinned_prompt_or_media_key_is_refused_not_rewritten ... ok
test fal_schema::tests::a_pinned_field_the_body_does_not_carry_is_left_to_fal ... ok
test fal_schema::tests::a_numeric_duration_matches_an_integer_enum_member ... ok
test fal_schema::tests::a_body_value_the_endpoint_does_not_offer_is_refused_not_substituted ... ok
```

`7` against the pin `"7s"` produces `"7s"` and **no** `Pinned` row — nothing was overridden, so
nothing is claimed. `app.rs` turns each row into a job advisory
(`"… always renders at 7s duration on this endpoint — your 5 setting does not apply"`), in the
voice already used four lines below it.

### AC3 — the audit sees it

- Check 4 now reads `schema.enums` from the schema already fetched and cached at `audit_fal.rs:63`.
- `fn schema_enum` — a second, string-only re-implementation of the enum logic **and** a second HTTP
  request per field — is deleted.
- New check 5, `pin_conflicts(spec, endpoint, schema)`, is pure and network-free: it checks the
  **online** axis (`ModelCapability::from_fal`) and the **offline** one (the spec's own flag: an
  `Enum` member that does not coerce to the pin, or a free-form `Number`/`Integer`/`Text`), plus the
  no-flag `aspect_ratio` case the submit path still sends.

```
$ cargo test -p hickeyfield-core --example audit_fal
running 3 tests
test tests::an_endpoint_that_pins_nothing_reports_nothing ... ok
test tests::a_const_the_spec_contradicts_is_reported ... ok
test tests::a_spec_that_mirrors_the_pin_reports_nothing ... ok
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

`a_const_the_spec_contradicts_is_reported` runs the real `fal-ai/veo3.1/extend-video` document
through `parse` and the S8 spec shape through `pin_conflicts`, and asserts the row names
`resolution`, pin `720p`, could-send `["1080p"]`.

### AC4 — re-probe of every existing fal route, and the triage

Both runs are keyless, full output checked in:

- `.forge/slices/S9-fal-schema-const-and-int-enums/audit_fal-before.log` — run at the start of the
  session with the **old** parser, before any source edit. It reproduces S8's recorded baseline
  exactly (`11 / 2 / 21 / 9`, UNREACHABLE 0, CHECKED 62), which is what makes it a valid "before".
- `.forge/slices/S9-fal-schema-const-and-int-enums/audit_fal-after.log` — final state.

```
$ env -u hickeyfield_FAL_KEY -u FAL_KEY cargo run -q -p hickeyfield-core --example audit_fal
=== SILENTLY IGNORED MEDIA — bills for the wrong generation (11) ===
=== MISSING REQUIRED FIELDS — 422 after a round trip (2) ===
=== DROPPED SETTINGS — the control has no effect (21) ===
=== REJECTED OPTIONS — the chip row offers invalid values (9) ===
=== PINNED FIELDS — the endpoint fixes this value and we would send another (0) ===
=== UNREACHABLE — fal serves no schema for this endpoint (0) ===
=== CHECKED — fal answered with a schema (66) ===

66 endpoint(s) audited against fal's own schema.
```

Section-by-section against the S8 baseline: `11 → 11`, `2 → 2`, `21 → 21`, `9 → 9`, PINNED `0`,
UNREACHABLE `0`, CHECKED `62 → 66`. **Every finding count is unchanged; the only movement is the
four new endpoints under CHECKED.** No re-landed id appears in any finding section
(`grep -cE 'ltx_2_5|veo3_1_extend|seedance_2_5'` over lines 1–104 = `0`).

An intermediate run was taken **after the parser change and before the re-land**, so a new finding
could not be confused with one the re-land caused. Its diff against the before-log is exactly one
hunk — the new (empty) section header:

```
$ diff audit_fal-before.log audit-midfix.log
95a96,98
> === PINNED FIELDS — the endpoint fixes this value and we would send another (0) ===
>   none
```

**Per-endpoint scan of the defect class.** The audit's section counts alone cannot answer "which
endpoint gained a `const` or an integer enum", so all 66 endpoints were fetched with `curl` and
scanned directly (keyless, free):

| endpoint | new `const` | new non-string enum | verdict |
|---|---|---|---|
| `bytedance/seed-audio-1.0` | — | `sample_rate [8000…48000]` | **No action.** Unmapped field (not duration/resolution/aspect), so it lands in `unmapped_enums`, which nothing renders. Nothing in the app sends `sample_rate`. |
| `fal-ai/inworld-tts` | — | `sample_rate_hertz [8000…48000]` | **No action**, same reason. |
| `fal-ai/wan/v2.7/text-to-video` | — | `duration [2..15]` | **No action needed — this is the fix working.** The `wan2_7` spec declares `duration` as a catalogue integer (default 5), so check 4 never compared them. The axis is now fal's own list instead of free-form: the chip row offers 2–15, the default 5 is on it, and a stale 20 becomes an honest refusal instead of a silent 422. Strictly better; nothing to change. |
| `fal-ai/wan/v2.7/image-to-video` | — | `duration [2..15]` | as above |
| `lightricks/ltx-2.5/{text,image}-to-video/pro` | — | `duration [6,8,10,"auto"]`, `fps [24,25,50]` | **Fixed here** (AC5). `fps` is unmapped and unsent. |
| `fal-ai/veo3.1/extend-video` | `duration "7s"`, `resolution "720p"` | — | **Fixed here** (AC5) |
| `bytedance/seedance-2.5/image-to-video` | `aspect_ratio "auto"` | — | **Fixed here** (AC5 + §3 below) |
| the other 58 endpoints | none | none | nothing to triage |

So the expected "non-zero number of new rows" (design §5) turned out to be **zero new findings on
the 62 pre-existing endpoints**, and the only two behaviour changes on a model this slice does not
name (`wan2_7`, both modes) are improvements. Reproduce the scan:

```
$ curl -s "https://fal.ai/api/openapi/queue/openapi.json?endpoint_id=fal-ai/wan/v2.7/text-to-video" \
  | python3 -c "import json,sys; s=next(v for k,v in json.load(sys.stdin)['components']['schemas'].items() if k.endswith('Input')); print(s['properties']['duration'])"
{'type': 'integer', 'default': 5, 'enum': [2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15], …}
```

### AC5 — the re-land

Landed: `ltx_2_5`, `ltx_2_5_animate`, `veo3_1_extend`, and Seedance 2.5's `InputMode::Image` with
`start_image`/`end_image` restored. Counts moved exactly as design §6.1 specified:
`FAL_ROUTE_MODES` 48→**51**, `IMPORTED_2026_09` 11→**14**, registry size `+27`→**+30**;
`FAL_MISSING_ROUTES` 3, `FAL_NO_MEDIA` 10, `NO_END_FRAME` 8, `NO_MEDIA_MODELS` 10 all unchanged
(Q5 confirmed by the probe). `JOB_TYPES` outer length stays 16: `veo3_1_extend` joined `animate`,
the two LTX ids joined `video`. `use_case.rs`'s negative Seedance assertion became a positive row.

```
$ cargo test -p hickeyfield-core --lib
test registry::tests::registry_covers_the_catalogue_minus_its_exclusions ... ok
test registry::tests::every_imported_model_resolves_to_an_exact_fal_endpoint ... ok
test registry::tests::every_imported_model_has_a_fal_route_with_a_real_adapter ... ok
test registry::tests::ltx_2_5_offers_only_the_durations_fal_enumerates ... ok
test registry::tests::the_extend_endpoint_is_priced_at_its_fixed_seven_seconds ... ok
test registry::tests::seedance_2_5_offers_a_thirty_second_clip ... ok
test use_case::tests::the_models_imported_from_fal_land_on_the_right_tab ... ok
test media::tests::every_exact_endpoint_is_a_route_the_registry_actually_has ... ok
test result: ok. 676 passed; 0 failed; 9 ignored; 0 measured; 0 filtered out
```

`the_extend_endpoint_is_priced_at_its_fixed_seven_seconds` asserts $1.40 audio-off and $2.80
audio-on (fal's own published pair) and that the settings can only produce those 7 seconds.
`ltx_2_5_offers_only_the_durations_fal_enumerates` asserts `durations == [6.0, 8.0, 10.0]` for both
LTX ids — `"auto"` is not a number, and the app-wide default 5 clamps to 6 in the UI.

### AC6 — capability / UI honesty

`capability.rs` production code is **unchanged**: because a pin is recorded in `enums`, `fal_axis`
returns `Axis::supported(vec![P], …)` and `is_free_form()` is false with zero edits — which is the
argument D1 was chosen for, now measured rather than asserted:

```
test capability::tests::a_const_pinned_axis_is_fixed_not_free_form ... ok
test capability::tests::a_catalogue_default_outside_a_pin_is_dropped ... ok
test capability::tests::one_spec_two_endpoints_and_the_pin_applies_to_only_one_of_them ... ok
test capability::tests::a_fal_field_with_no_published_enum_is_free_form_not_absent ... ok
```

The third is the path design §12.3 called "the proof requires running `model_capabilities` twice":
it drives `capability::for_route` over the **real** `seedance_2_5` spec with `InputMode::Image` and
then `InputMode::Text`, and asserts `aspect.values == ["auto"]` on the image-to-video endpoint and
seven values on the text-to-video one. Its pinned schema is built by running `fal_schema::parse` over
fal's own document shape, not hand-filled, so it fails if the parser stops reading `const`.

### AC7 — the gate

```
$ cargo test --workspace
running 685 tests
test result: ok. 676 passed; 0 failed; 9 ignored; 0 measured; 0 filtered out; finished in 1.55s
running 3 tests      (examples/audit_fal)
test result: ok. 3 passed; 0 failed; …
running 2 tests      (examples/fal_diff)
test result: ok. 2 passed; 0 failed; …
running 121 tests    (hickeyfield-tauri)
test result: ok. 120 passed; 0 failed; 1 ignored; …
  (doc-tests: 0/0/0)
                                            → 801 passed, 0 failed  (baseline 779)

$ cargo fmt --all --check
fmt: clean          (no output)

$ cargo clippy --workspace --all-targets -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 5.13s

$ ./scripts/lint-provenance.py
CDN hostnames
  PASS  no third-party media hosts referenced
Copy provenance
  PASS  no shipped string matches the reference corpus (80015 shingles indexed)

$ cargo run -q -p hickeyfield-core --example fal_diff -- --offline
fal index: 1491 model(s), bundled snapshot of 2026-09-16. Registry reaches 66 fal endpoint(s).

$ cd ui && pnpm test
 Test Files  14 passed (14)
      Tests  232 passed (232)

$ cd ui && pnpm build
dist/assets/index-IklELJ0D.js                 281.43 kB │ gzip: 88.29 kB
✓ built in 1.89s

$ cargo build --workspace
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 24.21s
```

---

## 3. Deviations from the design, and why

### D-1 (the one that matters) — `seedance_2_5`'s spec-level `aspect_ratio` narrowed to `["auto"]`

Design §6.4 says: *"leave the `aspect_ratio` enum … exactly as it is — it is fal's t2v list verbatim
and changing it would break the mode that works."* Design §3.2(2) says the **offline** capability
path must be checked against the pin. Design §8 says PINNED FIELDS must be 0 and a re-landed id in
any finding section means it does not land. With a 7-member spec enum those three cannot all hold,
and the audit said so out loud — this was the run before the change:

```
=== REJECTED OPTIONS — the chip row offers invalid values (10) ===
  seedance_2_5               bytedance/seedance-2.5/image-to-video
      offers aspect_ratio [16:9, 1:1, 21:9, 3:4, 4:3, 9:16] which the endpoint rejects (it takes [auto])
=== PINNED FIELDS — the endpoint fixes this value and we would send another (1) ===
  seedance_2_5               bytedance/seedance-2.5/image-to-video
      pins aspect_ratio = auto but the app can send [16:9, 1:1, 21:9, 3:4, 4:3, 9:16]
```

That is a real hazard, not an artefact: when fal's schema endpoint is unreachable,
`capability::for_route` falls back to `from_catalog`, the chip row is drawn from the **spec**, and
`app.rs` then always sends an explicit ratio — `16:9` to an endpoint that takes only `auto`, i.e. a
422. Per the design's own §5 triage table ("PINNED FIELDS on any model → **Fix here**"), I fixed it:
`registry.rs` now declares `("aspect_ratio", false, en(&["auto"]))` for `seedance_2_5`.

What this costs and what it does not:
- **Online (the normal path): nothing.** `fal_axis` reads the per-endpoint schema, so the
  text-to-video chip row still shows all seven ratios. `one_spec_two_endpoints_…` proves it.
- **Offline: the text-to-video aspect chip row collapses to `auto`.** That is a real, narrow
  reduction, and it is the direction that cannot 422. I judged "one fewer control while fal's schema
  host is unreachable" strictly better than "a mode that 422s while fal's schema host is
  unreachable", and better than re-withdrawing the animate mode for a third time.
- The rule the spec enum now follows is statable: *the values valid on every endpoint this model
  serves*. That is written into the code comment and into a new user-facing constraint string.

**A reviewer who disagrees should say so** — reverting is a one-line change plus re-accepting the
two audit rows above.

### D-2 — LTX 2.5 declares no `aspect_ratio` flag at all

Q4 came back "the two endpoints disagree". Rather than pick one list for a model-level spec (the
mistake D-1 is about), neither LTX spec declares the flag: the per-endpoint schema is the only thing
that can be right about it, and it is available on the online path. Audit is clean either way.

### D-3 — a live-document `include_str!` test was written and then removed

I first wrote a test that ran `parse` over the four checked-in `.forge/…/schemas/*.json` files via
`include_str!`. It passed, but it makes the **crate build** depend on a path under `.forge/`, so
deleting the slice's evidence directory would break `cargo build`. Replaced by
`the_two_ltx_endpoints_do_not_agree_on_aspect_ratio`, whose fixtures are the same document fragments
inlined. The raw documents remain as evidence, not as a build input.

### D-4 — `Vec<Pinned>` return type: all existing `.unwrap()` call sites did compile unchanged

Design §2.3 predicted this from reading; confirmed by compiling. The single production call site
(`app.rs`) is the only edit, and it does not drop the value on the floor (design §12.5's concern).

---

## 4. Non-vacuity: the mutations the design named, each run

Each mutation was applied, `cargo test -p hickeyfield-core --lib` run, then reverted.

| mutation | result |
|---|---|
| `FAL_ROUTE_MODES` Seedance back to `&[InputMode::Text]` | `use_case::tests::the_models_imported_from_fal_land_on_the_right_tab` **FAILED** (`use_case.rs:315`) |
| LTX `duration` back to `ValueSpec::Number` | `registry::tests::ltx_2_5_offers_only_the_durations_fal_enumerates` **FAILED** (`registry.rs:3034`) |
| drop `pinned.insert(name.clone())` in `parse` | 3 FAILED: `a_const_is_recorded_as_a_one_member_enum`, `a_const_beside_an_enum_wins`, `a_pinned_field_is_written_to_the_value_the_endpoint_fixes` |
| drop the whole `const` branch in `parse` | 8 FAILED, incl. all three capability axis tests and `a_pinned_prompt_or_media_key_is_refused_not_rewritten` |
| `scalar_text(v)` → `v.as_str()` in the enum loop (i.e. the old string-only filter) | 3 FAILED: `an_integer_enum_is_stringified_not_dropped`, `a_pure_integer_enum_is_kept`, `a_null_member_is_skipped_without_voiding_the_enum` |

Note for the TEST agent on the third row: dropping only the `pinned.insert` does **not** fail the
capability axis tests, and that is correct rather than a gap — `fal_axis` reads `enums`, which still
carries the one-member entry. The set exists to distinguish "fal offers one value" from "fal pins
this", which is what the submit path and the audit consume; the axis is right either way. The
fourth row is the mutation that breaks the axis.

---

## 5. Known limitations — not fixed here, stated rather than hidden

1. **Offline is still the weak mode for any model whose endpoints disagree.** D-1 closes the one
   case that exists today (Seedance 2.5), but the general shape remains: one `ModelSpec` serves N
   endpoints, and `capability::from_catalog` cannot know which one. A per-endpoint offline fallback
   (or a bundled schema snapshot) is a slice of its own. `pin_conflicts` is the guard that will
   catch the next instance.
2. **`cargo deny check` reports `advisories FAILED` — pre-existing and unrelated.** Verified by
   stashing every change and re-running at `HEAD`: identical failure. It is
   `warning[advisory-not-detected]` for `RUSTSEC-2024-0429` in `deny.toml:77` (a Linux-only `glib`
   ignore that no longer matches any node). No dependency was added or changed by this slice.
   `bans ok, licenses ok, sources ok`.
3. **AC7's "app builds + launches" is only half-proved.** `cargo build --workspace`,
   `cargo clippy --all-targets`, `ui/pnpm build` and the full test suite all pass, so the Tauri
   binary and the front end both compile and link. `scripts/build-macos.sh` was **not** run: it
   requires a `Developer ID Application` signing identity and a notarization credential, neither of
   which exists in this session, and there is no display to launch a window on. No `ui/` file and no
   startup path changed in this slice, but I cannot claim the `.app` was launched.
4. **`PerSecond` still quotes $0.00 for a `Billable` with no duration** (`cost.rs:182`). Unreachable
   for `veo3_1_extend` because the one-member enum always supplies 7, but the class stays on the
   backlog as design §6.3 recorded.
5. **`fps` is read but not offered.** LTX publishes `fps [24,25,50]`; it is now visible to the
   parser and lands in `unmapped_enums`, which nothing renders. Out of scope (design N1/N4 class).
6. **The advisory sentence is not covered by a test.** `app.rs`'s `Vec<Pinned>` → `job.advisories`
   loop has no unit test — `submit` there is not currently unit-testable without a live route, and I
   did not restructure it to make one. `reconcile` returning the rows *is* tested; the sentence that
   quotes them is compile-checked only.
7. **Per-property `default` is still not parsed** (design N1), `anyOf` ranges are still not clamped
   in-app (N2), and the media-key audit check (N4) is not written. All explicit non-goals.

---

## 6. Reproduce the whole thing

```
source "$HOME/.cargo/env"
export hickeyfield_FAL_KEY=x hickeyfield_HIGGSFIELD_KEY=x hickeyfield_GOOGLE_KEY=x \
       hickeyfield_OPENAI_KEY=x hickeyfield_XAI_KEY=x hickeyfield_BFL_KEY=x \
       hickeyfield_RECRAFT_KEY=x hickeyfield_VAIG_KEY=x     # dummies; no real key exists here

cargo test --workspace                                      # 801 passed, 0 failed
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
./scripts/lint-provenance.py
env -u hickeyfield_FAL_KEY -u FAL_KEY cargo run -q -p hickeyfield-core --example audit_fal
cargo run -q -p hickeyfield-core --example fal_diff -- --offline
cd ui && pnpm test && pnpm build
```

**Never** `cargo run --example first_light` — it spends money. It was not run here.
