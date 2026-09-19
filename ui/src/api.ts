/**
 * The bridge to the Rust core.
 *
 * Every call is lazily imported and falls back to a sane default when the Tauri
 * runtime is absent, so the whole UI runs in a plain browser via `pnpm dev`
 * without the shell. For a UI this size that is a large iteration-speed win.
 */

import type {
  CostEstimate,
  PriceStatus,
  ModelCapabilities,
  GenSettings,
  JobResult,
  JobSet,
  JoinRecord,
  KeyframeRecord,
  LocalModel,
  MediaRef,
  Model,
  PresetFamily,
  PromptPreview,
  RewriterChoice,
  Route,
  RenderOptions,
  RenderProgress,
  Storyboard,
  StoryboardPlan,
  StoryboardRender,
  StoryboardResult,
  SubmitInput,
  UseCase,
} from "./types";
import {
  MOCK_MODELS,
  MOCK_PRESETS,
  mockCancel,
  mockEstimate,
  mockGenerateStoryboard,
  mockListJobs,
  mockPlanStoryboard,
  mockPreview,
  mockSubmit,
  mockSubscribe,
} from "./mock";
import {
  PROVIDER_CATALOG,
  type LocalEndpoints,
  type ProviderInfo,
} from "./lib/providers";
import { planEnvImport } from "./lib/env-import";

async function invoke<T>(
  cmd: string,
  args?: Record<string, unknown>,
): Promise<T> {
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<T>(cmd, args);
}

export interface KeyState {
  provider: string;
  hasKey: boolean;
  hasSecret: boolean;
  needsKey: boolean;
  needsSecret: boolean;
}

/** Wire shape from Rust — serde emits snake_case. */
interface RawKeyState {
  provider: string;
  has_key: boolean;
  has_secret: boolean;
  needs_key: boolean;
  needs_secret: boolean;
}

const toKeyState = (r: RawKeyState): KeyState => ({
  provider: r.provider,
  hasKey: r.has_key,
  hasSecret: r.has_secret,
  needsKey: r.needs_key,
  needsSecret: r.needs_secret,
});

/**
 * Credential presence per provider. Note this only ever returns booleans —
 * the secret itself is never exposed to the webview by design.
 */
export async function keyStates(): Promise<KeyState[]> {
  try {
    const raw = await invoke<RawKeyState[]>("key_states");
    return raw.map(toKeyState);
  } catch {
    return [];
  }
}

/** Store or clear a credential. An empty `value` clears it. */
export async function setKey(
  provider: string,
  value: string,
  secretHalf = false,
): Promise<void> {
  await invoke("set_key", { provider, secretHalf, value });
}

export async function configuredProviders(): Promise<string[]> {
  try {
    return await invoke<string[]>("configured_providers");
  } catch {
    return [];
  }
}

export const isDesktop = (): boolean =>
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

/**
 * Normalise whatever `invoke` rejected with into an `Error`.
 *
 * Tauri rejects with the command's `Err` payload, which for our commands is a
 * bare `String`. Re-throwing that raw gives React an error boundary with no
 * `.message`, so the useful half of a provider rejection — the part naming the
 * missing key or the rejected parameter — disappears on the way to the screen.
 */
export function asError(e: unknown): Error {
  if (e instanceof Error) return e;
  if (typeof e === "string") return new Error(e);
  return new Error(
    typeof e === "object" && e !== null ? JSON.stringify(e) : String(e),
  );
}

// ── Provider setup ─────────────────────────────────────────────────────────

interface RawProviderInfo {
  slug: string;
  display_name?: string;
  displayName?: string;
  needs_key?: boolean;
  needsKey?: boolean;
  needs_secret?: boolean;
  needsSecret?: boolean;
  key_url?: string;
  keyUrl?: string;
  env_names?: string[];
  envNames?: string[];
  blurb?: string;
}

/**
 * Descriptions of every provider: where to get a key, what it unlocks.
 *
 * Falls back to the static catalogue, which is what makes the setup screens
 * render in a browser at all. Fields the Rust side leaves blank fall back
 * individually, so a partial payload degrades field by field rather than
 * throwing the whole entry away.
 */
export async function providerInfo(): Promise<ProviderInfo[]> {
  try {
    const raw = await invoke<RawProviderInfo[]>("provider_info");
    if (!Array.isArray(raw) || raw.length === 0) return PROVIDER_CATALOG;
    return raw.map((r) => {
      const known = PROVIDER_CATALOG.find((p) => p.slug === r.slug);
      return {
        slug: r.slug,
        displayName:
          r.display_name ?? r.displayName ?? known?.displayName ?? r.slug,
        needsKey: r.needs_key ?? r.needsKey ?? known?.needsKey ?? true,
        needsSecret: r.needs_secret ?? r.needsSecret ?? known?.needsSecret ?? false,
        keyUrl: r.key_url ?? r.keyUrl ?? known?.keyUrl ?? "",
        envNames: r.env_names ?? r.envNames ?? known?.envNames ?? [],
        blurb: r.blurb ?? known?.blurb ?? "",
      };
    });
  } catch {
    return PROVIDER_CATALOG;
  }
}

export interface ValidationResult {
  ok: boolean;
  detail: string;
}

/**
 * Ask the core to make a real authenticated call with the stored key.
 *
 * `null` means the check could not run at all — no shell, or a build that
 * predates the command. That is a third state, not a failure: rendering an
 * untestable key as "invalid" would send people re-pasting a key that works.
 */
export async function validateKey(
  provider: string,
): Promise<ValidationResult | null> {
  try {
    const raw = await invoke<{ ok: boolean; detail?: string }>("validate_key", {
      provider,
    });
    return { ok: Boolean(raw?.ok), detail: raw?.detail ?? "" };
  } catch {
    return null;
  }
}

export interface EnvImportResult {
  imported: string[];
  unknown: string[];
}

/**
 * Parse .env-style text and store every credential it recognises.
 *
 * The fallback path is not cosmetic: `set_key` already works, so on a desktop
 * build without `import_env` the TypeScript parser plus per-key writes gives
 * the real feature. Outside the shell nothing can be stored, so we report what
 * *would* be imported — the entire browser path is a simulation anyway.
 */
export async function importEnv(text: string): Promise<EnvImportResult> {
  try {
    const raw = await invoke<{ imported?: string[]; unknown?: string[] }>(
      "import_env",
      { text },
    );
    return { imported: raw?.imported ?? [], unknown: raw?.unknown ?? [] };
  } catch {
    const plan = planEnvImport(text);
    if (!isDesktop()) {
      return { imported: plan.providers, unknown: plan.unknown };
    }
    const stored = new Set<string>();
    const failed: string[] = [];
    for (const a of plan.assignments) {
      try {
        await setKey(a.provider, a.value, a.secretHalf);
        stored.add(a.provider);
      } catch {
        failed.push(`${a.provider}${a.secretHalf ? " (secret)" : ""}`);
      }
    }
    return {
      imported: [...stored],
      unknown: [...plan.unknown, ...failed.map((f) => `${f}: could not store`)],
    };
  }
}

/** Which local inference endpoints are up right now. */
export async function localEndpoints(): Promise<LocalEndpoints> {
  try {
    return await invoke<LocalEndpoints>("local_endpoints");
  } catch {
    return { comfyui: false, ollama: false };
  }
}

/**
 * Chat-capable models the local Ollama has installed, for the enhancer picker,
 * each tagged with Rust's suitability tier and already sorted best-first. An
 * empty list on any failure — a down daemon is an empty dropdown, never a thrown
 * error the picker would have to handle.
 */
export async function ollamaModels(): Promise<LocalModel[]> {
  try {
    return await invoke<LocalModel[]>("list_ollama_models");
  } catch {
    return [];
  }
}

/** Absolute path of the generated-asset library. Null outside the shell. */
export async function libraryRoot(): Promise<string | null> {
  try {
    return await invoke<string>("library_root");
  } catch {
    return null;
  }
}

/**
 * Open a URL in the user's real browser.
 *
 * A bare `<a href>` inside the shell navigates the webview itself, which
 * replaces the app with a login page and offers no way back. The opener plugin
 * hands the URL to the OS instead.
 */
export async function openExternal(url: string): Promise<void> {
  try {
    const { openUrl } = await import("@tauri-apps/plugin-opener");
    await openUrl(url);
  } catch {
    window.open(url, "_blank", "noopener,noreferrer");
  }
}

// ── Wire shapes ────────────────────────────────────────────────────────────
// Hand-written for now. These get replaced by tauri-specta output once the
// commands land; until then the converters are the single place snake_case
// stops, so no component ever has to know the wire spelling.

interface RawRoute {
  id: string;
  provider: string;
  slug: string;
  note?: string | null;
  available?: boolean;
  unavailable_reason?: string | null;
  unavailableReason?: string | null;
  promo_as_of?: string | null;
  promoAsOf?: string | null;
}

/**
 * Rust spells these snake_case. Mapped explicitly rather than passed straight
 * through, because a silent spelling mismatch here reads as "the model has no
 * capabilities" — which is precisely the state that used to make the UI invent
 * its own.
 */
interface RawCapabilities {
  supports_duration?: boolean;
  durations?: number[];
  default_duration?: number | null;
  duration_range?: [number, number] | null;
  supports_resolution?: boolean;
  resolutions?: string[];
  default_resolution?: string | null;
  supports_aspect?: boolean;
  aspects?: string[];
  default_aspect?: string | null;
  audio?: boolean;
  constraints?: string[];
  unsupported_roles?: string[];
  unsupportedRoles?: string[];
}

const toCapabilities = (
  c: RawCapabilities | undefined,
): ModelCapabilities | undefined =>
  c
    ? {
        supportsDuration: c.supports_duration ?? false,
        durations: c.durations ?? [],
        defaultDuration: c.default_duration ?? null,
        durationRange: c.duration_range ?? null,
        supportsResolution: c.supports_resolution ?? false,
        resolutions: c.resolutions ?? [],
        defaultResolution: c.default_resolution ?? null,
        supportsAspect: c.supports_aspect ?? false,
        aspects: c.aspects ?? [],
        defaultAspect: c.default_aspect ?? null,
        audio: c.audio ?? false,
        constraints: c.constraints ?? [],
        unsupportedRoles: c.unsupported_roles ?? c.unsupportedRoles ?? [],
      }
    : undefined;

interface RawModel {
  id: string;
  display_name?: string;
  displayName?: string;
  modality: Model["modality"];
  routes: RawRoute[];
  is_launch?: boolean;
  isLaunch?: boolean;
  subtitle?: string;
  capabilities?: RawCapabilities;
}

interface RawPreset {
  id: string;
  display_name?: string;
  displayName?: string;
  category: string;
  tags?: string[];
  description?: string;
  variants?: { model_id?: string; modelId?: string; preview_url?: string }[];
}

interface RawEstimate {
  usd: number;
  basis?: string;
  minimum_applied?: boolean;
  minimumApplied?: boolean;
}

interface RawJobSet {
  id: string;
  model_id?: string;
  modelId?: string;
  status: string;
  prompt: string;
  enhanced_prompt?: string | null;
  created_at?: string;
  createdAt?: string;
  results?: (JobResult & { poster_url?: string; local_path?: string | null })[];
  /**
   * Rust serialises `route_id` (a bare string). `route` is accepted too so a
   * future richer payload works without a flag day — but reading only `route`
   * left `job.route` undefined on every persisted job, which silently broke
   * Rerun's route restore.
   */
  route?: RawRoute | string;
  route_id?: string;
  routeId?: string;
  estimated_usd?: number | null;
  actual_usd?: number | null;
  preset_id?: string | null;
  preset_name?: string | null;
  settings?: GenSettings;
  media?: MediaRef[];
  enhancer_version?: string | null;
  enhance_note?: string | null;
  advisories?: string[];
  fail_reason?: string | null;
  storyboard_id?: string | null;
  storyboardId?: string | null;
  shot_index?: number | null;
  shotIndex?: number | null;
}

const toModel = (r: RawModel): Model => ({
  id: r.id,
  displayName: r.display_name ?? r.displayName ?? r.id,
  modality: r.modality,
  routes: (r.routes ?? []).map(toRoute),
  isLaunch: r.is_launch ?? r.isLaunch ?? false,
  subtitle: r.subtitle,
  capabilities: toCapabilities(r.capabilities),
});

const toPreset = (r: RawPreset): PresetFamily => ({
  id: r.id,
  displayName: r.display_name ?? r.displayName ?? r.id,
  category: r.category,
  tags: r.tags ?? [],
  description: r.description ?? "",
  variants: r.variants?.map((v) => ({
    modelId: v.model_id ?? v.modelId ?? "",
    previewUrl: v.preview_url,
  })),
});

/**
 * `route` may arrive as a bare id while the resolver is still being wired on
 * the Rust side. A rail that shows "fal:kling-video/v3" is worse than one that
 * shows the parsed pair, but far better than one that renders "[object
 * Object]" or crashes.
 */
function toRoute(raw: RawRoute | string): Route {
  if (typeof raw !== "string") {
    return {
      id: raw.id,
      provider: raw.provider,
      slug: raw.slug,
      note: raw.note ?? undefined,
      // Absent means "the shell did not say", which for a route we are being
      // handed is far likelier to be usable than not. A default of false would
      // grey out every route on an older build.
      available: raw.available ?? true,
      unavailableReason:
        raw.unavailable_reason ?? raw.unavailableReason ?? undefined,
      promoAsOf: raw.promo_as_of ?? raw.promoAsOf ?? undefined,
    };
  }
  const [provider, ...rest] = raw.split(":");
  return { id: raw, provider, slug: rest.join(":"), available: true };
}

const toJobSet = (r: RawJobSet): JobSet => ({
  id: r.id,
  modelId: r.model_id ?? r.modelId ?? "",
  status: r.status,
  prompt: r.prompt,
  enhancedPrompt: r.enhanced_prompt ?? null,
  createdAt: r.created_at ?? r.createdAt ?? new Date().toISOString(),
  results: (r.results ?? []).map((res) => ({
    url: res.url,
    kind: res.kind,
    width: res.width,
    height: res.height,
    poster: res.poster ?? res.poster_url,
    localPath: res.local_path ?? res.localPath ?? null,
  })),
  route: toRoute(r.route ?? r.route_id ?? r.routeId ?? ""),
  estimatedUsd: r.estimated_usd ?? null,
  actualUsd: r.actual_usd ?? null,
  presetId: r.preset_id ?? null,
  presetName: r.preset_name ?? null,
  settings: r.settings,
  media: r.media ?? [],
  enhancerVersion: r.enhancer_version ?? null,
  enhanceNote: r.enhance_note ?? null,
  advisories: r.advisories ?? [],
  failReason: r.fail_reason ?? null,
  storyboardId: r.storyboard_id ?? r.storyboardId ?? null,
  shotIndex: r.shot_index ?? r.shotIndex ?? null,
});

// ── Generator bridge ───────────────────────────────────────────────────────

export async function listModels(): Promise<Model[]> {
  try {
    const raw = await invoke<RawModel[]>("list_models");
    return raw.map(toModel);
  } catch {
    return MOCK_MODELS;
  }
}

/**
 * Every model's id → display name, UNFILTERED (hidden routes included).
 *
 * History must render a job on a never-used (hidden) model by name even while
 * that model is absent from the picker feeds. Off the desktop, folds the mock
 * roster so the browser build resolves names too.
 */
export async function modelNames(): Promise<Record<string, string>> {
  try {
    const raw = await invoke<{ id: string; display_name: string }[]>(
      "model_names",
    );
    const map: Record<string, string> = {};
    for (const m of raw) map[m.id] = m.display_name;
    return map;
  } catch {
    const map: Record<string, string> = {};
    for (const m of MOCK_MODELS) map[m.id] = m.displayName;
    return map;
  }
}

/** The persisted "show never-used routes" toggle (default OFF). */
export async function getShowNeverUsed(): Promise<boolean> {
  try {
    return await invoke<boolean>("get_show_never_used");
  } catch {
    return localStorage.getItem("hickeyfield.showNeverUsed") === "1";
  }
}

/** Persist the "show never-used routes" toggle. */
export async function setShowNeverUsed(on: boolean): Promise<void> {
  try {
    await invoke("set_show_never_used", { on });
  } catch {
    localStorage.setItem("hickeyfield.showNeverUsed", on ? "1" : "0");
  }
}

export async function listPresets(): Promise<PresetFamily[]> {
  try {
    const raw = await invoke<RawPreset[]>("list_presets");
    return raw.map(toPreset);
  } catch {
    return MOCK_PRESETS;
  }
}

/**
 * A null return is a real answer, not a failure: the provider publishes no
 * price for this call. Callers must render that as "price unavailable" and
 * never as free — see lib/cost.ts.
 */
export async function estimateCost(args: {
  modelId: string;
  routeId: string;
  settings: GenSettings;
}): Promise<CostEstimate | null> {
  try {
    const raw = await invoke<RawEstimate | null>("estimate_cost", args);
    if (!raw) return null;
    return {
      usd: raw.usd,
      basis: raw.basis ?? "",
      minimumApplied: raw.minimum_applied ?? raw.minimumApplied,
    };
  } catch (e) {
    // Only the absence of the shell justifies a fabricated number. A command
    // that ran and failed must surface, or the Generate button quotes a price
    // no provider agreed to.
    if (!isDesktop()) return mockEstimate(args.modelId, args.settings);
    throw asError(e);
  }
}

/**
 * Where the displayed prices came from — feed age and whether it is live. Fed
 * to the spend meter so the bundled snapshot is never shown as "current".
 * Falls back to a bundled-looking status off the desktop, matching what the
 * shell returns before its first live fetch.
 */
export async function priceStatus(): Promise<PriceStatus> {
  try {
    const raw = await invoke<{
      routes: number;
      ageSeconds?: number | null;
      live: boolean;
    }>("price_status");
    return {
      routes: raw.routes,
      ageSeconds: raw.ageSeconds ?? null,
      live: raw.live,
    };
  } catch (e) {
    if (!isDesktop()) return { routes: 0, ageSeconds: null, live: false };
    throw asError(e);
  }
}

/**
 * Authoritative capabilities for one model, asked at selection time.
 *
 * Deliberately not part of `listModels`: answering it properly means asking fal
 * for the endpoint schema, and doing that for all 68 models to draw a picker
 * would be 68 blocking requests. Returns `null` when we could not find out —
 * the caller must keep the catalogue's answer rather than blanking the chips.
 */
export async function modelCapabilities(
  modelId: string,
  routeId: string | null,
  hasMedia: boolean,
): Promise<ModelCapabilities | null> {
  try {
    const raw = await invoke<RawCapabilities>("model_capabilities", {
      modelId,
      routeId,
      hasMedia,
    });
    return toCapabilities(raw) ?? null;
  } catch {
    return null;
  }
}

/** The workspace tabs, and the media slots each one needs. */
export async function listUseCases(): Promise<UseCase[]> {
  try {
    const raw = await invoke<
      {
        slug: string;
        label: string;
        blurb: string;
        slots: [string, boolean][];
        requires_media?: boolean;
        requiresMedia?: boolean;
      }[]
    >("list_use_cases");
    return raw.map((u) => ({
      slug: u.slug,
      label: u.label,
      blurb: u.blurb,
      slots: u.slots ?? [],
      requiresMedia: u.requires_media ?? u.requiresMedia ?? false,
    }));
  } catch {
    return [];
  }
}

/**
 * Models that can do one job.
 *
 * Filtered in Rust, not here: the picker must not be able to offer something
 * the submit path would refuse.
 */
export async function modelsForUseCase(useCase: string): Promise<Model[]> {
  try {
    const raw = await invoke<RawModel[]>("models_for_use_case", { useCase });
    return raw.map(toModel);
  } catch {
    return MOCK_MODELS;
  }
}

/**
 * What this prompt leaves unsaid, for this job and these attachments.
 *
 * Deterministic and instant in Rust — a question that appears on one run and
 * not the next would be worse than none.
 */

export async function submitJob(input: SubmitInput): Promise<string> {
  try {
    // Wrapped in `{ input }`, not spread. Tauri matches command parameters by
    // name, and `submit_job` takes a single `input` argument — spreading the
    // fields gave "command submit_job missing required key input".
    const res = await invoke<{ job_set_id?: string; jobSetId?: string }>(
      "submit_job",
      { input } as unknown as Record<string, unknown>,
    );
    return res.job_set_id ?? res.jobSetId ?? "";
  } catch (e) {
    // The bug this replaces: `invoke` rejects both when Tauri is absent and
    // when the Rust command returns Err, and a bare catch cannot tell them
    // apart — so in the shipped binary a real provider rejection produced a
    // synthetic job that marched to "completed" with an invented seed and an
    // invented price. Never fabricate a generation inside the app.
    if (!isDesktop()) return mockSubmit(input);
    throw asError(e);
  }
}

/**
 * Compile the prompt for display WITHOUT submitting anything.
 *
 * Wrapped in `{ input }` exactly like {@link submitJob} — Tauri matches command
 * parameters by name, and `preview_prompt` takes a single `input`. The DTO is
 * already camelCase, but both spellings are accepted defensively so a rename on
 * either side degrades to a missing field, not a crash. Runs no route, price or
 * generation: it exists so the user can see (and edit) the enhancer's output
 * before spending money.
 */
export async function previewPrompt(input: SubmitInput): Promise<PromptPreview> {
  try {
    const raw = await invoke<Record<string, unknown>>("preview_prompt", {
      input,
    } as unknown as Record<string, unknown>);
    const str = (v: unknown): string | null =>
      typeof v === "string" ? v : null;
    return {
      prompt: str(raw.prompt) ?? "",
      original: str(raw.original) ?? input.prompt,
      enhanced: str(raw.enhanced),
      version: str(raw.version),
      note: str(raw.note),
    };
  } catch (e) {
    if (!isDesktop()) return mockPreview(input);
    throw asError(e);
  }
}

/**
 * Draft a model-aware shot list from one brief.
 *
 * Runs no route, price or generation — like {@link previewPrompt} it takes no
 * store handle on the Rust side, so drafting cannot bill a generation. A
 * rejected draft (unavailable backend, refusal, bad plan) rejects with the
 * honest reason; the caller shows it rather than fabricating a plan. Off the
 * desktop it returns a canned plan so the view is usable in a browser.
 */
export async function planStoryboard(args: {
  brief: string;
  targetSeconds: number;
  useCase: string;
  modelPrefs: string[];
  rewriter?: RewriterChoice | null;
}): Promise<StoryboardPlan> {
  try {
    return await invoke<StoryboardPlan>("plan_storyboard", args);
  } catch (e) {
    if (!isDesktop()) return mockPlanStoryboard(args);
    throw asError(e);
  }
}

/**
 * Generate every shot as one linked set.
 *
 * Each shot goes through the same paid submit path a single Generate uses — the
 * total is shown before this is ever called (the view disables Generate-all
 * until the summed estimate renders). Returns a per-shot outcome; a failed shot
 * carries its error and the batch continues.
 */
export async function generateStoryboard(input: {
  brief: string;
  targetSeconds: number;
  planJson: string;
  shots: SubmitInput[];
  /** The plan index each submitted shot maps to. Omitted on the first
   * generate-all (the shots are the whole plan in order); on a retry it carries
   * the original indices of the failed shots. */
  shotIndices?: number[];
  /** An existing storyboard to write a retry into, so retried shots join the
   * original group instead of starting a new one. */
  storyboardId?: string;
  /** S15: submit the shots as a chain (each continues from the previous shot's
   * last frame) rather than all at once. Only shot 0 is submitted now; the
   * runner drives the rest as each predecessor settles. */
  chained?: boolean;
  /** S16 D9: opt into bounded auto-retry of a below-floor continuity join (at
   * most 2 regenerations, each priced first). Off by default. */
  autoRetry?: boolean;
  /** S17: generate in keyframe-anchored mode — render one still per shot
   * boundary and anchor every shot on both ends. Off by default. */
  keyframeMode?: boolean;
  /** S17: the image model that renders the stills (defaults to qwen_image_3). */
  stillModelId?: string;
}): Promise<StoryboardResult> {
  try {
    return await invoke<StoryboardResult>("generate_storyboard", {
      input,
    } as unknown as Record<string, unknown>);
  } catch (e) {
    if (!isDesktop()) return mockGenerateStoryboard(input);
    throw asError(e);
  }
}

/**
 * S15: render a completed storyboard set into one stitched ad on disk.
 *
 * Spends nothing — it runs the bundled ffmpeg sidecar on the already-downloaded
 * clips locally. Returns the finished render (path, dimensions, duration, and
 * the set's Σ actual cost). Progress arrives on the `storyboard:render` event.
 */
export async function renderStoryboard(args: {
  storyboardId: string;
  options?: RenderOptions;
}): Promise<StoryboardRender> {
  const raw = await invoke<RawStoryboardRender>("render_storyboard", {
    storyboardId: args.storyboardId,
    options: args.options ?? {},
  } as unknown as Record<string, unknown>);
  return toStoryboardRender(raw);
}

/** S15: the finished renders, for the Library's set items. */
export async function listStoryboardRenders(): Promise<StoryboardRender[]> {
  try {
    const raw = await invoke<RawStoryboardRender[]>("list_storyboard_renders");
    return raw.map(toStoryboardRender);
  } catch {
    return [];
  }
}

interface RawStoryboardRender {
  storyboard_id?: string;
  storyboardId?: string;
  output_path?: string;
  outputPath?: string;
  width?: number | null;
  height?: number | null;
  seconds?: number | null;
  actual_usd?: number | null;
  actualUsd?: number | null;
  created_at?: number;
  createdAt?: number;
}

function toStoryboardRender(r: RawStoryboardRender): StoryboardRender {
  return {
    storyboardId: r.storyboard_id ?? r.storyboardId ?? "",
    outputPath: r.output_path ?? r.outputPath ?? "",
    width: r.width ?? null,
    height: r.height ?? null,
    seconds: r.seconds ?? null,
    actualUsd: r.actual_usd ?? r.actualUsd ?? null,
    createdAt: r.created_at ?? r.createdAt ?? 0,
  };
}

/** The stored storyboards, for the Library's group headers. */
export async function listStoryboards(): Promise<Storyboard[]> {
  try {
    const raw = await invoke<
      {
        id: string;
        brief: string;
        target_seconds?: number;
        targetSeconds?: number;
        plan_json?: string;
        planJson?: string;
        created_at?: number;
        createdAt?: number;
        joins_json?: string | null;
        joinsJson?: string | null;
        auto_retry?: boolean;
        autoRetry?: boolean;
        keyframes_json?: string | null;
        keyframesJson?: string | null;
      }[]
    >("list_storyboards");
    return raw.map((s) => {
      const joinsJson = s.joins_json ?? s.joinsJson ?? null;
      let joins: JoinRecord[] = [];
      if (joinsJson) {
        try {
          joins = parseJoinRecords(joinsJson);
        } catch {
          joins = [];
        }
      }
      const keyframesJson = s.keyframes_json ?? s.keyframesJson ?? null;
      let keyframes: KeyframeRecord[] = [];
      if (keyframesJson) {
        try {
          keyframes = parseKeyframeRecords(keyframesJson);
        } catch {
          keyframes = [];
        }
      }
      return {
        id: s.id,
        brief: s.brief,
        targetSeconds: s.target_seconds ?? s.targetSeconds ?? 0,
        planJson: s.plan_json ?? s.planJson ?? "{}",
        createdAt: s.created_at ?? s.createdAt ?? 0,
        joins,
        autoRetry: s.auto_retry ?? s.autoRetry ?? false,
        keyframes,
      };
    });
  } catch {
    return [];
  }
}

/**
 * Parse a set's `joins_json` (snake-case from Rust) into camelCase `JoinRecord`s.
 *
 * The S16 cut fields (`ssim`, `psnr`, `dy`, `du`, `dv`, `pass`, `applied`,
 * `kind`, `index`, `attempt`) are single-word — identical in both cases — but
 * the S17 end-frame fields are snake_case on the wire (`end_ssim` … `end_pass`)
 * and MUST be remapped, or `Library.endAdherenceSummary` (which reads `endSsim`/
 * `endPass`) never sees them and the end-adherence badge never renders on real
 * data (review B1). Both spellings are accepted, mirroring `parseKeyframeRecords`.
 */
export function parseJoinRecords(json: string): JoinRecord[] {
  const raw = JSON.parse(json) as {
    index: number;
    kind: "continuity" | "crossfade";
    applied: boolean;
    ssim: number | null;
    psnr: number | null;
    dy: number | null;
    du: number | null;
    dv: number | null;
    pass: boolean | null;
    attempt?: number;
    end_ssim?: number | null;
    endSsim?: number | null;
    end_psnr?: number | null;
    endPsnr?: number | null;
    end_dy?: number | null;
    endDy?: number | null;
    end_du?: number | null;
    endDu?: number | null;
    end_dv?: number | null;
    endDv?: number | null;
    end_pass?: boolean | null;
    endPass?: boolean | null;
  }[];
  return raw.map((j) => ({
    index: j.index,
    kind: j.kind,
    applied: j.applied,
    ssim: j.ssim ?? null,
    psnr: j.psnr ?? null,
    dy: j.dy ?? null,
    du: j.du ?? null,
    dv: j.dv ?? null,
    pass: j.pass ?? null,
    attempt: j.attempt ?? 0,
    endSsim: j.end_ssim ?? j.endSsim ?? null,
    endPsnr: j.end_psnr ?? j.endPsnr ?? null,
    endDy: j.end_dy ?? j.endDy ?? null,
    endDu: j.end_du ?? j.endDu ?? null,
    endDv: j.end_dv ?? j.endDv ?? null,
    endPass: j.end_pass ?? j.endPass ?? null,
  }));
}

/** Parse a set's `keyframes_json` (snake-case from Rust) into camelCase records. */
export function parseKeyframeRecords(json: string): KeyframeRecord[] {
  const raw = JSON.parse(json) as {
    index: number;
    job_set_id?: string | null;
    jobSetId?: string | null;
    local_path?: string | null;
    localPath?: string | null;
    estimated_usd?: number | null;
    estimatedUsd?: number | null;
    attempt?: number;
  }[];
  return raw.map((k) => ({
    index: k.index,
    jobSetId: k.job_set_id ?? k.jobSetId ?? null,
    localPath: k.local_path ?? k.localPath ?? null,
    estimatedUsd: k.estimated_usd ?? k.estimatedUsd ?? null,
    attempt: k.attempt ?? 0,
  }));
}

/**
 * S17: regenerate one keyframe still. Re-derives the still prompt from the
 * stored plan, prices it before it spends, and re-parks the set until the new
 * still settles. Returns the new still job id.
 */
export async function regenerateKeyframeStill(
  storyboardId: string,
  index: number,
): Promise<string> {
  return await invoke<string>("regenerate_keyframe_still", {
    storyboardId,
    index,
  } as unknown as Record<string, unknown>);
}

export async function listJobs(): Promise<JobSet[]> {
  try {
    const raw = await invoke<RawJobSet[]>("list_jobs");
    return raw.map(toJobSet);
  } catch (e) {
    if (!isDesktop()) return mockListJobs();
    throw asError(e);
  }
}

/**
 * Show a generated file in Finder or Explorer.
 *
 * Returns false when there is no shell to ask, so the caller can hide the
 * control rather than offering something that does nothing.
 */
export async function revealResult(path: string): Promise<boolean> {
  if (!isDesktop()) return false;
  try {
    await invoke("reveal_result", { path });
    return true;
  } catch {
    return false;
  }
}

/**
 * S15: open a finished render in the OS (reuses the opener permission, scoped
 * to the library). Off the desktop it is a no-op.
 */
export async function openResult(path: string): Promise<boolean> {
  if (!isDesktop()) return false;
  try {
    const { openPath } = await import("@tauri-apps/plugin-opener");
    await openPath(path);
    return true;
  } catch {
    // Fall back to reveal-in-folder, which is already wired and scoped.
    return revealResult(path);
  }
}

/**
 * S15: streamed storyboard render progress. Returns an unsubscribe function.
 * Mirrors `subscribeJobs`' async-setup/sync-teardown shape.
 */
export function subscribeRenderProgress(
  onProgress: (p: RenderProgress) => void,
): () => void {
  let cancelled = false;
  let detach: (() => void) | null = null;
  void (async () => {
    try {
      const { listen } = await import("@tauri-apps/api/event");
      const un = await listen<RawStoryboardRenderProgress>(
        "storyboard:render",
        (event) => {
          const p = event.payload;
          onProgress({
            storyboardId: p.storyboard_id ?? p.storyboardId ?? "",
            percent: p.percent ?? 0,
            done: p.done ?? false,
          });
        },
      );
      if (cancelled) un();
      else detach = un;
    } catch {
      // No desktop event bus in the browser; nothing streams.
    }
  })();
  return () => {
    cancelled = true;
    detach?.();
  };
}

interface RawStoryboardRenderProgress {
  storyboard_id?: string;
  storyboardId?: string;
  percent?: number;
  done?: boolean;
}

export async function cancelJob(jobSetId: string): Promise<void> {
  try {
    await invoke("cancel_job", { jobSetId });
  } catch (e) {
    if (!isDesktop()) return mockCancel(jobSetId);
    throw asError(e);
  }
}

/**
 * Remove a generation from history, and optionally its file.
 *
 * `deleteFiles` defaults to false: a row leaving a list is a much smaller act
 * than destroying an asset the user paid a provider to produce, so the
 * irreversible half is always explicit.
 *
 * Errors propagate. The caller must not drop the row from its own state until
 * this resolves, or a failed delete looks like a successful one until the next
 * refresh puts it back.
 */
export async function deleteJob(
  jobSetId: string,
  deleteFiles = false,
): Promise<void> {
  try {
    await invoke("delete_job", { jobSetId, deleteFiles });
  } catch (e) {
    if (!isDesktop()) return;
    throw asError(e);
  }
}

/**
 * Streamed job state. Returns an unsubscribe function.
 *
 * Async setup with a sync teardown: React effects cannot await their own
 * cleanup, so the flag guards the window where the effect is torn down before
 * `listen` has resolved. Without it a fast unmount leaks a live listener.
 */
export function subscribeJobs(onUpdate: (job: JobSet) => void): () => void {
  let cancelled = false;
  let detach: (() => void) | null = null;

  void (async () => {
    try {
      const { listen } = await import("@tauri-apps/api/event");
      const un = await listen<RawJobSet>("job:update", (event) => {
        onUpdate(toJobSet(event.payload));
      });
      if (cancelled) un();
      else detach = un;
    } catch {
      const un = mockSubscribe(onUpdate);
      if (cancelled) un();
      else detach = un;
    }
  })();

  return () => {
    cancelled = true;
    detach?.();
  };
}
