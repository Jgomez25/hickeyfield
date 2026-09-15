# S2-honest-free-tier — RELEASE

**Gate:** `python3 .forge/gate.py` → exit 0. All phases evidenced; TEST + REVIEW + SECURITY = PASS.

**What shipped:** the dishonest `Local` free tier is fixed at its root. `has_adapter()`
(`crates/hickeyfield-core/src/provider.rs`) now returns true only for providers with a real
runtime client (`Fal | Higgsfield`), dropping the aspirational `Local`. The sole Local-only
model `z_image` is therefore unroutable and filtered out of use-case tabs (consistent with the
3 other unroutable models), and `route_state` reports it unavailable with the honest reason
"Hickeyfield has no client for Local yet" instead of "$0.00 / Generate" that failed at submit.

**Regression (objective §A):**
- `cargo test --workspace` → 740 passed / 0 failed / 9 ignored.
- `cargo fmt --all --check` → 0 · `cargo clippy --workspace --all-targets -- -D warnings` → 0.
- `./scripts/lint-provenance.py` → exit 0.
- `cd ui && pnpm test` → 216 passed (incl. new `ModelPicker.test.tsx`) · `pnpm build` (tsc+vite) → ok.
- `tauri build --bundles app` → (recorded at release: built + launch smoke test).
- `cargo deny check` → baseline advisories only (h2/rustls; no manifest change) → separate slice.

**Security:** net improvement — removes a submit path that reached `client_for`→None; backend
guard enforced (`route::resolve` filters on `has_adapter`), so a crafted `local:z-image` route_id
cannot be forced through. No new findings.

**Landing:** commit on branch `forge/s2-honest-free-tier` (off the S1 branch head; local only, no push).

**Minor follow-ups (non-blocking):** ModelPicker.test.tsx describe/comment wording says
"hides/filtered" but asserts a disabled tile in the flat list (cosmetic); empty `Local` section
header in `clients.rs` now labels a client-less section (pre-existing; resolved when a Local
client ships). The Local/ComfyUI client itself remains future work (re-enables z_image via one edit).

**Status:** released (local branch).
