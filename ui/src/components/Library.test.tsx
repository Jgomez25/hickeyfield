import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, type ReactElement } from "react";
import { createRoot, type Root } from "react-dom/client";
import { Library } from "./Library";
import type { JobSet, Storyboard, StoryboardRender } from "../types";

/**
 * AC6 (S12). The Library gallery over the same `jobs` array (no new command):
 * 16:9 tiles with a cost-or-"unpriced" caption and the `data-job-anchor` the
 * meta pairing needs, a click that selects the job and opens the lightbox, and
 * client-side filters. No RTL — direct react-dom/client under jsdom.
 */

(globalThis as unknown as { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

function job(overrides: Partial<JobSet>): JobSet {
  return {
    id: "j",
    modelId: "seedance_2_5",
    status: "completed",
    prompt: "A lighthouse at dusk",
    createdAt: new Date().toISOString(),
    results: [{ url: "https://x/y.mp4", kind: "video", localPath: "/lib/y.mp4" }],
    route: { id: "fal:x", provider: "fal", slug: "x" },
    estimatedUsd: 0.41,
    actualUsd: null,
    ...overrides,
  };
}

const JOBS: JobSet[] = [
  job({ id: "priced", prompt: "Lighthouse at dusk", estimatedUsd: 0.41 }),
  job({ id: "unpriced", prompt: "Station clock", estimatedUsd: null, actualUsd: null }),
  job({
    id: "failed",
    prompt: "Refused by provider",
    status: "failed",
    results: [],
    estimatedUsd: null,
  }),
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

function chip(label: string): HTMLButtonElement {
  const found = Array.from(
    container.querySelectorAll<HTMLButtonElement>(".library-filters .chip-button"),
  ).find((b) => b.textContent === label);
  if (!found) throw new Error(`no filter chip ${label}`);
  return found;
}

describe("Library", () => {
  it("renders jobs as tiles with cost/unpriced captions and data-job-anchor", () => {
    render(
      <Library jobs={JOBS} selectedId={null} onSelect={noop} onOpenLightbox={noop} />,
    );
    const tiles = container.querySelectorAll(".library-tile[data-job-anchor]");
    expect(tiles.length).toBe(3);
    expect(container.textContent).toContain("$0.41");
    // The unpriced tile says so in words, never $0.00.
    const unpriced = container.querySelector('[data-job-anchor="unpriced"]')!;
    expect(unpriced.textContent).toContain("unpriced");
    expect(unpriced.textContent).not.toContain("$0.00");
  });

  it("selects the job and opens the lightbox on click", () => {
    const onSelect = vi.fn();
    const onOpenLightbox = vi.fn();
    render(
      <Library
        jobs={JOBS}
        selectedId={null}
        onSelect={onSelect}
        onOpenLightbox={onOpenLightbox}
      />,
    );
    const tile = container.querySelector<HTMLButtonElement>(
      '[data-job-anchor="priced"]',
    )!;
    act(() => tile.click());
    expect(onSelect).toHaveBeenCalledWith("priced");
    expect(onOpenLightbox).toHaveBeenCalledWith("priced");
  });

  it("the Failed filter shows only failed jobs", () => {
    render(
      <Library jobs={JOBS} selectedId={null} onSelect={noop} onOpenLightbox={noop} />,
    );
    act(() => chip("Failed").click());
    const tiles = container.querySelectorAll(".library-tile[data-job-anchor]");
    expect(tiles.length).toBe(1);
    expect(container.querySelector('[data-job-anchor="failed"]')).not.toBeNull();
    expect(container.querySelector('[data-job-anchor="priced"]')).toBeNull();
  });

  // S15 AC5/AC6: the storyboard set item's render controls.
  const SET: Storyboard = {
    id: "sb1",
    brief: "a 30s ad",
    targetSeconds: 30,
    planJson: "{}",
    createdAt: 0,
  };

  function shot(idx: number, done: boolean): JobSet {
    return job({
      id: `sb1-${idx}`,
      storyboardId: "sb1",
      shotIndex: idx,
      status: done ? "completed" : "in_progress",
      results: done
        ? [{ url: "https://x/s.mp4", kind: "video", localPath: `/lib/s${idx}.mp4` }]
        : [],
      actualUsd: done ? 1.2 : null,
    });
  }

  it("the Render button is disabled until every shot is Done, then enabled", () => {
    // One shot still running → disabled.
    render(
      <Library
        jobs={[shot(0, true), shot(1, false)]}
        selectedId={null}
        onSelect={noop}
        onOpenLightbox={noop}
        storyboards={[SET]}
        onRender={vi.fn()}
      />,
    );
    let btn = container.querySelector<HTMLButtonElement>(".storyboard-render")!;
    expect(btn).not.toBeNull();
    expect(btn.disabled).toBe(true);

    // Both done → enabled, and clicking asks to render this set.
    const onRender = vi.fn();
    render(
      <Library
        jobs={[shot(0, true), shot(1, true)]}
        selectedId={null}
        onSelect={noop}
        onOpenLightbox={noop}
        storyboards={[SET]}
        onRender={onRender}
      />,
    );
    btn = container.querySelector<HTMLButtonElement>(".storyboard-render")!;
    expect(btn.disabled).toBe(false);
    act(() => btn.click());
    expect(onRender).toHaveBeenCalledWith("sb1");
    // Money honesty: the panel says rendering is free.
    expect(container.textContent).toContain("Rendering is free");
  });

  it("a render event advances the progress bar", () => {
    render(
      <Library
        jobs={[shot(0, true), shot(1, true)]}
        selectedId={null}
        onSelect={noop}
        onOpenLightbox={noop}
        storyboards={[SET]}
        onRender={vi.fn()}
        renderProgress={{ sb1: { storyboardId: "sb1", percent: 0.42, done: false } }}
      />,
    );
    const bar = container.querySelector<HTMLElement>(".storyboard-render-progress")!;
    expect(bar.getAttribute("role")).toBe("progressbar");
    expect(bar.getAttribute("aria-valuenow")).toBe("42");
  });

  it("a finished render shows the sum actual and Open result", () => {
    const onOpenResult = vi.fn();
    const render1: StoryboardRender = {
      storyboardId: "sb1",
      outputPath: "/lib/day-0/ad.mp4",
      width: 1280,
      height: 720,
      seconds: 30,
      actualUsd: 2.4,
      createdAt: 0,
    };
    render(
      <Library
        jobs={[shot(0, true), shot(1, true)]}
        selectedId={null}
        onSelect={noop}
        onOpenLightbox={noop}
        storyboards={[SET]}
        onRender={vi.fn()}
        onOpenResult={onOpenResult}
        renders={[render1]}
      />,
    );
    expect(container.textContent).toContain("$2.40");
    const open = container.querySelector<HTMLButtonElement>(".storyboard-open-result")!;
    act(() => open.click());
    expect(onOpenResult).toHaveBeenCalledWith("/lib/day-0/ad.mp4");
  });

  it("an unpriced member makes the render total say unpriced, never $0", () => {
    const render1: StoryboardRender = {
      storyboardId: "sb1",
      outputPath: "/lib/day-0/ad.mp4",
      width: 1280,
      height: 720,
      seconds: 30,
      actualUsd: null,
      createdAt: 0,
    };
    render(
      <Library
        jobs={[shot(0, true), shot(1, true)]}
        selectedId={null}
        onSelect={noop}
        onOpenLightbox={noop}
        storyboards={[SET]}
        onRender={vi.fn()}
        renders={[render1]}
      />,
    );
    const cost = container.querySelector(".storyboard-result-cost")!;
    expect(cost.textContent).toContain("unpriced");
    expect(cost.textContent).not.toContain("$0.00");
  });

  it("shows the set's minimum continuity-join score and flags a low join (S16)", () => {
    const scored: Storyboard = {
      ...SET,
      joins: [
        {
          index: 1,
          kind: "continuity",
          applied: true,
          ssim: 0.93,
          psnr: 34,
          dy: 2,
          du: 1,
          dv: 1,
          pass: true,
          attempt: 0,
        },
        {
          index: 2,
          kind: "continuity",
          applied: true,
          ssim: 0.71,
          psnr: 22,
          dy: 3,
          du: 1,
          dv: 1,
          pass: false,
          attempt: 0,
        },
      ],
    };
    render(
      <Library
        jobs={[shot(0, true), shot(1, true), shot(2, true)]}
        selectedId={null}
        onSelect={noop}
        onOpenLightbox={noop}
        storyboards={[scored]}
        onRender={vi.fn()}
      />,
    );
    const badge = container.querySelector<HTMLElement>(".storyboard-join")!;
    expect(badge).not.toBeNull();
    // The minimum scored join (0.71) is shown, and the set is flagged low.
    expect(badge.textContent).toContain("0.71");
    expect(badge.getAttribute("data-pass")).toBe("false");
  });

  it("shows no join badge for an unscored set (S16)", () => {
    render(
      <Library
        jobs={[shot(0, true), shot(1, true)]}
        selectedId={null}
        onSelect={noop}
        onOpenLightbox={noop}
        storyboards={[SET]}
        onRender={vi.fn()}
      />,
    );
    expect(container.querySelector(".storyboard-join")).toBeNull();
  });
});
