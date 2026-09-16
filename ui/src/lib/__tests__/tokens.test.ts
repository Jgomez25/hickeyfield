import { describe, expect, it } from "vitest";
import { readdirSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

/**
 * AC2 (S12). The redesign is a token *re-valuation*, not a per-sheet rewrite:
 * every colour a token covers must resolve through `var()`, so a component
 * sheet may not hard-code a hex/rgb a token already models. This is the check
 * AC2 asks for — the repo ships no stylelint, so it is a file-reading vitest of
 * the same kind the repo already runs.
 *
 * Non-vacuous: drop any covered literal back into a component sheet and the
 * first test fails on that file; the second pins the new token names/values.
 */

const HERE = dirname(fileURLToPath(import.meta.url));
const SRC = join(HERE, "..", ".."); // ui/src
const COMPONENTS = join(SRC, "components");

// Literals a token already covers. Pure-black scrims (0,0,0) are deliberately
// not modelled by a colour token and are allowed; `#0f1113` is a bespoke
// blur-scrim in feed.css that maps to no token (recorded exception in design).
const COVERED = [
  // neutral surfaces (--cool-* / --bg-*)
  "#131416",
  "#1c1e21",
  "#2a2d32",
  "#23262a",
  "#18191c",
  // brand (--vermilion-* / --accent / --bloom*)
  "#ff4a1c",
  "#ff8a63",
  "255, 74, 28",
  "255,74,28",
  // danger (--danger*)
  "255, 84, 98",
  "255,84,98",
  // text: both the old values and the new, so neither is smuggled into a sheet
  "#828282",
  "#626262",
  "#9a9a9a",
  "#6e6e6e",
];

function cssFiles(dir: string): string[] {
  return readdirSync(dir)
    .filter((f) => f.endsWith(".css"))
    .map((f) => join(dir, f));
}

describe("S12 AC2 tokens", () => {
  it("component sheets use tokens, not covered colour literals", () => {
    for (const file of cssFiles(COMPONENTS)) {
      const text = readFileSync(file, "utf8").toLowerCase();
      for (const literal of COVERED) {
        expect(
          text,
          `${file} must reference a token, not the literal ${literal}`,
        ).not.toContain(literal.toLowerCase());
      }
    }
  });

  it("the new/re-valued tokens are defined in tokens.css", () => {
    const tokens = readFileSync(join(SRC, "tokens.css"), "utf8");
    for (const decl of [
      "--rail-nav: 72px",
      "--rail-settings: 400px",
      "--tile-thumb-ratio: 16 / 9",
      "--radius-card:",
      "--radius-panel:",
      "--bloom-strong:",
      "--text-secondary: #9a9a9a",
      "--text-tertiary: #6e6e6e",
    ]) {
      expect(tokens, `tokens.css must declare ${decl}`).toContain(decl);
    }
  });
});
