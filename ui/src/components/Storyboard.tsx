import { useCallback, useEffect, useRef, useState } from "react";
import type {
  CostEstimate,
  GenSettings,
  Model,
  PresetFamily,
  PromptPreview as Preview,
  RewriterChoice,
  Shot,
  StoryboardPlan,
  StoryboardResult,
  SubmitInput,
} from "../types";
import { formatUsd, totalCostCaption } from "../lib/cost";
import { ModelPicker } from "./ModelPicker";
import { PromptPreview } from "./PromptPreview";

/**
 * The Storyboard view: one brief → an editable, model-aware shot list → one
 * linked set of generations.
 *
 * The drafting and money calls are injected as props rather than imported, so
 * the whole surface is testable with fakes and no bridge — the same principle
 * the rest of the components follow. What the view owns is the editable plan
 * (local `shots`), the per-shot estimates, and the honest summed total: a shot
 * with no published price is never summed as $0 (see `totalCostCaption`), and
 * Generate-all stays disabled until that total has rendered, so nothing is ever
 * submitted without the cost shown.
 */
export function Storyboard({
  tab,
  models,
  cameraPresets,
  settings,
  enhancer,
  onPlan,
  onEstimate,
  onPreview,
  onGenerate,
  onGenerated,
}: {
  /** The current use-case slug the plan is validated against. */
  tab: string;
  /** Models runnable for the tab — the allowed set handed to the planner. */
  models: Model[];
  /** Camera-move presets for the per-shot camera picker. */
  cameraPresets: PresetFamily[];
  /** The rail's current settings, used as the base for each shot's estimate. */
  settings: GenSettings;
  enhancer: RewriterChoice | null;
  onPlan: (args: {
    brief: string;
    targetSeconds: number;
    useCase: string;
    modelPrefs: string[];
    rewriter?: RewriterChoice | null;
  }) => Promise<StoryboardPlan>;
  onEstimate: (shot: Shot) => Promise<CostEstimate | null>;
  onPreview: (shot: Shot) => Promise<Preview>;
  onGenerate: (input: {
    brief: string;
    targetSeconds: number;
    planJson: string;
    shots: SubmitInput[];
  }) => Promise<StoryboardResult>;
  /** Called after a successful generate-all (App switches to the Library). */
  onGenerated: (result: StoryboardResult) => void;
}) {
  const [brief, setBrief] = useState("");
  const [targetSeconds, setTargetSeconds] = useState(30);
  const [shots, setShots] = useState<Shot[]>([]);
  const [estimates, setEstimates] = useState<(CostEstimate | null)[]>([]);
  const [drafting, setDrafting] = useState(false);
  const [generating, setGenerating] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // Which shot's model picker is open, or null.
  const [pickerShot, setPickerShot] = useState<number | null>(null);
  // Which shot's inline preview is open, and its editable text per shot.
  const [previewShot, setPreviewShot] = useState<number | null>(null);
  const [preview, setPreview] = useState<Preview | null>(null);
  const [previewText, setPreviewText] = useState("");
  const [previewing, setPreviewing] = useState(false);
  // The exact previewed text per shot, sent verbatim as finalPrompt (S7).
  const [finalPrompts, setFinalPrompts] = useState<Record<number, string>>({});

  const modelName = useCallback(
    (id: string) => models.find((m) => m.id === id)?.displayName ?? id,
    [models],
  );

  // Re-estimate on every change to the shot list. A monotonic token guards
  // against an out-of-order async resolution overwriting a newer estimate.
  const estimateToken = useRef(0);
  useEffect(() => {
    if (shots.length === 0) {
      setEstimates([]);
      return;
    }
    const token = ++estimateToken.current;
    void Promise.all(shots.map((s) => onEstimate(s).catch(() => null))).then(
      (next) => {
        if (estimateToken.current === token) setEstimates(next);
      },
    );
  }, [shots, onEstimate]);

  const draft = useCallback(async () => {
    if (!brief.trim()) return;
    setDrafting(true);
    setError(null);
    try {
      const plan = await onPlan({
        brief,
        targetSeconds,
        useCase: tab,
        modelPrefs: models.map((m) => m.id),
        rewriter: enhancer,
      });
      setShots(plan.shots);
      setFinalPrompts({});
      setPreviewShot(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setDrafting(false);
    }
  }, [brief, targetSeconds, tab, models, enhancer, onPlan]);

  const patchShot = useCallback((i: number, patch: Partial<Shot>) => {
    setShots((prev) =>
      prev.map((s, idx) => (idx === i ? { ...s, ...patch } : s)),
    );
  }, []);

  const removeShot = useCallback((i: number) => {
    setShots((prev) => renumber(prev.filter((_, idx) => idx !== i)));
    setFinalPrompts({});
  }, []);

  const addShot = useCallback(() => {
    setShots((prev) => {
      const last = prev[prev.length - 1];
      const next: Shot = {
        title: "New shot",
        prompt: "",
        seconds: last?.seconds ?? 5,
        camera: null,
        modelId: last?.modelId ?? models[0]?.id ?? "",
        role: "middle",
        continuity: null,
        advisories: [],
      };
      return renumber([...prev, next]);
    });
  }, [models]);

  const moveShot = useCallback((i: number, dir: -1 | 1) => {
    setShots((prev) => {
      const j = i + dir;
      if (j < 0 || j >= prev.length) return prev;
      const next = [...prev];
      [next[i], next[j]] = [next[j], next[i]];
      return renumber(next);
    });
    setFinalPrompts({});
  }, []);

  const openPreview = useCallback(
    async (i: number) => {
      setPreviewShot(i);
      setPreviewing(true);
      setError(null);
      try {
        const p = await onPreview(shots[i]);
        setPreview(p);
        setPreviewText(p.enhanced ?? p.prompt);
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
      } finally {
        setPreviewing(false);
      }
    },
    [shots, onPreview],
  );

  const acceptPreview = useCallback(
    (i: number) => {
      setFinalPrompts((prev) => ({ ...prev, [i]: previewText }));
      setPreviewShot(null);
    },
    [previewText],
  );

  const total = totalCostCaption(estimates);
  const canGenerate =
    shots.length > 0 && estimates.length === shots.length && !generating;

  const generate = useCallback(async () => {
    if (shots.length === 0) return;
    setGenerating(true);
    setError(null);
    try {
      const plan: StoryboardPlan = {
        brief,
        targetSeconds,
        useCase: tab,
        shots,
      };
      const shotInputs: SubmitInput[] = shots.map((s, i) => ({
        modelId: s.modelId,
        routeId: routeFor(models, s.modelId),
        prompt: s.prompt,
        presetId: s.camera,
        settings: { ...settings, duration: s.seconds },
        media: [],
        rewriter: enhancer,
        finalPrompt: finalPrompts[i] ?? null,
      }));
      const result = await onGenerate({
        brief,
        targetSeconds,
        planJson: JSON.stringify(plan),
        shots: shotInputs,
      });
      onGenerated(result);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setGenerating(false);
    }
  }, [
    shots,
    brief,
    targetSeconds,
    tab,
    models,
    settings,
    enhancer,
    finalPrompts,
    onGenerate,
    onGenerated,
  ]);

  return (
    <section className="storyboard" aria-label="Storyboard">
      <header className="storyboard-head">
        <h1 className="storyboard-title">STORYBOARD</h1>
        <p className="storyboard-note">
          Drafting uses your enhancer — free on local Ollama; a hosted key bills
          one short completion. No clip is generated until you see the total.
        </p>
        <label className="storyboard-field">
          <span className="storyboard-label">Brief</span>
          <textarea
            className="storyboard-brief"
            value={brief}
            onChange={(e) => setBrief(e.currentTarget.value)}
            rows={3}
            placeholder="a 30-second ad for a cold-brew can…"
          />
        </label>
        <div className="storyboard-controls">
          <label className="storyboard-target">
            <span className="storyboard-label">Target length (s)</span>
            <input
              type="number"
              className="storyboard-target-input"
              min={2}
              value={targetSeconds}
              onChange={(e) =>
                setTargetSeconds(Number(e.currentTarget.value) || 0)
              }
            />
          </label>
          <button
            type="button"
            className="storyboard-draft"
            disabled={drafting || !brief.trim()}
            onClick={() => void draft()}
          >
            {drafting ? "Drafting…" : "Draft shot list"}
          </button>
        </div>
      </header>

      {error ? <p className="storyboard-error">{error}</p> : null}

      {shots.length > 0 ? (
        <>
          <ol className="storyboard-shots">
            {shots.map((shot, i) => {
              const est = estimates[i] ?? null;
              return (
                <li className="storyboard-shot" key={i} data-shot-index={i}>
                  <div className="storyboard-shot-head">
                    <span className="storyboard-shot-n">Shot {i + 1}</span>
                    <span className="storyboard-shot-role">{shot.role}</span>
                    <div className="storyboard-shot-move">
                      <button
                        type="button"
                        className="storyboard-move"
                        aria-label={`Move shot ${i + 1} up`}
                        disabled={i === 0}
                        onClick={() => moveShot(i, -1)}
                      >
                        ↑
                      </button>
                      <button
                        type="button"
                        className="storyboard-move"
                        aria-label={`Move shot ${i + 1} down`}
                        disabled={i === shots.length - 1}
                        onClick={() => moveShot(i, 1)}
                      >
                        ↓
                      </button>
                      <button
                        type="button"
                        className="storyboard-remove"
                        aria-label={`Delete shot ${i + 1}`}
                        onClick={() => removeShot(i)}
                      >
                        ✕
                      </button>
                    </div>
                  </div>

                  <input
                    className="storyboard-shot-title"
                    value={shot.title}
                    aria-label={`Shot ${i + 1} title`}
                    onChange={(e) =>
                      patchShot(i, { title: e.currentTarget.value })
                    }
                  />
                  <textarea
                    className="storyboard-shot-prompt"
                    value={shot.prompt}
                    aria-label={`Shot ${i + 1} prompt`}
                    rows={2}
                    onChange={(e) =>
                      patchShot(i, { prompt: e.currentTarget.value })
                    }
                  />

                  <div className="storyboard-shot-row">
                    <label className="storyboard-shot-seconds">
                      <span className="storyboard-label">Seconds</span>
                      <input
                        type="number"
                        className="storyboard-seconds-input"
                        min={1}
                        value={shot.seconds}
                        aria-label={`Shot ${i + 1} seconds`}
                        onChange={(e) =>
                          patchShot(i, {
                            seconds: Number(e.currentTarget.value) || 0,
                          })
                        }
                      />
                    </label>
                    <label className="storyboard-shot-camera">
                      <span className="storyboard-label">Camera</span>
                      <select
                        className="storyboard-camera-select"
                        value={shot.camera ?? ""}
                        aria-label={`Shot ${i + 1} camera`}
                        onChange={(e) =>
                          patchShot(i, {
                            camera: e.currentTarget.value || null,
                          })
                        }
                      >
                        <option value="">None</option>
                        {cameraPresets.map((c) => (
                          <option key={c.id} value={c.id}>
                            {c.displayName}
                          </option>
                        ))}
                      </select>
                    </label>
                    <button
                      type="button"
                      className="storyboard-model"
                      onClick={() => setPickerShot(i)}
                    >
                      {modelName(shot.modelId)}
                    </button>
                  </div>

                  {(shot.advisories ?? []).map((a, k) => (
                    <p className="storyboard-advisory" key={k}>
                      {a}
                    </p>
                  ))}

                  <div className="storyboard-shot-foot">
                    <button
                      type="button"
                      className="storyboard-preview-open"
                      onClick={() => void openPreview(i)}
                    >
                      Preview prompt
                    </button>
                    {finalPrompts[i] !== undefined ? (
                      <span className="storyboard-previewed">edited ✓</span>
                    ) : null}
                    <span
                      className="storyboard-shot-cost"
                      data-unpriced={est === null || undefined}
                    >
                      {est === null ? "unpriced" : formatUsd(est.usd)}
                    </span>
                  </div>

                  {previewShot === i && preview ? (
                    <PromptPreview
                      preview={preview}
                      value={previewText}
                      onChange={setPreviewText}
                      onRetry={() => void openPreview(i)}
                      onGenerate={() => acceptPreview(i)}
                      previewing={previewing}
                      pending={false}
                      estimate={est}
                      blockedReason={
                        previewText.trim() ? null : "Add a prompt to keep"
                      }
                    />
                  ) : null}
                </li>
              );
            })}
          </ol>

          <div className="storyboard-add-row">
            <button
              type="button"
              className="storyboard-add"
              onClick={addShot}
            >
              + Add shot
            </button>
          </div>

          <footer className="storyboard-foot">
            <div className="storyboard-total-line">
              <span className="storyboard-total-label">Total</span>
              <span
                className="storyboard-total"
                data-unpriced={total.tone === "unpriced" || undefined}
              >
                {total.text}
              </span>
            </div>
            <button
              type="button"
              className="storyboard-generate"
              disabled={!canGenerate}
              onClick={() => void generate()}
            >
              {generating
                ? "Generating…"
                : `Generate all ${shots.length} shots`}
            </button>
          </footer>
        </>
      ) : null}

      <ModelPicker
        open={pickerShot !== null}
        onClose={() => setPickerShot(null)}
        models={models}
        selectedModelId={
          pickerShot !== null ? (shots[pickerShot]?.modelId ?? null) : null
        }
        selectedRouteId={null}
        onSelect={(m) => {
          if (pickerShot !== null) patchShot(pickerShot, { modelId: m });
          setPickerShot(null);
        }}
      />
    </section>
  );
}

/** Re-derive `role` and `continuity` from position after a reorder/add/delete. */
function renumber(shots: Shot[]): Shot[] {
  return shots.map((s, i) => ({
    ...s,
    role: i === 0 ? "start" : i === shots.length - 1 ? "end" : "middle",
    continuity: i === 0 ? null : { lastFrameOf: i - 1 },
  }));
}

/** The first usable route id for a model, for the shot's submit payload. */
function routeFor(models: Model[], modelId: string): string {
  const m = models.find((x) => x.id === modelId);
  const usable = m?.routes.find((r) => r.available !== false);
  return usable?.id ?? m?.routes[0]?.id ?? "";
}
