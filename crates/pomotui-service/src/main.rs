use pomotui_protocol::{Handler, Request, Response, serve};
use pomotui_service::Service;
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
    let service = std::sync::Arc::new(std::sync::Mutex::new(service));
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
    serve(&listener, &mut SharedService(service), None)?;
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

struct SharedService(std::sync::Arc<std::sync::Mutex<Service>>);

impl Handler for SharedService {
    fn handle(&mut self, request: Request) -> Response {
        match self.0.lock() {
            Ok(mut service) => service.handle(request),
            Err(error) => Response::Error {
                error: pomotui_protocol::ProtocolError::Rejected {
                    message: format!("Timer Service state poisoned: {error}"),
                },
            },
        }
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
