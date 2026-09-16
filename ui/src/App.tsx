import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  configuredProviders,
  estimateCost,
  isDesktop,
  keyStates,
  libraryRoot,
  listJobs,
  listModels,
  listPresets,
  listUseCases,
  modelsForUseCase,
  localEndpoints,
  ollamaModels,
  previewPrompt,
  providerInfo,
  submitJob,
  modelCapabilities,
  deleteJob,
  subscribeJobs,
  planStoryboard,
  generateStoryboard,
  listStoryboards,
  listStoryboardRenders,
  renderStoryboard,
  subscribeRenderProgress,
  openResult,
  type KeyState,
} from "./api";
import type {
  CostEstimate,
  GenSettings,
  JobSet,
  LocalModel,
  MediaRef,
  MediaRole,
  Model,
  ModelCapabilities,
  PresetFamily,
  PromptPreview,
  RewriterChoice,
  RenderProgress,
  Shot,
  Storyboard as StoryboardRow,
  StoryboardRender,
  StoryboardResult,
  UseCase,
  WorkspaceTab,
} from "./types";
import { capabilitiesFor, resolveSettings, selectRoute } from "./lib/variants";
import { measureAspect, nearestAspect } from "./lib/media";
import { previewOf } from "./lib/media-input";
import { isRunning } from "./lib/status";
import {
  fromFileList,
  mergeMedia,
  pickViaDialog,
  sourceKey,
} from "./lib/media-input";
import {
  PROVIDER_CATALOG,
  isAppConfigured,
  type LocalEndpoints,
  type ProviderInfo,
} from "./lib/providers";
import { SettingsRail } from "./components/SettingsRail";
import { Stage } from "./components/Stage";
import { Library } from "./components/Library";
import { Storyboard } from "./components/Storyboard";
import { WorkspaceTabs } from "./components/WorkspaceTabs";
import { MetaRail } from "./components/MetaRail";
import { PresetPicker } from "./components/PresetPicker";
import { ModelPicker } from "./components/ModelPicker";
import { Onboarding } from "./components/Onboarding";
import { Settings } from "./components/Settings";
import { ConfirmDelete } from "./components/ConfirmDelete";
import { Overlay } from "./components/Overlay";
import { VideoPreview } from "./components/VideoPreview";
import { KeyIcon } from "./components/Icons";
import { isPlayableVideo, posterFor } from "./lib/media";
import { revealResult } from "./api";
import "./app.css";

const DEFAULT_SETTINGS: GenSettings = {
  duration: 5,
  resolution: "1080p",
  aspect: "16:9",
  audio: false,
  // Enhance ships on. Raw prompts underperform on every hosted video model,
  // and the enhancer that ran is recorded per job so the rewrite is auditable.
  enhance: true,
};

const MAX_REFERENCES = 7;

/**
 * Records that the user chose "Skip for now".
 *
 * Without it, onboarding reappears on every launch until a key is added, which
 * turns a deliberate decision into a nag. The Generate button still says the
 * app is unconfigured, so nothing is hidden — see GenerateButton.
 */
const SKIP_KEY = "hickeyfield.onboarding.skipped";

const readSkipped = (): boolean => {
  try {
    return window.localStorage.getItem(SKIP_KEY) === "1";
  } catch {
    // Private-mode webviews can throw on access. A storage failure must not
    // stop the app booting; it just means onboarding shows again.
    return false;
  }
};

export default function App() {
  const [models, setModels] = useState<Model[]>([]);
  const [presets, setPresets] = useState<PresetFamily[]>([]);
  const [jobs, setJobs] = useState<JobSet[]>([]);

  const [useCases, setUseCases] = useState<UseCase[]>([]);
  const [tab, setTab] = useState<WorkspaceTab>("text-to-video");
  const [modelId, setModelId] = useState<string | null>(null);
  const [routeId, setRouteId] = useState<string | null>(null);
  const [presetId, setPresetId] = useState<string | null>(null);
  const [prompt, setPrompt] = useState("");
  const [settings, setSettings] = useState<GenSettings>(DEFAULT_SETTINGS);
  const [media, setMedia] = useState<MediaRef[]>([]);

  const [estimate, setEstimate] = useState<CostEstimate | null>(null);
  const [pending, setPending] = useState(false);
  // The opt-in prompt preview: the enhancer's output shown before spending
  // money. `preview` is null when the panel is closed; `previewText` is the
  // editable string that Generate-from-preview sends VERBATIM as `finalPrompt`.
  const [preview, setPreview] = useState<PromptPreview | null>(null);
  const [previewText, setPreviewText] = useState("");
  const [previewing, setPreviewing] = useState(false);
  /**
   * The last submit failure, shown above the feed. Held here rather than in a
   * toast because a refused generation is something the user must act on —
   * add a key, change a model, drop an attachment — not something to dismiss.
   */
  const [submitError, setSubmitError] = useState<string | null>(null);
  const [overlay, setOverlay] = useState<
    null | "presets" | "models" | "onboarding" | "settings"
  >(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  // Studio (compose + stage) vs Library (gallery + meta). A separate axis from
  // the use-case `tab`; a two-value state, not a router (there is no URL).
  const [view, setView] = useState<"studio" | "library" | "storyboard">(
    "studio",
  );
  // Stored storyboards, for the Library group headers. Refreshed after a
  // generate-all so the new set has its brief to head the group with.
  const [storyboards, setStoryboards] = useState<StoryboardRow[]>([]);
  // S15: finished storyboard renders, and live render progress by set id.
  const [renders, setRenders] = useState<StoryboardRender[]>([]);
  const [renderProgress, setRenderProgress] = useState<
    Record<string, RenderProgress>
  >({});
  // The Library lightbox: the job whose result is shown large, or null.
  const [lightboxId, setLightboxId] = useState<string | null>(null);

  const [catalog, setCatalog] = useState<ProviderInfo[]>(PROVIDER_CATALOG);
  const [states, setStates] = useState<KeyState[]>([]);
  // null until the first read lands. Distinguishing "not loaded" from "nothing
  // configured" is what stops the Generate button flashing "Add a provider key"
  // for one frame on every launch.
  const [configured, setConfigured] = useState<string[] | null>(null);
  const [local, setLocal] = useState<LocalEndpoints | null>(null);
  // Chat models the local Ollama has installed, for the enhancer picker, and
  // the user's explicit enhancer choice (null = auto: the shell decides).
  const [ollamaModelList, setOllamaModelList] = useState<LocalModel[]>([]);
  const [enhancer, setEnhancer] = useState<RewriterChoice | null>(null);
  const [libraryPath, setLibraryPath] = useState<string | null>(null);

  // Object URLs for locally attached files. Held so they can be revoked;
  // leaking them keeps whole video files alive in memory for the session.
  const objectUrls = useRef<Set<string>>(new Set());

  useEffect(() => {
    void (async () => {
      const [u, m, p, j] = await Promise.all([
        listUseCases(),
        listModels(),
        listPresets(),
        listJobs(),
      ]);
      setUseCases(u);
      if (u.length > 0 && !u.some((x) => x.slug === tab)) setTab(u[0].slug);
      setModels(m);
      setPresets(p);
      setJobs(j);
      const first = m.find((x) => x.isLaunch && x.modality === "video") ?? m[0];
      if (first) {
        setModelId(first.id);
        setRouteId(first.routes[0]?.id ?? null);
      }
    })();
  }, []);

  useEffect(() => {
    const urls = objectUrls.current;
    return () => {
      for (const url of urls) URL.revokeObjectURL(url);
      urls.clear();
    };
  }, []);

  /** Re-read credential presence. Called after every change either screen makes. */
  const refreshKeys = useCallback(async () => {
    const [s, c] = await Promise.all([keyStates(), configuredProviders()]);
    setStates(s);
    setConfigured(c);
  }, []);

  const recheckLocal = useCallback(async () => {
    const [l, m] = await Promise.all([localEndpoints(), ollamaModels()]);
    setLocal(l);
    setOllamaModelList(m);
  }, []);

  useEffect(() => {
    void (async () => {
      const [info, s, c, l, root, models] = await Promise.all([
        providerInfo(),
        keyStates(),
        configuredProviders(),
        localEndpoints(),
        libraryRoot(),
        ollamaModels(),
      ]);
      setCatalog(info);
      setStates(s);
      setConfigured(c);
      setLocal(l);
      setLibraryPath(root);
      setOllamaModelList(models);
      // First run: nothing can generate, so say so up front rather than letting
      // the user build a prompt that has nowhere to go.
      if (!isAppConfigured(c, l) && !readSkipped()) setOverlay("onboarding");
    })();
  }, []);

  useEffect(
    () =>
      subscribeJobs((job) => {
        setJobs((prev) => {
          const next = prev.filter((j) => j.id !== job.id);
          return [job, ...next].sort(
            (a, b) => Date.parse(b.createdAt) - Date.parse(a.createdAt),
          );
        });
      }),
    [],
  );

  const model = useMemo(
    () => models.find((m) => m.id === modelId) ?? null,
    [models, modelId],
  );
  const route = useMemo(
    () => selectRoute(model, routeId),
    [model, routeId],
  );
  // The catalogue's answer, available immediately.
  const catalogCaps = useMemo(() => capabilitiesFor(model), [model]);
  // fal's own answer, fetched when the model or the attachments change.
  // Overrides the catalogue once it lands; until then the chips still work.
  const [liveCaps, setLiveCaps] = useState<ModelCapabilities | null>(null);
  const capabilities = liveCaps ?? catalogCaps;

  useEffect(() => {
    if (!modelId) return;
    let live = true;
    setLiveCaps(null);
    void modelCapabilities(modelId, routeId, media.length > 0).then((c) => {
      // Only replace on a real answer. A null means we could not reach fal,
      // and blanking the chip row because we are offline would be worse than
      // showing the catalogue's slightly-less-certain version.
      if (live && c) setLiveCaps(c);
    });
    return () => {
      live = false;
    };
  }, [modelId, routeId, media.length]);
  const preset = useMemo(
    () => presets.find((p) => p.id === presetId) ?? null,
    [presets, presetId],
  );

  // Switching model can strand a setting the new model cannot run. Clamping
  // here rather than at submit means the chip row never displays a value that
  // would be silently rewritten on the way out.
  useEffect(() => {
    setSettings((prev) => resolveSettings(prev, capabilities));
  }, [capabilities]);

  // Cost re-estimates on every settings change, so the debounce is what keeps
  // a dragged control from firing a request per frame.
  useEffect(() => {
    if (!modelId || !route) return;
    let live = true;
    const timer = window.setTimeout(() => {
      void estimateCost({ modelId, routeId: route.id, settings })
        .then((res) => {
          if (live) setEstimate(res);
        })
        .catch(() => {
          // A failed estimate must not quote a stale price for a model the
          // user has since switched away from. Clearing it makes the button
          // say "price unavailable" rather than something wrong.
          if (live) setEstimate(null);
        });
    }, 200);
    return () => {
      live = false;
      window.clearTimeout(timer);
    };
  }, [modelId, route, settings]);

  // The roster follows the job. Filtered in Rust so the picker cannot offer
  // something the submit path would refuse — which is exactly what happened
  // when every tab showed every model and a text-only model accepted a clip.
  useEffect(() => {
    if (useCases.length === 0) return;
    let live = true;
    void modelsForUseCase(tab).then((m) => {
      if (!live || m.length === 0) return;
      setModels(m);
      // A model that cannot do the new job must not stay selected. Falling
      // back to the first launch model keeps the rail usable rather than
      // leaving it in an unrunnable state the user has to notice and fix.
      setModelId((current) => {
        if (current && m.some((x) => x.id === current)) return current;
        const next = m.find((x) => x.isLaunch) ?? m[0];
        setRouteId(next?.routes[0]?.id ?? null);
        return next?.id ?? null;
      });
    });
    return () => {
      live = false;
    };
  }, [tab, useCases.length]);

  // Whether the user has picked an aspect themselves. Once they have, nothing
  // may move it under them — an attachment that silently overrode a deliberate
  // choice would be its own inconsistency, just a politer one.
  const aspectChosen = useRef(false);

  const patchSettings = useCallback((patch: Partial<GenSettings>) => {
    if (patch.aspect !== undefined) aspectChosen.current = true;
    setSettings((prev) => ({ ...prev, ...patch }));
  }, []);

  /**
   * Follow the shape of what was attached.
   *
   * The default aspect is a landscape one, so animating a portrait photo asked
   * for 16:9 and came back landscape — the user having said nothing about
   * aspect at all, and the interface having shown them a ratio they never
   * picked. Attaching a file is the clearest statement of intent available
   * about the shape of the result, so it sets the chip, visibly and
   * overridably, rather than being quietly contradicted at submit time.
   */
  useEffect(() => {
    if (aspectChosen.current) return;
    const first = media[0];
    const url = first ? previewOf(first) : undefined;
    if (!url || capabilities.aspects.length === 0) return;

    let live = true;
    const kind = first.role.includes("video") ? "video" : "image";
    void measureAspect(url, kind).then((ratio) => {
      if (!live || ratio === null) return;
      const snapped = nearestAspect(ratio, capabilities.aspects);
      // Not through `patchSettings`: this is us following the user, not the
      // user making a choice, and recording it as a choice would freeze the
      // chip against the *next* attachment.
      if (snapped) setSettings((prev) => ({ ...prev, aspect: snapped }));
    });
    return () => {
      live = false;
    };
  }, [media, capabilities.aspects]);

  /**
   * Attach files to a role.
   *
   * `files` is only used in a plain browser. Inside the app the native dialog
   * runs instead, because it is the only thing that yields a real filesystem
   * path — and a path is what Rust needs to upload the bytes. A `blob:` URL
   * from `createObjectURL` resolves in this webview and nowhere else.
   */
  const addMedia = useCallback(
    async (role: MediaRole, files: FileList | null) => {
      const multiple = role === "reference";
      let added = await pickViaDialog(role, multiple);
      if (added === null) added = await fromFileList(role, files, multiple);
      if (added.length === 0) return;

      for (const m of added) {
        if (m.preview) objectUrls.current.add(m.preview);
      }
      setMedia((prev) => {
        const merged = mergeMedia(prev, added, multiple);
        if (!multiple) return merged;
        // Trim to the cap here rather than in the merge, so the cap stays a
        // product decision and mergeMedia stays a pure list operation.
        const refs = merged.filter((m) => m.role === "reference");
        if (refs.length <= MAX_REFERENCES) return merged;
        const keep = new Set(refs.slice(0, MAX_REFERENCES).map(sourceKey));
        return merged.filter((m) => m.role !== "reference" || keep.has(sourceKey(m)));
      });
    },
    [],
  );

  const forget = useCallback((m: MediaRef) => {
    // Leaking an object URL keeps a whole video alive in memory for the
    // session, which on a 4K clip is very visible.
    if (m.preview && objectUrls.current.delete(m.preview)) {
      URL.revokeObjectURL(m.preview);
    }
  }, []);

  const removeByRole = useCallback(
    (role: MediaRole) => {
      setMedia((prev) => {
        for (const m of prev.filter((x) => x.role === role)) forget(m);
        return prev.filter((m) => m.role !== role);
      });
    },
    [forget],
  );

  const removeByKey = useCallback(
    (key: string) => {
      setMedia((prev) => {
        for (const m of prev.filter((x) => sourceKey(x) === key)) forget(m);
        return prev.filter((m) => sourceKey(m) !== key);
      });
    },
    [forget],
  );

  // One submit path for both buttons. `finalPrompt` is the exact previewed
  // text: when present the shell sends it verbatim and does NOT re-enhance.
  // Quick Generate calls `send()` with no argument, so its behaviour — a silent
  // enhance-and-submit — is unchanged.
  const send = useCallback(
    async (finalPrompt?: string) => {
      if (!modelId || !route) return false;
      setPending(true);
      setSubmitError(null);
      try {
        await submitJob({
          modelId,
          routeId: route.id,
          prompt,
          presetId,
          settings,
          media,
          rewriter: enhancer,
          finalPrompt: finalPrompt ?? null,
        });
        setJobs(await listJobs());
        return true;
      } catch (e) {
        setSubmitError(e instanceof Error ? e.message : String(e));
        try {
          setJobs(await listJobs());
        } catch {
          // Already reporting the more useful error.
        }
        return false;
      } finally {
        setPending(false);
      }
    },
    [modelId, route, prompt, presetId, settings, media, enhancer],
  );

  const onSubmit = useCallback(async () => {
    if (!modelId || !route) return;
    await send();
  }, [modelId, route, send]);

  // Fetch (or re-fetch) the preview. Guards the same way submit does — no model
  // or route means nothing to compile — and opens the panel with the compiled
  // text seeded into the editable box.
  const runPreview = useCallback(async () => {
    if (!modelId || !route) return;
    setPreviewing(true);
    setSubmitError(null);
    try {
      const p = await previewPrompt({
        modelId,
        routeId: route.id,
        prompt,
        presetId,
        settings,
        media,
        rewriter: enhancer,
        finalPrompt: null,
      });
      setPreview(p);
      // Seed the editable box with the rewrite when there was one, else the
      // compiled original — the exact text the user would otherwise send.
      setPreviewText(p.enhanced ?? p.prompt);
    } catch (e) {
      setSubmitError(e instanceof Error ? e.message : String(e));
    } finally {
      setPreviewing(false);
    }
  }, [modelId, route, prompt, presetId, settings, media, enhancer]);

  // Generate with exactly the text shown in the preview. On success the panel
  // is cleared so the next generation starts from the quick path again.
  const onGenerateFromPreview = useCallback(async () => {
    const ok = await send(previewText);
    if (ok) {
      setPreview(null);
      setPreviewText("");
    }
  }, [send, previewText]);

  // Storyboard bridge wrappers. Passed to the Storyboard view so the component
  // stays testable with fakes; here they close over the live tab, models,
  // settings and enhancer choice.
  const storyPlan = useCallback(
    (args: {
      brief: string;
      targetSeconds: number;
      useCase: string;
      modelPrefs: string[];
      rewriter?: RewriterChoice | null;
    }) => planStoryboard(args),
    [],
  );
  const storyEstimate = useCallback(
    (shot: Shot): Promise<CostEstimate | null> =>
      estimateCost({
        modelId: shot.modelId,
        routeId:
          models.find((m) => m.id === shot.modelId)?.routes.find(
            (r) => r.available !== false,
          )?.id ??
          models.find((m) => m.id === shot.modelId)?.routes[0]?.id ??
          "",
        settings: { ...settings, duration: shot.seconds },
      }),
    [models, settings],
  );
  const storyPreview = useCallback(
    (shot: Shot): Promise<PromptPreview> =>
      previewPrompt({
        modelId: shot.modelId,
        routeId:
          models.find((m) => m.id === shot.modelId)?.routes[0]?.id ?? "",
        prompt: shot.prompt,
        presetId: shot.camera,
        settings: { ...settings, duration: shot.seconds },
        media: [],
        rewriter: enhancer,
        finalPrompt: null,
      }),
    [models, settings, enhancer],
  );
  const onStoryboardGenerated = useCallback((_result: StoryboardResult) => {
    // The new set's jobs stream in over the existing subscription; refresh the
    // job list and the storyboard headers, then watch the set run in Library.
    void listJobs().then(setJobs);
    void listStoryboards().then(setStoryboards);
    setView("library");
  }, []);

  // Load the storyboard group headers once, and whenever we land on Library.
  useEffect(() => {
    if (view === "library") {
      void listStoryboards().then(setStoryboards);
      void listStoryboardRenders().then(setRenders);
    }
  }, [view]);

  // S15: stream render progress; on `done`, refresh the renders so the set item
  // shows the finished ad and its Σ actual cost.
  useEffect(
    () =>
      subscribeRenderProgress((p) => {
        setRenderProgress((prev) => ({ ...prev, [p.storyboardId]: p }));
        if (p.done) void listStoryboardRenders().then(setRenders);
      }),
    [],
  );

  // S15: start a render. Rendering spends nothing — it runs the bundled ffmpeg
  // on the already-downloaded clips locally.
  const onRenderStoryboard = useCallback((storyboardId: string) => {
    setRenderProgress((prev) => ({
      ...prev,
      [storyboardId]: { storyboardId, percent: 0, done: false },
    }));
    void renderStoryboard({ storyboardId })
      .then((r) => {
        setRenders((prev) => [
          r,
          ...prev.filter((x) => x.storyboardId !== storyboardId),
        ]);
        setRenderProgress((prev) => ({
          ...prev,
          [storyboardId]: { storyboardId, percent: 1, done: true },
        }));
      })
      .catch((e: unknown) => {
        // Surface the failure in the same banner every other refused action
        // uses, then clear the progress so the Render button returns.
        setSubmitError(e instanceof Error ? e.message : String(e));
        setRenderProgress((prev) => {
          const next = { ...prev };
          delete next[storyboardId];
          return next;
        });
      });
  }, []);

  const onRerun = useCallback(
    (job: JobSet) => {
      // Reload the job's exact inputs into the rail rather than resubmitting
      // blind, so the user can change one thing before paying again.
      setModelId(job.modelId);
      setRouteId(job.route?.id ?? null);
      setPrompt(job.prompt);
      setPresetId(job.presetId ?? null);
      if (job.settings) setSettings(job.settings);
      // Restore the attachments too. Without this, rerunning an
      // image-to-video job silently resubmitted it as text-to-video — the same
      // price for a generation the user did not ask to repeat.
      setMedia(job.media ?? []);
      document.getElementById("prompt-input")?.focus();
    },
    [],
  );

  // The Studio stage shows the selection when there is one, else the latest
  // (jobs are newest-first). Recents are the first six.
  const featured = useMemo(
    () => jobs.find((j) => j.id === selectedId) ?? jobs[0] ?? null,
    [jobs, selectedId],
  );
  const recents = useMemo(() => jobs.slice(0, 6), [jobs]);
  const featuredModelName = useMemo(() => {
    if (!featured) return "";
    return (
      models.find((m) => m.id === featured.modelId)?.displayName ??
      featured.modelId
    );
  }, [featured, models]);

  // Camera-move presets drive the composer strip (S11's "25 camera presets").
  const cameraPresets = useMemo(
    () => presets.filter((p) => p.category === "camera-control"),
    [presets],
  );

  // Extend is only honest when the roster has an edit-video use case to land on
  // and the job actually produced a saved clip to extend. When either is
  // missing the Stage hides the pill rather than offering a dead action (§D4).
  const extendUseCase = useMemo(
    () => useCases.find((u) => u.slug === "edit-video") ?? null,
    [useCases],
  );
  const onExtend = useCallback(
    (job: JobSet) => {
      const path = job.results?.find((r) => r.localPath)?.localPath;
      if (!path || !extendUseCase) return;
      // Prefill the source clip and land on the extend tab; the specific
      // route is then chosen by the roster filter for that tab. We do not
      // force-select veo3_1_extend — a wrong auto-select is worse than the
      // right tab (§8, Q4).
      setTab(extendUseCase.slug);
      setMedia([
        {
          role: "video",
          source: { kind: "local", path },
          name: path.split("/").pop() ?? "clip",
        },
      ]);
      setPrompt(job.prompt);
      setView("studio");
      document.getElementById("prompt-input")?.focus();
    },
    [extendUseCase],
  );

  const lightboxJob = useMemo(
    () => (lightboxId ? (jobs.find((j) => j.id === lightboxId) ?? null) : null),
    [lightboxId, jobs],
  );

  // Deletion is two steps: the card asks, the modal commits. Our accent is
  // red, so colour cannot carry destructive weight — the confirm does.
  const [pendingDelete, setPendingDelete] = useState<JobSet | null>(null);

  const onDelete = useCallback(
    (id: string) => {
      setPendingDelete(jobs.find((j) => j.id === id) ?? null);
    },
    [jobs],
  );

  const confirmDelete = useCallback(
    async (deleteFiles: boolean) => {
      const job = pendingDelete;
      setPendingDelete(null);
      if (!job) return;
      try {
        await deleteJob(job.id, deleteFiles);
        // Only drop it from the list once the shell agreed. Removing it first
        // is how a delete appeared to work and then reverted on next refresh.
        setJobs((prev) => prev.filter((j) => j.id !== job.id));
        if (selectedId === job.id) setSelectedId(null);
      } catch (e) {
        setSubmitError(e instanceof Error ? e.message : String(e));
      }
    },
    [pendingDelete, selectedId],
  );

  // Selection is the pairing mechanism between the two independently
  // scrolling columns. `nearest` means an already-visible counterpart does not
  // jump, which matters because hovering a meta card selects it.
  useEffect(() => {
    if (!selectedId) return;
    for (const attr of ["data-job-anchor", "data-meta-anchor"]) {
      document
        .querySelector(`[${attr}="${selectedId}"]`)
        ?.scrollIntoView({ block: "nearest", behavior: "smooth" });
    }
  }, [selectedId]);

  const runningCount = jobs.filter((j) => isRunning(j.status)).length;
  const needsSetup = configured !== null && !isAppConfigured(configured, local);

  const skipOnboarding = useCallback(() => {
    try {
      window.localStorage.setItem(SKIP_KEY, "1");
    } catch {
      // Not being able to remember the skip is a nuisance, not a failure.
    }
    setOverlay(null);
  }, []);

  return (
    <div className="shell">
      <header className="titlebar" data-tauri-drag-region>
        <div className="titlebar-left">
          <span className="wordmark">HICKEYFIELD</span>
          <nav className="view-switch" aria-label="View">
            <button
              type="button"
              className="view-switch-tab"
              aria-pressed={view === "studio"}
              onClick={() => setView("studio")}
            >
              Studio
            </button>
            <button
              type="button"
              className="view-switch-tab"
              aria-pressed={view === "storyboard"}
              onClick={() => setView("storyboard")}
            >
              Storyboard
            </button>
            <button
              type="button"
              className="view-switch-tab"
              aria-pressed={view === "library"}
              onClick={() => setView("library")}
            >
              Library
            </button>
          </nav>
        </div>
        <span className="titlebar-meta">
          {runningCount > 0 ? (
            <span className="titlebar-running">{runningCount} running</span>
          ) : null}
          <span className="titlebar-mode">
            {isDesktop() ? "Desktop" : "Browse mode · sample data"}
          </span>
        </span>
      </header>

      <div className="workspace" data-view={view}>
        <nav className="use-nav" aria-label="Use case">
          <WorkspaceTabs
            variant="nav"
            useCases={useCases}
            value={tab}
            onChange={setTab}
            panelId="generator-panel"
          />
          <button
            type="button"
            className="use-nav-keys"
            onClick={() => setOverlay("settings")}
            aria-label="Keys and settings"
            title="Keys and settings"
          >
            <KeyIcon size={20} />
            <span className="tabs-nav-label">Keys</span>
          </button>
        </nav>

        {view === "storyboard" ? (
          <Storyboard
            tab={tab}
            models={models}
            cameraPresets={cameraPresets}
            settings={settings}
            enhancer={enhancer}
            onPlan={storyPlan}
            onEstimate={storyEstimate}
            onPreview={storyPreview}
            onGenerate={generateStoryboard}
            onGenerated={onStoryboardGenerated}
          />
        ) : view === "studio" ? (
          <>
            <SettingsRail
              useCases={useCases}
              tab={tab}
              preset={preset}
              model={model}
              route={route}
              capabilities={capabilities}
              settings={settings}
              onSettingsChange={patchSettings}
              prompt={prompt}
              onPromptChange={setPrompt}
              media={media}
              onMediaAdd={addMedia}
              onMediaRemoveRole={removeByRole}
              onMediaRemoveKey={removeByKey}
              ollamaUp={local?.ollama ?? false}
              ollamaModels={ollamaModelList}
              openaiAvailable={states.some(
                (s) => s.provider === "openai" && s.hasKey,
              )}
              enhancer={enhancer}
              onEnhancerChange={setEnhancer}
              estimate={estimate}
              pending={pending}
              needsSetup={needsSetup}
              onOpenPresets={() => setOverlay("presets")}
              onOpenModels={() => setOverlay("models")}
              onOpenSetup={() => setOverlay("onboarding")}
              onSubmit={() => void onSubmit()}
              cameraPresets={cameraPresets}
              onPickPreset={(p) => setPresetId(p.id)}
              preview={preview}
              previewText={previewText}
              onPreviewTextChange={setPreviewText}
              previewing={previewing}
              onPreview={() => void runPreview()}
              onRetryPreview={() => void runPreview()}
              onGenerateFromPreview={() => void onGenerateFromPreview()}
            />

            <Stage
              job={featured}
              recents={recents}
              modelName={featuredModelName}
              error={submitError}
              onDismissError={() => setSubmitError(null)}
              onSelect={setSelectedId}
              onRetry={onRerun}
              onExtend={extendUseCase ? onExtend : null}
              onReveal={(path) => void revealResult(path)}
            />
          </>
        ) : (
          <>
            <Library
              jobs={jobs}
              selectedId={selectedId}
              onSelect={setSelectedId}
              onOpenLightbox={setLightboxId}
              storyboards={storyboards}
              renders={renders}
              renderProgress={renderProgress}
              onRender={onRenderStoryboard}
              onOpenResult={(path) => void openResult(path)}
            />

            <MetaRail
              jobs={jobs}
              models={models}
              selectedId={selectedId}
              onSelect={setSelectedId}
              onRerun={onRerun}
              onDelete={onDelete}
            />
          </>
        )}
      </div>

      <PresetPicker
        open={overlay === "presets"}
        onClose={() => setOverlay(null)}
        presets={presets}
        models={models}
        activeModelId={modelId}
        onPick={(p) => setPresetId(p.id)}
      />

      <ModelPicker
        open={overlay === "models"}
        onClose={() => setOverlay(null)}
        models={models}
        selectedModelId={modelId}
        selectedRouteId={route?.id ?? null}
        onSelect={(m, r) => {
          setModelId(m);
          setRouteId(r);
        }}
      />

      <Onboarding
        open={overlay === "onboarding"}
        onClose={() => setOverlay(null)}
        onSkip={skipOnboarding}
        catalog={catalog}
        states={states}
        local={local}
        onChanged={() => void refreshKeys()}
      />

      <Settings
        open={overlay === "settings"}
        onClose={() => setOverlay(null)}
        catalog={catalog}
        states={states}
        local={local}
        libraryPath={libraryPath}
        onChanged={() => void refreshKeys()}
        onRecheckLocal={recheckLocal}
        onRunOnboarding={() => setOverlay("onboarding")}
      />

      {/* A sibling of the other overlays now, not nested inside a titlebar
          button — the fix for the button-in-button invalid HTML (AC1). */}
      <ConfirmDelete
        open={pendingDelete !== null}
        hasFiles={Boolean(
          pendingDelete?.results?.some((r) => Boolean(r.localPath)),
        )}
        onCancel={() => setPendingDelete(null)}
        onConfirm={(files) => void confirmDelete(files)}
      />

      <Overlay
        open={lightboxJob !== null}
        onClose={() => setLightboxId(null)}
        title={lightboxJob ? "Result" : ""}
        variant="full"
      >
        {lightboxJob ? <Lightbox job={lightboxJob} /> : null}
      </Overlay>
    </div>
  );
}

/** The large result shown in the Library lightbox — the same media rendering
 * the feed uses, never a snapshot thumbnail. */
function Lightbox({ job }: { job: JobSet }) {
  const first = job.results?.[0];
  return (
    <div className="lightbox">
      {first && isPlayableVideo(first) ? (
        <VideoPreview result={first} className="lightbox-media" />
      ) : first ? (
        <img className="lightbox-media" src={posterFor(first)} alt="" />
      ) : (
        <span className="lightbox-empty">No output to show</span>
      )}
    </div>
  );
}
