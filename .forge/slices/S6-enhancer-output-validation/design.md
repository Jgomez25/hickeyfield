# S6-enhancer-output-validation — DESIGN

Authored directly (investigation complete; the hook is a single well-understood funnel).

## The one hook: `enhance_or_original` (crates/hickeyfield-core/src/enhancer.rs:516)
This is documented as "the function the submit path should call — never `Enhancer::rewrite` directly ... the single place that enforces the module's first commitment." It already restores the original when a changed reply is empty (:524-531). S6 adds a sibling check for a reply that is present but is NOT a rewrite.

### New pure function (near `precheck`, ~:542)
```rust
/// A model reply that is a refusal/apology/meta-comment or a non-rewrite, rather
/// than a scene description. Returns a short human reason when the reply must NOT
/// be submitted. Model-agnostic; anchored to the reply's shape to avoid tripping
/// on a legitimate rewrite that merely contains one of these words mid-sentence.
pub fn refusal_reason(reply: &str) -> Option<&'static str> { ... }
```
Rules (operate on `let t = reply.trim(); let lower = t.to_ascii_lowercase();`):
- **Refusal/meta prefixes** — if `lower` STARTS WITH (or starts within the first ~40 chars, to tolerate a leading quote/"Sure, ") any of: `i can't`, `i cant`, `i cannot`, `i can not`, `i'm unable`, `i am unable`, `i'm sorry`, `i am sorry`, `sorry,`, `sorry `, `i apologize`, `as an ai`, `as a language model`, `i won't`, `i will not`, `i'm not able`, `i am not able`, `unable to`, `i cannot fulfill`, `i can't help`, `i cannot assist`, `cannot assist`, `i'm just`, `i cannot create`, `i can't create`, `i cannot generate`, `i can't generate` → `Some("the model replied with a refusal")`. Anchor to the START (a real cinematic scene never opens with these), NOT a substring anywhere, to satisfy AC4.
- **Too short** — trimmed length `< 12` chars (a real scene rewrite is longer) → `Some("the model's reply was too short to be a prompt")`.
- Return `None` otherwise (plausible rewrite).

Note: the empty case is already handled at :524; keep it. Echo (reply == original) is NOT flagged as an error — an unchanged reply is benign (submitting the original) — so echo handling stays as-is; AC1's "echo" case is covered by the existing `changed()`/`prompt != original` logic, so S6 need not special-case it unless trivial. (If included, only flag echo when it would otherwise be marked Rewritten — but do not regress the legitimate "model returned the same good text" case. Keep S6 focused on refusal + too-short.)

### Apply in the funnel (:522-533)
After the empty-reply guard, before returning `out`:
```rust
if out.status.changed() {
    if let Some(why) = refusal_reason(&out.prompt) {
        return Rewritten::failed(
            &req.prompt,
            format!("The model declined to rewrite this prompt ({why}), so your original was used."),
            &version,
        );
    }
}
```
Use `Rewritten::failed` (status Failed → `changed()` false → the ORIGINAL is submitted; note surfaces via `enhance_note`/UI). This mirrors the existing empty-reply fallback exactly.

## Model-agnostic proof (AC3)
Both `LocalEnhancer` and `HostedEnhancer` are invoked only via `enhance_or_original` from the harness (`src-tauri/src/harness.rs` `finish_rewrite`). The guard in the funnel therefore covers both. A test asserts `enhance_or_original` with a fake Enhancer returning a refusal yields the original — and (light) confirms harness `finish_rewrite` calls the funnel, not `rewrite` directly.

## Tests (AC1–AC5)
- `refusal_reason` unit tests: "I can't complete this task." → Some; "I'm sorry, I can't do that." → Some; "As an AI, I…" → Some; a real rewrite sample (>=1 sentence cinematic) → None; "ok" (too short) → Some; a rewrite that contains "sorry" mid-sentence ("A soldier says sorry as the rain falls…") → None (anchor-to-start proves no false positive).
- `enhance_or_original` test: fake Enhancer → refusal reply → returns original prompt, status not-changed, note set.
- Regression gate per AC5.

## Files
- `crates/hickeyfield-core/src/enhancer.rs` — add `refusal_reason` + the funnel check + tests. No UI change required (the note already flows to `enhance_note`). No command/DTO/type change. No dependency added.

## Riskiest part
False positives (AC4): flagging a legitimate rewrite as a refusal would silently downgrade good enhancement to raw. Mitigation: anchor refusal patterns to the reply START (cinematic scenes never open with "I can't"/"I'm sorry"/"As an AI"), keep the too-short floor low (12), and test a mid-sentence-"sorry" sample stays None.
