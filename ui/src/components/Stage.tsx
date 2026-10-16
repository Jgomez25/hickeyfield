import { useEffect, useState } from "react";
import type { JobSet } from "../types";
import { gradientDataUri } from "../lib/placeholder";
import { isPlayableVideo, posterFor, formatDuration } from "../lib/media";
import { isRunning, isFailure, statusLabel } from "../lib/status";
import { stageCostCaption, formatUsd } from "../lib/cost";
import { VideoPreview } from "./VideoPreview";
import { RainbowPlaceholder, StatusLine } from "./Loader";

/** mm:ss from a millisecond elapsed count. */
function clock(ms: number): string {
  const total = Math.max(0, Math.floor(ms / 1000));
  const m = Math.floor(total / 60);
  const s = total % 60;
  return `${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
}

/** Ticks every second while `running`, so the stage timer counts up live. */
function useElapsed(createdAt: string | undefined, running: boolean): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (!running) return;
    const id = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(id);
  }, [running]);
  if (!createdAt) return 0;
  const started = Date.parse(createdAt);
  return Number.isFinite(started) ? now - started : 0;
}

/** First line of the prompt, the caption's title. */
function titleOf(job: JobSet): string {
  const line = (job.prompt ?? "").split("\n")[0].trim();
  return line.length > 0 ? line : "Untitled";
}

/** The first output that made it to disk, if any. */
function savedPath(job: JobSet): string | null {
  return job.results?.find((r) => r.localPath)?.localPath ?? null;
}

/** A compact cost for the small recents tiles: prefer actual, then estimate. */
function tileCost(job: JobSet): { text: string; unpriced: boolean } {
  if (typeof job.actualUsd === "number" && job.actualUsd >= 0) {
    return { text: formatUsd(job.actualUsd), unpriced: false };
  }
  if (typeof job.estimatedUsd === "number" && job.estimatedUsd >= 0) {
    return { text: formatUsd(job.estimatedUsd), unpriced: false };
  }
  return { text: "unpriced", unpriced: true };
}

function thumbFor(job: JobSet): string {
  const first = job.results?.[0];
  return first ? posterFor(first) : gradientDataUri(job.id);
}

/**
 * The Studio centre column: the latest (or selected) result large with its
 * caption and action pills, a recents strip below, and the submit-error banner.
 *
 * Stage owns no fetching — everything is data App already holds. The recents
 * tiles carry `data-job-anchor` so the App selection→scroll pairing effect
 * keeps working; the meta anchor is simply absent on Studio, which the effect's
 * optional-chaining no-ops.
 */
export function Stage({
  workspaceTitle = "Creative studio",
  imageMode = false,
  onPromptExample,
  job,
  recents,
  modelName,
  error,
  onDismissError,
  onSelect,
  onRetry,
  onExtend,
  onAnimate,
  onReveal,
}: {
  workspaceTitle?: string;
  imageMode?: boolean;
  onPromptExample?: (text: string) => void;
  /** The featured job — the selection, or the latest. Null when none exist. */
  job: JobSet | null;
  recents: JobSet[];
  modelName: string;
  error?: string | null;
  onDismissError?: () => void;
  onSelect: (id: string) => void;
  onRetry: (job: JobSet) => void;
  /** null hides the Extend pill: no extend use case is available (§D4). */
  onExtend: ((job: JobSet) => void) | null;
  onAnimate?: (job: JobSet) => void;
  onReveal: (path: string) => void;
}) {
  const running = job ? isRunning(job.status) : false;
  const failed = job ? isFailure(job.status) : false;
  const elapsed = useElapsed(job?.createdAt, running);
  const saved = job ? savedPath(job) : null;

  return (
    <section className="stage" aria-label="Studio">
      <header className="canvas-heading"><div><p className="eyebrow">CREATE WITHOUT LIMITS</p><h2>{workspaceTitle}</h2><p>{imageMode ? "Find your visual language. Make every detail your own." : "Your next scene starts here. Direct it, refine it, make it yours."}</p></div><span className="canvas-count">{recents.length} recent {recents.length === 1 ? "creation" : "creations"}</span></header>
      {error ? (
        <div className="feed-error" role="alert">
          <p className="feed-error-text">{error}</p>
          {onDismissError ? (
            <button
              type="button"
              className="btn btn-ghost btn-sm feed-error-dismiss"
              onClick={onDismissError}
            >
              Dismiss
            </button>
          ) : null}
        </div>
      ) : null}

      {job === null ? (
        <div className="canvas-welcome">
          <img src={imageMode ? "/art/portrait.jpg" : "/art/cinema.jpg"} alt="" />
          <div><span className="feature-tag">{imageMode ? "A NEW PERSPECTIVE" : "YOUR NEXT FRAME"}</span><h3>{imageMode ? "An idea. A thousand possibilities." : "Every great scene starts with an idea."}</h3><p>Choose a model, describe your vision, and create your first {imageMode ? "image" : "video"}.</p>
          {onPromptExample && <button className="btn btn-primary" onClick={() => onPromptExample(imageMode ? "Editorial portrait in a sculptural silver jacket, amber sunglasses, vivid blue sky, hard sunlight, analog film texture." : "A lone astronaut crosses volcanic dunes beneath a glowing planet. Slow cinematic dolly in, warm rim light, drifting dust, 35mm film.")}>Try a prompt <span aria-hidden="true">↗</span></button>}
          <small>Creative inspiration · Original studio artwork</small></div>
        </div>
      ) : (
        <>
          <header className="stage-head">
            <h3 className="stage-title">Preview</h3>
            <span className="stage-sub">Your selected creation</span>
            {running ? (
              <span className="stage-timer">
                <span className="stage-timer-dot" aria-hidden="true" />
                Rendering {clock(elapsed)}
              </span>
            ) : null}
          </header>

          <StageHero
            job={job}
            modelName={modelName}
            running={running}
            failed={failed}
            saved={saved}
            onRetry={onRetry}
            onExtend={onExtend}
            onAnimate={onAnimate}
            onReveal={onReveal}
          />

          {recents.length > 0 ? (
            <div className="stage-recents">
              <div className="stage-recents-head">
                <span className="stage-recents-title">Recent creations</span>
                <span className="stage-sub">{recents.length} shown</span>
              </div>
              <div className="stage-recents-grid">
                {recents.map((r) => {
                  const cost = tileCost(r);
                  const rr = isRunning(r.status);
                  return (
                    <button
                      key={r.id}
                      type="button"
                      className="stage-recent-tile"
                      data-job-anchor={r.id}
                      data-selected={r.id === job.id || undefined}
                      onClick={() => onSelect(r.id)}
                      style={{ backgroundImage: `url("${thumbFor(r)}")` }}
                      aria-label={titleOf(r)}
                    >
                      <span className="media-bevel" aria-hidden="true" />
                      <span className="stage-recent-name">{titleOf(r)}</span>
                      <span
                        className="stage-recent-cost"
                        data-running={rr || undefined}
                        data-unpriced={cost.unpriced || undefined}
                      >
                        {rr ? statusLabel(r.status) : cost.text}
                      </span>
                    </button>
                  );
                })}
              </div>
            </div>
          ) : null}
        </>
      )}
    </section>
  );
}

function StageHero({
  job,
  modelName,
  running,
  failed,
  saved,
  onRetry,
  onExtend,
  onAnimate,
  onReveal,
}: {
  job: JobSet;
  modelName: string;
  running: boolean;
  failed: boolean;
  saved: string | null;
  onRetry: (job: JobSet) => void;
  onExtend: ((job: JobSet) => void) | null;
  onAnimate?: (job: JobSet) => void;
  onReveal: (path: string) => void;
}) {
  const first = job.results?.[0];
  const cost = stageCostCaption(job);
  const settings = job.settings;
  const detail = [
    modelName,
    settings && first?.kind !== "image" ? formatDuration(settings.duration) : null,
    settings?.resolution,
  ]
    .filter(Boolean)
    .join(" · ");

  return (
    <div className="stage-hero">
      <div className="stage-hero-media">
        {running ? (
          <span className="stage-hero-running">
            <RainbowPlaceholder className="stage-hero-rainbow" ratio={16 / 9} />
            <span className="stage-hero-status">
              <StatusLine label={statusLabel(job.status)} />
            </span>
          </span>
        ) : failed ? (
          <span className="stage-hero-failed">{job.failReason ?? statusLabel(job.status)}</span>
        ) : first && isPlayableVideo(first) ? (
          <VideoPreview result={first} className="stage-hero-video" />
        ) : first ? (
          <img className="stage-hero-img" src={posterFor(first)} alt="" />
        ) : (
          <img className="stage-hero-img" src={gradientDataUri(job.id)} alt="" />
        )}
        <span className="media-bevel" aria-hidden="true" />
      </div>

      <div className="stage-hero-caption">
        <span className="stage-hero-name">{titleOf(job)}</span>
        <span className="stage-hero-detail">
          {detail}
          {detail ? " · " : ""}
          <span className="stage-hero-cost" data-unpriced={cost.tone === "unpriced" || undefined}>
            {cost.text}
          </span>
        </span>
      </div>

      <div className="stage-hero-pills">
        {onAnimate && first?.kind === "image" && (first.localPath || first.url) ? <button type="button" className="stage-pill" onClick={() => onAnimate(job)}>Animate image</button> : null}
        {onExtend && saved && first?.kind === "video" ? (
          <button
            type="button"
            className="stage-pill"
            onClick={() => onExtend(job)}
          >
            Extend
          </button>
        ) : null}
        <button
          type="button"
          className="stage-pill"
          onClick={() => onRetry(job)}
        >
          Retry
        </button>
        {saved ? (
          <button
            type="button"
            className="stage-pill"
            onClick={() => onReveal(saved)}
            title="Show the file in Finder"
          >
            Reveal
          </button>
        ) : null}
      </div>
    </div>
  );
}
