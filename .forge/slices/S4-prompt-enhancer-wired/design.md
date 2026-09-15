# S4-prompt-enhancer-wired — DESIGN

Upstream read: `.forge/objective.md` (§ "prompt-rewrite LLM wiring" is called out-of-scope for
the earlier contract but this slice is the backlog item that lands it), this slice's `slice.md`
(AC1–AC5, the verified context, the Anthropic wrinkle), the S2 `design.md`/`document.md` (house
format + the `has_adapter`/`Local` honesty precedent this slice must not undo), and
`docs/HARNESS-WIRING.md` (the 2026-08-05 read-only wiring spec; **superseded in part** — the
rewriter now lives in `enhancer.rs` as `LocalEnhancer`/`HostedEnhancer`, not the proposed
`rewrite.rs`, and `submit_job` already compiles through `harness::compile`). Where this design
and HARNESS-WIRING disagree, the code as it stands today wins and is cited.

## Problem, restated from the code

Everything downstream of the UI is already built and wired *except one hop*:

- `harness::compile` (`src-tauri/src/harness.rs:58-177`) runs the three enhance rules, and when
  `decision.enhance` is true it constructs a `LocalEnhancer`, calls `enhance_or_original`, and
  recompiles with the camera clause (`harness.rs:135-166`). Its `Rewriter` enum
  (`harness.rs:50-55`) has only `None` and `Ollama { model }`. There is **no variant that reaches
  `HostedEnhancer`**, even though `HostedEnhancer` and `HostedBackend::{OpenAi,Anthropic}` are
  complete and tested (`crates/hickeyfield-core/src/enhancer.rs:1051-1281`).
- `submit_job` (`commands.rs:700-788`) already threads `compiled.enhanced`/`compiled.version`/
  `compiled.note` onto the `JobSet` (`commands.rs:752-754`) and sends `&compiled.prompt` to the
  provider (`commands.rs:772`). It maps `SubmitInput.rewriter: Option<String>`
  (`commands.rs:695-697`) to `Rewriter::Ollama` (`commands.rs:722-725`).
- The **UI never sends `rewriter`.** `SubmitInput` in `ui/src/types.ts:211-218` has no `rewriter`
  field, and `submitJob` (`ui/src/api.ts:579-598`) posts only the fields it knows. So every real
  submission hits `Rewriter::None`, the `let Rewriter::Ollama … else` at `harness.rs:109` returns
  early, and the prompt is "sent as written" — the dead enhancer the user hit with "a banana with
  a smile".

So the fix is three connected edits: (1) a `Rewriter` variant that reaches `HostedEnhancer` and a
backend-selection step in `submit_job`; (2) a command that lists installed Ollama models; (3) a UI
control that lets the user pick a backend + model and sends it. The core rewrite engine, the note
plumbing, the persistence, and the send path are all already correct.

## Owner decision, and the Anthropic wrinkle

**RECOMMENDED SCOPE: ship OpenAI + Ollama now, DEFER hosted Anthropic.** Keep the `Rewriter` and
selection extensible so Anthropic drops in later with no reshaping.

Why defer Anthropic (the wrinkle, resolved):

- **OpenAI is nearly free to reach.** `ProviderId::OpenAi` already exists (`provider.rs:21`), is in
  `ProviderId::ALL` (`provider.rs:38`), has a keychain slot (`vault` keys by `ProviderId::slug`,
  `vault.rs:16-22`), appears in onboarding/Settings, and its key is readable with
  `vault::get(ProviderId::OpenAi, false)`. Crucially `OpenAi.has_adapter()` is **false**
  (`provider.rs:96-98` → `Fal | Higgsfield`), so an OpenAI key in `configured_providers()` does
  **not** make `route::resolve` believe the user can run OpenAI *image* models — the S8.3 concern in
  HARNESS-WIRING is already neutralised by the S2 `has_adapter` correction. Reusing the existing
  slot is the thin path.
- **Anthropic is not thin.** `ProviderId` has no `Anthropic` (`provider.rs:16-31`), so there is no
  keychain account, no Settings entry, and no way for a user to *enter* a key. Adding one is not one
  line: `ProviderId::ALL` grows 9→10 (`provider.rs:34`), and `slug`/`display_name`/`needs_key`/
  `needs_secret`/`from_slug` plus the **exhaustive `const fn features()` match** (`provider.rs:126+`)
  each need an arm, plus `ProviderInfo`/onboarding copy and `validate.rs`. That is a keychain-slot
  slice of its own, and the slice text explicitly says "favor OpenAI + Ollama now unless adding
  Anthropic is trivial." It is not trivial.
- **Extensibility is preserved for free.** `HostedBackend::Anthropic` already exists
  (`enhancer.rs:1052-1057`) and `HostedEnhancer` already speaks `/v1/messages`
  (`enhancer.rs:1129-1137`, `anthropic_body`/`anthropic_text` at `:1196-1232`). The `Rewriter::Hosted`
  variant below carries a `HostedBackend`, so the day `ProviderId::Anthropic` + its vault slot land,
  the only new code is one arm in `select_rewriter` reading that key. Nothing about the enum, the
  compile branch, or the DTO changes.

Open question **Q-ANTHROPIC** (default recorded): defer. If the verifier/owner rules Anthropic
in-scope, it is a strictly additive follow-on and does not invalidate anything here.

---

## AC1 — a `Rewriter` variant that reaches `HostedEnhancer`

### The enum (`harness.rs:50-55`)

Additive; existing `None`/`Ollama` and all their tests are untouched.

```rust
pub enum Rewriter<'a> {
    None,                                    // unchanged
    Ollama { model: &'a str },               // unchanged
    /// The user's own hosted key. `backend` selects the wire dialect; the key
    /// is read from the vault by the shell and borrowed in, never stored here.
    Hosted { backend: HostedBackend, api_key: &'a str, model: &'a str },
    /// Enhance was wanted but no backend is available. Carries the honest note
    /// authored in commands.rs (AC2). compile() surfaces it only if the three
    /// rules leave enhancement ON, so it never masks the end-frame reason.
    Unavailable { note: String },
}
```

`HostedBackend` is `hickeyfield_core::enhancer::HostedBackend` (re-export it in `harness.rs`'s `use`
alongside the existing `enhancer::{…}` import at `harness.rs:27-30`).

### The compile branch (mirror the Ollama branch, `harness.rs:135-166`)

The rewrite tail — build `EnhanceRequest`, `mode_for`, `corpus::system_prompt_for`,
`enhance_or_original`, and recompile with the camera clause — is **identical for Local and Hosted**;
only the concrete `Enhancer` differs and the `recipe_pin` provider/model pair differs. So:

1. Refactor the current early-return match (`harness.rs:109-117`) into a small dispatch that yields
   the concrete enhancer and its pin, or an early `Compiled` for `None`/`Unavailable`:
   - `Rewriter::None` → return `compiled_now` with `note: Some("no rewriter selected — sent as
     written")` (existing string, `harness.rs:115`).
   - `Rewriter::Unavailable { note }` → return `compiled_now` with `note: Some(note)`.
   - `Rewriter::Ollama { model }` → enhancer `LocalEnhancer::new(model, system)`
     (`enhancer.rs:852`), pin `("ollama", model)`.
   - `Rewriter::Hosted { backend, api_key, model }` → enhancer
     `HostedEnhancer::new(backend, api_key, model, system)` (`enhancer.rs:1088`), pin
     `(backend.slug(), model)` (`HostedBackend::slug`, `enhancer.rs:1060`).
2. Extract the shared tail (currently `harness.rs:147-176`) into a crate-internal helper:

```rust
// harness.rs, module-private. `parts` still carries the camera clause; `pin`
// is the (provider, model) pair for recipe_pin.
fn finish_rewrite(
    enhancer: &dyn hickeyfield_core::enhancer::Enhancer,
    req: &EnhanceRequest, parts: &PromptParts, mode: Mode,
    pin: (&str, &str), original: String, compiled_now: String,
) -> Compiled
```

   `finish_rewrite` runs `enhance_or_original(enhancer, req)` (`enhancer.rs:510`), and on
   `RewriteStatus::Rewritten` rebuilds `PromptParts { scene: out.prompt, ..parts.clone() }.compile()`
   and sets `version: Some(recipe_pin(corpus::CORPUS_ID, mode, Some(pin)))` (exactly the existing
   logic at `harness.rs:154-165`). This is the seam AC1's test drives.

Both branches keep the `mode_for`-returns-`None` guard (`harness.rs:135-145`, "no enhancer guidance
exists for this kind of model") and the `EnhanceRequest` construction that passes **`raw_prompt`,
not the compiled string** (`harness.rs:123-133`) — the reason the camera clause is never mangled.

### AC1 test — proving compile() with a hosted rewriter enhances (HTTP mocked)

`HostedEnhancer` currently hardcodes `OPENAI_CHAT_URL`/`ANTHROPIC_MESSAGES_URL`
(`enhancer.rs:108-109`, used at `:1123`/`:1131`) with **no override** — unlike `LocalEnhancer`,
which has `with_base_url` (`enhancer.rs:860-863`). This is the single obstacle to a real
HTTP-level hosted test. **This is the riskiest part of the slice — see the flag at the end.**

Recommended enabling change (small, symmetric, additive): give `HostedEnhancer` a
`with_base_url(url)` and route `call()` (`enhancer.rs:1118-1139`) through per-backend paths on that
base (default = today's two consts, so every existing hosted test at `enhancer.rs:1936-2041` is
unchanged). Then AC1's test is real, with **no new test dependency**:

- Spin a `std::net::TcpListener` on `127.0.0.1:0` in a thread, accept one connection, and write a
  canned OpenAI chat-completion body (`{"choices":[{"finish_reason":"stop","message":{"content":
  "A single banana with a wide grin, cinematic key light, shallow depth of field","refusal":null}}]}`
  — the exact shape `openai_text` reads, `enhancer.rs:1162-1189`).
- In `harness.rs` tests: build `HostedEnhancer::openai("sk-test","gpt-test",system).with_base_url(addr)`
  and pass it through `finish_rewrite` with a `push-in` camera preset; assert `out.enhanced` is
  `Some(...)`, `out.prompt` **contains the rendered camera template** (proving recompile) and the
  rewritten scene, `out.version == recipe_pin(CORPUS_ID, Mode::Image, Some(("openai","gpt-test")))`,
  and `out.note.is_none()`.
- A second test drives the **construction/dispatch** arm end-to-end: `compile(model, "a banana with
  a smile", None, &[], true, Rewriter::Hosted { backend: OpenAi, api_key: "", model: "gpt-x" })`
  with an empty key → `HostedEnhancer` refuses (`enhancer.rs:1248-1257`), and compile returns the
  original prompt with a non-empty `note` and `enhanced.is_none()` — the graceful-fallback contract,
  mirroring the existing Ollama test `a_missing_rewriter_still_produces_a_sendable_prompt`
  (`harness.rs:244-262`).

If the owner rejects touching `HostedEnhancer`, the fallback (recorded, not preferred) is to test
`finish_rewrite` with the module's existing `Stub` `Enhancer` pattern (`enhancer.rs:1299-1308`) —
this proves the harness recompile wiring but not the socket. The `with_base_url` route is strongly
preferred because it literally satisfies "HostedEnhancer HTTP mocked."

Worked example ("a banana with a smile", image model, OpenAI): `mode_for` → `Some(Mode::Image)`
(`enhancer.rs:314-315`); `system_prompt_for(Image)` loads base + `enhancer.image.v1.md`; the mocked
reply replaces the scene; with no preset the compiled prompt equals the rewritten scene; `version`
= `enhancer.v1/image+openai/gpt-test`.

---

## AC2 — backend selection + the honest fallback (in `commands.rs`)

### `SubmitInput.rewriter` becomes structured

`rewriter: Option<String>` (`commands.rs:695-697`) is replaced by an optional structured choice.
Safe to reshape: the UI does not send it today and no Rust caller depends on the string form.

```rust
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RewriterChoice {
    pub backend: String,        // "ollama" | "openai"
    pub model: Option<String>,  // required for ollama; the hosted model id for openai
}
// on SubmitInput:  #[serde(default)] pub rewriter: Option<RewriterChoice>,
```

### The selection function

Lives in `commands.rs` and is called by `submit_job` just before `harness::compile`
(replacing the inline match at `commands.rs:722-725`). Pure w.r.t. its inputs so it is unit-testable
without a Tauri `State`:

```rust
// Inputs are all data, so the test passes them directly:
//   choice:   the UI's explicit RewriterChoice (None = "auto / decide for me")
//   openai:   whether an OpenAI key is stored  (vault::get(OpenAi,false).is_some())
//   ollama:   detect_local().ollama            (clients.rs:691)
//   models:   installed Ollama tags, or Err if the daemon could not be asked
fn select_rewriter<'a>(
    choice: Option<&'a RewriterChoice>,
    openai_key: Option<&'a str>,
    ollama_up: bool,
    ollama_models: &[String],
) -> Rewriter<'a>
```

Priority (explicit choice wins; else auto; else honest note):

1. **Explicit `openai`** → if `openai_key` present and `choice.model` set, `Rewriter::Hosted {
   OpenAi, key, model }`; else `Rewriter::Unavailable` with the matching note.
2. **Explicit `ollama`** → if `ollama_up` and `choice.model` set (and, defensively, in
   `ollama_models`), `Rewriter::Ollama { model }`; else `Unavailable`.
3. **No choice (auto)** — the order the slice specifies: OpenAI key present → *(see Q-AUTO-MODEL
   below; default emits a "pick a model" note rather than inventing a hosted model id)*; else
   `ollama_up && !models.is_empty()` → `Rewriter::Ollama { first model }`; else `Unavailable`.
4. **Enhance off**: `submit_job` passes `Rewriter::None` regardless of the above — the
   `input.settings.enhance` bool already gates this and `compile`'s three rules own the final say.

`submit_job` reads the OpenAI key via `vault::get(ProviderId::OpenAi, false)` (`vault.rs:44`),
`ollama_up` via `detect_local().ollama` (already imported, `commands.rs:11`), and the model list via
the new command's inner function (below). The key `String` is bound in a local so
`Rewriter::Hosted { api_key: &key, .. }` borrows it for the `compile` call.

### The honest note strings (authored here; provenance-safe, original wording)

Carried on `Rewriter::Unavailable { note }` and surfaced by `compile` only when enhancement stays
on. Exact strings:

- **Nothing available** (no OpenAI key, Ollama not running):
  `"Enhance is on, but there is no rewriter to run it: no OpenAI key is stored and Ollama isn't running on this machine. Add an OpenAI key in Settings, or start Ollama and pick a model. Your prompt was sent exactly as you wrote it."`
- **Ollama up, no models installed:**
  `"Enhance is on and Ollama is running, but no models are installed yet — run `ollama pull gemma3:1b` (or any chat model), then choose it here. Your prompt was sent exactly as you wrote it."`
- **OpenAI chosen but no key stored:**
  `"Enhance is on and OpenAI is selected, but no OpenAI key is stored — add one in Settings, or switch the enhancer to Local. Your prompt was sent exactly as you wrote it."`
- **A backend chosen but no model picked:**
  `"Enhance is on, but no model was chosen for the enhancer — pick one next to the Enhance switch. Your prompt was sent exactly as you wrote it."`

These end with the same clause the honest-fallback family already uses in spirit ("sent as
written", `harness.rs:115`) and are original technical prose about this app, so
`scripts/lint-provenance.py` (≥3 five-word shingles vs. the Higgsfield corpus; no CDN hosts) cannot
trip on them.

### AC2 test (Rust, `commands.rs` tests)

`select_rewriter` unit tests, one per branch: explicit-openai-with-key → `Hosted{OpenAi}`;
explicit-ollama-with-model-installed → `Ollama`; auto with only Ollama+models → `Ollama{first}`;
auto with nothing → `Unavailable` whose note contains "no OpenAI key" and "Ollama isn't running";
ollama-up-no-models → note contains "no models are installed". Plus a `submit_job`-level test (stub
provider client, like the existing submit tests) asserting that with `enhance:true` and no backend,
the persisted job's `enhance_note` is the honest string and `enhanced_prompt`/`enhancer_version` are
`None`.

---

## AC3 — list-Ollama-models command + UI control + `SubmitInput` carrying the choice

### The command (reuses core; no new clients.rs function)

The core list function **already exists**: `enhancer::local_models(base_url) -> Result<Vec<String>,
String>` (`enhancer.rs:1014-1018`) with the embedding-model filter in `parse_local_models`
(`:1021-1033`, tested `:1908-1932`). `clients.rs::detect_local` only returns a bool
(`clients.rs:691-709`), so **no new clients.rs code is needed** — the slice's clients.rs pointer is
satisfied by the existing `enhancer::local_models`. Add a thin Tauri command:

```rust
#[tauri::command]
pub fn list_ollama_models() -> Vec<String>   // Err → empty list; the UI shows the honest reason
```

It calls `hickeyfield_core::enhancer::local_models(hickeyfield_core::clients::OLLAMA_URL)` and maps
`Err`/unreachable to `Vec::new()` (a running-but-empty Ollama and a down Ollama are already
distinguished by `local_endpoints().ollama`, `commands.rs:58-61`, so the command need not encode the
difference). Register in the `invoke_handler!` list (`src-tauri/src/lib.rs:64-88`) next to
`local_endpoints`.

### The UI control (smallest change that satisfies AC3)

A new `ui/src/components/EnhancerPicker.tsx`, rendered inside `SettingsRail` immediately after
`PromptCard` (`SettingsRail.tsx:145-155`), or folded into `PromptCard` beside the existing Enhance
`Toggle` (`PromptCard.tsx:85-92`). The Enhance enable/disable toggle already exists
(`settings.enhance`); this component adds only backend + model selection. It renders:

- **When a backend is available** (OpenAI key present, or Ollama up with ≥1 model): a small backend
  `<select>` (only the available options) and, for Ollama, a model `<select>` populated from
  `list_ollama_models`. For OpenAI it shows a model text input (no hardcoded default — hosted rosters
  churn, mirroring `HostedEnhancer`'s "no default model" doc, `enhancer.rs:1076-1079`).
- **When nothing is available** and `settings.enhance` is on: the honest reason text (the same class
  of message as AC2), **not a dead dropdown** — e.g. "Ollama isn't running and no OpenAI key is
  stored." with a hint to Settings. It is a rendered explanation, matching the app's `RouteDto`
  unavailable-reason philosophy (S2 design, `ModelPicker`).
- It is inert when `settings.enhance` is off.

Availability is derived from data the app already loads: `localEndpoints()` (`api.ts:235-241`, held
in `App.tsx` `local` state, refreshed by `recheckLocal`, `App.tsx:168-170`), `keyStates()`
(`App.tsx` `states`; OpenAI availability = the `openai` entry's `hasKey`), and the new
`ollamaModels()` wrapper. `App.tsx` holds new `enhancer` state `{ backend, model } | null` and
passes it down; selection defaults to Ollama+firstModel when Ollama is the only backend, else null
(auto).

### `SubmitInput` extension (the wire)

- `ui/src/types.ts` (`:211-218`): add `rewriter?: RewriterChoice | null` where
  `interface RewriterChoice { backend: string; model?: string }`.
- `ui/src/api.ts`: add `ollamaModels(): Promise<string[]>` (try `invoke("list_ollama_models")`,
  `catch → []`, matching the `localEndpoints` shape at `:235-241`), and include `rewriter` in the
  `submitJob` payload (`:579-598` — it already forwards the whole `input`, so only the type changes).
- `ui/src/App.tsx`: the `submitJob({ … })` call (`:400-407`) gains `rewriter: enhancer`.
  `DEFAULT_SETTINGS.enhance` stays `true` (`App.tsx:68`) — this slice does not touch the enhance
  default (the `Option<bool>` tri-state from HARNESS-WIRING H4 is a separate concern, out of scope).

### AC3 vitest test

New `ui/src/components/EnhancerPicker.test.tsx`, using the `react-dom/client` `createRoot` render
pattern S2 established in `ui/src/components/ModelPicker.test.tsx` (jsdom, `globals:true`; no React
Testing Library). It asserts: (a) given ollama models `["gemma3:1b","qwen3-vl:4b"]`, the model
options render and selecting one fires `onChange` with `{backend:"ollama",model:"gemma3:1b"}`;
(b) given no backends available and enhance on, the honest reason text renders and there is no
enabled `<select>`; (c) given an OpenAI key available, an OpenAI backend option renders.

---

## AC4 — enhancedPrompt/enhancerVersion populated and surfaced; the live check

### The path already exists — confirmed end to end

- `harness::compile` sets `Compiled.enhanced`/`.version` on a successful rewrite
  (`harness.rs:159-165`).
- `submit_job` copies them onto the `JobSet`: `enhanced_prompt: compiled.enhanced.clone()`,
  `enhancer_version: compiled.version.clone()`, `enhance_note: compiled.note.clone()`
  (`commands.rs:752-754`). These `JobSet` fields already exist (`engine.rs:39,46,50`) and are in the
  store's persisted/`ON CONFLICT` columns (enhanced_prompt is confirmed live by the existing submit
  path). The provider receives `&compiled.prompt` (the enhanced+recompiled string, `commands.rs:772`)
  — **no `app.rs` change is needed.**
- Serde emits snake_case → `RawJobSet` reads `enhanced_prompt`/`enhancer_version`/`enhance_note`
  (`api.ts:360,379,380`) and `toJobSet` maps them to `enhancedPrompt`/`enhancerVersion`/`enhanceNote`
  (`api.ts:437,454,455`); the `JobSet` TS fields exist (`types.ts:184,196,201`) and the meta rail
  already renders them. **The TS side has been ready the whole time; only `rewriter` was missing on
  the way in.**

So AC4 is closed by AC3's `rewriter` wire fix plus AC1/AC2's compile branch. The unit proof: a
`submit_job` test (stub provider) with a mocked-successful enhancer asserts the persisted job's
`enhanced_prompt != prompt` and `enhancer_version.is_some()`.

### The LIVE check (real Ollama on this machine)

An `#[ignore]`d harness test `a_banana_gets_a_cinematic_rewrite_against_a_real_daemon`
(`src-tauri/src/harness.rs` tests), gated on an env var naming the model, run by the human:

```sh
OLLAMA_ENHANCE_MODEL=gemma3:1b \
  cargo test -p hickeyfield --lib harness -- --ignored --nocapture
```

It compiles "a banana with a smile" on an image model with `enhance:true` and
`Rewriter::Ollama { model }`, reading the **real** `prompts/` corpus (the ignored enhancer test at
`enhancer.rs:2135-2178` is the template — it loads `enhancer.v1.md` + the overlay and asserts the
reply is non-empty, sentinel-free, and did not echo the input block). Asserts `out.enhanced` is
`Some`, distinct from the raw prompt, and `out.version` names `ollama/gemma3:1b`. The expanded
cinematic prompt and version pin are recorded verbatim in the TEST evidence.

**Model choice for the live run — recorded caution.** Use a **non-reasoning** model
(`gemma3:1b`, or a `qwen3` run with thinking disabled). `deepseek-r1:8b` emits `<think>…</think>`
blocks and `clean_reply` (`enhancer.rs:718-770`) strips code fences, quotes, and the notes sentinel
but **not** `<think>` tags — a reasoning model's output would leak its chain-of-thought into the
prompt. Fixing `clean_reply` for reasoning models is **out of scope** (Non-goal); the live check
simply picks a model that does not trigger it. `gemma3:1b` also fits the ~51KB corpus inside
`ollama_num_ctx`'s window (`enhancer.rs:893-900`), so the truncation guard
(`check_ollama_read_everything`, `:931-945`) will not refuse it.

---

---

## AC5 — regression gate + files touched + public-API surface

The named tests are in **Tests to add (by file)** below; the full file list and the one core
API change are in **Blast radius** further down. This section is the explicit gate.

**Regression commands (all must stay green, from the objective's protected baseline):**

```sh
cargo test --workspace                                       # 0 failed, >= 740 passed
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo deny check
./scripts/lint-provenance.py                                 # note strings authored to pass
( cd ui && pnpm build )                                      # tsc --noEmit + vite build
( cd ui && pnpm test )                                       # vitest run
# App still builds + launches (not a bare `cargo build`):
scripts/build-macos.sh   # or tauri build → src-tauri/target/release/bundle/Hickeyfield.app
```

Plus the human-run live checks (AC4), recorded in TEST evidence, not part of the automated gate:

```sh
OLLAMA_ENHANCE_MODEL=gemma3:1b \
  cargo test -p hickeyfield --lib harness -- --ignored --nocapture
```

**Files touched — Rust:**
- `src-tauri/src/harness.rs` — `Rewriter` variants + `compile` dispatch + `finish_rewrite` helper + tests.
- `src-tauri/src/commands.rs` — `RewriterChoice`, `select_rewriter`, `submit_job` wiring, `list_ollama_models` + tests.
- `src-tauri/src/lib.rs` — register `list_ollama_models` in `invoke_handler!` (`:64-88`).
- `crates/hickeyfield-core/src/enhancer.rs` — **the one core public-API change:** additive
  `HostedEnhancer::with_base_url` (+ route `call()` through the base); default = today's two consts,
  so every existing hosted test is unchanged. See Q-HOSTED-MOCK / Weakest part #1.

**Files touched — UI:**
- `ui/src/types.ts` — `RewriterChoice` + optional `rewriter` on `SubmitInput` (`:211-218`).
- `ui/src/api.ts` — `ollamaModels()` wrapper + `rewriter` forwarded by `submitJob` (`:579-598`).
- `ui/src/App.tsx` — `enhancer` state + pass into `submitJob` (`:400-407`).
- `ui/src/components/EnhancerPicker.tsx` (new) + `EnhancerPicker.test.tsx` (new); rendered in
  `SettingsRail`.

**No schema/DTO break beyond `SubmitInput.rewriter`** (UI does not send it today), and **no
`ProviderId`/`features()`/onboarding change** (Anthropic deferred).

---

## Tests to add (by file)

- **`src-tauri/src/harness.rs`**: AC1 hosted-success via `finish_rewrite` + mocked socket (camera
  clause survives, version pin correct); AC1 hosted construction/fallback via `compile(...,
  Rewriter::Hosted{ api_key:"" })`; the `#[ignore]` live Ollama banana test (AC4).
- **`crates/hickeyfield-core/src/enhancer.rs`**: if `with_base_url` is added, a test that `call`
  posts to the overridden base for both backends (guards the default-URL behaviour is unchanged for
  the existing hosted tests).
- **`src-tauri/src/commands.rs`**: `select_rewriter` branch tests (AC2); a `submit_job` test that
  `enhance:true` + no backend persists the honest `enhance_note` and leaves `enhanced_prompt` null;
  a `list_ollama_models` smoke test (maps unreachable → empty).
- **`crates/hickeyfield-core/src/clients.rs`**: **N/A — no new function.** `enhancer::local_models`
  already exists and is tested (`enhancer.rs:1908-1932`); the command reuses it. (Recorded, not
  skipped.)
- **`ui/`**: `EnhancerPicker.test.tsx` (AC3, three cases above). Existing `ModelPicker.test.tsx`
  stays green.

## Blast radius / public-API + DTO ripple

- **`SubmitInput.rewriter` shape change** (`Option<String>` → `Option<RewriterChoice>`,
  `commands.rs:695-697`) is the one wire-contract change. Safe: the UI does not send it yet;
  `mock.ts`/`onRerun` do not set it. TS `SubmitInput` (`types.ts:211-218`) gains the optional field.
- **New command `list_ollama_models`** — additive; register in `lib.rs:64-88`.
- **`Rewriter` enum** — additive variants; existing `None`/`Ollama` tests unchanged.
- **`HostedEnhancer::with_base_url`** — additive; existing hosted tests keep the default URLs and are
  untouched. `version()` (`enhancer.rs:1278-1280`) is unchanged, so recorded enhancer-version strings
  are stable.
- **No `JobSet`/store schema change** — `enhanced_prompt`/`enhancer_version`/`enhance_note` already
  exist and persist. **No `app.rs` change** — the send path already sends `compiled.prompt`.
- **No `ProviderId` change** (Anthropic deferred), so `ProviderId::ALL`, `features()`, onboarding,
  and `validate` are untouched — this keeps the S2 `has_adapter` honesty intact.

## Non-goals (recorded)

- Hosted **Anthropic** wiring / a new keychain slot (deferred; extensibility preserved).
- Stripping reasoning-model `<think>` blocks in `clean_reply` (the live check avoids them instead).
- Changing `settings.enhance` to the `Option<bool>` tri-state (HARNESS-WIRING H4) — separate concern.
- An enhance *preview* command / locked-toggle UI (HARNESS-WIRING §8.4 `enhance_preview`) — the note
  after submit is the honest surface for this slice; a live preview is future work.
- Any hosted-LLM token-cost estimate — unknown is not zero; the enhancer line does not quote a price.

## Open questions (each with a recorded default)

- **Q-ANTHROPIC** — add a hosted-Anthropic key slot now, or defer? **DEFAULT: defer** (OpenAI +
  Ollama now; enum/selection already accept Anthropic when its `ProviderId` + vault slot land).
- **Q-AUTO-MODEL** — in auto mode with an OpenAI key present but no model chosen, invent a default
  hosted model id or decline? **DEFAULT: decline with the "pick a model" note** — a hardcoded hosted
  id 404s when the roster moves, contradicting `HostedEnhancer`'s deliberate no-default stance
  (`enhancer.rs:1076-1079`). Auto only ever *runs* Ollama-with-first-installed-model; OpenAI always
  requires an explicit model from the picker.
- **Q-HOSTED-MOCK** — add `HostedEnhancer::with_base_url` for a real HTTP-level AC1 test, or test the
  harness seam with the trait `Stub` only? **DEFAULT: add `with_base_url`** (symmetric with
  `LocalEnhancer`, no new dependency, literally satisfies "HostedEnhancer HTTP mocked").
- **Q-PICKER-HOME** — new `EnhancerPicker` component vs. extending `PromptCard`? **DEFAULT: new
  component** rendered in `SettingsRail`, keeping `PromptCard` focused on the prompt + its two
  toggles.

## Weakest parts (verifier: start here)

1. **AC1's "HostedEnhancer HTTP mocked" vs. the hardcoded URLs.** `HostedEnhancer` has no base-url
   override today (`enhancer.rs:108-109,1118-1139`), and the repo has **no HTTP-mock dependency** —
   every hosted test checks the body/parser functions or uses a trait `Stub`. My design adds a tiny
   `with_base_url` + a std-`TcpListener` stub so the test is real with zero new tooling; if the owner
   rejects touching core, the fallback is the trait-`Stub` seam, which proves the harness recompile
   but not the socket. Confirm which is acceptable — this is the one place the AC text and the
   current code are in tension.
2. **`select_rewriter` auto-mode for OpenAI (Q-AUTO-MODEL).** Declining rather than inventing a
   hosted model id is honest but means auto never silently uses OpenAI. Confirm that matches the
   owner's "auto: OpenAI key present?" intent, or whether a UI-chosen default model is expected.
3. **The live-check model.** `deepseek-r1:8b`'s `<think>` blocks would leak into the prompt because
   `clean_reply` does not strip them. The design routes around this by mandating a non-reasoning
   model for the live run and recording it as a Non-goal; verify that is an acceptable scope line
   rather than a bug this slice must fix.
