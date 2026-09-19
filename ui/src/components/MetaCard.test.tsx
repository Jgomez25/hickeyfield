import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, type ReactElement } from "react";
import { createRoot, type Root } from "react-dom/client";
import { MetaCard } from "./MetaCard";
import type { JobSet } from "../types";

/**
 * AC5 (S10). The card carries the three cost states the argument for BYO-key
 * rests on: what we quoted, what it cost, and — only when both are known — the
 * delta. An estimate-only card must show no delta (inventing one from a missing
 * actual would be a false claim); an actual-known card shows the figure; both
 * present shows a signed delta toned as overrun or saving.
 *
 * Rendered directly with react-dom/client under jsdom, the no-RTL pattern the
 * other component tests use.
 */

(globalThis as unknown as { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

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

function job(overrides: Partial<JobSet>): JobSet {
  return {
    id: "j1",
    modelId: "kling3_0",
    status: "completed",
    prompt: "a cat in a hat",
    createdAt: new Date().toISOString(),
    results: [],
    route: { id: "fal:x", provider: "fal", slug: "x" },
    estimatedUsd: null,
    actualUsd: null,
    settings: {
      duration: 5,
      resolution: "720p",
      aspect: "16:9",
      audio: false,
      enhance: false,
    },
    ...overrides,
  };
}

const noop = () => {};

function fields() {
  return Array.from(container.querySelectorAll(".meta-card-fields dt")).map(
    (dt) => dt.textContent,
  );
}

function deltaCell() {
  return container.querySelector(".meta-card-delta");
}

describe("MetaCard cost states", () => {
  it("estimate only: shows the estimate, 'Not reported', and no delta row", () => {
    render(
      <MetaCard
        job={job({ estimatedUsd: 1.71, actualUsd: null })}
        model={null}
        selected={false}
        onSelect={noop}
        onRerun={noop}
        onDelete={noop}
      />,
    );
    expect(container.textContent).toContain("$1.71");
    expect(container.textContent).toContain("Not reported");
    expect(fields()).not.toContain("Delta");
    expect(deltaCell()).toBeNull();
  });

  it("actual known: shows the actual figure", () => {
    render(
      <MetaCard
        job={job({ estimatedUsd: null, actualUsd: 1.4 })}
        model={null}
        selected={false}
        onSelect={noop}
        onRerun={noop}
        onDelete={noop}
      />,
    );
    expect(container.textContent).toContain("$1.40");
    // No delta without both figures.
    expect(deltaCell()).toBeNull();
  });

  it("both present: shows a signed delta toned as an overrun", () => {
    render(
      <MetaCard
        job={job({ estimatedUsd: 1.4, actualUsd: 1.6 })}
        model={null}
        selected={false}
        onSelect={noop}
        onRerun={noop}
        onDelete={noop}
      />,
    );
    expect(fields()).toContain("Delta");
    const cell = deltaCell();
    expect(cell).not.toBeNull();
    expect(cell?.textContent).toBe("+$0.20");
    expect(cell?.getAttribute("data-tone")).toBe("over");
  });

  it("workhorse settled job renders the est-vs-settled delta", () => {
    // A4 (S20): a workhorse clip quoted at $0.12 (6s asked) that settled at
    // $0.13184 (6.592s delivered) shows a +$0.012 overrun on the card. The
    // test-locked .meta-card-fields / .meta-card-delta selectors are exercised.
    render(
      <MetaCard
        job={job({ estimatedUsd: 0.12, actualUsd: 0.13184 })}
        model={null}
        selected={false}
        onSelect={noop}
        onRerun={noop}
        onDelete={noop}
      />,
    );
    expect(fields()).toContain("Delta");
    const cell = deltaCell();
    expect(cell).not.toBeNull();
    expect(cell?.textContent).toBe("+$0.012");
    expect(cell?.getAttribute("data-tone")).toBe("over");
    // The locked field labels are all present and in order.
    expect(fields()).toEqual([
      "Route",
      "Estimated cost",
      "Actual cost",
      "Delta",
      "Enhancer",
    ]);
  });

  it("both present and under: shows a saving", () => {
    render(
      <MetaCard
        job={job({ estimatedUsd: 1.6, actualUsd: 1.4 })}
        model={null}
        selected={false}
        onSelect={noop}
        onRerun={noop}
        onDelete={noop}
      />,
    );
    const cell = deltaCell();
    expect(cell?.textContent).toBe("−$0.20");
    expect(cell?.getAttribute("data-tone")).toBe("under");
  });
});

// A pending (still running) job shows "Pending", never a delta.
describe("MetaCard while running", () => {
  it("does not draw a delta for an in-progress job", () => {
    render(
      <MetaCard
        job={job({ status: "in_progress", estimatedUsd: 1.4, actualUsd: null })}
        model={null}
        selected={false}
        onSelect={noop}
        onRerun={noop}
        onDelete={noop}
      />,
    );
    expect(container.textContent).toContain("Pending");
    expect(deltaCell()).toBeNull();
  });
});

// Silence unused import lint if vi is not otherwise referenced.
void vi;
