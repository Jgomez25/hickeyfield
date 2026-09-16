import type {
  CostEstimate,
  GenSettings,
  LocalModel,
  MediaRef,
  MediaRole,
  Model,
  ModelCapabilities,
  PresetFamily,
  PromptPreview as Preview,
  RewriterChoice,
  Route,
  UseCase,
  WorkspaceTab,
} from "../types";
import { useState } from "react";
import { PresetTile } from "./PresetTile";
import { Dropzone, FrameSlots, type FrameSlot } from "./MediaInputs";
import { PromptCard } from "./PromptCard";
import { EnhancerPicker } from "./EnhancerPicker";
import { ModelRow } from "./ModelRow";
import { CameraPreview } from "./CameraPreview";
import { ChipRow } from "./ChipRow";
import { GenerateButton } from "./GenerateButton";
import { PromptPreview } from "./PromptPreview";

const PANEL_ID = "generator-panel";

/** How many camera moves the strip shows before the "+N more" tile. */
const CAMERA_STRIP_LIMIT = 3;

/**
 * One camera-move tile in the composer strip. Tracks its own hover/focus so the
 * CSS-transform preview plays (the same `data-active` mechanism the preset
 * picker uses), and picks the preset on click.
 */
function CameraStripTile({
  preset,
  active,
  onPick,
}: {
  preset: PresetFamily;
  active: boolean;
  onPick: () => void;
}) {
  const [hot, setHot] = useState(false);
  return (
    <button
      type="button"
      className="camera-strip-tile"
      data-active={active || undefined}
      onClick={onPick}
      onMouseEnter={() => setHot(true)}
      onMouseLeave={() => setHot(false)}
      onFocus={() => setHot(true)}
      onBlur={() => setHot(false)}
      aria-pressed={active}
      aria-label={`Camera move ${preset.displayName}`}
    >
      <CameraPreview slug={preset.id} active={hot} />
      <span className="media-bevel" aria-hidden="true" />
      <span className="camera-strip-name">{preset.displayName}</span>
    </button>
  );
}

/**
 * Human labels for the media roles. The *slots* now come from the use case in
 * Rust, so a tab cannot render a slot the job does not use — the previous
 * hardcoded table gave "Edit Video" a Start Frame it never wanted.
 */
const ROLE_LABELS: Record<string, string> = {
  start: "Start Frame",
  end: "End Frame",
  reference: "Reference",
  video: "Source Clip",
  video_reference: "Motion Reference",
  audio: "Audio",
  audio_reference: "Voice Reference",
};

function slotsFor(
  useCase: UseCase | undefined,
  unsupported: string[] | undefined,
): FrameSlot[] {
  if (!useCase) return [];
  // The use case says which slots the *job* has; capabilities say which of
  // them the chosen model+route can actually hold. Both are needed: the job
  // decides the shape, the model decides what is fillable.
  const off = new Set(unsupported ?? []);
  return useCase.slots.map(([role, required]) => ({
    role: role as FrameSlot["role"],
    label: ROLE_LABELS[role] ?? role,
    required,
    unsupported: off.has(role),
  }));
}

export function SettingsRail({
  useCases,
  tab,
  preset,
  model,
  route,
  capabilities,
  settings,
  onSettingsChange,
  prompt,
  onPromptChange,
  media,
  onMediaAdd,
  onMediaRemoveRole,
  onMediaRemoveKey,
  ollamaUp,
  ollamaModels,
  openaiAvailable,
  enhancer,
  onEnhancerChange,
  estimate,
  pending,
  needsSetup,
  onOpenPresets,
  onOpenModels,
  onOpenSetup,
  onSubmit,
  cameraPresets,
  onPickPreset,
  preview,
  previewText,
  onPreviewTextChange,
  previewing,
  onPreview,
  onRetryPreview,
  onGenerateFromPreview,
}: {
  useCases: UseCase[];
  /** The active use-case slug — drives which frame slots the composer shows.
   * Selection itself now lives in the 72px nav (WorkspaceTabs), not here. */
  tab: WorkspaceTab;
  preset: PresetFamily | null;
  model: Model | null;
  route: Route | null;
  capabilities: ModelCapabilities;
  settings: GenSettings;
  onSettingsChange: (patch: Partial<GenSettings>) => void;
  prompt: string;
  onPromptChange: (next: string) => void;
  media: MediaRef[];
  onMediaAdd: (role: MediaRole, files: FileList | null) => void | Promise<void>;
  onMediaRemoveRole: (role: MediaRole) => void;
  onMediaRemoveKey: (key: string) => void;
  /** Whether the local Ollama daemon is up, its installed chat models, and
   * whether an OpenAI key is stored — the enhancer picker's availability. */
  ollamaUp: boolean;
  ollamaModels: LocalModel[];
  openaiAvailable: boolean;
  /** The explicit enhancer choice (null = auto). */
  enhancer: RewriterChoice | null;
  onEnhancerChange: (next: RewriterChoice | null) => void;
  estimate: CostEstimate | null;
  pending: boolean;
  /** No usable provider — the submit control becomes the way to fix that. */
  needsSetup: boolean;
  onOpenPresets: () => void;
  onOpenModels: () => void;
  onOpenSetup: () => void;
  onSubmit: () => void;
  /** Camera-control presets for the strip; "+N more" opens the full picker. */
  cameraPresets: PresetFamily[];
  onPickPreset: (preset: PresetFamily) => void;
  /** The current preview, or null when the panel is closed. */
  preview: Preview | null;
  /** The editable preview text — what Generate-from-preview sends verbatim. */
  previewText: string;
  onPreviewTextChange: (next: string) => void;
  /** A preview fetch is in flight (initial Preview or Retry). */
  previewing: boolean;
  onPreview: () => void;
  onRetryPreview: () => void;
  onGenerateFromPreview: () => void;
}) {
  // Chain presets carry their own prompt body; typing into the box would be
  // rejected by the provider, so the box is disabled rather than silently
  // ignored.
  const chained = Boolean(preset?.tags.includes("chain"));
  const blockedReason = chained
    ? media.length === 0
      ? "This preset needs exactly one image"
      : null
    : prompt.trim().length < 2
      ? "Add a prompt before starting generation"
      : null;

  return (
    <aside className="rail rail-settings" aria-label="Generation settings">
      <div className="rail-card">
        <div className="rail-stack" id={PANEL_ID} role="tabpanel">
          <PresetTile
            preset={preset}
            modelId={model?.id ?? null}
            modelName={model?.displayName ?? "No model"}
            onChange={onOpenPresets}
          />

          <FrameSlots
            slots={slotsFor(
              useCases.find((u) => u.slug === tab),
              capabilities?.unsupportedRoles,
            )}
            media={media}
            onAdd={onMediaAdd}
            onRemove={onMediaRemoveRole}
          />

          <Dropzone
            media={media}
            onAdd={onMediaAdd}
            onRemove={onMediaRemoveKey}
          />

          <PromptCard
            value={prompt}
            onChange={onPromptChange}
            audio={settings.audio}
            audioSupported={capabilities.audio}
            onAudioChange={(audio) => onSettingsChange({ audio })}
            enhance={settings.enhance}
            onEnhanceChange={(enhance) => onSettingsChange({ enhance })}
            disabled={chained}
            disabledReason="This preset writes its own prompt. Supply one image instead."
          />

          <EnhancerPicker
            enhance={settings.enhance}
            ollamaUp={ollamaUp}
            ollamaModels={ollamaModels}
            openaiAvailable={openaiAvailable}
            value={enhancer}
            onChange={onEnhancerChange}
          />

          <ModelRow
            model={model}
            route={route}
            capabilities={capabilities}
            onOpen={onOpenModels}
          />

          {cameraPresets.length > 0 ? (
            <div className="camera-strip">
              <span className="camera-strip-label">Camera move</span>
              <div className="camera-strip-grid">
                {cameraPresets.slice(0, CAMERA_STRIP_LIMIT).map((p) => (
                  <CameraStripTile
                    key={p.id}
                    preset={p}
                    active={preset?.id === p.id}
                    onPick={() => onPickPreset(p)}
                  />
                ))}
                <button
                  type="button"
                  className="camera-strip-more"
                  onClick={onOpenPresets}
                >
                  {cameraPresets.length > CAMERA_STRIP_LIMIT
                    ? `+${cameraPresets.length - CAMERA_STRIP_LIMIT} more`
                    : "More"}
                </button>
              </div>
            </div>
          ) : null}

          <ChipRow
            capabilities={capabilities}
            settings={settings}
            onChange={onSettingsChange}
          />

          <div className="rail-generate-sticky">
            <GenerateButton
              estimate={estimate}
              pending={pending}
              blockedReason={blockedReason}
              needsSetup={needsSetup}
              onSubmit={onSubmit}
              onSetup={onOpenSetup}
            />
          </div>

          {!needsSetup ? (
            <button
              type="button"
              className="preview-trigger"
              onClick={onPreview}
              disabled={Boolean(blockedReason) || pending || previewing}
            >
              {previewing && !preview
                ? "Preparing preview…"
                : "Preview prompt"}
            </button>
          ) : null}

          {preview ? (
            <PromptPreview
              preview={preview}
              value={previewText}
              onChange={onPreviewTextChange}
              onRetry={onRetryPreview}
              onGenerate={onGenerateFromPreview}
              previewing={previewing}
              pending={pending}
              estimate={estimate}
              blockedReason={
                previewText.trim().length < 2
                  ? "Add a prompt before generating"
                  : null
              }
            />
          ) : null}

          <p className="rail-disclaimer">
            Independent open-source project. Not affiliated with, endorsed by,
            or sponsored by Higgsfield, Inc.
          </p>
        </div>
      </div>
    </aside>
  );
}
