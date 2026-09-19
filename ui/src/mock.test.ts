import { describe, expect, it } from "vitest";
import { MOCK_JOBS, MOCK_MODELS, MOCK_PRESETS } from "./mock";
import { WORKHORSE_MODEL_ID } from "./types";

/**
 * A6 (S20b). The mock module is a default surface: it must not advertise a
 * never-used route (Seedance or Grok), and every list it seeds must lead with
 * the workhorse. A future edit that reintroduces a Seedance/Grok id — or that
 * drops the workhorse from the head of a preset/sample list — fails here.
 */

describe("mock module has no never-used routes and leads with the workhorse", () => {
  it("exports zero seedance/grok references across models, presets and jobs", () => {
    const blob = JSON.stringify({ MOCK_MODELS, MOCK_PRESETS, MOCK_JOBS });
    const hits = blob.match(/seedance|grok/gi) ?? [];
    expect(hits).toEqual([]);
  });

  it("leads the model roster with the workhorse", () => {
    expect(MOCK_MODELS[0]?.id).toBe(WORKHORSE_MODEL_ID);
  });

  it("leads every preset variant list with the workhorse", () => {
    for (const preset of MOCK_PRESETS) {
      if (!preset.variants || preset.variants.length === 0) continue;
      expect(
        preset.variants[0].modelId,
        `preset ${preset.id} must lead with the workhorse`,
      ).toBe(WORKHORSE_MODEL_ID);
    }
  });

  it("uses the workhorse for the sample in-progress job", () => {
    const sample = MOCK_JOBS.find((j) => j.status === "in_progress");
    expect(sample?.modelId).toBe(WORKHORSE_MODEL_ID);
  });
});
