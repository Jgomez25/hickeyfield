import type { Model, ModelCapabilities, Route } from "../types";
import { gradientDataUri } from "../lib/placeholder";

/**
 * The composer's model tile.
 *
 * Restyled from a flat row into the mockup tile — a placeholder thumbnail, the
 * model name, a provider chip and a capability line — while keeping the
 * test-locked `.model-item` / `.model-item-name` / `.chip-route` classes AC5
 * pins. It carries `.model-item-composer` so the picker's 5-column grid rules
 * (scoped under `.model-list`) cannot reach it.
 *
 * The thumbnail is deterministic original art from `gradientDataUri` — models
 * carry no thumbnail URL and no provider snapshot is ever rendered here. Price
 * is deliberately not duplicated: the authoritative estimate lives on the
 * sticky Generate button, and a second figure would be one more thing to drift.
 */
export function ModelRow({
  model,
  route,
  capabilities,
  onOpen,
}: {
  model: Model | null;
  route: Route | null;
  capabilities?: ModelCapabilities;
  onOpen: () => void;
}) {
  const durations = capabilities?.durations ?? [];
  const cap =
    durations.length > 0 ? `up to ${Math.max(...durations)}s` : null;

  return (
    <button
      type="button"
      className="model-item model-item-composer"
      onClick={onOpen}
      aria-label="Change model"
    >
      <span
        className="model-item-thumb"
        aria-hidden="true"
        style={
          model
            ? { backgroundImage: `url("${gradientDataUri(model.id)}")` }
            : undefined
        }
      >
        <span className="media-bevel" />
      </span>
      <span className="model-item-main">
        <span className="model-item-name">
          {model?.displayName ?? "Select a model"}
        </span>
        <span className="model-item-meta">
          {route ? <span className="chip-route">{route.provider}</span> : null}
          {cap ? <span className="model-item-cap">{cap}</span> : null}
        </span>
      </span>
      <span className="model-item-change">Change</span>
    </button>
  );
}
