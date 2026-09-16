import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, type ReactElement } from "react";
import { createRoot, type Root } from "react-dom/client";
import { Library } from "./Library";
import type { JobSet } from "../types";

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
});
