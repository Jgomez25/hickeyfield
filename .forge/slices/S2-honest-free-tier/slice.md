# S2-honest-free-tier — SLICE (DISCOVER + PLAN)

## Delivers (one sentence, end-to-end)
The dead `z_image`/`Local` "free tier" no longer masquerades as usable: instead of a "$0.00 / available / Generate" tile that fails at submit with "add a key for Local", the route presents an explicit, honest unavailable state (no local client exists yet).

## Acceptance criteria (testable)
- [ ] AC1 — The route/availability path reports `Local`-only routes (e.g. `z_image`) as UNAVAILABLE with an honest reason, using the existing `RouteDto`/unavailable-reason pattern — NOT available-at-$0.00. Root cause: `provider.rs` `has_adapter()` returns `true` for `Local` but `app.rs:34` `client_for` returns `None` and `clients.rs:413` has no `Local` client. A Rust test on the availability/DTO path asserts the unavailable state + reason.
- [ ] AC2 — (DECISION: hide, consistent with the 3 other unroutable models). `vitest` UI test: the `z_image`/Local-only model is NOT presented as a runnable/Generate tile in the use-case model picker (it is filtered out like other unroutable models). The honest unavailable+reason is asserted at the data layer in AC1 (route_state DTO), not as an in-tab disabled tile.
- [ ] AC3 (no false negatives) — Routes that ARE reachable (fal, Higgsfield with a key) still report available; the fix must not mark real routes unavailable. Test covers at least one reachable route staying available.
- [ ] AC4 (regression) — Global gate green: `cargo test --workspace` (0 failed, >=727 now), `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `./scripts/lint-provenance.py`, `pnpm build`, `pnpm test`. App still builds+launches.

## Approach + why this slice now
Independent of S1, no fal key needed. The honest fix is to make `has_adapter()` (or the availability/DTO layer that consumes it) tell the truth: `Local` has no runtime client today, so `Local`-only routes are unavailable. Preferred seam = align the availability signal with the real `client_for` capability rather than the aspirational `has_adapter()`. Small, high-trust user-facing correctness fix. Keeps a clean path for the future Local/ComfyUI client to flip these back to available.
