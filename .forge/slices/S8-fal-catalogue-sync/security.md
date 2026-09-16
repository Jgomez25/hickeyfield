# S8-fal-catalogue-sync — SECURITY (evidence)

Read-only pass, proportionate to what this slice actually touched: one new keyless
developer example (`examples/fal_diff.rs`), a printing fix in `examples/audit_fal.rs`, a
refreshed third-party data file vendored into the binary, registry/media table rows, docs.
No new dependency, no network call on any user-facing path, no credential handling.

---

## 1. `fal_diff --dump` — untrusted fetch that produces a committed artefact

**Path traversal / arbitrary overwrite: none.** `dump_snapshot`
(`crates/hickeyfield-core/examples/fal_diff.rs:78-99`) writes to **stdout** via `println!`
and takes no path, filename or destination argument at all; `main` accepts exactly two
flags and exits 2 on anything else (`:43-49`). There is no `File::create`, no
`std::fs::write` and no path built from fal's response anywhere in the example. The file
name in the documented recipe is supplied by the operator's own shell redirect
(`fal_catalogue.rs:45`), so nothing fal returns can steer where bytes land.

**Is fal's index ever interpreted?** No. `fal_catalogue::fetch` parses each page with
`serde_json::from_value::<Model>` into a fixed struct of `String`/`bool` fields
(`fal_catalogue.rs:400-422`); `dump_snapshot` re-serialises the same struct. Nothing is
`eval`'d, shelled out, used as a path, or used to construct a request URL — the only URL
built is `{FAL_MODELS_URL}?page={n}` with `n` a `u32` counter (`fal_catalogue.rs:812`).
Endpoint ids from the index never become routes: the registry's slugs are compile-time
literals, and the only place an index id is consumed is `Catalogue::get` in a test
assertion (`registry.rs:2972`) and `println!` in the report.

**Hostile / malformed index:** the pre-existing guards are intact and this slice did not
weaken them — `MAX_PAGES = 60` caps the crawl (`fal_catalogue.rs:72`), the per-request
`FETCH_TIMEOUT` is applied on the client builder (`:798`), an unreadable-row share above
`MAX_UNREADABLE_SHARE` aborts with `Malformed` rather than silently shrinking the roster
(`:840-847`), an empty result is an error not an empty catalogue (`:833`), and a bot
checkpoint is distinguished from a 500 (`is_bot_checkpoint`). `fal_diff` surfaces all of
that: `fetch()` errors go to stderr and `exit(1)` (`fal_diff.rs:54-62`) — no partial
catalogue is ever reported or dumped. Additionally `--dump` refuses outright when
`retired() > 0` (`:79-87`), which is a genuine integrity guard rather than a cosmetic one.

**Memory bound — accepted, pre-existing.** There is no explicit byte cap on a page body
(`resp.text()`); the bound is 60 pages × timeout. A hostile `fal.ai` could return large
bodies. Unchanged by this slice, developer-invoked only, TLS-authenticated origin. Low;
recommend a `Content-Length`/streaming cap as a general hardening follow-up, not as a
condition of this slice.

**Transport:** `https://fal.ai/api/models` and
`https://fal.ai/api/openapi/queue/openapi.json` (`prices.rs:58`, `fal_schema.rs:277`),
reqwest defaults (system trust store, no `danger_accept_invalid_certs` anywhere in the
crate). The browser-shaped headers are anti-WAF, not authentication.

## 2. Credentials

`grep` for `env::var`, `FAL_KEY`, `Authorization` and `bearer` across
`fal_catalogue.rs`, `fal_schema.rs`, `examples/fal_diff.rs` and `examples/audit_fal.rs`
returns **nothing**. Both tools are unauthenticated by construction, both were run in this
review with no key present and both completed (66 endpoints probed, 1491 rows diffed). No
secret can be read, logged or leaked by anything this slice added. The `audit_fal` output
change prints model ids and endpoint slugs only.

## 3. The vendored snapshot as untrusted third-party data in the binary

`vendor/fal-catalogue-snapshot.json` (17.8k lines changed) is compiled in via `include_str!`
(`fal_catalogue.rs:70`) and parsed as JSON on demand. Audited the refreshed file directly:

- 1491 rows, every `id` matches `[A-Za-z0-9._/-]+` — no `..`, no absolute paths, no schemes;
- **zero** control characters in any string field (so no terminal-escape payloads);
- every `thumbnailUrl` is `https://`; hosts are `v3b.fal.media` (1018),
  `storage.googleapis.com` (418), `v3.fal.media` (48), `refinery.fal.media` (4),
  `fal.media` (2), `pbs.twimg.com` (1);
- **no** Higgsfield CDN or CloudFront host from `scripts/lint-provenance.py`'s
  `FORBIDDEN_HOSTS` appears anywhere in the file (grep count 0).

**Hotlinking: not happening today, and worth pinning.** `Model::thumbnail_url`
(`fal_catalogue.rs:420-421`) is parsed but never read: no consumer exists in
`src-tauri/`, `ui/src/` or the core crate — the catalogue module is still wired to nothing
but the new example and one registry test, and the only `thumbnail` hits in `ui/` are CSS
comments about local media. So no third-party image is fetched or rendered. The residual
risk is forward-looking: the moment a card renders `thumbnail_url`, the app hotlinks
`storage.googleapis.com` and `pbs.twimg.com`, and the provenance lint **cannot** catch it —
`vendor/` is in `SKIP_DIRS` (`lint-provenance.py:52`) and those hosts are not in
`FORBIDDEN_HOSTS`. Severity low, no exploit today. Recommended follow-up (not applied):
either strip `thumbnailUrl` in `--dump`, or add the fal/googleapis/twimg media hosts to the
lint's forbidden list so a future UI change trips the gate rather than shipping silently.

**Provenance of the prose:** the snapshot carries fal's own pricing sentences. The strings
this slice *ships in source* — the `Route::noted` notes in `registry.rs` — are reworded,
and `./scripts/lint-provenance.py` passes (copy provenance PASS, 80015 shingles indexed).

## 4. Low-severity observations

- **L1 — terminal escape passthrough in the developer report.** `fal_diff.rs:242-248`
  (`truncate`) strips `\n`/`\r` from provider-controlled `title` / `model_family` /
  pricing prose before printing, but not `ESC`. A hostile index could in principle emit ANSI
  sequences into a maintainer's terminal. The current snapshot contains zero control
  characters, the tool is developer-only, and the impact is cosmetic-to-annoying. Fix if
  cheap: filter `c.is_control()` rather than just the two newline forms.
- **L2 — data integrity, not security: `--dump --offline` restamps the bundled snapshot
  with today's date** (`fal_diff.rs:88-90`). Detailed in `review.md` §M3; it can launder
  stale provenance into a committed file, which is why it is worth fixing even though it is
  not an attack surface.
- **L3 — `--dump > vendor/…json` truncates the fallback before the fetch runs**
  (`review.md` §M4). Denial-of-freshness at worst; the failure is loud (`include_str!`
  stops compiling) and git restores it.

## 5. Dependencies

`Cargo.lock` is **unchanged** (`git status --porcelain Cargo.lock` empty). The only
manifest edit is the `[[example]] name = "fal_diff", test = true` target
(`crates/hickeyfield-core/Cargo.toml:24-30`) — no new crate, no version bump, no feature
change. The baseline `h2` / `rustls` advisories therefore stand exactly as they did before
this slice; nothing here changes their reachability (no new network surface on a
user-facing path — both tools are `examples/`, not shipped in the Tauri binary).
`cargo deny` was not run by the implementer and is not required to clear this slice, since
the dependency graph is byte-identical.

---

No critical or high finding. Nothing here warrants its own security slice; L1–L3 are
hardening notes for the follow-up that fixes the review's blockers.

VERDICT: PASS
