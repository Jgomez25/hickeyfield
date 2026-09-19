import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, type ReactElement } from "react";
import { createRoot, type Root } from "react-dom/client";
import { Library } from "./Library";
import { parseJoinRecords } from "../api";
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
    modelId: "minimax_h3_max_turbo",
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

  // ── S17 (AC5): two scores per join, and the keyframe still cards ──────────

  it("the library item shows both the cut and the end-adherence score (S17)", () => {
    const scored: Storyboard = {
      ...SET,
      // A keyframe set (it has still boundaries) → the end badge is offered.
      keyframes: [
        { index: 0, jobSetId: "s0", localPath: "/lib/k0.png", estimatedUsd: 0.04, attempt: 0 },
        { index: 1, jobSetId: "s1", localPath: "/lib/k1.png", estimatedUsd: 0.04, attempt: 0 },
      ],
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
          // S17 end-frame adherence, scored against the ideal tier.
          endSsim: 0.91,
          endPsnr: 33,
          endDy: 3,
          endDu: 1,
          endDv: 1,
          endPass: true,
          attempt: 0,
        },
      ],
    };
    render(
      <Library
        jobs={[shot(0, true), shot(1, true)]}
        selectedId={null}
        onSelect={noop}
        onOpenLightbox={noop}
        storyboards={[scored]}
        onRender={vi.fn()}
      />,
    );
    const cut = container.querySelector<HTMLElement>(".storyboard-join")!;
    const end = container.querySelector<HTMLElement>(".storyboard-end")!;
    expect(cut).not.toBeNull();
    expect(cut.textContent).toContain("0.93");
    expect(end).not.toBeNull();
    expect(end.textContent).toContain("0.91");
    expect(end.getAttribute("data-pass")).toBe("true");
  });

  it("an unscored end shows a dash, never zero (S17)", () => {
    // A keyframe set whose joins are scored on the cut but not yet on the end
    // (end-* null); the badge must read "end —", never "end 0.00" (unknown is
    // not failing).
    const unscored: Storyboard = {
      ...SET,
      keyframes: [
        { index: 0, jobSetId: "s0", localPath: "/lib/k0.png", estimatedUsd: 0.04, attempt: 0 },
        { index: 1, jobSetId: "s1", localPath: "/lib/k1.png", estimatedUsd: 0.04, attempt: 0 },
      ],
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
          endSsim: null,
          endPsnr: null,
          endDy: null,
          endDu: null,
          endDv: null,
          endPass: null,
          attempt: 0,
        },
      ],
    };
    render(
      <Library
        jobs={[shot(0, true), shot(1, true)]}
        selectedId={null}
        onSelect={noop}
        onOpenLightbox={noop}
        storyboards={[unscored]}
        onRender={vi.fn()}
      />,
    );
    const end = container.querySelector<HTMLElement>(".storyboard-end")!;
    expect(end).not.toBeNull();
    expect(end.textContent).toContain("—");
    expect(end.textContent).not.toContain("0.00");
    expect(end.getAttribute("data-pass")).toBe("unscored");
  });

  it("renders the end badge from a REAL snake_case joins_json through the production mapping (S17 B1)", () => {
    // Regression for review B1: Rust serialises `end_ssim`/`end_pass` (snake),
    // the UI reads `endSsim`/`endPass` (camel). Earlier fixtures were camelCase
    // and bypassed the wire mapping, so the dead badge passed tests. Drive the
    // EXACT snake_case string fal/Rust emits through `parseJoinRecords` (the
    // production `listStoryboards` path) and assert the badge actually renders.
    const wire =
      '[{"index":1,"kind":"continuity","applied":true,"ssim":0.93,"psnr":34,"dy":2,"du":1,"dv":1,"pass":true,"attempt":0,"end_ssim":0.91,"end_psnr":33,"end_dy":3,"end_du":1,"end_dv":1,"end_pass":true}]';
    const joins = parseJoinRecords(wire);
    // The mapping actually produced the camelCase fields the component reads.
    expect(joins[0].endSsim).toBe(0.91);
    expect(joins[0].endPass).toBe(true);
    const set: Storyboard = {
      ...SET,
      keyframes: [
        { index: 0, jobSetId: "s0", localPath: "/lib/k0.png", estimatedUsd: 0.04, attempt: 0 },
        { index: 1, jobSetId: "s1", localPath: "/lib/k1.png", estimatedUsd: 0.04, attempt: 0 },
      ],
      joins,
    };
    render(
      <Library
        jobs={[shot(0, true), shot(1, true)]}
        selectedId={null}
        onSelect={noop}
        onOpenLightbox={noop}
        storyboards={[set]}
        onRender={vi.fn()}
      />,
    );
    const end = container.querySelector<HTMLElement>(".storyboard-end")!;
    expect(end).not.toBeNull();
    expect(end.textContent).toContain("0.91");
    expect(end.getAttribute("data-pass")).toBe("true");
  });

  it("shows NO end badge for a scored NON-keyframe set (S17 B1 guard)", () => {
    // Rust serialises `end_ssim:null` on every post-S17 join (the fields are
    // `#[serde(default)]`, not skipped), so a plain S16 chained set carries them
    // too. The end badge must still be suppressed when the set is not a keyframe
    // set — gated on the set having keyframe boundaries, not on the null fields.
    const wire =
      '[{"index":1,"kind":"continuity","applied":true,"ssim":0.93,"psnr":34,"dy":2,"du":1,"dv":1,"pass":true,"attempt":0,"end_ssim":null,"end_psnr":null,"end_dy":null,"end_du":null,"end_dv":null,"end_pass":null}]';
    const set: Storyboard = { ...SET, joins: parseJoinRecords(wire) }; // no keyframes
    render(
      <Library
        jobs={[shot(0, true), shot(1, true)]}
        selectedId={null}
        onSelect={noop}
        onOpenLightbox={noop}
        storyboards={[set]}
        onRender={vi.fn()}
      />,
    );
    // The cut badge still shows (S16); the end badge is suppressed.
    expect(container.querySelector(".storyboard-join")).not.toBeNull();
    expect(container.querySelector(".storyboard-end")).toBeNull();
  });

  it("a settled still shows a thumbnail and offers Regenerate still (S17)", () => {
    const stillJob = job({
      id: "still-0",
      storyboardId: "sb1",
      shotIndex: undefined,
      status: "completed",
      results: [{ url: "https://x/k0.png", kind: "image", localPath: "/lib/k0.png" }],
    });
    const kfSet: Storyboard = {
      ...SET,
      keyframes: [
        {
          index: 0,
          jobSetId: "still-0",
          localPath: "/lib/k0.png",
          estimatedUsd: 0.04,
          attempt: 0,
        },
      ],
    };
    const onRegenerateStill = vi.fn();
    render(
      <Library
        jobs={[stillJob, shot(0, true)]}
        selectedId={null}
        onSelect={noop}
        onOpenLightbox={noop}
        storyboards={[kfSet]}
        onRender={vi.fn()}
        onRegenerateStill={onRegenerateStill}
      />,
    );
    const card = container.querySelector<HTMLElement>(
      '.storyboard-keyframe-card[data-keyframe="0"]',
    )!;
    expect(card).not.toBeNull();
    expect(card.getAttribute("data-state")).toBe("ready");
    expect(card.querySelector(".storyboard-keyframe-thumb")).not.toBeNull();
    const regen = card.querySelector<HTMLButtonElement>(".storyboard-regenerate-still")!;
    act(() => regen.click());
    expect(onRegenerateStill).toHaveBeenCalledWith("sb1", 0);
  });

  it("a failed still shows the parked state and offers Regenerate still (S17)", () => {
    const failedStill = job({
      id: "still-1",
      storyboardId: "sb1",
      shotIndex: undefined,
      status: "failed",
      results: [],
      estimatedUsd: null,
    });
    const kfSet: Storyboard = {
      ...SET,
      keyframes: [
        {
          index: 1,
          jobSetId: "still-1",
          localPath: null,
          estimatedUsd: 0.04,
          attempt: 2,
        },
      ],
    };
    render(
      <Library
        jobs={[failedStill]}
        selectedId={null}
        onSelect={noop}
        onOpenLightbox={noop}
        storyboards={[kfSet]}
        onRender={vi.fn()}
        onRegenerateStill={vi.fn()}
      />,
    );
    const card = container.querySelector<HTMLElement>(
      '.storyboard-keyframe-card[data-keyframe="1"]',
    )!;
    expect(card.getAttribute("data-state")).toBe("parked");
    expect(card.textContent).toContain("parked");
    expect(card.querySelector(".storyboard-keyframe-thumb")).toBeNull();
    expect(card.querySelector(".storyboard-regenerate-still")).not.toBeNull();
  });
});
