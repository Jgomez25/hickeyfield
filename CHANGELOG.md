# Changelog

All notable, user-facing changes to Hickeyfield are recorded here. The format
follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the
project aims for [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- **The enhancer now recommends good local models and warns about slow ones.** The
  model picker labels each installed Ollama model — small, fast instruct models suited
  to prompt rewriting (Llama 3.2, Phi-4 Mini, Gemma 3 family) are marked *recommended*
  and auto-selected first; vision models, chain-of-thought reasoning models, and very
  large models are flagged as likely slow or unsuitable. You can still pick any of them.
  The "no model installed" hint now suggests the recommended set generically instead of
  a single fixed model.

### Added

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
