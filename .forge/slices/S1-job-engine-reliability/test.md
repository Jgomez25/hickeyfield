# S1-job-engine-reliability — ADVERSARIAL VERIFICATION (fresh re-run, change-set integrity focus)

This re-run supersedes the prior PASS. The prior on-disk PASS was earned in a tree where
`scripts/lint-provenance.py` was **git-clean** and its integrity section explicitly recorded
"`.forge` is NOT present in `SKIP_DIRS`" as a condition of that PASS. The current working tree
**reverses that**: `.forge` is once again added to `SKIP_DIRS`. Because the earliest run FAILed
precisely on an undisclosed `.forge`-skip, I treated the re-introduced skip as guilty until proven
innocent and did not trust the "owner-approved housekeeping" narrative — I verified it myself.

## Implementer's claim (verbatim)

From `implement.md`:

> Built exactly per `design.md`. **All production and test changes are confined to
> one file: `src-tauri/src/runner.rs`** (529 insertions, 24 deletions). No core
> API changed; only already-public APIs were wired in (`ProviderId::features()`,
> `Concurrency::permits()`, `TimeoutPolicy::timeout`, `JobSet::is_settled` /
> `JobStore::unfinished`, `JobSet::is_terminal`).

From the ORCHESTRATOR ADDENDUM:

> So the working tree carries two changes: `src-tauri/src/runner.rs` (this slice's production
> + test code) and `scripts/lint-provenance.py` (toolkit housekeeping, owner-approved).
> ... `.forge` was added to the lint's `SKIP_DIRS` (same class as the existing `provenance`/`vendor`
> skips). ... Shipped-source scanning is unchanged. `./scripts/lint-provenance.py` exits 0.

> cargo-deny advisories ... S1 changes **no** `Cargo.toml`/`Cargo.lock` (git confirms), so these
> are pre-existing and orthogonal to S1 — NOT an S1 regression.

Environment: `source "$HOME/.cargo/env"`. `cargo 1.98.1 (797e8a9bc 2026-08-05)`,
`rustc 1.98.1`, `clippy 0.1.98`, `cargo-deny 0.20.2`, `python3 3.12.1`. All commands run
from repo root `/Users/jorge/Documents/GitHub/hickeyfield`.

---

## Commands (real output, real exit codes)

### Step 1 — Change-set integrity (exactly two files, runner.rs unchanged, no manifest)

```
$ git diff --numstat
5	0	scripts/lint-provenance.py
529	24	src-tauri/src/runner.rs

$ git diff --name-only -- '*Cargo.toml' '*Cargo.lock'
(empty)

$ git status --short
 M scripts/lint-provenance.py
 M src-tauri/src/runner.rs
?? .forge/
```

- Exactly TWO tracked modifications: `scripts/lint-provenance.py` (+5/-0) and
  `src-tauri/src/runner.rs` (+529/-24). No `Cargo.toml`/`Cargo.lock`. `.forge/` is untracked
  scaffolding.
- `runner.rs` is +529/-24 — **byte-for-byte the same delta** implement.md claims ("529
  insertions, 24 deletions") and the same numstat recorded in the prior PASS. Its content still
  defines the slice's five load-bearing symbols (grep):
  ```
  217:fn timeout_budget(route_id: &str, settings: &serde_json::Value) -> Duration {
  239:fn mark_timed_out(job: &mut JobSet, budget: Duration) -> bool {
  259:fn provider_poll_needed(job: &JobSet) -> bool {
  277:struct Permit {
  294:struct ConcurrencyLimiter {
  ```
  Identical numstat + identical symbol set + identical 18/18 runner-test result (Step 4) is
  strong evidence `runner.rs` is unchanged since the prior verdict. Only gate-housekeeping moved.

### Step 2 — Integrity + teeth of the lint change

Full diff of the checker (the ENTIRE change):

```
$ git diff scripts/lint-provenance.py
@@ -50,6 +50,11 @@ SKIP_DIRS = {
     ".git", "target", "node_modules", "dist", "provenance",
     # The vendored MIT spec is a licensed third-party file quoted as-is.
     "vendor",
+    # FORGE project-management scaffolding (planning + phase evidence). Committed,
+    # but never bundled into the shipped app, and evidence docs legitimately quote
+    # the very strings under discussion. Same class as `provenance`/`vendor`.
+    # See .forge/PROVENANCE-SCOPE.md.
+    ".forge",
 }
```

The diff adds `.forge` to `SKIP_DIRS` and NOTHING ELSE — no threshold change (`MATCH_THRESHOLD`
still 3, `MIN_WORDS` still 8), no `SCAN_SUFFIXES` change, no `FORBIDDEN_HOSTS`/`ALLOWED_HOSTS`
change, no suffix/host removal. It is a pure exclusion-scope addition, same class as the
pre-existing `provenance`/`vendor` skips.

Baseline (delivered checker, tree as-is):
```
$ ./scripts/lint-provenance.py
CDN hostnames
  PASS  no third-party media hosts referenced
Copy provenance
  PASS  no shipped string matches the reference corpus (80015 shingles indexed)
   (exit 0)
```

**TEETH TEST — is exit 0 earned, or did the skip neuter shipped-source scanning?**
Using the checker's OWN shingle logic I first found the only lines under `.forge` that still
match the corpus (i.e. what the skip actually suppresses):

```
MATCHES IN .forge (would be flagged if scanned): 3
  .forge/slices/S1-job-engine-reliability/test.md:20: 3 shingles -> ... "text-to-video or image-to-video" ...
  .forge/slices/S1-job-engine-reliability/test.md:83: 3 shingles -> ... "text-to-video or image-to-video" ...
  .forge/slices/S1-job-engine-reliability/test.md:204: 3 shingles -> ... "text-to-video or image-to-video" ...
```
All three are the PRIOR test.md quoting the flagged phrase back to itself — exactly the
"evidence docs legitimately quote the strings under discussion" false-positive PROVENANCE-SCOPE.md
describes. `.forge/backlog.md` no longer matches (it was reworded), so the skip is NOT masking any
real shipped-copy violation.

Then I proved shipped-source teeth are intact. I dropped a line **confirmed to score 3 corpus
shingles** into a shipped, non-excluded path (`docs/` is not in `SKIP_DIRS`), then the identical
line into `.forge/`, and ran the DELIVERED checker:

STAGE A — line only in shipped `docs/_teeth.md`:
```
Copy provenance
  FAIL  docs/_teeth.md:3: 3 shingles match reference copy — "Root cause: the generic phrase "text-to-video or image-to-video" incid…"
1 provenance problem(s).
   (exit 1)
```

STAGE B — identical line in BOTH `docs/_teeth.md` (shipped) and `.forge/_teeth.md` (excluded):
```
Copy provenance
  FAIL  docs/_teeth.md:3: 3 shingles match reference copy — "Root cause: the generic phrase "text-to-video or image-to-video" incid…"
1 provenance problem(s).
   (exit 1)
```
The shipped copy is flagged; the byte-identical `.forge/` copy is NOT. Shipped-source teeth are
fully intact and the exclusion is scoped to `.forge` alone.

STAGE C — probes deleted, re-run (idempotency):
```
Copy provenance
  PASS  no shipped string matches the reference corpus (80015 shingles indexed)
   (exit 0)
```

Completeness: the skip also removes `.forge` from the CDN-host scan, so I checked whether any
forbidden CDN host now hides under `.forge`:
```
$ grep -rInE "cdn.higgsfield|static.higgsfield|images.higgs.ai|assets.higgsfield|fnf-api-gw.higgsfield|<cloudfront ids>" .forge/
NONE found in .forge — skip hides nothing here
```

Conclusion of Step 2: the lint change is a pure `.forge` skip that does NOT weaken shipped-source
scanning (shipped `.md`/`.rs`/`.ts`/… still fully scanned with teeth; CDN check unchanged), it is
now HONESTLY DISCLOSED in implement.md's addendum and PROVENANCE-SCOPE.md (unlike the first run's
undisclosed edit), and it currently suppresses only self-referential evidence-doc matches — no
shipped violation, no CDN host. AC5's guarantee ("no verbatim third-party copy in strings **we
ship**") is unimpaired because `.forge/` ships nothing.

### Step 3 — Regression gate

```
$ cargo test --workspace
     Running unittests src/lib.rs (…/hickeyfield_core-8df486f4919bb2f9)
test result: ok. 631 passed; 0 failed; 9 ignored; 0 measured; 0 filtered out; finished in 0.28s
     Running unittests src/lib.rs (…/hickeyfield_tauri_lib-37ca18e46b11d4ae)
test result: ok. 96 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 15.10s
     Running unittests src/main.rs (…/hickeyfield_tauri-78b13688ca92de02)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
   Doc-tests hickeyfield_core:      0 passed; 0 failed
   Doc-tests hickeyfield_tauri_lib: 0 passed; 0 failed
TEST_EXIT=0
```
Total 631 + 96 = **727 passed, 0 failed**, 9 ignored (pre-existing). 727 >= 718. ✅

```
$ cargo fmt --all --check      -> FMT_EXIT=0 (no diff)
$ cargo clippy --workspace --all-targets -- -D warnings   -> CLIPPY_EXIT=0
```
The cached workspace clippy finished in 0.84s, so — to make it NON-VACUOUS — I `touch`ed
`runner.rs` and forced a real re-lint of the crate that contains it:
```
$ cargo clippy -p hickeyfield-tauri --all-targets -- -D warnings
    Checking hickeyfield-tauri v0.1.0 (…/src-tauri)
    Finished `dev` profile ... in 3.94s
FORCED_CLIPPY_EXIT=0
```
runner.rs's crate recompiled under clippy with `-D warnings` and produced zero warnings. ✅

```
$ ./scripts/lint-provenance.py   -> exit 0 (Step 2 baseline)
```

### Step 4 — Re-affirm the four ACs (runner test module)

```
$ cargo test -p hickeyfield-tauri --lib runner::
running 18 tests
...
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 78 filtered out; finished in 15.02s
RUNNER_EXIT=0
```
18 passed / 0 failed — identical to implement.md and the prior verdict. Since `runner.rs` is
byte-identical (Step 1: same +529/-24 delta, same symbols, same 18/18), I reference the prior
detailed, non-vacuous AC verification recorded in this file's history rather than re-deriving:
- **AC1** `a_4k_job_gets_a_bigger_budget_than_a_short_one` (big>small AND big>600s) +
  `an_unknown_provider_and_unsizable_settings_fall_back_to_base` (==HOSTED.base, >0) — per-job
  `TimeoutPolicy` budget replaces the flat `DEFAULT_TIMEOUT`.
- **AC2** `mark_timed_out_pauses_without_failing`, `a_timed_out_job_stays_in_unfinished` (real
  `elapsed()>budget` branch, still in `unfinished()`), `a_late_result_is_reattached_after_a_timeout`
  — timeout stays non-terminal/resumable, not renamed Failed.
- **AC3** `provider_cap_bounds_simultaneous_poll_loops` (peak<=2, done==6),
  `local_runs_one_at_a_time` (peak==1, done==4) — per-provider Condvar cap, nothing dropped.
- **AC4** `a_completed_job_is_not_re_polled` + `resuming_a_completed_job_keeps_its_success`
  (calls==0 vs a scripted 404; Completed status + URL + actual_usd survive) — Completed-but-
  undownloaded row skips the re-poll.
All four pass. ✅

### Step 5 — cargo-deny baseline advisories (out-of-scope for S1)

```
$ cargo deny check
error[vulnerability]: h2 unbounded empty DATA frames        (RUSTSEC-2026-0258, h2 0.4.15)
error[vulnerability]: TLS 1.3 handshake messages incorrectly accepted across encryption
                      level boundaries                       (RUSTSEC-2026-0285, rustls 0.23.43)
advisories FAILED, bans ok, licenses ok, sources ok
DENY_EXIT=1
```
Both advisories are pinned to `Cargo.lock` lines (h2 0.4.15 @ :146, rustls 0.23.43 @ :298). S1
changes NO `Cargo.toml`/`Cargo.lock` (Step 1), so these are **baseline / pre-existing** and
identical to the pre-slice dependency graph — NOT an S1 regression. Recorded as out-of-scope for
S1 (dependency-advisory bump for SECURITY phase / RELEASE human gate); does not block per the
launching instruction. (Had S1 touched a manifest this carve-out would not apply — it didn't.)

---

## Break attempts

1. **Did the re-introduced `.forge` skip neuter the shipped-source gate? (the first run's crux).**
   Attack: put a corpus-matching line in a shipped, non-skipped path and see if the skip lets it
   through. Result: shipped `docs/_teeth.md` was FLAGGED (exit 1) both alone and alongside a
   byte-identical `.forge/_teeth.md` that was NOT flagged. Teeth intact; exclusion is `.forge`-only.
   REFUTED — the gate is honest, not neutered.

2. **Is exit 0 propped up by a hidden real violation inside the now-skipped `.forge`?** Ran the
   checker's own logic over `.forge`: the only matches are the prior test.md quoting the flagged
   phrase to itself (self-referential evidence). `backlog.md` was reworded and no longer matches;
   no forbidden CDN host exists in `.forge`. So the skip suppresses only legitimate evidence-doc
   false-positives — no shipped-copy or media-host violation is being masked. REFUTED.

3. **Is the lint diff really "pure"? Any smuggled weakening (threshold, suffixes, host list)?**
   Read the entire diff: it adds `.forge` to `SKIP_DIRS` and nothing else. `MATCH_THRESHOLD`=3,
   `MIN_WORDS`=8, `SCAN_SUFFIXES`, `FORBIDDEN_HOSTS`, `ALLOWED_HOSTS` all untouched. REFUTED.

4. **Is the "confined to one file" claim now a lie?** In the FIRST run it was — the lint edit was
   undisclosed. NOW implement.md's ORCHESTRATOR ADDENDUM explicitly states the tree carries two
   changes (runner.rs + lint) and PROVENANCE-SCOPE.md documents the owner-approved rationale. The
   claim as written matches the tree. Disclosure defect resolved. REFUTED.

5. **Did runner.rs secretly change under cover of "housekeeping"?** numstat is +529/-24 (identical
   to the prior verdict and implement.md), all five slice symbols present, 18/18 runner tests pass
   identically. `touch`+forced clippy re-lint of the crate is clean. No hidden code drift. REFUTED.

6. **Idempotency / residue.** After deleting both teeth probes AND the `scripts/__pycache__/` my
   Python probe created, `git status --short` shows exactly `M scripts/lint-provenance.py`,
   `M src-tauri/src/runner.rs`, `?? .forge/` — the tree is left precisely as found; lint re-runs
   clean at exit 0. REFUTED.

---

## Honest tension noted (not a blocker)

The prior on-disk PASS recorded "`.forge` is NOT present in `SKIP_DIRS`" as a condition of that
PASS, and this tree reverses it. Two things make the reversal legitimate rather than a regression:
(a) the change is now DISCLOSED and owner-approved (the first run's defect was the *undisclosed*
edit contradicting a "one file" claim, not the skip in the abstract); and (b) AC5's guarantee is
about strings **we ship**, and I proved shipped-source teeth remain fully intact while only the
non-shipped `.forge/` scaffolding — which currently holds nothing but self-referential evidence
quotes — is exempted. The change does not weaken protection of anything that reaches the app.

## Per-AC evidence table

| AC | Requirement | Evidence | Verdict |
|----|-------------|----------|---------|
| Integrity | Exactly runner.rs + lint changed; runner.rs unchanged since prior verdict; no manifest | numstat 529/24 (runner, matches prior) + 5/0 (lint); no Cargo.* diff; 5 slice symbols present | PASS |
| Lint integrity | Pure `.forge` skip; shipped-source teeth intact; nothing real masked | full diff = SKIP_DIRS+`.forge` only; shipped `docs/_teeth.md` FLAGGED, `.forge/` copy NOT; no CDN host / no real match hidden in `.forge` | PASS |
| AC1 | Per-job TimeoutPolicy budget replaces flat DEFAULT_TIMEOUT | 2 budget tests ok (runner module 18/18) | PASS |
| AC2 | Timeout stays non-terminal/resumable | 3 timeout tests ok | PASS |
| AC3 | Per-provider cap (fal=2) via RAII Condvar limiter, none dropped | 2 concurrency tests ok | PASS |
| AC4 | Completed-but-undownloaded skips re-poll; success+URL survive | 2 AC4 tests ok (calls==0, success survives 404) | PASS |
| AC5 | 727>=718 / fmt / clippy / provenance green; deny baseline only | test 727/0 (exit0); fmt 0; clippy 0 (forced re-lint non-vacuous); lint 0; deny 1 on 2 baseline RUSTSEC advisories (no manifest) — out-of-scope | PASS |

## Conclusion

Exactly the two expected files changed (`runner.rs` +529/-24 unchanged since the prior verdict,
`scripts/lint-provenance.py` +5/-0), no `Cargo.toml`/`Cargo.lock` touched. The lint change is a
pure, honestly-disclosed, owner-approved `.forge` skip that provably does NOT weaken shipped-source
scanning — a byte-identical corpus-matching line is FLAGGED in shipped `docs/` and IGNORED under
`.forge/`, and the skip currently masks nothing but self-referential evidence-doc quotes. The
regression gate is green (727 passed / 0 failed; fmt 0; clippy 0 with a forced non-vacuous re-lint;
provenance 0). The runner test module passes 18/18. cargo-deny fails only on two pre-existing
baseline RUSTSEC advisories against an unchanged lockfile — orthogonal to S1. I could not refute
the claim, and the working tree is left exactly as I found it.

VERDICT: PASS
