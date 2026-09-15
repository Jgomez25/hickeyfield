# S6-enhancer-output-validation — ADVERSARIAL TEST

## Implementer's claim (verbatim, from implement.md)

> When the enhancer model replies with a refusal/apology/meta-comment (or a reply
> too short to be a scene) instead of a rewritten prompt, the single funnel
> `enhance_or_original` now detects it, restores the user's ORIGINAL prompt, and
> attaches an honest note — so a refusal is never submitted verbatim to a paid
> provider. Model-agnostic: the guard lives in the one funnel every backend
> (Local/Ollama + Hosted/OpenAI) passes through.

## Commands (real output)

### git diff --stat — change is confined
```
 .forge/state.json                       |   8 +-
 crates/hickeyfield-core/src/enhancer.rs | 181 ++++++++++++++++++++++++++++++++
 2 files changed, 188 insertions(+), 1 deletion(-)
```
Only `enhancer.rs` + `.forge` are touched. No UI/DTO/command/dependency file changed. PASS.

### AC1 — pure `refusal_reason` unit tests
```
test enhancer::tests::refusal_reason_flags_a_meta_comment ... ok
test enhancer::tests::refusal_reason_tolerates_a_leading_quote_or_lead_in ... ok
test enhancer::tests::refusal_reason_flags_an_apology_even_before_the_refusal ... ok
test enhancer::tests::refusal_reason_does_not_trip_on_sorry_mid_sentence ... ok
test enhancer::tests::refusal_reason_flags_a_plain_refusal ... ok
test enhancer::tests::refusal_reason_flags_a_reply_too_short_to_be_a_prompt ... ok
test enhancer::tests::refusal_reason_passes_a_real_cinematic_rewrite ... ok
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 655 filtered out
```
Read the tests (enhancer.rs:1616-1707): they are non-vacuous, each asserting a
concrete `Some(...)`/`None`. "I can't complete this task."→Some; apology-before-refusal
("I'm sorry, I can't…")→Some; "As an AI…"→Some; "ok"→Some(too short); the banana
cinematic rewrite→None. Confirmed.

### AC2/AC3 — funnel tests
```
test enhancer::tests::the_funnel_refuses_a_sentinel_even_if_the_enhancer_would_not ... ok
test enhancer::tests::the_funnel_passes_a_genuine_rewrite_through_unchanged ... ok
test enhancer::tests::the_funnel_restores_the_original_when_the_model_refuses ... ok
test result: ok. 3 passed; 0 failed
```
`the_funnel_restores_the_original_when_the_model_refuses`: a `Stub` returning a
`Rewritten::succeeded(..., "I can't complete this task.", ...)` yields `out.prompt ==
original`, `!out.changed()`, note contains "declined to rewrite". A good-rewrite stub
still passes through as `Rewritten` with `note == None`. Confirmed against source
(enhancer.rs:536-546: the guard runs only when `out.status.changed()`, returns
`Rewritten::failed` ⇒ `changed()` false ⇒ original submitted).

**Single funnel (AC3):** the ONLY non-test `.rewrite(` call is enhancer.rs:522, inside
`enhance_or_original`. Every other `.rewrite(` is in tests. The production submit path
`src-tauri/src/harness.rs:85` (`finish_rewrite`) calls `enhance_or_original(enhancer, req)`,
and harness.rs:234-243 builds either `LocalEnhancer` (Ollama) or `HostedEnhancer`
(OpenAI/Anthropic) into that same `enhancer` before handing it to `finish_rewrite`.
Both backends therefore route through the guard identically. Confirmed.

### AC5 — regression gate
```
cargo test --workspace : 653 passed; 0 failed; 9 ignored   (core)
                         117 passed; 0 failed; 1 ignored   (tauri-lib)
                         => 770 passed, 0 failed   (matches claim)
cargo fmt --all --check           : exit 0
cargo clippy --workspace --all-targets -- -D warnings : exit 0
./scripts/lint-provenance.py      : exit 0  (CDN PASS, Copy provenance PASS)
cd ui && pnpm test                : exit 0  — Test Files 12 passed, Tests 224 passed
cd ui && pnpm build               : exit 0  — built in 852ms
```
All pass.

## Break attempts

### AC4 — false positives (the critical risk)
Drove the real `hickeyfield_core::enhancer::refusal_reason` from a scratch crate
(path-dep, outside the repo tree) with adversarially crafted legit rewrites that embed
risky words NOT at the start. Every result matched the required verdict:

| Input | flagged | verdict |
|---|---|---|
| "A soldier whispers sorry as the rain falls over the ruined street." | No | correct (mandated AC4 case) |
| "A weary traveler stares… **he cannot** look away as the sun sets…" | No | correct |
| "Frozen in fear, **unable to** move, the child watches the storm…" | No | correct |
| "In a neon-drenched lab, an **ancient AI** hologram flickers…" | No | correct |
| "She **apologizes** silently, mouthing **sorry** to the departing train…" | No | correct |
| "The knight **will not** yield, sword raised against the dragon's roar…" | No | correct |
| "…join in, creating a kaleidoscope…" (long, no risky prefix) | No | correct |

No legitimate mid-sentence occurrence of sorry / cannot / unable to / AI / will not was
flagged. The anchoring (strip leading quote + short "Sure,/Okay,/Well," lead-in, then
prefix-match) holds. No AC4 false positive found.

### Edge probes (as requested)
| Input | result |
|---|---|
| Long refusal: "I can't help you with that, but here is a scene: a banana dances…" | flagged (refusal) — starts with "i can't" |
| Quote then refusal: `"I can't do this."` | flagged (quote stripped) |
| Lead-in + refusal: "Sure, I can't do that." | flagged (lead-in stripped) |
| "Sorry, I cannot." | flagged |
| "As a language model developed by X." | flagged |
| "A stormy sea." (13 chars) | NOT flagged (above floor) |
| "Storm." (<12) | flagged (too short) |
| "Well, a lone wolf howls at the crimson moon…" | NOT flagged (lead-in stripped, legit) |

### Note on start-anchored scenes (acknowledged tradeoff, not a defect)
A legit scene that literally OPENS with a refusal-shaped phrase is downgraded, e.g.
"Unable to sleep, she paces the moonlit corridor…" → flagged, and "I cannot stop
watching the waves…" → flagged. This is the design's explicit, documented tradeoff
("a real cinematic scene never opens with these"; anchor to start). It is safe (submits
the ORIGINAL, never a paid garbage render) and does NOT violate AC4, whose requirement
is specifically about words appearing MID-sentence — all of which pass. Not a refutation.

### LIVE reproduction (bonus)
`llama3.2:1b` confirmed installed via `/api/tags`. Drove the real
`enhance_or_original` + `LocalEnhancer::new("llama3.2:1b", …)` on "Dancing singing
banana" 4×. This run the 1B COMPLIED every time (non-deterministic — it did not emit
"I can't complete this task." today) and produced genuine cinematic rewrites; all four
correctly returned `status=Rewritten, changed=true, note=None` — i.e. the guard did NOT
false-trip on real (if verbose) rewrites. It did not reproduce the refusal this run, as
the task anticipated. The deterministic funnel unit test (feeding the exact refusal
string through the funnel) is the binding proof and passes.

## Per-AC verdict

| AC | Result |
|---|---|
| AC1 pure refusal_reason flags non-rewrites | PASS |
| AC2 funnel returns ORIGINAL + note on refusal | PASS |
| AC3 model-agnostic single funnel | PASS |
| AC4 no false positives on legit rewrites | PASS (7 adversarial cases + mandated case) |
| AC5 regression (770 tests, fmt, clippy, provenance, ui 224 + build) | PASS |

Diff confined to enhancer.rs + .forge. Repo tree left as found (scratch probe lived
outside the repo). No refutation found.

VERDICT: PASS
