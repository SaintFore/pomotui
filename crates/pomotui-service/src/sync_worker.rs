use crate::Service;
use pomotui_protocol::{Command, Handler, Request, Response};
use pomotui_sync::Document as SyncDocument;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub trait SyncFileAdapter: Send + Sync + 'static {
    /// Discovers bounded transport-specific conflict siblings.
    ///
    /// # Errors
    ///
    /// Returns a stage-ready filesystem diagnostic.
    fn discover_conflicts(&self, _path: &Path) -> Result<Vec<std::path::PathBuf>, String> {
        Ok(Vec::new())
    }

    /// Ensures an already-current publication is durable before cleanup.
    /// # Errors
    /// Returns a durability diagnostic.
    fn confirm_publication(&self, _path: &Path, _document: &str) -> Result<(), String> {
        Ok(())
    }

    /// Observes source bytes and, where supported, stable filesystem identity.
    /// # Errors
    /// Returns a candidate-read diagnostic.
    fn observe_candidate(
        &self,
        path: &Path,
    ) -> Result<Option<pomotui_platform::SyncCandidate>, String> {
        self.read(path).map(|source| {
            source.map(|source| pomotui_platform::SyncCandidate::retained(path.into(), source))
        })
    }

    /// Retires an exact observation after successful durable import and publication.
    /// # Errors
    /// Unsupported cleanup and changed candidates are retained with a diagnostic.
    fn cleanup_candidate(
        &self,
        _main: &Path,
        candidate: &pomotui_platform::SyncCandidate,
    ) -> Result<(), String> {
        Err(format!(
            "cleanup unsupported; retained {}",
            candidate.path.display()
        ))
    }

    /// Reads a synchronization document, or `None` when the path is missing.
    ///
    /// # Errors
    /// Returns a stage-ready filesystem diagnostic.
    fn read(&self, path: &Path) -> Result<Option<String>, String>;

    /// Atomically replaces the destination with a validated document.
    ///
    /// # Errors
    ///
    /// Returns a stage-ready validation or filesystem diagnostic.
    fn replace(&self, path: &Path, document: &str) -> Result<(), String>;
}

struct PlatformSyncFile;

impl SyncFileAdapter for PlatformSyncFile {
    fn discover_conflicts(&self, path: &Path) -> Result<Vec<std::path::PathBuf>, String> {
        pomotui_platform::discover_sync_conflicts(path)
    }
    fn confirm_publication(&self, path: &Path, document: &str) -> Result<(), String> {
        pomotui_platform::confirm_sync_publication(path, document)
    }
    fn observe_candidate(
        &self,
        path: &Path,
    ) -> Result<Option<pomotui_platform::SyncCandidate>, String> {
        pomotui_platform::observe_sync_candidate(path)
    }
    fn cleanup_candidate(
        &self,
        main: &Path,
        candidate: &pomotui_platform::SyncCandidate,
    ) -> Result<(), String> {
        pomotui_platform::cleanup_sync_candidate(main, candidate)
    }
    fn read(&self, path: &Path) -> Result<Option<String>, String> {
        pomotui_platform::read_sync_file(path)
    }

    fn replace(&self, path: &Path, document: &str) -> Result<(), String> {
        pomotui_platform::replace_sync_file(path, document)
    }
}

#[derive(Clone)]
pub struct SyncWorkerTrigger {
    sender: std::sync::mpsc::SyncSender<()>,
    rebuild_requested: Arc<std::sync::atomic::AtomicBool>,
}

impl SyncWorkerTrigger {
    pub fn request(&self) {
        let _ = self.sender.try_send(());
    }

    pub fn request_rebuild(&self) {
        self.rebuild_requested
            .store(true, std::sync::atomic::Ordering::Release);
        self.request();
    }
}

pub struct SyncWorker {
    trigger: SyncWorkerTrigger,
    thread: std::thread::JoinHandle<()>,
}

pub struct SyncServiceHandler {
    service: Arc<Mutex<Service>>,
    trigger: SyncWorkerTrigger,
}

impl SyncServiceHandler {
    #[must_use]
    pub const fn new(service: Arc<Mutex<Service>>, trigger: SyncWorkerTrigger) -> Self {
        Self { service, trigger }
    }
}

impl Handler for SyncServiceHandler {
    fn handle(&mut self, request: Request) -> Response {
        let schedule =
            request.command.mutates() && !matches!(request.command, Command::SyncDisable);
        let rebuild = matches!(request.command, Command::SyncRebuild);
        let response = match self.service.lock() {
            Ok(mut service) => service.handle(request),
            Err(error) => Response::Error {
                error: pomotui_protocol::ProtocolError::Rejected {
                    message: format!("Timer Service state poisoned: {error}"),
                },
            },
        };
        if schedule && !matches!(response, Response::Error { .. }) {
            if rebuild {
                self.trigger.request_rebuild();
            } else {
                self.trigger.request();
            }
        }
        response
    }
}

impl SyncWorker {
    /// Starts the named, single-flight synchronization worker.
    ///
    /// # Errors
    ///
    /// Returns an error if the operating system cannot spawn the worker thread.
    pub fn start(service: Arc<Mutex<Service>>, interval: Duration) -> Result<Self, String> {
        Self::start_with_file_adapter(service, interval, Arc::new(PlatformSyncFile))
    }

    /// Starts a worker with a supplied filesystem adapter.
    ///
    /// # Errors
    ///
    /// Returns an error if the operating system cannot spawn the worker thread.
    pub fn start_with_file_adapter(
        service: Arc<Mutex<Service>>,
        interval: Duration,
        file: Arc<dyn SyncFileAdapter>,
    ) -> Result<Self, String> {
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        let rebuild_requested = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let trigger = SyncWorkerTrigger {
            sender,
            rebuild_requested: Arc::clone(&rebuild_requested),
        };
        let thread = std::thread::Builder::new()
            .name("pomotui-sync-worker".into())
            .spawn(move || worker_loop(service, receiver, rebuild_requested, interval, file))
            .map_err(|error| format!("cannot start synchronization worker: {error}"))?;
        trigger.request();
        Ok(Self { trigger, thread })
    }

    #[must_use]
    pub fn trigger(&self) -> SyncWorkerTrigger {
        self.trigger.clone()
    }

    pub fn shutdown(self) {
        drop(self.trigger);
        let _ = self.thread.join();
    }
}

#[allow(clippy::needless_pass_by_value)]
fn worker_loop(
    service: Arc<Mutex<Service>>,
    receiver: std::sync::mpsc::Receiver<()>,
    rebuild_requested: Arc<std::sync::atomic::AtomicBool>,
    interval: Duration,
    file: Arc<dyn SyncFileAdapter>,
) {
    let mut next_interval = std::time::Instant::now() + interval;
    loop {
        let wait = next_interval.saturating_duration_since(std::time::Instant::now());
        match receiver.recv_timeout(wait) {
            Ok(()) | Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return,
        }
        if std::time::Instant::now() >= next_interval {
            next_interval = std::time::Instant::now() + interval;
        }
        while receiver.try_recv().is_ok() {}
        let rebuild = rebuild_requested.swap(false, std::sync::atomic::Ordering::AcqRel);
        run_sync_attempt(&service, file.as_ref(), rebuild);
    }
}

#[allow(clippy::too_many_lines, clippy::needless_continue)]
fn run_sync_attempt(service: &Arc<Mutex<Service>>, file: &dyn SyncFileAdapter, rebuild: bool) {
    let Some(work) = service
        .lock()
        .ok()
        .and_then(|mut service| service.begin_sync_work().ok())
    else {
        return;
    };
    if rebuild {
        let retained = match service.lock() {
            Ok(guard) => match guard.sync_document_for(&work.path) {
                Ok(document) => document,
                Err(error) => {
                    drop(guard);
                    finish_failure(service, &work.path, "rebuild", error);
                    return;
                }
            },
            Err(_) => return,
        };
        let result = retained
            .to_json()
            .and_then(|document| file.replace(&work.path, &document));
        match result {
            Ok(()) => finish_success(
                service,
                &work.path,
                retained.records().len(),
                Some(
                    "rebuilt from locally known records; unseen remote records cannot be recovered"
                        .into(),
                ),
            ),
            Err(error) => finish_failure(service, &work.path, "rebuild", error),
        }
        return;
    }

    for attempt in 1..=3 {
        let source = match file.read(&work.path) {
            Ok(source) => source,
            Err(error) => {
                finish_failure(service, &work.path, "read", error);
                return;
            }
        };
        let Some(source_document) = source.as_deref() else {
            finish_failure(
                service,
                &work.path,
                "read",
                "sync file does not exist; use `pomotui sync rebuild` to create it from local records"
                    .into(),
            );
            return;
        };
        let mut incoming = match SyncDocument::from_json(source_document) {
            Ok(document) => document,
            Err(error) => {
                finish_failure(service, &work.path, "validate", error);
                return;
            }
        };
        let candidates = match file.discover_conflicts(&work.path) {
            Ok(candidates) => candidates,
            Err(error) => {
                finish_failure(service, &work.path, "discover", error);
                return;
            }
        };
        let mut observations = Vec::new();
        for candidate in &candidates {
            let observed = match file.observe_candidate(candidate) {
                Ok(Some(observed)) => observed,
                Ok(None) => continue,
                Err(error) => {
                    finish_failure(service, &work.path, "candidate-read", error);
                    return;
                }
            };
            let records = SyncDocument::from_json(&observed.source);
            observations.push(observed);

            let records = match records {
                Ok(records) => records,
                Err(error) => {
                    finish_failure(
                        service,
                        &work.path,
                        "candidate-validate",
                        format!("{}: {error}", candidate.display()),
                    );
                    return;
                }
            };
            incoming = match pomotui_sync::union_documents(&incoming, &records) {
                Ok(records) => records,
                Err(error) => {
                    finish_failure(
                        service,
                        &work.path,
                        "candidate-integrity",
                        format!("{}: {error}", candidate.display()),
                    );
                    return;
                }
            };
        }
        let retained = match service.lock() {
            Ok(mut service) => match service.apply_sync_document(&work.path, &incoming) {
                Ok(records) => records,
                Err(error) => {
                    service.finish_sync_failure(&work.path, "import", error);
                    return;
                }
            },
            Err(error) => {
                eprintln!("sync worker cannot lock Timer Service: {error}");
                return;
            }
        };
        let document = match retained.to_json() {
            Ok(document) => document,
            Err(error) => {
                finish_failure(service, &work.path, "serialize", error);
                return;
            }
        };
        match file.read(&work.path) {
            Ok(current) if current == source => {
                if source.as_deref() == Some(document.as_str()) {
                    if !observations.is_empty()
                        && let Err(error) = file.confirm_publication(&work.path, &document)
                    {
                        finish_failure(service, &work.path, "publication", error);
                        return;
                    }
                    finish_cleanup(
                        service,
                        file,
                        &work.path,
                        &observations,
                        retained.records().len(),
                    );
                    return;
                }
                match file.replace(&work.path, &document) {
                    Ok(()) => {
                        finish_cleanup(
                            service,
                            file,
                            &work.path,
                            &observations,
                            retained.records().len(),
                        );
                    }
                    Err(error) => finish_failure(service, &work.path, "replace", error),
                }
                return;
            }
            Ok(_) if attempt < 3 => continue,
            Ok(_) => {
                finish_failure(
                    service,
                    &work.path,
                    "compare",
                    "sync file changed during three consecutive attempts; a later trigger will retry"
                        .into(),
                );
                return;
            }
            Err(error) => {
                finish_failure(service, &work.path, "compare", error);
                return;
            }
        }
    }
}

fn finish_cleanup(
    service: &Arc<Mutex<Service>>,
    file: &dyn SyncFileAdapter,
    main: &Path,
    observations: &[pomotui_platform::SyncCandidate],
    records: usize,
) {
    for observation in observations {
        if let Err(error) = file.cleanup_candidate(main, observation) {
            finish_failure(service, main, "cleanup", error);
            return;
        }
    }
    finish_success(service, main, records, None);
}

fn finish_success(
    service: &Arc<Mutex<Service>>,
    path: &Path,
    records: usize,
    warning: Option<String>,
) {
    if let Ok(mut service) = service.lock() {
        service.finish_sync_success(path, records, warning);
    }
}

fn finish_failure(service: &Arc<Mutex<Service>>, path: &Path, stage: &str, error: String) {
    if let Ok(mut service) = service.lock() {
        service.finish_sync_failure(path, stage, error);
    }
}
