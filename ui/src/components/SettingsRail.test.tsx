import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, type ReactElement } from "react";
import { createRoot, type Root } from "react-dom/client";
import { SettingsRail } from "./SettingsRail";
import type {
  GenSettings,
  Model,
  ModelCapabilities,
  PresetFamily,
  Route,
} from "../types";

/**
 * AC5 (S12). The composer keeps the test-locked `.generate-button` and the
 * `.model-item*` classes (now a tile), shows a camera strip (≤3 moves + a
 * "more" affordance), and keeps the legal notice visible at its foot. The
 * enhancer/preview children are unchanged DOM, so their own suites stand.
 * No RTL — direct react-dom/client under jsdom.
 */

(globalThis as unknown as { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

const CAPS: ModelCapabilities = {
  supportsDuration: true,
  durations: [5, 10],
  supportsResolution: true,
  resolutions: ["720p", "1080p"],
  supportsAspect: true,
  aspects: ["16:9"],
  audio: false,
  constraints: [],
  unsupportedRoles: [],
};

const ROUTE: Route = { id: "fal:seedance", provider: "fal", slug: "seedance", available: true };

const MODEL: Model = {
  id: "seedance_2_5",
  displayName: "Seedance 2.5",
  modality: "video",
  isLaunch: true,
  routes: [ROUTE],
  capabilities: CAPS,
};

const SETTINGS: GenSettings = {
  duration: 10,
  resolution: "1080p",
  aspect: "16:9",
  audio: false,
  enhance: true,
};

function camera(id: string, name: string): PresetFamily {
  return { id, displayName: name, category: "camera-control", tags: [], description: "" };
}

const CAMERA_PRESETS: PresetFamily[] = [
  camera("dolly-in", "Dolly in"),
  camera("drone-orbit", "Orbit"),
  camera("crane-up", "Crane up"),
  camera("pan-left", "Pan left"),
];

let container: HTMLDivElement;
let root: Root;

beforeEach(() => {
  container = document.createElement("div");
  document.body.appendChild(container);
  root = createRoot(container);
});

afterEach(() => {
  act(() => root.unmount());
  container.remove();
});

function render(node: ReactElement) {
  act(() => root.render(node));
}

const noop = () => {};

function props() {
  return {
    useCases: [],
    tab: "text-to-video",
    preset: null,
    model: MODEL,
    route: ROUTE,
    capabilities: CAPS,
    settings: SETTINGS,
    onSettingsChange: noop,
    prompt: "a lighthouse",
    onPromptChange: noop,
    media: [],
    onMediaAdd: noop,
    onMediaRemoveRole: noop,
    onMediaRemoveKey: noop,
    ollamaUp: false,
    ollamaModels: [],
    openaiAvailable: false,
    enhancer: null,
    onEnhancerChange: noop,
    estimate: { usd: 0.43, basis: "10s at 1080p" },
    pending: false,
    needsSetup: false,
    onOpenPresets: noop,
    onOpenModels: noop,
    onOpenSetup: noop,
    onSubmit: noop,
    cameraPresets: CAMERA_PRESETS,
    onPickPreset: vi.fn(),
    preview: null,
    previewText: "",
    onPreviewTextChange: noop,
    previewing: false,
    onPreview: noop,
    onRetryPreview: noop,
    onGenerateFromPreview: noop,
  };
}

describe("SettingsRail composer (S12)", () => {
  it("keeps the sticky priced Generate button", () => {
    render(<SettingsRail {...props()} />);
    const gen = container.querySelector(".generate-button");
    expect(gen).not.toBeNull();
    expect(container.querySelector(".rail-generate-sticky")).not.toBeNull();
  });

  it("renders the model as a tile keeping the pinned .model-item* classes", () => {
    render(<SettingsRail {...props()} />);
    const tile = container.querySelector(".model-item.model-item-composer");
    expect(tile).not.toBeNull();
    expect(tile!.querySelector(".model-item-name")?.textContent).toContain(
      "Seedance 2.5",
    );
    // Provider chip keeps the .chip-route class; capability line is derived.
    expect(tile!.querySelector(".chip-route")?.textContent).toContain("fal");
    expect(tile!.textContent).toContain("up to 10s");
  });

  it("shows a camera strip of at most three moves plus a 'more' affordance", () => {
    render(<SettingsRail {...props()} />);
    const frames = container.querySelectorAll(".camera-strip .cam-frame");
    expect(frames.length).toBeLessThanOrEqual(3);
    expect(frames.length).toBe(3);
    const more = container.querySelector(".camera-strip-more");
    expect(more).not.toBeNull();
    expect(more!.textContent).toContain("more");
  });

  it("keeps the legal notice visible at the composer foot", () => {
    render(<SettingsRail {...props()} />);
    const note = container.querySelector(".rail-disclaimer");
    expect(note).not.toBeNull();
    expect(note!.textContent).toContain(
      "Not affiliated with, endorsed by, or sponsored by Higgsfield",
    );
  });
});
