# S9-fal-schema-const-and-int-enums — DESIGN

Contract: `.forge/slices/S9-fal-schema-const-and-int-enums/slice.md` (AC1–AC7).
Upstream evidence this design is built on: `.forge/slices/S8-fal-catalogue-sync/review.md`
(B1, B2, B3, RE-REVIEW 1 B4, RE-REVIEW 1 §4), `.forge/slices/S8-fal-catalogue-sync/test.md`
(§2.3, §2.4, RE-TEST 1 B2/B3, RE-TEST 2 F3/F4, RE-TEST 3), `.forge/backlog.md:200-210`,
`.forge/objective.md` (§A no-regression gates). S8's `design.md` is the house style followed here.

Slice shape: **one parser change + one submit-policy change + one audit check + a re-land of three
withdrawn model ids and one withdrawn input mode.** No new module, no new dependency, and — by
design, see §4 — **no file under `ui/` changes**.

---

## 0. Session constraint, stated before anything is claimed

This design was authored **without shell access**: no `curl`, no `git show`, no `cargo`. Every
`file:line` below was read from the working tree at `ce8b283` in this session and is first-hand.
Every **live fal schema fragment** below is quoted from S8's recorded evidence — which was measured
keyless by the S8 reviewer/verifier and cross-checked between two independent passes — and is cited
to the file and line it was recorded in.

**MUST, first task of IMPLEMENT:** re-fetch the four affected endpoint documents keyless and paste
them into `implement.md` before writing code:

```
curl -s "https://fal.ai/api/openapi/queue/openapi.json?endpoint_id=lightricks/ltx-2.5/text-to-video/pro"
curl -s "https://fal.ai/api/openapi/queue/openapi.json?endpoint_id=lightricks/ltx-2.5/image-to-video/pro"
curl -s "https://fal.ai/api/openapi/queue/openapi.json?endpoint_id=fal-ai/veo3.1/extend-video"
curl -s "https://fal.ai/api/openapi/queue/openapi.json?endpoint_id=bytedance/seedance-2.5/image-to-video"
```

If any of the four disagrees with §1.1 below, **the schema wins and this design is amended in
`implement.md`**, not the other way round. A design that asserts a vendor's API from memory is the
exact failure `fal_schema.rs:1-23` was written after.

`first_light.rs` MUST NOT be run in this slice. No key exists; nothing here spends money.

### 0.1 Ground truth, as recorded

| Fact | Recorded at |
|---|---|
| `bytedance/seedance-2.5/image-to-video` → `aspect_ratio` = `{ "default": "auto", "type": "string", "title": "Aspect Ratio", "description": "The aspect ratio of the generated video. Always \"auto\" for image-to-video", "const": "auto" }` | `S8/test.md:1038-1043`, independently in `S8/review.md:344-347` |
| `fal-ai/veo3.1/extend-video` → `duration: {"type":"string","default":"7s","const":"7s"}` and `resolution: {"type":"string","default":"720p","const":"720p"}`; both keys are *accepted*, neither is an `enum` | `S8/review.md:86-89` |
| `fal-ai/veo3.1/extend-video` media keys `['video_url']`, required `['prompt','video_url']` — binds on the key `media::fal_keys` writes for `MediaRole::Video` | `S8/test.md:373` |
| LTX 2.5 `duration` is a closed **mixed** enum `{6, 8, 10, "auto"}` on both `lightricks/ltx-2.5/{text,image}-to-video/pro`; today's parser records `["auto"]` alone | `S8/review.md:45-61`, slice.md:12-14 |
| `lightricks/ltx-2.5/image-to-video/pro` media keys `['image_url','end_image_url']`, required `['prompt','image_url']` — both are keys the binder writes (`media.rs:197`, `media.rs:215`) | `S8/test.md:367` |
| LTX 2.5 resolution enum `720p,1080p`; price $0.12/s @720p, $0.17/s @1080p (fal also lists a 4K tier the spec makes unselectable) | `S8/test.md:439-440`, `S8/review.md:203` |
| `fal-ai/veo3.1/extend-video` price $0.20/s audio-off, $0.40/s audio-on | `S8/test.md:441` |
| `alibaba/wan-3.0-prime/*` `duration` = `{"default":5,"anyOf":[{"type":"integer","minimum":2,"maximum":30},{"type":"null"}]}` — free-form, **must not** become an enum | `S8/test.md:1097-1099` |
| Across the 12 endpoints the 11 landed models reach, **exactly one** `const` and **zero** non-string enum members | `S8/review.md:437-456`, `S8/test.md:861-879` |
| Baseline at HEAD: `cargo test --workspace` 779 passed / 0 failed; audit `11 / 2 / 21 / 9`, `UNREACHABLE 0`, `CHECKED 62`; `fal_diff --offline` "Registry reaches 62" | `S8/review.md:658-673` (RE-REVIEW 3), `S8/test.md:1256-1279` |

### 0.2 The defect chain, re-read in the working tree this session

1. `crates/hickeyfield-core/src/fal_schema.rs:216-226` (`parse`) keeps only `v.as_str()` members of
   `properties[*].enum`, drops the entry when the survivors are empty, and never reads `const`.
2. `fal_schema.rs:146` (`reconcile`) iterates `schema.enums`, so a field with no recorded enum is
   passed through verbatim — and a field with a *truncated* enum refuses every real value
   (`coerce`, `:80-100`, returns `None` ⇒ `Refusal` ⇒ `src-tauri/src/app.rs:438-439` ⇒
   `JobError::Permanent`).
3. `crates/hickeyfield-core/src/capability.rs:603-619` (`fal_axis`) reads `schema.enums`; an absent
   entry with the field still `accepted` yields `Axis::supported(vec![], …)` ⇒ `is_free_form()`
   true (`capability.rs:283-285`).
4. `src-tauri/src/commands.rs:291-317` turns that into `CapabilitiesDto { supports_*: true,
   values: [] }`; `ui/src/components/ChipRow.tsx:96` hides the aspect chip on an empty list and
   `ui/src/lib/variants.ts:145-147` cannot clamp against an empty list, so a stale value survives.
5. `crates/hickeyfield-core/examples/audit_fal.rs:135-154` (check 4) needs *both* sides to
   enumerate, and its private `schema_enum` (`:191-214`) re-implements the same string-only filter —
   so the gate is blind in precisely the two cases that matter.

---

## 1. AC1 — the parser

### 1.1 D1 (decision): enrich `enums` **and** record which fields are pinned

`EndpointSchema` (`fal_schema.rs:31-42`) gains **one** field:

```rust
pub struct EndpointSchema {
    pub accepted: BTreeSet<String>,
    pub required: BTreeSet<String>,
    pub enums: BTreeMap<String, Vec<String>>,
    /// Fields the endpoint pins to a single value with `const`.
    ///
    /// Also present in `enums` as a one-member list, because that is what every
    /// consumer already reads; this set is what distinguishes "fal offers
    /// exactly one value" from "fal pins this and will 422 anything else",
    /// which the submit path and the audit each say differently.
    #[serde(default)]
    pub pinned: BTreeSet<String>,
}
```

Rules:

- A `const` MUST be recorded as a **one-member `enums` entry** *and* its field name added to
  `pinned`.
- `pinned` MUST default on deserialize (`#[serde(default)]`), because `EndpointSchema` is
  `Serialize`/`Deserialize` (`fal_schema.rs:31`) and a future bundled snapshot of schemas must not
  fail to load for lack of a field.
- Nothing may read `pinned` to decide *which values are legal* — that is `enums`' job. `pinned` only
  decides **what to do about an illegal one** (§2) and **what to call it in a report** (§3).

**Rejected alternative — a standalone `pinned: BTreeMap<String,String>` and no change to `enums`.**
Four call sites already read `enums` and would each need to learn about a second map:
`coerce` (`fal_schema.rs:81`), `reconcile` (`:146`), `fal_axis` (`capability.rs:607`) and
`from_fal`'s `unmapped_enums` (`capability.rs:480-485`). Two structures answering "what values does
this field take" is two sources of truth about one question, which is the shape of the bug being
fixed. Recording the pin in `enums` makes every existing consumer correct *by not changing it* —
`fal_axis` in particular becomes right with **zero edits** (§4).

**Rejected alternative — `enums` only, no `pinned` set.** Then a one-member enum and a `const` are
indistinguishable, and AC3's second check ("a const-pinned field the app would send with a different
value") cannot name what it found, nor can the submit path tell a user "this endpoint always renders
at 7s" rather than "7s is the only option". The set costs one `BTreeSet<String>` and six test-fixture
literals (§1.4).

### 1.2 The parse algorithm

Rewrite only the enum block of `fal_schema.rs:213-226`. For each `(name, prop)` in
`properties`:

1. **`const` first.** If `prop.get("const")` is a scalar (string / number / bool), record
   `enums[name] = vec![stringify(const)]` and `pinned.insert(name)`. A `const` that is an object,
   array or `null` is **ignored entirely** (no enum, no pin) — we do not model it and pretending to
   would be worse than silence.
2. **`enum` otherwise.** If the field is not pinned and `prop["enum"]` is an array, stringify every
   member. **All-or-nothing (MUST):** if *any* member fails to stringify (object, array), record
   **no** enum for that field. A partially-recorded enum is exactly the LTX 2.5 failure
   (`S8/review.md:51-61`): it looks authoritative and refuses valid values.
   A `null` member is **skipped without voiding the entry** — `null` in a fal enum means "may be
   omitted", not a value the app can send — but an enum whose members are *all* `null` records
   nothing.
3. **Const alongside enum:** the `const` wins and the field is pinned. `const` is the narrower
   statement, and fal generates these from pydantic `Literal[...]`, so the enum in that case is the
   wider declared type, not a second offer.
4. `anyOf` / `oneOf` / `allOf` are **not** descended into (unchanged behaviour). This is what keeps
   Wan 3.0 Prime's `{"anyOf":[{"type":"integer","minimum":2,"maximum":30},{"type":"null"}]}`
   free-form, and AC1 requires a test for it.

`stringify` MUST be the existing `scalar_text` (`fal_schema.rs:180-190`), moved above `parse` (or
left where it is — order is irrelevant in Rust; no other change). Reusing it is load-bearing: it
already renders a whole-valued float as an integer (`6.0 → "6"`, `:185`), which is the same rule the
body side uses, so **the two sides cannot spell a number differently.** Writing a second stringifier
here would reintroduce the `"4.0"`-matches-nothing bug at `:178-179`.

### 1.3 Worked examples

| fal property JSON | recorded `enums` | `pinned` |
|---|---|---|
| `{"type":"string","const":"7s","default":"7s"}` | `duration → ["7s"]` | `{duration}` |
| `{"type":"string","const":"auto","default":"auto"}` | `aspect_ratio → ["auto"]` | `{aspect_ratio}` |
| `{"enum":[6,8,10,"auto"]}` | `duration → ["6","8","10","auto"]` | `{}` |
| `{"enum":[480,720,1080]}` | `resolution → ["480","720","1080"]` | `{}` |
| `{"enum":["720p","1080p"]}` | `resolution → ["720p","1080p"]` | `{}` (unchanged from today) |
| `{"anyOf":[{"type":"integer","minimum":2,"maximum":30},{"type":"null"}],"default":5}` | *(absent)* | `{}` |
| `{"enum":[4,null,8]}` | `duration → ["4","8"]` | `{}` |
| `{"enum":[{"w":1}, "auto"]}` | *(absent — all-or-nothing)* | `{}` |

Worked end-to-end example (LTX 2.5, the B1 failure): fal declares `duration` `{6,8,10,"auto"}`.
After the fix `enums["duration"] = ["6","8","10","auto"]`. The registry spec declares
`en(&["6","8","10","auto"])` (§6.2). `capability::fal_axis` yields `values = ["6","8","10","auto"]`;
`CapabilitiesDto::from_capability`'s `num` closure (`commands.rs:293-298`) parses three of them to
`[6.0, 8.0, 10.0]` and silently drops `"auto"`, so the chip row shows 6/8/10;
`resolveSettings` (`variants.ts:131-136`) clamps the app-wide default `5` to `nearest(5,[6,8,10])`
= **6**; the body carries `duration: 6.0`; `reconcile` → `coerce("duration","6")` matches member
`"6"` and writes the **string** `"6"`. Today every one of those submits is refused before the
request is built.

### 1.4 Collateral the implementer will hit

Adding a field to `EndpointSchema` breaks every struct literal. There are **six**, all in tests:
`fal_schema.rs:396`, `:413`, `:432`, `:445`, `:470` and `capability.rs:761`. Each MUST gain
`pinned: Default::default()` (or `..Default::default()`); none may be deleted or `#[ignore]`d.

---

## 2. AC2 — coerce, reconcile, and what happens at submit

### 2.1 `coerce` is NOT changed

`fal_schema.rs:80-100` keeps its exact semantics: exact match, case-insensitive match, unit-suffix
match, else `None`. A pinned field is just a one-member list to it, so `coerce("duration","7")`
against `["7s"]` already returns `Some("7s")` through the suffix rule at `:90-97`, and
`coerce("aspect_ratio","16:9")` against `["auto"]` already returns `None`. The numeric-duration
requirement in AC2 ("6.0 must match member `6`") is satisfied by `scalar_text` at `reconcile`'s
boundary (`:153`, `:180-190`) and is already covered by
`a_numeric_duration_becomes_the_string_the_endpoint_enumerates` (`:485-495`) — the new part is only
that the enum now *exists* for integer-valued endpoints.

### 2.2 D2 (decision): a const-pinned field is **coerced to the pin, loudly** — except on user content

Three candidate policies were on the table (slice.md AC2 leaves the choice to design):

| policy | effect on Seedance 2.5 i2v with a stale `aspect: "16:9"` | verdict |
|---|---|---|
| refuse (today's behaviour for any non-member) | `JobError::Permanent` — a tile that cannot generate | **rejected**: this is precisely the dead-tile outcome S8 withdrew four paths to avoid (`S8/review.md:262-268`) |
| drop the key, let fal default | works, but re-creates the "omitted field = a third opinion" failure `aspect.rs:36-42` was written to end, and the job record would not state the shape | **rejected** |
| **write the pinned value and record an advisory** | body gets `"auto"`, the job carries "Seedance 2.5 always renders image-to-video at `auto`" | **chosen** |

Why this is not the silent substitution the module forbids (`fal_schema.rs:137-141`): substitution
is wrong when a *neighbour* is chosen in place of the user's pick — 6s instead of the 8s they paid
for. A `const` endpoint offers **no** neighbour; the pinned value is the only render it can produce,
the capability layer already shows it as the single option (§4), and the job carries a sentence
saying so. Refusing would cost the user a generation fal would happily have run.

**Field classes (MUST):**

- **Class A — settings fields** (everything that is not `prompt` and not in
  `EndpointSchema::media_keys()`, `fal_schema.rs:54-65`): coerce to the pin, emit a note.
- **Class B — `prompt` and any media key**: **never rewritten.** If such a field is pinned and the
  body's value differs, return a `Refusal`. Overwriting the user's prompt or their uploaded URL with
  a schema constant would be the module's founding failure committed in a new place. No live
  endpoint does this today; the guard exists because D1 makes it *possible* for the first time.
- **Class C — a pinned field the body does not carry**: nothing happens. `reconcile` only touches
  keys present in the body (`:147-150`), so an endpoint that pins a field the app never sends stays
  fal's business.

### 2.3 Signature change and the call site

```rust
/// A field the endpoint pins, rewritten to the value it pins.
pub struct Pinned { pub field: String, pub from: String, pub to: String }

pub fn reconcile(
    schema: &EndpointSchema,
    body: &mut serde_json::Map<String, serde_json::Value>,
) -> Result<Vec<Pinned>, Refusal>;
```

- `Ok(vec![])` is the common case, so every existing test that calls `.unwrap()` / `.expect(...)` /
  `.expect_err(...)` (`fal_schema.rs:493-553`) still compiles unchanged — checked by reading all of
  them. Only the **one** production call site changes:
  `src-tauri/src/app.rs:438-439`, which today maps the error and discards the unit. It becomes: map
  the error exactly as now, and for each `Pinned` push a sentence onto `job.advisories` in the voice
  already used four lines below (`app.rs:441-453`), e.g.
  `"Veo 3.1 Extend always renders at 7s on this endpoint — your 5s setting does not apply"`.
- Ordering inside `reconcile` MUST be: pinned handling first, then the existing enum pass. The
  pinned pass writes the member; the enum pass then finds it already equal and rewrites it as the
  enum's own string, so the type half of the contract (`:157-162`) still holds.

### 2.4 Worked example (the B2 failure, end to end)

`veo3_1_extend`, body built from default settings today:
`{"duration":5.0,"prompt":"…","resolution":"1080p"}` (`S8/review.md:93`).

After this slice: the spec no longer offers 1080p or a free duration (§6.2), so the UI clamps to
7s/720p **before** submit; the body is `{"duration":7.0,"prompt":"…","resolution":"720p","video_url":…}`;
`reconcile` writes `"7s"` (suffix rule) and `"720p"` (already equal, retyped); `Pinned` is empty
because nothing had to be rewritten. If a *stale recipe* carries `duration: 5.0`, the pinned pass
rewrites it to `"7s"` and the job says so. In neither case does fal see a value it 422s.

---

## 3. AC3 — the audit

`crates/hickeyfield-core/examples/audit_fal.rs`.

### 3.1 Check 4 uses the enriched enums, and the duplicate fetcher dies

- `:141` `schema_enum(&endpoint, flag)` MUST become `schema.enums.get(flag).cloned().map(|v| v.into_iter().collect::<BTreeSet<_>>())`,
  using the `schema` already fetched and cached at `:63`.
- `fn schema_enum` (`:191-214`) MUST be **deleted**. It is a second, string-only copy of `parse`'s
  enum logic and a second network request per field; leaving it is how the two would disagree again
  (the same duplication argument as `S8/review.md:172-176`).
- The module doc (`:7-16`) gains the fifth check, and `:16`'s wording for check 4 stays.

Consequence to expect and record, not to suppress: check 4 now fires on **pins** too (a spec offering
`1080p` against a `const: "720p"` shows up as a rejected option), and on **integer enums** it
previously could not see. That is the point; §5 says what to do with the new rows.

### 3.2 Check 5 — "the app would send a value this endpoint pins differently"

A pure, unit-testable function in the example (so it can be tested without a network, exactly as
`fal_diff::fal_minus_registry` is — `S8/design.md:137-158`):

```rust
/// Fields this endpoint pins where the app can still produce a different value.
/// `(field, pinned_value, values_we_could_send)`
fn pin_conflicts(spec: &ModelSpec, endpoint: &str, schema: &EndpointSchema)
    -> Vec<(String, String, Vec<String>)>;
```

Semantics (MUST), for each `field` in `schema.pinned` with pin `P`:

1. **The live path.** Build `capability::ModelCapability::from_fal(spec, endpoint, schema)`
   (`capability.rs:458`) and read the axis for `field` when it is one of `MAPPED_FIELDS`
   (`capability.rs:67-71`). Any value in `axis.values` other than `P` is a conflict. (After §1 this
   is structurally impossible for a mapped axis, which is the assertion worth pinning: the check is
   the regression guard on `fal_axis` staying schema-sourced.)
2. **The offline path**, which is what actually bit `veo3_1_extend`: if the schema fetch fails,
   `capability::for_route` falls back to `from_catalog` (`capability.rs:589-594`) and the axis comes
   from the **spec**. So if `spec.flag(field)` (`catalog.rs:220-225`) exists and is either free-form
   (`ValueSpec::Number` / `Integer` / `Text`) or an `Enum` containing a member that does not
   `coerce` to `P`, that is a conflict.
3. `aspect_ratio` is checked even when the spec declares no such flag, because `app.rs:329-336`
   derives the key from the **schema** and then always sends something explicit
   (`app.rs:344-359`) — the exact path that produced B4.

Printed as its own section, between REJECTED OPTIONS and UNREACHABLE, using the existing `section`
helper (`:216-224`):

```
=== PINNED FIELDS — the endpoint fixes this value and we would send another (N) ===
  veo3_1_extend              fal-ai/veo3.1/extend-video
      pins duration = 7s but the app can send [free-form number]
```

`crates/hickeyfield-core/Cargo.toml` MUST gain `[[example]] name = "audit_fal" \n test = true`
beside the existing `fal_diff` target (`Cargo.toml:27-29`), so the fixture test runs inside
`cargo test --workspace` rather than only on demand.

---

## 4. AC6 — capability and UI honesty (and why `ui/` does not change)

**No file under `ui/` is edited by this slice.** That is a claim, so here is the chain, each link
read this session:

1. `capability::fal_axis` (`capability.rs:603-619`) reads `schema.enums`; with the pin recorded it
   returns `Axis::supported(vec![P], …)`. `Axis::supported` (`:257-276`) drops a catalogue default
   that is not in `values`, so Veo's catalogue `1080p` is discarded rather than shown against a
   `720p`-only endpoint. `is_free_form()` (`:283-285`) is now **false**. **Zero edits to
   `capability.rs` production code.**
2. `CapabilitiesDto::from_capability` (`commands.rs:291-317`): `durations` = `num(["7s"])` = `[7.0]`
   (the closure trims a trailing `s`, `:296`); `resolutions` = `["720p"]`; `aspects` = `["auto"]`.
3. `durationLadder` (`ui/src/lib/variants.ts:45-53`) returns `[7]` because `caps.durations` is
   non-empty — the invented `[3,4,5,6,8,10,12]` ladder is not reached. `ChipRow.tsx:71-105` renders
   one chip per axis with exactly one option: **fixed and visible**, which is what AC6 asks for.
4. `resolveSettings` (`variants.ts:124-160`), called on every capability change
   (`ui/src/App.tsx:261-263`), clamps `duration 5 → 7` (`:131-136`), `resolution "1080p" → "720p"`
   (`:141-143`, via `resolutions[0]`), `aspect "16:9" → "auto"` (`:145-147`).
5. The attach-time aspect snap (`App.tsx:332-351`) cannot undo it: `nearestAspect`
   (`ui/src/lib/media.ts:125-146`) skips any option that is not `w:h` (`:138`) and returns `null`
   when nothing matches, and `App.tsx:346` only sets on a non-null snap. With `aspects = ["auto"]`
   the snap is a no-op — the failure mode `S8/review.md:414-416` warned about is closed by the data,
   not by new UI code.
6. `AspectPlan::decide(true, Some("auto"), …)` (`aspect.rs:43-59`) ⇒ `Sent("auto")` ⇒ `app.rs:354-359`
   inserts it ⇒ `reconcile` finds it already equal to the pin.

AC6's Rust test therefore lives in `capability.rs` (§7). **Vitest: N/A — no `ui/` source changes**;
`pnpm test` / `pnpm build` still run as AC7 gates and MUST stay green (232 tests, `S8/test.md:1277-1279`).

One consequence to verify rather than assume: `ModelCapability::unmapped_enums`
(`capability.rs:480-485`) now also receives pinned *unmapped* fields (e.g. a `const` on
`bitrate_mode`). Nothing renders it (grep: it is read nowhere outside `capability.rs`), but the
capability tests around `:880-940` MUST be re-run and updated if one asserts its exact contents.

---

## 5. AC4 — re-probing every existing fal route, and how new findings are triaged

The parser change is **behaviour-changing for models this slice does not otherwise touch**: a field
whose enum was previously invisible (integer members) now exists, so a spec value outside it becomes
a `Refusal` — a *new* permanent failure where there used to be a silent 422 or an accidental
success. Finding those before release is the whole of AC4.

**Procedure (MUST, in order):**

1. Build with the new parser, then run keyless:
   `env -u hickeyfield_FAL_KEY -u FAL_KEY cargo run -q -p hickeyfield-core --example audit_fal | tee .forge/slices/S9-fal-schema-const-and-int-enums/audit_fal-<date>-postfix.log`
2. Diff the section counts against the recorded baseline `11 / 2 / 21 / 9`, `UNREACHABLE 0`,
   `CHECKED 62` (`S8/review.md:666-669`). Only **increases** need triage; `CHECKED` rises to 66 after
   §6.
3. Triage each new row by this table. The rule exists so "log it" cannot be used to wave through a
   money bug:

| new finding | action |
|---|---|
| **PINNED FIELDS** (new check) on any model | **Fix here.** A pin the app contradicts is a guaranteed 422. Fix = mirror the pin in the spec as a one-member enum, or withdraw the mode (the S8 precedent, `media.rs:689-697`). |
| **REJECTED OPTIONS** newly visible because the endpoint enumerates integers | **Fix here** if the model is reachable in the UI: replace the spec's `ValueSpec::Number`/invented enum with fal's list verbatim. |
| **REJECTED OPTIONS** on a model already in the 9-row baseline | Out of scope — already logged (`S8/test.md:325-327`). Restate the row, do not re-fix. |
| **SILENTLY IGNORED MEDIA / MISSING REQUIRED** count moves at all | **Investigate before release.** This slice touches neither; a move means an unexpected coupling. |
| **UNREACHABLE > 0** | **Blocker.** AC4 says it stays 0. |
| Anything else | Log in `implement.md` with the endpoint, the JSON fragment, and why it is not this slice's to fix. |

4. Record a per-endpoint table (endpoint · new `const` · new integer enum · verdict) in
   `implement.md`, in the shape of `S8/review.md:441-453`. That table is the evidence for AC4, not
   the section counts alone.

Expectation, stated so a surprise is visible: S8 scanned all 12 endpoints of the 11 landed models
and found **one** `const` and **zero** non-string enums (`S8/test.md:861-879`). The remaining ~50
endpoints in `CHECKED` were never scanned for this defect class, so a non-zero number of new rows is
the *expected* outcome and is not, by itself, a failure of this slice.

---

## 6. AC5 — the re-land

### 6.1 The counts, exactly

| literal | file:line | from | to | why |
|---|---|---|---|---|
| `FAL_ROUTE_MODES: [...; 48]` | `media.rs:599` | 48 | **51** | three rows re-added (§6.3) |
| `IMPORTED_2026_09: [&str; 11]` | `registry.rs:2817` | 11 | **14** | `ltx_2_5`, `ltx_2_5_animate`, `veo3_1_extend` |
| registry size `+ 27` | `registry.rs:2239` | +27 | **+30** | three new registry ids; the explanatory comment at `:2228-2236` MUST be rewritten (it currently *explains* the withdrawal) |
| `JOB_TYPES` groups | `registry.rs:272-280`, `:286-320` | — | — | outer length stays **16**; `veo3_1_extend` joins `"animate"`, `ltx_2_5` and `ltx_2_5_animate` join `"video"`. Omitting a row trips the `debug_assert` at `registry.rs:464-468` |
| `FAL_MISSING_ROUTES` 3 / `FAL_NO_MEDIA` 10 / `NO_END_FRAME` 8 / `NO_MEDIA_MODELS` 10 | `media.rs:772`, `:793`, `:828`, `use_case.rs:36` | — | **unchanged unless the probe says otherwise** | LTX i2v declares `image_url` + `end_image_url` and Veo extend declares `video_url` (`S8/test.md:367,373`), so none of these lists gains a row. Confirm against the §0 re-fetch; if a row is needed, bump the length. |

Use-case guards, `use_case.rs:283-319`:
- add `("ltx_2_5", UseCase::TextToVideo)`, `("ltx_2_5_animate", UseCase::ImageToVideo)`,
  `("veo3_1_extend", UseCase::EditVideo)` to the positive loop at `:291-306`;
- **flip** the negative at `:317` — `assert!(!supports(&reg["seedance_2_5"], UseCase::ImageToVideo));`
  becomes a positive row `("seedance_2_5", UseCase::ImageToVideo)` in the loop. The comment at
  `:292-294` (which cites the `const` as the reason for withholding it) MUST be rewritten, not left;
- the `kling3_0_pro` / `wan3_0_prime` negatives at `:315-316` stay — different reason (a start-frame
  key the binder cannot write, `S8/test.md:356-362`), not fixed by this slice.
- Note the axes differ: `veo3_1_extend`'s **tab** is `EditVideo` (a `[Video]`-mode route) while its
  **JOB_TYPES** group is `animate` (enhancer off). Both are asserted; neither implies the other.

### 6.2 The three specs

In `picker_only_specs()` (`registry.rs:1542`), built with the `spec()` helper. Every enumerated set
is fal's, verbatim from the §0 re-fetch — a value invented here is the failure this slice exists to
end.

**`ltx_2_5` / `ltx_2_5_animate`** (shared `ltx_flags`, the S8 shape at the old `registry.rs:2067`):
```
("prompt", true, ValueSpec::Text),
("duration", false, en(&["6", "8", "10", "auto"])),   // was ValueSpec::Number — the B1 defect
("resolution", false, en(&["720p", "1080p"])),
// animate only:
("start_image", false, ValueSpec::Media),
("end_image",   false, ValueSpec::Media),
```
Default 6 is delivered by clamping, not by a `default` field: `spec()` sets `default: None`
(`S8/design.md:190`), and `resolveSettings` clamps 5 → `nearest(5,[6,8,10])` = 6
(`variants.ts:114` — ties go to the smaller value, and 5 is nearer 6 than 8). A test pins it (§7).
Routes: `PerSecondTiered { tiers: [(720, 0.12), (1080, 0.17)] }` for both
(`cost.rs:103`, prices from `S8/test.md:439-440`), with a route note stating the unselectable 4K tier.

**`veo3_1_extend`**:
```
("prompt", true, ValueSpec::Text),
("video",  true, ValueSpec::Media),        // shape copied from grok_extend_video, registry.rs:1852-1860
("duration",   false, en(&["7s"])),        // fal's own spelling of the const
("resolution", false, en(&["720p"])),
```
Mirroring the pins as one-member enums (rather than dropping the fields) is deliberate: it makes the
**offline** capability path (`from_catalog`, `catalog.rs:242-268` → `numeric_options`/`parse_seconds`
at `:195-204`, which reads `"7s"` as 7.0) agree with the online one, which is what keeps the price
right when fal's schema endpoint is unreachable. Constraint string: "fal fixes this endpoint at 7
seconds and 720p; the length you add is not adjustable."

### 6.3 D3 (decision): the cost model for a fixed 7s

**Keep `per_second_audio(0.20, 2.0)`** (`registry.rs:523-528`), *because* the duration is pinned to 7
on both capability paths (§6.2), so `Billable::seconds` (`commands.rs:613-625`) is 7.0 and
`CostModel::PerSecond` (`cost.rs:176-195`) quotes `7 × 0.20 = $1.40`, or `$2.80` with audio — fal's
own published pair (`S8/test.md:441`).

`CostModel::Flat { usd: 1.40 }` (`cost.rs:106`) was considered and **rejected**: it cannot express
fal's audio doubling, so it would under-quote by 2× on every audio-on extend — the worse direction,
and the same class as the `PerImageTiered` gap already on the backlog (`S8/test.md:460-473`).

Residual hazard, recorded: `PerSecond` uses `b.seconds.unwrap_or(0.0)` (`cost.rs:182`), so a
`Billable` with no duration quotes **$0.00**. With the one-member enum the settings always carry 7,
so this is unreachable here; it is the pre-existing F2 class (`S8/test.md:475-486`) and stays on the
backlog.

### 6.4 D4 (decision): Seedance 2.5 keeps a 7-member aspect enum on t2v while i2v is pinned

This is the "one spec, two endpoints" case the contract calls out, and the answer is that **the
schema layer is per-endpoint and the spec layer is per-model, so the pin must be applied at the
schema layer only** — which it already is:

- `fal_schema::for_endpoint` is keyed by the **endpoint** (`fal_schema.rs:254`, cache at `:192-195`),
  and `app.rs:311-315` fetches it *after* `resolve_endpoint` (`app.rs:277-286`) has chosen
  `…/text-to-video` or `…/image-to-video`.
- `capability::for_route` takes the `InputMode` (`capability.rs:581-595`) and `commands.rs:509-521`
  passes `Image` when media is attached — the comment at `commands.rs:506-508` records that this
  split already exists for Kling 3.0.
- So with one `ModelSpec` carrying the t2v enum (`registry.rs:1576-1580`), the **i2v** capability is
  `["auto"]` (from the pin) and the **t2v** capability is the 7-member list. The spec's job is only
  to be the offline fallback and the source of `media_flags`.

Therefore the `seedance_2_5` edits are:
- `media.rs:697` → `("bytedance/seedance-2.5", false, &[InputMode::Text, InputMode::Image])`, and the
  comment at `:689-696` (which explains the withdrawal and names this slice) MUST be replaced by one
  sentence recording that the pin is now honoured at the schema layer;
- restore `("start_image", false, ValueSpec::Media)` and `("end_image", false, ValueSpec::Media)` at
  `registry.rs:1564-1565`, deleting the placeholder comment;
- delete the second constraint string at `registry.rs:1554-1556` ("Text-to-video only for now …");
  keep the first (4–30s);
- leave the `aspect_ratio` enum at `:1576-1580` **exactly as it is** — it is fal's t2v list verbatim
  (`S8/test.md:385`) and changing it would break the mode that works.
- The audit's `SILENTLY IGNORED MEDIA` count MUST stay **11**: check 1 fires only when *no* mode
  takes media (`audit_fal.rs:75-91`), and with `Image` re-claimed the i2v endpoint does. This is the
  reverse of the N6/F3 move recorded at `S8/test.md:1145-1163`.

**The path that MUST be verified by hand at implement time** (it is the one this design cannot
prove from files alone): with `hasMedia = true`, `model_capabilities` → `for_route(…, InputMode::Image, fal_schema::for_endpoint)`
→ `from_fal` on `bytedance/seedance-2.5/image-to-video` → `aspects == ["auto"]`, and with
`hasMedia = false` the same call on `…/text-to-video` → the 7-member list. Reproduce it with the
throwaway probe pattern the S8 verifier used (`S8/test.md:804-816`), in a scratch crate, never in
the repo.

### 6.5 Order of work

1. Parser + `pinned` + fixtures (§1). 2. `reconcile`/`Pinned` + `app.rs` advisory (§2).
3. Capability test only (§4). 4. Audit checks 4/5 + `Cargo.toml` (§3). 5. **Re-probe and triage**
(§5) — before any re-land, so a new finding cannot be mistaken for one the re-land caused.
6. Re-land (§6). 7. Docs (§9). Each step leaves `cargo test --workspace` green.

---

## 7. Test plan — every AC to a named test

All Rust tests live beside the code they test, in the existing `#[cfg(test)] mod tests` blocks.

| AC | test | file | asserts |
|---|---|---|---|
| AC1 | `a_const_is_recorded_as_a_one_member_enum` | `fal_schema.rs` | fixture with `{"type":"string","const":"7s","default":"7s"}` ⇒ `enums["duration"] == ["7s"]` **and** `pinned.contains("duration")` |
| AC1 | `an_integer_enum_is_stringified_not_dropped` | `fal_schema.rs` | fixture `{"enum":[6,8,10,"auto"]}` ⇒ `["6","8","10","auto"]` — the LTX 2.5 document shape |
| AC1 | `a_pure_integer_enum_is_kept` | `fal_schema.rs` | `{"enum":[480,720,1080]}` ⇒ `["480","720","1080"]` |
| AC1 | `a_const_beside_an_enum_wins` | `fal_schema.rs` | both present ⇒ one member + pinned |
| AC1 | `a_nullable_integer_is_not_enumerated` | `fal_schema.rs` | the Wan 3.0 Prime `anyOf` fixture verbatim ⇒ `enums` has no `duration`, `pinned` empty |
| AC1 | `an_enum_we_cannot_fully_read_is_recorded_as_nothing` | `fal_schema.rs` | a member that is an object ⇒ no entry (all-or-nothing) |
| AC1 | `published_enums_are_captured` (existing, `:330`) | `fal_schema.rs` | unchanged — string enums must not move |
| AC2 | `a_pinned_field_is_written_to_the_value_the_endpoint_fixes` | `fal_schema.rs` | body `aspect_ratio:"16:9"` + pin `"auto"` ⇒ body `"auto"`, one `Pinned{from:"16:9",to:"auto"}` |
| AC2 | `a_pinned_prompt_or_media_key_is_refused_not_rewritten` | `fal_schema.rs` | pinned `prompt` / `image_url` ⇒ `Err(Refusal)`, body untouched |
| AC2 | `a_numeric_duration_matches_an_integer_enum_member` | `fal_schema.rs` | body `6.0` vs `["6","8","10","auto"]` ⇒ `"6"` (the B1 submit, now passing) |
| AC2 | `a_pinned_duration_accepts_the_bare_number` | `fal_schema.rs` | body `7` vs pin `"7s"` ⇒ `"7s"`, **no** `Pinned` row (nothing was overridden) |
| AC2 | existing `a_body_value_the_endpoint_does_not_offer_is_refused_not_substituted` (`:518`) | `fal_schema.rs` | unchanged — a genuine non-member is still refused |
| AC3 | `a_const_the_spec_contradicts_is_reported` | `examples/audit_fal.rs` (new `mod tests`) | `pin_conflicts` on a hand-built const fixture + a spec offering `1080p` ⇒ one row naming `resolution`, `720p`; and returns empty when the spec mirrors the pin. **No network** |
| AC4 | (evidence, not a test) | `implement.md` | the post-fix keyless audit log + the per-endpoint triage table of §5; `UNREACHABLE 0` |
| AC5 | `every_imported_model_resolves_to_an_exact_fal_endpoint` (existing, driven by `IMPORTED_2026_09`, `registry.rs:2839`) | `registry.rs` | picks up the three re-landed ids for free |
| AC5 | `every_imported_model_has_a_fal_route_with_a_real_adapter` (existing, `:2881`) | `registry.rs` | same |
| AC5 | `registry_covers_the_catalogue_minus_its_exclusions` (existing, `:2214`) | `registry.rs` | `+30`; comment rewritten |
| AC5 | `the_models_imported_from_fal_land_on_the_right_tab` (existing, `use_case.rs:283`) | `use_case.rs` | three new positive rows + the flipped `seedance_2_5` row |
| AC5 | `every_exact_endpoint_is_a_route_the_registry_actually_has` (existing, `media.rs:1662`) | `media.rs` | the three new `FAL_ROUTE_MODES` rows have real routes |
| AC5 | `the_extend_endpoint_is_priced_at_its_fixed_seven_seconds` (new) | `registry.rs` | `estimate` for `veo3_1_extend` with `Billable{seconds:7.0, audio:false}` == 1.40 and with `audio:true` == 2.80 |
| AC5 | `ltx_2_5_offers_only_the_durations_fal_enumerates` (new) | `registry.rs` | `capabilities().durations == [6.0, 8.0, 10.0]` (`"auto"` is not a number) — non-vacuous: it fails if the flag reverts to `ValueSpec::Number` |
| AC5 | `seedance_2_5_offers_a_thirty_second_clip` (existing, `registry.rs:2889` per `S8/review.md:468`) | `registry.rs` | unchanged; guards the mode that already worked |
| AC6 | `a_const_pinned_axis_is_fixed_not_free_form` (new) | `capability.rs` | `from_fal` with a pinned `aspect_ratio` ⇒ `aspect.values == ["auto"]`, `!aspect.is_free_form()`, `support == Yes` |
| AC6 | `a_catalogue_default_outside_a_pin_is_dropped` (new) | `capability.rs` | catalogue default `1080p` + pin `720p` ⇒ `resolution.default == None`, values `["720p"]` |
| AC6 | existing `a_fal_field_with_no_published_enum_is_free_form_not_absent` (`capability.rs:937`) | `capability.rs` | unchanged — free-form must survive the fix |
| AC7 | the gate (§8) | — | 0 failed, ≥ 779 + the new tests |

**Mutation checks the TEST agent should run** (named here so they are not invented late): revert
`("bytedance/seedance-2.5", false, &[Text, Image])` to `[Text]` ⇒ the flipped `use_case` row must
fail; change the LTX duration flag back to `ValueSpec::Number` ⇒ `ltx_2_5_offers_only_the_durations_fal_enumerates`
must fail; drop the `pinned` insert in `parse` ⇒ AC2's pinned test and AC6's axis test must fail.

---

## 8. Gate commands (AC7), exact

```
cargo test --workspace                                   # 0 failed, >= 779 + new tests
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo deny check                                         # no dependency change is expected
./scripts/lint-provenance.py
env -u hickeyfield_FAL_KEY -u FAL_KEY cargo run -q -p hickeyfield-core --example audit_fal
cargo run -q -p hickeyfield-core --example fal_diff -- --offline   # expect "Registry reaches 66"
cd ui && pnpm test && pnpm build                         # expect 14 files / 232 tests
scripts/build-macos.sh    # or `tauri build`; the .app must launch (window, not blank)
```

Expected audit deltas from the recorded baseline `11 / 2 / 21 / 9 / UNREACHABLE 0 / CHECKED 62`
(`S8/review.md:666-669`): `CHECKED` **62 → 66** (`…/ltx-2.5/text-to-video/pro`,
`…/ltx-2.5/image-to-video/pro`, `fal-ai/veo3.1/extend-video`, `bytedance/seedance-2.5/image-to-video`);
`SILENTLY IGNORED MEDIA` stays **11** (§6.4); `PINNED FIELDS` **0**; `UNREACHABLE` **0**; any other
movement is a §5 triage item. A re-landed id appearing in *any* finding section means it does not
land — S8's own rule (`S8/review.md:262-268`), applied to the models it was applied to before.

**Never** `cargo run --example first_light` (spends money, no key in session). **Never** bare
`cargo build` for a runnable app (README: blank window).

---

## 9. Documentation (the DOCUMENT step)

- **`CHANGELOG.md`** — one `### Fixed` bullet under `## [Unreleased]` (`:7`, `:48`) in the existing
  user-facing voice: the app now honours a fal endpoint that fixes a setting, and LTX 2.5 Pro, LTX
  2.5 Pro Animate, Veo 3.1 Extend and Seedance 2.5's animate-an-image mode are available. The S8
  bullet under `### Added` (`:19`) that qualifies Seedance 2.5 as "text-to-video for now" MUST be
  amended in the same commit, not left to contradict the new one.
- **`crates/hickeyfield-core/src/registry.rs:1534-1541`** — the doc comment currently records LTX 2.5
  and Veo 3.1 Extend as "probed and withdrawn: their schemas pin fields by `const` or integer enums
  the parser cannot yet read". Rewrite: they are landed, and the parser reads both.
- **`crates/hickeyfield-core/src/media.rs:689-696`** and **`use_case.rs:292-294`** — the two comments
  that name this slice as the condition for re-claiming `InputMode::Image`. Both MUST be rewritten to
  state the condition is met and how (§6.4), never merely deleted.
- **`crates/hickeyfield-core/src/fal_schema.rs:1-23`** — the module's opening list of "three bugs in
  one day" gains the fourth: a field the endpoint *pins*, which is neither a name nor a mode nor an
  unaccepted key, and which the string-only enum reader could not see.
- **`examples/audit_fal.rs:7-16`** — "Four checks" becomes five.
- **`.forge/backlog.md:200-210`** — mark the slice's own entry done; leave the S8 minors at `:211-214`
  open (none is this slice's).
- **README** — checked at implement time for a list of fal examples; if one exists nothing is added
  (no new example ships), otherwise **N/A**.
- **`docs/FIRST-LIGHT.md`** — **N/A**: this slice proves nothing live and spends nothing, so the
  open "cost accuracy unverified" item stays open.
- **`document.md`** — written after the docs land, recording file + section for each change.

---

## 10. Non-goals

- **N1.** Parsing fal's per-property `default`. `capability.rs:216-223` records that it is not parsed
  and borrows the catalogue's default instead; changing that is the `nano_banana_2_edit` 0.5K-default
  bug (`S8/review.md:137-146`), a pricing/default slice.
- **N2.** Descending into `anyOf` / `oneOf` / `allOf` for ranges (`minimum`/`maximum`). Wan 3.0 Prime's
  2–30s is documented in prose (`registry.rs:2003-2004`) and enforced by fal; an in-app clamp is its
  own slice and §1.2(4) explicitly keeps it free-form.
- **N3.** `CostModel::PerImageTiered` and `PerToken` returning `None` on unknown dimensions
  (`S8/test.md:460-486`). Both remain backlog.
- **N4.** The media-key audit check comparing `media::fal_keys` against the endpoint's declared keys
  (F3, `S8/test.md:488-502`). Related, but a different blind spot and a different check.
- **N5.** Re-landing MiniMax H3 Max (`required: ["prompt","prompt_expansion_mode"]` with no default,
  `S8/test.md:337-344`) or claiming `InputMode::Image` for `kling3_0_pro` / `wan3_0_prime` (their i2v
  endpoints want `start_image_url`, `S8/test.md:356-362`). Neither is a `const`/int-enum problem.
- **N6.** Any change to `media::bind`, `fal_keys`, `route::resolve`, the enhancer, or any `ui/` file.
- **N7.** A live paid generation.

## 11. Open questions (each with the default this build assumes)

- **Q1 — `pinned` as a set, or fold the pin into a richer enum type?** **Default: a `BTreeSet<String>`
  beside `enums`** (§1.1). A new `enum FieldValues { Enum(Vec<String>), Pinned(String) }` would be
  cleaner in the abstract and would touch four consumers plus every fixture; the set is additive and
  reversible.
- **Q2 — should a pinned Class-A mismatch be an advisory or a hard refusal?** **Default: advisory +
  coerce** (§2.2). If the owner would rather see a refusal, it is a two-line change in `reconcile`
  and one test flip — but it re-creates the dead-tile outcome S8 spent three commits removing.
- **Q3 — does `fal-ai/veo3.1/extend-video` publish `generate_audio`?** Not established by any recorded
  evidence. **Default: if the §0 re-fetch shows the field, declare it and price
  `per_second_audio(0.20, 2.0)`; if it does not, declare no audio flag and price `per_second(0.40)`**,
  with the route note saying the rate assumes audio. Over-quoting is recoverable; under-quoting by 2×
  is the failure mode the objective's money-trust section exists for.
- **Q4 — do LTX 2.5's two endpoints agree on their enums?** They are separate documents. **Default:
  one shared `ltx_flags` only if the §0 re-fetch shows both enumerate identically; otherwise two
  specs.** S8 shared them (`S8/review.md:36-38`) and B1 hit both identically, which is evidence for
  but not proof of agreement.
- **Q5 — does re-landing `veo3_1_extend` need a `NO_END_FRAME` / `FAL_NO_MEDIA` row?** **Default: no**
  — `video_url` is its only media key (`S8/test.md:373`) and `MediaRole::End` is never offered on an
  `EditVideo` tab. Confirm with the probe; if wrong, add the row and bump the literal.
- **Q6 — `4K` on LTX 2.5.** fal publishes a $0.30/s 4K tier that the `["720p","1080p"]` enum makes
  unselectable (`S8/test.md:439`). **Default: keep it unselectable**; adding a tier the estimator
  would misprice is worse than not offering it.

## 12. The riskiest parts (where the verifier should start)

1. **The parser change is global.** It alters what the app believes about **every** fal endpoint,
   including ~50 in `CHECKED` that were never scanned for this defect class. The concrete danger is
   the *new* refusal: an endpoint that enumerates integers now has an enum, and a spec value outside
   it becomes `JobError::Permanent` where yesterday it was a silent 422 or an accidental success —
   i.e. this slice can break a model it never names. §5 is the whole mitigation, and it is procedural
   (an audit run plus a triage table), not mechanical. **This is the weakest link in the design.**
2. **D2 is a judgement call about the user's money.** Coercing to the pin is a value the user did not
   type. The argument that it is not a substitution (no neighbour exists) is sound for `aspect_ratio:
   "auto"` and `duration: "7s"`, and the Class-B guard keeps it away from prompts and uploads — but
   it is the decision most worth a second opinion, and Q2 records the reversal.
3. **Seedance 2.5's two endpoints through one spec.** §6.4 argues from the code that the pin is
   applied per-endpoint, but the proof requires running `model_capabilities` twice with different
   `hasMedia` — which this design could not execute. If that path is wrong, re-claiming
   `InputMode::Image` re-opens B4 on the catalogue's headline model.
4. **Array-length literals**, again: `media.rs:599` (48→51), `registry.rs:2817` (11→14),
   `registry.rs:2239` (+27→+30). Each fails loudly, but they are the highest-frequency edit here and
   the S8 history shows all three moved in both directions already.
5. **`reconcile`'s signature change** reaches `src-tauri/src/app.rs:438-439`. It is one call site and
   the existing tests compile unchanged — but a `Vec<Pinned>` dropped on the floor at that call site
   would make the advisory silently disappear, which is the honesty failure this module names in its
   own doc comment (`fal_schema.rs:21-23`).
