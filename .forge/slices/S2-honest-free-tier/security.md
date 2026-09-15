# S2-honest-free-tier — SECURITY (evidence)

Read-only review of the slice's working-tree diff (uncommitted on
`forge/s2-honest-free-tier`). Scope: only what this slice touched, not a
whole-repo audit.

## What changed (attack surface)

The only production-code change is one line of behaviour in
`crates/hickeyfield-core/src/provider.rs:96` — `has_adapter()` drops
`ProviderId::Local`, so it now returns `true` for `Fal | Higgsfield` only.
Everything else in the diff is test code:

- `crates/hickeyfield-core/src/registry.rs` — `#[cfg(test)]` only (adds `z_image`
  to the expected unroutable set).
- `crates/hickeyfield-core/src/route.rs` — `#[cfg(test)]` only (new test
  `a_local_only_route_cannot_be_resolved_until_a_client_exists`, comment edits).
- `src-tauri/src/app.rs` — `#[cfg(test)]` only (drift-guard + free-tier tests).
- `src-tauri/src/commands.rs` — `#[cfg(test)]` only (route_state tests).
- `ui/src/components/ModelPicker.test.tsx` — new vitest test, no production code.

This is a strictly-more-restrictive, negative-surface change: `has_adapter()`
now returns `false` where it used to return `true`, which can only remove
reachable routes, never add them.

## Boundaries examined

### Authz / capability signal — the load-bearing question (CLEAR)
The availability signal now matches real capability, and the guard is enforced
on the **backend**, not just the UI:

- `route::resolve` builds `reachable` by filtering on
  `available.contains(&r.provider) && r.provider.has_adapter()`
  (`crates/hickeyfield-core/src/route.rs:153`). With `has_adapter(Local)=false`,
  a Local route is never selected — including the cheapest-policy path, where
  `z_image`'s `$0.00` route previously won and then failed at submit.
- The **pinned** path also checks `has_adapter()` first
  (`crates/hickeyfield-core/src/route.rs:133`): a crafted `route_id` of
  `local:z-image` returns `RouteError::PinUnusable`, never a usable `Route`.
- `submit_job` resolves through exactly this function
  (`src-tauri/src/commands.rs:729`) before it ever calls
  `submit_to_provider` → `client_for`. So a Local route cannot be forced through
  the backend by a crafted `route_id` even if the UI filtering were bypassed.
  `z_image`'s only route is `local:z-image` (`registry.rs:1334`), so it is fully
  unreachable end-to-end.

Exploit path that is now CLOSED: previously an agent/user submitting
`model_id=z_image` (or pinning `local:z-image`) reached `client_for` →
`None` (`src-tauri/src/app.rs:52`) and surfaced a misleading "add a key for
Local" error on a keyless provider — a confusing dead end, not a breach, but a
real footgun. It is now refused earlier with an honest `NoAdapter`. No new path
is opened.

### Injection (SQL / command / path / template / XSS / prompt) — CLEAR
No new external input is parsed. The one new string is static and
server-generated: `"Hickeyfield has no client for {display_name} yet"`
(`src-tauri/src/commands.rs:212`), where `display_name` is a fixed enum label,
not user data. No query, shell, path, or template is constructed from it. The
UI test renders `unavailableReason` into a `title` attribute, but that value is
fixture data / a static backend string, not attacker-controlled content.

### Info leak in the unavailable reason — CLEAR
`"Hickeyfield has no client for Local yet"` contains only a provider display
name. No secret, key, path, or internal identifier. Confirmed fine.

### Secrets handling — CLEAR (improved)
`client_for` still reads the keychain per-request and returns `None` for
non-adapter providers (`src-tauri/src/app.rs:34-54`); unchanged. `route_state`
checks `has_adapter()` *before* `needs_key()` (`commands.rs:208` then `:217`),
so a keyless provider can no longer be told to "add a key" — the specific lie
this slice removes. `needs_key(Local)` still returns `false`
(`provider.rs:101`), preserving the keyless fact for the day a local adapter
lands (flip `has_adapter` back and it re-enables everywhere).

### Dependency / supply-chain — CLEAR
No manifest change: `git diff` over `*.toml`, `*.lock`, `package.json`,
`pnpm-lock.yaml` is empty. The pre-existing h2/rustls advisories are baseline,
unchanged, and out of scope for this slice. The new UI test imports only
existing tooling (`vitest`, `react`, `react-dom/client`); no dependency added.

### Regression to fal universal-uploader / local-endpoint detection — CLEAR
`uploader_for` (`src-tauri/src/app.rs:64`) matches on `ProviderId` directly with
a fal fallback and never consults `has_adapter()`; unaffected. The
Ollama/local-endpoint enhancer paths (`detect_local`, `is_ready`,
`local_endpoints` at `commands.rs:47-61`) key off genuine endpoint detection and
`needs_key`, not `has_adapter`; `is_ready()` already refuses to count keyless
Local as ready. A grep confirms `has_adapter` is referenced only in the
route/availability layer, never in the uploader or enhancer paths. Unaffected.

## Findings

None. No Critical, High, Medium, or Low finding at any boundary examined. The
change is a net security improvement (closes a confusing keyless-submit dead
end) and is otherwise neutral — strictly more restrictive, no new input, no new
egress, no new dependency, backend guard confirmed enforced independent of the
UI.

## Backlog recommendation

None required from this slice (no Critical/High). Unrelated standing item, not
introduced here: the baseline h2/rustls advisories remain tracked in their own
slice.

VERDICT: PASS
