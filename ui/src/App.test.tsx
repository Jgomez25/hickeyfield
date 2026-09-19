import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import App from "./App";

/**
 * The A1 cross-tab workhorse-preference test lives in its own file,
 * `App.workhorse.test.tsx`, with its own scoped `vi.mock("./api")` whose
 * per-tab fixtures are hand-ordered so the preference logic is falsifiable
 * (workhorse never at index 0). Keeping it out of this file leaves these S12
 * tests running against the plain browse-mode api.
 *
 * AC1 + AC3 (S12) at the App level.
 *
 * AC1 (non-vacuous button-in-button): the delete confirmation is now a sibling
 * of the other overlays, not nested inside a titlebar <button>. The test opens
 * the confirmation (so a real `role="dialog"` with its own buttons is on
 * screen) and asserts there is still no `button` inside another `button` — the
 * exact structural invariant the fix restores. Re-nesting ConfirmDelete in a
 * button (the pre-fix bug) makes `document.querySelectorAll("button button")`
 * non-empty and fails this test.
 *
 * AC3: the meta rail is mounted only on Library and the workspace carries the
 * per-view grid attribute; toggling the view flips both, and the selection
 * pairing effect throws on neither view.
 *
 * Runs under jsdom in browse mode (no Tauri), where the api layer serves mock
 * jobs/models — enough to render both views and a MetaCard to delete.
 */

(globalThis as unknown as { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

let container: HTMLDivElement;
let root: Root;

beforeEach(() => {
  // Skip onboarding so the first-run overlay does not open over the workspace.
  window.localStorage.setItem("hickeyfield.onboarding.skipped", "1");
  container = document.createElement("div");
  document.body.appendChild(container);
  root = createRoot(container);
});

afterEach(() => {
  act(() => root.unmount());
  container.remove();
  window.localStorage.clear();
});

// Let the mount effects (Promise.all data loads) settle.
async function flush() {
  for (let i = 0; i < 5; i += 1) {
    // eslint-disable-next-line no-await-in-loop
    await act(async () => {
      await new Promise((r) => setTimeout(r, 0));
    });
  }
}

function click(el: Element | null) {
  if (!el) throw new Error("nothing to click");
  act(() => (el as HTMLElement).click());
}

function viewTab(label: string): HTMLButtonElement {
  const found = Array.from(
    container.querySelectorAll<HTMLButtonElement>(".view-switch-tab"),
  ).find((b) => b.textContent === label);
  if (!found) throw new Error(`no view tab ${label}`);
  return found;
}

describe("App view switch + delete confirmation (S12)", () => {
  it("mounts the meta rail on Library only, and flips the workspace view", async () => {
    await act(async () => {
      root.render(<App />);
    });
    await flush();

    // Studio: composer + stage, no meta rail.
    expect(container.querySelector('.workspace[data-view="studio"]')).not.toBeNull();
    expect(container.querySelector(".stage")).not.toBeNull();
    expect(container.querySelector(".rail-meta")).toBeNull();

    // Switch to Library: gallery + meta rail, no composer/stage. The selection
    // pairing effect must not throw as the meta anchor appears.
    click(viewTab("Library"));
    await flush();

    expect(container.querySelector('.workspace[data-view="library"]')).not.toBeNull();
    expect(container.querySelector(".library")).not.toBeNull();
    expect(container.querySelector(".rail-meta")).not.toBeNull();
    expect(container.querySelector(".stage")).toBeNull();
  });

  it("opens the delete confirmation as a dialog with no button-in-button", async () => {
    await act(async () => {
      root.render(<App />);
    });
    await flush();

    // No nesting at rest.
    expect(document.querySelectorAll("button button").length).toBe(0);

    // Go to Library (where the MetaCards, and their delete buttons, live).
    click(viewTab("Library"));
    await flush();

    const del = document.querySelector('button[aria-label="Delete generation"]');
    expect(del).not.toBeNull();
    click(del);
    await flush();

    // The confirmation is genuinely open — a real dialog with its own buttons…
    const dialog = document.querySelector('[role="dialog"]');
    expect(dialog).not.toBeNull();
    expect(dialog!.querySelectorAll("button").length).toBeGreaterThan(0);

    // …and yet no interactive element nests another. This is the fix: the
    // dialog is a sibling of the overlays, not inside a titlebar <button>.
    expect(document.querySelectorAll("button button").length).toBe(0);
  });
});
