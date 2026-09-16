# S14-minimax-h3-max-constant-fields — DESIGN

Contract: `.forge/slices/S14-minimax-h3-max-constant-fields/slice.md` (AC1–AC6).
Upstream this design is built on:
`.forge/objective.md` (§A no-regression gates; live-key constraint),
`.forge/slices/S8-fal-catalogue-sync/implement.md` §8.1 (why H3 Max was refused; the import recipe),
`.forge/slices/S9-fal-schema-const-and-int-enums/design.md` (const/enum enrichment, `reconcile`,
`Pinned`, per-endpoint schema layer; house style followed here),
`.forge/slices/S10-pricing-accuracy-and-cost-tracker/design.md` (`PerSecondTiered`, `prices::audit`,
`Route::floored`/`min_billed_seconds`). The four **live schemas** are vendored in this slice at
`.forge/slices/S14-minimax-h3-max-constant-fields/schemas/*.json` (fetched keyless 2026-09-16) and
are quoted verbatim below.

Slice shape: **one ~40-line constant-fields mechanism (data in `media.rs`, pure writer in
`fal_schema.rs`, one call site in `app.rs`) + two audit changes + two model ids landed via the
S8/S9 recipe + one pricing-height map fix.** No new dependency. **No `ui/` source changes**
expected (§D6). `first_light.rs` MUST NOT run — no key exists and nothing here spends money.

---

## 0. Session constraint, stated before anything is claimed

This design was authored **without running `cargo`/`curl`/`git`** (a `tauri build` holds the working
tree; the launching agent forbids touching `ui/`, `src-tauri/`, `crates/`). Every `file:line` below
was read first-hand from the working tree at `584fe04` this session. Every fal schema fragment is
quoted from the four vendored `schemas/*.json` this slice ships, not from memory — the failure
`fal_schema.rs:1-31` was written after is a design that asserts a vendor's API from memory.

> **MUST, first task of IMPLEMENT:** re-read the four vendored schemas (or re-fetch them keyless) and
> confirm §0.1 before writing code. If any disagrees, **the schema wins** and this design is amended
> in `implement.md`.
>
> ```
> curl -s "https://fal.ai/api/openapi/queue/openapi.json?endpoint_id=minimax/h3-max/text-to-video"
> curl -s "https://fal.ai/api/openapi/queue/openapi.json?endpoint_id=minimax/h3-max/image-to-video"
> curl -s "https://fal.ai/api/openapi/queue/openapi.json?endpoint_id=minimax/h3-max-turbo/text-to-video"
> curl -s "https://fal.ai/api/openapi/queue/openapi.json?endpoint_id=minimax/h3-max-turbo/image-to-video"
> ```

### 0.1 Ground truth, transcribed from the vendored schemas (2026-09-16)

All four endpoints (`minimax/h3-max/{text,image}-to-video`,
`minimax/h3-max-turbo/{text,image}-to-video`) share this input surface. Quoted from
`schemas/minimax_h3-max_text-to-video.json` and `…_image-to-video.json` (turbo identical bar the
titles):

| fact | verbatim | which endpoints |
|---|---|---|
| `required` | `["prompt", "prompt_expansion_mode"]` | all four |
| `prompt_expansion_mode` | `{"type":"string","default":"balanced","examples":["disabled","balanced","quality"]}` — **no `enum`, no `const`**; a required *free string* fal defaults but still demands | all four |
| `resolution` | `{"enum":["480P","768P","1080P"],"type":"string","default":"768P"}` — **UPPERCASE `P`** | all four |
| `duration` | `{"type":"integer","minimum":5,"maximum":15,"default":5}` — free-form integer, ceiling **15** (fal-enforced; not an enum) | all four |
| `aspect_ratio` | `{"enum":["21:9","16:9","4:3","1:1","3:4","9:16"],"type":"string","default":"16:9"}` | **t2v only** — absent from both i2v `properties` |
| `image_url` | `{"anyOf":[{"type":"string"},{"type":"null"}]}` (optional first frame) | **i2v only** |
| `end_image_url` | `{"anyOf":[{"type":"string"},{"type":"null"}]}` (optional last frame) | **i2v only** |
| `seed`, `sync_mode`, `enable_safety_checker` | present, not app-driven | all four |

Confirmations against current source, read this session:

- **`prompt_expansion_mode` is exactly the S8 refusal:** required, defaulted, no app mechanism to
  send a constant (`S8/implement.md:243-253`). It is neither `prompt` nor a media key, so
  `audit_fal.rs` check 2 (`:104-116`) flags it as MISSING REQUIRED FIELDS today, and
  `app.rs`'s unknown-key sweep never *adds* a field — every submit would 422. This slice's whole
  mechanism is to send it.
- **Resolution case is already handled by `coerce`** (`fal_schema.rs:103-123`,
  case-insensitive at `:110`): `coerce("resolution","1080p")` against `["480P","768P","1080P"]`
  returns `"1080P"`. AC4 needs **no new coercion code**, only a test (§4).
- **The i2v media keys are exactly what the binder writes:** `fal_keys(MediaRole::Start,…)` →
  `image_url`, `fal_keys(MediaRole::End,…)` → `end_image_url` (`media.rs:197,215`). So the animate
  mode binds cleanly and **`end_image_url` needs no `NO_END_FRAME` entry** — the slug is not in that
  list (`media.rs:842-855`) and must stay out.
- **`resolve_endpoint` produces the exact slugs:** a family-root entry `("minimax/h3-max", false,
  &[Text, Image])` in `FAL_ROUTE_MODES` resolves via the suffix path (`media.rs:943-956`,
  `InputMode::suffix`, `:571-580`) to `minimax/h3-max/text-to-video` and
  `minimax/h3-max/image-to-video`; likewise `minimax/h3-max-turbo/*`. Verified: the root matches no
  `MODE_TAILS` and is not exact, so `format!("{slug}{suffix}")` fires.
- **`minimax_h3` is a different model** (`minimax/h3`, `PerSecondTiered` after S10,
  `registry.rs:1128-1152`, spec `:1772-1784`). Add H3 Max **alongside**; never re-point it.

### 0.2 fal price prose (from the vendored index, quoted in the slice)

- `minimax/h3-max/{text,image}-to-video`: **"$0.025/s at 480p, $0.04/s at 768p, $0.08/s at 1080p
  — promotional launch rates"**.
- `minimax/h3-max-turbo/*`: **"$0.0125 / $0.02 / $0.04"** (same note).

This is multi-figure, resolution-conditioned prose. `fal_catalogue::parse_pricing` refuses it (>1
figure + resolution words — `S10/design.md §0.2`, `prices.rs:1145-1165`), so `prices::audit`
classifies these endpoints as `AuditRow::Prose` (or `NoSnapshot` if the endpoint is absent from the
bundled snapshot) — **never `Mismatch`**. See D4 for the honest consequence.

---

## 1. AC1 — the constant-fields mechanism (D1)

### D1 (decision): data in `media.rs`, pure writer in `fal_schema.rs`, one call in `app.rs`

**Where the data lives — a `media.rs` table keyed by the family-root slug**, beside its exact
siblings `FAL_ROUTE_MODES`, `FAL_NO_MEDIA`, `NO_END_FRAME` (`media.rs:599,807,842`):

```rust
/// Body fields a fal endpoint REQUIRES with a value the app does not surface as a
/// setting — fal lists a default yet still rejects the request when the key is
/// absent. Keyed by the family-root slug because every input mode of the family
/// needs the same constant. Written verbatim into the body before submit
/// (`fal_schema::apply_constants`), respecting a user-set field and the schema.
const FAL_CONSTANT_FIELDS: [(&str, &[(&str, &str)]); 2] = [
    ("minimax/h3-max",       &[("prompt_expansion_mode", "balanced")]),
    ("minimax/h3-max-turbo", &[("prompt_expansion_mode", "balanced")]),
];

pub fn fal_constants(slug: &str) -> &'static [(&str, &str)] {
    FAL_CONSTANT_FIELDS.iter().find(|(s, _)| *s == slug).map_or(&[], |(_, c)| *c)
}
```

**Where the mechanism lives — a pure writer in `fal_schema.rs`**, beside `reconcile`:

```rust
/// Write each `(key, value)` into `body` for a field the endpoint fixes as a
/// constant, before `reconcile`. A constant is NOT a setting: it exists because
/// fal requires the key.
///
/// Rules (MUST):
/// - Never override a field already present — a value the user set (or the binder
///   wrote) always wins; a constant only fills a gap.
/// - With a schema, write only when the endpoint `accepts` the key; a constant an
///   endpoint does not declare would be swept away anyway, and sending it to a
///   strict endpoint 422s.
/// - With NO schema (offline / uncached), write it regardless: fal requires the
///   key, so omitting it is a guaranteed 422, while sending a field every endpoint
///   we ship this for is known to take is the lesser risk. Justified: the four H3
///   Max endpoints all declare `prompt_expansion_mode`, so the offline write is
///   correct for every route this table currently covers.
pub fn apply_constants(
    constants: &[(&str, &str)],
    schema: Option<&EndpointSchema>,
    body: &mut serde_json::Map<String, serde_json::Value>,
) {
    for (key, value) in constants {
        if body.contains_key(*key) {
            continue;
        }
        let accepted = schema.map_or(true, |s| s.accepts(key));
        if accepted {
            body.insert((*key).to_string(), serde_json::Value::String((*value).to_string()));
        }
    }
}
```

**The one call site — `app.rs`**, immediately after `fal_schema` is fetched (`app.rs:314-318`) and
**before** the AspectPlan block (`:327`) and the schema-validation block (`:381`):

```rust
if route.provider == ProviderId::Fal {
    hickeyfield_core::fal_schema::apply_constants(
        hickeyfield_core::media::fal_constants(&route.slug),
        fal_schema.as_ref(),
        &mut body,
    );
}
```

Ordering is load-bearing and worked through below (§1.1).

**Why this split, not a `Route` field or a registry table.** Three options were on the table
(slice AC1 leaves it to design):

| option | verdict |
|---|---|
| **`media.rs` data + `fal_schema.rs` writer** (chosen) | "which fal endpoint requires which constant" is a *provider fact about an endpoint* — the exact category of `FAL_ROUTE_MODES`/`NO_END_FRAME`, all in `media.rs` keyed by slug — while "how to write it respecting the schema" is a *submit-path mechanism*, the exact category of `reconcile`, in `fal_schema.rs`. Both are pure and unit-testable in `core`; one table row covers both t2v and i2v; the audit and `app.rs` both already hold the slug. |
| a `constants: &[(&str,&str)]` field on `Route` (`route.rs:18-39`) | `Route` is serialized and per-provider; a fal-only concept would be `None` on every Vaig/Higgsfield route and would duplicate the constant across the two modes' routes. Rejected: bloats the serialized struct with a provider-specific field for no reuse. |
| a per-route table in `registry.rs::priced_routes` | mixes an endpoint-shape fact into the pricing/spec module; the audit would have to reach into pricing to learn it. Rejected: `registry.rs` is about who-serves-it and what-it-costs, not what-fields-the-wire-needs. |

### 1.1 Data flow, worked end to end (H3 Max Turbo, t2v, online)

User picks MiniMax H3 Max Turbo, no media, prompt "a red door", resolution left at default.

1. `bind` writes nothing (no media); `prompt` inserted (`app.rs:233-238`); the settings loop
   inserts `resolution` (spec-declared, §3) and `duration`. `aspect_ratio` inserted from settings.
2. `endpoint = resolve_endpoint("minimax/h3-max-turbo", Text, true)` →
   `minimax/h3-max-turbo/text-to-video` (`app.rs:280-289`).
3. `fal_schema = for_endpoint(…)` → the t2v schema (accepts `prompt_expansion_mode`, `resolution`,
   `aspect_ratio`; enumerates `resolution` = `["480P","768P","1080P"]`, `aspect_ratio` = the six).
4. **`apply_constants`**: body has no `prompt_expansion_mode`, schema accepts it →
   `body["prompt_expansion_mode"] = "balanced"`.
5. AspectPlan block: `aspect_key = Some("aspect_ratio")` (schema accepts it) → the chosen ratio is
   written explicitly (`app.rs:354-362`).
6. Unknown-key sweep: `prompt_expansion_mode`, `resolution`, `aspect_ratio` all `accepts` → kept.
7. `reconcile`: `resolution` "768P" already an exact enum member → rewritten as `"768P"` (retype
   no-op); `prompt_expansion_mode` has no enum → passed through verbatim; `aspect_ratio` matches →
   kept. No `Pinned` rows (H3 Max pins nothing). Body ships with `prompt_expansion_mode: "balanced"`
   and fal no longer 422s.

**Image mode (i2v):** step 3's schema **does not** `accepts("aspect_ratio")`, so step 5's
`aspect_key = None` → the AspectPlan `(_, key)` arm removes `aspect`/`aspect_ratio` from the body
(`app.rs:366-372`). This is the mechanism that keeps the t2v-only ratio off the animate wire — the
generic sweep is redundant here but agrees. `apply_constants` still writes
`prompt_expansion_mode` (i2v accepts it); `bind` wrote `image_url`/`end_image_url` for the attached
stills.

**Offline (schema uncached):** `fal_schema = None`. `apply_constants` writes
`prompt_expansion_mode` anyway (fal requires it). The AspectPlan block derives `aspect_key` from the
spec instead (`app.rs:336-338`) — see the recorded residual in D3 and Q2.

### 1.2 "Never override a user-set field"

`apply_constants` skips any key already in `body`. For H3 Max this is theoretical (the app surfaces
no `prompt_expansion_mode` control and no spec flag names it), but the guard is the whole point of a
*mechanism* rather than a hardcode: the next required-with-default field a provider adds may collide
with a real setting, and the user's value must win. Pinned by a test (§7).

---

## 2. AC2 — the audit (D2)

`crates/hickeyfield-core/examples/audit_fal.rs`.

### D2 (decision): check 2 subtracts route constants; a new check 6 verifies each constant

**Check 2 (`:99-116`) MUST treat a declared constant as supplied.** Load the route's constants and
add them to the exclusion filter:

```rust
let constant_keys: Vec<&str> = media::fal_constants(&route.slug).iter().map(|(k, _)| *k).collect();
let never_sent: Vec<&str> = schema.required.iter().map(String::as_str)
    .filter(|k| *k != "prompt" && !schema.media_keys().contains(k) && !constant_keys.contains(k))
    .collect();
```

Without this, all four H3 Max endpoints land in MISSING REQUIRED FIELDS and the AC5 clean run is
impossible — the field *is* supplied now, by the mechanism, so the audit must know it.

**New check 6 — "CONSTANT NOT ACCEPTED / OUT OF ENUM".** A constant is only safe if the endpoint
takes the key and (when it enumerates the key) the value is a member. Pure, network-free, over the
already-fetched schema:

```rust
// 6. A route constant the endpoint does not accept, or whose value the endpoint
//    enumerates and this value is not among. Either is a wasted/refused field.
for (key, value) in media::fal_constants(&route.slug) {
    if !schema.accepts(key) {
        bad_constants.push(format!("{:<26} {endpoint}\n      constant `{key}` = {value} is not a field this endpoint accepts", m.id));
    } else if schema.enums.contains_key(*key) && schema.coerce(key, value).is_none() {
        bad_constants.push(format!("{:<26} {endpoint}\n      constant `{key}` = {value} is outside the endpoint's enum [{}]", m.id, schema.enums[*key].join(", ")));
    }
}
```

Printed with the existing `section` helper (`:285-293`), placed after PINNED FIELDS and before
UNREACHABLE. For H3 Max: `prompt_expansion_mode` is accepted and has no enum → `coerce` returns
`Some` → **clean**. The check earns its keep as a guard: mistype the constant key, or pin a value an
endpoint enumerates differently, and the build's audit says so.

The module doc (`:7-21`) becomes **six checks** (add check 6). Check 5 (PINNED) and `pin_conflicts`
are untouched — H3 Max declares no `const`, so PINNED stays 0.

`Cargo.toml` already has `[[example]] name = "audit_fal", test = true` (S9), so the new fixture tests
run inside `cargo test --workspace`.

---

## 3. AC3 — land the two models (D3)

### D3 (decision): two model ids, one spec each, both modes via `FAL_ROUTE_MODES`

Following the S8/S9 import recipe (`S8/implement.md §8.1`): spec + priced route + `JOB_TYPES` row +
`FAL_ROUTE_MODES` + use-case tabs + counts.

**The two specs** in `picker_only_specs()` (`registry.rs:1613`), built with `spec()`/`en()`
(`:2323-2350`), every enumerated set fal's own from §0.1. Both are `Modality::Video`. Shared shape:

```
("prompt",       true,  ValueSpec::Text),
("start_image",  false, ValueSpec::Media),        // i2v `image_url`  via fal_keys(Start)
("end_image",    false, ValueSpec::Media),        // i2v `end_image_url` via fal_keys(End)
("duration",     false, ValueSpec::Number),       // free-form int; fal enforces 5..15
("resolution",   false, en(&["480P", "768P", "1080P"])),   // UPPERCASE, verbatim
("aspect_ratio", false, en(&["21:9", "16:9", "4:3", "1:1", "3:4", "9:16"])),  // t2v list
```

- ids/display: `minimax_h3_max` → "MiniMax H3 Max"; `minimax_h3_max_turbo` → "MiniMax H3 Max Turbo".
- constraint note (both): `"fal renders 5–15 second clips (default 5); 1080P is a latent refinement
  of a native 768P source."` The 5–15 ceiling is fal's own (`maximum: 15`), not invented.
- constraint note (both): `"Animating a still always follows the image's own shape; the aspect
  control applies to text-to-video only."` — mirrors the Seedance `const` note (`registry.rs:1625`).

**Why `aspect_ratio` is declared on the shared spec even though i2v has no such key.** One spec is
per-model; the two endpoints disagree (t2v has six ratios, i2v has none). The *online* path — the
normal case — is governed per-endpoint by the schema: `capability::fal_axis` reads `schema.enums`
(`capability.rs:603-618`), so t2v shows six ratios and i2v shows none, and `app.rs`'s AspectPlan
drops the key on i2v (§1.1, verified from `app.rs:332-373`). The spec's aspect enum is read in
exactly one situation: **offline t2v**, where it gives the correct axis. The offline-i2v residual
(the key is sent to an endpoint that does not declare it) is recorded in **Q2** — it is offline-only
and, per the i2v schema's own note ("If both images are omitted, the request is handled as
text-to-video (16:9 by default)"), low-risk. This is the same one-spec/two-endpoint reasoning S9
applied to Seedance 2.5 (`S9/design.md §6.4`), adapted to a key that is *absent* on i2v rather than
`const`-pinned.

**`ValueSpec::Number` for duration**, not an enum: fal declares a free integer range 5–15, exactly
like Wan 3.0 Prime's 2–30 (`registry.rs:2079-2101`), which is `ValueSpec::Number` with a prose
constraint. Enumerating 5..15 would invent members and re-create the LTX class of bug in reverse.

**The routes** in `priced_routes` (`registry.rs:610`). The fal route slug **MUST** be the family
root `minimax/h3-max` / `minimax/h3-max-turbo` (a test asserts every `FAL_ROUTE_MODES` slug has a
matching fal route — `media.rs:1686-1691`):

```rust
"minimax_h3_max" => vec![(
    Route::noted(ProviderId::Fal, "minimax/h3-max",
        "$0.025/s 480p, $0.04/s 768p, $0.08/s 1080p — promotional launch rate per fal's index on 2026-09-16"),
    CostModel::PerSecondTiered { tiers: vec![(480, 0.025), (768, 0.04), (1080, 0.08)] },
)],
"minimax_h3_max_turbo" => vec![(
    Route::noted(ProviderId::Fal, "minimax/h3-max-turbo",
        "$0.0125/s 480p, $0.02/s 768p, $0.04/s 1080p — promotional launch rate per fal's index on 2026-09-16"),
    CostModel::PerSecondTiered { tiers: vec![(480, 0.0125), (768, 0.02), (1080, 0.04)] },
)],
```

`PerSecondTiered` keys tiers by height with `height <= max_h` (`cost.rs:176-193`) — see D5 for the
`768p` height-map fix that makes the middle tier reachable.

**`FAL_ROUTE_MODES` (`media.rs:599`):** add two rows, both `&[InputMode::Text, InputMode::Image]`
(both modes bind — §0.1). Bump the length literal `51 → 53`.

```rust
("minimax/h3-max",       false, &[InputMode::Text, InputMode::Image]),
("minimax/h3-max-turbo", false, &[InputMode::Text, InputMode::Image]),
```

---

## 4. AC4 — resolution case

`reconcile`'s `coerce` is already case-insensitive (`fal_schema.rs:110`). The chip row offers the
schema's members directly (uppercase, online) or the spec's uppercase enum (offline). The wire
therefore carries `"768P"`/`"1080P"` exactly, whether the stored setting is upper- or lowercase.
**No code change; one test** (§7, `resolution_case_matches_the_uppercase_members`).

---

## 5. AC5 / AC6 — counts, tabs, gate (D5)

### D5 (decision): the counts, exactly

| literal | file:line | from → to | why |
|---|---|---|---|
| `FAL_ROUTE_MODES: [...; 51]` | `media.rs:599` | 51 → **53** | two family-root rows (§3) |
| registry size `+ 30` | `registry.rs:2443` | +30 → **+32** | two new picker-only ids; rewrite the comment `:2431-2440` to record the H3 Max pair and the constant-fields mechanism that made them landable |
| imported list | `registry.rs:3036` | new **`IMPORTED_2026_09_H3: [&str; 2]`** | see below |
| `JOB_TYPES` "video" group | `registry.rs:299-334` | +2 ids | `minimax_h3_max`, `minimax_h3_max_turbo`; omitting a row trips the `debug_assert` at `registry.rs:506-510` |
| `resolution_height` | `commands.rs:613-626` | +`"768p" => Some(768)` | so the 768P tier is reachable — see below |
| `NO_END_FRAME` / `FAL_NO_MEDIA` / `FAL_MISSING_ROUTES` | `media.rs:842,807,786` | **unchanged** | H3 Max i2v declares `end_image_url` and takes media; the family root resolves. Confirm against the §0 re-read; if wrong, bump the literal. |

**The imported list — a new `IMPORTED_2026_09_H3` const, not an extension of `IMPORTED_2026_09`.**
`IMPORTED_2026_09` (`registry.rs:3036`, `[&str; 14]`) is the S8/S9 sweep. H3 Max landed later, on a
new mechanism, and re-fetched today; a distinct const keeps that provenance legible and mirrors S9's
own `[&str; 14]` framing. The two existing loops (`:3064`, `:3106`) MUST also iterate the new const
(chain or a second loop), so `every_imported_model_resolves_to_an_exact_fal_endpoint` and
`every_imported_model_has_a_fal_route_with_a_real_adapter` cover both H3 Max ids for free.

```rust
const IMPORTED_2026_09_H3: [&str; 2] = ["minimax_h3_max", "minimax_h3_max_turbo"];
```

**Use-case guards (`use_case.rs:283-326`):** add four positive rows to the loop — both ids on both
`TextToVideo` **and** `ImageToVideo` (both modes served). No negative rows (unlike Kling 3.0 Pro /
Wan 3.0 Prime, whose i2v used an unbindable `start_image_url`; H3 Max i2v uses `image_url`, which the
binder writes).

```rust
("minimax_h3_max",       UseCase::TextToVideo),
("minimax_h3_max",       UseCase::ImageToVideo),
("minimax_h3_max_turbo", UseCase::TextToVideo),
("minimax_h3_max_turbo", UseCase::ImageToVideo),
```

**Why `resolution_height` needs `768p`.** The tier lookup needs a pixel height; `resolution_height`
(`commands.rs:613`) lowercases the label and matches a fixed set that has `480p`, `1080p` but **not
`768p`** (`:615-624`). A stored `"768P"` → `"768p"` → `None` → no `Billable.height` →
`PerSecondTiered` returns `None` (`cost.rs:180-182`) → the Generate button reads "price unknown" for
the default resolution. Adding `"768p" => Some(768)` makes the middle tier reachable: height 768 →
`768 <= 768` → `$0.04/s` (turbo `$0.02/s`). This is the one line in `src-tauri/` this slice needs and
it is a genuine gap the tiered price exposes. `480P`→480 and `1080P`→1080 already map.

### AC6 — regression gate

`cargo test --workspace` MUST stay at 0 failed and **≥ the S13 released baseline** (IMPLEMENT records
the exact pre-change total from the working tree and adds the new tests — the count only rises).
`cd ui && pnpm test && pnpm build` stay green (no `ui/` change expected). CHANGELOG updated (§9).
`tauri build` / launch is the release phase. **No paid job.**

### D6 (decision): UI — nothing new

The model tiles and chip rows render from data (`CapabilitiesDto::from_capability`,
`commands.rs:324-353`; `ChipRow`/`variants.ts` consume `resolutions`/`durations`/`aspects`). H3 Max's
axes flow through the same path S8's imports used, so **no `ui/` source changes are expected**;
`vitest` is **N/A unless a `ui/` file changes**. If IMPLEMENT finds a UI file must change, add a
`vitest` test and note the deviation in `implement.md`.

---

## 6. AC-by-AC coverage of the mechanism vs pricing (D4)

### D4 (decision): pricing is tiered; the price audit reports it as `Prose`, honestly

The tiers transcribe §0.2 exactly. `prices::audit` (`prices.rs:1619`) will **not** machine-verify
them: fal's prose is multi-figure and resolution-conditioned, which `parse_pricing` refuses by design
(`S10/design.md §0.2`). The H3 Max endpoints therefore classify as `AuditRow::Prose` (matches the
prose on inspection), or `AuditRow::NoSnapshot` if the bundled `fal-catalogue-snapshot.json` has no
row for them (the snapshot predates the promotional launch; **IMPLEMENT MUST check** and record
which). **Neither variant fails the build** (`prices.rs:1426-1429`, only `Mismatch` does). This is
stated so "the price audit passed" is not misread as "the H3 Max price was verified" — it was
transcribed and is verifiable only against the live fal pricing API (the S10 AC6 seam), not
hermetically. `price_check` MUST still print `MISMATCH 0`.

---

## 7. Test plan — every AC to a named test

Rust tests live in the `#[cfg(test)] mod tests` block beside the code.

| AC | test | file | asserts |
|---|---|---|---|
| AC1 | `fal_constants_names_the_required_field_for_h3_max` | `media.rs` | `fal_constants("minimax/h3-max")` and `…-turbo` == `[("prompt_expansion_mode","balanced")]`; an unknown slug == `[]` |
| AC1 | `a_constant_is_written_when_the_schema_accepts_it` | `fal_schema.rs` | `apply_constants([("prompt_expansion_mode","balanced")], Some(&schema_that_accepts), &mut body)` inserts it |
| AC1 | `a_constant_never_overrides_a_user_value` | `fal_schema.rs` | body already has the key → value unchanged after `apply_constants` |
| AC1 | `a_constant_is_dropped_when_the_schema_rejects_the_key` | `fal_schema.rs` | schema without the key → key absent after `apply_constants` |
| AC1 | `a_constant_is_sent_when_no_schema_is_cached` | `fal_schema.rs` | `apply_constants(…, None, &mut body)` inserts it (offline path) |
| AC2 | `a_required_constant_is_not_flagged_as_missing` | `examples/audit_fal.rs` | with `fal_constants` covering `prompt_expansion_mode`, check 2 does not list it (fixture route+schema) |
| AC2 | `a_constant_the_endpoint_rejects_is_reported` | `examples/audit_fal.rs` | check 6 flags a constant whose key the schema does not accept, and one outside an enumerated key; returns empty when the key is accepted and enum-free |
| AC3 | `the_h3_max_models_are_present` | `registry.rs` | `registry()` contains `minimax_h3_max`, `minimax_h3_max_turbo`; `minimax_h3` still present and on `minimax/h3` |
| AC3 | `h3_max_resolves_to_both_exact_endpoints` | `registry.rs` (via `IMPORTED_2026_09_H3`) | Text→`minimax/h3-max/text-to-video`, Image→`…/image-to-video`; turbo likewise |
| AC3 | `h3_max_offers_only_the_resolutions_fal_enumerates` | `registry.rs` | `capabilities().resolutions == ["480P","768P","1080P"]` for both ids — non-vacuous (fails if reverted to `ValueSpec::Text`) |
| AC3/AC4 | `resolution_case_matches_the_uppercase_members` | `fal_schema.rs` | `coerce("resolution","1080p")` against `["480P","768P","1080P"]` → `"1080P"`; and a body `"768P"` reconciles to `"768P"` |
| AC3 (price) | `h3_max_is_priced_per_the_launch_tiers` | `registry.rs` | `estimate` with `Billable::video(5.0, w, 768)` → `5×0.04`; at 480 → `5×0.025`; at 1080 → `5×0.08`; turbo halved-ish per §0.2 |
| AC5 | `the_768p_tier_is_reachable` | `commands.rs` | `resolution_height("768P") == Some(768)` and `Billable::from` a `{resolution:"768P"}` settings yields `height:Some(768)` |
| AC5 | `the_models_imported_from_fal_land_on_the_right_tab` (existing, `use_case.rs:284`) | `use_case.rs` | picks up the four new positive rows |
| AC5 | `every_imported_model_resolves_to_an_exact_fal_endpoint` (existing, `registry.rs:3059`) | `registry.rs` | now also iterates `IMPORTED_2026_09_H3` |
| AC5 | `registry_covers_the_catalogue_minus_its_exclusions` (existing, `registry.rs:2418`) | `registry.rs` | `+32`; comment rewritten |
| AC5 | `every_exact_endpoint_is_a_route_the_registry_actually_has` (existing, `media.rs:1686`) | `media.rs` | the two new `FAL_ROUTE_MODES` slugs have real routes |
| AC5 | (evidence, not a test) | `implement.md` | keyless `audit_fal` full run: UNREACHABLE 0, the four H3 Max endpoints CHECKED with zero findings, MISSING REQUIRED unchanged elsewhere, PINNED 0, check 6 0 |
| AC6 | the gate (§8) | — | 0 failed, ≥ S13 baseline + new tests; `pnpm` green |

**Mutation checks the TEST agent should run** (named so they are not invented late): drop the
`!constant_keys.contains(k)` term in check 2 ⇒ `a_required_constant_is_not_flagged_as_missing` fails;
remove the `body.contains_key` guard in `apply_constants` ⇒ `a_constant_never_overrides_a_user_value`
fails; revert `resolution_height` ⇒ `the_768p_tier_is_reachable` and the 768 pricing case fail;
change the resolution flag to `ValueSpec::Text` ⇒ `h3_max_offers_only_the_resolutions_fal_enumerates`
fails.

---

## 8. Gate commands (AC6), exact

```
# dummy keys so any shell test reading a key env does not skip/panic:
export hickeyfield_FAL_KEY=x hickeyfield_HIGGSFIELD_KEY=x hickeyfield_HIGGSFIELD_SECRET=x
cargo test --workspace                                   # 0 failed, >= S13 baseline + new tests
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
./scripts/lint-provenance.py
env -u hickeyfield_FAL_KEY -u FAL_KEY cargo run -q -p hickeyfield-core --example audit_fal
env -u hickeyfield_FAL_KEY -u FAL_KEY cargo run -q -p hickeyfield-core --example price_check
# UI (Node 24):
export PATH="/private/tmp/claude-501/-Users-jorge-Documents-GitHub-hickeyfield/79be25e2-8f71-4192-9e6d-cd0e9d068f1c/scratchpad/node-v24.8.0-darwin-arm64/bin:$PATH"
cd ui && pnpm test && pnpm build
# release phase: scripts/build-macos.sh (or `tauri build`); the .app must launch (window, not blank)
```

Expected audit deltas from the S13 baseline: `CHECKED` **+4** (`minimax/h3-max/{text,image}-to-video`,
`minimax/h3-max-turbo/{text,image}-to-video`); MISSING REQUIRED FIELDS **unchanged** (the four are
now covered by check 2's constant subtraction); PINNED **0**; the new CONSTANT check **0**;
UNREACHABLE **0**. `price_check` **MISMATCH 0** (the H3 Max rows are `Prose`/`NoSnapshot`, §D4).
`cargo deny check` is not in this slice's run list and no dependency changes; run it if the release
phase requires it. **Never** `first_light` (spends money, no key). **Never** bare `cargo build` for a
runnable app (README: blank window).

---

## 9. Documentation (the DOCUMENT step)

- **`CHANGELOG.md`** — one `### Added` bullet under `## [Unreleased]` (`:28`), user-facing voice:
  MiniMax H3 Max and H3 Max Turbo (text-to-video and animate-an-image) are now available and priced
  at fal's promotional launch rates; and one `### Changed`/`### Fixed` line that the app can now send
  a body field a fal endpoint requires but does not surface as a control.
- **`crates/hickeyfield-core/src/media.rs`** — doc comment on the new `FAL_CONSTANT_FIELDS` table
  (its purpose and the offline rule).
- **`crates/hickeyfield-core/src/fal_schema.rs`** — doc comment on `apply_constants` (already sketched
  in §D1); the module's opening list (`:1-31`) may note the fifth failure class: a required field the
  app has no setting for.
- **`crates/hickeyfield-core/examples/audit_fal.rs:7-21`** — "Six checks"; describe check 6.
- **`crates/hickeyfield-core/src/registry.rs`** — the `picker_only_specs` doc comment / size-test
  comment (`:2431-2440`) records the H3 Max pair and the mechanism that unlocked it.
- **README** — checked for a fal-examples list; none exists (grep found none) and no new example
  ships, so **N/A**.
- **`docs/FIRST-LIGHT.md`** — **N/A**: this slice proves nothing live and spends nothing.
- **`.forge/backlog.md`** — mark the S14 entry done; leave any S8 minors open.
- **`document.md`** — written after the docs land, recording file + section for each change.

---

## 10. Non-goals (recorded, not skipped)

- **N1.** Surfacing `prompt_expansion_mode` as a user control. It is a required constant here; a
  chip for it (disabled/balanced/quality) is a later capability slice, not this mechanism.
- **N2.** Machine-verifying the H3 Max price hermetically. fal's conditional prose is `Prose` by
  design (§D4); live verification is the S10 AC6 seam.
- **N3.** Turning `duration` into an enum, or clamping 5–15 in-app. fal enforces the range; the axis
  stays free-form (`ValueSpec::Number`), the S8/S9 rule for integer ranges.
- **N4.** Any change to `media::bind`, `fal_keys`, `route::resolve`, the enhancer, or `ui/` source.
- **N5.** A live paid generation; re-pointing `minimax_h3`; adding H3 Max Turbo/Max variants beyond
  the four endpoints in `schemas/`.

---

## 11. Open questions (each with the default this build assumes)

- **Q1 — constant data in `media.rs` or a `Route` field?** **Default: `media.rs` table + `fal_schema`
  writer** (§D1). A `Route` field is reversible if a future non-fal provider needs per-route
  constants, but no such case exists and the table is the DRY, sibling-consistent choice.
- **Q2 — offline i2v sends `aspect_ratio` the endpoint does not declare. Accept it?** **Default:
  accept.** Online (normal) the AspectPlan block drops it (§1.1, verified); offline is rare and the
  i2v endpoint treats an image-following request leniently. The strictly-safest alternative — omit
  `aspect_ratio` from the shared spec entirely, losing the offline t2v aspect axis — is worse for the
  common case. Reversible by splitting into t2v/i2v specs (four ids), which the contract's D3 rules
  out.
- **Q3 — does the bundled `fal-catalogue-snapshot.json` carry the H3 Max endpoints?** **Default:
  unknown until IMPLEMENT checks; either `Prose` (present) or `NoSnapshot` (absent) — both pass
  `price_check`.** If a snapshot refresh is wanted to carry the prose, that is an S8-style `fal_diff
  --dump` step, out of this slice's scope; record the outcome in `implement.md`.
- **Q4 — `ValueSpec::Number` vs `Integer` for duration?** **Default: `Number`**, matching Wan 3.0
  Prime (`registry.rs:2091`). If the axis renders oddly, `Integer` is a one-line change; behaviour is
  identical for the estimator.
- **Q5 — new `IMPORTED_2026_09_H3` const or extend `IMPORTED_2026_09` to 16?** **Default: a new
  const** (§D5) for provenance clarity; extending the existing one is equivalent mechanically and
  reversible.

---

## 12. The riskiest parts (where the verifier should start)

1. **The `apply_constants` ordering in `app.rs`.** The constant must be written *after* the schema
   fetch and *before* the AspectPlan/sweep/reconcile block, and the offline branch (`schema: None`
   → write anyway) is the load-bearing judgement — omitting the field is a guaranteed 422, sending it
   offline is a bet that every covered endpoint takes it. Sound for the four H3 Max endpoints; the
   verifier should confirm the call site's position and that the sweep (`app.rs:407-437`) keeps, not
   drops, the constant online. **Weakest link.**
2. **The `768p` height map.** A silent gap: without it the default-resolution price reads "unknown",
   and it is a `src-tauri/` change the audit/`price_check` will not catch (they price against
   `Billable::video`, not the settings path). `the_768p_tier_is_reachable` is the guard; confirm it
   is non-vacuous.
3. **One spec, two disagreeing endpoints (aspect on t2v only).** §D3/§1.1 argue from code that the
   online path is per-endpoint correct, but the proof needs `model_capabilities` run twice
   (`hasMedia` true/false); this design could not execute it. If wrong, the animate tab re-opens the
   S9 B4 class on a headline model. Reproduce with the throwaway-probe pattern S9 used.
4. **Array-length literals.** `media.rs:599` (51→53) and `registry.rs:2443` (+30→+32). Each fails
   loudly, but they are the highest-frequency edit and S8's history shows both moved in both
   directions.
5. **Audit check 2's subtraction.** If `constant_keys` is not wired into the filter, the four
   endpoints land in MISSING REQUIRED FIELDS and AC5 cannot be clean; if it is wired too broadly it
   could hide a genuine missing field on another route. The fixture test pins both directions.
