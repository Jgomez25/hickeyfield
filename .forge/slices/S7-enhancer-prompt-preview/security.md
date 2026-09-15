# S7-enhancer-prompt-preview — SECURITY (proportionate)

Read-only security review of the slice diff. The slice adds a `preview_prompt` command,
a verbatim `final_prompt` submit path, and a UI preview panel. No new dependencies, no
manifest changes, no new network hosts.

## Trust surface — unchanged, no new exposure
`preview_prompt` runs the enhancer on demand via the exact same path `submit_job` already
uses: `probe_enhancer_env` → `select_rewriter` → `harness::compile`. The enhancer targets
are unchanged — local Ollama at `hickeyfield_core::clients::OLLAMA_URL` (`/api/tags` +
chat) and the hosted OpenAI backend via `Rewriter::Hosted`. No new endpoint, host, or
scheme is introduced. This is the same free-Ollama / small-OpenAI surface reviewed in
S4/S5/S6.

## OpenAI key stays server-side — CONFIRMED
On the enhance path `probe_enhancer_env` reads the key with `vault::get(ProviderId::OpenAi,
false)` and passes it only into `Rewriter::Hosted { api_key, .. }` (harness.rs:196-200),
which constructs `HostedEnhancer` for the outbound API call (harness.rs:237-238). The key
is a borrowed `&str` used solely to authorize that HTTP request. `PreviewDto` exposes only
`prompt`, `original`, `enhanced`, `version`, `note` — all derived from rewrite *content*,
decision reasons, or static strings (harness.rs:99-227). The key never enters `enhanced`,
`note`, `version`, or `prompt`, and is never logged by the new code. No new leak path.

## final_prompt is not an injection sink — CONFIRMED
`final_prompt` is the user's own prompt text. On submit it becomes `compiled.prompt` and is
handed to `submit_to_provider` (commands.rs:1025), which sends it as a JSON body field to
fal — not into a shell command, URL path, or SQL. It is the user attacking their own
generation with their own text; there is no privilege boundary to cross. No sanitization
gap of security consequence.

## Billing abuse — structurally impossible — CONFIRMED
`preview_prompt` takes no `State` (commands.rs:928), so it holds no handle to the store,
prices, runner, or fal client and cannot reach `submit_to_provider` (whose only call site
is commands.rs:1025 inside `submit_job`). A preview therefore cannot create a job or incur
a paid generation. The signature is the guarantee; verified by
`preview_returns_a_compiled_prompt_and_creates_no_job`.

## DoS / resource bounds — CONFIRMED
The preview enhancer call inherits the same client timeout as submit's enhance path (the
shared `harness::compile` → enhancer clients, ~120s ceiling) and the same single `/api/tags`
probe. No unbounded loops, no recursion, no user-controlled iteration count. `final_prompt`
is bounded by normal prompt handling. Retry is a user-initiated single re-call, gated in
the UI by `previewing`/`pending` (button disabled while a fetch is in flight) — no client
hammering primitive. A preview cannot wedge the app or exhaust resources beyond what the
existing enhance path already permits.

## Dependencies / manifest — CONFIRMED unchanged
`git diff` shows no changes to any `Cargo.toml`, `Cargo.lock`, `package.json`, or
`pnpm-lock.yaml`. No new crate or npm package. `deny.toml`/provenance surface untouched.

## Verdict rationale
No new secrets exposure, no injection sink, no billing bypass, no DoS primitive, no new
dependency or host. The preview reuses an already-reviewed trust surface and adds a strict
subset of submit's capabilities (compile only, no State). Nothing on a critical path is
unresolved.

VERDICT: PASS
