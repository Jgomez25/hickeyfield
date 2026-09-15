import { useEffect } from "react";
import type { RewriterChoice } from "../types";

/**
 * The enhancer backend + model control that rides next to the Enhance toggle.
 *
 * The Enhance *switch* lives in PromptCard; this only picks *what* rewrites the
 * prompt. Two honesty rules shape it:
 *
 * - It never offers a dead dropdown. When nothing can run — Ollama down and no
 *   OpenAI key — it renders the reason in words, matching the app's habit of
 *   explaining an unavailable choice rather than greying out a mystery control.
 * - It names no default hosted model. Hosted rosters churn, so OpenAI shows a
 *   free-text model field the user fills in, mirroring the shell's refusal to
 *   invent a model id.
 *
 * It is inert when Enhance is off: the rewrite will not run, so there is nothing
 * to configure.
 */
export function EnhancerPicker({
  enhance,
  ollamaUp,
  ollamaModels,
  openaiAvailable,
  value,
  onChange,
}: {
  enhance: boolean;
  ollamaUp: boolean;
  ollamaModels: string[];
  openaiAvailable: boolean;
  value: RewriterChoice | null;
  onChange: (next: RewriterChoice | null) => void;
}) {
  const hasOllama = ollamaUp && ollamaModels.length > 0;
  const hasOpenai = openaiAvailable;
  const firstModel = ollamaModels[0] ?? "";

  // Commit a concrete default so what the picker *shows* is what submit *sends*.
  // Without this the control renders "Local (Ollama) + first model" selected
  // while the submitted rewriter stays null — the exact mismatch that made the
  // enhancer look dead in a real key+Ollama config. Fires only when Enhance is
  // on, a backend is available, and the current value is missing or names a
  // backend that is no longer available; the guard makes it self-terminating
  // (no render loop) and re-resolving when availability changes.
  const currentBackendAvailable =
    (value?.backend === "ollama" && hasOllama) ||
    (value?.backend === "openai" && hasOpenai);
  useEffect(() => {
    if (!enhance) return;
    if (!hasOllama && !hasOpenai) return;
    if (value && currentBackendAvailable) return;
    if (hasOllama) onChange({ backend: "ollama", model: firstModel });
    else onChange({ backend: "openai", model: undefined });
  }, [
    enhance,
    hasOllama,
    hasOpenai,
    firstModel,
    value,
    currentBackendAvailable,
    onChange,
  ]);

  if (!enhance) return null;

  if (!hasOllama && !hasOpenai) {
    const reason =
      ollamaUp && ollamaModels.length === 0
        ? "Ollama is running but no chat model is installed. Run `ollama pull gemma3:1b`, or add an OpenAI key in Settings."
        : "Ollama isn't running and no OpenAI key is stored. Start Ollama and install a model, or add an OpenAI key in Settings.";
    return (
      <div className="enhancer-picker enhancer-picker-unavailable">
        <p className="enhancer-unavailable">
          Enhance is on, but there is no rewriter to run it. {reason} Your prompt
          will be sent exactly as you wrote it.
        </p>
      </div>
    );
  }

  // Which backend is active: the explicit choice, else the first available one.
  const backend = value?.backend ?? (hasOllama ? "ollama" : "openai");

  const pickBackend = (next: string) => {
    if (next === "ollama") {
      // Never carry a hosted model id over as an Ollama tag: keep the model
      // only if we were already on Ollama, otherwise resolve to the first
      // installed model.
      const model =
        value?.backend === "ollama" && value.model ? value.model : firstModel;
      onChange({ backend: "ollama", model });
    } else {
      // Likewise, only keep the model if we were already on OpenAI.
      const model = value?.backend === "openai" ? value.model : undefined;
      onChange({ backend: "openai", model });
    }
  };

  return (
    <div className="enhancer-picker">
      <label className="enhancer-field">
        <span className="enhancer-label">Enhancer</span>
        <select
          className="enhancer-backend"
          value={backend}
          onChange={(e) => pickBackend(e.currentTarget.value)}
        >
          {hasOllama ? <option value="ollama">Local (Ollama)</option> : null}
          {hasOpenai ? <option value="openai">OpenAI</option> : null}
        </select>
      </label>

      {backend === "ollama" && hasOllama ? (
        <label className="enhancer-field">
          <span className="enhancer-label">Model</span>
          <select
            className="enhancer-model"
            value={value?.model ?? firstModel}
            onChange={(e) =>
              onChange({ backend: "ollama", model: e.currentTarget.value })
            }
          >
            {ollamaModels.map((m) => (
              <option key={m} value={m}>
                {m}
              </option>
            ))}
          </select>
        </label>
      ) : null}

      {backend === "openai" ? (
        <label className="enhancer-field">
          <span className="enhancer-label">Model</span>
          <input
            className="enhancer-model"
            type="text"
            placeholder="e.g. gpt-5"
            value={value?.backend === "openai" ? (value.model ?? "") : ""}
            onChange={(e) =>
              onChange({ backend: "openai", model: e.currentTarget.value })
            }
          />
        </label>
      ) : null}
    </div>
  );
}
