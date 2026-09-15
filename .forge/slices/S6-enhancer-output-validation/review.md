# S6-enhancer-output-validation — CODE REVIEW

Reviewer: read-only. Scope: the slice's working-tree diff only.
Files touched by the slice: `crates/hickeyfield-core/src/enhancer.rs` (code + tests) and
`.forge/state.json` (FORGE bookkeeping). No other source, doc, config, manifest, or UI file
changed — confirmed via `git status` / `git diff --name-only`. `Cargo.toml`/lockfiles
untouched (no new deps).

Verification run (read-only):
- `cargo test -p hickeyfield-core enhancer::tests::refusal` → 7 passed, 0 failed.
- `cargo test -p hickeyfield-core enhancer::tests::the_funnel` → 3 passed, 0 failed.
- `cargo clippy -p hickeyfield-core --all-targets -- -D warnings` → clean, exit 0.

## What I checked and found clean

### Correctness of `refusal_reason` (enhancer.rs:609-668) — CLEAN
- Anchoring is genuinely to the START, not a substring search. Trace:
  `lower` (line 611) → `trim_start_matches` strips only *leading* quote chars (:649) →
  `trim_start` (:650) → at most ONE short lead-in ("sure, ", "sure ", "okay, ", "okay ",
  "ok, ", "well, ") is stripped, with a `break` after the first match (:651-656) →
  `head.starts_with(p)` (:657). A refusal word appearing mid-sentence is therefore never
  matched. The AC4 negative sample ("A soldier whispers sorry as the rain falls…") stays
  `None` — verified by `refusal_reason_does_not_trip_on_sorry_mid_sentence`, which passes.
- The lead-in strip is bounded: `trim_start_matches` only consumes leading quote/whitespace,
  and exactly one known short lead-in is removed. It cannot "skip half the reply." A scene
  like "Surely footing…" is not over-stripped because `strip_prefix("sure ")` requires the
  trailing space.
- The `< 12`-char floor uses `t.chars().count()` (:663), i.e. character count, not byte
  indexing — multibyte-safe.
- The deviation documented in implement.md (prefix-anchor instead of the design's literal
  "first ~40 chars" substring window) is a correct, stronger interpretation of the same
  AC4 requirement; the design text itself said "anchor to the beginning, NOT a substring
  anywhere." Behaviour on every named AC case matches the spec.

### Funnel guard (enhancer.rs:532-546) — CLEAN
- Placed after the empty-reply guard, gated on `out.status.changed()`, and returns via
  `Rewritten::failed`. `Rewritten::failed` sets `prompt == original` and status `Failed`
  (:449-458), so `changed()` (:473-474) becomes false and the ORIGINAL is submitted with
  the note surfacing through `note`. Mirrors the existing empty-reply fallback exactly.
- Good-rewrite path is not regressed: `refusal_reason` returns `None` for a real rewrite,
  so `out` is returned unchanged (status `Rewritten`, note `None`). Verified by
  `the_funnel_passes_a_genuine_rewrite_through_unchanged`.

### Tests — CLEAN (non-tautological)
- `refusal_reason_*`: plain refusal, apology-before-refusal, meta ("As an AI"), leading
  quote / "Sure," lead-in, too-short, real cinematic rewrite (None), and the mandated
  mid-sentence-"sorry" negative (None). All exercise real inputs and assert exact outputs.
- Funnel tests drive a fake `Enhancer` (`Stub`) end-to-end through `enhance_or_original`
  and assert the original is submitted with a note on refusal, and the good rewrite passes
  through. These are behavioural, not asserted.

### Model-agnostic (AC3) — CLEAN
- `enhance_or_original` is the sole production submit path: the only non-test call site is
  `src-tauri/src/harness.rs:85`. Every other `.rewrite(` reference is a test. Both
  `LocalEnhancer` and `HostedEnhancer` reach the provider only through this funnel, so the
  guard covers both backends identically.

### AI-tells — CLEAN
- No invented/nonexistent APIs. `Rewritten::failed`, `RewriteStatus::changed`,
  `to_ascii_lowercase`, `trim_start_matches`, `strip_prefix`, `chars().count()` are all
  real and used correctly.
- No comment contradicting the code; docs on `refusal_reason` accurately describe the
  anchoring behaviour.

## Findings (ranked)

### Minor — redundant / unreachable entries in `REFUSALS` (enhancer.rs:634-642)
`starts_with` on the shorter prefixes already subsumes several later entries:
`"i can't help"` (:635), `"i can't create"` (:640), `"i can't generate"` (:642) are all
covered by `"i can't"` (:616); `"i cannot fulfill"` (:634), `"i cannot assist"` (:636),
`"i cannot create"` (:639), `"i cannot generate"` (:641) are all covered by `"i cannot"`
(:618). They can never add a match. Standard: dead/duplicated code. Concrete fix
(follow-up, not required to merge): drop the subsumed entries, keeping only distinct
prefixes such as `"cannot assist"` (:637), which is *not* subsumed.

### Minor — `< 12` floor can downgrade a legitimate terse rewrite (enhancer.rs:663)
A genuinely good but very short rewrite (< 12 chars) that comes back with status
`Rewritten` would be discarded and the original submitted. The direction is SAFE (the user
gets their own prompt plus an honest note, never a billed blank/garbage), and the design
fixes the floor at 12 deliberately, so this is acceptable for the slice's scope. Standard:
possible false positive. Fix (optional follow-up): if terse-but-valid rewrites are ever
observed, lower the floor or gate it on word count.

### Minor — note wording is slightly awkward for the too-short reason (enhancer.rs:540-542)
When `why` is "the reply was too short to be a prompt", the composed note reads "The model
declined to rewrite this prompt (the reply was too short to be a prompt), so your original
was used." Grammatical and honest, just clumsy. Standard: clarity. Fix (optional): phrase
the too-short branch separately, or reword the outer template.

## Not in scope / not regressed
Echo handling is intentionally unchanged (documented limitation); existing `changed()`
logic already covers it. Empty-reply and precheck guards are untouched and still pass.

No blocker or major findings on any critical path. The three minor items are non-blocking.

VERDICT: PASS
