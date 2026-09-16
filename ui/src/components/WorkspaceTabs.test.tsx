import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, type ReactElement } from "react";
import { createRoot, type Root } from "react-dom/client";
import { WorkspaceTabs } from "./WorkspaceTabs";
import type { UseCase } from "../types";

/**
 * AC3 (S12). The 72px use-case nav is WorkspaceTabs restyled to the vertical
 * icon variant — same tablist, same `data-tab`, same roving tabindex, same
 * `onChange`. This pins that the nav variant drives the use-case tab exactly as
 * the row variant does (a click and an arrow key both change selection), so the
 * layout change is CSS + an icon, not a behaviour change.
 *
 * No RTL — direct react-dom/client render under jsdom, the repo's pattern.
 */

(globalThis as unknown as { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

const USE_CASES: UseCase[] = [
  { slug: "text-to-video", label: "New Video", blurb: "", slots: [], requiresMedia: false },
  { slug: "image-to-video", label: "Animate", blurb: "", slots: [], requiresMedia: false },
  { slug: "edit-video", label: "Edit Video", blurb: "", slots: [], requiresMedia: false },
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

describe("WorkspaceTabs nav variant (S12 use-case nav)", () => {
  it("renders the vertical nav with the pinned tablist/data-tab structure", () => {
    render(
      <WorkspaceTabs
        variant="nav"
        useCases={USE_CASES}
        value="text-to-video"
        onChange={() => {}}
        panelId="generator-panel"
      />,
    );
    const list = container.querySelector('[role="tablist"]');
    expect(list).not.toBeNull();
    expect(list!.classList.contains("tabs-nav")).toBe(true);
    expect(container.querySelectorAll('[role="tab"][data-tab]').length).toBe(3);
    // The selected tab controls the composer panel, and it is the one in the
    // tab order (roving).
    const selected = container.querySelector<HTMLButtonElement>(
      '[data-tab="text-to-video"]',
    )!;
    expect(selected.getAttribute("aria-selected")).toBe("true");
    expect(selected.getAttribute("aria-controls")).toBe("generator-panel");
    expect(selected.tabIndex).toBe(0);
  });

  it("clicking a nav item changes the use-case tab", () => {
    const onChange = vi.fn();
    render(
      <WorkspaceTabs
        variant="nav"
        useCases={USE_CASES}
        value="text-to-video"
        onChange={onChange}
        panelId="generator-panel"
      />,
    );
    const edit = container.querySelector<HTMLButtonElement>(
      '[data-tab="edit-video"]',
    )!;
    act(() => edit.click());
    expect(onChange).toHaveBeenCalledWith("edit-video");
  });

  it("arrow keys rove between nav items (vertical + horizontal)", () => {
    const onChange = vi.fn();
    render(
      <WorkspaceTabs
        variant="nav"
        useCases={USE_CASES}
        value="text-to-video"
        onChange={onChange}
        panelId="generator-panel"
      />,
    );
    const list = container.querySelector<HTMLDivElement>('[role="tablist"]')!;
    act(() => {
      list.dispatchEvent(
        new KeyboardEvent("keydown", { key: "ArrowDown", bubbles: true }),
      );
    });
    // Down advances from the first to the second use case.
    expect(onChange).toHaveBeenCalledWith("image-to-video");
  });
});
