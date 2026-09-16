//! SQLite-backed job persistence.
//!
//! Implements `hickeyfield_core::engine::JobStore`. The core defines the trait and
//! stays database-free so its tests run in milliseconds; the concrete storage
//! lives here, where it is allowed to be slow and platform-aware.
//!
//! Everything is written on submit. That is what lets a generation survive the
//! window closing and reappear on relaunch — the property that separates this
//! from the web build, which could only have offered it by putting the user's
//! key on a server.

use hickeyfield_core::engine::{JobError, JobSet, JobStore};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Mutex;

/// One join's recorded continuity in a chained set (S16 D7), serialised into
/// [`Storyboard::joins_json`]. `index` is the join *into* shot `index` (there is
/// no join 0). A scoring failure records the numbers as `None`, never `0.0` — a
/// zero would read as a failed join and fire a needless paid regenerate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JoinRecord {
    pub index: usize,
    /// `"continuity"` (a start frame was applied — a hard cut) or `"crossfade"`
    /// (no start frame — a blended join). The render reads this per join (D8).
    pub kind: String,
    pub applied: bool,
    pub ssim: Option<f64>,
    pub psnr: Option<f64>,
    pub dy: Option<f64>,
    pub du: Option<f64>,
    pub dv: Option<f64>,
    /// `Some(true/false)` once scored against the floor; `None` when unscored.
    pub pass: Option<bool>,
    /// How many regenerations this join has had. Bounds the auto-retry loop.
    pub attempt: u32,
    // ── S17: end-frame adherence (keyframe mode, D4) ─────────────────────────
    // The SSIM (+ colour drift) between this shot's ACTUAL last frame and its
    // planned still `K_{index+1}`. All `Option` and `#[serde(default)]` so a
    // pre-S17 row (or an unmeasured end) deserialises with `None` — purely
    // additive, no migration. A `None` end reads as "not failing", never 0.
    #[serde(default)]
    pub end_ssim: Option<f64>,
    #[serde(default)]
    pub end_psnr: Option<f64>,
    #[serde(default)]
    pub end_dy: Option<f64>,
    #[serde(default)]
    pub end_du: Option<f64>,
    #[serde(default)]
    pub end_dv: Option<f64>,
    /// `Some(true/false)` once the end adherence is scored against the IDEAL
    /// tier (0.90/6/3 — keyframe mode is the 99% path); `None` when unscored.
    #[serde(default)]
    pub end_pass: Option<bool>,
}

/// Parse the join records from a set's `joins_json`, or an empty list.
pub fn parse_joins(joins_json: Option<&str>) -> Vec<JoinRecord> {
    joins_json
        .and_then(|s| serde_json::from_str::<Vec<JoinRecord>>(s).ok())
        .unwrap_or_default()
}

/// Insert or replace the record for its `index`, keeping the list ordered.
/// The newest score for a join wins (latest by write — Q5).
pub fn upsert_join(joins: &mut Vec<JoinRecord>, rec: JoinRecord) {
    match joins.iter_mut().find(|j| j.index == rec.index) {
        Some(existing) => *existing = rec,
        None => joins.push(rec),
    }
    joins.sort_by_key(|j| j.index);
}

/// One keyframe still boundary of a keyframe-anchored set (S17, D2), serialised
/// into [`Storyboard::keyframes_json`]. `index` is the boundary `k` in `0..=N`.
/// This is the crash-safe record of "which still job renders this boundary, and
/// where its PNG landed" — the shot payloads bind a still by a concrete path,
/// which does not exist until the still job settles and downloads.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KeyframeRecord {
    pub index: usize,
    /// The still image job's id, once submitted (`None` before submit).
    pub job_set_id: Option<String>,
    /// The downloaded still PNG, once the still job settles (`None` while it is
    /// still queued/running, or if it failed → the set is parked).
    pub local_path: Option<String>,
    /// Priced at submit through `submit_one` (may be `None` → unpriced, e.g. a
    /// GPT still tier; never summed as $0).
    pub estimated_usd: Option<f64>,
    /// How many times this still has been regenerated. Bounds the STILL
    /// auto-retry loop, separate from a shot join's `JoinRecord.attempt` (D4).
    #[serde(default)]
    pub attempt: u32,
}

/// Parse the keyframe records from a set's `keyframes_json`, or an empty list.
pub fn parse_keyframes(keyframes_json: Option<&str>) -> Vec<KeyframeRecord> {
    keyframes_json
        .and_then(|s| serde_json::from_str::<Vec<KeyframeRecord>>(s).ok())
        .unwrap_or_default()
}

/// Insert or replace the record for its `index`, keeping the list ordered — a
/// still regenerate or a settle rewrites its boundary record.
pub fn upsert_keyframe(keyframes: &mut Vec<KeyframeRecord>, rec: KeyframeRecord) {
    match keyframes.iter_mut().find(|k| k.index == rec.index) {
        Some(existing) => *existing = rec,
        None => keyframes.push(rec),
    }
    keyframes.sort_by_key(|k| k.index);
}

pub struct SqliteStore {
    conn: Mutex<Connection>,
}

/// One storyboard grouping row. `plan_json` is a serialised
/// `hickeyfield_core::StoryboardPlan`, kept verbatim so S14 can read the shot
/// list (roles + recorded continuity) back without a lossy re-derivation.
#[derive(Debug, Clone, PartialEq)]
pub struct Storyboard {
    pub id: String,
    pub brief: String,
    pub target_seconds: f64,
    pub plan_json: String,
    pub created_at: i64,
    /// S15: whether this set is chained (each shot continues from the previous
    /// shot's last frame). `false` is S13's all-at-once behaviour.
    pub chained: bool,
    /// S15: the ordered `SubmitInput` payloads for the WHOLE plan (serialised
    /// `Vec`), so a chained set can advance itself after a relaunch without
    /// re-deriving the user's per-shot edits from `plan_json`. `None` for an
    /// all-at-once set.
    pub shots_json: Option<String>,
    /// S15: the crossfade overlap the plan was reconciled to (D4). `None`
    /// defaults to `DEFAULT_CROSSFADE_SECONDS` at render time.
    pub overlap_seconds: Option<f64>,
    /// S15 (reserved for the continuity-score slice, memo §0): a serialised
    /// `[{index, kind, applied, ssim, …, pass, attempt}]` recording each join's
    /// measured continuity. Written by the deferred per-join SSIM scorer; stored
    /// and round-tripped now so that slice is purely additive. `None` today.
    pub joins_json: Option<String>,
    /// S16 (D9): the optional "auto-retry low joins" toggle. `false` (off) for
    /// every set generated before this slice — the safe default that spends
    /// nothing without an explicit opt-in.
    pub auto_retry: bool,
    /// S17 (D2): a serialised `Vec<KeyframeRecord>` for a keyframe-anchored set
    /// (one still per shot boundary, `N+1`). `None` for a non-keyframe set — the
    /// honest value every pre-S17 set reads. Migration 11 adds the column.
    pub keyframes_json: Option<String>,
}

/// S15: one finished, stitched ad for a storyboard set. Re-rendering overwrites
/// (one ad per set). `actual_usd` is the sum of the member shots' `actual_usd`
/// at render time — `None` if any member is unpriced, never summed as $0.
#[derive(Debug, Clone, PartialEq)]
pub struct StoryboardRender {
    pub storyboard_id: String,
    pub output_path: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub seconds: Option<f64>,
    pub actual_usd: Option<f64>,
    pub created_at: i64,
}

fn map_err(e: impl std::fmt::Display) -> JobError {
    // Storage failures are not the provider's fault and retrying the *network*
    // will not help, so they are permanent from the engine's point of view.
    JobError::Permanent(format!("store: {e}"))
}

impl SqliteStore {
    pub fn open(path: &Path) -> Result<Self, JobError> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(map_err)?;
        }
        let conn = Connection::open(path).map_err(map_err)?;
        Self::from_conn(conn)
    }

    #[cfg(test)]
    pub fn in_memory() -> Result<Self, JobError> {
        Self::from_conn(Connection::open_in_memory().map_err(map_err)?)
    }

    fn from_conn(conn: Connection) -> Result<Self, JobError> {
        // WAL so a long-running poll write never blocks a UI read.
        conn.pragma_update(None, "journal_mode", "WAL")
            .map_err(map_err)?;
        conn.pragma_update(None, "foreign_keys", "ON")
            .map_err(map_err)?;
        let store = SqliteStore {
            conn: Mutex::new(conn),
        };
        store.migrate()?;
        Ok(store)
    }

    /// Schema version lives in `user_version`. Migrations are append-only:
    /// never renumber, never edit a shipped step.
    fn migrate(&self) -> Result<(), JobError> {
        let conn = self.conn.lock().unwrap();
        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .map_err(map_err)?;

        if version < 1 {
            conn.execute_batch(
                r#"
                CREATE TABLE IF NOT EXISTS job_sets (
                    id              TEXT PRIMARY KEY,
                    model_id        TEXT NOT NULL,
                    route_id        TEXT NOT NULL,
                    request_id      TEXT NOT NULL,
                    status          TEXT NOT NULL,
                    prompt          TEXT NOT NULL,
                    enhanced_prompt TEXT,
                    preset_id       TEXT,
                    created_at      INTEGER NOT NULL,
                    updated_at      INTEGER NOT NULL,
                    results         TEXT NOT NULL DEFAULT '[]',
                    estimated_usd   REAL,
                    actual_usd      REAL,
                    fail_reason     TEXT,
                    settings        TEXT NOT NULL DEFAULT '{}'
                );
                -- The library lists newest-first, and reattach-on-launch scans
                -- by status; both would table-scan without these.
                CREATE INDEX IF NOT EXISTS idx_jobs_created ON job_sets(created_at DESC);
                CREATE INDEX IF NOT EXISTS idx_jobs_status  ON job_sets(status);
                PRAGMA user_version = 1;
                "#,
            )
            .map_err(map_err)?;
        }

        if version < 2 {
            // Attached inputs. Without these Rerun restored an image-to-video
            // job as text-to-video: a different generation, at the same price,
            // that the user did not ask for.
            conn.execute_batch(
                r#"
                ALTER TABLE job_sets ADD COLUMN media TEXT NOT NULL DEFAULT '[]';
                PRAGMA user_version = 2;
                "#,
            )
            .map_err(map_err)?;
        }

        if version < 3 {
            // The endpoint actually submitted to. Rebuilding it from route_id
            // gave the family root, and fal answers 405 there — the job ran and
            // could not be read back.
            conn.execute_batch(
                r#"
                ALTER TABLE job_sets ADD COLUMN endpoint TEXT NOT NULL DEFAULT '';
                PRAGMA user_version = 3;
                "#,
            )
            .map_err(map_err)?;
        }

        if version < 4 {
            // What rewrote the prompt, and why it did not. Without these a
            // generation cannot be reproduced — the same prompt through a
            // different corpus is a different generation.
            conn.execute_batch(
                r#"
                ALTER TABLE job_sets ADD COLUMN enhancer_version TEXT;
                ALTER TABLE job_sets ADD COLUMN enhance_note TEXT;
                PRAGMA user_version = 4;
                "#,
            )
            .map_err(map_err)?;
        }

        if version < 5 {
            // Advisories: things the user should know that did not fail the
            // job. A setting the endpoint silently discards is the case this
            // exists for.
            conn.execute_batch(
                r#"
                ALTER TABLE job_sets ADD COLUMN advisories TEXT NOT NULL DEFAULT '[]';
                PRAGMA user_version = 5;
                "#,
            )
            .map_err(map_err)?;
        }

        if version < 6 {
            // The timeout-state bound. `poll_cycles` counts timed-out sessions
            // and `stalled` records that the runner gave up auto-resuming after
            // the cap. Old rows default to 0/false — never-yet-stalled, which
            // is correct — so no status enum migration is needed (status stays
            // a TEXT column parsed by serde).
            conn.execute_batch(
                r#"
                ALTER TABLE job_sets ADD COLUMN poll_cycles INTEGER NOT NULL DEFAULT 0;
                ALTER TABLE job_sets ADD COLUMN stalled INTEGER NOT NULL DEFAULT 0;
                PRAGMA user_version = 6;
                "#,
            )
            .map_err(map_err)?;
        }

        if version < 7 {
            // Price provenance. `priced_from`/`priced_at` are write-once (set at
            // submit beside `estimated_usd`, absent from the upsert's UPDATE
            // set); `actual_source` updates alongside `actual_usd` when the
            // downloaded output is measured. All nullable — old rows have no
            // provenance, which is the honest value, not a guessed one.
            conn.execute_batch(
                r#"
                ALTER TABLE job_sets ADD COLUMN priced_from   TEXT;
                ALTER TABLE job_sets ADD COLUMN priced_at     INTEGER;
                ALTER TABLE job_sets ADD COLUMN actual_source TEXT;
                PRAGMA user_version = 7;
                "#,
            )
            .map_err(map_err)?;
        }

        if version < 8 {
            // Storyboards. A `storyboards` grouping row holds the plan JSON; two
            // columns on the existing jobs table (`job_sets`) tag each generated
            // shot to its set and record its order. The grouping table is
            // deliberately NOT called `job_sets` — that name is already the jobs
            // table (see the `version < 1` block). `storyboard_id`/`shot_index`
            // are NULL for every standalone job, and write-once (absent from the
            // upsert's UPDATE set, like `estimated_usd`): a job's set membership
            // is fixed at submit and never re-tagged.
            conn.execute_batch(
                r#"
                CREATE TABLE IF NOT EXISTS storyboards (
                    id             TEXT PRIMARY KEY,
                    brief          TEXT NOT NULL,
                    target_seconds REAL NOT NULL,
                    plan_json      TEXT NOT NULL,
                    created_at     INTEGER NOT NULL
                );
                ALTER TABLE job_sets ADD COLUMN storyboard_id TEXT;
                ALTER TABLE job_sets ADD COLUMN shot_index    INTEGER;
                PRAGMA user_version = 8;
                "#,
            )
            .map_err(map_err)?;
        }

        if version < 9 {
            // S15 storyboard render + chaining. Three columns on the existing
            // `storyboards` group row let a chained set advance itself after a
            // relaunch (`chained` + the full ordered `shots_json` payloads) and
            // record the crossfade the plan was reconciled to. A new
            // `storyboard_renders` table holds the one finished ad per set. All
            // additive; old storyboards read `chained=0` (all-at-once), which is
            // the honest S13 value.
            conn.execute_batch(
                r#"
                ALTER TABLE storyboards ADD COLUMN chained         INTEGER NOT NULL DEFAULT 0;
                ALTER TABLE storyboards ADD COLUMN shots_json      TEXT;
                ALTER TABLE storyboards ADD COLUMN overlap_seconds REAL;
                ALTER TABLE storyboards ADD COLUMN joins_json      TEXT;
                CREATE TABLE IF NOT EXISTS storyboard_renders (
                    storyboard_id TEXT PRIMARY KEY,
                    output_path   TEXT NOT NULL,
                    width         INTEGER,
                    height        INTEGER,
                    seconds       REAL,
                    actual_usd    REAL,
                    created_at    INTEGER NOT NULL
                );
                PRAGMA user_version = 9;
                "#,
            )
            .map_err(map_err)?;
        }

        if version < 10 {
            // S16: an optional per-set "auto-retry low joins" toggle (D9). One
            // additive nullable column, default 0 (off) — old sets read as off,
            // the honest value. NOT a `UNIQUE(storyboard_id, shot_index)` index
            // (design Q7): a retry/regenerate mints a new job id at an existing
            // index, which such an index would make illegal.
            conn.execute_batch(
                r#"
                ALTER TABLE storyboards ADD COLUMN auto_retry INTEGER NOT NULL DEFAULT 0;
                PRAGMA user_version = 10;
                "#,
            )
            .map_err(map_err)?;
        }

        if version < 11 {
            // S17: keyframe-anchored sets. One additive nullable column holding
            // the serialised `Vec<KeyframeRecord>` (the N+1 still boundaries).
            // NULL for every pre-S17 (non-keyframe) set — the honest value. The
            // end-frame-adherence numbers ride on `joins_json` (additive Option
            // fields on `JoinRecord`), so no separate migration is needed there.
            conn.execute_batch(
                r#"
                ALTER TABLE storyboards ADD COLUMN keyframes_json TEXT;
                PRAGMA user_version = 11;
                "#,
            )
            .map_err(map_err)?;
        }
        Ok(())
    }

    pub fn delete(&self, id: &str) -> Result<(), JobError> {
        self.conn
            .lock()
            .unwrap()
            .execute("DELETE FROM job_sets WHERE id = ?1", params![id])
            .map(|_| ())
            .map_err(map_err)
    }

    /// Persist a storyboard grouping row. Called once, before any of its shots
    /// are submitted, so the group survives even if a shot fails.
    pub fn upsert_storyboard(&self, s: &Storyboard) -> Result<(), JobError> {
        self.conn
            .lock()
            .unwrap()
            .execute(
                r#"
                INSERT INTO storyboards
                    (id, brief, target_seconds, plan_json, created_at,
                     chained, shots_json, overlap_seconds, joins_json, auto_retry,
                     keyframes_json)
                VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)
                ON CONFLICT(id) DO UPDATE SET
                    brief           = excluded.brief,
                    target_seconds  = excluded.target_seconds,
                    plan_json       = excluded.plan_json,
                    chained         = excluded.chained,
                    shots_json      = excluded.shots_json,
                    overlap_seconds = excluded.overlap_seconds,
                    joins_json      = excluded.joins_json,
                    auto_retry      = excluded.auto_retry,
                    keyframes_json  = excluded.keyframes_json
                "#,
                params![
                    s.id,
                    s.brief,
                    s.target_seconds,
                    s.plan_json,
                    s.created_at,
                    s.chained as i64,
                    s.shots_json,
                    s.overlap_seconds,
                    s.joins_json,
                    s.auto_retry as i64,
                    s.keyframes_json,
                ],
            )
            .map(|_| ())
            .map_err(map_err)
    }

    /// Persist the finished render for a set. One row per storyboard (re-render
    /// overwrites).
    pub fn upsert_render(&self, r: &StoryboardRender) -> Result<(), JobError> {
        self.conn
            .lock()
            .unwrap()
            .execute(
                r#"
                INSERT INTO storyboard_renders
                    (storyboard_id, output_path, width, height, seconds, actual_usd, created_at)
                VALUES (?1,?2,?3,?4,?5,?6,?7)
                ON CONFLICT(storyboard_id) DO UPDATE SET
                    output_path = excluded.output_path,
                    width       = excluded.width,
                    height      = excluded.height,
                    seconds     = excluded.seconds,
                    actual_usd  = excluded.actual_usd,
                    created_at  = excluded.created_at
                "#,
                params![
                    r.storyboard_id,
                    r.output_path,
                    r.width.map(|w| w as i64),
                    r.height.map(|h| h as i64),
                    r.seconds,
                    r.actual_usd,
                    r.created_at,
                ],
            )
            .map(|_| ())
            .map_err(map_err)
    }

    /// The finished render for one set, or `None`. The Library lists renders
    /// with `renders()`; this singular reader is exercised by the store test.
    #[allow(dead_code)]
    pub fn render(&self, storyboard_id: &str) -> Result<Option<StoryboardRender>, JobError> {
        self.conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT storyboard_id, output_path, width, height, seconds, actual_usd, created_at \
                 FROM storyboard_renders WHERE storyboard_id = ?1",
                params![storyboard_id],
                row_to_render,
            )
            .optional()
            .map_err(map_err)
    }

    /// Every finished render, for the Library's set items.
    pub fn renders(&self) -> Result<Vec<StoryboardRender>, JobError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT storyboard_id, output_path, width, height, seconds, actual_usd, created_at \
                 FROM storyboard_renders ORDER BY created_at DESC",
            )
            .map_err(map_err)?;
        let rows = stmt.query_map([], row_to_render).map_err(map_err)?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(map_err)
    }

    /// One storyboard by id, or `None`. Part of the store surface S14 (render)
    /// and the migration-8 test read; the Library groups client-side and the
    /// header list uses `storyboards()`, so nothing in S13 production calls this
    /// singular reader yet.
    #[allow(dead_code)]
    pub fn storyboard(&self, id: &str) -> Result<Option<Storyboard>, JobError> {
        self.conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT id, brief, target_seconds, plan_json, created_at, \
                        chained, shots_json, overlap_seconds, joins_json, auto_retry, keyframes_json \
                 FROM storyboards WHERE id = ?1",
                params![id],
                row_to_storyboard,
            )
            .optional()
            .map_err(map_err)
    }

    /// Every storyboard, newest first — for the Library group headers.
    pub fn storyboards(&self) -> Result<Vec<Storyboard>, JobError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT id, brief, target_seconds, plan_json, created_at, \
                        chained, shots_json, overlap_seconds, joins_json, auto_retry, keyframes_json \
                 FROM storyboards ORDER BY created_at DESC",
            )
            .map_err(map_err)?;
        let rows = stmt.query_map([], row_to_storyboard).map_err(map_err)?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(map_err)
    }

    /// The jobs tagged to one storyboard, ordered by `shot_index`. S14 (render)
    /// consumes this to chain the set; S13's Library groups client-side off
    /// `list_jobs`, so it is exercised by the store test rather than production.
    #[allow(dead_code)]
    pub fn jobs_in_storyboard(&self, storyboard_id: &str) -> Result<Vec<JobSet>, JobError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT * FROM job_sets WHERE storyboard_id = ?1 \
                 ORDER BY shot_index ASC, created_at ASC",
            )
            .map_err(map_err)?;
        let rows = stmt
            .query_map(params![storyboard_id], row_to_job)
            .map_err(map_err)?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(map_err)
    }

    #[cfg(test)]
    pub fn count(&self) -> Result<i64, JobError> {
        self.conn
            .lock()
            .unwrap()
            .query_row("SELECT COUNT(*) FROM job_sets", [], |r| r.get(0))
            .map_err(map_err)
    }
}

fn row_to_storyboard(row: &rusqlite::Row<'_>) -> rusqlite::Result<Storyboard> {
    Ok(Storyboard {
        id: row.get("id")?,
        brief: row.get("brief")?,
        target_seconds: row.get("target_seconds")?,
        plan_json: row.get("plan_json")?,
        created_at: row.get("created_at")?,
        // Pre-v9 rows have none of these; they read as an all-at-once (S13) set,
        // the honest value rather than a guessed chain.
        chained: row.get::<_, i64>("chained").unwrap_or(0) != 0,
        shots_json: row.get::<_, Option<String>>("shots_json").unwrap_or(None),
        overlap_seconds: row.get::<_, Option<f64>>("overlap_seconds").unwrap_or(None),
        joins_json: row.get::<_, Option<String>>("joins_json").unwrap_or(None),
        auto_retry: row.get::<_, i64>("auto_retry").unwrap_or(0) != 0,
        // Pre-v11 rows have no keyframes column; they read `None` (non-keyframe).
        keyframes_json: row
            .get::<_, Option<String>>("keyframes_json")
            .unwrap_or(None),
    })
}

fn row_to_render(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoryboardRender> {
    Ok(StoryboardRender {
        storyboard_id: row.get("storyboard_id")?,
        output_path: row.get("output_path")?,
        width: row.get::<_, Option<i64>>("width")?.map(|n| n.max(0) as u32),
        height: row
            .get::<_, Option<i64>>("height")?
            .map(|n| n.max(0) as u32),
        seconds: row.get("seconds")?,
        actual_usd: row.get("actual_usd")?,
        created_at: row.get("created_at")?,
    })
}

fn row_to_job(row: &rusqlite::Row<'_>) -> rusqlite::Result<JobSet> {
    let status: String = row.get("status")?;
    let results: String = row.get("results")?;
    let settings: String = row.get("settings")?;
    Ok(JobSet {
        id: row.get("id")?,
        model_id: row.get("model_id")?,
        route_id: row.get("route_id")?,
        request_id: row.get("request_id")?,
        // A status we cannot parse must not drop the row — the job still
        // exists and the user still has a right to see it.
        status: serde_json::from_str(&format!("\"{status}\""))
            .unwrap_or(hickeyfield_core::JobStatus::InProgress),
        prompt: row.get("prompt")?,
        enhanced_prompt: row.get("enhanced_prompt")?,
        enhancer_version: row.get("enhancer_version").unwrap_or_default(),
        enhance_note: row.get("enhance_note").unwrap_or_default(),
        advisories: row
            .get::<_, String>("advisories")
            .ok()
            .and_then(|a| serde_json::from_str(&a).ok())
            .unwrap_or_default(),
        preset_id: row.get("preset_id")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        results: serde_json::from_str(&results).unwrap_or_default(),
        estimated_usd: row.get("estimated_usd")?,
        actual_usd: row.get("actual_usd")?,
        // Pre-v7 rows have no provenance columns; they load as None, the honest
        // value. Stored as the serde kebab-case string, parsed back the same
        // way `status` is (a value we cannot parse degrades to None, not a
        // dropped row).
        priced_from: row
            .get::<_, Option<String>>("priced_from")
            .ok()
            .flatten()
            .and_then(|s| serde_json::from_str(&format!("\"{s}\"")).ok()),
        priced_at: row.get::<_, Option<i64>>("priced_at").unwrap_or(None),
        actual_source: row
            .get::<_, Option<String>>("actual_source")
            .ok()
            .flatten()
            .and_then(|s| serde_json::from_str(&format!("\"{s}\"")).ok()),
        fail_reason: row.get("fail_reason")?,
        settings: serde_json::from_str(&settings).unwrap_or(serde_json::Value::Null),
        // Pre-v3 rows have '', which the runner reads as "fall back to the
        // route slug" — correct for them, since they predate suffixing.
        endpoint: row.get("endpoint").unwrap_or_default(),
        // Rows written before schema v2 carry '[]'; a malformed blob degrades
        // to "no attachments" rather than dropping the whole job.
        media: row
            .get::<_, String>("media")
            .ok()
            .and_then(|m| serde_json::from_str(&m).ok())
            .unwrap_or_default(),
        // Pre-v6 rows have neither column; they load as never-stalled, which is
        // correct — the cap has simply never been reached for them.
        poll_cycles: row
            .get::<_, i64>("poll_cycles")
            .map(|n| n.max(0) as u32)
            .unwrap_or(0),
        stalled: row.get::<_, i64>("stalled").unwrap_or(0) != 0,
        // Pre-v8 rows have neither column; they load as standalone (None),
        // which is the honest value rather than a guessed set membership.
        storyboard_id: row
            .get::<_, Option<String>>("storyboard_id")
            .unwrap_or(None),
        shot_index: row
            .get::<_, Option<i64>>("shot_index")
            .ok()
            .flatten()
            .map(|n| n.max(0) as u32),
    })
}

impl JobStore for SqliteStore {
    fn upsert(&self, j: &JobSet) -> Result<(), JobError> {
        let status = serde_json::to_string(&j.status)
            .map_err(map_err)?
            .trim_matches('"')
            .to_string();
        let results = serde_json::to_string(&j.results).map_err(map_err)?;
        let settings = serde_json::to_string(&j.settings).map_err(map_err)?;
        let media = serde_json::to_string(&j.media).map_err(map_err)?;
        let endpoint = j.endpoint.clone();
        let advisories = serde_json::to_string(&j.advisories).map_err(map_err)?;
        // Enum provenance stored as its serde kebab string, quotes trimmed —
        // the same shape `status` uses. `None` stays SQL NULL.
        let priced_from = j
            .priced_from
            .map(|p| serde_json::to_string(&p).map(|s| s.trim_matches('"').to_string()))
            .transpose()
            .map_err(map_err)?;
        let actual_source = j
            .actual_source
            .map(|a| serde_json::to_string(&a).map(|s| s.trim_matches('"').to_string()))
            .transpose()
            .map_err(map_err)?;

        self.conn
            .lock()
            .unwrap()
            .execute(
                r#"
                INSERT INTO job_sets (
                    id, model_id, route_id, request_id, status, prompt,
                    enhanced_prompt, preset_id, created_at, updated_at,
                    results, estimated_usd, actual_usd, fail_reason, settings,
                    media, endpoint, enhancer_version, enhance_note, advisories,
                    poll_cycles, stalled, priced_from, priced_at, actual_source,
                    storyboard_id, shot_index
                ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23,?24,?25,?26,?27)
                ON CONFLICT(id) DO UPDATE SET
                    status          = excluded.status,
                    enhanced_prompt = excluded.enhanced_prompt,
                    enhancer_version = excluded.enhancer_version,
                    enhance_note    = excluded.enhance_note,
                    advisories      = excluded.advisories,
                    updated_at      = excluded.updated_at,
                    results         = excluded.results,
                    actual_usd      = excluded.actual_usd,
                    fail_reason     = excluded.fail_reason,
                    endpoint        = excluded.endpoint,
                    poll_cycles     = excluded.poll_cycles,
                    stalled         = excluded.stalled,
                    -- `priced_from`/`priced_at` are write-once: deliberately
                    -- absent here, mirroring `estimated_usd`. `actual_source`
                    -- tracks `actual_usd` and so updates with it.
                    -- `storyboard_id`/`shot_index` are likewise write-once: a
                    -- job's set membership is fixed at submit, so they too are
                    -- absent here — a rerun-poll must never re-tag a shot.
                    actual_source   = excluded.actual_source
                "#,
                params![
                    j.id,
                    j.model_id,
                    j.route_id,
                    j.request_id,
                    status,
                    j.prompt,
                    j.enhanced_prompt,
                    j.preset_id,
                    j.created_at,
                    j.updated_at,
                    results,
                    j.estimated_usd,
                    j.actual_usd,
                    j.fail_reason,
                    settings,
                    media,
                    endpoint,
                    j.enhancer_version,
                    j.enhance_note,
                    advisories,
                    j.poll_cycles,
                    j.stalled as i64,
                    priced_from,
                    j.priced_at,
                    actual_source,
                    j.storyboard_id,
                    j.shot_index,
                ],
            )
            .map(|_| ())
            .map_err(map_err)
    }

    fn get(&self, id: &str) -> Result<Option<JobSet>, JobError> {
        self.conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT * FROM job_sets WHERE id = ?1",
                params![id],
                row_to_job,
            )
            .optional()
            .map_err(map_err)
    }

    fn all(&self) -> Result<Vec<JobSet>, JobError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT * FROM job_sets ORDER BY created_at DESC")
            .map_err(map_err)?;
        let rows = stmt.query_map([], row_to_job).map_err(map_err)?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(map_err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hickeyfield_core::engine::{Output, OutputKind};
    use hickeyfield_core::JobStatus;

    fn job(id: &str, status: JobStatus) -> JobSet {
        JobSet {
            id: id.into(),
            model_id: "kling-3.0".into(),
            route_id: "fal:kling/v3/pro".into(),
            request_id: "req".into(),
            endpoint: String::new(),
            status,
            prompt: "a cat wearing a hat".into(),
            enhanced_prompt: None,
            enhancer_version: None,
            enhance_note: None,
            advisories: Vec::new(),
            preset_id: None,
            created_at: 1,
            updated_at: 1,
            results: vec![],
            estimated_usd: Some(0.67),
            actual_usd: None,
            priced_from: None,
            priced_at: None,
            actual_source: None,
            fail_reason: None,
            settings: serde_json::json!({"duration": 8}),
            media: Vec::new(),
            poll_cycles: 0,
            stalled: false,
            storyboard_id: None,
            shot_index: None,
        }
    }

    #[test]
    fn round_trips_every_field() {
        let s = SqliteStore::in_memory().unwrap();
        let mut j = job("a", JobStatus::Completed);
        j.enhanced_prompt = Some("a tabby cat in a red fedora".into());
        j.results = vec![Output {
            url: "https://cdn/x.mp4".into(),
            kind: OutputKind::Video,
            local_path: Some("/lib/x.mp4".into()),
        }];
        j.actual_usd = Some(0.71);
        s.upsert(&j).unwrap();
        assert_eq!(s.get("a").unwrap().unwrap(), j);
    }

    #[test]
    fn migration_7_adds_provenance_columns_and_roundtrips() {
        use hickeyfield_core::cost::{ActualSource, PriceProvenance};
        let s = SqliteStore::in_memory().unwrap();
        // in_memory() runs every migration; the schema is at the latest version.
        let v: i64 = s
            .conn
            .lock()
            .unwrap()
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, 11, "in_memory must migrate to the latest schema");

        let mut j = job("p", JobStatus::Completed);
        j.priced_from = Some(PriceProvenance::Feed);
        j.priced_at = Some(1234);
        j.actual_source = None;
        s.upsert(&j).unwrap();
        assert_eq!(s.get("p").unwrap().unwrap(), j);

        // Write-once: a later upsert must NOT change priced_from/priced_at, but
        // must update actual_source alongside actual_usd.
        let mut update = s.get("p").unwrap().unwrap();
        update.priced_from = Some(PriceProvenance::Literal);
        update.priced_at = Some(9999);
        update.actual_usd = Some(0.5);
        update.actual_source = Some(ActualSource::Probe);
        s.upsert(&update).unwrap();

        let got = s.get("p").unwrap().unwrap();
        assert_eq!(
            got.priced_from,
            Some(PriceProvenance::Feed),
            "priced_from is write-once"
        );
        assert_eq!(got.priced_at, Some(1234), "priced_at is write-once");
        assert_eq!(
            got.actual_source,
            Some(ActualSource::Probe),
            "actual_source updates"
        );
        assert_eq!(got.actual_usd, Some(0.5));
    }

    #[test]
    fn migration_8_adds_storyboards_and_columns_and_roundtrips() {
        let s = SqliteStore::in_memory().unwrap();

        // A storyboard row (plan JSON) round-trips.
        let sb = Storyboard {
            id: "sb1".into(),
            brief: "a 30s ad for a cold-brew can".into(),
            target_seconds: 30.0,
            plan_json: r#"{"brief":"x","targetSeconds":30.0,"useCase":"text-to-video","shots":[]}"#
                .into(),
            created_at: 42,
            chained: false,
            shots_json: None,
            overlap_seconds: None,
            joins_json: None,
            auto_retry: false,
            keyframes_json: None,
        };
        s.upsert_storyboard(&sb).unwrap();
        assert_eq!(s.storyboard("sb1").unwrap().unwrap(), sb);

        // A job tagged to the storyboard round-trips its set membership.
        let mut j = job("shot-0", JobStatus::Queued);
        j.storyboard_id = Some("sb1".into());
        j.shot_index = Some(0);
        s.upsert(&j).unwrap();
        let back = s.get("shot-0").unwrap().unwrap();
        assert_eq!(back.storyboard_id.as_deref(), Some("sb1"));
        assert_eq!(back.shot_index, Some(0));

        // A standalone job has no set membership.
        s.upsert(&job("solo", JobStatus::Queued)).unwrap();
        let solo = s.get("solo").unwrap().unwrap();
        assert_eq!(solo.storyboard_id, None);
        assert_eq!(solo.shot_index, None);

        // Write-once: a later upsert must NOT re-tag a shot to a different set.
        let mut retag = back.clone();
        retag.storyboard_id = Some("sb-other".into());
        retag.shot_index = Some(5);
        retag.status = JobStatus::Completed;
        s.upsert(&retag).unwrap();
        let after = s.get("shot-0").unwrap().unwrap();
        assert_eq!(
            after.storyboard_id.as_deref(),
            Some("sb1"),
            "storyboard_id is write-once"
        );
        assert_eq!(after.shot_index, Some(0), "shot_index is write-once");
        assert_eq!(after.status, JobStatus::Completed, "status still updates");
    }

    #[test]
    fn jobs_in_a_storyboard_are_ordered_by_shot_index() {
        let s = SqliteStore::in_memory().unwrap();
        s.upsert_storyboard(&Storyboard {
            id: "set".into(),
            brief: "b".into(),
            target_seconds: 24.0,
            plan_json: "{}".into(),
            created_at: 1,
            chained: false,
            shots_json: None,
            overlap_seconds: None,
            joins_json: None,
            auto_retry: false,
            keyframes_json: None,
        })
        .unwrap();
        // Insert out of order; the query must return shot_index order.
        for (id, idx) in [("c", 2u32), ("a", 0), ("b", 1)] {
            let mut j = job(id, JobStatus::Completed);
            j.storyboard_id = Some("set".into());
            j.shot_index = Some(idx);
            s.upsert(&j).unwrap();
        }
        // A job in a different set must not appear.
        let mut other = job("x", JobStatus::Completed);
        other.storyboard_id = Some("elsewhere".into());
        other.shot_index = Some(0);
        s.upsert(&other).unwrap();

        let ids: Vec<_> = s
            .jobs_in_storyboard("set")
            .unwrap()
            .into_iter()
            .map(|j| j.id)
            .collect();
        assert_eq!(ids, vec!["a", "b", "c"]);
    }

    #[test]
    fn the_chained_columns_and_a_render_round_trip() {
        // S15 migration 9: the chain state on a storyboard and a render row both
        // survive a write/read, and `user_version` is 9.
        let s = SqliteStore::in_memory().unwrap();
        let sb = Storyboard {
            id: "chain".into(),
            brief: "a 30s ad".into(),
            target_seconds: 30.0,
            plan_json: "{}".into(),
            created_at: 7,
            chained: true,
            shots_json: Some(r#"[{"modelId":"m","prompt":"p"}]"#.into()),
            overlap_seconds: Some(0.0),
            joins_json: None,
            auto_retry: false,
            keyframes_json: None,
        };
        s.upsert_storyboard(&sb).unwrap();
        assert_eq!(s.storyboard("chain").unwrap().unwrap(), sb);

        let r = StoryboardRender {
            storyboard_id: "chain".into(),
            output_path: "/lib/day-0/ad.mp4".into(),
            width: Some(1280),
            height: Some(720),
            seconds: Some(30.0),
            actual_usd: Some(2.4),
            created_at: 8,
        };
        s.upsert_render(&r).unwrap();
        assert_eq!(s.render("chain").unwrap().unwrap(), r);
        // Re-render overwrites (one ad per set).
        let mut r2 = r.clone();
        r2.actual_usd = None; // an unpriced member → None, never 0
        r2.seconds = Some(28.5);
        s.upsert_render(&r2).unwrap();
        assert_eq!(s.render("chain").unwrap().unwrap(), r2);
        assert_eq!(s.renders().unwrap().len(), 1);

        let version: i64 = s
            .conn
            .lock()
            .unwrap()
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(version, 11, "migrations up to 11 must have run");
    }

    #[test]
    fn joins_and_the_auto_retry_toggle_round_trip() {
        // S16: the per-join score records and the auto-retry toggle survive a
        // write/read, and `user_version` is 10.
        let s = SqliteStore::in_memory().unwrap();
        let mut joins = Vec::new();
        upsert_join(
            &mut joins,
            JoinRecord {
                index: 1,
                kind: "continuity".into(),
                applied: true,
                ssim: Some(0.93),
                psnr: Some(34.0),
                dy: Some(2.0),
                du: Some(1.0),
                dv: Some(1.0),
                pass: Some(true),
                attempt: 0,
                end_ssim: None,
                end_psnr: None,
                end_dy: None,
                end_du: None,
                end_dv: None,
                end_pass: None,
            },
        );
        // A scoring failure records None, never 0 — and upsert replaces by index.
        upsert_join(
            &mut joins,
            JoinRecord {
                index: 1,
                kind: "continuity".into(),
                applied: true,
                ssim: None,
                psnr: None,
                dy: None,
                du: None,
                dv: None,
                pass: None,
                attempt: 1,
                end_ssim: None,
                end_psnr: None,
                end_dy: None,
                end_du: None,
                end_dv: None,
                end_pass: None,
            },
        );
        assert_eq!(joins.len(), 1, "upsert replaces by index");
        assert_eq!(joins[0].attempt, 1);

        let sb = Storyboard {
            id: "sc".into(),
            brief: "b".into(),
            target_seconds: 30.0,
            plan_json: "{}".into(),
            created_at: 1,
            chained: true,
            shots_json: None,
            overlap_seconds: Some(0.0),
            joins_json: Some(serde_json::to_string(&joins).unwrap()),
            auto_retry: true,
            keyframes_json: None,
        };
        s.upsert_storyboard(&sb).unwrap();
        let got = s.storyboard("sc").unwrap().unwrap();
        assert_eq!(got, sb, "joins_json + auto_retry round-trip");
        assert!(got.auto_retry);
        let parsed = parse_joins(got.joins_json.as_deref());
        assert_eq!(parsed[0].ssim, None, "unscored stays None, never 0");
    }

    #[test]
    fn migration_11_adds_keyframes_json_and_roundtrips() {
        // S17: `user_version` is 11, and a keyframe set's records round-trip;
        // a non-keyframe set reads `keyframes_json: None`.
        let s = SqliteStore::in_memory().unwrap();
        let version: i64 = s
            .conn
            .lock()
            .unwrap()
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(version, 11, "migrations up to 11 must have run");

        let mut kfs = Vec::new();
        upsert_keyframe(
            &mut kfs,
            KeyframeRecord {
                index: 1,
                job_set_id: Some("still-1".into()),
                local_path: None,
                estimated_usd: Some(0.04),
                attempt: 0,
            },
        );
        upsert_keyframe(
            &mut kfs,
            KeyframeRecord {
                index: 0,
                job_set_id: Some("still-0".into()),
                local_path: Some("/lib/k0.png".into()),
                estimated_usd: Some(0.04),
                attempt: 0,
            },
        );
        // upsert keeps the list ordered by index.
        assert_eq!(kfs.iter().map(|k| k.index).collect::<Vec<_>>(), vec![0, 1]);
        // A regenerate rewrites its boundary (latest write wins).
        upsert_keyframe(
            &mut kfs,
            KeyframeRecord {
                index: 1,
                job_set_id: Some("still-1b".into()),
                local_path: Some("/lib/k1.png".into()),
                estimated_usd: Some(0.04),
                attempt: 1,
            },
        );
        assert_eq!(kfs.len(), 2, "upsert replaces by index");
        assert_eq!(kfs[1].attempt, 1);

        let kf_sb = Storyboard {
            id: "kf".into(),
            brief: "a 30s keyframe ad".into(),
            target_seconds: 30.0,
            plan_json: "{}".into(),
            created_at: 3,
            chained: true,
            shots_json: None,
            overlap_seconds: Some(0.0),
            joins_json: None,
            auto_retry: false,
            keyframes_json: Some(serde_json::to_string(&kfs).unwrap()),
        };
        s.upsert_storyboard(&kf_sb).unwrap();
        let got = s.storyboard("kf").unwrap().unwrap();
        assert_eq!(got, kf_sb, "keyframes_json round-trips");
        let parsed = parse_keyframes(got.keyframes_json.as_deref());
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[1].job_set_id.as_deref(), Some("still-1b"));

        // A non-keyframe set reads None.
        let plain = Storyboard {
            id: "plain".into(),
            brief: "b".into(),
            target_seconds: 30.0,
            plan_json: "{}".into(),
            created_at: 4,
            chained: false,
            shots_json: None,
            overlap_seconds: None,
            joins_json: None,
            auto_retry: false,
            keyframes_json: None,
        };
        s.upsert_storyboard(&plain).unwrap();
        assert_eq!(s.storyboard("plain").unwrap().unwrap().keyframes_json, None);
    }

    #[test]
    fn a_pre_s17_join_record_deserialises_with_absent_end_fields() {
        // A joins_json written before S17 has no end-* keys; it must parse with
        // the end fields `None` (additive, no migration).
        let old = r#"[{"index":1,"kind":"continuity","applied":true,"ssim":0.93,"psnr":34.0,"dy":2.0,"du":1.0,"dv":1.0,"pass":true,"attempt":0}]"#;
        let parsed = parse_joins(Some(old));
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].end_ssim, None);
        assert_eq!(parsed[0].end_pass, None);
        assert_eq!(parsed[0].ssim, Some(0.93));
    }

    #[test]
    fn upsert_updates_rather_than_duplicating() {
        let s = SqliteStore::in_memory().unwrap();
        let mut j = job("a", JobStatus::Queued);
        s.upsert(&j).unwrap();
        j.status = JobStatus::Completed;
        j.updated_at = 99;
        s.upsert(&j).unwrap();

        assert_eq!(s.count().unwrap(), 1);
        let got = s.get("a").unwrap().unwrap();
        assert_eq!(got.status, JobStatus::Completed);
        assert_eq!(got.updated_at, 99);
    }

    #[test]
    fn survives_reopening_the_same_file() {
        // The whole point of the store: quit mid-generation, come back, it is
        // still there and still known to be unfinished.
        let dir = std::env::temp_dir().join(format!("hickeyfield-test-{}", std::process::id()));
        let path = dir.join("jobs.sqlite");
        let _ = std::fs::remove_dir_all(&dir);

        {
            let s = SqliteStore::open(&path).unwrap();
            s.upsert(&job("running", JobStatus::InProgress)).unwrap();
        }
        {
            let s = SqliteStore::open(&path).unwrap();
            let pending = s.unfinished().unwrap();
            assert_eq!(pending.len(), 1);
            assert_eq!(pending[0].id, "running");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn unfinished_excludes_fully_settled_work() {
        let s = SqliteStore::in_memory().unwrap();
        s.upsert(&job("running", JobStatus::InProgress)).unwrap();
        s.upsert(&job("failed", JobStatus::Failed)).unwrap();

        let mut downloaded = job("downloaded", JobStatus::Completed);
        downloaded.results = vec![Output {
            url: "u".into(),
            kind: OutputKind::Video,
            local_path: Some("/lib/u.mp4".into()),
        }];
        s.upsert(&downloaded).unwrap();

        let mut awaiting_download = job("awaiting", JobStatus::Completed);
        awaiting_download.results = vec![Output {
            url: "v".into(),
            kind: OutputKind::Video,
            local_path: None,
        }];
        s.upsert(&awaiting_download).unwrap();

        let mut ids: Vec<_> = s.unfinished().unwrap().into_iter().map(|j| j.id).collect();
        ids.sort();
        assert_eq!(ids, vec!["awaiting", "running"]);
    }

    #[test]
    fn listing_is_newest_first() {
        let s = SqliteStore::in_memory().unwrap();
        for (id, t) in [("old", 1), ("new", 3), ("mid", 2)] {
            let mut j = job(id, JobStatus::Completed);
            j.created_at = t;
            s.upsert(&j).unwrap();
        }
        let ids: Vec<_> = s.all().unwrap().into_iter().map(|j| j.id).collect();
        assert_eq!(ids, vec!["new", "mid", "old"]);
    }

    #[test]
    fn the_enhancer_pin_survives_a_round_trip() {
        // Without it a generation cannot be reproduced: the same prompt through
        // a different corpus or a different local model is a different result,
        // and the recipe would claim otherwise.
        let s = SqliteStore::in_memory().unwrap();
        let mut j = job("a", JobStatus::Completed);
        j.enhanced_prompt = Some("a rewritten scene".into());
        j.enhancer_version = Some("enhancer.v1/video+ollama/qwen2.5:7b".into());
        j.enhance_note = None;
        s.upsert(&j).unwrap();
        let back = s.get("a").unwrap().unwrap();
        assert_eq!(back.enhanced_prompt.as_deref(), Some("a rewritten scene"));
        assert_eq!(
            back.enhancer_version.as_deref(),
            Some("enhancer.v1/video+ollama/qwen2.5:7b")
        );
    }

    #[test]
    fn a_job_that_was_not_rewritten_stores_no_pin() {
        // Never a placeholder — see the field's doc comment.
        let s = SqliteStore::in_memory().unwrap();
        let mut j = job("b", JobStatus::Completed);
        j.enhance_note = Some("enhancement is off for this model".into());
        s.upsert(&j).unwrap();
        let back = s.get("b").unwrap().unwrap();
        assert!(back.enhancer_version.is_none());
        assert!(back.enhance_note.is_some());
    }

    #[test]
    fn attached_media_survives_a_round_trip() {
        // Without this, Rerun restored an image-to-video job as text-to-video:
        // a different generation, at the same price, that the user did not ask
        // for.
        use hickeyfield_core::{MediaRef, MediaRole};
        let s = SqliteStore::in_memory().unwrap();
        let mut j = job("a", JobStatus::Queued);
        j.media = vec![
            MediaRef::url(MediaRole::Start, "https://cdn/first.png"),
            MediaRef::url(MediaRole::End, "https://cdn/last.png"),
        ];
        s.upsert(&j).unwrap();

        let back = s.get("a").unwrap().unwrap();
        assert_eq!(back.media.len(), 2);
        assert_eq!(back.media[0].role, MediaRole::Start);
        assert_eq!(back.media[1].role, MediaRole::End);
    }

    #[test]
    fn a_v1_database_upgrades_without_losing_rows() {
        // Migrations are append-only, so an existing library must survive the
        // new column rather than being rebuilt.
        let s = SqliteStore::in_memory().unwrap();
        {
            let conn = s.conn.lock().unwrap();
            conn.execute_batch("PRAGMA user_version = 1;").unwrap();
            // Every column added after v1, so the replay actually starts there.
            conn.execute_batch(
                "ALTER TABLE job_sets DROP COLUMN media; \
                 ALTER TABLE job_sets DROP COLUMN endpoint; \
                 ALTER TABLE job_sets DROP COLUMN enhancer_version; \
                 ALTER TABLE job_sets DROP COLUMN enhance_note; \
                 ALTER TABLE job_sets DROP COLUMN advisories; \
                 ALTER TABLE job_sets DROP COLUMN poll_cycles; \
                 ALTER TABLE job_sets DROP COLUMN stalled; \
                 ALTER TABLE job_sets DROP COLUMN priced_from; \
                 ALTER TABLE job_sets DROP COLUMN priced_at; \
                 ALTER TABLE job_sets DROP COLUMN actual_source; \
                 ALTER TABLE job_sets DROP COLUMN storyboard_id; \
                 ALTER TABLE job_sets DROP COLUMN shot_index; \
                 DROP TABLE IF EXISTS storyboards; \
                 DROP TABLE IF EXISTS storyboard_renders;",
            )
            .unwrap();
        }
        {
            // Written the way v1 wrote them: no media column at all.
            let conn = s.conn.lock().unwrap();
            conn.execute(
                r#"INSERT INTO job_sets
                   (id, model_id, route_id, request_id, status, prompt,
                    created_at, updated_at, results, settings)
                   VALUES ('old','m','fal:x','r','completed','hi',1,1,'[]','{}')"#,
                [],
            )
            .unwrap();
        }

        s.migrate().unwrap();
        let back = s.get("old").unwrap().unwrap();
        assert_eq!(back.id, "old");
        // A pre-v2 row has no attachments, which is correct rather than lossy.
        assert!(back.media.is_empty());
        // A pre-v6 row has never reached the timeout cap.
        assert_eq!(back.poll_cycles, 0);
        assert!(!back.stalled);
    }

    #[test]
    fn stall_fields_round_trip() {
        // The v6 columns survive a write/read: a job the runner stalled after
        // several timed-out sessions loads back with its counter and flag.
        let s = SqliteStore::in_memory().unwrap();
        let mut j = job("stalled", JobStatus::InProgress);
        j.poll_cycles = 3;
        j.stalled = true;
        s.upsert(&j).unwrap();

        let back = s.get("stalled").unwrap().unwrap();
        assert_eq!(back.poll_cycles, 3);
        assert!(back.stalled);
        assert_eq!(back, j, "every field, including the v6 pair, round-trips");

        // And an update persists the new values rather than the defaults.
        j.poll_cycles = 5;
        s.upsert(&j).unwrap();
        assert_eq!(s.get("stalled").unwrap().unwrap().poll_cycles, 5);
    }

    #[test]
    fn migration_is_idempotent() {
        let s = SqliteStore::in_memory().unwrap();
        s.migrate().unwrap();
        s.migrate().unwrap();
        assert_eq!(s.count().unwrap(), 0);
    }

    #[test]
    fn delete_removes_one_row() {
        let s = SqliteStore::in_memory().unwrap();
        s.upsert(&job("a", JobStatus::Completed)).unwrap();
        s.upsert(&job("b", JobStatus::Completed)).unwrap();
        s.delete("a").unwrap();
        assert_eq!(s.count().unwrap(), 1);
        assert!(s.get("a").unwrap().is_none());
    }
}
