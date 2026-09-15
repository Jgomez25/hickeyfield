import type { CostEstimate, PromptPreview as Preview } from "../types";
import { GenerateButton } from "./GenerateButton";
import { SparkleIcon } from "./Icons";

/**
 * The prompt-preview panel: what the enhancer produced, before any money is
 * spent.
 *
 * The quick Generate stays a silent enhance-and-submit. This is the opt-in
 * second path: the user sees the compiled wire prompt (camera clause + any
 * rewrite), can edit it, can Retry for a different rewrite, and then Generates
 * with *exactly* the shown text — sent verbatim as `finalPrompt`, never
 * re-enhanced.
 *
 * The editable box is seeded from `enhanced ?? prompt` by the caller; when the
 * rewrite did not run (enhance off, no backend, or an S6 refusal) that is the
 * original-plus-camera-clause, and the honest `note` says why — so the panel is
 * always something the user can read, edit, and generate, never a dead end.
 */
export function PromptPreview({
  preview,
  value,
  onChange,
  onRetry,
  onGenerate,
  previewing,
  pending,
  estimate,
  blockedReason,
}: {
  preview: Preview;
  /** The editable text — the exact string that will be sent as `finalPrompt`. */
  value: string;
  onChange: (next: string) => void;
  /** Re-run `preview_prompt` for a different rewrite. */
  onRetry: () => void;
  /** Submit `value` verbatim. */
  onGenerate: () => void;
  /** A preview fetch is in flight (Retry). */
  previewing: boolean;
  /** A generation is in flight. */
  pending: boolean;
  estimate: CostEstimate | null;
  /** Non-null blocks Generate (e.g. the edit was emptied). */
  blockedReason: string | null;
}) {
  const busy = previewing || pending;
  return (
    <section className="prompt-preview" aria-label="Prompt preview">
      <header className="prompt-preview-head">
        <SparkleIcon size={14} className="prompt-preview-icon" />
        <span className="prompt-preview-title">Preview</span>
      </header>

      <label className="prompt-preview-field">
        <span className="prompt-preview-label">
          Prompt to send — edit freely
        </span>
        <textarea
          className="prompt-preview-text"
          value={value}
          onChange={(e) => onChange(e.currentTarget.value)}
          rows={5}
          disabled={busy}
          spellCheck
        />
      </label>

      <div className="prompt-preview-original">
        <span className="prompt-preview-label">Your original</span>
        <p className="prompt-preview-original-body">{preview.original}</p>
      </div>

      {preview.note ? (
        <p className="prompt-preview-note">{preview.note}</p>
      ) : null}

      <div className="prompt-preview-actions">
        <button
          type="button"
          className="prompt-preview-retry"
          onClick={onRetry}
          disabled={busy}
        >
          {previewing ? "Retrying…" : "Retry for a different rewrite"}
        </button>
        <span className="prompt-preview-caveat">
          Rewrites vary between runs.
        </span>
      </div>

      <GenerateButton
        estimate={estimate}
        pending={pending}
        blockedReason={blockedReason}
        onSubmit={onGenerate}
      />
    </section>
  );
}
