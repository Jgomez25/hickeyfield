import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, type ReactElement } from "react";
import { createRoot, type Root } from "react-dom/client";
import { EnhancerPicker } from "./EnhancerPicker";
import type { LocalModel } from "../types";

/**
 * AC3/AC4 (S5-enhancer-model-curation, extending S4). The enhancer control must:
 *  - show the installed Ollama models when a backend is available, and emit the
 *    chosen backend+model — the exact `rewriter` shape App forwards to
 *    `submit_job`;
 *  - auto-commit the *top-ranked* model (Rust hands the list pre-sorted, so
 *    `[0]` is recommended when one is installed);
 *  - badge recommended models and warn on discouraged ones (via `data-tier`),
 *    while keeping every option selectable;
 *  - show an honest reason (not a dead dropdown) when nothing can run;
 *  - offer OpenAI when a key is stored.
 *
 * Rendered directly with `react-dom/client` under the existing jsdom setup, the
 * same no-RTL pattern ModelPicker.test.tsx uses.
 */

// The pre-sorted, tier-tagged shape Rust hands the picker. `recommended` leads
// because `local_models` sorts best-first; the UI must not re-sort.
const RANKED: LocalModel[] = [
  { name: "gemma3:1b", tier: "recommended" },
  { name: "qwen2.5:7b", tier: "neutral" },
  { name: "qwen3-vl:4b", tier: "discouraged" },
];

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

// React tracks a controlled <select>'s value; set it through the native setter
// then dispatch a bubbling change so React's onChange fires. This is the
// canonical fireEvent trick, kept local so no new tooling is pulled in.
function selectValue(select: HTMLSelectElement, value: string) {
  const setter = Object.getOwnPropertyDescriptor(
    HTMLSelectElement.prototype,
    "value",
  )!.set!;
  act(() => {
    setter.call(select, value);
    select.dispatchEvent(new Event("change", { bubbles: true }));
  });
}

describe("EnhancerPicker", () => {
  it("auto-commits the top-ranked (recommended) model on mount, no interaction", () => {
    // The MAJOR review bug: the picker showed "Local (Ollama) + first model"
    // selected while the submitted rewriter stayed null. It must emit the
    // resolved default as soon as it renders so submit truth == what is shown —
    // and that default is `[0]`, the recommended model Rust sorted to the front.
    const onChange = vi.fn();
    render(
      <EnhancerPicker
        enhance
        ollamaUp
        ollamaModels={RANKED}
        openaiAvailable
        value={null}
        onChange={onChange}
      />,
    );
    expect(onChange).toHaveBeenCalledWith({
      backend: "ollama",
      model: "gemma3:1b",
    });
  });

  it("commits a discouraged model when it is the only one installed", () => {
    // Discouraged is a warning, not a ban: with nothing better installed, the
    // picker still commits a runnable default rather than leaving submit null.
    const onChange = vi.fn();
    render(
      <EnhancerPicker
        enhance
        ollamaUp
        ollamaModels={[{ name: "qwen3-vl:4b", tier: "discouraged" }]}
        openaiAvailable={false}
        value={null}
        onChange={onChange}
      />,
    );
    expect(onChange).toHaveBeenCalledWith({
      backend: "ollama",
      model: "qwen3-vl:4b",
    });
  });

  it("does not emit on mount when Enhance is off or nothing is available", () => {
    const off = vi.fn();
    render(
      <EnhancerPicker
        enhance={false}
        ollamaUp
        ollamaModels={RANKED}
        openaiAvailable
        value={null}
        onChange={off}
      />,
    );
    expect(off).not.toHaveBeenCalled();

    const none = vi.fn();
    render(
      <EnhancerPicker
        enhance
        ollamaUp={false}
        ollamaModels={[]}
        openaiAvailable={false}
        value={null}
        onChange={none}
      />,
    );
    expect(none).not.toHaveBeenCalled();
  });

  it("renders the installed models in the given order and emits the chosen rewriter", () => {
    const onChange = vi.fn();
    render(
      <EnhancerPicker
        enhance
        ollamaUp
        ollamaModels={RANKED}
        openaiAvailable={false}
        value={null}
        onChange={onChange}
      />,
    );

    const modelSelect =
      container.querySelector<HTMLSelectElement>("select.enhancer-model");
    expect(modelSelect).not.toBeNull();
    const options = Array.from(modelSelect!.querySelectorAll("option")).map(
      (o) => o.value,
    );
    // Passed through untouched — the order Rust supplied, no UI re-sort.
    expect(options).toEqual(["gemma3:1b", "qwen2.5:7b", "qwen3-vl:4b"]);

    // Choosing a model emits exactly the wire shape App forwards to submit_job.
    selectValue(modelSelect!, "qwen3-vl:4b");
    expect(onChange).toHaveBeenCalledWith({
      backend: "ollama",
      model: "qwen3-vl:4b",
    });
  });

  it("badges recommended, warns discouraged, and keeps every option selectable", () => {
    render(
      <EnhancerPicker
        enhance
        ollamaUp
        ollamaModels={RANKED}
        openaiAvailable={false}
        value={null}
        onChange={vi.fn()}
      />,
    );

    const modelSelect =
      container.querySelector<HTMLSelectElement>("select.enhancer-model")!;
    const options = Array.from(modelSelect.querySelectorAll("option"));
    const byValue = (v: string) => options.find((o) => o.value === v)!;

    // Tier drives a data attribute (styling/test hook) and a readable suffix.
    expect(byValue("gemma3:1b").dataset.tier).toBe("recommended");
    expect(byValue("gemma3:1b").textContent).toContain("(recommended)");
    expect(byValue("qwen2.5:7b").dataset.tier).toBe("neutral");
    expect(byValue("qwen2.5:7b").textContent).not.toContain("(");
    expect(byValue("qwen3-vl:4b").dataset.tier).toBe("discouraged");
    expect(byValue("qwen3-vl:4b").textContent).toContain(
      "may be slow / not ideal here",
    );

    // No option is disabled — a discouraged model stays selectable.
    expect(options.every((o) => !o.disabled)).toBe(true);
    const onChange = vi.fn();
    render(
      <EnhancerPicker
        enhance
        ollamaUp
        ollamaModels={RANKED}
        openaiAvailable={false}
        value={{ backend: "ollama", model: "gemma3:1b" }}
        onChange={onChange}
      />,
    );
    const sel =
      container.querySelector<HTMLSelectElement>("select.enhancer-model")!;
    selectValue(sel, "qwen3-vl:4b");
    expect(onChange).toHaveBeenCalledWith({
      backend: "ollama",
      model: "qwen3-vl:4b",
    });
  });

  it("shows an honest reason and no enabled control when nothing can run", () => {
    render(
      <EnhancerPicker
        enhance
        ollamaUp={false}
        ollamaModels={[]}
        openaiAvailable={false}
        value={null}
        onChange={vi.fn()}
      />,
    );

    // No dead dropdown.
    expect(container.querySelector("select")).toBeNull();
    // The reason is surfaced in words, and promises the prompt is sent as-is.
    const text = container.textContent ?? "";
    expect(text).toContain("Ollama isn't running");
    expect(text).toContain("no OpenAI key");
    expect(text).toContain("sent exactly as you wrote it");
  });

  it("offers OpenAI when a key is stored", () => {
    const onChange = vi.fn();
    render(
      <EnhancerPicker
        enhance
        ollamaUp={false}
        ollamaModels={[]}
        openaiAvailable
        value={null}
        onChange={onChange}
      />,
    );

    const backend =
      container.querySelector<HTMLSelectElement>("select.enhancer-backend");
    expect(backend).not.toBeNull();
    const options = Array.from(backend!.querySelectorAll("option")).map(
      (o) => o.value,
    );
    expect(options).toContain("openai");
    // A hosted model field is offered (no invented default id).
    expect(
      container.querySelector<HTMLInputElement>("input.enhancer-model"),
    ).not.toBeNull();
  });

  it("renders nothing when Enhance is off", () => {
    render(
      <EnhancerPicker
        enhance={false}
        ollamaUp
        ollamaModels={RANKED}
        openaiAvailable
        value={null}
        onChange={vi.fn()}
      />,
    );
    expect(container.querySelector(".enhancer-picker")).toBeNull();
  });
});
