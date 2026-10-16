import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import App from "./App";
import { MOCK_MODELS } from "./mock";
import { WORKHORSE_MODEL_ID, type Model } from "./types";

/**
 * A1 — the workhorse preference, made FALSIFIABLE.
 *
 * This file owns the single behaviour "the Studio opens on the workhorse and
 * falls back to it". The earlier version of this test (in App.test.tsx) was a
 * fixture-ordering tautology: it handed App a roster and per-tab lists whose
 * `[0]` was ALREADY the expected model in every asserted state, so stripping
 * every workhorse-preference site from App.tsx left it GREEN.
 *
 * The fixtures below are built so the workhorse is NEVER at index 0 and index 0
 * is never the expected answer, and so the fallback branch (App.tsx ~:351) — not
 * the survival branch — is the deciding factor at least once:
 *
 *   initialRoster  [gemini_omni(video,¬launch), kling_3_0(video,launch),
 *                   minimax_h3_max_turbo(workhorse), wan_2_6]
 *       → m[0] is a non-launch video; the first LAUNCH video is kling, not the
 *         workhorse. Only `m.find(WORKHORSE)` yields H3 Max Turbo. Kills the
 *         mutations "initial → m[0]" and "initial → first isLaunch video".
 *
 *   text-to-video  videoA [gemini_omni, kling_3_0, WORKHORSE, wan_2_6]
 *       → contains the workhorse (not at [0]); the initial workhorse selection
 *         SURVIVES the re-list.
 *
 *   text-to-image  imageList [recraft_v4_1(image,¬launch), nano_banana_pro(launch),
 *                             seedream_5_pro(launch)]
 *       → no workhorse; m[0] is a NON-launch image model, so the answer
 *         (first LAUNCH = nano_banana_pro) differs from m[0]. Kills
 *         "fallback → m[0]" for the image tab.
 *
 *   image-to-video videoB [wan_2_6(launch), kling_3_0(launch), WORKHORSE]
 *       → contains the workhorse (not at [0], and NOT first isLaunch — wan_2_6
 *         precedes it) and does NOT contain the currently selected image model,
 *         so the survival branch fails and the FALLBACK branch runs. Only
 *         `m.find(WORKHORSE)` yields H3 Max Turbo. Kills "fallback → m[0]" and
 *         "fallback → first isLaunch".
 *
 * "Never a never-used id" is a SERVER-SIDE guarantee (the real
 * `modelsForUseCase`/`list_models` never emit Seedance or Grok by default),
 * owned by the Rust tests `list_models_hides_the_never_used_routes_by_default`
 * and `is_never_used_covers_exactly_the_registry_seedance_and_grok_ids` (A3). It
 * cannot be honestly proven by a UI mock whose lists we author, so this test
 * instead asserts the image-tab landing model isLaunch and is the expected id.
 */

const byId = (id: string): Model => {
  const m = MOCK_MODELS.find((x) => x.id === id);
  if (!m) throw new Error(`fixture model ${id} missing from MOCK_MODELS`);
  return m;
};

// Workhorse is deliberately NOT at index 0 in any list below.
const initialRoster: Model[] = [
  byId("gemini_omni"), // video, isLaunch:false  → wrong under m[0]
  byId("kling_3_0"), // video, isLaunch:true    → wrong under "first isLaunch"
  byId(WORKHORSE_MODEL_ID), // the only correct answer
  byId("wan_2_6"),
];
const videoA: Model[] = [
  byId("gemini_omni"),
  byId("kling_3_0"),
  byId(WORKHORSE_MODEL_ID),
  byId("wan_2_6"),
];
const imageList: Model[] = [
  byId("recraft_v4_1"), // image, isLaunch:false → wrong under m[0]
  byId("nano_banana_pro"), // image, isLaunch:true  → the correct fallback
  byId("seedream_5_pro"),
];
const videoB: Model[] = [
  byId("wan_2_6"), // video, isLaunch:true  → wrong under m[0] AND "first isLaunch"
  byId("kling_3_0"),
  byId(WORKHORSE_MODEL_ID), // workhorse present but neither [0] nor first launch
];

vi.mock("./api", async (importOriginal) => {
  const actual = await importOriginal<typeof import("./api")>();
  const useCase = (slug: string, label: string) => ({
    slug,
    label,
    blurb: "",
    slots: [] as [string, boolean][],
    requiresMedia: false,
  });
  return {
    ...actual,
    // Initial pick reads this full roster; workhorse is at index 2.
    listModels: async () => initialRoster,
    listUseCases: async () => [
      useCase("text-to-video", "Text to Video"),
      useCase("image-to-video", "Image to Video"),
      useCase("text-to-image", "Text to Image"),
    ],
    // Per-tab lists. In production the picker is Rust-filtered; here each slug
    // returns a hand-ordered list (see the file header for the ordering rules).
    modelsForUseCase: async (slug: string) => {
      if (slug === "text-to-image") return imageList;
      if (slug === "image-to-video") return videoB;
      return videoA; // text-to-video (initial tab)
    },
  };
});

(globalThis as unknown as { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

let container: HTMLDivElement;
let root: Root;

beforeEach(() => {
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

describe("A1: Studio opens on and falls back to the workhorse", () => {
  it("prefers the workhorse over m[0] and over the first launch model", async () => {
    await act(async () => {
      root.render(<App />);
    });
    await flush();

    click(Array.from(container.querySelectorAll(".view-switch-tab")).find(b => b.textContent === "Studio") ?? null);
    await flush();

    // The composer's model summary row always renders the currently selected
    // model, so its name is the selection under test.
    const selectedName = () =>
      container.querySelector(".model-item-composer .model-item-name")
        ?.textContent ?? "";
    const clickTab = (slug: string) =>
      click(
        container.querySelector<HTMLButtonElement>(
          `[role="tab"][data-tab="${slug}"]`,
        ),
      );

    // 1) INITIAL PICK. The roster leads with a non-launch video (gemini_omni)
    //    and its first launch video is kling_3_0, yet the Studio opens on the
    //    workhorse. Neither m[0] nor "first isLaunch video" yields this.
    expect(selectedName()).toContain("H3 Max Turbo");

    // 2) FALLBACK AWAY into an image tab with no workhorse. m[0] there is a
    //    NON-launch model (recraft), so the selection falling to the first
    //    LAUNCH model (nano_banana_pro) proves it is not just taking the head.
    clickTab("text-to-image");
    await flush();
    const imageName = selectedName();
    expect(imageName).not.toContain("H3 Max Turbo");
    expect(imageName).toContain("Nano Banana Pro");
    // The landing model is a real LAUNCH route with the expected id. (Excluding
    // never-used Seedance/Grok ids from the list is a server-side guarantee,
    // owned by the Rust A3 tests list_models_hides_the_never_used_routes_by_default
    // and is_never_used_covers_exactly_the_registry_seedance_and_grok_ids.)
    const chosenImage = imageList.find((m) => m.displayName === imageName);
    expect(chosenImage?.isLaunch).toBe(true);
    expect(chosenImage?.id).toBe("nano_banana_pro");

    // 3) FALLBACK INTO a video tab from that non-workhorse selection. videoB
    //    contains the workhorse (not at [0], and wan_2_6 is an earlier launch
    //    model) but NOT the selected image model, so the survival branch fails
    //    and the fallback branch must actively prefer the workhorse.
    clickTab("image-to-video");
    await flush();
    expect(selectedName()).toContain("H3 Max Turbo");

    // 4) SURVIVAL across two video tabs that both list the workhorse (neither at
    //    index 0): the selection is retained, not snapped to a head.
    clickTab("text-to-video");
    await flush();
    expect(selectedName()).toContain("H3 Max Turbo");
  });
});
