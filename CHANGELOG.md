# Changelog

All notable, user-facing changes to Hickeyfield are recorded here. The format
follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the
project aims for [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

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
  clip is generated until you see the total. (Rendering and stitching the set into one film
  comes in a later release — this slice plans and generates the shots.)

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
