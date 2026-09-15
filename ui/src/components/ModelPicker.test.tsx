import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, type ReactElement } from "react";
import { createRoot, type Root } from "react-dom/client";
import { ModelPicker } from "./ModelPicker";
import type { Model } from "../types";

/**
 * AC2 (S2-honest-free-tier). The dead `z_image`/`Local` "free tier" must not be
 * presented as a runnable/Generate option: it is filtered out exactly like the
 * three other unroutable models, so in the picker its tile is disabled and its
 * route chip carries the honest unavailable reason with no selectable
 * affordance. AC3 guard: a real (fal) route sitting next to it stays runnable.
 *
 * The repo ships no React Testing Library, so this renders `ModelPicker`
 * directly with `react-dom/client` under the existing jsdom vitest setup and
 * asserts on DOM attributes/text — no new tooling.
 */

// React 19's `act` requires this flag to flush effects without warning.
(globalThis as unknown as { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

const LOCAL_REASON = "Hickeyfield has no client for Local yet";

// A `z_image`-shaped model: its only route is Local, reported unavailable by
// the shell (AC1's data), so the picker has nothing runnable to offer.
const zImage: Model = {
  id: "z_image",
  displayName: "Z-Image",
  modality: "image",
  isLaunch: false,
  subtitle: "open weights",
  routes: [
    {
      id: "local:z-image",
      provider: "local",
      slug: "z-image",
      note: "open weights: free on a detected local endpoint",
      available: false,
      unavailableReason: LOCAL_REASON,
    },
  ],
};

// A control model whose fal route is available — the UI-level AC3 guard that
// the fix does not grey out real routes.
const control: Model = {
  id: "nano_banana_2",
  displayName: "Nano Banana 2",
  modality: "image",
  isLaunch: true,
  subtitle: "fast image edits",
  routes: [
    {
      id: "fal:fal-ai/nano-banana-2",
      provider: "fal",
      slug: "fal-ai/nano-banana-2",
      available: true,
    },
  ],
};

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

function itemFor(name: string): HTMLLIElement {
  const items = Array.from(container.querySelectorAll<HTMLLIElement>("li.model-item"));
  const found = items.find((li) =>
    li.querySelector(".model-item-name")?.textContent?.includes(name),
  );
  if (!found) throw new Error(`no model item rendered for ${name}`);
  return found;
}

describe("ModelPicker hides unroutable Local-only models", () => {
  it("renders z_image's tile disabled with the honest reason and no select affordance", () => {
    const onSelect = vi.fn();
    render(
      <ModelPicker
        open
        onClose={() => {}}
        models={[zImage, control]}
        selectedModelId={null}
        selectedRouteId={null}
        onSelect={onSelect}
      />,
    );

    const item = itemFor("Z-Image");

    // The main tile is not a runnable/Generate affordance.
    const main = item.querySelector<HTMLButtonElement>("button.model-item-main");
    expect(main).not.toBeNull();
    expect(main!.disabled).toBe(true);
    expect(item.getAttribute("data-unrunnable")).toBe("true");

    // The Local route chip is greyed out and explains itself.
    const chip = item.querySelector<HTMLButtonElement>("button.chip-route");
    expect(chip).not.toBeNull();
    expect(chip!.disabled).toBe(true);
    expect(chip!.getAttribute("data-unavailable")).toBe("true");
    expect(chip!.title).toBe(LOCAL_REASON);

    // The reason is surfaced to the user, not swallowed.
    expect(container.textContent).toContain(LOCAL_REASON);

    // And clicking either disabled affordance selects nothing.
    act(() => {
      main!.click();
      chip!.click();
    });
    expect(onSelect).not.toHaveBeenCalled();
  });

  it("keeps a real fal route runnable and selectable (AC3)", () => {
    const onSelect = vi.fn();
    render(
      <ModelPicker
        open
        onClose={() => {}}
        models={[zImage, control]}
        selectedModelId={null}
        selectedRouteId={null}
        onSelect={onSelect}
      />,
    );

    const item = itemFor("Nano Banana 2");
    const main = item.querySelector<HTMLButtonElement>("button.model-item-main");
    expect(main).not.toBeNull();
    expect(main!.disabled).toBe(false);
    expect(item.hasAttribute("data-unrunnable")).toBe(false);

    act(() => {
      main!.click();
    });
    expect(onSelect).toHaveBeenCalledWith("nano_banana_2", "fal:fal-ai/nano-banana-2");
  });
});
