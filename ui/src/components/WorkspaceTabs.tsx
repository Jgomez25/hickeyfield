import type { ReactNode } from "react";
import type { UseCase, WorkspaceTab } from "../types";

/**
 * Original, single-weight line icons for the use-case nav. Drawn here rather
 * than traced from any product's iconography (same rule as Icons.tsx). Keyed by
 * the Rust use-case slug so the right glyph lands whatever order the roster
 * arrives in; an unmapped slug falls back to a neutral square.
 */
const NAV_ICONS: Record<string, ReactNode> = {
  "text-to-video": (
    <>
      <rect x="3" y="5" width="18" height="14" rx="2" />
      <path d="M10 9l5 3-5 3z" />
    </>
  ),
  "image-to-video": (
    <>
      <rect x="3" y="3" width="18" height="18" rx="2" />
      <circle cx="9" cy="9" r="2" />
      <path d="M21 15l-5-5L5 21" />
    </>
  ),
  "text-to-image": (
    <>
      <rect x="3" y="3" width="18" height="18" rx="2" />
      <path d="M3 9h18M9 21V9" />
    </>
  ),
  "edit-image": (
    <>
      <path d="M12 20h9" />
      <path d="M16.5 3.5a2.1 2.1 0 013 3L7 19l-4 1 1-4z" />
    </>
  ),
  "edit-video": <path d="M4 6h16M4 12h10M4 18h16" />,
};

function NavGlyph({ slug }: { slug: string }) {
  return (
    <svg
      width="20"
      height="20"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.75}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden
      focusable={false}
    >
      {NAV_ICONS[slug] ?? <rect x="4" y="4" width="16" height="16" rx="3" />}
    </svg>
  );
}

/**
 * Roving-tabindex tablist: only the selected tab is in the tab order, and the
 * arrow keys move between them. Three adjacent buttons that all take a Tab
 * stop is the most common way a segmented control fails a keyboard pass.
 *
 * `variant="nav"` restyles the same tablist as the vertical 72px use-case rail
 * (icon over label). It is a class + icon change only — the roles, `data-tab`,
 * roving tabindex and `onChange` are identical, so the AC3 nav→tab wiring is the
 * same code path the row variant is already tested against.
 */
export function WorkspaceTabs({
  value,
  onChange,
  panelId,
  useCases,
  variant = "row",
}: {
  value: WorkspaceTab;
  onChange: (tab: WorkspaceTab) => void;
  panelId: string;
  /** Comes from Rust, so a tab can never exist that no model can serve. */
  useCases: UseCase[];
  variant?: "row" | "nav";
}) {
  const TABS = useCases.map((u) => ({ id: u.slug, label: u.label, blurb: u.blurb }));
  const index = TABS.findIndex((t) => t.id === value);
  if (TABS.length === 0) return null;

  const onKeyDown = (e: React.KeyboardEvent<HTMLDivElement>) => {
    // Down/Right advance, Up/Left retreat — so the same handler works for the
    // horizontal row and the vertical nav.
    const delta =
      e.key === "ArrowRight" || e.key === "ArrowDown"
        ? 1
        : e.key === "ArrowLeft" || e.key === "ArrowUp"
          ? -1
          : 0;
    if (delta === 0) return;
    e.preventDefault();
    const next = TABS[(index + delta + TABS.length) % TABS.length];
    onChange(next.id);
    const el = e.currentTarget.querySelector<HTMLButtonElement>(
      `[data-tab="${next.id}"]`,
    );
    el?.focus();
  };

  const nav = variant === "nav";

  return (
    <div
      className={nav ? "tabs tabs-nav" : "tabs"}
      role="tablist"
      aria-label="Workspace"
      aria-orientation={nav ? "vertical" : undefined}
      onKeyDown={onKeyDown}
    >
      {TABS.map((tab) => (
        <button
          key={tab.id}
          type="button"
          role="tab"
          data-tab={tab.id}
          className={nav ? "tabs-tab tabs-nav-tab" : "tabs-tab"}
          aria-selected={tab.id === value}
          aria-controls={panelId}
          tabIndex={tab.id === value ? 0 : -1}
          onClick={() => onChange(tab.id)}
          title={tab.blurb}
        >
          {nav ? <NavGlyph slug={tab.id} /> : null}
          <span className={nav ? "tabs-nav-label" : undefined}>{tab.label}</span>
        </button>
      ))}
    </div>
  );
}
