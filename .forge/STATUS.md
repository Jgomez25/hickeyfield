# FORGE STATUS — resume here (written 2026-09-16, context nearly full)

Project: hickeyfield — production-hardening + tool surfaces (personal/local build, no push).
Agents: use `model: "opus"` by default; `fable` only when necessary (owner preference).

## Released on `main` (all installed to /Applications)
S1 reliability(+F1/F3/F5) · S2 honest-free-tier · S4 enhancer-wired · S5 model-curation ·
S6 refusal-guard · S7 prompt-preview/edit/retry. Tests 773 Rust / 232 UI at S7.

## IN FLIGHT: S8-fal-catalogue-sync — branch `forge/s8-fal-catalogue-sync` (UNCOMMITTED)
Implemented + green (779 tests, snapshot refreshed 2026-09-16 = 1,491 models, audit_fal
UNREACHABLE=0, 13 new + seedance_2_5 promoted with native 30s). Evidence: slice/design/implement
written; TEST + REVIEW/SECURITY agents were running at cutoff — check
`.forge/slices/S8-fal-catalogue-sync/{test,review,security}.md` for VERDICT lines.
VERDICTS: SECURITY PASS · REVIEW **FAIL** · TEST (pending at cutoff — read test.md).
REVIEW blockers (all real): B1 `ltx_2_5`+`ltx_2_5_animate` cannot generate — fal duration enum
{6,8,10,"auto"}, `fal_schema::parse` (fal_schema.rs:216) keeps only STRING enum members → records
["auto"] → reconcile refuses default 5 → JobError::Permanent every submit. B2 `veo3_1_extend` 422s —
fal pins duration ("7s") + resolution ("720p") by `const`; parser ignores `const`; app sends 5.0 +
1080p. B3 root cause: `audit_fal` check 4 fires only when both sides enumerate, and the parser
reads neither `const` nor integer enums → "CHECKED/0 findings" is hollow for duration/resolution.
DECIDED REPAIR (no agents allowed this session): (1) REMOVE the 3 models from S8 (spec in
picker_only_specs, priced_routes arm, JOB_TYPES row, FAL_ROUTE_MODES entry, fix array-length
literals + count tests) — the slice's own "never land an unrunnable slug" rule; keep the 11
verified. (2) Re-run mechanical gate: cargo test --workspace, fmt, clippy, provenance,
audit_fal (keyless), pnpm test/build. (3) Commit to branch `forge/s8-fal-catalogue-sync` ONLY —
do NOT merge to main until a fresh adversarial TEST re-verifies the reduced set (gate.py needs
TEST=PASS; never hand-edit a verdict). (4) NEW SLICE to log: "fal-schema-const-and-int-enums" —
make fal_schema read `const` and integer enum members (stringify), make audit_fal check 4 use
them, then re-land ltx_2_5/ltx_2_5_animate (duration enum 6/8/10, default 6) and veo3_1_extend
(fixed 7s/720p, drop those from the body/cost). Also re-check EXISTING routes against the fixed
parser — the same bug may hide broken existing models.
Minor (non-blocking): Wan 3.0 Prime "no ceiling" comment false; nano_banana_2_edit defaults 0.5K
but priced at 1K; `--dump --offline` restamps stale data; thumbnailUrl fields in the vendored
snapshot (418 storage.googleapis.com) are parsed but never rendered — keep it that way (hotlink).
Then (only after re-TEST PASS): gate.py, release.md, ff-merge to main, `tauri build --bundles
app`, reinstall (codesign --deep -s -, cp to /Applications, open).
Known gaps (honest, logged in implement.md): MiniMax H3 Max NOT landed (fal requires
`prompt_expansion_mode` w/ no default → needs a per-route constant-fields mechanism = own slice);
kling3_0_pro / wan3_0_prime t2v-only (i2v wants `start_image_url`, app sends `image_url` — and
existing `kling3_0` Animate-Image is ALREADY dead for the same reason → fix in a slice);
minimax_hailuo stays Higgsfield-only; resolver commits = ab06e31 + 8d8ecfb (not the 3 I guessed).

## QUEUED (owner order: money before looks): S9 → S10 → Storyboard
### S9 pricing accuracy + actual-cost tracker (owner: "many prices wrong; add a verifier")
- `actual_usd` hardcoded None in both clients (clients.rs ~:241/:395); apply_poll only writes Some.
- fal facts (memory `fal-cost-apis`): price lookup `GET api.fal.ai/v1/models/pricing?endpoint_id=`
  (key) → unit_price+unit{image,megapixel,second,request,compute_second}; usage
  `GET /v1/models/usage` (ADMIN key) → line items cost_total etc, minute buckets, NO request_id;
  requests `GET /v1/models/requests/by-endpoint` → per request_id `duration`, NO cost.
- Design: (a) price audit tool: every fal route's CostModel vs pricing API; fix mismatches;
  (b) per-job actual = unit_price × measured quantity (output seconds/images/megapixels, or
  request `duration` for compute_second) → write `actual_usd`; (c) reconcile vs usage line items
  in aggregate, flag drift; (d) UI: estimated vs actual per job + running spend.
- Cost-internals facts (explore done; not on disk elsewhere):
  · Live feed prices only ~5 of 54 fal routes (prices.rs::fal::parse_prose accepts 5 sentence
    templates, :819-841); 40 fal routes are hardcoded literals in registry.rs priced_routes
    (:600-1577), 14 Unknown. Feed wins over literal (src-tauri/src/pricing.rs:95-100). Bundled
    vendor/prices-snapshot.json = 23 quotes (18 vaig, 5 fal), one timestamp; refresh 24h.
  · TWO amplifier bugs: cost.rs:181/:203 `seconds.unwrap_or(0.0)` → confident $0.00 instead of
    None (violates "unknown never zero"); `with_minimum_seconds` (cost.rs:262, fal's 15s floor)
    defined but NEVER called → short clips under-quoted up to 3x. Fix both in S9.
  · Only guard: prices.rs:1483 test, 10x tolerance over the 23 bundled quotes. NO test compares
    a literal to fal's index `pricingInfoOverride` — yet fal_catalogue.rs::parse_pricing (:1171,
    `Model::pricing()` :430) already reduces 193 models to a Rate (537 prose, 761 none) and is
    used only by examples/fal_diff.rs printing. examples/price_check.rs (:44-87) already computes
    literal-vs-live diff — unwired.
  · actual_usd: FalClient::poll reads only `status`+`error` (clients.rs:224,:240); result body
    only URLs (outputs_from :77-124); HEADERS DROPPED in json_or_err (:30-42); zero hits for
    billable/metrics/x-fal/usage in the tree. engine.rs apply_poll early-return (:423-427) would
    drop a cost-only delta. UI: MetaCard.tsx:140-154 always "Not reported"; spend meter
    (MetaRail.tsx:26-43, lib/cost.ts:87) = estimate sum; `price_status` cmd registered (lib.rs:80)
    with ZERO UI callers; no estimated-vs-actual delta anywhere. Store: estimated_usd is
    write-once (store.rs:258-270, good); no provenance cols (priced_from/feed/priced_at/model).
  · S9 seams: (1) promote price_check.rs body → `prices::audit()`; (2) HERMETIC test: every fal
    route whose catalogue row is a `Pricing::Rate` must match its literal within tolerance,
    print prose for the rest; (3) 10x → hard fail, 1.2x → warning; (4) add transcribed_at/source
    beside each CostModel; (5) surface via price_status → MetaRail. Reconciliation: (6) capture
    headers+body candidates in poll → PollResult.actual_usd (downstream plumbing complete);
    (7) fix engine.rs:423 guard; (8) migration: job provenance cols; (9) MetaCard delta + running
    variance; (10) partial win needing no provider data: re-estimate at completion from the
    SETTLED output duration/resolution and reconcile vs estimated_usd (catches unwrap_or(0.0) +
    missing floor). Per-job actual = pricing-API unit_price × measured quantity (see memory
    fal-cost-apis); usage API for aggregate reconciliation (no request_id there).
### S10 redesign ("look like modern Higgsfield" — layout/feel only; provenance lint bans
### Higgsfield copy/media hosts; original assets)
UI explore findings (not on disk otherwise):
- Real token layer: `ui/src/tokens.css` 141 props, dark-only, documented cascade tokens→fonts→
  base.css→component sheets (app/rail/feed/meta/picker/setup.css). Re-skin = re-value tokens.
- Layout `app.css:51-58`: fixed grid 410px/1fr/390px (tokens.css:211-212), NO @media anywhere.
- Weak spots: ModelPicker.tsx = text-only list (no thumbnails/price); ResultsFeed = single
  column, no gallery/grid/lightbox/library; zero preview media (25 presets all CSS-transform
  fallback via CameraPreview/placeholder.ts); flat 10-item left rail (SettingsRail.tsx:206-253);
  type scale unused (only h-xs/h-sm).
- Bugs to fix first: `App.tsx:582-591` ConfirmDelete rendered INSIDE the titlebar <button>
  (invalid HTML); dead `ModelBrowser.tsx` + `browser.css` (~600 lines, imported nowhere);
  dangling `--color-vermilion` (picker.css:259 → use --accent) and `.btn-quiet` (ResultsFeed.tsx:53).
- Test-locked class names to KEEP: .model-item .model-item-name .model-item-main .chip-route
  .enhancer-picker .enhancer-model .enhancer-backend .prompt-preview-text .prompt-preview-retry
  .generate-button + data-unrunnable/data-unavailable/data-tier. Everything else free.
- Safest seams, in order: tokens.css re-valuation → base.css primitives → per-sheet restyle
  (feed.css gallery grid, picker.css list→tile grid keeping li/button skeleton) → additive
  breakpoints. Do NOT rewrite the 3-col grid first (scrollIntoView pairing App.tsx:544-551).
- Legal notice at SettingsRail.tsx:249-252 must stay visible. Design canvas (`/design` skill)
  was loaded but NOT started per owner (context limit) — do a 2–3 artboard mockup first.
### Storyboard / 30s ad (S11+): owner wants one brief → local Llama splits into a model-aware
shot list → editable storyboard (S7 preview per shot, S6 guard) → chain w/ last-frame continuity
→ crossfade → stitch. Facts: compositor.rs (1,695 lines) has concat/crossfade/trim/normalize/
variants/captions/ducking, tested, but NOTHING spawns ffmpeg (no render cmd; ffmpeg::locate has
0 callers). Missing: last-frame extract (~15 lines), render_timeline cmd (sidecar +
ProgressReader), job→job plan (JobSet relation + migration 7; hook runner.rs:431 download_outputs
/ OnUpdate). Continuity: MediaRole::Start→image_url, End→end_image_url (NO_END_FRAME list
media.rs:786); one extend model registered (grok_extend_video). Seedance 2.5 = native 30s (no
stitch); Veo 3.1 extend-video → 30s. Cost = Σ estimate_cost per clip + crossfade overlap.

## Open follow-ups (logged in backlog.md)
F1b resume drain · F4 retry_job/stalled UI (HIGH) · dep-bump h2/rustls · F6 adaptive enhancer
timeout under load · F7 probe-on-select unloadable models · F8 invalidate stale preview ·
S3 reachable-logs · MiniMax H3 Max constant-fields · kling3_0 i2v `start_image_url` fix.

## ===== HANDOFF (owner: "stop here and hand off to next session", 2026-09-16) =====
S8 is committed to branch `forge/s8-fal-catalogue-sync` as WIP — NOT merged to main, NOT pushed.
Done this session after the REVIEW FAIL: the 3 unrunnable models (ltx_2_5, ltx_2_5_animate,
veo3_1_extend) were REMOVED from registry.rs (JOB_TYPES, priced_routes arms, picker_only_specs,
test lists; IMPORTED_2026_09 14->11), media.rs (FAL_ROUTE_MODES 51->48), use_case.rs tests, and
CHANGELOG.md; addendum appended to implement.md. 11 verified models remain.
Mechanical gate after removal: cargo test = 656 pass / **1 FAIL** —
`registry::tests::registry_covers_the_catalogue_minus_its_exclusions` (registry.rs:2230), almost
certainly a registry-size/coverage count that still expects the 3 removed ids or the old total
(+30 -> +27). Fix that assertion first. clippy: no warnings. provenance: PASS. UI: 232 pass, build
ok. audit_fal: NOT re-confirmed (my grep for its section headers matched nothing — re-run
`env -u hickeyfield_FAL_KEY -u FAL_KEY cargo run -p hickeyfield-core --example audit_fal` and
read UNREACHABLE/CHECKED directly).
NEXT SESSION, in order: (1) fix the 1 failing test; re-run cargo test/clippy/fmt/provenance/
audit_fal/pnpm. (2) Spawn fresh REVIEW + TEST verifiers (opus) over the REDUCED set — never
hand-edit a verdict. (3) `python3 .forge/gate.py` → release.md → ff-merge to main → `tauri build
--bundles app` → reinstall (codesign --deep -s -, cp to /Applications, open). (4) Then the queued
slices per owner order: fal-schema-const-and-int-enums (re-land the 3 + re-probe all routes),
S9 pricing+tracker, S10 redesign, Storyboard. Owner prefs: agents `model:"opus"` by default,
`fable` only when necessary; owner is context-limited — keep agent count low.
