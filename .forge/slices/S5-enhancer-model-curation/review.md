# S5-enhancer-model-curation — REVIEW (code review, read-only)

Reviewer scope: the working-tree diff for this slice (uncommitted on branch
`forge/s5-model-curation`), i.e. `git diff HEAD` over `enhancer.rs`,
`commands.rs`, `ui/{types,api,App}.ts(x)`, `EnhancerPicker.tsx` + `.test.tsx`,
`SettingsRail.tsx`. Judged against AC1–AC6 in slice.md. I changed nothing.

## Verification run (read-only)

- `cargo test -p hickeyfield-core enhancer` — 75 passed, 0 failed (incl. the new
  tiering/sort/parse tests).
- `cargo test -p hickeyfield-tauri` — 117 passed, 0 failed (incl.
  `auto_picks_the_top_ranked_model`, `auto_with_only_discouraged_models_still_picks_one`,
  `an_explicit_ollama_choice_with_an_installed_model_runs_local`,
  `a_list_of_installed_models_never_errors`).
- `cargo clippy -p hickeyfield-core` and `-p hickeyfield-tauri` — no warnings/errors.
- `cargo fmt --check` — clean.
- `cd ui && pnpm test` — 12 files, 224 tests passed (EnhancerPicker: 8).
- `./scripts/lint-provenance.py` — PASS (no shipped string matches the reference
  corpus; the genericized copy is original wording).

## What I checked and found clean

- **AC1 `model_tier` precedence (discouraged-first) and the size scan.** Traced
  the flagged tags by hand and against the passing unit tests:
  - `qwen3-coder:30b` (`parameter_size "30.5B"`) → 30.5 ≥ 20 → Discouraged.
  - `gemma3:27b` (size absent) → name scan reads `27b`=27 → Discouraged, and
    size beats the `gemma3` recommend rule because the discouraged block returns
    first (`enhancer.rs:1090`). Correct per AC1.
  - `gemma3n:e2b` → scan reads `e2b`=2.0 (<20), family split at `:` gives
    `gemma3n` which `starts_with("gemma3")` → Recommended. Correct.
  - `llama3.2:3b` → the `3.2` in the family is NOT grabbed as a size because it
    is followed by `:`, not `b`; the trailing `3b`=3 is read → Recommended.
    Correct. The largest-match rule (`parse_param_billions`, `enhancer.rs:1155`)
    keeps `30` from `qwen3-coder:30b` rather than the leading `3`.
  - Vision (`-vl`/`llava`/`vision`) and reasoning (`deepseek-r1`/`qwq`/`-r1`)
    win even at small sizes. Verified `qwen3-vl:4b`, `deepseek-r1:8b`, `qwq`.
- **AC1 `parse_param_billions` robustness.** No panic paths (byte scan, checked
  `parse::<f64>` via `if let Ok`). Case-insensitive on the trailing `b`
  (`eq_ignore_ascii_case`). `"nope"`/missing → `None`. Empty string → `None`.
- **AC2 shape + sort.** `LocalModel{name,tier}` (serde camelCase), `ModelTier`
  (serde lowercase) match the TS union/interface in `types.ts`. Embedding-only
  filter preserved. `sort_by_key(tier_rank)` is stable, so Ollama's intra-tier
  order is preserved; `tier_rank` maps Recommended<Neutral<Discouraged. The
  stable-sort/parameter_size test asserts exactly this.
- **AC3 single source of truth.** Every call site updated to `Vec<LocalModel>`:
  `local_models`, `list_ollama_models` (`commands.rs:802`), `select_rewriter`
  signature + `.first()` auto branch (`commands.rs:781`) + explicit membership
  `m.name == model` (`commands.rs:765`), `api.ts` `ollamaModels`, `App.tsx`
  state, `SettingsRail`/`EnhancerPicker` props. UI consumes the list untouched —
  `ollamaModels[0]?.name` for the default (`EnhancerPicker.tsx:62`) and
  `ollamaModels.map` with no re-sort/filter (`:149`). show==submit guard intact.
  `auto_picks_the_top_ranked_model` uses a fixture that lists the vision model
  FIRST and proves the recommended one is chosen — non-tautological.
- **AC3 honest fallback intact.** `select_rewriter` empty-list branches still
  return the honest `NOTE_*` unavailable rewriters. The enhance-time failure
  path (a model that lists in `/api/tags` but errors on `/api/chat`) is
  untouched and still honest: `enhance_or_original` → non-`Rewritten` status →
  `harness.rs:105` returns the original compiled prompt with a note, never a
  faked success and never a blocked generation. Confirmed as requested.
- **AC4 badges.** `tierSuffix` + `data-tier` on each `<option>`; no option is
  `disabled` (discouraged stays selectable). vitest covers auto-commit of the
  recommended model, commit of a lone discouraged model, and the data-tier/label
  assertions.
- **AC5 copy.** `NOTE_OLLAMA_NO_MODELS` and `EnhancerPicker.tsx:95` genericized
  to the go-to set; the Rust note keeps the exact provenance tail
  "...sent exactly as you wrote it." Provenance lint passes.
- **AC6 regression.** Full core + tauri suites, clippy, fmt, provenance, and the
  UI suite all green (above).
- **Dead code / AI-tells.** `tier_rank` and `tierSuffix` are both used; no
  invented APIs; doc comments match behavior (the `LocalEnhancer`/`HostedEnhancer`
  doc updates accurately describe the new local-ranking-vs-hosted distinction).

## Findings (ranked)

Minor / informational only — none block merge.

1. **Minor — `parse_param_billions` requires `b` immediately after the digits**
   (`enhancer.rs:1148`). A `details.parameter_size` reported with a space
   (`"3.2 B"`) would parse to `None` and fall back to the name scan. Ollama's
   actual format is space-less (`"3.2B"`), so this does not bite in practice.
   Fix if desired: skip optional whitespace before the `b` check.
2. **Minor — largest-match size heuristic can over-read a contrived tag**
   (`enhancer.rs:1155`). A name like `yi34b:9b` would yield 34, not the 9b tag,
   and be flagged large. No such curated/realistic tag exists and the tradeoff
   is documented; noting for awareness only.
3. **Minor — broad substring matches** for `-r1` and `vision`
   (`enhancer.rs:1084-1087`). Only contrived tags would false-positive; the
   real reasoning/vision families are matched correctly. Acceptable.
4. **Note — `parse_local_models` made `pub`** (`enhancer.rs:1035`). This is a
   legitimate test seam: the tauri command test drives the raw-`/api/tags`→ranked
   chain through it (`commands.rs` `auto_picks_the_top_ranked_model`). Not a leak.
5. **Note (pre-existing, out of scope)** — if the currently selected Ollama model
   is uninstalled while other models remain, `EnhancerPicker`'s default-commit
   effect does not re-resolve (its guard only fires when the whole backend goes
   unavailable). This behavior predates S5 and is unchanged by this slice.

Known limitations from the live test (heavy-load 120s timeout; an unloadable
GGUF that name-tiers but errors at enhance-time) are logged as follow-ups and are
not slice failures; the honest-fallback path that catches the latter is confirmed
present above.

VERDICT: PASS
