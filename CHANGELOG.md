# Changelog

All notable, user-facing changes to Hickeyfield are recorded here. The format
follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the
project aims for [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Expensive routes the owner never uses are hidden from every picker, and the app refuses
  to run one by mistake.** Every Seedance and every Grok route — the ones that bill the
  provider for video the owner has decided not to buy — is hidden from all model pickers by
  default. A new Settings control, **"Show never-used routes (Seedance, Grok)"** (off by
  default), re-lists them when you want them. The refusal is enforced in the core, not just
  the UI: with the setting off, a hidden route is turned away before any provider is contacted,
  with a message that names the toggle — so a stored job, a re-run, a storyboard shot or a
  chained retry cannot slip one through. Jobs already made on a hidden route still show their
  real name and cost in history; nothing is deleted. This sits on top of, and is separate from,
  the spend gate below. (Persisted in a new `app_pref` settings table via store migration 12.
  Test counts rise to 979 Rust from 971 and 321 UI from 313.)

- **A per-second video quote now says it bills on the clip you get, not the clip you asked
  for, and re-priced settled cost proves it.** For the everyday workhorse (MiniMax H3 Max
  Turbo, $0.02/s at 768p) and every other per-second video model, the estimate note now states
  that billing is on delivered output seconds and that a clip can run longer than requested. When
  a job finishes, its settled cost is re-priced from the file's measured duration at the same
  route rate, and the job card shows the estimate-versus-settled delta: a 6-second workhorse clip
  measured at 6.592 seconds settles at $0.13184 (not the $0.12 a flat 6 seconds would suggest),
  and a 5-second Grok Imagine 1.5 clip measured at 5.04 seconds settles at $0.7056. (This settled
  number is re-priced from the delivered duration, not read from a provider invoice — matching a
  true billed charge is a later change.)

- **The workhorse's launch price now carries the date it was read and warns when it goes
  stale.** MiniMax H3 Max and H3 Max Turbo run on a promotional launch rate (read from the
  provider index on 2026-09-16); each now carries that date as a structured marker, surfaced as a
  small "promotional rate" badge in the model picker, and a monthly staleness re-check flags any
  promotional rate whose marker is more than 30 days old so it can be re-verified against the
  provider. The re-check is reachable from the keyless price audit
  (`cargo run -p hickeyfield-core --example price_check`) and, like every price tool here, spends
  nothing. (The marker flags marker age only; it does not detect an actual provider price change.)
  Test counts rise to 971 Rust (from 961) and 313 UI (from 312).

- **No process can spend money unless it explicitly opted in.** A single process-local spend
  gate now guards every billable request — the fal and Higgsfield generation submits and the
  hosted-LLM enhancer and storyboard-planner calls — and refuses with a clear message
  (`JobError::SpendBlocked`) before any HTTP when the process did not opt in. The packaged app
  turns spending on for itself at startup, so nothing changes for a normal user; tests, examples
  and any future CLI leave it off by default. The `first_light` example now refuses to run
  without `HICKEYFIELD_ALLOW_SPEND=1`, prints what a run would cost first, and no longer teaches
  copying the key out of the keychain. A new `scripts/lint-spend.py` gate fails the build if any
  source reads a raw credential outside the vault, strips a credential env var, or enables
  spending from a test. This closes the last path behind the 2026-09-16 incident, where a unit
  test reached the keychain and submitted real generations.

- **A model's duration control now offers the provider's real range instead of a generic
  ladder capped at 12 seconds.** MiniMax H3 Max offers 5–15 seconds, Wan 3.0 Prime 2–30, and
  every model whose provider publishes or documents a range its real ceiling (models that
  publish neither still use the generic ladder); switching to a model clamps a staged
  duration into its range, and a value outside the range is clamped with an advisory before
  anything is priced, so the estimate is for what will actually render. The keyless model
  audit gains a check that a model's documented range never exceeds the range fal publishes.

- **A 30-second storyboard fills any model's per-call ceiling and holds continuity with a
  shared world block.** A brief longer than a model's ceiling is split to fill it (2×15 on
  H3 Max), and the planner composes one shared world description — subject, palette,
  lighting, lens, grade — verbatim into every shot, with each non-final shot ending on a
  static hold the next shot opens on. The validator rejects a continuous run that drops the
  world block, repeats a beat, omits a hold, or uses a scene-break word.

- **Every chained join is scored for continuity and flagged.** When a chained shot settles,
  the bundled ffmpeg measures how well its first frame matches the previous shot's last frame
  (SSIM plus colour drift) and stores a per-join score; the Library shows the set's lowest
  join score and flags a low one. Scoring spends nothing — it runs locally on saved frames.
  A below-floor join can be regenerated "from the same frame" through the same money-gated
  path (cost shown first), and an optional bounded auto-retry (off by default, at most two
  attempts, each priced first) can do it automatically. A below-floor join flags but never
  blocks paid work, and a join that cannot be scored is left unscored, never counted as a
  failure.

- **A storyboard can now be generated in keyframe-anchored mode.** The app renders one still
  per shot boundary (a shared world block + the shots' hold descriptions), prices every still
  into the total shown before anything is spent, and anchors each shot on *both* ends — the
  previous clip's true last frame in, the next planned still out — so every join is pinned
  from both sides and scored against the ideal tier plus an end-frame-adherence score (the
  clip's actual last frame vs its planned still). Money honesty: the total includes the
  stills; a failed still parks the set with a clear error and never submits a shot against a
  missing anchor; stills and shots have separate ≤2 auto-retry bounds; scoring spends nothing
  (local ffmpeg on saved frames). The toggle enables only when every shot's model can take an
  end frame and an image model is available; the threshold calibration on real footage stays
  human-run.

### Changed

- **The app now starts, and stays, on the everyday workhorse.** Studio opens on MiniMax H3
  Max Turbo ($0.02/s at 768p), keeps it when you switch use-case tabs wherever it fits, and a
  new storyboard shot defaults to it — instead of whatever model happened to sort first. The
  launch set that seeds these defaults swaps the old Seedance family out for the workhorse, and
  the demo data, samples and provider blurbs no longer lead with — or advertise — Seedance or
  Grok.

- **The app can now send a required setting a provider does not expose as a control.** Some
  fal endpoints demand a body field even though they publish a default for it; the request
  used to be refused before it was sent. Those fields are now supplied automatically, only
  ever for an endpoint that accepts them and never overriding a value you set, and the
  keyless model audit verifies every such field against fal's own schema.

- **The window is redesigned into a stage-first Studio and a Library gallery.** Studio now
  shows your latest result large — with its model, duration, resolution and its estimated
  *and* actual cost on one caption — a strip of recent generations, and a 72-pixel use-case
  nav (Video, Animate, Image, Edit, Extend, plus Keys) down the left. The composer keeps the
  same controls, reordered around a model tile and a sticky, priced Generate bar. A new
  Library view is a gallery of every generation with a lightbox and the per-generation cost
  card, and the model picker is now a tile grid. The legal notice stays visible and no
  behaviour, pricing or provider wiring changed.

- **The enhancer now recommends good local models and warns about slow ones.** The
  model picker labels each installed Ollama model — small, fast instruct models suited
  to prompt rewriting (Llama 3.2, Phi-4 Mini, Gemma 3 family) are marked *recommended*
  and auto-selected first; vision models, chain-of-thought reasoning models, and very
  large models are flagged as likely slow or unsuitable. You can still pick any of them.
  The "no model installed" hint now suggests the recommended set generically instead of
  a single fixed model.

### Added

- **A storyboard can now be chained and rendered into one finished ad.** With Chain on
  (the default when every shot's model can continue from a frame), each shot is generated
  from the previous shot's true last frame, so the second beat continues the first in
  pixels rather than being re-drawn from the prompt; a shot whose model has no
  image-to-video route falls back to the prompt with a note. Once every shot is done, one
  click stitches the clips through the bundled ffmpeg — a continuity join is a clean cut,
  any other join a short crossfade — into a single ad that opens from the Library with its
  total actual cost, behind a real progress bar. Rendering spends nothing: it runs on your
  own machine, and the app says so. Chaining runs the same per-shot estimate and guard a
  single generation does, shows the total before anything is submitted, and never submits a
  shot whose predecessor failed. A storyboard can also be a single shot now, so a
  one-take 30-second clip or a two-shot 30-second chain are both drafts you can make.

- **MiniMax H3 Max and H3 Max Turbo are in the picker, for text-to-video and
  animate-an-image.** Both render 5-to-15-second clips at 480P, 768P or 1080P, with a choice
  of six aspect ratios when you start from a prompt. They are priced at fal's promotional
  launch rates — H3 Max at $0.025/$0.04/$0.08 per second across the three resolutions, and
  the Turbo tier at half that — and the Generate button now prices the default 768P
  resolution correctly.

- **A Storyboard view turns one brief into a whole shot list you can edit and generate as a
  set.** Describe what you want ("a 30-second ad for …") and pick a target length; the local
  or hosted enhancer drafts a model-aware shot list — per shot a title, prompt, duration,
  camera move, model and continuity role — validated so every model is one that can actually
  run the job, every camera move is real, and the durations sum to roughly the length you
  asked for. Each shot's duration is clamped to what its model actually offers, with an
  advisory when it changes; a single-shot plan on Seedance 2.5 can keep a native 30 seconds.
  Edit any shot's prompt, seconds, camera or model, reorder, add or delete, and preview each
  shot's compiled prompt before spending anything. The view shows the summed estimate — a
  shot with no published price is marked, never counted as free — and Generate all submits
  every shot as one linked set through the same money path a single generation uses, so
  nothing is submitted without the total shown. The Library groups a generated set under its
  brief. Drafting is free on local Ollama and costs one short completion on a hosted key; no
  clip is generated until you see the total. If a shot fails (a refusal, a missing key), the
  others still start and you can retry just the failed ones — the retry re-uses the same full
  plan and its subset total, and lands the retried shots back in the original set rather than
  a new group. (Rendering and stitching the set into one film comes in a later release — this
  slice plans and generates the shots.)

- **Every finished job now records what it actually cost, next to what we quoted.** When
  a generation completes and its file is saved, the app measures the seconds, resolution
  and image count the provider actually produced and prices that at the same rate as the
  estimate, so each card shows estimated cost, actual cost, and the difference between
  them. The session spend meter shows where its prices came from — a live feed with its
  age, or the bundled snapshot, never implied to be current. A new keyless price audit
  checks every fal route's built-in price against fal's own published figure and fails
  the build if a hand-typed number ever drifts from it.

- **fal's newest models are in the picker, and a 30-second clip is now possible.**
  Seedance 2.5 moved from a placeholder to a real, priced model and offers every
  length fal takes, up to a native 30 seconds — the longest single clip the app has
  ever been able to ask for. Alongside it: Kling 3.0 Pro and Wan 3.0 Prime.
  On the image side: GPT Image 2.5 in both of its tiers, Qwen Image 3, and — as
  their own entries, because they are separate endpoints — the edit paths for Nano
  Banana Pro, Nano Banana 2 and Seedream 5.0 Pro. Each was checked against the
  provider's published schema before being offered, so a model that appears under a
  tab is one that can actually run there.

- **Preview, edit, and retry the enhanced prompt before generating.** A new
  "Preview prompt" button shows exactly what the enhancer will send — the rewritten
  scene plus your original for reference — in an editable box. Edit it if it assumed
  too much, hit Retry for a different rewrite, then Generate with exactly the text
  shown. The one-click Generate still works as before for when you trust the enhancer.
  The preview is a dry run: it never starts or charges a generation.

- **The prompt enhancer now actually rewrites your prompt.** Previously the Enhance
  switch was on by default but nothing behind it ran, so a short prompt went to the
  model word-for-word. It now expands your scene through the filmmaking corpus using
  a local model you pick (any model you have installed in Ollama, auto-detected) or a
  hosted OpenAI key, and shows which model rewrote each job. When Enhance is on but no
  model backend is set up, it tells you plainly instead of silently sending the prompt
  as written.

### Fixed

- **An unknown price no longer reads as $0.00, and the provider-minimum machinery is
  wired.** Two per-second billing shapes quoted a paid generation as free when the clip
  length or resolution was not yet known; they now say the price is unavailable, the
  same honest unknown the rest of the app uses. The 15-second billing floor some
  providers charge is now applied to any route that declares one (no current route does,
  so no short clip is over-quoted) rather than being dead code.

- **Four generation paths that could never run are now available, and a whole class
  of silent rejection is gone.** Some providers fix a setting for you — an endpoint
  that always renders seven seconds at 720p, or always takes its shape from the image
  you attached — and some list their lengths as plain numbers rather than text. The
  app read neither, so it sent a value the provider would refuse and the generation
  failed before it started. It now reads both, shows a fixed setting as the single
  option it is, and says so on the job when a saved setting cannot apply. With that
  in place: LTX 2.5 Pro and LTX 2.5 Pro Animate (6, 8 or 10 seconds at 720p or
  1080p), Veo 3.1 Extend for continuing a clip, and Seedance 2.5's animate-an-image
  mode — which can also transition between a start and an end frame — all work and
  are in the picker.

- **A model that refuses to rewrite no longer costs you a generation.** If the
  local or hosted enhancer replies with a refusal, apology, or other non-prompt
  ("I can't complete this task", "As an AI…") instead of a rewritten scene, the
  app now detects it and submits your original prompt with a note explaining what
  happened — instead of sending the refusal text to the paid model.

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
- **A job that never comes back is eventually set aside instead of retried
  forever.** If a provider keeps a request open with no result after several full
  waiting cycles, or after seven days (by which point providers have discarded the
  output), the job is marked as stalled and stops being auto-retried on every
  launch, so stuck jobs no longer pile up in the background. The job's record is
  kept so it can be tried again with Rerun.
- **A model that could never actually run is no longer shown as a free option.**
  The one model served solely by the built-in "Local" provider (`z_image`) used to
  appear priced at $0.00 with a Generate button, then fail the moment you pressed
  it, because the app has no local (ComfyUI or Ollama) client to run it yet. Local
  is now treated as having no runnable client, so that model is hidden alongside the
  other routes the app cannot execute and is reported as unavailable with a plain
  reason instead of a price it cannot honour. It returns on its own once a real
  local client ships.
