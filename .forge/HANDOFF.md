# HANDOFF — hickeyfield FORGE — PAUSED by owner after S18 (2026-09-16, evening)

## State at pause
main = S1–S18 released and installed to /Applications (S8 catalogue sync · S9 schema const/int-enums ·
S10 pricing audit + actual cost · S11 redesign mockup · S12 redesign code (+picker hotfix) · S13 storyboard
plan · S14 MiniMax H3 Max · S15 storyboard render/chaining · S16 duration ranges + measured continuity ·
S17 keyframe-anchored mode + keychain hotfix · S18 live-spend gate). main tip after S18 evidence: 872669c.
Tests 961 Rust (+9 real-sidecar ignored) / 312 UI. Nothing pushed to any remote.

## Money (read first)
- 2026-09-16 15:39: $0.36 spent by a unit test (9 qwen stills) via the keychain fallback — see STATUS
  "INCIDENT". Closed structurally: S17 `vault::keychain_blocked()` (no keychain under cfg(test) /
  `HICKEYFIELD_NO_KEYCHAIN=1`) + S18 spend gate (`HICKEYFIELD_ALLOW_SPEND=1` required per process; the
  packaged app enables it at startup) + `scripts/lint-spend.py`. TEST proved behind an unroutable proxy:
  no flag → no socket; flag → network. Residual spenders: the installed app, and a human running
  first_light with the flag. NOTHING ELSE CAN SPEND.
- 2026-09-16 17:35: the OWNER ran a Seedance 2.5 job in the app ("Motum — ENGINEER"): est. $1.95 →
  actual $6.18. Not an incident; a 3.2× under-quote in our per-token estimator → backlog S20.
- Every agent command: `export HICKEYFIELD_NO_KEYCHAIN=1 hickeyfield_FAL_KEY=x …` (dummy keys SET,
  never stripped); never set HICKEYFIELD_ALLOW_SPEND; never run first_light.

## Resume order (owner's priorities: continuity is "the money in the bank problem")
1. Owner-only: launch the installed app, confirm a normal estimate + Generate works (S18 proof).
2. S20 per-token pricing accuracy (draft in scratchpad lost on restart — re-derive from backlog note).
3. S19 shot trim handles (per-shot in/out before stitching; ±10% target tolerance).
4. Human-run: calibrate continuity thresholds (0.80/12/6 floor, 0.90/6/3 ideal) on one paid 2×15 set.
5. S18 follow-ups (gate in a non-overridable trait default; lint scans shell/CI; budget cap).
Rules: Opus 4.8 = OMIT the Agent `model` param; max two repair rounds then escalate; FORGE loop with
gate.py exit 0; never hand-edit a verdict; keep agent count low; JARVIS persona, terse.

---
# (previous handoffs kept below)
# HANDOFF — hickeyfield FORGE session → next session (updated 2026-09-16, evening)

## INCIDENT + RULE (2026-09-16): a test that stripped the dummy key env fell through to the macOS
keychain and spent $0.36 on fal (9 qwen stills). Fixed: vault blocks the keychain under cfg(test) /
HICKEYFIELD_NO_KEYCHAIN=1 (45f2416). Every agent command must export the dummy `hickeyfield_*_KEY=x`
vars AND `HICKEYFIELD_NO_KEYCHAIN=1`. S17 paused (partial WIP on forge/s17-keyframe; UI mid-edit).

## State now
main = S1–S12 released and installed (S8 catalogue sync, S9 schema const/int-enums, S10 pricing audit +
actual cost, S11 redesign mockup https://claude.ai/artifact/2des9awBapmx8rSZN8wvAe, S12 redesign code).
Tests 820 Rust / 272 UI. Nothing pushed. Current: S13-storyboard-plan on `forge/s13-storyboard-plan`
(contract written; DESIGN next), then S14-storyboard-render.
Rules learned: Opus 4.8 = OMIT the Agent `model` param (frontmatter pins claude-opus-4-8; the family
alias overrides it); max two repair rounds per slice, then escalate to the owner.

---
# (previous handoff kept below)

## Paste this as the first message of the next session
```
Resume the hickeyfield FORGE build. Repo: /Users/jorge/Documents/GitHub/hickeyfield (my fork
Jgomez25/hickeyfield; never push to upstream). Source of truth is on disk — read, in order:
.forge/HANDOFF.md, .forge/STATUS.md (esp. its "HANDOFF" section), .forge/backlog.md, and
.forge/slices/S8-fal-catalogue-sync/*.md. Also read these memory files explicitly by path (they
may not auto-load because the project dir changed):
~/.claude/projects/-Users-jorge-Documents-GitHub/memory/{hickeyfield-dev-setup,fal-cost-apis,prefer-opus-agents}.md

Rules: FORGE loop (DESIGN→IMPLEMENT→TEST→REVIEW→SECURITY→DOCUMENT→RELEASE, gate.py must exit 0,
never hand-edit a verdict). Spawn agents with model "opus" by default, "fable" only when
necessary; keep agent count low. Land each slice: commit on its forge/* branch → ff-merge to main
→ `tauri build --bundles app` → reinstall to /Applications (codesign --force --deep -s -, cp, open).
Never put my fal key in chat; audit_fal/fal_diff are keyless; never run first_light.rs or
anything that spends money without asking. Speak as JARVIS (calm, precise, "sir"), terse.

First task: finish S8-fal-catalogue-sync on branch forge/s8-fal-catalogue-sync — it is WIP:
(1) fix the 1 failing test registry::tests::registry_covers_the_catalogue_minus_its_exclusions
(registry.rs ~:2230, a count still expecting the 3 removed models); (2) re-run the mechanical
gate incl. keyless audit_fal; (3) fresh REVIEW + TEST verifiers over the reduced 11-model set;
(4) gate.py → release.md → merge → build → install. Then, in this order: the new slice
"fal-schema-const-and-int-enums" (backlog), S9 pricing accuracy + actual-cost tracker,
S10 redesign (mockup first via /design, Higgsfield-like layout, ORIGINAL assets — provenance
lint bans their copy/media), then the Storyboard 30s-ad flow.
```

## Environment setup the next session must redo (ephemeral parts)
```bash
source "$HOME/.cargo/env"                      # rustc/cargo 1.98 (persistent)
# Node 24 for the UI (system node is 20.11 — too old for Vite 7); scratchpad copies vanish:
S=/private/tmp/claude-501/<session-scratchpad>; mkdir -p "$S"; cd "$S"
curl -fsSL https://nodejs.org/dist/v24.8.0/node-v24.8.0-darwin-arm64.tar.gz | tar xz
export PATH="$S/node-v24.8.0-darwin-arm64/bin:$HOME/.local/bin:$PATH"   # + pnpm shim
# Avoid the macOS Keychain GUI hang in commands::tests (dummy values, never real keys):
export hickeyfield_FAL_KEY=x hickeyfield_HIGGSFIELD_KEY=x hickeyfield_GOOGLE_KEY=x \
  hickeyfield_OPENAI_KEY=x hickeyfield_XAI_KEY=x hickeyfield_BFL_KEY=x \
  hickeyfield_RECRAFT_KEY=x hickeyfield_VAIG_KEY=x
# ffmpeg sidecar (if missing): ./scripts/fetch-ffmpeg.sh
```
Gate commands: `cargo test --workspace` · `cargo fmt --all --check` ·
`cargo clippy --workspace --all-targets -- -D warnings` · `./scripts/lint-provenance.py` ·
`env -u hickeyfield_FAL_KEY -u FAL_KEY cargo run -p hickeyfield-core --example audit_fal` ·
`cd ui && pnpm test && pnpm build` · `python3 .forge/gate.py`.

## Document index
- `.forge/STATUS.md` — resume anchor: released slices, S8 state + REVIEW blockers + decided
  repair, grounded notes for S9 (pricing), S10 (redesign seams), Storyboard (compositor facts).
- `.forge/backlog.md` — all slices + follow-ups (F1b, F4, F6–F8, dep-bump, parser slice, minors).
- `.forge/objective.md` — the original contract; `.forge/gate.py`, `new-slice.sh` — the toolkit.
- `.forge/slices/S1…S8/` — full nine-phase evidence per slice (S8 has REVIEW FAIL + addendum).
- `CHANGELOG.md`, `docs/FIRST-LIGHT.md` (corrected), `docs/PARITY.md`.
- Memory: `hickeyfield-dev-setup` (env), `fal-cost-apis` (price/usage APIs, no per-request cost),
  `prefer-opus-agents` (agent model preference).

## Git state at handoff
- `main` = S1–S7 released, installed to /Applications. Pushed to origin.
- `forge/s8-fal-catalogue-sync` = S8 WIP (11 verified models; 1 failing count test). Pushed.
- Other `forge/s*` branches are already fast-forwarded into main (kept for evidence).
