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
  imageModelAvailable = true,
  stillEstimate = null,
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
  /** S17: whether a runnable text-to-image model exists for the stills. The
   * Keyframe toggle enables only when true (always true today: qwen_image_3). */
  imageModelAvailable?: boolean;
  /** S17: the per-still cost, folded into the total shown before Generate.
   * `null` → unpriced (a GPT tier), which marks the whole total "unpriced". */
  stillEstimate?: CostEstimate | null;
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
    shotIndices?: number[];
    storyboardId?: string;
    chained?: boolean;
    autoRetry?: boolean;
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
  // The last generate-all outcome, kept when some shots failed so the user
  // sees which failed (and why) and can retry just those. `null` while a batch
  // fully succeeded (App switched to the Library) or nothing has run yet.
  const [outcome, setOutcome] = useState<StoryboardResult | null>(null);
  // S15: chain the shots (each continues from the previous shot's last frame).
  // Defaults on only when every shot's model can serve image-to-video; the user
  // can override, and a re-draft re-applies the default.
  const [chained, setChained] = useState(false);
  // S16 D9: opt into bounded auto-retry of a below-floor join (≤2 attempts,
  // each priced first). Off by default — nothing spends without an opt-in.
  const [autoRetry, setAutoRetry] = useState(false);
  // S17: keyframe-anchored mode — render one still per boundary and anchor every
  // shot on both ends. Off by default; a keyframe set is chained by nature.
  const [keyframeMode, setKeyframeMode] = useState(false);
  const canChain = shots.length > 1;
  const allChainable = canChain && shots.every((s) => s.chainable ?? false);
  // The Keyframe toggle enables only when every shot's model can bind an end
  // frame AND a runnable image model exists for the stills (D5) — so the toggle
  // default cannot drift from the scheduler's binding gate.
  const allEndFrameCapable =
    canChain && shots.every((s) => s.endFrameCapable ?? false);
  const canKeyframe = allEndFrameCapable && imageModelAvailable;

  /** Close any open inline preview — called on reorder/delete so a stale
   * preview cannot be accepted for the shot that now sits at that index. */
  const closePreview = useCallback(() => {
    setPreviewShot(null);
    setPreview(null);
    setPreviewText("");
  }, []);

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

  // The Chain toggle defaults on only when every shot can continue from a
  // frame; it follows the current plan (a re-draft re-applies the default).
  useEffect(() => {
    setChained(shots.length > 1 && shots.every((s) => s.chainable ?? false));
    // S17: keyframe mode is opt-in (off by default) and only offered when the
    // whole set can bind end frames; a re-draft that loses that capability
    // clears it so it cannot ride a stale plan.
    setKeyframeMode(false);
  }, [shots]);

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
      setOutcome(null);
      closePreview();
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

  const removeShot = useCallback(
    (i: number) => {
      setShots((prev) => renumber(prev.filter((_, idx) => idx !== i)));
      setFinalPrompts({});
      closePreview();
    },
    [closePreview],
  );

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

  const moveShot = useCallback(
    (i: number, dir: -1 | 1) => {
      setShots((prev) => {
        const j = i + dir;
        if (j < 0 || j >= prev.length) return prev;
        const next = [...prev];
        [next[i], next[j]] = [next[j], next[i]];
        return renumber(next);
      });
      setFinalPrompts({});
      closePreview();
    },
    [closePreview],
  );

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

  // S17: in keyframe mode the total the Generate button gates on includes the
  // N+1 stills (a still per shot boundary). An unpriced still (a GPT tier →
  // `stillEstimate === null`) marks the whole total "unpriced" — never $0.
  const stillCount = shots.length + 1;
  const totalEstimates =
    keyframeMode && canChain
      ? [...estimates, ...Array.from({ length: stillCount }, () => stillEstimate)]
      : estimates;
  const total = totalCostCaption(totalEstimates);
  const targetInvalid = !(targetSeconds > 0);
  const canGenerate =
    shots.length > 0 && estimates.length === shots.length && !generating;

  // The shots the last batch reported as failed (by their index in `shots`),
  // still editable in the cards above so the user can fix and retry just those.
  const failedIndices =
    outcome?.shots.filter((s) => s.error).map((s) => s.shotIndex) ?? [];
  // The retry total, shown before any resubmit so money honesty holds on retry.
  const retryTotal = totalCostCaption(failedIndices.map((i) => estimates[i] ?? null));

  // Submit the shots at `indices` (in order) through the money-gated bridge and
  // return the per-shot result. Shared by generate-all and retry-failed so both
  // run the identical estimate + S6 guard + S7 verbatim-prompt path per shot.
  //
  // The `planJson` always carries the FULL plan (every shot the user saw and
  // that validated), NEVER the submitted subset — the server re-validates the
  // whole plan (3..=8 shots, ±10% sum) and then matches each submitted shot to
  // its plan index. Sending a subset plan was the M2 bug: a sub-3-shot retry
  // tripped the server's count/sum gate and never resubmitted.
  //
  // On the first generate-all every shot is submitted in order, so the shots ARE
  // the plan and no `shotIndices`/`storyboardId` are sent (the server treats the
  // list positionally, exactly as before). On a RETRY only the failed subset is
  // submitted, so `retryStoryboardId` is passed: that makes the call send
  // `shotIndices` (the original plan index of each submitted shot) and write the
  // new job rows into the ORIGINAL group instead of minting a fresh one.
  const submitBatch = useCallback(
    async (indices: number[], retryStoryboardId?: string): Promise<StoryboardResult> => {
      const isRetry = retryStoryboardId !== undefined;
      const plan: StoryboardPlan = {
        brief,
        targetSeconds,
        useCase: tab,
        shots, // the FULL plan, not just `indices`
      };
      const shotInputs: SubmitInput[] = indices.map((i) => {
        const s = shots[i];
        return {
          modelId: s.modelId,
          routeId: routeFor(models, s.modelId),
          prompt: s.prompt,
          presetId: s.camera,
          settings: { ...settings, duration: s.seconds },
          media: [],
          rewriter: enhancer,
          finalPrompt: finalPrompts[i] ?? null,
        };
      });
      return onGenerate({
        brief,
        targetSeconds,
        planJson: JSON.stringify(plan),
        shots: shotInputs,
        shotIndices: isRetry ? indices : undefined,
        storyboardId: retryStoryboardId,
        // Chaining applies only to a first generate-all; a retry re-submits a
        // single failed shot and the runner's hook resumes the chain from it. A
        // keyframe set is chained by nature (S17).
        chained: isRetry ? undefined : chained || (keyframeMode && canKeyframe),
        // Auto-retry rides on a chained first generate-all only.
        autoRetry: isRetry ? undefined : (chained || keyframeMode) && autoRetry,
        // S17: keyframe-anchored mode + the still model (default qwen_image_3).
        keyframeMode: isRetry ? undefined : keyframeMode && canKeyframe,
      });
    },
    [shots, brief, targetSeconds, tab, models, settings, enhancer, finalPrompts, onGenerate, chained, autoRetry, keyframeMode, canKeyframe],
  );

  const generate = useCallback(async () => {
    if (shots.length === 0) return;
    setGenerating(true);
    setError(null);
    try {
      // The result's shotIndex is 0..shots.length-1, aligned with `shots`.
      const result = await submitBatch(shots.map((_, i) => i));
      const failed = result.shots.filter((s) => s.error);
      if (failed.length === 0) {
        // Every shot started — hand off to the Library.
        setOutcome(null);
        onGenerated(result);
      } else {
        // Keep the failed shots in view with their errors and a retry button.
        setOutcome(result);
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setGenerating(false);
    }
  }, [shots, submitBatch, onGenerated]);

  const retryFailed = useCallback(async () => {
    const indices = failedIndices;
    if (indices.length === 0) return;
    setGenerating(true);
    setError(null);
    try {
      // Resubmit ONLY the failed shots, into the ORIGINAL storyboard group, and
      // with the FULL plan so the server re-validates the whole plan (not the
      // failed subset). The server tags each retried shot with its original plan
      // index, so `retried.shots` already speak in the card numbering the user
      // sees — no re-basing needed.
      const retried = await submitBatch(indices, outcome?.storyboardId);
      // Merge: a previously-failed shot that now started replaces its old entry.
      const byIndex = new Map(retried.shots.map((s) => [s.shotIndex, s]));
      const merged: StoryboardResult = {
        storyboardId: outcome?.storyboardId ?? retried.storyboardId,
        shots: (outcome?.shots ?? []).map((s) => byIndex.get(s.shotIndex) ?? s),
      };
      if (merged.shots.every((s) => !s.error)) {
        setOutcome(null);
        onGenerated(merged);
      } else {
        setOutcome(merged);
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setGenerating(false);
    }
  }, [failedIndices, submitBatch, outcome, onGenerated]);

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
            disabled={drafting || !brief.trim() || targetInvalid}
            onClick={() => void draft()}
          >
            {drafting ? "Drafting…" : "Draft shot list"}
          </button>
        </div>
        {targetInvalid ? (
          <p className="storyboard-target-invalid" role="alert">
            Enter a target length of at least 1 second.
          </p>
        ) : null}
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

          {outcome && failedIndices.length > 0 ? (
            <section className="storyboard-outcome" role="alert">
              <p className="storyboard-outcome-summary">
                {outcome.shots.length - failedIndices.length} of{" "}
                {outcome.shots.length} shots submitted; {failedIndices.length}{" "}
                failed. Fix the shots below and retry just those.
              </p>
              <ul className="storyboard-outcome-fails">
                {outcome.shots
                  .filter((s) => s.error)
                  .map((s) => (
                    <li
                      className="storyboard-outcome-fail"
                      key={s.shotIndex}
                      data-shot-index={s.shotIndex}
                    >
                      <span className="storyboard-outcome-fail-n">
                        Shot {s.shotIndex + 1}
                      </span>
                      <span className="storyboard-outcome-fail-why">
                        {s.error}
                      </span>
                    </li>
                  ))}
              </ul>
              <div className="storyboard-outcome-retry-line">
                <span className="storyboard-outcome-retry-total">
                  Retry total: {retryTotal.text}
                </span>
                <button
                  type="button"
                  className="storyboard-retry"
                  disabled={generating}
                  onClick={() => void retryFailed()}
                >
                  {generating
                    ? "Retrying…"
                    : `Retry ${failedIndices.length} failed`}
                </button>
              </div>
            </section>
          ) : null}

          <footer className="storyboard-foot">
            {canChain ? (
              <div className="storyboard-chain">
                <label className="storyboard-chain-toggle">
                  <input
                    type="checkbox"
                    checked={chained}
                    aria-label="Chain shots with continuity"
                    onChange={(e) => setChained(e.currentTarget.checked)}
                  />
                  <span>Chain shots (continue each from the last frame)</span>
                </label>
                {!allChainable ? (
                  <p className="storyboard-chain-note">
                    Some shots' models can't continue from a frame — those shots
                    will be generated from the prompt.
                  </p>
                ) : null}
                {chained ? (
                  <label className="storyboard-autoretry-toggle">
                    <input
                      type="checkbox"
                      checked={autoRetry}
                      aria-label="Auto-retry low-continuity joins"
                      onChange={(e) => setAutoRetry(e.currentTarget.checked)}
                    />
                    <span>
                      Auto-retry a low-continuity join (up to 2×, each priced
                      first)
                    </span>
                  </label>
                ) : null}
                <label
                  className="storyboard-keyframe-toggle"
                  data-disabled={!canKeyframe || undefined}
                >
                  <input
                    type="checkbox"
                    checked={keyframeMode && canKeyframe}
                    disabled={!canKeyframe}
                    aria-label="Keyframe-anchor every shot on both ends"
                    onChange={(e) => setKeyframeMode(e.currentTarget.checked)}
                  />
                  <span>
                    Keyframe mode (a still per boundary, both ends anchored)
                  </span>
                </label>
                {!canKeyframe ? (
                  <p className="storyboard-keyframe-note">
                    {!imageModelAvailable
                      ? "No image model is available to render the stills."
                      : "Every shot's model must take an end frame to keyframe-anchor the set."}
                  </p>
                ) : null}
                {keyframeMode && canKeyframe ? (
                  <div className="storyboard-keyframe-cards">
                    {Array.from({ length: shots.length + 1 }, (_, k) => (
                      <div className="storyboard-keyframe-card" key={k}>
                        <span className="storyboard-keyframe-index">
                          Still {k + 1} of {shots.length + 1}
                        </span>
                        <span className="storyboard-keyframe-prompt">
                          {keyframeStillText(shots, k)}
                        </span>
                      </div>
                    ))}
                  </div>
                ) : null}
              </div>
            ) : null}
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

/** S17: the derived still text for keyframe boundary `k` (preview only). K0 is
 * shot 0's opening framing; K_{i+1} is shot i's ending hold clause — the same
 * derivation the server uses (a still per boundary, both ends anchored). */
function keyframeStillText(shots: Shot[], k: number): string {
  const holdClause = (action: string): string => {
    const lower = action.toLowerCase();
    const at = lower.lastIndexOf("hold of ");
    if (at >= 0) return action.slice(at + "hold of ".length).replace(/\.$/, "").trim();
    return action.trim();
  };
  if (k === 0) {
    const a = shots[0]?.action ?? shots[0]?.prompt ?? "";
    return a.split(".")[0]?.trim() || "opening frame";
  }
  const shot = shots[k - 1];
  const a = shot?.action ?? shot?.prompt ?? "";
  return holdClause(a) || "held frame";
}

/** The first usable route id for a model, for the shot's submit payload. */
function routeFor(models: Model[], modelId: string): string {
  const m = models.find((x) => x.id === modelId);
  const usable = m?.routes.find((r) => r.available !== false);
  return usable?.id ?? m?.routes[0]?.id ?? "";
}
