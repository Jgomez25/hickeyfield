# First light — 2026-08-05

The first time Hickeyfield ever talked to a real provider. Run with a live fal key via
`crates/hickeyfield-core/examples/first_light.rs`, which reads the key from `FAL_KEY`. Because
this example spends real money it refuses to run unless the process opts in with
`HICKEYFIELD_ALLOW_SPEND=1` — the same switch the packaged app sets for itself at startup (see
`crates/hickeyfield-core/src/spend_gate.rs`). Set `FAL_KEY` yourself — never copy it out of the
keychain into your shell history — and add the flag to acknowledge the spend:

```sh
HICKEYFIELD_ALLOW_SPEND=1 FAL_KEY=… \
  cargo run -p hickeyfield-core --example first_light
```

Without the flag the example prints the estimated cost and exits without sending anything; and
even if that refusal were bypassed, `FalClient::submit` is itself gated, so no billable request
can leave a process that did not opt in.

Everything in the registry until now was transcribed from documents. This is the first
evidence about which parts of it are true.

---

## What worked

**Upload.** `media.rs`'s fal path moved real bytes on the first attempt:
`POST /storage/upload/initiate` → signed `PUT` → public URL at `v3b.fal.media`. The two-step
flow verified against the official clients was correct.

**The dialect fix.** Binding a start frame produced `{"image_url": "…"}` — fal's parameter
name, not the catalogue's `image`. Had this shipped before the `Dialect` layer landed, every
fal image-to-video call would have 422'd. This is now confirmed live rather than by
inspection.

**Auth.** `Authorization: Key {key}` is the correct header shape for fal.

---

## What broke: the input-mode suffix was never implemented (resolved)

> **Resolved.** The resolver exists and is wired. `media::resolve_endpoint` reads the measured
> mode table `media::FAL_ROUTE_MODES`, and the submit path calls it before every fal request
> (`src-tauri/src/app.rs`). Both landed in `ab06e31` (2026-08-05, the commit this document was
> written alongside) and were extended to Higgsfield's fal-shaped mirror in `8d8ecfb`
> (2026-08-28). The narrative below is kept as written because it is the record of how the
> failure was found, not a description of today's code. What is still true: the mode list
> cannot be derived, only measured — which is why `examples/audit_fal` re-probes it and why
> no slug is added to the registry without passing that probe.

The first real submit failed:

```
model Seedream 5.0 Pro via bytedance/seedream/v5/pro
est   $0.0675
FAIL  submit rejected: HTTP 404: {"detail":"Application \"seedream\" not found"}
```

Probing all 36 fal route slugs against fal's public schema endpoint
(`fal.ai/api/openapi/queue/openapi.json?endpoint_id=…`, unauthenticated) gave:

| | Count | Meaning |
|---|---|---|
| Valid as-is | **16** | image models, mostly single-endpoint |
| Valid **with an input-mode suffix** | **17** | the adapter gap — see below |
| Not on fal at all | **3** | `kling3_0_turbo`, `minimax_hailuo`, `wan2_6` |

**33 of 36 slugs are correct.** The failure is not bad data — it is a missing feature that
`registry.rs` documents in its own header and that was never built:

> **Route slugs are family roots.** A single Higgsfield model maps to one endpoint per input
> mode on every provider (`/text-to-video`, `/image-to-video`, `/reference-to-video`, …), and
> the input mode is not known until the user has attached their media. The adapter appends the
> suffix; the route names the family.

Confirmed by probe — the bare root 404s while both suffixed forms resolve:

```
404  fal-ai/kling-video/v2.5-turbo/pro
200  fal-ai/kling-video/v2.5-turbo/pro/text-to-video
200  fal-ai/kling-video/v2.5-turbo/pro/image-to-video
```

So `FalClient::submit` posts a family root that can never resolve. **Every video generation
would have failed**, and no test could have caught it, because the only source of truth is
fal's live endpoint list.

### Not every family offers every mode

The suffix cannot be assumed — it must be *known*:

- `kling-omni-flf` offers **only** `/image-to-video`.
- `wan2_2_video` offers all four, including the image ones.
- `seedream_v5_pro` and `_lite` are image models and take `/text-to-image`.

Guessing here fails the same way the bare root does. This is the argument for **Phase C2** —
fetch and cache fal's published per-endpoint OpenAPI rather than hand-maintaining a suffix
table that silently rots.

### Three models are not on fal

`kling3_0_turbo` (`fal-ai/kling-video/v3/turbo`), `minimax_hailuo`
(`fal-ai/minimax/hailuo-2.3`) and `wan2_6` (`fal-ai/wan/v2.6`) resolve under no suffix. These
came from Higgsfield's *own picker*, which lists their in-house and newer versions, not from
fal's catalogue. Either they route elsewhere or they should be excluded — the registry's
`EXCLUSIONS` mechanism already exists for exactly this.

---

## Consequences

1. **Build the input-mode resolver.** *(done — `media::resolve_endpoint`, see the note above.)*
   Selection is deterministic from the attached media:
   no media → `text-to-*`; a start frame → `image-to-*`; a source video → `video-to-video`.
   The mode's *availability* per route is not deterministic and must come from fal's schema.
2. **`EXCLUSIONS` the three missing models**, or find their real route. *(done differently:
   `media::FAL_MISSING_ROUTES` pins them instead. `EXCLUSIONS` removes a model from the
   registry outright, which would also remove the Higgsfield route that can still run it;
   the pin only refuses the fal path, and `use_case::route_serves` keeps the dead tile out of
   the picker. Re-checked against fal's index on 2026-09-16: all three slugs are still
   unserved, though fal now serves Hailuo 2.3 and Kling v3 turbo under **tier-qualified**
   paths — `fal-ai/minimax/hailuo-2.3/pro/…`, `fal-ai/kling-video/v3/turbo/pro/…` — which are
   different endpoints with a different parameter surface, not the pinned slugs.)*
3. **Cost accuracy is still unverified against a live charge.** The run never reached a
   completed generation, so the estimator has still never been checked against a real
   invoice. Two things now narrow the gap without spending money:
   - a **keyless, hermetic price audit** (`cargo run -p hickeyfield-core --example price_check`)
     compares every fal route's built-in price against fal's own published figure in the
     vendored index snapshot and fails the build on a mismatch — see "Verifying prices
     against the live fal API" below for what it can and cannot check;
   - each completed job now records an **actual cost** measured from the file it produced,
     shown next to the estimate, so the first real generation will surface any drift on its
     own card.
   A live end-to-end charge comparison remains open.
4. `docs/PARITY.md` §3.3 can drop "no live generation has ever completed" for upload and
   binding, but **not** for the end-to-end path.

## How a per-second video estimate settles (output seconds, promotional rate)

A per-second video model bills on the seconds it *delivers*, not the seconds you *request*, and
a delivered clip can run slightly longer than asked. The estimate note now says this on every
per-second quote. When a job completes, its settled cost is re-priced from the file's measured
duration at the same route rate the estimate used, and the job card shows the delta.

Worked example — the workhorse (MiniMax H3 Max Turbo, $0.02/s at 768p): a 6-second clip whose
file measures 6.592 seconds settles at 0.02 × 6.592 = **$0.13184**, not the $0.12 a flat
6 seconds implies. (Grok Imagine 1.5 at 720p, 5 s → 5.04 s, settles $0.7056.) This settled
figure is re-priced from the delivered duration, not read from a provider invoice — reconciling
against a true billed charge is a separate, later change.

The H3 Max and H3 Max Turbo tiers run on a **promotional launch rate** (read from the provider
index on 2026-09-16). Each carries that date as a marker, shown as a small "promotional rate"
badge in the model picker. The price audit's re-check flags any promotional rate whose marker is
more than 30 days old, so it can be re-verified before it silently rots. The re-check only knows
the marker's *age*; it cannot tell whether the provider has actually changed the price.

## Verifying prices against the live fal API (human-run, spends no money)

The hermetic audit can only compare the handful of fal routes whose price prose reduces to
a single unconditional rate. The rest are conditional tables (per-resolution, audio-on/off)
the parser deliberately refuses. To check those, fal exposes a structured price lookup
(`GET /v1/models/pricing?endpoint_id=…`) that returns `{ unit_price, unit }` per endpoint —
a lookup, not a generation, so it starts and bills nothing.

An ignored integration test drives it against every fal route and prints a diff table. It
skips silently when no key is present, so the normal test suite never runs it, and it never
logs your key:

```sh
hickeyfield_FAL_KEY=…  cargo test -p hickeyfield-core \
    --test live_fal_pricing -- --ignored --nocapture
```

A row flagged `<-- DIFF` means the built-in literal in `registry.rs` disagrees with fal's
live figure by more than 20%; update the literal (citing fal's number) and re-run the
hermetic audit. This never runs `first_light` and never submits a job.

## Still unproven after this run

- Any completed generation, and therefore any cost comparison.
- The poll → download → `local_path` path (`runner.rs`), since nothing completed.
- Reattach-on-relaunch.
- Everything on Windows.
