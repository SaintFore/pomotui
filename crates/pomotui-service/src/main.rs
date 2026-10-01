use pomotui_protocol::{Command, Handler, Request, Response, serve};
use pomotui_service::Service;
use pomotui_sync::Document as SyncDocument;
use serde::Deserialize;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut listenfd = listenfd::ListenFd::from_env();
    let listener = if let Some(listener) = listenfd.take_unix_listener(0)? {
        listener
    } else {
        let path = socket_path();
        if path.exists() {
            std::fs::remove_file(&path)?;
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let listener = UnixListener::bind(&path)?;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
        listener
    };
    let data_home = std::env::var_os("XDG_DATA_HOME").map_or_else(
        || {
            std::env::var_os("HOME").map_or_else(
                || std::path::PathBuf::from(".local/share"),
                |home| std::path::PathBuf::from(home).join(".local/share"),
            )
        },
        std::path::PathBuf::from,
    );
    let data_dir = data_home.join("pomotui");
    std::fs::create_dir_all(&data_dir)?;
    eprintln!("Timer Service opening {}", data_dir.display());
    let mut service = Service::open(&data_dir.join("pomotui.sqlite3"))?;
    if let Some(settings) = load_settings()? {
        if settings.volume > 100 {
            return Err("volume must be between 0 and 100".into());
        }
        service.configure_durations(pomotui_domain::SessionDurations::new(
            u64::from(settings.focus) * 60,
            u64::from(settings.short_break) * 60,
            u64::from(settings.long_break) * 60,
        )?)?;
        service.configure_cycle(settings.rounds_per_cycle)?;
        let sound = settings.sound.map(|sound| {
            if sound == "builtin:complete" {
                std::path::PathBuf::from("/usr/share/sounds/freedesktop/stereo/complete.oga")
            } else {
                std::path::PathBuf::from(sound)
            }
        });
        service.configure_reminder(settings.reminder_enabled, sound, settings.volume);
    }
    service.enable_background_sync();
    let service = std::sync::Arc::new(std::sync::Mutex::new(service));
    let (sync_sender, sync_receiver) = std::sync::mpsc::sync_channel(1);
    let rebuild_requested = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let sync_service = std::sync::Arc::clone(&service);
    let sync_rebuild = std::sync::Arc::clone(&rebuild_requested);
    let sync_worker = std::thread::Builder::new()
        .name("pomotui-sync-worker".into())
        .spawn(move || sync_worker(sync_service, sync_receiver, sync_rebuild))?;
    let _ = sync_sender.try_send(());
    eprintln!("Timer Service ready");
    let ticker = std::sync::Arc::clone(&service);
    std::thread::Builder::new()
        .name("pomotui-deadline-ticker".into())
        .spawn(move || {
            loop {
                std::thread::sleep(std::time::Duration::from_millis(250));
                if let Ok(mut service) = ticker.lock() {
                    service.tick();
                }
            }
        })?;
    let mut handler = SharedService {
        service,
        sync_sender: sync_sender.clone(),
        rebuild_requested,
    };
    let result = serve(&listener, &mut handler, None);
    drop(handler);
    drop(sync_sender);
    let _ = sync_worker.join();
    result?;
    Ok(())
}

fn socket_path() -> std::path::PathBuf {
    std::env::var_os("POMOTUI_SOCKET").map_or_else(
        || {
            std::env::var_os("XDG_RUNTIME_DIR")
                .map_or_else(
                    || std::path::PathBuf::from("/tmp/pomotui-runtime"),
                    std::path::PathBuf::from,
                )
                .join("pomotui/pomotui.sock")
        },
        std::path::PathBuf::from,
    )
}

struct SharedService {
    service: std::sync::Arc<std::sync::Mutex<Service>>,
    sync_sender: std::sync::mpsc::SyncSender<()>,
    rebuild_requested: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl Handler for SharedService {
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
                self.rebuild_requested
                    .store(true, std::sync::atomic::Ordering::Release);
            }
            let _ = self.sync_sender.try_send(());
        }
        response
    }
}

#[allow(clippy::needless_pass_by_value)]
fn sync_worker(
    service: std::sync::Arc<std::sync::Mutex<Service>>,
    receiver: std::sync::mpsc::Receiver<()>,
    rebuild_requested: std::sync::Arc<std::sync::atomic::AtomicBool>,
) {
    let mut next_interval = std::time::Instant::now() + std::time::Duration::from_secs(30);
    loop {
        let wait = next_interval.saturating_duration_since(std::time::Instant::now());
        match receiver.recv_timeout(wait) {
            Ok(()) | Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return,
        }
        if std::time::Instant::now() >= next_interval {
            next_interval = std::time::Instant::now() + std::time::Duration::from_secs(30);
        }
        while receiver.try_recv().is_ok() {}
        let rebuild = rebuild_requested.swap(false, std::sync::atomic::Ordering::AcqRel);
        run_sync_attempt(&service, rebuild);
    }
}

#[allow(clippy::too_many_lines, clippy::needless_continue)]
fn run_sync_attempt(service: &std::sync::Arc<std::sync::Mutex<Service>>, rebuild: bool) {
    let Some(work) = service
        .lock()
        .ok()
        .and_then(|mut service| service.begin_sync_work().ok())
    else {
        return;
    };
    if rebuild {
        let retained = match service.lock() {
            Ok(mut service) => match service.apply_sync_import(&work.path, &[], 0) {
                Ok(records) => records,
                Err(error) => {
                    service.finish_sync_failure(&work.path, "rebuild", error);
                    return;
                }
            },
            Err(error) => {
                eprintln!("sync worker cannot lock Timer Service: {error}");
                return;
            }
        };
        let result = SyncDocument::new(&retained)
            .and_then(|document| document.to_json())
            .and_then(|document| pomotui_platform::replace_sync_file(&work.path, &document));
        match result {
            Ok(()) => finish_success(
                service,
                &work.path,
                retained.len(),
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
        let source = match pomotui_platform::read_sync_file(&work.path) {
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
        let incoming = match SyncDocument::from_json(source_document) {
            Ok(document) => document.into_records(),
            Err(error) => {
                finish_failure(service, &work.path, "validate", error);
                return;
            }
        };
        let retained = match service.lock() {
            Ok(mut service) => {
                match service.apply_sync_import(&work.path, &incoming, incoming.len()) {
                    Ok(records) => records,
                    Err(error) => {
                        service.finish_sync_failure(&work.path, "import", error);
                        return;
                    }
                }
            }
            Err(error) => {
                eprintln!("sync worker cannot lock Timer Service: {error}");
                return;
            }
        };
        let document = match SyncDocument::new(&retained).and_then(|document| document.to_json()) {
            Ok(document) => document,
            Err(error) => {
                finish_failure(service, &work.path, "serialize", error);
                return;
            }
        };
        match pomotui_platform::read_sync_file(&work.path) {
            Ok(current) if current == source => {
                match pomotui_platform::replace_sync_file(&work.path, &document) {
                    Ok(()) => finish_success(service, &work.path, retained.len(), None),
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

fn finish_success(
    service: &std::sync::Arc<std::sync::Mutex<Service>>,
    path: &std::path::Path,
    records: usize,
    warning: Option<String>,
) {
    if let Ok(mut service) = service.lock() {
        service.finish_sync_success(path, records, warning);
    }
}

fn finish_failure(
    service: &std::sync::Arc<std::sync::Mutex<Service>>,
    path: &std::path::Path,
    stage: &str,
    error: String,
) {
    if let Ok(mut service) = service.lock() {
        service.finish_sync_failure(path, stage, error);
    }
}

#[derive(Deserialize)]
#[serde(default)]
struct ServiceSettings {
    #[serde(rename = "focus_minutes")]
    focus: u16,
    #[serde(rename = "short_break_minutes")]
    short_break: u16,
    #[serde(rename = "long_break_minutes")]
    long_break: u16,
    rounds_per_cycle: u8,
    reminder_enabled: bool,
    sound: Option<String>,
    volume: u8,
}

impl Default for ServiceSettings {
    fn default() -> Self {
        Self {
            focus: 25,
            short_break: 5,
            long_break: 15,
            rounds_per_cycle: 4,
            reminder_enabled: true,
            sound: None,
            volume: 100,
        }
    }
}

fn load_settings() -> Result<Option<ServiceSettings>, Box<dyn std::error::Error>> {
    let config_home = std::env::var_os("XDG_CONFIG_HOME").map_or_else(
        || {
            std::env::var_os("HOME").map_or_else(
                || std::path::PathBuf::from(".config"),
                |home| std::path::PathBuf::from(home).join(".config"),
            )
        },
        std::path::PathBuf::from,
    );
    let path = config_home.join("pomotui/config.toml");
    match std::fs::read_to_string(path) {
        Ok(source) => Ok(Some(toml::from_str(&source)?)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}
