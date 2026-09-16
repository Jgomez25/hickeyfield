import { useMemo, useState } from "react";
import type { JobSet } from "../types";
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
}: {
  jobs: JobSet[];
  selectedId: string | null;
  onSelect: (id: string) => void;
  onOpenLightbox: (id: string) => void;
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
        <div className="library-grid">
          {shown.map((job) => {
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
                style={
                  failed
                    ? undefined
                    : { backgroundImage: `url("${thumbFor(job)}")` }
                }
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
          })}
        </div>
      )}
    </section>
  );
}
