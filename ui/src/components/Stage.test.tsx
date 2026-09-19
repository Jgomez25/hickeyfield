import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, type ReactElement } from "react";
import { createRoot, type Root } from "react-dom/client";
import { Stage } from "./Stage";
import type { JobSet } from "../types";

/**
 * AC4 (S12). The Studio stage: the latest/selected result large with a caption,
 * action pills wired only when honest, a recents strip that keeps the
 * `data-job-anchor` selection pairing, a live timer while running, and an empty
 * state. No RTL — direct react-dom/client under jsdom.
 */

(globalThis as unknown as { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

function job(overrides: Partial<JobSet>): JobSet {
  return {
    id: "j1",
    modelId: "minimax_h3_max_turbo",
    status: "completed",
    prompt: "A lighthouse keeper climbs the stairs at dusk",
    createdAt: new Date(Date.now() - 41_000).toISOString(),
    // Image results, not video: jsdom has no HTMLMediaElement.play, so the repo
    // renders the <img> branch in tests (the same limitation ResultCard's suite
    // works around). The real app renders the clip through VideoPreview.
    results: [{ url: "https://x/y.png", kind: "image", localPath: "/lib/y.png" }],
    route: { id: "fal:x", provider: "fal", slug: "x" },
    estimatedUsd: 0.43,
    actualUsd: 0.41,
    settings: {
      duration: 10,
      resolution: "1080p",
      aspect: "16:9",
      audio: false,
      enhance: true,
    },
    ...overrides,
  };
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
  act(() => root.render(node));
}

const noop = () => {};

describe("Stage", () => {
  it("renders the featured job large with its caption and the recents strip", () => {
    const j = job({});
    render(
      <Stage
        job={j}
        recents={[j]}
        modelName="H3 Max Turbo"
        onSelect={noop}
        onRetry={noop}
        onExtend={noop}
        onReveal={noop}
      />,
    );
    expect(container.querySelector(".stage-hero")).not.toBeNull();
    expect(container.textContent).toContain("A lighthouse keeper");
    // Caption carries model · duration · resolution · est→actual.
    expect(container.textContent).toContain("H3 Max Turbo");
    expect(container.textContent).toContain("est. $0.43 → actual $0.41");
    // Recents tiles keep the data-job-anchor the pairing effect scrolls to.
    const tiles = container.querySelectorAll(".stage-recent-tile[data-job-anchor]");
    expect(tiles.length).toBe(1);
  });

  it("shows a mm:ss timer while a job is running", () => {
    render(
      <Stage
        job={job({ status: "in_progress" })}
        recents={[]}
        modelName="H3 Max Turbo"
        onSelect={noop}
        onRetry={noop}
        onExtend={noop}
        onReveal={noop}
      />,
    );
    const timer = container.querySelector(".stage-timer");
    expect(timer).not.toBeNull();
    expect(timer!.textContent).toMatch(/Rendering \d\d:\d\d/);
  });

  it("hides Extend and Reveal when there is no saved file, keeps Retry", () => {
    const onExtend = vi.fn();
    render(
      <Stage
        job={job({ results: [{ url: "https://x/y.png", kind: "image" }] })}
        recents={[]}
        modelName="H3 Max Turbo"
        onSelect={noop}
        onRetry={noop}
        onExtend={onExtend}
        onReveal={noop}
      />,
    );
    const pills = Array.from(container.querySelectorAll(".stage-pill")).map(
      (p) => p.textContent,
    );
    expect(pills).toContain("Retry");
    expect(pills).not.toContain("Extend");
    expect(pills).not.toContain("Reveal");
  });

  it("hides the Extend pill when no extend use case is available (onExtend null)", () => {
    render(
      <Stage
        job={job({})}
        recents={[]}
        modelName="H3 Max Turbo"
        onSelect={noop}
        onRetry={noop}
        onExtend={null}
        onReveal={noop}
      />,
    );
    const pills = Array.from(container.querySelectorAll(".stage-pill")).map(
      (p) => p.textContent,
    );
    expect(pills).not.toContain("Extend");
    // A saved file still offers Reveal.
    expect(pills).toContain("Reveal");
  });

  it("shows an empty state when nothing has been generated", () => {
    render(
      <Stage
        job={null}
        recents={[]}
        modelName=""
        onSelect={noop}
        onRetry={noop}
        onExtend={null}
        onReveal={noop}
      />,
    );
    expect(container.querySelector(".empty-state")).not.toBeNull();
    expect(container.textContent).toContain("NOTHING GENERATED YET");
  });
});
