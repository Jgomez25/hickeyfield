# S17-keyframe-anchored-mode — DOCUMENT

## What shipped
- **Keyframe mode** for a storyboard: N+1 stills derived deterministically from the world block and
  the shots' hold clauses (still model default `qwen_image_3`, the only priced fal text-to-image route;
  canvas passed as `image_size {width,height}` through the schema-gated override), submitted first
  through the gated path and priced into the total shown before Generate-all; a failed still parks
  the set. Shot 0 waits for all stills (exactly-once barrier, relaunch-safe); shot i is anchored on
  BOTH ends — `image_url` = actual last frame of clip i−1, `end_image_url` = still i+1 — on models that
  take an end frame; `NO_END_FRAME` models fall back to start-only with a join advisory.
- **Tier-2 scoring**: keyframe joins are judged against the ideal thresholds (0.90 / 6 / 3) and an
  end-frame adherence score (SSIM between the planned still and the clip's actual last frame, read from
  the real last frame via the sidecar) is recorded per join; regenerate-still and regenerate-shot go
  through the money gate with separate bounded budgets and the in-flight guard.
- **UI**: Keyframe toggle in Storyboard; still cards (ready / pending / parked, Regenerate); priced
  total including stills; both scores per join on the Library set item. Migration 11 (`keyframes_json`).
- **Money-safety hardening carried in this slice** (from the 2026-09-16 incident): `vault::get` never
  consults the keychain under `cfg(test)` or `HICKEYFIELD_NO_KEYCHAIN=1`; the env-stripping test helper
  is deleted; every keyframe test pins an unavailable route and fails before HTTP; the TEST verifier ran
  the whole workspace with every external host routed to an unroutable proxy: zero provider reach.

## Docs changed
- `CHANGELOG.md` — `[Unreleased] › Added`: keyframe mode; `› Fixed`: the keychain guard.
- `.forge/STATUS.md` — incident record + rules. `.forge/backlog.md` — next: S18 live-spend gate
  (examples/binaries refuse to spend without `HICKEYFIELD_ALLOW_SPEND=1`; first_light reads raw
  `FAL_KEY` today), S19 shot trim handles; minors from REVIEW; threshold calibration human-run.

## Evidence
- TEST PASS (proxy-proven), SECURITY PASS, REVIEW FAIL→ repair round 1 (join-record casing so the
  end-adherence badge renders on real data; `end_frame_capable` on the resolved route; priced-still
  validator check; ideal-threshold gating) → see appended verdicts. Gate at release recorded in
  release.md. Nothing spent after the incident; no key in any agent shell.
