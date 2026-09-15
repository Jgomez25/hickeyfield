//! The job runner: owns the poll loops and streams updates to the UI.
//!
//! One task per running job, on tokio's blocking pool — but the provider poll
//! itself is gated by a per-provider concurrency limiter sized from
//! `ProviderId::features().max_concurrent`. A new fal account allows *two*
//! simultaneous requests, so at most two of its poll loops touch the wire at
//! once and the rest park on a slot until one frees; extra jobs queue and none
//! are dropped. The ceiling is the provider's and we now honour it rather than
//! amplifying it.
//!
//! Patience is per-job too: each poll session runs for the provider's
//! `TimeoutPolicy` applied to the requested work (a 4K clip earns more than a
//! still), and a session that runs out is *paused*, not failed — the row stays
//! in `unfinished()` so the next launch re-attaches it and a late result is
//! still collected.
//!
//! The runner is why a generation survives the window closing: it lives in the
//! Rust process, writes every transition to SQLite, and is restarted from the
//! store on launch rather than from anything the webview remembers.

use crate::library::Library;
use hickeyfield_core::engine::{
    apply_poll, Backoff, JobError, JobSet, JobStore, ProviderClient, TimeoutPolicy,
};
use hickeyfield_core::{Billable, ProviderId};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Builds a client for a route id (`provider:slug`). Returns `None` when the
/// user holds no key for that provider — a normal state, not an error.
pub type ClientFactory = Arc<dyn Fn(&str) -> Option<Arc<dyn ProviderClient>> + Send + Sync>;

/// Called on every observed change, so the UI can stream rather than poll.
pub type OnUpdate = Arc<dyn Fn(&JobSet) + Send + Sync>;

pub struct Runner {
    store: Arc<dyn JobStore>,
    clients: ClientFactory,
    on_update: OnUpdate,
    /// Outputs are downloaded here as soon as a job completes. Provider URLs
    /// expire — Higgsfield's own API deletes after seven days — so a result
    /// that only exists as a link is a result the user will eventually lose.
    library: Arc<Library>,
    /// Guards against two loops polling the same job, which would double the
    /// request rate against a provider that is already rate-limiting us.
    watching: Arc<Mutex<HashSet<String>>>,
    /// Jobs the user has deleted while a poll loop was still running.
    ///
    /// Without this, `delete_job` removed the row and the very next poll tick
    /// wrote it straight back — a destructive action that appeared to succeed
    /// and silently reverted, which is worse than one that visibly fails.
    abandoned: Arc<Mutex<HashSet<String>>>,
    /// Caps simultaneous provider polls per provider at the provider's own
    /// in-flight limit, so six queued fal jobs no longer 429 each other.
    limiter: Arc<ConcurrencyLimiter>,
}

impl Runner {
    pub fn new(
        store: Arc<dyn JobStore>,
        clients: ClientFactory,
        on_update: OnUpdate,
        library: Arc<Library>,
    ) -> Self {
        Runner {
            store,
            clients,
            on_update,
            library,
            watching: Arc::new(Mutex::new(HashSet::new())),
            abandoned: Arc::new(Mutex::new(HashSet::new())),
            limiter: Arc::new(ConcurrencyLimiter::new()),
        }
    }

    /// Restart polling for everything left unfinished by the last session.
    /// Returns how many were resumed.
    pub fn resume_all(&self) -> Result<usize, JobError> {
        let pending = self.store.unfinished()?;
        let n = pending.len();
        for job in pending {
            self.watch(job);
        }
        Ok(n)
    }

    /// Begin polling a job. Idempotent — calling twice starts one loop.
    ///
    /// The `watching` insert happens synchronously here, before the task is
    /// spawned, so a queued job is remembered and de-duplicated even while it is
    /// parked waiting for a concurrency slot: it is queued, never dropped.
    pub fn watch(&self, job: JobSet) {
        {
            let mut watching = self.watching.lock().unwrap();
            if !watching.insert(job.id.clone()) {
                return;
            }
        }

        let store = Arc::clone(&self.store);
        let clients = Arc::clone(&self.clients);
        let on_update = Arc::clone(&self.on_update);
        let watching = Arc::clone(&self.watching);
        let abandoned = Arc::clone(&self.abandoned);
        let library = Arc::clone(&self.library);
        let limiter = Arc::clone(&self.limiter);
        // Sized once, off the loop, from the job's own settings (AC1).
        let budget = timeout_budget(&job.route_id, &job.settings);

        tauri::async_runtime::spawn_blocking(move || {
            let id = job.id.clone();
            run_watched(
                job,
                budget,
                store,
                clients,
                on_update,
                Arc::clone(&abandoned),
                library,
                limiter,
            );
            watching.lock().unwrap().remove(&id);
            abandoned.lock().unwrap().remove(&id);
        });
    }

    pub fn is_watching(&self, id: &str) -> bool {
        self.watching.lock().unwrap().contains(id)
    }

    /// Stop caring about a job, so a running poll loop cannot resurrect it.
    ///
    /// Marks the tombstone *before* the caller deletes the row: the reverse
    /// order leaves a window in which a poll tick re-inserts it.
    pub fn forget(&self, id: &str) {
        self.abandoned.lock().unwrap().insert(id.to_string());
    }

    pub fn cancel(&self, id: &str) -> Result<(), JobError> {
        let Some(job) = self.store.get(id)? else {
            return Err(JobError::Permanent(format!("no such job: {id}")));
        };
        let Some(client) = (self.clients)(&job.route_id) else {
            return Err(JobError::Permanent("no client for this route".into()));
        };
        // route_id is `provider:slug`; fal scopes its queue under the slug, so
        // strip the provider prefix rather than sending the whole id.
        let app = if job.endpoint.is_empty() {
            job.route_id.split_once(':').map(|(_, s)| s).unwrap_or("")
        } else {
            job.endpoint.as_str()
        };
        client.cancel(app, &job.request_id)
    }
}

/// Pull every output onto disk. A completed job whose bytes are still only on
/// the provider's CDN is not finished — see `JobSet::is_settled`.
///
/// A download failure is recorded on the job but never flips it back to
/// failed: the generation succeeded and the user was charged for it, so the
/// honest state is "done, but we could not save it".
fn download_outputs(
    mut job: JobSet,
    store: Arc<dyn JobStore>,
    on_update: OnUpdate,
    library: Arc<Library>,
) {
    if job.status.phase() != hickeyfield_core::Phase::Completed || job.results.is_empty() {
        return;
    }
    let Ok(client) = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
    else {
        return;
    };

    let mut changed = false;
    let prompt = job.prompt.clone();
    let created = job.created_at;
    for out in job.results.iter_mut().filter(|o| o.local_path.is_none()) {
        match library.fetch(out, &prompt, created, &client) {
            Ok(path) => {
                out.local_path = Some(path.display().to_string());
                changed = true;
            }
            Err(e) => tracing::warn!("could not save {}: {e}", out.url),
        }
    }

    if changed {
        job.updated_at = now_secs();
        let _ = store.upsert(&job);
        on_update(&job);
    }
}

/// Patience budget for one poll session: the provider's `TimeoutPolicy` applied
/// to the job's requested work. Pure — no `Instant`, no store — so a test can
/// assert the budget value directly (AC1).
///
/// `route_id` is `provider:slug` (see `app::client_for`). An unroutable prefix
/// falls back to the known-good hosted budget; a settings blob that will not
/// deserialize into a `SettingsDto` (e.g. the `Value::Null` `submit_job` writes
/// when serialization fails, or a pre-policy legacy row) sizes to `None`, which
/// `TimeoutPolicy::timeout` maps to `policy.base` — never zero, since a zero
/// budget would fail every job on its first poll tick.
fn timeout_budget(route_id: &str, settings: &serde_json::Value) -> Duration {
    let policy = route_id
        .split(':')
        .next()
        .and_then(ProviderId::from_slug)
        .map(|p| p.features().timeout)
        .unwrap_or(TimeoutPolicy::HOSTED);
    // Same conversion submit and pricing use (commands.rs), so patience and
    // price are derived from one view of the job.
    let work = serde_json::from_value::<crate::commands::SettingsDto>(settings.clone())
        .ok()
        .map(|s| Billable::from(&s));
    policy.timeout(work.as_ref())
}

/// A timeout is a pause, not a death. Leaves `job.status` non-terminal so
/// `is_settled()` stays false and `unfinished()` still returns the row, so the
/// next `resume_all` re-attaches it and a late provider result can still be
/// downloaded. Records an advisory (never a `fail_reason`, which would mark the
/// job failed). Returns whether it changed anything, to gate the upsert, and
/// de-duplicates the note so repeated timeout/relaunch cycles do not stack
/// identical advisories.
fn mark_timed_out(job: &mut JobSet, budget: Duration) -> bool {
    const NOTE: &str = "no result yet after";
    if job.advisories.iter().any(|a| a.starts_with(NOTE)) {
        return false;
    }
    job.advisories.push(format!(
        "{NOTE} {} min of polling; still running at the provider — will keep \
         trying when the app next launches",
        budget.as_secs() / 60
    ));
    job.updated_at = now_secs();
    true
}

/// Whether the provider still has something to tell us. A terminal job is one
/// the provider has finished with; re-polling it can only overwrite what it
/// already reported — and if its status URL has since 404'd, overwrite a paid,
/// recoverable success with `Failed`. Only Completed-but-undownloaded rows reach
/// the runner still terminal (via `unfinished()`); those skip the poll and go
/// straight to the download retry (AC4).
fn provider_poll_needed(job: &JobSet) -> bool {
    !job.is_terminal()
}

/// A blocking counting semaphore, one slot per provider, sized from each
/// provider's `Features::max_concurrent`. It gates the provider poll only — not
/// the CDN download — because a provider's in-flight cap is on generation
/// requests, not result fetches.
struct Slot {
    /// Permits currently available. Guarded by its own mutex; `acquire` waits on
    /// `free` while this is zero, so it can never underflow.
    avail: Mutex<u32>,
    free: Condvar,
}

/// RAII permit. Releases its slot on `Drop`, which runs on *every* exit path out
/// of the guarded block — normal return, timeout, missing client, or a panic
/// unwinding through the poll loop — so a slot cannot leak and stall the queue.
struct Permit {
    slot: Arc<Slot>,
}

impl Drop for Permit {
    fn drop(&mut self) {
        // Recover from a poisoned lock rather than double-panic: a slot that is
        // never returned would stop the queue forever, the one outcome worse
        // than the panic already unwinding.
        let mut avail = self.slot.avail.lock().unwrap_or_else(|e| e.into_inner());
        *avail += 1;
        // Wake exactly one parked acquirer; the increment above is what it will
        // observe, so no wakeup is missed and none is wasted on a full slot.
        self.slot.free.notify_one();
    }
}

struct ConcurrencyLimiter {
    slots: HashMap<ProviderId, Arc<Slot>>,
}

impl ConcurrencyLimiter {
    fn new() -> Self {
        let mut slots = HashMap::new();
        for p in ProviderId::ALL {
            slots.insert(
                p,
                Arc::new(Slot {
                    // `permits()` is guaranteed >= 1 (provider.rs), so no slot
                    // starts empty and the queue can never deadlock on one.
                    avail: Mutex::new(p.features().max_concurrent.permits()),
                    free: Condvar::new(),
                }),
            );
        }
        ConcurrencyLimiter { slots }
    }

    /// Block until a permit for `provider` is free, then take it. Waits on the
    /// condvar (re-checking under the lock, so a spurious wakeup or a missed
    /// notify cannot let it through with `avail == 0`) and only decrements when
    /// a permit is actually available.
    fn acquire(&self, provider: ProviderId) -> Permit {
        let slot = Arc::clone(&self.slots[&provider]);
        let mut avail = slot.avail.lock().unwrap_or_else(|e| e.into_inner());
        while *avail == 0 {
            avail = slot.free.wait(avail).unwrap_or_else(|e| e.into_inner());
        }
        *avail -= 1;
        drop(avail);
        Permit { slot }
    }
}

/// The body of one watched job, off the tokio runtime so it is unit-testable.
///
/// A Completed-but-undownloaded job (returned by `unfinished()` because its
/// bytes are not local yet) skips the provider poll entirely (AC4) and goes
/// straight to the download retry. Everything else acquires one per-provider
/// permit (AC3) around `poll_until_terminal` and releases it — via `Permit`'s
/// `Drop` at the end of the block — before the download, which is not capped.
#[allow(clippy::too_many_arguments)]
fn run_watched(
    job: JobSet,
    budget: Duration,
    store: Arc<dyn JobStore>,
    clients: ClientFactory,
    on_update: OnUpdate,
    abandoned: Arc<Mutex<HashSet<String>>>,
    library: Arc<Library>,
    limiter: Arc<ConcurrencyLimiter>,
) {
    let id = job.id.clone();
    // Checked before acquiring a permit, so a job the user deleted while it was
    // parked in the queue never consumes a slot another job is waiting for.
    if abandoned.lock().unwrap().contains(&id) {
        return;
    }

    let finished = if provider_poll_needed(&job) {
        // The permit is held only for the poll: it is scoped to this block and
        // its `Drop` releases the slot before `download_outputs` runs below.
        let _permit = job
            .route_id
            .split(':')
            .next()
            .and_then(ProviderId::from_slug)
            .map(|p| limiter.acquire(p));
        poll_until_terminal(
            job,
            budget,
            Arc::clone(&store),
            clients,
            Arc::clone(&on_update),
            Arc::clone(&abandoned),
        )
    } else {
        // Already terminal (Completed, bytes not yet saved): never re-poll it.
        job
    };

    // Check once more before writing the outputs: a job deleted during its final
    // poll would otherwise reappear complete.
    if !abandoned.lock().unwrap().contains(&id) {
        download_outputs(finished, store, on_update, library);
    }
}

/// The loop itself. Blocking, so it runs on the blocking pool. Returns the
/// job in its final state so the caller can fetch its outputs.
fn poll_until_terminal(
    mut job: JobSet,
    budget: Duration,
    store: Arc<dyn JobStore>,
    clients: ClientFactory,
    on_update: OnUpdate,
    abandoned: Arc<Mutex<HashSet<String>>>,
) -> JobSet {
    let is_abandoned = {
        let id = job.id.clone();
        let abandoned = Arc::clone(&abandoned);
        move || abandoned.lock().unwrap().contains(&id)
    };
    let Some(client) = clients(&job.route_id) else {
        job.status = hickeyfield_core::JobStatus::Failed;
        job.fail_reason = Some(format!(
            "no credentials for {} — add a key in Settings",
            job.route_id.split(':').next().unwrap_or("this provider")
        ));
        job.updated_at = now_secs();
        let _ = store.upsert(&job);
        on_update(&job);
        return job;
    };

    // Computed once: fal's status endpoint is scoped under the originating app,
    // and the bare form answers 405.
    // What submit actually called. Pre-v3 rows have none, and they predate
    // suffixing, so the route slug is right for them.
    let app_path = if job.endpoint.is_empty() {
        job.route_id
            .split_once(':')
            .map(|(_, slug)| slug.to_string())
            .unwrap_or_default()
    } else {
        job.endpoint.clone()
    };

    let started = Instant::now();
    let interval = client.poll_interval();
    let backoff = Backoff::default();
    let mut consecutive_errors: u32 = 0;

    loop {
        // Checked at the top of every tick rather than only at the end, so a
        // long-running generation stops costing us polls the moment the user
        // deletes it.
        if is_abandoned() {
            return job;
        }
        if started.elapsed() > budget {
            // A pause, not a failure: the job stays non-terminal so it remains
            // in unfinished() and the next launch re-attaches it (AC2).
            if mark_timed_out(&mut job, budget) {
                let _ = store.upsert(&job);
                on_update(&job);
            }
            return job;
        }

        match client.poll(&app_path, &job.request_id) {
            Ok(result) => {
                consecutive_errors = 0;
                if apply_poll(&mut job, result, now_secs()) && !is_abandoned() {
                    let _ = store.upsert(&job);
                    on_update(&job);
                }
                if job.is_terminal() {
                    return job;
                }
                std::thread::sleep(interval);
            }
            Err(e) if e.is_retryable() => {
                consecutive_errors += 1;
                match backoff.delay(consecutive_errors) {
                    Some(d) => std::thread::sleep(d),
                    None => {
                        // Exhausted. The generation may well have succeeded on
                        // the provider's side, so say that rather than claiming
                        // it failed outright.
                        job.status = hickeyfield_core::JobStatus::Failed;
                        job.fail_reason = Some(format!(
                            "lost contact with the provider after {} attempts: {e}",
                            backoff.max_attempts
                        ));
                        job.updated_at = now_secs();
                        let _ = store.upsert(&job);
                        on_update(&job);
                        return job;
                    }
                }
            }
            Err(e) => {
                job.status = hickeyfield_core::JobStatus::Failed;
                job.fail_reason = Some(e.to_string());
                job.updated_at = now_secs();
                let _ = store.upsert(&job);
                on_update(&job);
                return job;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hickeyfield_core::engine::{Output, OutputKind, PollResult};
    use hickeyfield_core::JobStatus;
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    #[derive(Default)]
    struct MemStore(Mutex<HashMap<String, JobSet>>);
    impl JobStore for MemStore {
        fn upsert(&self, j: &JobSet) -> Result<(), JobError> {
            self.0.lock().unwrap().insert(j.id.clone(), j.clone());
            Ok(())
        }
        fn get(&self, id: &str) -> Result<Option<JobSet>, JobError> {
            Ok(self.0.lock().unwrap().get(id).cloned())
        }
        fn all(&self) -> Result<Vec<JobSet>, JobError> {
            Ok(self.0.lock().unwrap().values().cloned().collect())
        }
    }

    /// Returns a scripted sequence of poll results, then repeats the last.
    struct ScriptedClient {
        script: Mutex<Vec<Result<PollResult, JobError>>>,
        calls: AtomicUsize,
    }

    impl ScriptedClient {
        fn new(script: Vec<Result<PollResult, JobError>>) -> Self {
            ScriptedClient {
                script: Mutex::new(script),
                calls: AtomicUsize::new(0),
            }
        }
    }

    impl ProviderClient for ScriptedClient {
        fn submit(
            &self,
            _m: &str,
            _b: &serde_json::Value,
        ) -> Result<hickeyfield_core::engine::Submission, JobError> {
            Ok(hickeyfield_core::engine::Submission {
                request_id: "req".into(),
                status_url: None,
            })
        }
        fn poll(&self, _model_id: &str, _id: &str) -> Result<PollResult, JobError> {
            let n = self.calls.fetch_add(1, Ordering::SeqCst);
            let s = self.script.lock().unwrap();
            s.get(n)
                .cloned()
                .unwrap_or_else(|| s.last().unwrap().clone())
        }
        fn poll_interval(&self) -> Duration {
            Duration::from_millis(1)
        }
    }

    fn job(id: &str) -> JobSet {
        JobSet {
            id: id.into(),
            model_id: "m".into(),
            route_id: "fal:m".into(),
            request_id: "req".into(),
            endpoint: String::new(),
            status: JobStatus::Queued,
            prompt: "p".into(),
            enhanced_prompt: None,
            enhancer_version: None,
            enhance_note: None,
            advisories: Vec::new(),
            preset_id: None,
            created_at: 0,
            updated_at: 0,
            results: vec![],
            estimated_usd: None,
            actual_usd: None,
            fail_reason: None,
            settings: serde_json::json!({}),
            media: Vec::new(),
        }
    }

    fn ok(status: JobStatus) -> Result<PollResult, JobError> {
        Ok(PollResult {
            status,
            outputs: vec![],
            fail_reason: None,
            actual_usd: None,
        })
    }

    fn run(script: Vec<Result<PollResult, JobError>>) -> (Arc<MemStore>, usize) {
        let store = Arc::new(MemStore::default());
        let client: Arc<dyn ProviderClient> = Arc::new(ScriptedClient::new(script));
        let updates = Arc::new(AtomicUsize::new(0));
        let u = Arc::clone(&updates);

        poll_until_terminal(
            job("j"),
            // Generous budget: scripted terminal results settle in ms, so the
            // existing tests are unaffected by the timeout branch.
            TimeoutPolicy::HOSTED.timeout(None),
            Arc::clone(&store) as Arc<dyn JobStore>,
            Arc::new(move |_| Some(Arc::clone(&client))),
            Arc::new(move |_| {
                u.fetch_add(1, Ordering::SeqCst);
            }),
            Arc::new(Mutex::new(HashSet::new())),
        );
        let n = updates.load(Ordering::SeqCst);
        (store, n)
    }

    #[test]
    fn runs_to_completion_and_persists_each_transition() {
        let (store, updates) = run(vec![
            ok(JobStatus::Queued),
            ok(JobStatus::InProgress),
            Ok(PollResult {
                status: JobStatus::Completed,
                outputs: vec![Output {
                    url: "https://a/x.mp4".into(),
                    kind: OutputKind::Video,
                    local_path: None,
                }],
                fail_reason: None,
                actual_usd: Some(0.5),
            }),
        ]);
        let j = store.get("j").unwrap().unwrap();
        assert_eq!(j.status, JobStatus::Completed);
        assert_eq!(j.results.len(), 1);
        assert_eq!(j.actual_usd, Some(0.5));
        // Queued->Queued is not a change, so only two transitions are emitted.
        assert_eq!(updates, 2, "should emit only on real changes");
    }

    #[test]
    fn a_transient_error_is_retried_rather_than_failing_the_job() {
        let (store, _) = run(vec![
            Err(JobError::Transient("502".into())),
            Err(JobError::Transient("502".into())),
            ok(JobStatus::Completed),
        ]);
        assert_eq!(
            store.get("j").unwrap().unwrap().status,
            JobStatus::Completed
        );
    }

    #[test]
    fn a_permanent_error_fails_immediately_with_the_reason() {
        let (store, _) = run(vec![Err(JobError::Permanent("401 bad key".into()))]);
        let j = store.get("j").unwrap().unwrap();
        assert_eq!(j.status, JobStatus::Failed);
        assert!(j.fail_reason.unwrap().contains("401"));
    }

    #[test]
    fn exhausted_retries_say_contact_was_lost_not_that_it_failed() {
        // The provider may well have produced the output; claiming a hard
        // failure would be a lie the user cannot check.
        let (store, _) = run(vec![Err(JobError::Transient("timeout".into()))]);
        let j = store.get("j").unwrap().unwrap();
        assert_eq!(j.status, JobStatus::Failed);
        let reason = j.fail_reason.unwrap();
        assert!(reason.contains("lost contact"), "got: {reason}");
    }

    #[test]
    fn an_abandoned_job_is_never_written_back() {
        // The delete bug: the row was removed, the next poll tick upserted it,
        // and the job reappeared seconds later. A destructive action that
        // silently reverts is worse than one that visibly fails.
        let store = Arc::new(MemStore::default());
        let client: Arc<dyn ProviderClient> = Arc::new(ScriptedClient::new(vec![Ok(PollResult {
            status: hickeyfield_core::JobStatus::InProgress,
            outputs: vec![],
            fail_reason: None,
            actual_usd: None,
        })]));
        let tomb = Arc::new(Mutex::new(HashSet::new()));
        tomb.lock().unwrap().insert("j".to_string());

        let out = poll_until_terminal(
            job("j"),
            TimeoutPolicy::HOSTED.timeout(None),
            Arc::clone(&store) as Arc<dyn JobStore>,
            Arc::new(move |_| Some(Arc::clone(&client))),
            Arc::new(|_| {}),
            Arc::clone(&tomb),
        );
        // Returned immediately without touching the store.
        assert!(
            store.all().unwrap().is_empty(),
            "an abandoned job must not be persisted"
        );
        assert_eq!(out.id, "j");
    }

    #[test]
    fn a_live_job_is_still_written() {
        // The other half: the tombstone must not suppress ordinary progress.
        let (store, updates) = run(vec![Ok(PollResult {
            status: hickeyfield_core::JobStatus::Completed,
            outputs: vec![],
            fail_reason: None,
            actual_usd: None,
        })]);
        assert!(!store.all().unwrap().is_empty());
        assert!(updates > 0);
    }

    #[test]
    fn a_missing_credential_is_explained_not_swallowed() {
        let store = Arc::new(MemStore::default());
        poll_until_terminal(
            job("j"),
            TimeoutPolicy::HOSTED.timeout(None),
            Arc::clone(&store) as Arc<dyn JobStore>,
            Arc::new(|_| None),
            Arc::new(|_| {}),
            Arc::new(Mutex::new(HashSet::new())),
        );
        let j = store.get("j").unwrap().unwrap();
        assert_eq!(j.status, JobStatus::Failed);
        let reason = j.fail_reason.unwrap();
        assert!(reason.contains("fal"), "should name the provider: {reason}");
        assert!(reason.contains("Settings"), "should say where to fix it");
    }

    #[test]
    fn nsfw_is_terminal_and_keeps_its_own_status() {
        // Collapsing this into a generic failure would hide why it happened and
        // that the provider refunds it.
        let (store, _) = run(vec![ok(JobStatus::Nsfw)]);
        assert_eq!(store.get("j").unwrap().unwrap().status, JobStatus::Nsfw);
    }

    #[test]
    fn intermediate_stages_do_not_terminate_the_loop() {
        let (store, updates) = run(vec![
            ok(JobStatus::Dna),
            ok(JobStatus::Script),
            ok(JobStatus::Visuals),
            ok(JobStatus::Completed),
        ]);
        assert_eq!(
            store.get("j").unwrap().unwrap().status,
            JobStatus::Completed
        );
        assert_eq!(updates, 4);
    }

    // ----- AC1: per-job timeout budget replaces the flat DEFAULT_TIMEOUT -----

    #[test]
    fn a_4k_job_gets_a_bigger_budget_than_a_short_one() {
        // Same 10s clip, two resolutions. Asking for more output must never
        // shorten the deadline — the property the old flat 600s violated.
        let big = timeout_budget(
            "fal:m",
            &serde_json::json!({"duration": 10.0, "resolution": "4k", "aspect": "16:9"}),
        );
        let small = timeout_budget(
            "fal:m",
            &serde_json::json!({"duration": 10.0, "resolution": "720p", "aspect": "16:9"}),
        );
        assert!(
            big > small,
            "a 4K job must earn more patience than a 720p one: {big:?} vs {small:?}"
        );
        assert!(
            big > Duration::from_secs(600),
            "the 4K budget must exceed the old flat 600s: {big:?}"
        );
    }

    #[test]
    fn an_unknown_provider_and_unsizable_settings_fall_back_to_base() {
        // Unroutable prefix -> HOSTED policy; a settings blob that will not
        // deserialize (the Value::Null submit_job writes on a serialize failure,
        // and the shape a pre-policy legacy row degrades to) -> work None ->
        // policy.base. Base, never zero: a zero budget fails every job on tick 1.
        let fallback = timeout_budget("nope:m", &serde_json::Value::Null);
        assert_eq!(fallback, TimeoutPolicy::HOSTED.timeout(None));
        assert_eq!(fallback, TimeoutPolicy::HOSTED.base);
        assert!(fallback > Duration::ZERO);
        // A known provider with an unsizable row degrades to base the same way.
        assert_eq!(
            timeout_budget("fal:m", &serde_json::Value::Null),
            TimeoutPolicy::HOSTED.timeout(None)
        );
    }

    // ----- AC2: a timed-out job is paused, not failed, and stays resumable ----

    #[test]
    fn mark_timed_out_pauses_without_failing() {
        let mut j = job("j");
        j.status = JobStatus::InProgress;

        let changed = mark_timed_out(&mut j, Duration::from_secs(900));
        assert!(changed, "the first timeout records an advisory");
        assert_eq!(
            j.status,
            JobStatus::InProgress,
            "status must stay exactly what the provider last reported"
        );
        assert!(!j.is_terminal(), "a paused job is not terminal");
        assert!(j.fail_reason.is_none(), "a pause is not a failure");
        assert_eq!(j.advisories.len(), 1, "one advisory recorded");

        let again = mark_timed_out(&mut j, Duration::from_secs(900));
        assert!(!again, "a second timeout on the same job is de-duplicated");
        assert_eq!(j.advisories.len(), 1, "the advisory is not stacked");
    }

    #[test]
    fn a_timed_out_job_stays_in_unfinished() {
        // A tiny budget forces the real `started.elapsed() > budget` branch in
        // ~ms, deterministically, against an always-InProgress client.
        let store = Arc::new(MemStore::default());
        let client: Arc<dyn ProviderClient> =
            Arc::new(ScriptedClient::new(vec![ok(JobStatus::InProgress)]));

        let out = poll_until_terminal(
            job("j"),
            Duration::from_millis(1),
            Arc::clone(&store) as Arc<dyn JobStore>,
            Arc::new(move |_| Some(Arc::clone(&client))),
            Arc::new(|_| {}),
            Arc::new(Mutex::new(HashSet::new())),
        );

        assert!(
            !out.is_terminal(),
            "a timeout must not mark the job terminal"
        );
        assert!(
            !out.is_settled(),
            "an unsettled job is still owed to the user"
        );
        assert!(
            out.advisories.iter().any(|a| a.contains("no result yet")),
            "the pause is recorded as an advisory"
        );
        // unfinished() is exactly what resume_all reattaches on next launch.
        let unfinished = store.unfinished().unwrap();
        assert!(
            unfinished.iter().any(|j| j.id == "j"),
            "the timed-out job must remain resumable via unfinished()"
        );
    }

    #[test]
    fn a_late_result_is_reattached_after_a_timeout() {
        // A job left InProgress with the timeout advisory (how resume_all finds
        // it), re-polled with a generous budget against a client that now
        // answers Completed + outputs + actual_usd. The paid result survives.
        let store = Arc::new(MemStore::default());
        let mut j = job("late");
        j.status = JobStatus::InProgress;
        mark_timed_out(&mut j, Duration::from_secs(900));
        store.upsert(&j).unwrap();

        let client: Arc<dyn ProviderClient> = Arc::new(ScriptedClient::new(vec![Ok(PollResult {
            status: JobStatus::Completed,
            outputs: vec![Output {
                url: "https://a/late.mp4".into(),
                kind: OutputKind::Video,
                local_path: None,
            }],
            fail_reason: None,
            actual_usd: Some(0.75),
        })]));

        let out = poll_until_terminal(
            j,
            TimeoutPolicy::HOSTED.timeout(None),
            Arc::clone(&store) as Arc<dyn JobStore>,
            Arc::new(move |_| Some(Arc::clone(&client))),
            Arc::new(|_| {}),
            Arc::new(Mutex::new(HashSet::new())),
        );

        assert_eq!(
            out.status,
            JobStatus::Completed,
            "the late result completed"
        );
        assert_eq!(out.results.len(), 1, "the output was captured");
        assert_eq!(out.results[0].url, "https://a/late.mp4");
        assert_eq!(
            out.actual_usd,
            Some(0.75),
            "billing on the late result stuck"
        );
    }

    // ----- AC3: per-provider concurrency cap, none dropped, no deadlock -------

    #[test]
    fn provider_cap_bounds_simultaneous_poll_loops() {
        // Six jobs, a fal cap of two. At most two may hold a permit at once and
        // all six must eventually run — the extras park, none are dropped.
        let limiter = Arc::new(ConcurrencyLimiter::new());
        let inflight = Arc::new(AtomicUsize::new(0));
        let max = Arc::new(AtomicUsize::new(0));
        let done = Arc::new(AtomicUsize::new(0));

        let mut handles = Vec::new();
        for _ in 0..6 {
            let limiter = Arc::clone(&limiter);
            let inflight = Arc::clone(&inflight);
            let max = Arc::clone(&max);
            let done = Arc::clone(&done);
            handles.push(std::thread::spawn(move || {
                let permit = limiter.acquire(ProviderId::Fal);
                let now = inflight.fetch_add(1, Ordering::SeqCst) + 1;
                max.fetch_max(now, Ordering::SeqCst);
                std::thread::sleep(Duration::from_millis(5));
                inflight.fetch_sub(1, Ordering::SeqCst);
                drop(permit); // release on this path
                done.fetch_add(1, Ordering::SeqCst);
            }));
        }
        for h in handles {
            h.join().unwrap();
        }

        assert!(
            max.load(Ordering::SeqCst) <= 2,
            "fal is capped at two in flight, but saw {} at once",
            max.load(Ordering::SeqCst)
        );
        assert_eq!(
            done.load(Ordering::SeqCst),
            6,
            "every queued job must run — the cap queues, it does not drop"
        );
    }

    #[test]
    fn local_runs_one_at_a_time() {
        // Local inference is serialized to one GPU job: max in flight is exactly 1.
        let limiter = Arc::new(ConcurrencyLimiter::new());
        let inflight = Arc::new(AtomicUsize::new(0));
        let max = Arc::new(AtomicUsize::new(0));
        let done = Arc::new(AtomicUsize::new(0));

        let mut handles = Vec::new();
        for _ in 0..4 {
            let limiter = Arc::clone(&limiter);
            let inflight = Arc::clone(&inflight);
            let max = Arc::clone(&max);
            let done = Arc::clone(&done);
            handles.push(std::thread::spawn(move || {
                let _permit = limiter.acquire(ProviderId::Local);
                let now = inflight.fetch_add(1, Ordering::SeqCst) + 1;
                max.fetch_max(now, Ordering::SeqCst);
                std::thread::sleep(Duration::from_millis(3));
                inflight.fetch_sub(1, Ordering::SeqCst);
                done.fetch_add(1, Ordering::SeqCst);
            }));
        }
        for h in handles {
            h.join().unwrap();
        }

        assert_eq!(
            max.load(Ordering::SeqCst),
            1,
            "one GPU, one local job at a time"
        );
        assert_eq!(done.load(Ordering::SeqCst), 4, "none dropped");
    }

    // ----- AC4: a Completed-but-undownloaded job is not re-polled on relaunch -

    #[test]
    fn a_completed_job_is_not_re_polled() {
        let mut completed = job("c");
        completed.status = JobStatus::Completed;
        assert!(
            !provider_poll_needed(&completed),
            "a terminal job has nothing left to poll for"
        );

        let mut running = job("r");
        running.status = JobStatus::InProgress;
        assert!(
            provider_poll_needed(&running),
            "a running job must still be polled"
        );
    }

    #[test]
    fn resuming_a_completed_job_keeps_its_success() {
        // A Completed job whose output is already saved locally (so
        // download_outputs no-ops offline). Its client is scripted to a 404 —
        // the expired-status-URL case that used to flip the row to Failed. The
        // guard must skip the poll entirely: poll count 0, success preserved.
        let store = Arc::new(MemStore::default());
        let mut j = job("done");
        j.status = JobStatus::Completed;
        j.actual_usd = Some(1.25);
        j.results = vec![Output {
            url: "https://a/done.mp4".into(),
            kind: OutputKind::Video,
            local_path: Some("/tmp/already-saved.mp4".into()),
        }];
        store.upsert(&j).unwrap();

        let scripted = Arc::new(ScriptedClient::new(vec![Err(JobError::Permanent(
            "HTTP 404: status url expired".into(),
        ))]));
        let counter = Arc::clone(&scripted);
        let client: Arc<dyn ProviderClient> = scripted;

        let library = Arc::new(Library::new(
            std::env::temp_dir().join("hickeyfield-ac4-never-written"),
        ));

        run_watched(
            j,
            TimeoutPolicy::HOSTED.timeout(None),
            Arc::clone(&store) as Arc<dyn JobStore>,
            Arc::new(move |_| Some(Arc::clone(&client))),
            Arc::new(|_| {}),
            Arc::new(Mutex::new(HashSet::new())),
            library,
            Arc::new(ConcurrencyLimiter::new()),
        );

        assert_eq!(
            counter.calls.load(Ordering::SeqCst),
            0,
            "a completed job must never be re-polled on relaunch"
        );
        let stored = store.get("done").unwrap().unwrap();
        assert_eq!(
            stored.status,
            JobStatus::Completed,
            "the terminal-success record must survive the relaunch"
        );
        assert_eq!(
            stored.results[0].url, "https://a/done.mp4",
            "the result URL must survive"
        );
        assert_eq!(
            stored.actual_usd,
            Some(1.25),
            "the billing record must survive"
        );
    }
}
