//! The FFmpeg sidecar: finding it, probing it, licence-checking it, and reading
//! its progress.
//!
//! Hickeyfield ships a **static** FFmpeg per target as a Tauri `externalBin`.
//! Static matters: dynamic builds fail dylib resolution once they are inside an
//! `.app` bundle, and the failure surfaces as a generic spawn error long after
//! the user has installed.
//!
//! Nothing in this module knows what a filtergraph is — [`crate::compositor`]
//! builds the argv, this module runs the binary. Keeping the two apart is what
//! lets the whole graph builder be unit-tested with no process spawning.
//!
//! ## Licensing is a hard constraint, not a footnote
//!
//! Default FFmpeg is LGPL-2.1+. `--enable-gpl` (which we need for libx264)
//! makes it GPL-2.0+, which is fine: Hickeyfield is AGPL-3.0-or-later, FFmpeg's
//! "or later" reaches GPL-3, and AGPL-3 §13 explicitly permits the combination.
//! **`--enable-nonfree` makes the binary legally unredistributable.** A build
//! configured that way cannot ship in our installer at all, so [`licence_check`]
//! exists to make that catchable in CI rather than in a takedown notice.

use serde::{Deserialize, Serialize};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

/// The `externalBin` stem declared in `tauri.conf.json`.
pub const SIDECAR_STEM: &str = "ffmpeg";

/// The Rust target triple this binary was built for.
///
/// Derived from `std::env::consts`, which are compile-time constants, so this
/// is the real build target rather than a runtime guess. Tauri requires
/// sidecars on disk to be suffixed with exactly this string.
pub fn target_triple() -> String {
    let arch = std::env::consts::ARCH;
    match std::env::consts::OS {
        "macos" => format!("{arch}-apple-darwin"),
        "windows" => format!("{arch}-pc-windows-msvc"),
        "linux" => format!("{arch}-unknown-linux-gnu"),
        other => format!("{arch}-unknown-{other}"),
    }
}

/// The on-disk file name Tauri expects in `src-tauri/binaries/`, e.g.
/// `ffmpeg-aarch64-apple-darwin` or `ffmpeg-x86_64-pc-windows-msvc.exe`.
pub fn sidecar_file_name() -> String {
    format!(
        "{SIDECAR_STEM}-{}{}",
        target_triple(),
        std::env::consts::EXE_SUFFIX
    )
}

/// Find the sidecar in `dir`.
///
/// Two names are accepted because Tauri renames the file on the way into the
/// bundle: the triple-suffixed name is what lives in the repo and what the
/// bundler consumes, but inside `Contents/MacOS/` (or next to the `.exe`) it is
/// just `ffmpeg`. Checking only one of the two works in dev and fails in a
/// shipped build, or vice versa — which is a miserable bug to find late.
pub fn locate(dir: &Path) -> Result<PathBuf, FfmpegError> {
    let suffixed = dir.join(sidecar_file_name());
    if suffixed.is_file() {
        return Ok(suffixed);
    }
    let bare = dir.join(format!("{SIDECAR_STEM}{}", std::env::consts::EXE_SUFFIX));
    if bare.is_file() {
        return Ok(bare);
    }
    Err(FfmpegError::NotFound(format!(
        "no {} or {} in {}",
        sidecar_file_name(),
        SIDECAR_STEM,
        dir.display()
    )))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FfmpegError {
    /// The sidecar is missing. Almost always means `scripts/fetch-ffmpeg.sh`
    /// was never run, so the message should say so.
    NotFound(String),
    /// The binary exists but would not start.
    Spawn(String),
    /// It started and told us something we could not use.
    Probe(String),
    /// The build is not one we are allowed to redistribute.
    Licence(String),
    /// A render or last-frame extraction spawned, but did not produce its
    /// output: a non-zero exit, a timeout, or a missing output file. Unlike a
    /// probe (which is *given* no output and exits non-zero by design), these
    /// helpers WRITE a file, so a failed exit is a real failure — never a
    /// silent skip. Carries the bounded stderr tail.
    Render(String),
}

impl std::fmt::Display for FfmpegError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FfmpegError::NotFound(m) => write!(f, "ffmpeg sidecar not found: {m}"),
            FfmpegError::Spawn(m) => write!(f, "could not run ffmpeg: {m}"),
            FfmpegError::Probe(m) => write!(f, "could not probe ffmpeg: {m}"),
            FfmpegError::Licence(m) => write!(f, "ffmpeg licence check failed: {m}"),
            FfmpegError::Render(m) => write!(f, "ffmpeg render failed: {m}"),
        }
    }
}

impl std::error::Error for FfmpegError {}

// ---------------------------------------------------------------------------
// Encoders
// ---------------------------------------------------------------------------

/// An H.264 encoder we know how to drive.
///
/// H.264 only, deliberately: it is the one codec that plays everywhere a user
/// might drop an export, and every platform we ship has a hardware path for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Encoder {
    /// Apple, all Macs we support.
    VideoToolbox,
    /// NVIDIA.
    Nvenc,
    /// Intel Quick Sync.
    Qsv,
    /// AMD Advanced Media Framework.
    Amf,
    /// Software. Always present, always correct, always slowest.
    Libx264,
}

impl Encoder {
    /// The name to pass to `-c:v`.
    pub const fn as_str(self) -> &'static str {
        match self {
            Encoder::VideoToolbox => "h264_videotoolbox",
            Encoder::Nvenc => "h264_nvenc",
            Encoder::Qsv => "h264_qsv",
            Encoder::Amf => "h264_amf",
            Encoder::Libx264 => "libx264",
        }
    }

    pub const fn is_hardware(self) -> bool {
        !matches!(self, Encoder::Libx264)
    }

    /// Encoder-specific quality flags.
    ///
    /// This cannot be one shared set of flags: `-crf` is a libx264 concept, and
    /// passing it to the others is either ignored (silently producing a
    /// default-quality file) or rejected. The mappings below are each the
    /// documented constant-quality knob for that encoder.
    pub fn quality_args(self, q: Quality) -> Vec<String> {
        match (self, q) {
            (Encoder::Libx264, Quality::Crf(n)) => {
                vec![
                    "-crf".into(),
                    n.to_string(),
                    "-preset".into(),
                    "medium".into(),
                ]
            }
            // VideoToolbox has no CRF. On Apple Silicon it takes `-q:v` 1..100,
            // higher being better — the inverse sense of CRF, hence the flip.
            // `-allow_sw 1` keeps the export working on hardware without an
            // H.264 encode block instead of failing at the last step of a long
            // render.
            (Encoder::VideoToolbox, Quality::Crf(n)) => {
                let qv = 100i32.saturating_sub(i32::from(n) * 2).clamp(1, 100);
                vec![
                    "-q:v".into(),
                    qv.to_string(),
                    "-allow_sw".into(),
                    "1".into(),
                ]
            }
            // NVENC ignores `-cq` unless the bitrate target is explicitly zero.
            // Omitting `-b:v 0` is the classic "why is my quality setting doing
            // nothing" bug.
            (Encoder::Nvenc, Quality::Crf(n)) => vec![
                "-rc".into(),
                "vbr".into(),
                "-cq".into(),
                n.to_string(),
                "-b:v".into(),
                "0".into(),
                "-preset".into(),
                "p5".into(),
            ],
            (Encoder::Qsv, Quality::Crf(n)) => {
                vec![
                    "-global_quality".into(),
                    n.to_string(),
                    "-preset".into(),
                    "medium".into(),
                ]
            }
            (Encoder::Amf, Quality::Crf(n)) => vec![
                "-rc".into(),
                "cqp".into(),
                "-qp_i".into(),
                n.to_string(),
                "-qp_p".into(),
                n.to_string(),
            ],
            // Bitrate is the one control every encoder spells the same way.
            // `bufsize` at 2x the target is the usual VBV choice; leaving it
            // unset lets some encoders drift far above the requested rate.
            (_, Quality::BitrateKbps(k)) => vec![
                "-b:v".into(),
                format!("{k}k"),
                "-maxrate".into(),
                format!("{k}k"),
                "-bufsize".into(),
                format!("{}k", u32::from(k).saturating_mul(2)),
            ],
        }
    }
}

/// How to ask for a given output quality.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Quality {
    /// Constant quality, on the libx264 CRF scale (0 lossless, 23 default,
    /// 51 worst). Translated per encoder by [`Encoder::quality_args`].
    Crf(u8),
    /// A fixed target bitrate in kbit/s.
    BitrateKbps(u16),
}

impl Default for Quality {
    fn default() -> Self {
        // 20 is visibly clean for AI-generated footage, which is soft to begin
        // with, without inflating a 10-minute export to gigabytes.
        Quality::Crf(20)
    }
}

/// Which platform's preference chain to use. Split out from `cfg!` so the
/// per-platform behaviour is testable on any host — otherwise the Windows
/// chain would only ever be exercised on a Windows CI runner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostOs {
    MacOs,
    Windows,
    Other,
}

impl HostOs {
    pub fn current() -> Self {
        match std::env::consts::OS {
            "macos" => HostOs::MacOs,
            "windows" => HostOs::Windows,
            _ => HostOs::Other,
        }
    }
}

const MACOS_CHAIN: &[Encoder] = &[Encoder::VideoToolbox, Encoder::Libx264];
const WINDOWS_CHAIN: &[Encoder] = &[Encoder::Nvenc, Encoder::Qsv, Encoder::Amf, Encoder::Libx264];
const FALLBACK_CHAIN: &[Encoder] = &[Encoder::Libx264];

/// Preference order for `os`.
///
/// macOS is not a chain in practice — every Mac we support has VideoToolbox —
/// but libx264 stays behind it so a broken VideoToolbox still produces a file.
/// Windows genuinely needs the full ladder: the GPU vendor is unknown until we
/// ask the binary what it was built with.
pub const fn preference_chain_for(os: HostOs) -> &'static [Encoder] {
    match os {
        HostOs::MacOs => MACOS_CHAIN,
        HostOs::Windows => WINDOWS_CHAIN,
        HostOs::Other => FALLBACK_CHAIN,
    }
}

pub fn preference_chain() -> &'static [Encoder] {
    preference_chain_for(HostOs::current())
}

/// Pick the first encoder in `chain` that `available` contains.
///
/// Falls back to libx264 rather than erroring: a slow export beats no export,
/// and libx264 is present in every GPL build we would ever ship.
pub fn select_from(chain: &[Encoder], available: &[String]) -> Encoder {
    chain
        .iter()
        .copied()
        .find(|e| available.iter().any(|a| a == e.as_str()))
        .unwrap_or(Encoder::Libx264)
}

/// [`select_from`] against the host platform's chain.
pub fn select_encoder(available: &[String]) -> Encoder {
    select_from(preference_chain(), available)
}

/// Extract encoder names from `ffmpeg -encoders`.
///
/// The listing has a legend block before a `------` separator; entries after it
/// are `<6 flag chars> <name> <description>`. Parsing from the separator rather
/// than pattern-matching the flags avoids mistaking legend rows (`V..... =
/// Video`) for encoders named `=`.
pub fn parse_encoders(stdout: &str) -> Vec<String> {
    let body = match stdout.split_once("\n ------") {
        Some((_, rest)) => rest,
        // No separator: either a truncated capture or a future format change.
        // Scanning everything is a safe degradation — a bogus extra name can
        // only ever fail to match a chain entry.
        None => stdout,
    };
    body.lines()
        .filter_map(|line| {
            let mut it = line.split_whitespace();
            let flags = it.next()?;
            let name = it.next()?;
            // Flag columns are exactly six of `VASFXBD.`; anything else is prose.
            if flags.len() == 6 && flags.chars().all(|c| "VASFXBD.".contains(c)) {
                Some(name.to_string())
            } else {
                None
            }
        })
        .collect()
}

/// The probe result, cached process-wide.
///
/// Probing costs a process spawn and the answer cannot change while the app is
/// running — the GPU does not appear halfway through a session — so this is
/// resolved once at startup and read for free thereafter.
static PROBED_ENCODER: OnceLock<Encoder> = OnceLock::new();

/// Ask the sidecar which encoders it has and return the best one we can use.
pub fn probe_encoders(bin: &Path) -> Result<Encoder, FfmpegError> {
    if let Some(cached) = PROBED_ENCODER.get() {
        return Ok(*cached);
    }
    let out = run(bin, &["-hide_banner", "-encoders"])?;
    let chosen = select_encoder(&parse_encoders(&out));
    Ok(*PROBED_ENCODER.get_or_init(|| chosen))
}

/// The already-probed encoder, if [`probe_encoders`] has run. Lets the UI
/// render "Hardware: VideoToolbox" without risking a spawn on the paint path.
pub fn cached_encoder() -> Option<Encoder> {
    PROBED_ENCODER.get().copied()
}

// ---------------------------------------------------------------------------
// Licence
// ---------------------------------------------------------------------------

/// What `ffmpeg -L` says the build is licensed under.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Licence {
    /// `--enable-gpl`, no `--enable-version3`.
    Gpl2OrLater,
    /// `--enable-gpl --enable-version3`.
    Gpl3OrLater,
    /// No `--enable-gpl`. Usable, but has no libx264, so not what we ship.
    Lgpl,
    /// `--enable-nonfree`. Cannot be redistributed at all.
    NonFree,
    Unknown,
}

impl Licence {
    pub const fn is_gpl(self) -> bool {
        matches!(self, Licence::Gpl2OrLater | Licence::Gpl3OrLater)
    }

    /// Whether we are allowed to put this binary in an installer.
    pub const fn is_redistributable(self) -> bool {
        !matches!(self, Licence::NonFree)
    }
}

/// Classify the output of `ffmpeg -L`.
///
/// The strings come from FFmpeg's own `show_license()`, which prints one of
/// four fixed blurbs chosen at configure time. LGPL is checked before GPL
/// because the LGPL blurb contains the substring "General Public License" too —
/// checking in the other order silently mislabels every LGPL build as GPL.
pub fn parse_licence(text: &str) -> Licence {
    let lower = text.to_ascii_lowercase();
    if lower.contains("nonfree") || lower.contains("not legally redistributable") {
        return Licence::NonFree;
    }
    if lower.contains("lesser general public license") {
        return Licence::Lgpl;
    }
    if lower.contains("general public license") {
        if lower.contains("version 3") {
            Licence::Gpl3OrLater
        } else {
            Licence::Gpl2OrLater
        }
    } else {
        Licence::Unknown
    }
}

/// The `configure` line from `ffmpeg -version`, split into flags.
///
/// The GPL obligation is to record the exact configuration of the build we
/// ship, so `NOTICE` is generated from this rather than transcribed by hand.
pub fn parse_configure_flags(version_stdout: &str) -> Vec<String> {
    version_stdout
        .lines()
        .find_map(|l| l.trim().strip_prefix("configuration:"))
        .map(|c| c.split_whitespace().map(str::to_string).collect())
        .unwrap_or_default()
}

/// Assert the binary at `bin` is a GPL build with no non-free parts.
///
/// Run this in CI against the fetched sidecar. A `--enable-nonfree` build looks
/// and behaves identically to a legal one — the only way to notice is to ask,
/// and the consequence of not asking is shipping something we have no right to
/// distribute.
pub fn licence_check(bin: &Path) -> Result<Licence, FfmpegError> {
    let licence = parse_licence(&run(bin, &["-hide_banner", "-L"])?);
    if !licence.is_redistributable() {
        return Err(FfmpegError::Licence(
            "build reports non-free components; --enable-nonfree binaries are not redistributable"
                .into(),
        ));
    }
    if !licence.is_gpl() {
        return Err(FfmpegError::Licence(format!(
            "expected a GPL build (we need libx264), got {licence:?}"
        )));
    }
    // Belt and braces: the licence blurb is chosen at configure time, but so is
    // the configuration string, and disagreement between the two would mean the
    // binary is not what it claims.
    let flags = parse_configure_flags(&run(bin, &["-hide_banner", "-version"])?);
    if flags.iter().any(|f| f == "--enable-nonfree") {
        return Err(FfmpegError::Licence(
            "configuration contains --enable-nonfree".into(),
        ));
    }
    Ok(licence)
}

fn run(bin: &Path, args: &[&str]) -> Result<String, FfmpegError> {
    let out = Command::new(bin)
        .args(args)
        .output()
        .map_err(|e| FfmpegError::Spawn(format!("{}: {e}", bin.display())))?;
    // `-encoders` and `-L` both write to stdout, but FFmpeg has historically
    // moved informational output between the two streams, so read both.
    let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&out.stderr));
    if text.trim().is_empty() {
        return Err(FfmpegError::Probe(format!(
            "{} {:?} produced no output",
            bin.display(),
            args
        )));
    }
    Ok(text)
}

// ---------------------------------------------------------------------------
// Progress
// ---------------------------------------------------------------------------

/// One progress sample from `-progress pipe:1`.
///
/// This is the one place in Hickeyfield where a real percentage is honest. A
/// generation has no knowable total, so its UI shows elapsed time and a phase;
/// a render has a duration we computed ourselves, so [`Progress::percent`] is a
/// measurement rather than a guess.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Progress {
    /// Output position in seconds.
    pub seconds_done: f64,
    /// Frames written so far.
    pub frame: u64,
    /// True on the final block (`progress=end`).
    pub done: bool,
}

impl Progress {
    /// Fraction complete in 0.0..=1.0 against a known total.
    ///
    /// Clamped because FFmpeg's last block can report a position a few
    /// milliseconds past the nominal duration, and a progress bar that reaches
    /// 101% reads as a bug.
    pub fn percent(&self, total_seconds: f64) -> f64 {
        if self.done {
            return 1.0;
        }
        if total_seconds <= 0.0 {
            return 0.0;
        }
        (self.seconds_done / total_seconds).clamp(0.0, 1.0)
    }
}

/// Incremental parser for `-progress pipe:1`.
///
/// FFmpeg emits `key=value` lines and terminates each block with
/// `progress=continue` or `progress=end`. Feed it whatever a read returns:
/// pipe reads split wherever the kernel feels like it, so a chunk routinely
/// ends mid-key. Buffering the tail is the entire reason this is a struct and
/// not a function.
#[derive(Debug, Default)]
pub struct ProgressReader {
    pending: String,
    seconds_done: f64,
    frame: u64,
}

impl ProgressReader {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed a chunk; returns one [`Progress`] per completed block.
    pub fn push(&mut self, chunk: &str) -> Vec<Progress> {
        self.pending.push_str(chunk);
        let mut out = Vec::new();
        // Keep the trailing fragment (everything after the last newline) for
        // the next call.
        let split = match self.pending.rfind('\n') {
            Some(i) => i + 1,
            None => return out,
        };
        let complete: String = self.pending.drain(..split).collect();
        for line in complete.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let (key, value) = (key.trim(), value.trim());
            match key {
                // `out_time_us` is microseconds. So, confusingly, is
                // `out_time_ms` — a long-standing FFmpeg misnomer that has
                // never been fixed for compatibility. Reading `out_time_ms` as
                // milliseconds makes every render look 1000x faster than it is.
                "out_time_us" => {
                    if let Ok(us) = value.parse::<i64>() {
                        self.seconds_done = us as f64 / 1_000_000.0;
                    }
                }
                "out_time" if self.seconds_done == 0.0 => {
                    if let Some(s) = parse_timecode(value) {
                        self.seconds_done = s;
                    }
                }
                "frame" => {
                    if let Ok(f) = value.parse::<u64>() {
                        self.frame = f;
                    }
                }
                "progress" => out.push(Progress {
                    seconds_done: self.seconds_done,
                    frame: self.frame,
                    done: value == "end",
                }),
                _ => {}
            }
        }
        out
    }
}

/// `HH:MM:SS.mmm` -> seconds. FFmpeg writes `N/A` before the first frame.
fn parse_timecode(tc: &str) -> Option<f64> {
    let mut secs = 0.0;
    for part in tc.split(':') {
        secs = secs * 60.0 + part.parse::<f64>().ok()?;
    }
    Some(secs)
}

// ---------------------------------------------------------------------------
// Probing a settled output (the actual-cost measurement)
// ---------------------------------------------------------------------------

/// What a downloaded output actually is, measured from the file rather than the
/// request. The billed quantity is the settled truth: fal renders (and charges)
/// the seconds and resolution it produced, which the provider may have rounded
/// or coerced away from what was asked.
///
/// Every field is [`Option`] and **never a zero on failure** — an unmeasurable
/// output yields `None`, which prices to an unknown cost, never to `$0.00`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MediaDims {
    pub seconds: Option<f64>,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

/// Parse the stderr of `ffmpeg -i <file>`.
///
/// Pure and unit-testable on a captured string. `ffmpeg -i` with no output file
/// prints the input's `Duration:` and the first `Video:` stream's `WxH`, then
/// exits non-zero with `At least one output file must be specified` — which is
/// normal and carries exactly the information we want.
///
/// Reads the `Duration: HH:MM:SS.ss` line (skipping FFmpeg's `N/A` placeholder)
/// and the first `NNNNxNNNN` on the first video stream line. Anything it cannot
/// read stays `None`.
pub fn parse_probe(stderr: &str) -> MediaDims {
    let mut dims = MediaDims {
        seconds: None,
        width: None,
        height: None,
    };
    for line in stderr.lines() {
        let t = line.trim();
        if dims.seconds.is_none() {
            if let Some(idx) = t.find("Duration:") {
                let tc = t[idx + "Duration:".len()..]
                    .trim_start()
                    .split(',')
                    .next()
                    .unwrap_or("")
                    .trim();
                if !tc.eq_ignore_ascii_case("N/A") {
                    dims.seconds = parse_timecode(tc);
                }
            }
        }
        if dims.width.is_none() && t.contains("Video:") {
            if let Some((w, h)) = parse_dimensions(t) {
                dims.width = Some(w);
                dims.height = Some(h);
            }
        }
    }
    dims
}

/// The first `NNNNxNNNN` resolution in a video-stream line.
///
/// Scans for an `x` flanked by digits on both sides. Rejects `0x31637661` (the
/// codec tag, whose left digit is a zero) and `SAR 1:1` / `DAR 16:9` (no `x`),
/// so the `1280x720` frame size is what it returns.
fn parse_dimensions(line: &str) -> Option<(u32, u32)> {
    let b = line.as_bytes();
    for i in 0..b.len() {
        if b[i] != b'x' {
            continue;
        }
        let mut l = i;
        while l > 0 && b[l - 1].is_ascii_digit() {
            l -= 1;
        }
        let mut r = i + 1;
        while r < b.len() && b[r].is_ascii_digit() {
            r += 1;
        }
        if l == i || r == i + 1 {
            continue;
        }
        match (line[l..i].parse::<u32>(), line[i + 1..r].parse::<u32>()) {
            (Ok(w), Ok(h)) if w > 0 && h > 0 => return Some((w, h)),
            _ => continue,
        }
    }
    None
}

/// Probe a downloaded file with the sidecar.
///
/// `ffmpeg -i` exits non-zero because it is given no output file; [`run`] reads
/// its combined output regardless, so that exit code is not an error here. A
/// genuine spawn failure still surfaces as [`FfmpegError::Spawn`]. Fields that
/// cannot be read come back `None`, never `0`.
pub fn probe_file(bin: &Path, file: &Path) -> Result<MediaDims, FfmpegError> {
    let out = run_probe(bin, file)?;
    Ok(parse_probe(&out))
}

/// Wall-clock ceiling for one probe. A hostile or malformed downloaded file that
/// makes the demuxer loop must not wedge a `spawn_blocking` thread forever
/// (tokio's blocking pool is finite, so enough of them stall the whole runner).
/// 20s is far longer than any real header read and short enough to bound the
/// damage.
const PROBE_TIMEOUT: Duration = Duration::from_secs(20);

/// Cap on captured stderr. `ffmpeg -i` writes a fixed header block; anything
/// past 64 KiB is a misbehaving input trying to exhaust memory, and the fields
/// we read (`Duration:`, the first `Video:` line) are near the top regardless.
const MAX_PROBE_CAPTURE: usize = 64 * 1024;

/// The argv for probing untrusted media. Split out so the hardening flags are
/// unit-testable without spawning: `-nostdin` (never block reading stdin),
/// `-protocol_whitelist file` **before** `-i` (refuse `http`/`concat`/etc. an
/// input file might chain to), and no output file (`-i` alone prints the header
/// and exits non-zero, which is expected).
fn probe_args(file: &str) -> [&str; 6] {
    [
        "-nostdin",
        "-hide_banner",
        "-protocol_whitelist",
        "file",
        "-i",
        file,
    ]
}

/// Append `chunk` to `buf` without letting it grow past `cap`. Extracted so the
/// bound is testable without a process.
fn bounded_push(buf: &mut Vec<u8>, chunk: &[u8], cap: usize) {
    if buf.len() >= cap {
        return;
    }
    let room = cap - buf.len();
    buf.extend_from_slice(&chunk[..chunk.len().min(room)]);
}

/// Spawn `ffmpeg -i <file>` on untrusted media with a wall-clock timeout, a kill
/// on overrun, `stdin` closed, the file protocol whitelisted, and bounded
/// capture. Returns the combined stdout+stderr, or a `Probe` error on timeout.
///
/// Both pipes are drained on their own threads so a full pipe buffer cannot
/// deadlock the wait, and the drain itself caps at [`MAX_PROBE_CAPTURE`].
fn run_probe(bin: &Path, file: &Path) -> Result<String, FfmpegError> {
    let file = file.to_string_lossy().into_owned();
    let args = probe_args(&file);
    let mut child = Command::new(bin)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| FfmpegError::Spawn(format!("{}: {e}", bin.display())))?;

    let drain = |mut r: Option<Box<dyn Read + Send>>| {
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            if let Some(r) = r.as_mut() {
                let mut chunk = [0u8; 8192];
                loop {
                    match r.read(&mut chunk) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => bounded_push(&mut buf, &chunk[..n], MAX_PROBE_CAPTURE),
                    }
                }
            }
            buf
        })
    };
    let out_h = drain(
        child
            .stdout
            .take()
            .map(|s| Box::new(s) as Box<dyn Read + Send>),
    );
    let err_h = drain(
        child
            .stderr
            .take()
            .map(|s| Box::new(s) as Box<dyn Read + Send>),
    );

    let deadline = Instant::now() + PROBE_TIMEOUT;
    let timed_out = loop {
        match child.try_wait() {
            Ok(Some(_)) => break false,
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    break true;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => return Err(FfmpegError::Spawn(format!("{}: {e}", bin.display()))),
        }
    };

    let stdout = out_h.join().unwrap_or_default();
    let stderr = err_h.join().unwrap_or_default();
    if timed_out {
        return Err(FfmpegError::Probe(format!(
            "ffmpeg -i timed out after {}s probing {}",
            PROBE_TIMEOUT.as_secs(),
            bin.display()
        )));
    }
    let mut text = String::from_utf8_lossy(&stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&stderr));
    Ok(text)
}

// ---------------------------------------------------------------------------
// Last-frame extraction (storyboard continuity)
// ---------------------------------------------------------------------------

/// The argv for extracting the final frame of `clip` to `out` as a PNG.
///
/// Split out so the sandboxing is unit-testable without a spawn, exactly like
/// [`probe_args`]. `-protocol_whitelist file` and `-sseof` are **input**
/// options and MUST precede `-i` (a whitelist after `-i` protects nothing, and
/// `-sseof` after `-i` is an *output* seek).
///
/// **Getting the *true* last frame on any GOP.** `-sseof -3` seeks the demuxer
/// to ~3s before EOF, which lands on a keyframe at or before that point even on
/// a sparse-GOP stream (a `-sseof -0.05` too close to EOF can seek *past* the
/// last keyframe and emit nothing — proven by the keyframe-sparse ignored test,
/// design §10.3). Then, instead of `-frames:v 1` (which would grab the *first*
/// frame after the seek, ~3s early), `-update 1` **overwrites** the same PNG for
/// every decoded frame from the seek point to EOF, so the file left on disk is
/// the genuine final frame. Decoding ≤3s is cheap.
fn last_frame_args(clip: &str, out: &str, scale: Option<(u32, u32)>) -> Vec<String> {
    let mut a: Vec<String> = [
        "-nostdin",
        "-hide_banner",
        "-loglevel",
        "error",
        "-y",
        "-protocol_whitelist",
        "file",
        "-sseof",
        "-3",
        "-i",
        clip,
        "-update",
        "1",
    ]
    .map(String::from)
    .to_vec();
    // S15 continuity (#4): the next shot's canvas follows this frame, so extract
    // it at that shot's target resolution when it differs from the clip. Even
    // dimensions are the caller's responsibility (yuv420p needs them); the
    // filter forces the exact box.
    if let Some((w, h)) = scale {
        a.push("-vf".into());
        a.push(format!("scale={w}:{h}:flags=lanczos"));
    }
    a.push(out.to_string());
    a
}

/// Extract the final frame of `clip` as a PNG at `out`, via the sidecar.
///
/// Sandboxed exactly like the probe ([`run_probe`]): stdin closed,
/// `-protocol_whitelist file` before `-i`, a wall-clock timeout with kill,
/// bounded stderr. **Unlike the probe this WRITES a file**, so a non-zero exit
/// or a missing output is a real [`FfmpegError::Render`], never a silent skip
/// (AC1). Returns `out` on success.
pub fn last_frame(
    bin: &Path,
    clip: &Path,
    out: &Path,
    size: Option<(u32, u32)>,
) -> Result<PathBuf, FfmpegError> {
    let clip_s = clip.to_string_lossy().into_owned();
    let out_s = out.to_string_lossy().into_owned();
    let args = last_frame_args(&clip_s, &out_s, size);
    // A single-frame decode is quick; reuse the probe's fixed 20s ceiling.
    run_to_file(bin, &args, PROBE_TIMEOUT)?;
    if !out.is_file() {
        return Err(FfmpegError::Render(format!(
            "ffmpeg produced no last frame at {}",
            out.display()
        )));
    }
    Ok(out.to_path_buf())
}

/// Spawn `ffmpeg` with `args` (an argv vector, never a shell string), drain and
/// bound stderr, enforce a wall-clock deadline with a kill on overrun, and
/// require `status.success()`. Shared by [`last_frame`] and [`render`] — the
/// same hardening skeleton as [`run_probe`], but this asserts success because
/// these helpers write an output. Discards stdout unless a reader is wired by
/// the caller (see [`render`], which reads progress instead of calling this).
fn run_to_file(bin: &Path, args: &[String], timeout: Duration) -> Result<(), FfmpegError> {
    let mut child = Command::new(bin)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| FfmpegError::Spawn(format!("{}: {e}", bin.display())))?;

    let err_h = {
        let mut r = child.stderr.take();
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            if let Some(r) = r.as_mut() {
                let mut chunk = [0u8; 8192];
                loop {
                    match r.read(&mut chunk) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => bounded_push(&mut buf, &chunk[..n], MAX_PROBE_CAPTURE),
                    }
                }
            }
            buf
        })
    };

    let deadline = Instant::now() + timeout;
    let timed_out = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Err(status),
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    break Ok(());
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => return Err(FfmpegError::Spawn(format!("{}: {e}", bin.display()))),
        }
    };

    let stderr = err_h.join().unwrap_or_default();
    match timed_out {
        Ok(()) => Err(FfmpegError::Render(format!(
            "ffmpeg timed out after {}s",
            timeout.as_secs()
        ))),
        Err(status) if status.success() => Ok(()),
        Err(status) => Err(FfmpegError::Render(format!(
            "ffmpeg exited {}: {}",
            status
                .code()
                .map(|c| c.to_string())
                .unwrap_or_else(|| "signal".into()),
            String::from_utf8_lossy(&stderr).trim()
        ))),
    }
}

// ---------------------------------------------------------------------------
// Rendering a timeline (streaming progress)
// ---------------------------------------------------------------------------

/// Run a compositor argv to completion, streaming progress.
///
/// Spawns the sidecar with `args` (which already carries `-progress pipe:1` from
/// [`crate::compositor::Timeline::to_args`]), feeds stdout to a
/// [`ProgressReader`], and calls `on_progress` per completed block. `timeout`
/// is the render's own scaled deadline (the shell sizes it to the total
/// duration). stderr is drained and bounded for the error message. A non-zero
/// exit, a timeout, or a spawn failure is a typed error — a render either
/// produces its file or says why, never silently.
pub fn render(
    bin: &Path,
    args: &[String],
    timeout: Duration,
    mut on_progress: impl FnMut(Progress),
) -> Result<(), FfmpegError> {
    let mut child = Command::new(bin)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| FfmpegError::Spawn(format!("{}: {e}", bin.display())))?;

    // stderr on its own drain thread so a full pipe cannot deadlock the wait.
    let err_h = {
        let mut r = child.stderr.take();
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            if let Some(r) = r.as_mut() {
                let mut chunk = [0u8; 8192];
                loop {
                    match r.read(&mut chunk) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => bounded_push(&mut buf, &chunk[..n], MAX_PROBE_CAPTURE),
                    }
                }
            }
            buf
        })
    };

    // stdout (the progress stream) is read on this thread so we can invoke
    // `on_progress` live. A `progress=end` block sets `done`.
    let mut reader = ProgressReader::new();
    let mut stdout = child.stdout.take();
    let deadline = Instant::now() + timeout;
    let mut chunk = [0u8; 8192];
    let mut timed_out = false;
    if let Some(out) = stdout.as_mut() {
        loop {
            // Bound each read by polling the deadline between reads: a wedged
            // encoder that stops emitting progress is killed here.
            if Instant::now() >= deadline {
                let _ = child.kill();
                timed_out = true;
                break;
            }
            match out.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    let text = String::from_utf8_lossy(&chunk[..n]);
                    for p in reader.push(&text) {
                        on_progress(p);
                    }
                }
            }
        }
    }

    let status = child.wait();
    let stderr = err_h.join().unwrap_or_default();
    if timed_out {
        return Err(FfmpegError::Render(format!(
            "render timed out after {}s",
            timeout.as_secs()
        )));
    }
    match status {
        Ok(s) if s.success() => Ok(()),
        Ok(s) => Err(FfmpegError::Render(format!(
            "ffmpeg exited {}: {}",
            s.code()
                .map(|c| c.to_string())
                .unwrap_or_else(|| "signal".into()),
            String::from_utf8_lossy(&stderr).trim()
        ))),
        Err(e) => Err(FfmpegError::Spawn(format!("{}: {e}", bin.display()))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ENCODERS_MACOS: &str = "\
Encoders:
 V..... = Video
 A..... = Audio
 S..... = Subtitle
 .F.... = Frame-level multithreading
 ------
 V....D libx264              libx264 H.264 / AVC (codec h264)
 V....D libx264rgb           libx264 H.264 RGB (codec h264)
 V....D h264_videotoolbox    VideoToolbox H.264 Encoder (codec h264)
 A....D aac                  AAC (Advanced Audio Coding)
";

    const ENCODERS_WINDOWS_NVIDIA: &str = "\
Encoders:
 V..... = Video
 ------
 V....D libx264              libx264 H.264 / AVC (codec h264)
 V....D h264_amf             AMD AMF H.264 Encoder (codec h264)
 V....D h264_nvenc           NVIDIA NVENC H.264 encoder (codec h264)
 V....D h264_qsv             H264 (Intel Quick Sync Video) (codec h264)
";

    #[test]
    fn sidecar_name_carries_the_target_triple() {
        // Tauri resolves `externalBin` by exact triple suffix; a mismatch here
        // is a bundle that builds and then cannot find its own ffmpeg.
        let name = sidecar_file_name();
        assert!(name.starts_with("ffmpeg-"), "{name}");
        assert!(name.contains(&target_triple()), "{name}");
        assert!(name.ends_with(std::env::consts::EXE_SUFFIX), "{name}");
    }

    #[test]
    fn triples_match_the_shapes_tauri_expects() {
        let t = target_triple();
        assert!(
            t.contains("apple-darwin") || t.contains("pc-windows-msvc") || t.contains("linux"),
            "unexpected triple {t}"
        );
    }

    #[test]
    fn encoder_listing_skips_the_legend() {
        let found = parse_encoders(ENCODERS_MACOS);
        assert!(found.contains(&"h264_videotoolbox".to_string()));
        assert!(found.contains(&"libx264".to_string()));
        assert!(found.contains(&"aac".to_string()));
        // `V..... = Video` must not become an encoder called `=`.
        assert!(!found.iter().any(|e| e == "="), "{found:?}");
    }

    #[test]
    fn macos_prefers_videotoolbox() {
        let avail = parse_encoders(ENCODERS_MACOS);
        assert_eq!(
            select_from(preference_chain_for(HostOs::MacOs), &avail),
            Encoder::VideoToolbox
        );
    }

    #[test]
    fn windows_walks_nvenc_qsv_amf_then_software() {
        let all = parse_encoders(ENCODERS_WINDOWS_NVIDIA);
        let chain = preference_chain_for(HostOs::Windows);
        assert_eq!(select_from(chain, &all), Encoder::Nvenc);

        // Drop them one at a time and confirm the ladder, not just the top rung.
        let no_nvenc: Vec<_> = all.iter().filter(|e| *e != "h264_nvenc").cloned().collect();
        assert_eq!(select_from(chain, &no_nvenc), Encoder::Qsv);

        let amf_only: Vec<String> = vec!["h264_amf".into(), "libx264".into()];
        assert_eq!(select_from(chain, &amf_only), Encoder::Amf);

        let software: Vec<String> = vec!["libx264".into()];
        assert_eq!(select_from(chain, &software), Encoder::Libx264);
    }

    #[test]
    fn a_mac_never_picks_a_windows_encoder() {
        // A build that somehow reports nvenc on macOS must still choose
        // VideoToolbox; the chain is the authority, not the listing order.
        let mut avail = parse_encoders(ENCODERS_MACOS);
        avail.insert(0, "h264_nvenc".into());
        assert_eq!(
            select_from(preference_chain_for(HostOs::MacOs), &avail),
            Encoder::VideoToolbox
        );
    }

    #[test]
    fn an_empty_listing_still_yields_a_working_encoder() {
        assert_eq!(
            select_from(preference_chain_for(HostOs::Windows), &[]),
            Encoder::Libx264
        );
    }

    #[test]
    fn quality_flags_are_encoder_specific() {
        let x264 = Encoder::Libx264.quality_args(Quality::Crf(18));
        assert_eq!(x264, vec!["-crf", "18", "-preset", "medium"]);

        // CRF is meaningless to VideoToolbox; it must be translated, not passed.
        let vt = Encoder::VideoToolbox.quality_args(Quality::Crf(18));
        assert!(!vt.contains(&"-crf".to_string()), "{vt:?}");
        assert_eq!(vt, vec!["-q:v", "64", "-allow_sw", "1"]);

        // Without `-b:v 0`, nvenc quietly ignores `-cq`.
        let nv = Encoder::Nvenc.quality_args(Quality::Crf(18));
        let bv = nv.iter().position(|a| a == "-b:v").expect("needs -b:v");
        assert_eq!(nv[bv + 1], "0");
        assert!(nv.contains(&"-cq".to_string()));

        assert_eq!(
            Encoder::Qsv.quality_args(Quality::Crf(18))[0],
            "-global_quality"
        );
        assert!(Encoder::Amf
            .quality_args(Quality::Crf(18))
            .contains(&"cqp".to_string()));
    }

    #[test]
    fn bitrate_sets_a_vbv_buffer() {
        let a = Encoder::Libx264.quality_args(Quality::BitrateKbps(8000));
        assert_eq!(
            a,
            vec!["-b:v", "8000k", "-maxrate", "8000k", "-bufsize", "16000k"]
        );
    }

    #[test]
    fn hardware_is_everything_except_libx264() {
        assert!(Encoder::VideoToolbox.is_hardware());
        assert!(Encoder::Nvenc.is_hardware());
        assert!(!Encoder::Libx264.is_hardware());
    }

    #[test]
    fn nonfree_is_rejected_however_it_is_worded() {
        let msg = "This version of ffmpeg has nonfree parts compiled in.\n\
                   Therefore it is not legally redistributable.\n";
        assert_eq!(parse_licence(msg), Licence::NonFree);
        assert!(!parse_licence(msg).is_redistributable());
    }

    #[test]
    fn gpl_versions_are_distinguished_and_both_accepted() {
        let v2 = "ffmpeg is free software; you can redistribute it and/or modify it under \
                  the terms of the GNU General Public License as published by the Free \
                  Software Foundation; either version 2 of the License, or (at your option) \
                  any later version.";
        let v3 = v2.replace("version 2", "version 3");
        assert_eq!(parse_licence(v2), Licence::Gpl2OrLater);
        assert_eq!(parse_licence(&v3), Licence::Gpl3OrLater);
        // AGPL-3 §13 permits the combination in both directions.
        assert!(parse_licence(v2).is_gpl() && parse_licence(&v3).is_gpl());
    }

    #[test]
    fn lgpl_is_not_mistaken_for_gpl() {
        // The LGPL blurb contains "General Public License" as a substring; an
        // ordering mistake here labels an LGPL build as GPL and we would ship
        // it believing libx264 is present.
        let lgpl = "ffmpeg is free software; you can redistribute it and/or modify it under \
                    the terms of the GNU Lesser General Public License as published by the \
                    Free Software Foundation; either version 2.1 of the License.";
        assert_eq!(parse_licence(lgpl), Licence::Lgpl);
        assert!(!parse_licence(lgpl).is_gpl());
    }

    #[test]
    fn configure_flags_are_recoverable_for_the_notice_file() {
        let version = "ffmpeg version 8.1 Copyright (c) 2000-2026 the FFmpeg developers\n\
                       built with Apple clang version 13.1.6\n  \
                       configuration: --prefix=/x --enable-gpl --enable-libx264 --enable-libfreetype\n\
                       libavutil 60. 26.100\n";
        let flags = parse_configure_flags(version);
        assert!(flags.contains(&"--enable-gpl".to_string()));
        assert!(flags.contains(&"--enable-libfreetype".to_string()));
        assert!(!flags.contains(&"--enable-nonfree".to_string()));
        assert_eq!(flags.len(), 4);
    }

    #[test]
    fn missing_configure_line_yields_no_flags_rather_than_panicking() {
        assert!(parse_configure_flags("ffmpeg version 8.1\n").is_empty());
    }

    #[test]
    fn progress_parses_a_whole_block() {
        let mut r = ProgressReader::new();
        let out = r.push(
            "frame=25\nfps=0.00\nbitrate=  72.8kbits/s\ntotal_size=8373\n\
             out_time_us=920000\nout_time_ms=920000\nout_time=00:00:00.920000\n\
             dup_frames=0\ndrop_frames=0\nspeed=80.1x\nprogress=continue\n",
        );
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].frame, 25);
        assert!((out[0].seconds_done - 0.92).abs() < 1e-9, "{out:?}");
        assert!(!out[0].done);
    }

    #[test]
    fn progress_survives_a_split_line() {
        // The failure this guards: a pipe read ends mid-key, the fragment is
        // parsed as a whole line, and the position jumps backwards on screen.
        let mut r = ProgressReader::new();
        assert!(r.push("frame=10\nout_ti").is_empty(), "no block yet");
        assert!(r.push("me_us=5000").is_empty(), "still no newline");
        let out = r.push("00\nprogress=continue\n");
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].frame, 10);
        assert!((out[0].seconds_done - 0.5).abs() < 1e-9, "{out:?}");
    }

    #[test]
    fn progress_emits_one_sample_per_block() {
        let mut r = ProgressReader::new();
        let out = r.push(
            "frame=1\nout_time_us=40000\nprogress=continue\n\
             frame=2\nout_time_us=80000\nprogress=continue\n\
             frame=3\nout_time_us=120000\nprogress=end\n",
        );
        assert_eq!(out.len(), 3);
        assert_eq!(
            out.iter().map(|p| p.frame).collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
        assert!(out[2].done);
        assert!(!out[0].done && !out[1].done);
    }

    #[test]
    fn progress_ignores_the_not_available_placeholder() {
        // FFmpeg writes `out_time=N/A` before the first frame lands.
        let mut r = ProgressReader::new();
        let out = r.push("frame=0\nout_time_us=N/A\nout_time=N/A\nprogress=continue\n");
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].seconds_done, 0.0);
    }

    #[test]
    fn progress_falls_back_to_the_timecode() {
        let mut r = ProgressReader::new();
        let out = r.push("frame=50\nout_time=00:01:02.500000\nprogress=continue\n");
        assert!((out[0].seconds_done - 62.5).abs() < 1e-6, "{out:?}");
    }

    #[test]
    fn percentage_is_bounded_and_finishes_at_one() {
        let p = Progress {
            seconds_done: 30.0,
            frame: 900,
            done: false,
        };
        assert!((p.percent(60.0) - 0.5).abs() < 1e-9);
        // A final block that overshoots the nominal duration must still read 1.0.
        let over = Progress {
            seconds_done: 60.04,
            frame: 1801,
            done: false,
        };
        assert!((over.percent(60.0) - 1.0).abs() < 1e-9);
        let end = Progress {
            seconds_done: 0.0,
            frame: 0,
            done: true,
        };
        assert_eq!(end.percent(60.0), 1.0);
        // No total known: report nothing rather than dividing by zero.
        assert_eq!(p.percent(0.0), 0.0);
    }

    #[test]
    fn locate_reports_where_it_looked() {
        let err = locate(Path::new("/nonexistent-hickeyfield-dir")).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("ffmpeg-"), "{msg}");
        assert!(msg.contains("/nonexistent-hickeyfield-dir"), "{msg}");
    }

    // Captured verbatim from the bundled sidecar
    // `ffmpeg-aarch64-apple-darwin -hide_banner -i probe.mp4` on a synthesised
    // 2s 1280x720 clip (`testsrc=duration=2:size=1280x720:rate=10`).
    const PROBE_STDERR: &str = "\
Input #0, mov,mp4,m4a,3gp,3g2,mj2, from '/tmp/probe.mp4':
  Metadata:
    major_brand     : isom
    minor_version   : 512
    compatible_brands: isomiso2avc1mp41
    encoder         : Lavf62.12.100
  Duration: 00:00:02.00, start: 0.000000, bitrate: 87 kb/s
  Stream #0:0[0x1](und): Video: h264 (High 4:4:4 Predictive) (avc1 / 0x31637661), yuv444p(progressive), 1280x720 [SAR 1:1 DAR 16:9], 82 kb/s, 10 fps, 10 tbr, 10240 tbn (default)
    Metadata:
      handler_name    : VideoHandler
      encoder         : Lavc62.28.100 libx264
At least one output file must be specified
";

    #[test]
    fn parse_probe_reads_duration_and_resolution() {
        let d = parse_probe(PROBE_STDERR);
        assert_eq!(d.seconds, Some(2.0), "{d:?}");
        assert_eq!(d.width, Some(1280), "{d:?}");
        assert_eq!(d.height, Some(720), "{d:?}");
    }

    #[test]
    fn parse_probe_returns_none_fields_never_zero_on_junk() {
        // A truncated / unreadable probe must yield None, not a confident 0.
        let d = parse_probe("ffmpeg version 7\nsome banner\nno streams here");
        assert_eq!(d.seconds, None);
        assert_eq!(d.width, None);
        assert_eq!(d.height, None);
        // FFmpeg's pre-first-frame N/A duration is not read as a number.
        let na = parse_probe("  Duration: N/A, start: 0.0, bitrate: N/A");
        assert_eq!(na.seconds, None);
    }

    #[test]
    fn parse_dimensions_ignores_the_codec_tag_and_sar() {
        // The codec tag `0x31637661` and `SAR 1:1 DAR 16:9` must not be read as
        // a resolution; the frame size is.
        let (w, h) = parse_dimensions(
            "Video: h264 (avc1 / 0x31637661), yuv444p, 1920x1080 [SAR 1:1 DAR 16:9]",
        )
        .unwrap();
        assert_eq!((w, h), (1920, 1080));
    }

    #[test]
    fn probe_args_sandbox_the_untrusted_input() {
        // Security M1: the probe must close stdin, whitelist only the file
        // protocol *before* -i (so an input cannot chain to http/concat), and
        // pass no output file. Asserted on the argv so it holds without a spawn.
        let a = probe_args("/tmp/x.mp4");
        assert_eq!(a[0], "-nostdin");
        let wl = a.iter().position(|s| *s == "-protocol_whitelist").unwrap();
        assert_eq!(a[wl + 1], "file");
        let i = a.iter().position(|s| *s == "-i").unwrap();
        assert!(wl < i, "protocol whitelist must precede -i: {a:?}");
        assert_eq!(a[i + 1], "/tmp/x.mp4");
        assert!(!a.contains(&"-y"), "a probe writes no output file");
    }

    #[test]
    fn probe_capture_is_bounded() {
        // Security M1: a misbehaving input cannot make us buffer unbounded
        // stderr. `bounded_push` stops at the cap and never over-reads a chunk.
        let cap = 100;
        let mut buf = Vec::new();
        for _ in 0..10 {
            bounded_push(&mut buf, &[b'x'; 30], cap);
        }
        assert_eq!(buf.len(), cap, "capture must stop at the cap");
        // A chunk that would straddle the cap is truncated, not dropped whole.
        let mut b2 = vec![0u8; cap - 5];
        bounded_push(&mut b2, &[1u8; 40], cap);
        assert_eq!(b2.len(), cap);
    }

    // NOTE: the timeout+kill path (`run_probe` deadline) is not fixture-tested.
    // Making the real sidecar hang deterministically without a network or a
    // crafted malicious file is not feasible in a keyless unit test, and a
    // sleep-based fake binary would be a platform-specific fixture. The pieces
    // that *are* deterministic — the sandboxing argv and the bounded capture —
    // are unit-tested above; the timeout wiring is exercised end-to-end by the
    // ignored real-sidecar probe below (which completes well under the ceiling).

    #[test]
    #[ignore = "spawns the bundled sidecar; run with --ignored"]
    fn probe_file_reads_a_real_sidecar_output() {
        // Synthesise a clip with the sidecar, then probe it with the same
        // binary — the end-to-end check that `parse_probe` tolerates the real
        // output. Locates the sidecar next to the repo, falling back to PATH.
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../src-tauri/binaries");
        let bin = locate(&dir).expect("bundled sidecar; run scripts/fetch-ffmpeg.sh");
        let tmp = std::env::temp_dir().join("hickeyfield-probe-test.mp4");
        let status = Command::new(&bin)
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                "testsrc=duration=2:size=1280x720:rate=10",
            ])
            .arg(&tmp)
            .status()
            .expect("synthesise clip");
        assert!(status.success());
        let d = probe_file(&bin, &tmp).unwrap();
        assert_eq!(d.seconds, Some(2.0), "{d:?}");
        assert_eq!(d.width, Some(1280), "{d:?}");
        assert_eq!(d.height, Some(720), "{d:?}");
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn last_frame_args_sandbox_the_input_and_seek_from_eof() {
        // AC1: the last-frame extract must close stdin, whitelist only the file
        // protocol *before* -i, seek from EOF *before* -i, write exactly one
        // image, and end on the output path. Asserted on the argv, no spawn.
        let a = last_frame_args("/tmp/clip.mp4", "/tmp/out.png", None);
        assert_eq!(a[0], "-nostdin");
        let wl = a.iter().position(|s| s == "-protocol_whitelist").unwrap();
        assert_eq!(a[wl + 1], "file");
        let sseof = a.iter().position(|s| s == "-sseof").unwrap();
        assert_eq!(a[sseof + 1], "-3", "seek a safe window before EOF");
        let i = a.iter().position(|s| s == "-i").unwrap();
        assert!(wl < i, "protocol whitelist must precede -i: {a:?}");
        assert!(
            sseof < i,
            "-sseof is an input seek and must precede -i: {a:?}"
        );
        assert_eq!(a[i + 1], "/tmp/clip.mp4");
        // `-update 1` overwrites for every decoded frame → the true last frame;
        // no `-frames:v 1` (that would grab the first frame after the seek).
        assert!(a.iter().any(|s| s == "-update"), "overwrite write: {a:?}");
        assert!(
            !a.iter().any(|s| s == "-frames:v"),
            "no first-frame cap: {a:?}"
        );
        assert_eq!(a.last().unwrap(), "/tmp/out.png", "output path is last");
        assert!(
            !a.iter().any(|s| s.starts_with("scale=")),
            "no scale filter when size is None: {a:?}"
        );
    }

    #[test]
    fn last_frame_args_scale_to_the_sets_canvas_when_requested() {
        // S15 D-C/#4: the next shot's canvas follows this frame, so when the
        // caller passes the set's size the extract scales to it with lanczos,
        // before the output path.
        let a = last_frame_args("/tmp/clip.mp4", "/tmp/out.png", Some((1280, 720)));
        let vf = a.iter().position(|s| s == "-vf").expect("scale filter");
        assert_eq!(a[vf + 1], "scale=1280:720:flags=lanczos");
        let i = a.iter().position(|s| s == "-i").unwrap();
        assert!(vf > i, "the scale is an output filter, after -i: {a:?}");
        assert_eq!(a.last().unwrap(), "/tmp/out.png", "output path stays last");
    }

    #[test]
    fn last_frame_failure_is_a_typed_render_error() {
        // AC1: a missing input must be a typed `Render` error, never an `Ok`
        // with no file. `run_to_file` sees a non-zero exit and reports it; even
        // if a build somehow exits 0 without writing, `last_frame`'s
        // `out.is_file()` check turns that into `Render` too. Uses whatever
        // `ffmpeg` is on PATH; if none, the spawn error is also acceptable —
        // the point is it is never a silent success.
        let bin = std::path::Path::new("ffmpeg");
        let out = std::env::temp_dir().join("hickeyfield-nope-lastframe.png");
        let _ = std::fs::remove_file(&out);
        let r = last_frame(
            bin,
            std::path::Path::new("/nonexistent/definitely-missing.mp4"),
            &out,
            None,
        );
        assert!(r.is_err(), "a missing input must never succeed silently");
        assert!(
            matches!(r, Err(FfmpegError::Render(_)) | Err(FfmpegError::Spawn(_))),
            "{r:?}"
        );
        assert!(!out.is_file(), "no output file must be left behind");
    }

    #[test]
    #[ignore = "spawns the bundled sidecar; run with --ignored"]
    fn last_frame_writes_a_png_with_the_clips_dimensions() {
        // AC1 §10.3: on the real sidecar, the last frame of a keyframe-sparse
        // synthesised clip (testsrc with a 250-frame GOP) writes a PNG whose
        // dimensions match the clip. Proves the last-frame extract does not seek past
        // the last keyframe and emit nothing.
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../src-tauri/binaries");
        let bin = locate(&dir).expect("bundled sidecar; run scripts/fetch-ffmpeg.sh");
        let clip = std::env::temp_dir().join("hickeyfield-lastframe-src.mp4");
        let status = Command::new(&bin)
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                "testsrc=duration=2:size=1280x720:rate=10",
                // Sparse keyframes: one every 250 frames, so the whole 20-frame
                // clip has a single keyframe at the head — the hostile case for
                // an EOF seek.
                "-g",
                "250",
            ])
            .arg(&clip)
            .status()
            .expect("synthesise clip");
        assert!(status.success());
        let out = std::env::temp_dir().join("hickeyfield-lastframe-out.png");
        let _ = std::fs::remove_file(&out);
        let got = last_frame(&bin, &clip, &out, None).expect("extract last frame");
        assert!(got.is_file(), "the PNG must exist");
        let d = probe_file(&bin, &out).unwrap();
        assert_eq!(d.width, Some(1280), "{d:?}");
        assert_eq!(d.height, Some(720), "{d:?}");
        let _ = std::fs::remove_file(&clip);
        let _ = std::fs::remove_file(&out);
    }

    #[test]
    #[ignore = "spawns the bundled sidecar; run with --ignored"]
    fn render_stitches_to_the_expected_duration() {
        // AC3/AC4 §10.4: three 4s testsrc clips, crossfaded 1s each, must render
        // to 4+4+4 - 2*1 = 10s within ±0.2s on the real sidecar; and a single
        // clip renders to its own duration (no stitch). The pure
        // `total_duration` is proven by the compositor's own tests; this is the
        // only thing that catches the *sidecar* drifting from the maths.
        use crate::compositor::{Clip, OutputSpec, Timeline};
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../src-tauri/binaries");
        let bin = locate(&dir).expect("bundled sidecar; run scripts/fetch-ffmpeg.sh");
        let tmp = std::env::temp_dir();
        let mut clips_on_disk = Vec::new();
        for i in 0..3 {
            let p = tmp.join(format!("hickeyfield-render-src-{i}.mp4"));
            let status = Command::new(&bin)
                .args([
                    "-y",
                    "-f",
                    "lavfi",
                    "-i",
                    "testsrc=duration=4:size=640x480:rate=25",
                    "-g",
                    "250",
                ])
                .arg(&p)
                .status()
                .expect("synthesise clip");
            assert!(status.success());
            clips_on_disk.push(p);
        }

        // Three-clip crossfade → 10s.
        let out = tmp.join("hickeyfield-render-out.mp4");
        let _ = std::fs::remove_file(&out);
        let clips: Vec<Clip> = clips_on_disk
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let c = Clip::new(p.clone(), 0.0, 4.0);
                if i == 0 {
                    c
                } else {
                    c.crossfade_in(1.0)
                }
            })
            .collect();
        let mut spec = OutputSpec::new(640, 480, 25, out.clone());
        spec.encoder = probe_encoders(&bin).unwrap();
        let t = Timeline::new(clips, spec);
        assert!(
            (t.total_duration() - 10.0).abs() < 1e-9,
            "pure maths: {}",
            t.total_duration()
        );
        render(&bin, &t.to_args(), Duration::from_secs(120), |_| {}).expect("render");
        let d = probe_file(&bin, &out).unwrap();
        let secs = d.seconds.expect("probed duration");
        assert!(
            (secs - 10.0).abs() <= 0.2,
            "stitched duration {secs}s, want 10±0.2"
        );

        // Single clip → its own duration, no crossfade/concat.
        let out1 = tmp.join("hickeyfield-render-out-single.mp4");
        let _ = std::fs::remove_file(&out1);
        let mut spec1 = OutputSpec::new(640, 480, 25, out1.clone());
        spec1.encoder = probe_encoders(&bin).unwrap();
        let t1 = Timeline::new(vec![Clip::new(clips_on_disk[0].clone(), 0.0, 4.0)], spec1);
        render(&bin, &t1.to_args(), Duration::from_secs(120), |_| {}).expect("render single");
        let d1 = probe_file(&bin, &out1).unwrap();
        let secs1 = d1.seconds.expect("probed duration");
        assert!(
            (secs1 - 4.0).abs() <= 0.2,
            "single-clip duration {secs1}s, want 4±0.2"
        );

        for p in &clips_on_disk {
            let _ = std::fs::remove_file(p);
        }
        let _ = std::fs::remove_file(&out);
        let _ = std::fs::remove_file(&out1);
    }
}
