# Changelog

All notable, user-facing changes to Hickeyfield are recorded here. The format
follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the
project aims for [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
- **A job that never comes back is eventually set aside instead of retried
  forever.** If a provider keeps a request open with no result after several full
  waiting cycles, or after seven days (by which point providers have discarded the
  output), the job is marked as stalled and stops being auto-retried on every
  launch, so stuck jobs no longer pile up in the background. The job's record is
  kept so it can be tried again with Rerun.
