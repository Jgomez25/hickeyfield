import { useMemo, useState } from "react";
import type { JobSet, Storyboard } from "../types";
import { gradientDataUri } from "../lib/placeholder";
import { posterFor } from "../lib/media";
import { isFailure } from "../lib/status";
import { stageCostCaption } from "../lib/cost";
import { EmptyState } from "./EmptyState";

type Filter = "all" | "video" | "image" | "failed";

const FILTERS: { id: Filter; label: string }[] = [
  { id: "all", label: "All" },
  { id: "video", label: "Video" },
  { id: "image", label: "Image" },
  { id: "failed", label: "Failed" },
];

function hasKind(job: JobSet, kind: "video" | "image"): boolean {
  return (job.results ?? []).some((r) => r.kind === kind);
}

function matches(job: JobSet, filter: Filter): boolean {
  switch (filter) {
    case "video":
      return hasKind(job, "video");
    case "image":
      return hasKind(job, "image");
    case "failed":
      return isFailure(job.status);
    default:
      return true;
  }
}

function titleOf(job: JobSet): string {
  const line = (job.prompt ?? "").split("\n")[0].trim();
  return line.length > 0 ? line : "Untitled";
}

function thumbFor(job: JobSet): string {
  const first = job.results?.[0];
  return first ? posterFor(first) : gradientDataUri(job.id);
}

/**
 * The Library gallery — a second presentation of the same `jobs` array the feed
 * and meta rail already render (§D5.1: no new Rust command, no persistence).
 *
 * Each tile carries `data-job-anchor` so the App selection→scroll pairing with
 * the meta rail keeps working, and clicking a tile both selects the job (so its
 * MetaCard highlights) and opens the lightbox App renders.
 */
export function Library({
  jobs,
  selectedId,
  onSelect,
  onOpenLightbox,
  storyboards = [],
}: {
  jobs: JobSet[];
  selectedId: string | null;
  onSelect: (id: string) => void;
  onOpenLightbox: (id: string) => void;
  /**
   * Storyboard group headers. Additive (S13): a job with `storyboardId` renders
   * inside its set's group, headed by the set's brief; standalone jobs render
   * exactly as before. Empty (the default) means no grouping, so the S12 test
   * that omits it is unchanged.
   */
  storyboards?: Storyboard[];
}) {
  const [filter, setFilter] = useState<Filter>("all");
  const shown = useMemo(
    () => jobs.filter((j) => matches(j, filter)),
    [jobs, filter],
  );
  const unpriced = useMemo(
    () => jobs.filter((j) => stageCostCaption(j).tone === "unpriced").length,
    [jobs],
  );
  // Split the shown jobs into storyboard groups (in the storyboards' order) and
  // the standalone remainder. A group with no shown jobs is omitted.
  const groups = useMemo(() => {
    const byId = new Map<string, JobSet[]>();
    for (const j of shown) {
      if (j.storyboardId) {
        const list = byId.get(j.storyboardId) ?? [];
        list.push(j);
        byId.set(j.storyboardId, list);
      }
    }
    const ordered = storyboards
      .filter((s) => byId.has(s.id))
      .map((s) => ({ storyboard: s, jobs: sortByShot(byId.get(s.id)!) }));
    // Any grouped id with no matching header still renders under a generic head.
    for (const [id, list] of byId) {
      if (!ordered.some((g) => g.storyboard.id === id)) {
        ordered.push({
          storyboard: {
            id,
            brief: "Storyboard",
            targetSeconds: 0,
            planJson: "{}",
            createdAt: 0,
          },
          jobs: sortByShot(list),
        });
      }
    }
    return ordered;
  }, [shown, storyboards]);
  const standalone = useMemo(
    () => shown.filter((j) => !j.storyboardId),
    [shown],
  );

  return (
    <section className="library" aria-label="Library">
      <header className="library-head">
        <h1 className="library-title">LIBRARY</h1>
        <span className="stage-sub">
          {jobs.length} item{jobs.length === 1 ? "" : "s"}
          {unpriced > 0 ? ` · ${unpriced} unpriced` : ""}
        </span>
        <div className="library-filters" role="group" aria-label="Filter">
          {FILTERS.map((f) => (
            <button
              key={f.id}
              type="button"
              className="chip chip-button"
              aria-pressed={filter === f.id}
              onClick={() => setFilter(f.id)}
            >
              {f.label}
            </button>
          ))}
        </div>
      </header>

      {shown.length === 0 ? (
        <EmptyState
          heading="Nothing here yet"
          explanation="Generations you make land in the Library, newest first. Change the filter to see other kinds."
        />
      ) : (
        <>
          {groups.map((g) => (
            <section
              className="library-storyboard-group"
              key={g.storyboard.id}
              data-storyboard={g.storyboard.id}
            >
              <h2 className="library-storyboard-head">
                <span className="library-storyboard-brief">
                  {g.storyboard.brief || "Storyboard"}
                </span>
                <span className="library-storyboard-count">
                  {g.jobs.length} shot{g.jobs.length === 1 ? "" : "s"}
                </span>
              </h2>
              <div className="library-grid">
                {g.jobs.map((job) =>
                  tile(job, selectedId, onSelect, onOpenLightbox),
                )}
              </div>
            </section>
          ))}
          {standalone.length > 0 ? (
            <div className="library-grid">
              {standalone.map((job) =>
                tile(job, selectedId, onSelect, onOpenLightbox),
              )}
            </div>
          ) : null}
        </>
      )}
    </section>
  );
}

/** Newest-first is the list order; within a set, `shotIndex` order reads best. */
function sortByShot(jobs: JobSet[]): JobSet[] {
  return [...jobs].sort((a, b) => (a.shotIndex ?? 0) - (b.shotIndex ?? 0));
}

/** One gallery tile. Extracted so grouped and standalone tiles are identical. */
function tile(
  job: JobSet,
  selectedId: string | null,
  onSelect: (id: string) => void,
  onOpenLightbox: (id: string) => void,
) {
  const cost = stageCostCaption(job);
  const failed = isFailure(job.status);
  return (
    <button
      key={job.id}
      type="button"
      className="library-tile"
      data-job-anchor={job.id}
      data-selected={job.id === selectedId || undefined}
      data-failed={failed || undefined}
      onClick={() => {
        onSelect(job.id);
        onOpenLightbox(job.id);
      }}
      onMouseEnter={() => onSelect(job.id)}
      style={failed ? undefined : { backgroundImage: `url("${thumbFor(job)}")` }}
      aria-label={titleOf(job)}
    >
      {failed ? null : <span className="media-bevel" aria-hidden="true" />}
      <span className="library-tile-caption">
        <span className="library-tile-name">{titleOf(job)}</span>
        <span
          className="library-tile-cost"
          data-unpriced={cost.tone === "unpriced" || undefined}
        >
          {cost.text}
        </span>
      </span>
    </button>
  );
}
