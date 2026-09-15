# S1-job-engine-reliability — DOCUMENT

Upstream read for this phase: `.forge/objective.md` (§B reliability criteria),
`.forge/slices/S1-job-engine-reliability/slice.md` (AC1–AC5), `design.md`,
`implement.md`, `test.md` (VERDICT: PASS), `.forge/PROVENANCE-SCOPE.md`,
`docs/PARITY.md`, `docs/FIRST-LIGHT.md`, `README.md`, and
`scripts/lint-provenance.py`. This phase adds documentation only; it changes no
production code. All prose below is original wording.

## What S1 shipped (recap, for the reader of this file)

All production + test code landed in one file, `src-tauri/src/runner.rs` (per
`implement.md` / `test.md`, +529/-24, VERDICT: PASS): per-job `TimeoutPolicy`
budgets replaced the flat 600 s `DEFAULT_TIMEOUT`; a timed-out job stays
non-terminal/resumable; provider polls are capped per provider
(`Concurrency::Provider(2)` for fal); and a Completed-but-undownloaded job is not
re-polled (and so not flipped to Failed) on relaunch. A separate, owner-approved
FORGE-toolkit change added `.forge` to `scripts/lint-provenance.py` `SKIP_DIRS`
(see `.forge/PROVENANCE-SCOPE.md`). Live end-to-end proof against a real fal key
is **not** part of S1 — it is slice #4 (`.forge/backlog.md`), still pending.

## Files changed in the DOCUMENT phase

| File | Kind | Change |
|---|---|---|
| `CHANGELOG.md` (repo root) | **created** | No changelog existed at the repo root (only vendored ones under `ui/node_modules/`). Created a "Keep a Changelog"-style file with an `[Unreleased] › Fixed` section describing the four reliability fixes in user-facing terms. |
| `docs/PARITY.md` | edited | In §3.3 ("M1's exit criteria have never been run"): annotated the *reattach-on-relaunch* and *429-backoff* bullets to mark the code-level fixes as landed in S1, and added a dated S1 update note. The note is explicit that only the code fixes landed and that the live e2e exercise is unchanged and owned by slice #4 — no overstatement. |
| `.forge/slices/S1-job-engine-reliability/document.md` | replaced stub | This evidence file. |
| `README.md` | **not touched** | See "README decision" below. |

## Changelog entry added (quoted verbatim)

The `[Unreleased] › Fixed` section of the new `CHANGELOG.md`:

```markdown
## [Unreleased]

### Fixed

- **Long and high-resolution jobs are no longer given up on while they are still
  running.** The job runner now decides how long to keep waiting from the
  provider's own timeout policy, scaled to the work you asked for, instead of one
  flat ten-minute limit for everything. A 4K or long clip that the provider is
  still producing — and still charging you for — keeps being tracked rather than
  being declared failed underneath it.
- **A generation that runs past its waiting window is no longer lost.** When a
  polling session runs out of time it is paused, not failed, so the app picks the
  job back up the next time it launches and a result that lands late is still
  downloaded. You are no longer billed for output the app then throws away.
- **Queued jobs stop rate-limiting each other.** The runner now holds open only
  as many simultaneous requests to a provider as that provider permits (two, for
  fal). Additional jobs wait for a free slot instead of all starting at once and
  triggering each other's rate limits, and no queued job is dropped.
- **A completed result that has not been saved yet survives a relaunch.** On
  restart the runner no longer re-queries a job the provider had already
  finished, so an expired status link can no longer turn a paid, completed result
  into a failure. The app just retries saving the file you already paid for.
```

The four bullets map 1:1 to AC1 (per-job timeout budget), AC2 (timed-out job
stays resumable), AC3 (per-provider concurrency cap), AC4 (no relaunch flip to
Failed). The FORGE-toolkit `SKIP_DIRS` change is deliberately **not** in the
changelog: it is internal dev tooling, not a user-facing change, and the task
asked that changelog entries stay user-facing and factual.

## PARITY.md edit (quoted verbatim)

The two bullets in §3.3 now read:

```markdown
- Quit-mid-generation → reattach-on-relaunch: hardened in code by **slice S1** (a timed-out
  job now stays resumable, and a completed-but-undownloaded job is no longer flipped to Failed
  on relaunch — each pinned by a `runner.rs` unit test); still never exercised against a real
  job, which is slice #4.
- 429 backoff: unit-tested, never seen live. **Slice S1** additionally caps concurrent provider
  polls (two for fal) so queued jobs no longer trip each other's rate limits; still not observed
  against the live API.
```

and the dated note added after the "highest-value next action" line:

```markdown
> **Update — slice S1 (job-engine-reliability), 2026-09-14.** The runner-side reliability
> bugs behind these gaps landed as code fixes only: per-job `TimeoutPolicy` budgets replaced
> the flat 600 s timeout, a timed-out job now stays resumable instead of being dropped, provider
> polls are concurrency-capped (two for fal) so queued jobs stop self-inflicting 429s, and a
> completed-but-undownloaded job is no longer re-polled into Failed on relaunch — each pinned by
> a `runner.rs` unit test. The live end-to-end exercise of these paths against a real fal key is
> unchanged and still pending; it is owned by slice #4, not S1.
```

Deliberately **not** overstated: the note says "code fixes only," repeats that the
live exercise is "unchanged and still pending," and names slice #4 as its owner.
PARITY.md's live-proof gaps (§3.3 "No live generation has ever completed",
`FIRST-LIGHT.md` "Still unproven after this run") were left intact — S1 did not
close them, so nothing there was marked resolved.

## README decision — N/A (recorded, not skipped)

**N/A — no README claim is touched by S1.** I checked every claim in `README.md`
that borders the runner:

- "**Jobs run in the Rust core**, so they survive the window closing and reattach
  on relaunch." This is an architectural statement; S1 makes it *more* dependable
  (a timeout no longer drops the row; a completed job is no longer flipped to
  Failed on relaunch) but does not change the sentence's truth at the level it is
  stated. Editing it to add reliability detail would exceed what the README covers
  and risk implying live-proven behaviour that is slice #4's, not S1's.
- "What is *verified end to end* is the fal route on macOS — upload, submit, poll,
  download, play." This claim is about the **live** end-to-end path, which S1 does
  not deliver (slice #4 does). It is therefore out of scope for S1's DOCUMENT
  phase; touching it here would either overstate S1 or pre-empt #4. (Whether that
  existing claim is fully consistent with `FIRST-LIGHT.md`'s "no live generation
  has ever completed" is a pre-existing question, not introduced or resolved by
  S1.)

So the README is intentionally left unchanged.

## Provenance lint

**Constraint:** this DOCUMENT-phase environment exposes no shell/execution tool
(only file read/write/search), so I could **not** run
`./scripts/lint-provenance.py` myself here. What I can attest:

- All prose I added (CHANGELOG.md, the PARITY.md §3.3 annotations + note) is my
  own original wording, describing internal runner mechanics; none of it is
  lifted from any third-party product copy.
- By the checker's own rules (`scripts/lint-provenance.py`): the copy check flags
  a line only when it shares **≥ 3** five-word shingles with the reference corpus
  (`MATCH_THRESHOLD = 3`, `SHINGLE = 5`, `MIN_WORDS = 8`), and the corpus is the
  Higgsfield product/i18n strings. Prose about timeout budgets, poll sessions,
  concurrency caps and relaunch behaviour is not product marketing copy and is
  very unlikely to reach that threshold. The CDN-host check cannot trip either:
  the only URLs I added are `keepachangelog.com` and `semver.org`, neither of
  which is in `FORBIDDEN_HOSTS`.
- `docs/` and the repo-root `CHANGELOG.md` are **scanned** (not in `SKIP_DIRS`),
  so my additions are subject to the full check — as intended. The delivered tree
  was recorded at lint **exit 0** in `test.md` (Step 2/Step 3).

**Verifier action requested:** run `./scripts/lint-provenance.py` from the repo
root and confirm it still exits 0 with `CHANGELOG.md` and the edited
`docs/PARITY.md` in the tree. This is the weakest point of this phase precisely
because I could not execute it here. If any new line trips it, the fix is to
reword that line (never to relax the threshold).

## Weakest parts (verifier, start here)

1. **Un-run provenance lint.** I could not execute `./scripts/lint-provenance.py`
   in this environment (no shell tool). The claim that my new prose passes is
   reasoned, not observed — run it.
2. **PARITY.md wording balance.** Confirm the §3.3 annotations + note read as
   "code fixes landed, live proof still pending (#4)" and do not accidentally
   imply S1 proved anything against a live key.
3. **Changelog scope call.** I kept the FORGE `SKIP_DIRS` change out of the
   user-facing changelog and the README untouched (N/A recorded). Confirm those
   are the right scope boundaries for a user-facing changelog.
