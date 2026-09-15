import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, type ReactElement } from "react";
import { createRoot, type Root } from "react-dom/client";
import { PromptPreview } from "./PromptPreview";
import type { PromptPreview as Preview } from "../types";

/**
 * S7 (AC3). The preview panel must:
 *  - show the compiled/enhanced prompt (in the editable box), the original for
 *    reference, and the honest note;
 *  - fire onChange as the user edits the text that will be sent verbatim;
 *  - re-run the preview from Retry;
 *  - submit the shown text from Generate.
 *
 * Rendered directly with react-dom/client under the existing jsdom setup — the
 * same no-RTL pattern EnhancerPicker.test.tsx uses.
 */

const PREVIEW: Preview = {
  prompt: "a lighthouse at dawn, shot on 35mm.",
  original: "a lighthouse at dawn",
  enhanced: "a lighthouse at dawn, shot on 35mm.",
  version: "hickeyfield-enhance-1.2",
  note: "Enhanced with a local model.",
};

(
  globalThis as unknown as { IS_REACT_ACT_ENVIRONMENT: boolean }
).IS_REACT_ACT_ENVIRONMENT = true;

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

// Set a controlled <textarea>'s value through the native setter, then dispatch
// a bubbling input event so React's onChange fires — the canonical fireEvent
// trick, kept local so no new tooling is pulled in.
function typeInto(el: HTMLTextAreaElement, value: string) {
  const setter = Object.getOwnPropertyDescriptor(
    HTMLTextAreaElement.prototype,
    "value",
  )!.set!;
  act(() => {
    setter.call(el, value);
    el.dispatchEvent(new Event("input", { bubbles: true }));
  });
}

describe("PromptPreview", () => {
  it("shows the editable prompt, the original, and the note", () => {
    render(
      <PromptPreview
        preview={PREVIEW}
        value={PREVIEW.enhanced ?? PREVIEW.prompt}
        onChange={vi.fn()}
        onRetry={vi.fn()}
        onGenerate={vi.fn()}
        previewing={false}
        pending={false}
        estimate={null}
        blockedReason={null}
      />,
    );

    const textarea =
      container.querySelector<HTMLTextAreaElement>("textarea.prompt-preview-text");
    expect(textarea).not.toBeNull();
    expect(textarea!.value).toBe("a lighthouse at dawn, shot on 35mm.");

    const text = container.textContent ?? "";
    expect(text).toContain("a lighthouse at dawn"); // the original block
    expect(text).toContain("Enhanced with a local model."); // the note
  });

  it("fires onChange as the user edits the prompt to send", () => {
    const onChange = vi.fn();
    render(
      <PromptPreview
        preview={PREVIEW}
        value={PREVIEW.enhanced ?? PREVIEW.prompt}
        onChange={onChange}
        onRetry={vi.fn()}
        onGenerate={vi.fn()}
        previewing={false}
        pending={false}
        estimate={null}
        blockedReason={null}
      />,
    );

    const textarea =
      container.querySelector<HTMLTextAreaElement>("textarea.prompt-preview-text")!;
    typeInto(textarea, "my own words");
    expect(onChange).toHaveBeenCalledWith("my own words");
  });

  it("re-runs the preview from Retry", () => {
    const onRetry = vi.fn();
    render(
      <PromptPreview
        preview={PREVIEW}
        value={PREVIEW.enhanced ?? PREVIEW.prompt}
        onChange={vi.fn()}
        onRetry={onRetry}
        onGenerate={vi.fn()}
        previewing={false}
        pending={false}
        estimate={null}
        blockedReason={null}
      />,
    );

    const retry =
      container.querySelector<HTMLButtonElement>("button.prompt-preview-retry")!;
    act(() => retry.click());
    expect(onRetry).toHaveBeenCalledTimes(1);
  });

  it("submits the shown text from Generate", () => {
    const onGenerate = vi.fn();
    render(
      <PromptPreview
        preview={PREVIEW}
        value={PREVIEW.enhanced ?? PREVIEW.prompt}
        onChange={vi.fn()}
        onRetry={vi.fn()}
        onGenerate={onGenerate}
        previewing={false}
        pending={false}
        estimate={null}
        blockedReason={null}
      />,
    );

    const generate =
      container.querySelector<HTMLButtonElement>("button.generate-button")!;
    expect(generate.disabled).toBe(false);
    act(() => generate.click());
    expect(onGenerate).toHaveBeenCalledTimes(1);
  });

  it("blocks Generate when the edit is emptied", () => {
    const onGenerate = vi.fn();
    render(
      <PromptPreview
        preview={PREVIEW}
        value=""
        onChange={vi.fn()}
        onRetry={vi.fn()}
        onGenerate={onGenerate}
        previewing={false}
        pending={false}
        estimate={null}
        blockedReason="Add a prompt before generating"
      />,
    );

    const generate =
      container.querySelector<HTMLButtonElement>("button.generate-button")!;
    expect(generate.disabled).toBe(true);
  });
});
