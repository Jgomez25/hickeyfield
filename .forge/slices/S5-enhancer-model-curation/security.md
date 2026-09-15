# S5-enhancer-model-curation — SECURITY (evidence)

Read-only review of the slice diff only (enhancer.rs, commands.rs, ui/api.ts,
ui/types.ts, ui/EnhancerPicker.tsx + tests). This is a low-surface, mostly-pure
change: model-suitability tiering over the Ollama `/api/tags` list. Reviewed
proportionately.

## Scope reviewed

- `parse_local_models` / `model_tier` / `parse_param_billions`
  (`crates/hickeyfield-core/src/enhancer.rs:1024-1173`)
- `select_rewriter` / `list_ollama_models` (`src-tauri/src/commands.rs:733-802`)
- UI consumption of the tiered list (`ui/src/components/EnhancerPicker.tsx`,
  `ui/src/api.ts`, `ui/src/types.ts`)

## Findings

No Critical, High, Medium, or Low findings. Details of each boundary examined
below — silence would not be clearance, so each is stated explicitly.

### Malformed / hostile `/api/tags` response — graceful, no panic (PASS)

`parse_local_models` (enhancer.rs:1024-1050) degrades safely on every hostile
shape:
- Missing/wrong-typed `models`: `v.get("models").and_then(Value::as_array)`
  returns `None` → `Vec::new()` (enhancer.rs:1025-1027). Covered by the
  `"models": "nope"` and `{}` tests (enhancer.rs:2103-2104).
- Missing `name`, missing `capabilities`, missing `details.parameter_size`,
  wrong types: all read through `.get().and_then(Value::as_str)` /
  `filter_map`, so a bad entry is skipped or falls back — never indexed, never
  `.unwrap()`ed. No indexing or `.unwrap()`/`.expect()` exists on the parse
  path (those appear only in `#[cfg(test)]` and the ignored live-daemon test).
- Non-UTF8 / huge strings: `serde_json` string values are always valid UTF-8;
  a huge `parameter_size` is scanned once, linearly (see below). The fetch
  itself is bounded by `LOCAL_LIST_TIMEOUT` (enhancer.rs:1028).
- `sort_by_key(tier_rank)` is total and infallible; `split(':').next()` always
  yields ≥1 element and the `unwrap_or(&lower)` is belt-and-suspenders
  (enhancer.rs:1112).

### `parse_param_billions` — no ReDoS, no panic (PASS)

Despite the `\d+(\.\d+)?b` phrasing in the doc comment, the implementation is a
hand-rolled single-pass byte scanner (enhancer.rs:1131-1173), **not** a regex —
so there is no regex engine and no backtracking. It is strictly O(n) over the
input, so no ReDoS is possible even on an adversarial `parameter_size`.
Panic-safety: `s[start..i]` slices only ever start on an ASCII digit byte and
end on the byte after ASCII digits/dots (a `b`/`B` or end), both guaranteed
char boundaries, so the slice cannot panic on multibyte input; `seen_dot`
prevents a multi-dot string reaching `parse`; and the `parse::<f64>` result is
handled with `if let Ok`, so a non-parseable token is simply ignored. No
integer/index arithmetic that can overflow or go out of bounds.

### Model name → Ollama request — no injection (PASS)

The selected/auto model name flows `select_rewriter` → `Rewriter::Ollama{model}`
→ `ollama_body(model, ...)` (enhancer.rs:916-920) → `.json(&body)`
(enhancer.rs:987). It is placed as a JSON value via `serde_json::json!` and
serialized by reqwest, so any special characters in a model name are escaped by
the JSON serializer — no string interpolation into the body, no header, and no
URL. There is **no** shell/process invocation anywhere on this path
(`Command::`/`process::` absent from the diff and file), so command injection is
not reachable. In the UI the name is rendered as React text content and as an
`<option value>` (EnhancerPicker.tsx:150-152) — React escapes both, so no XSS.
The `data-tier` attribute is bound to the closed Rust enum (`recommended` /
`neutral` / `discouraged`), not free text.

### SSRF / egress — unchanged, localhost-const (PASS)

`local_models` is only ever called with `hickeyfield_core::clients::OLLAMA_URL`
(commands.rs:801), a hardcoded `http://127.0.0.1:11434` (clients.rs:673); the
tags path is the const `OLLAMA_TAGS_PATH = "/api/tags"` (enhancer.rs:106). The
slice introduces no user- or daemon-controlled URL and no new outbound target.

### Secrets — none on this path (PASS)

This path handles no API keys. `HostedEnhancer` (keys) is untouched by the
logic change. Nothing new is logged or persisted; the only interpolation of
untrusted data into a string is the daemon's own response inside an error
message (enhancer.rs:965), which is localhost data, not a secret.

### Dependency / supply chain — no change (PASS)

No manifest or lockfile change in the diff (`git diff` shows no `Cargo.toml`,
`Cargo.lock`, or `package.json`). No regex crate (or any crate) was added — the
`\d+(\.\d+)?b` behavior is implemented by hand. The pre-existing baseline
h2/rustls advisories are out of scope for this slice and unchanged.

### Authz / tenant isolation / destructive ops (PASS)

Not applicable: this is a local desktop Tauri command over a loopback daemon,
no multi-tenant boundary, no destructive operation, no privilege change. Auto
model selection can only pick from models the local daemon reports installed
(commands.rs:762-782); a "discouraged" tier is a warning, not a gate, which is
the documented intent and carries no security consequence.

## Backlog

No Critical/High findings, so no new security slices are recommended to the
orchestrator.

VERDICT: PASS
