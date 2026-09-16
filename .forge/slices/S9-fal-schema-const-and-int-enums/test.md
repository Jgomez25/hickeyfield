# S9-fal-schema-const-and-int-enums — TEST (adversarial verification)

Author: verifier (not the implementer). Every block below is output I captured this
session, not the implementer's paste. Environment: dummy keys exported; Node 24 on PATH;
`first_light.rs` never run; nothing paid submitted.

## The claim under test (implement.md, verbatim)

> AC1 — the parser … `parse` now: reads a scalar `const` as a one-member `enums` entry **and**
> adds the field to `pinned` … otherwise stringifies every enum member through the **existing**
> `scalar_text` … all-or-nothing … a `null` member is skipped without voiding it …

> AC7 — the gate … `cargo test --workspace` → 801 passed, 0 failed … audit `11 / 2 / 21 / 9`,
> PINNED 0, UNREACHABLE 0, CHECKED 66 … `fal_diff --offline` "Registry reaches 66" … `pnpm test`
> 232 passed … `pnpm build` ✓

The full AC-by-AC claim is at `implement.md:105-330`; its §3 openly deviates from design §6.4 by
narrowing Seedance 2.5's spec-level `aspect_ratio` enum to `["auto"]`. That deviation is the item
I probed hardest (see Break attempts §3).

## Commands

### AC7 gate — workspace tests, fmt, clippy, provenance

```
$ cargo test --workspace   (per-binary result lines)
  src/lib.rs (hickeyfield_core):        676 passed; 0 failed; 9 ignored
  examples/audit_fal.rs:                  3 passed; 0 failed
  examples/fal_diff.rs:                   2 passed; 0 failed
  src/lib.rs (hickeyfield_tauri_lib):   120 passed; 0 failed; 1 ignored
  → 676 + 3 + 2 + 120 = 801 passed, 0 failed   (claim: 801/0 — MATCH)

$ cargo fmt --all --check         → FMT CLEAN (exit 0)
$ cargo clippy --workspace --all-targets -- -D warnings  → exit 0, Finished
$ ./scripts/lint-provenance.py
  CDN hostnames     PASS  no third-party media hosts referenced
  Copy provenance   PASS  no shipped string matches the reference corpus (80015 shingles indexed)
  → exit 0
```

### AC7 gate — keyless live audit + offline diff

```
$ env -u hickeyfield_FAL_KEY -u FAL_KEY cargo run -q -p hickeyfield-core --example audit_fal   (exit 0)
=== SILENTLY IGNORED MEDIA — bills for the wrong generation (11) ===
=== MISSING REQUIRED FIELDS — 422 after a round trip (2) ===
=== DROPPED SETTINGS — the control has no effect (21) ===
=== REJECTED OPTIONS — the chip row offers invalid values (9) ===
=== PINNED FIELDS — the endpoint fixes this value and we would send another (0) ===
=== UNREACHABLE — fal serves no schema for this endpoint (0) ===
=== CHECKED — fal answered with a schema (66) ===
  → 11 / 2 / 21 / 9, PINNED 0, UNREACHABLE 0, CHECKED 66   (claim — MATCH)

$ cargo run -q -p hickeyfield-core --example fal_diff -- --offline
fal index: 1491 model(s), bundled snapshot of 2026-09-16. Registry reaches 66 fal endpoint(s).
```

This live run's finding sections are byte-identical to the checked-in `audit_fal-after.log`
(`diff` of everything above CHECKED → "IDENTICAL finding sections"). The three re-landed ids do
NOT appear in any finding section: `grep -icE 'ltx_2_5|veo3_1_extend|seedance_2_5'` over the
finding sections = **0**. (The `veo3_1`, `veo3_1_fast`, `veo3_1_lite` rows that a loose `veo3.1`
grep catches are unrelated pre-existing models, not `veo3_1_extend`.)

### AC7 gate — UI

```
$ cd ui && pnpm test      → Test Files 14 passed (14); Tests 232 passed (232); exit 0
$ cd ui && pnpm build     → dist/assets/index-*.js built; exit 0
```

### AC1/AC2/AC3/AC5/AC6 — named tests

```
$ cargo test -p hickeyfield-core --lib fal_schema     → 33 passed; 0 failed
    incl. a_const_is_recorded_as_a_one_member_enum, an_integer_enum_is_stringified_not_dropped,
    a_pure_integer_enum_is_kept, a_const_beside_an_enum_wins, a_nullable_integer_is_not_enumerated,
    a_null_member_is_skipped_without_voiding_the_enum, an_enum_we_cannot_fully_read_is_recorded_as_nothing,
    a_pinned_field_is_written_to_the_value_the_endpoint_fixes, a_pinned_prompt_or_media_key_is_refused_not_rewritten
$ cargo test -p hickeyfield-core --lib capability::tests  → 36 passed; 0 failed
    incl. a_const_pinned_axis_is_fixed_not_free_form, a_catalogue_default_outside_a_pin_is_dropped,
    one_spec_two_endpoints_and_the_pin_applies_to_only_one_of_them, a_fal_field_with_no_published_enum_is_free_form_not_absent
$ cargo test -p hickeyfield-core --example audit_fal   → 3 passed; 0 failed
    an_endpoint_that_pins_nothing_reports_nothing, a_const_the_spec_contradicts_is_reported,
    a_spec_that_mirrors_the_pin_reports_nothing
```

### Live schemas re-fetched myself (all 4 re-landed endpoints)

The live fal documents match implement.md's pasted fragments exactly:
- `lightricks/ltx-2.5/text-to-video/pro`: `duration enum [6,8,10,auto]`, `aspect [16:9,9:16]`, `resolution [720p,1080p]`, `fps [24,25,50]`
- `lightricks/ltx-2.5/image-to-video/pro`: `aspect [auto,16:9,9:16]`, `image_url`, `end_image_url`
- `fal-ai/veo3.1/extend-video`: `duration const "7s"`, `resolution const "720p"`, `aspect enum [auto,16:9,9:16]`, `video_url`
- `bytedance/seedance-2.5/image-to-video`: `aspect_ratio const "auto"`, `duration enum [auto,4..30]`, `resolution [480p,720p,1080p]`

## Break attempts

### (1) End-to-end body via the shipped library, against the LIVE schema

Scratch crate outside the repo, path-dep on `hickeyfield-core`, calling `capability::for_route`
(with real `fal_schema::for_endpoint`) and `fal_schema::reconcile`. ONLINE capability axes:

```
ltx_2_5 (t2v):        duration [6,8,10,auto]  resolution [720p,1080p]  aspect [16:9,9:16]     all free_form=false
ltx_2_5_animate (i2v):duration [6,8,10,auto]  resolution [720p,1080p]  aspect [auto,16:9,9:16] all free_form=false
veo3_1_extend (video):duration ["7s"]          resolution ["720p"]      aspect [auto,16:9,9:16] all free_form=false
seedance_2_5 (i2v):   aspect ["auto"]  (pin honoured)
seedance_2_5 (t2v):   aspect [auto,21:9,16:9,4:3,1:1,3:4,9:16]  (7 ratios — online t2v UNCHANGED)
```

reconcile against the live schema (what fal would actually receive):

```
[ltx t2v clamped 6.0]        OK   body duration:"6"                pins=[]
[ltx t2v raw 5.0]            REFUSED: takes duration of 6,8,10,auto, not 5   (a Refusal, NOT a fal-422 send)
[ltx animate i2v 8.0]        OK   body duration:"8"                pins=[]
[veo extend stale 5.0/1080p] OK   duration:"7s", resolution:"720p" pins=[(duration,5,7s),(resolution,1080p,720p)]
[veo extend clamped 7.0/720p]OK   duration:"7s", resolution:"720p" pins=[]
[seedance i2v aspect 16:9]   OK   aspect_ratio:"auto"              pins=[(aspect_ratio,16:9,auto)]
```

So for all four re-landed paths reconcile emits only values the live schema enumerates/pins —
fal cannot 422 on duration/resolution/aspect_ratio. The `duration 5.0 → 6` clamp is UI-side
(`ui/src/lib/variants.ts` `resolveSettings`→`nearest(5,[6,8,10])`; ties go smaller, so =6);
verified by reading the code and by `pnpm test` green. If 5.0 ever reached reconcile unclamped it
is a clean Refusal, not a bad send — AC2 ("reconcile never sends a value fal would 422") holds
in both directions.

### (2) Wan 3.0 Prime must stay free-form

Offline (`for_route` with `|_| None`): `wan3_0_prime duration values=[] free_form=true`. Parser
does not descend into its `anyOf[{integer 2..30},{null}]`. Fixture test
`a_nullable_integer_is_not_enumerated` passes. No regression.

### (3) The Seedance §6.4 deviation — I measured both paths

```
OFFLINE seedance_2_5 t2v aspect: ["auto"]            (design §6.4 wanted the 7-ratio list here)
OFFLINE seedance_2_5 i2v aspect: ["auto"]            (correct — matches the const pin)
ONLINE  seedance_2_5 t2v aspect: 7 ratios            (recovered from the per-endpoint schema)
ONLINE  seedance_2_5 i2v aspect: ["auto"]            (the pin)
```

Judgment: collapsing OFFLINE t2v aspect to `["auto"]` is a real, narrow reduction, but it does
**not** violate any AC. AC6 asks that a const-pinned axis be shown as fixed and never as a
free-form control whose value is discarded — offline t2v shows a fixed single *valid* value
(`auto` is accepted by the t2v endpoint), free_form=false, so AC6's letter holds. The "never
advertise what fal rejects" rule is about not offering invalid options; showing fewer valid ones
under-advertises but advertises nothing fal rejects. Crucially, honouring design §6.4 verbatim
(keep the 7-member spec enum) puts `seedance_2_5` into REJECTED OPTIONS (10) **and** PINNED
FIELDS (1) — I confirmed those exact rows are what the implementer's before-change run shows —
which would violate AC4 ("no re-landed id in a finding section") and AC5 ("passing the fixed
audit with zero findings"). So the deviation is not merely defensible; the §6.4 alternative would
fail hard ACs. The implementer disclosed it (implement.md §3, D-1) with a one-line revert path. I
accept it and do NOT think §6.4 should have been honoured as written.

### (4) Veo 3.1 Extend cost at fixed 7s, audio on/off — computed independently

```
veo3_1_extend 7s audio-off: Some(1.4000000000000001)   → $1.40
veo3_1_extend 7s audio-on:  Some(2.8000000000000003)   → $2.80
caps durations=[7.0] resolutions=["720p"] audio=true
veo3_1_extend seconds=None: Some(0.0)                  → limitation #4 (PerSecond→$0.00), unreachable
                                                         because durations=[7.0] forces 7; acknowledged.
```

Matches fal's published pair; the fixed-7s duration is not offered as an adjustable axis.

### (5) Mutation sanity — scratch copy of the repo, tests re-run per mutation

```
drop the whole `const` branch in parse            → 8 FAILED
  (a_const_is_recorded…, a_const_beside_an_enum_wins, a_pinned_field_is_written…,
   a_pinned_duration_accepts_the_bare_number, a_pinned_prompt_or_media_key_is_refused…,
   + the 3 capability axis tests)
scalar_text(v) → v.as_str() in the enum loop      → 3 FAILED
  (an_integer_enum_is_stringified_not_dropped, a_pure_integer_enum_is_kept,
   a_null_member_is_skipped_without_voiding_the_enum)
seedance FAL_ROUTE_MODES back to [Text]           → the_models_imported_from_fal_land_on_the_right_tab FAILED
LTX duration back to ValueSpec::Number            → ltx_2_5_offers_only_the_durations_fal_enumerates FAILED
```

Every mutation the implementer named fails the test it named. The tests are non-vacuous.

### (6) AC4 — before/after audit diff, checked myself

```
$ diff (before finding sections) (after finding sections)
95a96,98
>   === PINNED FIELDS — the endpoint fixes this value and we would send another (0) ===
>     none
```

The only change between before and after (below CHECKED) is the added empty PINNED header;
SILENTLY IGNORED MEDIA / MISSING REQUIRED / DROPPED / REJECTED counts are unchanged (11/2/21/9),
CHECKED 62→66. No new finding on a pre-existing endpoint was hidden by the Seedance narrowing.
UNREACHABLE stays 0. My live run reproduces the after-log exactly.

### Other checks

- No dependency added: diff vs main touches only `crates/hickeyfield-core/Cargo.toml` (adds the
  `[[example]] audit_fal / test = true` target); `Cargo.lock` unchanged. So the pre-existing
  `cargo deny` advisory failure (implement.md limitation #2) is unrelated to this slice.
- `git status` at finish shows only `.forge/.../test.md` changed by me (review.md/security.md are
  the concurrent verifiers').

## Findings (none blocking)

- F-note-1: OFFLINE Seedance t2v aspect narrows from 7 ratios to `["auto"]` (design §6.4
  deviation). Judged acceptable — no AC violated, no 422 hazard, disclosed, and the §6.4
  alternative would fail AC4/AC5. Documented above.
- F-note-2: `veo3_1_extend` quotes $0.00 if `Billable.seconds` is `None` (cost.rs PerSecond) —
  unreachable because durations=[7.0]; on the pre-existing backlog. Not this slice's regression.
- F-note-3: the `app.rs` advisory sentence built from `Vec<Pinned>` has no unit test
  (implement.md limitation #6). `reconcile` returning the pins IS tested and I confirmed the pins
  are produced (probe §1); the consumption loop is compile-checked. Not an AC failure.

All seven acceptance criteria hold under adversarial re-run: parser reads const + stringifies
integer enums and preserves free-form (AC1); reconcile coerces/refuses without ever sending a
fal-422 value (AC2); audit check 5 exists and its fixtures pass (AC3); re-probe shows finding
counts unchanged, UNREACHABLE 0, no re-landed id in findings (AC4); the four paths re-land with
correct enums, routes, and the $1.40/$2.80 cost (AC5); pinned axes render fixed, not free-form
(AC6); and the full gate is green with 801/0 tests, clean fmt/clippy/provenance, 232 UI tests and
a build (AC7).

VERDICT: PASS
