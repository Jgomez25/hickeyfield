# S9-fal-schema-const-and-int-enums — SECURITY (evidence)

Read-only pass, scoped to `git diff main...HEAD`. What the slice actually touched:
`crates/hickeyfield-core/src/fal_schema.rs` (parser reads `const` and stringifies non-string
enum members; `reconcile` now coerces pinned settings and returns `Vec<Pinned>`),
`crates/hickeyfield-core/examples/audit_fal.rs` (check 5 `pin_conflicts`, fetch consolidated),
`src-tauri/src/app.rs:438-452` (advisory from schema-derived values),
`capability.rs`/`media.rs`/`use_case.rs`/`registry.rs` re-lands, `Cargo.toml` (`[[example]]
audit_fal test = true`), `CHANGELOG.md`. **No new dependency; `Cargo.lock` and `deny.toml` are
byte-identical to `main`** (`git diff main...HEAD --name-only` matches neither).

The central new trust question: a **remote, unauthenticated** OpenAPI document
(`https://fal.ai/api/openapi/queue/openapi.json?endpoint_id=…`, `fal_schema.rs:375-394`) can now
**write values into an outbound request body** (`fal_schema.rs:216`) and **into user-visible
advisory text persisted on the job** (`app.rs:446-453`), where before it could only *reject*.
That is the boundary everything below is measured against.

---

## Findings, by severity

### M1 — Medium. A pin can move a cost-bearing axis after the price was quoted; the estimate is never recomputed

`src-tauri/src/commands.rs:1013` stores `estimated_usd` from the **settings** at job creation.
`src-tauri/src/app.rs:438` then calls `reconcile`, which for a `const`-pinned field **overwrites
the body value** (`fal_schema.rs:216`) and only appends a prose advisory afterwards
(`app.rs:446-453`). Nothing re-prices, and `estimated_usd` is never updated.

Exploitation / failure path: fal publishes (or changes) a `const` on a cost-bearing key of a route
already shipped — e.g. `duration: "10s"` on a `CostModel::PerSecond`/`PerSecondTiered` route
(`registry.rs:934-959`), or a `resolution` pin one tier up. The schema is fetched live on the
submit path, so no app release is needed. A user re-runs a stored job ("Persisted so Rerun repeats
the generation the user actually ran", `commands.rs:1016-1017`) or has stale saved settings, sees
$X on the Generate button, and is billed for the pinned axis instead. Before this slice the same
mismatch was a `Refusal` — i.e. **zero spend**; the slice converts it into a spend at a value the
user did not choose. Today's live pins are benign (Veo Extend 7s/720p is mirrored into the spec at
`registry.rs:2142-2143` and priced at the fixed 7s; Seedance 2.5's `aspect_ratio: "auto"` is not
cost-bearing), so this is exposure rather than a present overcharge — hence Medium, not High.

Fix: after `reconcile` returns a non-empty `Vec<Pinned>`, either (a) re-evaluate the cost model
against the rewritten body and refuse if the new figure exceeds the stored `estimated_usd`
(a fixed tolerance, or any increase at all), or (b) restrict the Class-A coercion to axes that
cannot raise price and refuse on `duration`/`resolution`/audio pins that do. Design §2.2 chose
"coerce loudly" on honesty grounds and explicitly flagged this as "the decision most worth a second
opinion" (design.md:653) — it did not consider the estimate going stale.

### L1 — Low. The Class-B guard protects `prompt` only, not other user-authored text fields

`fal_schema.rs:209` refuses a rewrite only when `field == "prompt"` or the name matches the media
heuristic. `negative_prompt` is a real `ValueSpec::Text` flag on shipped models
(`registry.rs:2038`, `registry.rs:2209`) and therefore reaches the body through the settings loop
(`app.rs:251-271`).

Path: fal pins `negative_prompt` (or any future text control) with a `const`; the user's typed text
is replaced by the schema constant and sent, producing a render they did not describe. It is
disclosed — the advisory fires — but the advisory reads "always renders at <constant> negative
prompt", which does not say "we discarded what you typed". Severity Low: no live endpoint does this,
and the outcome is visible.

Fix: widen Class B to any field whose spec flag is `ValueSpec::Text` (or, minimally, any name
containing `prompt`), refusing rather than rewriting.

### L2 — Low. The media half of the Class-B guard is a name heuristic, while the caller already knows the real keys

`fal_schema.rs:209` asks `schema.media_keys()`, which matches on `_url` / `_urls` / `_image` /
`_video` suffixes (`fal_schema.rs:77-88`). `submit_to_provider` has already computed the *actual*
bound media keys from the dialect (`app.rs:213-215, 218-228`) and does not pass them in.

Path: a fal endpoint names a media field outside the heuristic (`input_images`, `frames`,
`reference`) **and** pins it with a `const`; the user's uploaded URL is silently replaced by the
constant, the render uses fal's asset, and the user is billed for a generation that never touched
their file — the exact Gemini Omni class the module exists to stop, re-entered through the new
write path. Every fal media key in today's dialect table (`media.rs:183-222`) matches the heuristic,
so this is latent. Severity Low.

Fix: give `reconcile` the caller's `media_keys` (or take a `&[&str]` of protected keys) instead of
re-deriving them from names.

### L3 — Low. No length or cardinality bound on schema-derived text that reaches the body, the DB and the UI

`parse` (`fal_schema.rs:278-352`) copies `const` and enum members verbatim with no cap; `reconcile`
writes one into the body (`:216`) and `app.rs:446-453` interpolates `p.to` / `p.field` into an
advisory that is persisted (`store.rs:242-255`) and rendered. `fetch` (`fal_schema.rs:384-393`)
reads the response body with no size limit (pre-existing). The slice widens what is retained
(numbers as well as strings, plus `pinned`), so it is worth naming rather than a new hole.

Path: a compromised or hostile `fal.ai` response returns a multi-megabyte `const`, or a control-char
/ bidi-override string; it lands in the request body, in an advisory row in SQLite, and in the meta
rail forever. Impact is bounded (below), but the job record is permanently polluted and the advisory
can be made to read as something the app did not say.

Fix: cap a recorded `const`/enum member (e.g. 256 chars) and the member count, and skip anything
longer; optionally cap the fetch body.

### L4 — Low, PRE-EXISTING, NOT introduced here. `cargo deny check advisories` fails on `main` as well

Run in this session (`cargo deny check advisories`, from `~/.cargo/bin/cargo-deny`): **advisories
FAILED** with two vulnerabilities plus one stale ignore:

- `RUSTSEC-2026-0258` — `h2 0.4.15`, unbounded empty DATA frames (low severity upstream; patched in
  0.4.16). Reaches us through `reqwest 0.13.1` / `hyper 1.11.0` / `tauri 2.11.5`.
- `RUSTSEC-2026-0285` — `rustls 0.23.43`, TLS 1.3 handshake messages accepted across encryption
  level boundaries. Transcript still authenticated, so not a handshake-forgery.
- `warning[advisory-not-detected]` at `deny.toml:77` — the `RUSTSEC-2024-0429` glib ignore no longer
  matches any crate.

**Confirmed not introduced by this slice**: `Cargo.lock` and `deny.toml` do not appear in
`git diff main...HEAD --name-only`, and `deny.toml` was last touched in `ab06e31` (2026-08-05). The
implementer's report at implement.md is accurate. Fix belongs in its own maintenance slice:
`cargo update -p h2 -p rustls`, then drop the dead `deny.toml:77` line.

---

## Boundaries examined where I found nothing

- **XSS / UI injection from schema text.** Advisories render as React text children
  (`ui/src/components/MetaCard.tsx:170-174`); `grep dangerouslySetInnerHTML ui/src` returns nothing.
  Escaped. No finding beyond L3's length concern.
- **SQL injection.** Advisories are `serde_json::to_string`'d and bound as `?20` in a parameterised
  statement (`store.rs:242-255`). No string-built SQL anywhere in the slice's reach. No finding.
- **Prompt injection via fetched content.** Schema-derived strings reach the request body, the job
  record, the UI and `tracing`. They do **not** reach the enhancer or any LLM: the enhancer consumes
  `job.prompt` only, and no advisory/schema value is read back into a model call. The one place a
  constant could displace user text is L1, which is a substitution, not an injection into a model's
  instructions. No finding.
- **Command / path injection.** The slice introduces no `Command`, no `fs` write, no path built from
  schema content. `audit_fal` writes to stdout only. No finding.
- **SSRF / egress.** The only URL built is
  `https://fal.ai/api/openapi/queue/openapi.json?endpoint_id={endpoint}` (`fal_schema.rs:384-387`,
  unchanged by this slice). `endpoint` is derived from compile-time registry slugs through
  `media::resolve_endpoint`, never from user input or from fetched content; the scheme and host are
  literals, so a schema cannot redirect the next fetch. The interpolation is unencoded — pre-existing,
  and not reachable with attacker-controlled data. No finding.
- **Secrets.** `fetch` sends no `Authorization` header and no key (`fal_schema.rs:378-389`);
  `audit_fal` contains no `env`/`KEY`/`Authorization`/token reference at all (grepped) — the audit
  stays keyless as claimed. The new advisory and the new `tracing::warn!` interpolate only
  `model.display_name` and schema field/value text — no key, no URL, no media URL. No finding.
- **Destructive-op gates / authz / tenant isolation.** Single-user desktop app; the slice adds no
  command, no IPC surface, no file or DB deletion. `Cargo.toml`'s `[[example]] audit_fal test = true`
  compiles the example as a test harness, so `cargo test --workspace` runs its `#[test]` fns and
  **not** `main()` — no network call is added to CI, and `pin_conflicts` is pure (schema passed in).
  No finding.
- **Re-land surface** (`media.rs` FAL_ROUTE_MODES 48→51, `use_case.rs` guards, `registry.rs`
  specs/routes). These are vendored constant tables; no untrusted input, no new privilege. The
  Seedance 2.5 `InputMode::Image` re-land is gated by a per-endpoint schema fetch, so the t2v
  contract is unaffected (`capability.rs:1246-1290` proves both). No finding.
- **Behavioural regression check.** `cargo test -p hickeyfield-core --lib fal_schema` — 33 passed,
  0 failed, including the Class-B refusal test
  (`a_pinned_prompt_or_media_key_is_refused_not_rewritten`, `fal_schema.rs:766-780`). The design's
  claim that a `const` on `prompt`/`image_url` is **refused, not applied**, is verified in code
  (`fal_schema.rs:209-214`) and by test, with the body left untouched.

---

## Recommended to the orchestrator

No Critical/High, so no finding here forces a new slice. Two are worth backlog entries at the
owner's discretion (I do not edit `.forge/backlog.md`):

1. **`pin-aware-cost-estimate`** (from M1) — re-price or refuse when a `const` rewrite moves a
   cost-bearing axis after `estimated_usd` was stored.
2. **`dependency-advisories-h2-rustls`** (from L4, pre-existing on `main`) — `cargo update -p h2
   -p rustls`, delete the stale `deny.toml:77` ignore, get `cargo deny check advisories` green.

L1/L2/L3 are cheap hardening of `reconcile`'s new write path and could ride along with either.

VERDICT: PASS
