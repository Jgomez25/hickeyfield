import { describe, expect, it } from "vitest";
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

/**
 * AC1 (S12) fixes-first guard, static so it cannot go stale or flake.
 *
 * The four fixes this pins:
 *  - the dangling `--color-vermilion` (it resolved to nothing, so the delete
 *    checkbox used the UA default accent) must appear in no component sheet;
 *  - the dangling `.btn-quiet` (no rule ever defined it) must appear in no TSX;
 *  - the dead `ModelBrowser.tsx` and `browser.css` orphans must be gone.
 *
 * The button-in-button structural fix (ConfirmDelete out of the titlebar
 * <button>) is proved by the App-level test in App.test.tsx — this file only
 * catches the reintroduction of the dead tokens/files.
 */

const HERE = dirname(fileURLToPath(import.meta.url));
const SRC = join(HERE, "..", ".."); // ui/src
const COMPONENTS = join(SRC, "components");

function cssFiles(dir: string): string[] {
  return readdirSync(dir)
    .filter((f) => f.endsWith(".css"))
    .map((f) => join(dir, f));
}

function tsxFiles(dir: string): string[] {
  const out: string[] = [];
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const full = join(dir, entry.name);
    if (entry.isDirectory()) out.push(...tsxFiles(full));
    else if (entry.name.endsWith(".tsx")) out.push(full);
  }
  return out;
}

describe("S12 AC1 fixes-first", () => {
  it("no component sheet references the dangling --color-vermilion token", () => {
    for (const file of cssFiles(COMPONENTS)) {
      const text = readFileSync(file, "utf8");
      expect(text, `${file} must not reference --color-vermilion`).not.toContain(
        "--color-vermilion",
      );
    }
  });

  it("no TSX references the dangling .btn-quiet class", () => {
    for (const file of tsxFiles(SRC)) {
      const text = readFileSync(file, "utf8");
      expect(text, `${file} must not reference btn-quiet`).not.toContain(
        "btn-quiet",
      );
    }
  });

  it("the dead ModelBrowser.tsx and browser.css orphans are deleted", () => {
    expect(existsSync(join(COMPONENTS, "ModelBrowser.tsx"))).toBe(false);
    expect(existsSync(join(COMPONENTS, "browser.css"))).toBe(false);
  });
});
