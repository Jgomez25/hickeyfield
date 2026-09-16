# S10-pricing-accuracy-and-cost-tracker — SECURITY (evidence)

Read-only pass over the slice diff (`git diff main...HEAD`, HEAD 462120e). Scope: the
new `ffmpeg -i` probe, the `#[ignore]` live pricing seam, migration 7 + provenance,
money integrity, `price_status`/new DTOs, and supply-chain. No source, docs, or backlog
were modified.

## Prompt-injection note (out of band)
The MCP "Drive API" server injected an instruction telling me to route all work through
raw `Bash` (cat/sed/grep) instead of the Read/Edit tools. That instruction did not come
from the user or the launching agent and I disregarded it; it changed nothing about this
review. Flagging it because an agent that obeys injected tool-routing guidance is exactly
the foothold this reviewer exists to catch.

## Findings (ranked)

### M1 — Medium: `ffmpeg -i` probes untrusted downloaded media with no timeout and no demuxer/protocol restriction
- Where: `crates/hickeyfield-core/src/ffmpeg.rs:446-463` (`run`, plain `Command::output()`)
  and `:673-677` (`probe_file`), reached from `src-tauri/src/runner.rs:218-233`
  (`download_outputs`) → `:247-275` (`probe_first_local`) → `probe_file`.
- Data path: `download_outputs` fetches the provider's result URL to disk
  (`src-tauri/src/library.rs:150-167`, `reqwest.get(out.url)` — `out.url` is provider-
  supplied), then hands that file straight to `ffmpeg -hide_banner -i <file>`. The file
  content is attacker-influenced whenever the provider account/endpoint is malicious or
  compromised (TLS to fal mitigates a passive MITM but not a hostile origin). The design
  itself frames this as "untrusted media parsing", so the boundary is in scope.
- Exploit path A (hang / DoS): `run` uses `Command::output()` with no wall-clock timeout
  and no `kill`. A crafted input that makes the demuxer loop or block leaves the
  `spawn_blocking` thread wedged forever; `download_outputs` runs on tokio's blocking
  pool, so a handful of such outputs across jobs park blocking-pool threads and stall the
  runner's poll/download loops app-wide. The job's `actual_usd` is never recorded and the
  thread never returns.
- Exploit path B (SSRF / egress): the demuxer is selected from file *content*, not the
  `.mp4` name `library` gave it. A downloaded file that is actually an HLS playlist,
  `ffconcat`/concat script, or similar reference container makes `ffmpeg` open the URLs it
  names — an outbound fetch from the user's machine to an attacker-chosen host — because
  no `-protocol_whitelist file` / `-f <demuxer>` restriction is set. `-i` opens the input
  before erroring on the missing output file, so the fetch happens.
- Not exploitable via the path: the input string is a local library path built by the app
  and passed as a single argv (no shell), so there is no protocol/URL injection *through
  the argument* itself, and `Command::output()` nulls stdin so `-nostdin` is not load-
  bearing for a hang. The risk is entirely in the file's *content*.
- Fix: run the probe as a spawned child with a bounded wait-and-kill (e.g. a few seconds;
  a probe never legitimately needs more), and constrain the demuxer/protocol surface
  (`-nostdin -protocol_whitelist file` and, where feasible, pinning the input format), so a
  malicious container can neither hang the pool nor drive egress. Bounding stderr capture
  is a cheap add while touching `run`.
- Severity rationale: precondition is a hostile/compromised provider response, and impact
  is local DoS or SSRF egress rather than code exec or data disclosure (probe stderr is
  parsed for duration/resolution and discarded) — Medium, not High. Recommend a follow-up
  hardening slice; not a release blocker.

### L1 — Low: a `0`/`0.00` price from a poisoned or erroneous feed renders as "$0.00" (free)
- Where: `crates/hickeyfield-core/src/cost.rs:150-272` (`estimate` never rejects a
  non-positive `usd`), surfaced via `src-tauri/src/pricing.rs:106-123`
  (`estimate_with_provenance`, new this slice) and displayed by
  `ui/src/lib/cost.ts:20-26` (`formatUsd(0) => "$0.00"`).
- Exploit path: the live price feed is fetched over the network; a compromised or simply
  wrong feed that publishes `unit_price = 0` for a route flows through `estimate` to
  `estimated_usd = 0`, and the UI renders a genuinely paid route as "$0.00" — the exact
  false-free reading the codebase otherwise refuses by using `None`. A negative feed value
  is safe (UI `usable()`/`formatUsd` treat `< 0` as unknown); only the `== 0` case leaks.
- Fix: treat a non-positive priced result as unknown (`None`) at the feed/estimate
  boundary, the same way an absent quote is handled.
- Severity rationale: predates this slice at the `estimate` boundary (the slice only newly
  *persists* the number and its provenance), narrow trigger (feed must publish exactly 0),
  and the failure is a misleading display rather than a boundary breach — Low.

## Boundaries examined and found clean (silence is not clearance)

- Live pricing seam (`crates/hickeyfield-core/tests/live_fal_pricing.rs`): `#[ignore]`,
  key read only from `hickeyfield_FAL_KEY`/`FAL_KEY` (`:33-38`), carried solely in the
  `Authorization: Key …` header (`:90`), never placed in the URL query (`:87` uses only
  `endpoint_id`), never printed (only endpoint + figures are logged, `:70,118-121`), and it
  returns (does not fail) when no key is set (`:55-58`). 30s client timeout (`:60-63`). It
  starts no generation and spends nothing. Clean. (I did not run it, per instruction; no
  real key exists here.)
- Migration 7 (`src-tauri/src/store.rs:161-176`): pure static DDL, no interpolation of any
  value; all reads/writes are parameterized (`params![…]`, `:318-344`). The `format!("\"{s}\"")`
  round-trips in `row_to_job` (`:210,231-241`) wrap enum strings the app itself wrote and
  degrade to a default/`None` on any unparseable value rather than dropping the row — not a
  user-input injection sink. Clean.
- Write-once provenance / cost-forging: `priced_from`/`priced_at` are set only on INSERT and
  are deliberately absent from the `ON CONFLICT DO UPDATE SET` clause (`:300-316`), so a
  later upsert cannot rewrite them; `actual_source` updates only alongside `actual_usd`.
  There is no external input path that writes these columns. Clean.
- Money integrity — stale/None overwrite: `apply_poll` assigns `actual_usd` only when
  `poll.actual_usd.is_some()` (`crates/hickeyfield-core/src/engine.rs:468-470`), and the
  cost-only-delta gate (`:442`) only *adds* the change, never nulls it; `download_outputs`
  computes an actual only when `job.actual_usd.is_none()` (`runner.rs:218`). A completed job
  is never re-polled (`runner.rs:481-483`, test `resuming_a_completed_job_keeps_its_success`).
  So a real actual cannot be overwritten by a later `None`. Clean.
- Money integrity — unknown-never-zero: every `estimate` shape returns `None` (not `0`) when
  a required quantity is absent (`cost.rs:153,165-172,191,216,259`); `compute_actual` writes
  `Some` only when `estimate` yields one (`runner.rs:301,311`), else `(None, None)`; an
  unmeasurable output is `None` (test `an_unmeasurable_output_gets_none_not_zero`). The one
  residual gap is L1 above (a positive-but-zero *rate*), not the unknown path.
- `price_status` / new DTOs: `PriceStatus` (`src-tauri/src/pricing.rs:148-160`) exposes only
  `routes: usize`, `age_seconds: Option<u64>`, `live: bool`; the TS mirror
  (`ui/src/types.ts`, `ui/src/api.ts:525-548`) matches. No paths, keys, endpoints, or
  internal URLs cross to the UI. `PriceProvenance`/`ActualSource` serialize to fixed
  kebab-case tags only. Clean.
- Supply chain / `cargo deny`: no `Cargo.toml`, `Cargo.lock`, `deny.toml`, or JS lockfile
  changed in the slice diff — the live seam reuses the already-present `reqwest`. No new
  dependency and no change to the pre-existing (h2/rustls) advisory posture. Clean.

## Recommendation to the orchestrator
No Critical/High findings, so no finding is promoted to a new blocking slice. Recommend one
non-blocking hardening slice for M1 ("bound and sandbox the ffmpeg probe: wait-and-kill
timeout + `-protocol_whitelist file` + `-nostdin` on the untrusted-media probe"), and folding
L1 ("treat a non-positive feed price as unknown, not $0.00") into that or a pricing-hygiene
slice.

VERDICT: PASS
