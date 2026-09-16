//! Rendering a storyboard set into one finished ad (S15 §D3).
//!
//! Every shot's downloaded clip is normalised, joined (a continuity join is a
//! hard cut, a non-continuity join a crossfade — §D-B), concatenated, and
//! written through the bundled ffmpeg sidecar with a real progress bar. The
//! result is adopted into the Library as a set item carrying the set's Σ actual
//! cost. **The render spends nothing**: it spawns only the local sidecar on
//! local files — no provider, no network, no charge — and its cost figure is
//! the sum of the member shots already billed (S10), never a new charge.

use crate::app::AppState;
use crate::store::StoryboardRender;
use hickeyfield_core::compositor::{Clip, OutputSpec, Timeline};
use hickeyfield_core::ffmpeg::{self, Encoder, Quality};
use hickeyfield_core::MediaRole;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Duration;
use tauri::{AppHandle, Emitter, State};

/// How to render. Defaults: the set's stored overlap (or the crossfade default)
/// and no captions.
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RenderOptions {
    /// Crossfade seconds for non-continuity joins. `None` reads the set's
    /// stored `overlap_seconds` (0.0 for a chained set — its joins are hard
    /// cuts), falling back to `DEFAULT_CROSSFADE_SECONDS`.
    #[serde(default)]
    pub overlap_seconds: Option<f64>,
}

/// A finished render, for the bridge.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StoryboardRenderDto {
    pub storyboard_id: String,
    pub output_path: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub seconds: Option<f64>,
    /// Σ of the member shots' actual cost (S10). `None` if any member is
    /// unpriced — never summed as $0. The render itself adds nothing.
    pub actual_usd: Option<f64>,
    pub created_at: i64,
}

impl From<StoryboardRender> for StoryboardRenderDto {
    fn from(r: StoryboardRender) -> Self {
        StoryboardRenderDto {
            storyboard_id: r.storyboard_id,
            output_path: r.output_path,
            width: r.width,
            height: r.height,
            seconds: r.seconds,
            actual_usd: r.actual_usd,
            created_at: r.created_at,
        }
    }
}

/// A progress tick emitted to the UI on `storyboard:render`.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RenderProgressDto {
    pub storyboard_id: String,
    pub percent: f64,
    pub done: bool,
}

const fn even(n: u32) -> u32 {
    n & !1
}

/// Render a completed storyboard set into one stitched ad.
#[tauri::command]
pub fn render_storyboard(
    app: AppHandle,
    state: State<'_, AppState>,
    storyboard_id: String,
    options: RenderOptions,
) -> Result<StoryboardRenderDto, String> {
    let sb = state
        .store
        .storyboard(&storyboard_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("no such storyboard: {storyboard_id}"))?;

    let jobs = state
        .store
        .jobs_in_storyboard(&storyboard_id)
        .map_err(|e| e.to_string())?;
    if jobs.is_empty() {
        return Err("this storyboard has no shots to render.".into());
    }

    // Server-side gate (§10.5), not only the UI's disabled button: every shot
    // must be Completed with a local video output, or the timeline maths and
    // the render are undefined.
    let bin = ffmpeg::locate(&state.ffmpeg_dir)
        .map_err(|e| format!("the ffmpeg sidecar is unavailable: {e}"))?;

    let overlap = options
        .overlap_seconds
        .or(sb.overlap_seconds)
        .unwrap_or(hickeyfield_core::storyboard::DEFAULT_CROSSFADE_SECONDS);

    // The recorded per-join kinds (S16 D8), authoritative over the media-
    // presence heuristic: a scored set records each join's `kind` in
    // `joins_json`, so a partially-chained set (a shot that fell back text-only
    // while the set is chained) renders each join as what actually happened,
    // not a re-derivation. Empty for a set generated before this slice.
    let joins = crate::store::parse_joins(sb.joins_json.as_deref());

    // Probe each clip for its duration and dimensions; build the ordered clip
    // list with per-join kinds.
    let mut clips: Vec<Clip> = Vec::with_capacity(jobs.len());
    let mut max_w = 0u32;
    let mut max_h = 0u32;
    let mut actual_sum = Some(0.0f64);
    for (i, job) in jobs.iter().enumerate() {
        let local = job
            .results
            .iter()
            .find_map(|o| o.local_path.clone())
            .ok_or_else(|| {
                format!(
                    "shot {} is not finished (no downloaded clip) — every shot must be Done to render.",
                    i + 1
                )
            })?;
        let dims = ffmpeg::probe_file(&bin, Path::new(&local)).map_err(|e| e.to_string())?;
        let dur = dims.seconds.ok_or_else(|| {
            format!("could not read the duration of shot {} — refusing to render a set whose timeline cannot be computed.", i + 1)
        })?;
        if let (Some(w), Some(h)) = (dims.width, dims.height) {
            max_w = max_w.max(w);
            max_h = max_h.max(h);
        }
        // Σ actual: unknown if ANY member is unpriced (never summed as $0).
        actual_sum = match (actual_sum, job.actual_usd) {
            (Some(acc), Some(v)) => Some(acc + v),
            _ => None,
        };

        let mut clip = Clip::new(local, 0.0, dur);
        if i > 0 {
            // Prefer the recorded join kind (D8); fall back to the media-
            // presence heuristic only for a set with no scored join at this
            // index (generated before this slice, or never scored). A
            // continuity join is a hard cut (no crossfade); a crossfade join
            // blends. §D-B.
            let continuity = match joins.iter().find(|j| j.index == i) {
                Some(rec) => rec.kind == "continuity",
                None => sb.chained && job.media.iter().any(|m| m.role == MediaRole::Start),
            };
            if !continuity && overlap > 0.0 {
                clip = clip.crossfade_in(overlap);
            }
        }
        clips.push(clip);
    }

    if max_w == 0 || max_h == 0 {
        return Err(
            "could not read the dimensions of the shots — refusing to guess a canvas.".into(),
        );
    }
    let (w, h) = (even(max_w), even(max_h));

    // Output into a temp file under the set's render dir, then adopt into the
    // Library. The encoder is the host's best (VideoToolbox on this Mac).
    let render_dir = state.renders_root.join(&storyboard_id);
    std::fs::create_dir_all(&render_dir).map_err(|e| e.to_string())?;
    let tmp_out = render_dir.join("storyboard.mp4");
    let _ = std::fs::remove_file(&tmp_out);

    let encoder = ffmpeg::probe_encoders(&bin).unwrap_or(Encoder::Libx264);
    let mut spec = OutputSpec::new(w, h, 30, tmp_out.clone());
    spec.encoder = encoder;
    spec.crf_or_bitrate = Quality::Crf(20);
    let timeline = Timeline::new(clips, spec);

    let problems = timeline.validate();
    if !problems.is_empty() {
        return Err(format!(
            "the render timeline is invalid: {}",
            problems.join("; ")
        ));
    }
    let total = timeline.total_duration();

    // Timeout scaled to the render's own length: a 30s ad on VideoToolbox
    // finishes far inside this; a wedged encode is killed (§D6).
    let timeout = Duration::from_secs(60) + Duration::from_secs_f64(total * 30.0);

    let sid_for_progress = storyboard_id.clone();
    let app_for_progress = app.clone();
    let on_progress = move |p: ffmpeg::Progress| {
        let _ = app_for_progress.emit(
            "storyboard:render",
            RenderProgressDto {
                storyboard_id: sid_for_progress.clone(),
                percent: p.percent(total),
                done: false,
            },
        );
    };

    ffmpeg::render(&bin, &timeline.to_args(), timeout, on_progress).map_err(|e| e.to_string())?;

    // Probe the output and record what actually landed.
    let probed = ffmpeg::probe_file(&bin, &tmp_out).map_err(|e| e.to_string())?;

    // Adopt the finished ad into the Library as a real, findable item.
    let stem = if sb.brief.trim().is_empty() {
        "storyboard".to_string()
    } else {
        format!("{}-storyboard", sb.brief.trim())
    };
    let final_path = state
        .library
        .adopt(&tmp_out, &stem, sb.created_at)
        .map_err(|e| e.to_string())?;

    let render = StoryboardRender {
        storyboard_id: storyboard_id.clone(),
        output_path: final_path.to_string_lossy().into_owned(),
        width: probed.width,
        height: probed.height,
        seconds: probed.seconds,
        actual_usd: actual_sum,
        created_at: crate::runner::now_secs(),
    };
    state
        .store
        .upsert_render(&render)
        .map_err(|e| e.to_string())?;

    // Clean the intermediates now the ad is safe in the Library; keep the
    // frames dir (the deferred continuity-score slice reuses the PNGs).
    let _ = std::fs::remove_file(&tmp_out);

    let _ = app.emit(
        "storyboard:render",
        RenderProgressDto {
            storyboard_id: storyboard_id.clone(),
            percent: 1.0,
            done: true,
        },
    );

    Ok(render.into())
}

/// The stored renders, for the Library's set items.
#[tauri::command]
pub fn list_storyboard_renders(
    state: State<'_, AppState>,
) -> Result<Vec<StoryboardRenderDto>, String> {
    Ok(state
        .store
        .renders()
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(StoryboardRenderDto::from)
        .collect())
}

#[cfg(test)]
mod tests {
    use hickeyfield_core::compositor::{Clip, OutputSpec, Timeline, Transition};

    /// AC3: N probed clips build a normalise → crossfade → concat timeline with
    /// N−1 joins; a single clip has no join. This is the pure timeline shape the
    /// render builds; the sidecar run is the `--ignored` ffmpeg test.
    #[test]
    fn render_builds_a_normalize_crossfade_concat_timeline() {
        // Three 4s clips, all crossfade joins (a non-chained set).
        let clips = vec![
            Clip::new("/a.mp4", 0.0, 4.0),
            Clip::new("/b.mp4", 0.0, 4.0).crossfade_in(0.5),
            Clip::new("/c.mp4", 0.0, 4.0).crossfade_in(0.5),
        ];
        let spec = OutputSpec::new(640, 480, 30, "/out.mp4");
        let t = Timeline::new(clips, spec);
        assert!(t.validate().is_empty(), "{:?}", t.validate());
        // 12 − 2×0.5 = 11.
        assert!((t.total_duration() - 11.0).abs() < 1e-9);
        let n_xfade = t.clips.iter().filter(|c| c.transition.is_some()).count();
        assert_eq!(n_xfade, 2, "N−1 crossfade joins");

        // A single clip: no join.
        let one = Timeline::new(
            vec![Clip::new("/a.mp4", 0.0, 4.0)],
            OutputSpec::new(640, 480, 30, "/o.mp4"),
        );
        assert_eq!(one.clips.len(), 1);
        assert!(one.clips[0].transition.is_none());
        assert!((one.total_duration() - 4.0).abs() < 1e-9);
    }

    /// AC3/D-B: a chained 2×15 set with a continuity join is a hard cut (no
    /// crossfade), so the stitched length keeps all 30s; the same set as
    /// crossfades is 29.5s. Proves the join kind is load-bearing on duration.
    #[test]
    fn a_continuity_join_is_a_hard_cut_keeping_every_second() {
        let hard = Timeline::new(
            vec![
                Clip::new("/a.mp4", 0.0, 15.0),
                Clip::new("/b.mp4", 0.0, 15.0),
            ],
            OutputSpec::new(1280, 720, 30, "/o.mp4"),
        );
        assert!(hard.clips[1].transition.is_none(), "continuity = hard cut");
        assert!((hard.total_duration() - 30.0).abs() < 1e-9);

        let faded = Timeline::new(
            vec![
                Clip::new("/a.mp4", 0.0, 15.0),
                Clip::new("/b.mp4", 0.0, 15.0).crossfade_in(0.5),
            ],
            OutputSpec::new(1280, 720, 30, "/o.mp4"),
        );
        assert!(matches!(
            faded.clips[1].transition,
            Some(Transition::Crossfade { .. })
        ));
        assert!((faded.total_duration() - 29.5).abs() < 1e-9);
    }
}
