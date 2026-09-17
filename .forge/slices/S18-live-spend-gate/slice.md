# S18-live-spend-gate — SLICE (DISCOVER + PLAN)

Owner context: on 2026-09-16 a unit test spent $0.36 via the keychain fallback (see STATUS incident). The
vault hotfix closed tests; this slice closes every other path so spending is impossible without an
explicit, per-process opt-in — structural, not procedural.

## Delivers (one sentence, end-to-end)
No binary, example or test in this repository can spend money unless the process was started with an
explicit `HICKEYFIELD_ALLOW_SPEND=1`: every provider client construction and every live example checks
the flag, refuses with a clear message otherwise, and a repo-wide test proves no credential can be read
outside the vault — closing the last path the 2026-09-16 incident review found.

## Ground truth (S17 security.md; STATUS incident)
- `crates/hickeyfield-core/examples/first_light.rs:31` reads raw `FAL_KEY` (not the vault) and submits a
  real generation; its docstring (:20) tells the reader to copy the keychain key into `FAL_KEY`.
- `src-tauri/src/vault.rs::get` is the single vault read; `keychain_blocked()` covers cfg(test) and
  `HICKEYFIELD_NO_KEYCHAIN=1`. `src-tauri` has no integration tests/examples; core examples compile
  without cfg(test).
- Provider clients live in `src-tauri/src/clients.rs` (+ core adapters); `route::resolve` yields a
  provider only when a key is present.

## Acceptance criteria (testable)
- [ ] AC1 — Spend gate: a single `spend_gate::allowed()` (env `HICKEYFIELD_ALLOW_SPEND=1`, default off;
  the packaged app sets it at startup in `lib.rs` so real users are unaffected) checked in every
  provider client constructor / submit path; when disallowed, submit returns a typed
  `JobError::SpendBlocked` before any HTTP; tests: blocked by default in `cargo test`, allowed only with
  the flag, the app's startup sets it.
- [ ] AC2 — Examples: `first_light.rs` (and any other live example) refuses to run without the flag and
  prints what it would cost; reads the key via the vault (or a documented env) with the docstring's
  keychain-copy instruction removed; keyless examples (audit_fal, fal_diff, price_check) unaffected.
- [ ] AC3 — Repo-wide guard test: a test greps the source for `keyring::`/`get_password` outside vault.rs,
  for `FAL_KEY`/`hickeyfield_*_KEY` reads outside vault.rs, and for `remove_var("hickeyfield_` anywhere —
  failing the build if any appears.
- [ ] AC4 — Docs: FIRST-LIGHT.md and README state the two flags and the rule; CHANGELOG entry.
- [ ] AC5 (regression) — full gate green; keyless audit unchanged; app builds + launches and can still
  generate (flag set at startup) — verified by the owner, not an agent.
