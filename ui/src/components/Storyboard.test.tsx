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

  it("a cleared target length shows a validation message and blocks drafting", () => {
    const props = base();
    render(<Storyboard {...props} />);
    typeInto(
      container.querySelector<HTMLTextAreaElement>("textarea.storyboard-brief")!,
      "a brief",
    );
    // Clear the target length → 0.
    typeInto(
      container.querySelector<HTMLInputElement>(".storyboard-target-input")!,
      "",
    );
    expect(container.querySelector(".storyboard-target-invalid")).not.toBeNull();
    expect(
      container.querySelector<HTMLButtonElement>("button.storyboard-draft")!.disabled,
    ).toBe(true);
  });

  it("a full success switches to the Library", async () => {
    const onGenerated = vi.fn();
    const props = base({
      onGenerate: vi.fn(async () => ({
        storyboardId: "sb",
        shots: [0, 1, 2, 3].map((i) => ({
          shotIndex: i,
          jobSetId: `j${i}`,
          error: null,
        })),
      })),
      onGenerated,
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
    await act(async () => {
      container.querySelector<HTMLButtonElement>("button.storyboard-generate")!.click();
    });
    await flush();

    expect(onGenerated).toHaveBeenCalledTimes(1);
    // No partial-failure summary on a clean run.
    expect(container.querySelector(".storyboard-outcome")).toBeNull();
  });

  it("a partial failure shows the failed shots and retry submits only those", async () => {
    const onGenerated = vi.fn();
    // A server-faithful mock: it re-validates the FULL plan carried in
    // `planJson` exactly as `generate_storyboard` does (3..=8 shots — the very
    // gate that the M2 bug tripped when a subset plan was sent), and honours
    // `shotIndices` + `storyboardId`. If the retry sent a 1-shot subset plan
    // (the old bug), this throws "3 to 8 shots" and the test would fail — so the
    // mock cannot mask the integration defect the way the previous vi.fn did.
    const onGenerate = vi.fn(
      async (input: {
        planJson: string;
        shots: unknown[];
        shotIndices?: number[];
        storyboardId?: string;
      }) => {
        const plan = JSON.parse(input.planJson) as { shots: unknown[] };
        if (plan.shots.length < 3 || plan.shots.length > 8) {
          throw new Error("a storyboard needs 3 to 8 shots");
        }
        const indices = input.shotIndices ?? input.shots.map((_, i) => i);
        const isRetry = Boolean(input.storyboardId);
        return {
          storyboardId: input.storyboardId ?? "sb1",
          shots: indices.map((idx) => ({
            shotIndex: idx,
            // On the first generate shot index 2 is refused; on the retry
            // (storyboardId set) it now starts.
            jobSetId: idx === 2 && !isRetry ? null : `j${idx}`,
            error: idx === 2 && !isRetry ? "the model declined this shot" : null,
          })),
        };
      },
    );
    const props = base({ onGenerate, onGenerated });
    render(<Storyboard {...props} />);
    typeInto(
      container.querySelector<HTMLTextAreaElement>("textarea.storyboard-brief")!,
      "brief",
    );
    await act(async () => {
      container.querySelector<HTMLButtonElement>("button.storyboard-draft")!.click();
    });
    await flush();
    await act(async () => {
      container.querySelector<HTMLButtonElement>("button.storyboard-generate")!.click();
    });
    await flush();

    // The partial failure is surfaced, with the failed shot's error text, and
    // the batch did NOT switch to the Library.
    const outcome = container.querySelector(".storyboard-outcome");
    expect(outcome).not.toBeNull();
    expect(outcome!.textContent).toContain("the model declined this shot");
    const fails = container.querySelectorAll(".storyboard-outcome-fail");
    expect(fails.length).toBe(1);
    expect(fails[0].getAttribute("data-shot-index")).toBe("2");
    expect(onGenerated).not.toHaveBeenCalled();

    // The subset (retry) total is shown BEFORE any resubmit — money honesty.
    const retryTotal = container.querySelector(".storyboard-outcome-retry-total");
    expect(retryTotal).not.toBeNull();
    expect(retryTotal!.textContent?.trim().length).toBeGreaterThan(0);

    // Retry submits ONLY the failed shot.
    await act(async () => {
      container.querySelector<HTMLButtonElement>("button.storyboard-retry")!.click();
    });
    await flush();

    expect(onGenerate).toHaveBeenCalledTimes(2);
    const retryArg = onGenerate.mock.calls[1][0] as {
      planJson: string;
      shots: unknown[];
      shotIndices?: number[];
      storyboardId?: string;
    };
    // Exactly the one failed shot is resubmitted...
    expect(retryArg.shots.length).toBe(1);
    // ...but the FULL 4-shot plan is sent (the M2 fix — a subset plan would be
    // rejected by the server's 3..=8 count gate)...
    expect((JSON.parse(retryArg.planJson) as { shots: unknown[] }).shots.length).toBe(4);
    // ...and the retry targets the original failed index and the original group.
    expect(retryArg.shotIndices).toEqual([2]);
    expect(retryArg.storyboardId).toBe("sb1");
    // All shots now succeeded → hand off to the Library, summary cleared.
    expect(onGenerated).toHaveBeenCalledTimes(1);
    expect(container.querySelector(".storyboard-outcome")).toBeNull();
  });

  it("reordering closes an open prompt preview", async () => {
    const props = base();
    render(<Storyboard {...props} />);
    typeInto(
      container.querySelector<HTMLTextAreaElement>("textarea.storyboard-brief")!,
      "brief",
    );
    await act(async () => {
      container.querySelector<HTMLButtonElement>("button.storyboard-draft")!.click();
    });
    await flush();

    // Open shot 1's inline preview.
    await act(async () => {
      container
        .querySelector<HTMLButtonElement>(
          ".storyboard-shot[data-shot-index='0'] .storyboard-preview-open",
        )!
        .click();
    });
    await flush();
    expect(container.querySelector(".prompt-preview-text")).not.toBeNull();

    // Move shot 1 down — the stale preview must close so it cannot be accepted
    // for the shot that now sits at index 0.
    await act(async () => {
      container
        .querySelector<HTMLButtonElement>(
          ".storyboard-shot[data-shot-index='0'] .storyboard-move:not([disabled])",
        )!
        .click();
    });
    await flush();
    expect(container.querySelector(".prompt-preview-text")).toBeNull();
  });

  // S15 AC5: the Chain toggle defaults on only when every shot is chainable.
  async function draftWith(planShots: Shot[]) {
    const props = base({ onPlan: vi.fn(async () => ({
      brief: "b",
      targetSeconds: 30,
      useCase: "text-to-video",
      shots: planShots,
    })) });
    render(<Storyboard {...props} />);
    typeInto(
      container.querySelector<HTMLTextAreaElement>("textarea.storyboard-brief")!,
      "a chained ad",
    );
    await act(async () => {
      container.querySelector<HTMLButtonElement>("button.storyboard-draft")!.click();
    });
    await flush();
    return props;
  }

  function chainableShots(flags: boolean[]): Shot[] {
    return flags.map((c, i) => ({
      title: `Shot ${i + 1}`,
      prompt: `beat ${i + 1}`,
      seconds: 15,
      camera: null,
      modelId: "kling_3_0",
      role: i === 0 ? "start" : i === flags.length - 1 ? "end" : "middle",
      continuity: i === 0 ? null : { lastFrameOf: i - 1 },
      advisories: [],
      chainable: c,
    }));
  }

  it("the chain toggle defaults on only when every shot is chainable", async () => {
    await draftWith(chainableShots([true, true]));
    const toggle = container.querySelector<HTMLInputElement>(
      ".storyboard-chain-toggle input",
    )!;
    expect(toggle.checked).toBe(true);
    expect(container.querySelector(".storyboard-chain-note")).toBeNull();
  });

  it("the chain toggle defaults off with a note when a shot is not chainable", async () => {
    await draftWith(chainableShots([true, false]));
    const toggle = container.querySelector<HTMLInputElement>(
      ".storyboard-chain-toggle input",
    )!;
    expect(toggle.checked).toBe(false);
    expect(container.querySelector(".storyboard-chain-note")).not.toBeNull();
  });

  it("generate-all passes the chained flag as chosen", async () => {
    const props = await draftWith(chainableShots([true, true]));
    await act(async () => {
      container.querySelector<HTMLButtonElement>("button.storyboard-generate")!.click();
    });
    await flush();
    expect(props.onGenerate).toHaveBeenCalledWith(
      expect.objectContaining({ chained: true }),
    );
  });

  it("the auto-retry toggle appears only when chaining, and rides the payload (S16)", async () => {
    const props = await draftWith(chainableShots([true, true]));
    // Chaining is on (both shots chainable) → the auto-retry toggle is offered,
    // off by default (nothing spends without an opt-in).
    const auto = container.querySelector<HTMLInputElement>(
      ".storyboard-autoretry-toggle input",
    )!;
    expect(auto).not.toBeNull();
    expect(auto.checked).toBe(false);

    // Turning chaining OFF hides the auto-retry toggle entirely.
    const chain = container.querySelector<HTMLInputElement>(
      ".storyboard-chain-toggle input",
    )!;
    act(() => chain.click());
    expect(
      container.querySelector(".storyboard-autoretry-toggle"),
    ).toBeNull();

    // Back on, opt into auto-retry, and generate → the payload carries it.
    act(() => chain.click());
    const auto2 = container.querySelector<HTMLInputElement>(
      ".storyboard-autoretry-toggle input",
    )!;
    act(() => auto2.click());
    await act(async () => {
      container.querySelector<HTMLButtonElement>("button.storyboard-generate")!.click();
    });
    await flush();
    expect(props.onGenerate).toHaveBeenCalledWith(
      expect.objectContaining({ chained: true, autoRetry: true }),
    );
  });
});
