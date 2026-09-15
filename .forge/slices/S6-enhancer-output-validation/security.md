# S6-enhancer-output-validation — SECURITY REVIEW (light, proportionate)

Reviewer: read-only. This is a small, pure-logic change (`refusal_reason` + one funnel
guard) in `crates/hickeyfield-core/src/enhancer.rs`. Review scaled accordingly.

## Panic / DoS surface on adversarial model output — CLEAN
`refusal_reason` treats the reply as fully untrusted (it is raw model output). Checked:
- No `unwrap`, `expect`, `panic`, `[]` indexing, or byte-offset slicing in the added code.
- `t.chars().count()` (enhancer.rs:663) counts characters — no byte-boundary panic on
  multibyte / non-ASCII input.
- `to_ascii_lowercase()` (:611) only rewrites ASCII bytes and leaves other bytes intact;
  it allocates one String of the same byte length. `trim`, `trim_start`,
  `trim_start_matches`, `strip_prefix`, and `starts_with` are all char-boundary-safe and
  allocation-free.
- Cost is O(n) in the reply length with a single O(n) allocation — no quadratic blowup, no
  unbounded recursion, no backtracking (fixed prefix list, no regex). A "huge reply" costs
  linear time/memory, and enhancer replies are already bounded upstream by the provider
  client. No practical DoS.

## Net security/correctness effect — POSITIVE
The change prevents shipping untrusted model output (a refusal/apology/meta-comment) to a
paid downstream generator. Before S6 a refusal like "I can't complete this task." was
stored as a "Rewritten" prompt and billed to the paid provider. Now the funnel restores the
user's original with an honest note. This strictly reduces the blast radius of malformed or
adversarial enhancer output; it adds no new trust in that output (it is only pattern-matched
to decide whether to DISCARD it, never executed or forwarded on match).

## Bypass analysis — CLEAN
- `enhance_or_original` (enhancer.rs:516) is the single production submit path; the only
  non-test caller is `src-tauri/src/harness.rs:85`. No production code calls
  `Enhancer::rewrite` directly, so a refusal cannot reach the generator via a path that
  skips the guard.
- The guard fires whenever the reply is a genuine rewrite candidate (`status.changed()`).
  If a backend instead returns status `Failed`/`Refused`, `changed()` is already false and
  the original is submitted anyway — so the "skip the guard" case is itself safe.
- Detection is best-effort English-phrase matching: a refusal phrased outside the pattern
  list or in another language would slip through. This is an availability/quality gap
  (worst case reverts to the pre-S6 behaviour of forwarding that text), NOT a security
  escalation, and it is an honestly documented limitation. Non-blocking.

## Secrets / network / dependencies / manifest — CLEAN
- No secrets, credentials, tokens, or logging of sensitive data added.
- No network calls, no I/O — pure string logic.
- No new dependencies; `Cargo.toml` / lockfiles unchanged. Baseline transitive crates
  (h2 / rustls) are untouched and out of scope for this slice.
- The user's email is not referenced anywhere in the change.

## Verification (read-only)
- `cargo clippy -p hickeyfield-core --all-targets -- -D warnings` → clean, exit 0.
- Slice refusal + funnel tests pass (10 new tests).

No security blockers or majors. The change is a net safety improvement.

VERDICT: PASS
