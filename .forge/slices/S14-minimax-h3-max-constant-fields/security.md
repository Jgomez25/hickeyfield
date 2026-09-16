# S14-minimax-h3-max-constant-fields — SECURITY (evidence)

Read-only security pass over the slice diff (`git diff main...HEAD`, HEAD e953d65). Scope:
the constant-fields mechanism, its call site, the audit, the two new models, the 768p map,
and the CHANGELOG. No source, docs, or config changed by this review.

## Findings

**None.** No Critical/High/Medium/Low finding with a concrete exploitation path was found
at any boundary examined. The per-boundary results below record what was checked and why it
is clear — silence is not clearance, so each boundary is stated explicitly.

## Boundaries examined

### 1. Injection — constant body fields (`media.rs:789-826`, `fal_schema.rs:260-297`)
- The constant table `FAL_CONSTANT_FIELDS` (`media.rs:812-817`) holds two rows, both string
  **literals** (`"minimax/h3-max"` / `"minimax/h3-max-turbo"` → `("prompt_expansion_mode",
  "balanced")`). No value is user-, network-, or schema-derived; `fal_constants`
  (`media.rs:823-826`) is a pure lookup returning `&'static` data. An attacker has no input
  channel into what key/value is injected. CLEAR.
- `apply_constants` (`fal_schema.rs:277-297`) can only (a) refuse to overwrite an
  already-present key (`body.contains_key` guard, line 283), and (b) with a live schema,
  insert only a key the endpoint `accepts` (line 287). It cannot overwrite a user/binder
  field of the same name, and online it cannot inject a key the endpoint rejects (that key
  is dropped). Verified by tests `a_constant_never_overrides_a_user_value` and
  `a_constant_is_dropped_when_the_schema_rejects_the_key`. CLEAR.
- Offline (no cached schema) branch inserts the literal regardless (line 285 `is_none_or`).
  Failure path if a future table row named a key some endpoint of the family rejects: that
  endpoint would 422 after a round trip — **no money is billed on a 422**, and the branch is
  bounded to the literal table (currently only the H3 Max family, which all four endpoints
  declare `prompt_expansion_mode`). This is a documented functional constraint, not a
  security exposure, and check 6 of the keyless audit catches a mistyped/unaccepted constant
  at build/audit time (`audit_fal.rs:282-296`). CLEAR.

### 2. Prompt injection via `prompt_expansion_mode`
- The value `"balanced"` instructs fal to rewrite the user's prompt server-side. This is
  fal's own feature over the user's own prompt; the slice introduces no new fetched- or
  attacker-content path into the prompt. The value is a fixed literal, not attacker-settable
  in a way that changes behavior beyond what fal already exposes. CLEAR.

### 3. Money-safety — can the constant change what fal bills?
- Schema description text (all four `schemas/*.json`, `prompt_expansion_mode`): *"How much
  effort to spend rewriting the prompt before generation. 'disabled' skips prompt expansion.
  'balanced' returns in about a second. 'quality' spends up to ~30s on a richer prompt."*
  Per the schema's own words the field controls **prompt-rewriting effort/latency**, not
  output video length. Billing is `CostModel::PerSecondTiered` over the separate `duration`
  field (`registry.rs:1162-1181`), which the constant does not touch. From the schema text I
  can verify the constant does not lengthen the billed clip. The estimate is per-second
  tiered and, per the task, the S10 settled-actual-cost seam catches any drift. CLEAR.
- Prices (`registry.rs:1162-1181`) are literal tiers with an inline provenance note ("fal
  index 2026-09-16", promotional launch rate) and are classified `Prose` by `prices::audit`
  (not build-failing) — no snapshot value is fabricated as verified. CLEAR.

### 4. Call-site ordering / logging (`app.rs:320-331`)
- The one new call is guarded by `route.provider == ProviderId::Fal`, runs after the schema
  fetch and before reconcile, and only mutates `body`. No new value reaches logs, telemetry,
  or error strings; nothing secret is added to the request beyond the literal constant.
  CLEAR.

### 5. Secrets / keyless audit (`examples/audit_fal.rs`)
- Grep for `env|KEY|Authorization|api_key|secret|token` across the audit example: **no
  matches**. The example reads no key and no environment; it hits fal's unauthenticated
  schema endpoint only. The new pure helpers `unsupplied_required` and `constant_issues`
  operate on already-fetched schema data and literal constants — no I/O, no secrets. CLEAR.

### 6. `resolution_height` 768p map (`commands.rs:619`)
- One literal arm `"768p" => Some(768)` on a `match` over a lowercased label. No user string
  reaches a query, path, command, or template — it maps to a `u32` used for price height.
  No injection surface. CLEAR.

### 7. SSRF / egress / path / tenant isolation
- The slice adds no new URL construction, no new outbound host, no filesystem path handling,
  and no authz/tenant boundary. `image_url`/`end_image_url` binding is pre-existing (S8/S9)
  and unchanged by this slice. Nothing to exploit here. CLEAR.

### 8. Rendered snapshot content (`registry.rs`)
- No `thumbnailUrl` (or any snapshot-sourced URL) is rendered by the new registry rows —
  confirmed absent in the registry diff. The vendored `schemas/*.json` contain a
  `thumbnailUrl` inside fal's **output** schema definition, but those files are consumed only
  by tests/audit and are not rendered or fetched by the app. CLEAR.

### 9. Dependency / supply-chain
- No manifest change in the slice diff; no new crate, no new feature. `cargo deny` posture
  unchanged, as stated in implement.md. CLEAR.

## Recommendation to the orchestrator
No Critical/High findings — no new backlog security slice is required from this pass.

VERDICT: PASS
