import { useMemo, useState } from "react";
import type {
  JobSet,
  JoinRecord,
  RenderProgress,
  Storyboard,
  StoryboardRender,
} from "../types";
import { gradientDataUri } from "../lib/placeholder";
import { posterFor, displayUrl } from "../lib/media";
import { isFailure } from "../lib/status";
import { formatUsd, stageCostCaption } from "../lib/cost";
import { EmptyState } from "./EmptyState";

/** A shot is "done" when it completed and its clip is downloaded locally — the
 * gate for rendering, since the stitch reads the local files. */
/**
 * S16: the set's minimum continuity-join score, as a badge. Reads the scored
 * joins (an unscored join carries `null` numbers, never 0, so it is excluded).
 * `data-pass` is false when any scored join is below the floor, so the badge can
 * flag a low set at a glance. Returns null when nothing has been scored.
 */
function joinSummary(joins: JoinRecord[] | undefined) {
  const scored = (joins ?? []).filter((j) => j.ssim != null);
  if (scored.length === 0) return null;
  const min = scored.reduce(
    (lo, j) => (j.ssim != null && j.ssim < lo ? j.ssim : lo),
    1,
  );
  const anyBelow = scored.some((j) => j.pass === false);
  return (
    <span
      className="storyboard-join"
      data-pass={anyBelow ? "false" : "true"}
      title="Lowest continuity-join score across the set"
    >
      join {min.toFixed(2)}
      {anyBelow ? " — low" : " — continuous"}
    </span>
  );
}

/**
 * S17: the set's end-frame-adherence badge — how tightly each clip's actual last
 * frame matched its planned still (keyframe mode). Reads the scored ends only
 * (an unscored end is `null`, never 0, so it shows "end —" and never flags a
 * paid regenerate). Returns null for a non-keyframe set (no end scores at all).
 */
function endAdherenceSummary(joins: JoinRecord[] | undefined) {
  const all = joins ?? [];
  // A keyframe set has end-* fields present (even if unscored → null); a
  // non-keyframe set omits them entirely (undefined) → no badge.
  const hasEndField = all.some((j) => j.endSsim !== undefined);
  if (!hasEndField) return null;
  const scored = all.filter((j) => j.endSsim != null);
  if (scored.length === 0) {
    return (
      <span className="storyboard-end" data-pass="unscored" title="End-frame adherence not yet scored">
        end —
      </span>
    );
  }
  const min = scored.reduce(
    (lo, j) => (j.endSsim != null && j.endSsim < lo ? j.endSsim : lo),
    1,
  );
  const anyBelow = scored.some((j) => j.endPass === false);
  return (
    <span
      className="storyboard-end"
      data-pass={anyBelow ? "false" : "true"}
      title="Lowest end-frame-adherence score across the set"
    >
      end {min.toFixed(2)}
      {anyBelow ? " — drift" : " — anchored"}
    </span>
  );
}

function isDone(job: JobSet): boolean {
  return (
    job.status === "completed" &&
    (job.results ?? []).some((r) => r.localPath)
  );
}

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
 * S17: the keyframe-anchored set's still boundaries, one card per boundary
 * (K0..KN). A settled still shows its downloaded PNG thumbnail; a still still in
 * flight shows a "rendering" note; a **failed** still shows the PARKED state —
 * the set will not submit a shot against a missing anchor (D2) — and every card
 * offers **Regenerate still** (D5). The still's live status is read from its own
 * job row (`jobSetId`), so "parked" vs "rendering" is honest. Returns null for a
 * non-keyframe set (no boundaries), so a plain S13/S15/S16 set is unchanged.
 */
function keyframeStills(
  storyboard: Storyboard,
  jobs: JobSet[],
  onRegenerateStill?: (storyboardId: string, index: number) => void,
) {
  const keyframes = storyboard.keyframes ?? [];
  if (keyframes.length === 0) return null;
  const jobById = new Map(jobs.map((j) => [j.id, j]));
  return (
    <div className="storyboard-keyframe-stills" data-keyframe-set={storyboard.id}>
      {keyframes.map((kf) => {
        const job = kf.jobSetId ? jobById.get(kf.jobSetId) : undefined;
        const failed = job != null && isFailure(job.status);
        const settled = Boolean(kf.localPath);
        const state = settled ? "ready" : failed ? "parked" : "pending";
        return (
          <div
            className="storyboard-keyframe-card"
            data-keyframe={kf.index}
            data-state={state}
            key={kf.index}
          >
            <span className="storyboard-keyframe-index">K{kf.index}</span>
            {settled ? (
              <img
                className="storyboard-keyframe-thumb"
                src={displayUrl({
                  url: "",
                  kind: "image",
                  localPath: kf.localPath ?? null,
                })}
                alt={`Keyframe ${kf.index} still`}
              />
            ) : (
              <span className="storyboard-keyframe-state" data-state={state}>
                {failed
                  ? "still failed — the set is parked"
                  : "still rendering…"}
              </span>
            )}
            {onRegenerateStill ? (
              <button
                type="button"
                className="storyboard-regenerate-still"
                onClick={() => onRegenerateStill(storyboard.id, kf.index)}
              >
                Regenerate still
              </button>
            ) : null}
          </div>
        );
      })}
    </div>
  );
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
  renders = [],
  renderProgress = {},
  onRender,
  onOpenResult,
  onRegenerateStill,
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
  /** S15: finished renders, keyed by storyboard. A set with a render shows its
   * finished ad tile + Σ actual + Open result; one without shows a Render
   * button (enabled once every shot is Done). */
  renders?: StoryboardRender[];
  /** S15: live render progress, by storyboard id. */
  renderProgress?: Record<string, RenderProgress>;
  /** S15: start a render for a set. Absent → the render controls are hidden. */
  onRender?: (storyboardId: string) => void;
  /** S15: open a finished render in the OS. */
  onOpenResult?: (path: string) => void;
  /** S17: regenerate a keyframe boundary's still. Absent → the Regenerate-still
   * button is hidden. */
  onRegenerateStill?: (storyboardId: string, index: number) => void;
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
                {joinSummary(g.storyboard.joins)}
                {endAdherenceSummary(g.storyboard.joins)}
              </h2>
              {keyframeStills(
                g.storyboard,
                g.jobs,
                onRegenerateStill,
              )}
              <div className="library-grid">
                {g.jobs.map((job) =>
                  tile(job, selectedId, onSelect, onOpenLightbox),
                )}
              </div>
              {onRender
                ? renderControls(
                    g.storyboard.id,
                    g.jobs,
                    renders.find((r) => r.storyboardId === g.storyboard.id),
                    renderProgress[g.storyboard.id],
                    onRender,
                    onOpenResult,
                  )
                : null}
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

/**
 * S15: the render controls for one storyboard set.
 *
 * - A finished render → its ad line with Σ actual cost (never $0 when a member
 *   is unpriced), its duration, and **Open result**.
 * - No render yet → a **Render** button, enabled only when every shot is Done
 *   (completed + downloaded), so a half-finished set cannot be stitched.
 * - While rendering → a real progress bar (the one honest percentage: the
 *   duration is known).
 */
function renderControls(
  storyboardId: string,
  jobs: JobSet[],
  render: StoryboardRender | undefined,
  progress: RenderProgress | undefined,
  onRender: (id: string) => void,
  onOpenResult?: (path: string) => void,
) {
  const allDone = jobs.length > 0 && jobs.every(isDone);
  const rendering = progress !== undefined && !progress.done;
  const cost =
    render == null
      ? null
      : render.actualUsd == null
        ? "unpriced"
        : formatUsd(render.actualUsd);

  return (
    <div className="storyboard-render-controls" data-storyboard-render={storyboardId}>
      {render ? (
        <div className="storyboard-result">
          <span className="storyboard-result-cost" data-unpriced={cost === "unpriced" || undefined}>
            Total actual {cost}
          </span>
          {render.seconds != null ? (
            <span className="storyboard-result-dur">{render.seconds.toFixed(1)}s</span>
          ) : null}
          <button
            type="button"
            className="storyboard-open-result"
            onClick={() => onOpenResult?.(render.outputPath)}
          >
            Open result
          </button>
        </div>
      ) : (
        <button
          type="button"
          className="storyboard-render"
          disabled={!allDone || rendering}
          onClick={() => onRender(storyboardId)}
        >
          {rendering
            ? "Rendering…"
            : allDone
              ? "Render the ad"
              : "Render (all shots must finish)"}
        </button>
      )}
      {rendering ? (
        <div
          className="storyboard-render-progress"
          role="progressbar"
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={Math.round((progress?.percent ?? 0) * 100)}
        >
          <span
            className="storyboard-render-progress-fill"
            style={{ width: `${Math.round((progress?.percent ?? 0) * 100)}%` }}
          />
        </div>
      ) : null}
      <span className="storyboard-render-free">Rendering is free — it runs on your machine.</span>
    </div>
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
