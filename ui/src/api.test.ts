import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { SubmitInput } from "./types";

/**
 * S7 (AC2/AC3). Two contracts the preview slice leans on, pinned at the bridge:
 *  - `previewPrompt` maps the Rust `PreviewDto` into a `PromptPreview` and wraps
 *    its argument as `{ input }` (Tauri matches by name), exactly like submitJob;
 *  - `submitJob` forwards the whole input, so the previewed/edited `finalPrompt`
 *    reaches the shell verbatim rather than being dropped on the way.
 *
 * `@tauri-apps/api/core` is mocked so no shell is needed; `__TAURI_INTERNALS__`
 * is set so `isDesktop()` takes the real bridge path instead of the browser mock.
 */

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));

import { previewPrompt, submitJob } from "./api";

const INPUT: SubmitInput = {
  modelId: "kling3_0",
  routeId: "fal:kling",
  prompt: "a lighthouse at dawn",
  presetId: null,
  settings: {
    duration: 5,
    resolution: "1080p",
    aspect: "16:9",
    audio: false,
    enhance: true,
  },
  media: [],
  rewriter: null,
};

beforeEach(() => {
  (window as unknown as { __TAURI_INTERNALS__: unknown }).__TAURI_INTERNALS__ =
    {};
  invokeMock.mockReset();
});

afterEach(() => {
  delete (window as unknown as { __TAURI_INTERNALS__?: unknown })
    .__TAURI_INTERNALS__;
});

describe("previewPrompt", () => {
  it("wraps the input as { input } and maps the DTO to a PromptPreview", async () => {
    invokeMock.mockResolvedValue({
      prompt: "a lighthouse at dawn, shot on 35mm.",
      original: "a lighthouse at dawn",
      enhanced: "a lighthouse at dawn, shot on 35mm.",
      version: "hickeyfield-enhance-1.2",
      note: null,
    });

    const got = await previewPrompt(INPUT);

    expect(invokeMock).toHaveBeenCalledWith("preview_prompt", {
      input: INPUT,
    });
    expect(got).toEqual({
      prompt: "a lighthouse at dawn, shot on 35mm.",
      original: "a lighthouse at dawn",
      enhanced: "a lighthouse at dawn, shot on 35mm.",
      version: "hickeyfield-enhance-1.2",
      note: null,
    });
  });

  it("keeps optional fields null when the DTO omits a rewrite", async () => {
    invokeMock.mockResolvedValue({
      prompt: "a lighthouse at dawn, shot on 35mm.",
      original: "a lighthouse at dawn",
      enhanced: null,
      version: null,
      note: "Enhance is off — your prompt was sent exactly as you wrote it.",
    });

    const got = await previewPrompt(INPUT);
    expect(got.enhanced).toBeNull();
    expect(got.version).toBeNull();
    expect(got.note).toContain("exactly as you wrote it");
  });
});

describe("submitJob", () => {
  it("forwards the whole input, carrying finalPrompt to the shell verbatim", async () => {
    invokeMock.mockResolvedValue({ jobSetId: "job-1" });

    const edited = "an edited lighthouse prompt, shot on 35mm.";
    await submitJob({ ...INPUT, finalPrompt: edited });

    expect(invokeMock).toHaveBeenCalledWith("submit_job", {
      input: expect.objectContaining({ finalPrompt: edited }),
    });
  });
});
