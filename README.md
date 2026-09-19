# Hickeyfield

A free, open-source desktop studio for AI video and image generation. Bring
your own API keys and pay providers directly at cost — there is no subscription,
no credits, no plan gating, and nothing to pay us.

A *hickey* is the speck of dirt that prints as a haloed blemish — the flaw that
proves a thing went through a real press rather than a renderer.

> Hickeyfield is an independent open-source project. It is not affiliated with,
> endorsed by, or sponsored by Higgsfield, Inc.

**Status: early, and honest about it.** Hickeyfield generates: you add a provider
key, pick a use case, attach media if the job needs it, and get real video and
images back, saved to a folder you choose. What is *verified end to end* is the
fal route on macOS — upload, submit, poll, download, play. Windows builds in CI
on every commit but has not been exercised by hand. The surfaces beyond the
generator (library, audio, studios) are not built yet.

## Install

The guided install walks you through it start to finish, on macOS or Windows:

**[publikhq.com/hickeyfield](https://publikhq.com/hickeyfield)**

Or build it yourself — see [Development](#development). You will need a key from
at least one provider; [fal.ai](https://fal.ai) alone is enough to use
everything the app can currently reach.

## Why

The best AI media tools are harnesses: they wrap third-party models in presets
and a good interface. The interface is the product, and the interface is
reproducible. What isn't reproducible is subsidised bulk model pricing — which
is exactly what a subscription buys you, and exactly what you don't need if you
already hold your own API keys.

So: same features, same workflow, your keys, at cost.

### What that costs

On flagship video you may pay slightly more per generation than a bulk
subscriber, because large platforms buy in volume and subsidise. On Veo
Fast/Lite, Grok direct, Gemini Omni, Seedream, Recraft, upscaling and every
self-hosted path, you pay considerably less. And you never pay for a month you
didn't use.

Hickeyfield always shows the real USD cost before you submit, computed from live
provider price feeds rather than an opaque credit integer.

## What it does today

The generator is organised by **use case** rather than by model, and each one
offers only the models that can actually do that job — so a model that cannot
take a video never appears under *Edit Video*, instead of failing after you have
attached one and pressed Generate.

- **New Video** · text to video
- **Animate Image** · a still becomes a shot
- **Edit Video** · change something in footage you already have
- **New Image** · text to image

There is also a **Storyboard** view: from one brief and a target length, the local or
hosted enhancer drafts a model-aware shot list — per shot a prompt, duration, camera move,
model and continuity role — which you edit, preview and then generate as one linked set. A
target longer than a model's per-call ceiling is split to fill it (a 30-second brief on
MiniMax H3 Max becomes two 15-second shots), and a shared *world block* — subject, palette,
lighting, lens, grade — is composed verbatim into every shot so a chained set holds its
identity across cuts. Each join between consecutive shots is scored for continuity by the
bundled ffmpeg (a local, free measurement); the Library shows the set's lowest join score
and flags a low one, and a below-floor join can be regenerated from the same frame — cost
shown first — or, with an optional toggle, retried automatically up to twice. Drafting is
free on local Ollama and costs one short completion on a hosted key; no clip is generated
until the summed total is shown, and a shot with no published price is marked rather than
counted as free. Duration controls offer the range a model's provider publishes or documents
(H3 Max 5–15s, Wan 3.0 Prime 2–30s) rather than a generic ladder — models that publish neither
fall back to the generic ladder — and a value outside the range is clamped with an advisory
before it is priced.

Everything a request depends on is checked against the provider's own published
schema before anything is sent: which inputs the endpoint accepts, which values
it enumerates, and what shape the result will be. A setting the endpoint does
not have is reported rather than silently dropped, and a value it will not
accept is refused before you are charged rather than after.

Prompts can be expanded through a filmmaking corpus — camera, lens, light,
motion — either deterministically from a preset, or through a local model via
Ollama. Nothing is sent anywhere for this unless you ask for it.

## Architecture

Tauri v2 — a Rust core with a React UI in the system webview.

```
crates/hickeyfield-core/   provider adapters, routing, job engine, presets, costs
                        (no Tauri dependency, so tests run in seconds)
src-tauri/              the desktop shell: commands, events, window lifecycle
ui/                     React + Vite frontend
scripts/                signing, verification and provenance tooling
provenance/             one-way hashes used by the copy-provenance lint
```

Being native rather than a web app is load-bearing, not incidental:

- **No CORS**, so every provider is reachable directly, including the several
  that refuse browser requests outright.
- **No proxy**, and therefore no server that could hold or leak a key.
- **Keys live in the OS keychain** — macOS Keychain, Windows Credential Manager
  — and never cross into the webview. The UI can ask *whether* a key is set; it
  can never read one.
- **Jobs run in the Rust core**, so they survive the window closing and reattach
  on relaunch.
- **Export uses native FFmpeg** with hardware encoding, so there is no length or
  resolution ceiling.

## Development

Requires Rust (1.85+), Node 24, and pnpm 11.

```sh
cd ui && pnpm install
cd .. && ui/node_modules/.bin/tauri dev
```

Run the gates the way CI does:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo deny check
./scripts/lint-provenance.py
./scripts/lint-spend.py
```

### Spending and credentials

Two environment flags govern money and keys, and both default to the safe
setting:

- **`HICKEYFIELD_NO_KEYCHAIN=1`** — never read the login keychain (which holds
  your real keys). Every automated run — CI, agents, tests — sets this, so a
  test can never fall through to a real credential. Test builds block the
  keychain unconditionally.
- **`HICKEYFIELD_ALLOW_SPEND=1`** — the *only* way any binary or example makes a
  billable request. The packaged app sets it for itself at startup, so a normal
  user is unaffected; everything else — `cargo test`, the examples, a future CLI
  — leaves it off, and a billable call there is refused before any HTTP with a
  message naming this flag.

The rule: **no process spends unless it opted in, and tests and examples never
opt in.** `scripts/lint-spend.py` enforces the mechanics — no raw credential
read outside the vault, no env-stripping, and no test enabling spending.

### Never-used routes

Some registered routes are ones this build is set never to buy — every Seedance
and every Grok video and image route — because they bill the provider for output
that is not worth their cost here. They are **hidden from every model picker by
default**, not deleted: the registry, prices and capability data all remain, so a
job you already ran on one still shows its real name and cost in history.

A single Settings control, **"Show never-used routes (Seedance, Grok)"** (off by
default), re-lists them when you want them. The guard is not just the hidden
button: the refusal is **server-side, in the core** — with the setting off, a
hidden route is turned away before any provider is contacted, and the message
names the toggle. This is **separate from and additional to** the spend gate
above: the spend gate governs *whether this process may spend at all*, while this
governs *which routes are offered and permitted*, so a hidden route is refused
even in a process that has opted into spending.

**Always build through `tauri build`** (or `scripts/build-macos.sh`), never a
bare `cargo build`. A plain cargo build does not bundle the frontend, and the
resulting app opens to a blank white window.

### Releasing on macOS

```sh
scripts/build-macos.sh          # signs, notarizes, staples both app and dmg
scripts/verify-macos.sh         # proves it is actually installable
```

The build script refuses to proceed with anything other than a Developer ID
Application identity, because a development certificate signs cleanly and is
still rejected by Gatekeeper on someone else's machine. The verify script
quarantines a copy the way a browser download would and asks Gatekeeper the same
question it asks at double-click time — a bundle can pass `codesign --verify`
and still be refused.

## Provenance

Two CI lints protect the line between reimplementing a workflow and copying
someone's work:

- **No third-party media hosts** may appear anywhere in the repo. Every preview
  and sample is generated by us.
- **No verbatim third-party product copy** in strings we ship. Short functional
  labels are unavoidable; anything longer is written by us. This is checked
  against one-way hashes, so no third-party text is ever stored here.

The model catalogue is seeded from `higgsfield-ai/cli`'s `MODELS.md`, which is
MIT licensed and published by its authors as a machine-readable specification.
See `NOTICE`.

## License

AGPL-3.0-or-later. See `LICENSE`, and `NOTICE` for bundled third-party
components.
