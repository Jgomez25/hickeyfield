# S13-storyboard-plan — DOCUMENT

## What shipped
- **Storyboard view** (third titlebar view): brief + target length → Draft. The configured enhancer
  backend (local Ollama or hosted key) drafts 3–8 shots through a `Chat` seam over the existing wire
  functions; `storyboard::validate` enforces the S6 refusal guard, JSON shape, model ids runnable for
  the tab, camera slugs, and a ±10% duration sum. Durations clamp to each model's S9 axis (advisory
  on change); a single-shot native-30s plan is allowed on Seedance 2.5.
- Shot cards: edit prompt/seconds/camera/model (existing picker), reorder, add/delete; per-shot
  Preview reuses S7; the total sums per-shot estimates and marks unpriced; Generate-all is disabled
  until the total is shown. Partial failure shows each failed shot's error; "Retry failed" resubmits
  only those, with the subset total shown first, into the original set.
- **Generate-all** submits every shot through the unchanged single-job path (`submit_one`, factored
  byte-for-byte out of `submit_job`): estimate + S6 guard + S7 final_prompt per shot. Server-side: full
  plan re-validated, ≤8 shots, per-index model/duration/route pinned, brief ≤4 KiB, plan ≤64 KiB, an
  already-started index refused; all before any store write or submit.
- **Store**: migration 8 adds `storyboards` (the contract's `job_sets` name collides with the jobs
  table) and write-once `jobs.storyboard_id`/`shot_index`; Library groups a set. Job and storyboard
  ids are now collision-proof (`{secs}-{model}-{seq}`) — the old per-second id could silently
  overwrite a same-model job.
- Continuity fields (role, previous-shot reference) recorded in the plan for S15 render; not applied.

## Docs changed
- `CHANGELOG.md` — `[Unreleased] › Added`: storyboard planning, in original wording.
- `.forge/backlog.md` — follow-ups (non-blocking): retried shot leaves the original Failed row in the
  group (cosmetic double tile); within-batch duplicate `shot_indices` not deduplicated (local-only);
  retry does not check the plan against the group's stored plan; live JSON hit-rate measured only on
  llama3.2:3b (3/3) — other local models unmeasured.

## Evidence
- TEST / REVIEW / SECURITY = PASS on HEAD 7250335 after two repair rounds (B1 id collision, M1/M2
  partial-failure + retry, security L1–L4). cargo test --workspace 852 / 0; fmt · clippy · provenance
  clean; pnpm test 285 / build ok; keyless audit_fal unchanged. Free local Ollama draft: 3/3 parsed.
- Nothing spent; no key in the session; the paid submit loop is proven offline up to route resolve.
