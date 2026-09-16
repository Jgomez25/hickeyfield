import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, type ReactElement } from "react";
import { createRoot, type Root } from "react-dom/client";
import { Storyboard } from "./Storyboard";
import type { CostEstimate, Model, Shot, StoryboardPlan } from "../types";

/**
 * S13 (AC3). The Storyboard view must render editable shot cards from a draft,
 * re-sum the total when a shot changes, and mark the total "unpriced" when any
 * shot has no published price (never $0). Rendered directly with
 * react-dom/client under jsdom — the same no-RTL pattern the other tests use.
 * The drafting/estimate/generate calls are injected, so nothing touches a bridge.
 */

(
  globalThis as unknown as { IS_REACT_ACT_ENVIRONMENT: boolean }
).IS_REACT_ACT_ENVIRONMENT = true;

const MODEL: Model = {
  id: "kling_3_0",
  displayName: "Kling 3.0",
  modality: "video",
  isLaunch: true,
  routes: [{ id: "fal:kling", provider: "fal", slug: "kling", available: true }],
};

const SETTINGS = {
  duration: 5,
  resolution: "1080p",
  aspect: "16:9",
  audio: false,
  enhance: true,
};

function planOf(seconds: number[]): StoryboardPlan {
  const shots: Shot[] = seconds.map((s, i) => ({
    title: `Shot ${i + 1}`,
    prompt: `beat ${i + 1}`,
    seconds: s,
    camera: null,
    modelId: "kling_3_0",
    role: i === 0 ? "start" : i === seconds.length - 1 ? "end" : "middle",
    continuity: i === 0 ? null : { lastFrameOf: i - 1 },
    advisories: [],
  }));
  return { brief: "a 30s ad", targetSeconds: 30, useCase: "text-to-video", shots };
}

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
  act(() => {
    root.render(node);
  });
}

// Flush pending microtasks (the async estimate effect) inside act.
async function flush() {
  await act(async () => {
    await Promise.resolve();
    await Promise.resolve();
  });
}

function typeInto(el: HTMLInputElement | HTMLTextAreaElement, value: string) {
  const proto =
    el instanceof HTMLTextAreaElement
      ? HTMLTextAreaElement.prototype
      : HTMLInputElement.prototype;
  const setter = Object.getOwnPropertyDescriptor(proto, "value")!.set!;
  act(() => {
    setter.call(el, value);
    el.dispatchEvent(new Event("input", { bubbles: true }));
  });
}

function base(overrides: Partial<Parameters<typeof Storyboard>[0]> = {}) {
  return {
    tab: "text-to-video",
    models: [MODEL],
    cameraPresets: [],
    settings: SETTINGS,
    enhancer: null,
    onPlan: vi.fn(async () => planOf([8, 8, 8, 6])),
    // A per-second price so an edit visibly moves the total.
    onEstimate: vi.fn(
      async (shot: Shot): Promise<CostEstimate> => ({
        usd: shot.seconds * 0.1,
        basis: "",
      }),
    ),
    onPreview: vi.fn(async () => ({
      prompt: "p",
      original: "p",
      enhanced: null,
      version: null,
      note: null,
    })),
    onGenerate: vi.fn(async () => ({ storyboardId: "sb", shots: [] })),
    onGenerated: vi.fn(),
    ...overrides,
  };
}

describe("Storyboard", () => {
  it("draft renders cards and editing a shot updates the total", async () => {
    const props = base();
    render(<Storyboard {...props} />);

    // Draft needs a non-empty brief.
    typeInto(
      container.querySelector<HTMLTextAreaElement>("textarea.storyboard-brief")!,
      "a 30s ad for a cold-brew can",
    );
    await act(async () => {
      container.querySelector<HTMLButtonElement>("button.storyboard-draft")!.click();
    });
    await flush();

    const cards = container.querySelectorAll(".storyboard-shot[data-shot-index]");
    expect(cards.length).toBe(4);

    // 8+8+8+6 = 30s → $3.00 at $0.10/s.
    const total = container.querySelector(".storyboard-total")!;
    expect(total.textContent).toBe("$3.00");

    // Bump shot 1 from 8s to 18s → total 40s → $4.00.
    const seconds = container.querySelector<HTMLInputElement>(
      ".storyboard-shot[data-shot-index='0'] .storyboard-seconds-input",
    )!;
    typeInto(seconds, "18");
    await flush();
    expect(container.querySelector(".storyboard-total")!.textContent).toBe("$4.00");
  });

  it("an unpriced shot marks the total", async () => {
    const props = base({
      onPlan: vi.fn(async () => planOf([10, 10, 10])),
      // The middle shot has no published price.
      onEstimate: vi.fn(async (shot: Shot): Promise<CostEstimate | null> =>
        shot.title === "Shot 2" ? null : { usd: 1, basis: "" },
      ),
    });
    render(<Storyboard {...props} />);
    typeInto(
      container.querySelector<HTMLTextAreaElement>("textarea.storyboard-brief")!,
      "brief",
    );
    await act(async () => {
      container.querySelector<HTMLButtonElement>("button.storyboard-draft")!.click();
    });
    await flush();

    const total = container.querySelector<HTMLElement>(".storyboard-total")!;
    expect(total.dataset.unpriced).toBe("true");
    expect(total.textContent).toContain("unpriced");
    expect(total.textContent).not.toContain("$0.00");
  });

  it("does not draft with an empty brief", () => {
    const props = base();
    render(<Storyboard {...props} />);
    const draft = container.querySelector<HTMLButtonElement>(
      "button.storyboard-draft",
    )!;
    expect(draft.disabled).toBe(true);
  });
});
