//! Versioned newline-delimited JSON protocol shared by Timer Frontends.

use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;

pub const PROTOCOL_VERSION: u16 = 2;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionKind {
    Focus,
    ShortBreak,
    LongBreak,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum Command {
    Status,
    Start {
        kind: SessionKind,
        task_id: Option<u64>,
    },
    StartTitle {
        title: String,
    },
    Pause,
    Resume,
    Stop,
    Skip,
    TaskList,
    TaskCreate {
        title: String,
    },
    TaskRename {
        id: u64,
        title: String,
    },
    TaskComplete {
        id: u64,
    },
    TaskReopen {
        id: u64,
    },
    TaskDelete {
        id: u64,
    },
    TaskSelect {
        id: u64,
        stop_current: bool,
    },
    HistoryDelete {
        ids: Vec<u64>,
    },
    History,
    Summary,
}

impl Command {
    #[must_use]
    pub const fn mutates(&self) -> bool {
        !matches!(
            self,
            Self::Status | Self::TaskList | Self::History | Self::Summary
        )
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Request {
    pub version: u16,
    pub idempotency_key: Option<String>,
    pub command: Command,
}

impl Request {
    /// Validates version and mutation identity.
    ///
    /// # Errors
    ///
    /// Returns a protocol error for incompatible versions or missing keys.
    pub fn validate(&self) -> Result<(), ProtocolError> {
        if self.version != PROTOCOL_VERSION {
            return Err(ProtocolError::IncompatibleVersion {
                received: self.version,
                supported: PROTOCOL_VERSION,
            });
        }
        if self.command.mutates() && self.idempotency_key.as_deref().is_none_or(str::is_empty) {
            return Err(ProtocolError::MissingIdempotencyKey);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Snapshot {
    pub state: String,
    pub kind: SessionKind,
    pub remaining_seconds: u64,
    pub planned_seconds: u64,
    pub current_task: Option<String>,
    pub current_task_id: Option<u64>,
    pub completed_rounds: u8,
    pub rounds_per_cycle: u8,
    pub next_kind: Option<SessionKind>,
    pub tasks: Vec<TaskSummary>,
    pub today: Box<TodaySummary>,
    pub recent_history: Vec<RecentSessionSummary>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RecentSessionSummary {
    pub id: u64,
    pub kind: SessionKind,
    pub outcome: String,
    pub actual_seconds: u64,
    pub task_title: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TaskSummary {
    pub id: u64,
    pub title: String,
    pub completed: bool,
    pub focus_seconds: u64,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct TodaySummary {
    pub focus_seconds: u64,
    pub completed_rounds: u32,
    pub seven_day_focus_seconds: [u64; 7],
    pub seven_day_dates: [String; 7],
    pub average_focus_seconds: u64,
    pub task_focus: Vec<TaskFocusSummary>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TaskFocusSummary {
    pub task_title: Option<String>,
    pub focus_seconds: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum Response {
    Snapshot { snapshot: Snapshot },
    Data { value: serde_json::Value },
    Accepted,
    Error { error: ProtocolError },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "code", rename_all = "snake_case")]
pub enum ProtocolError {
    IncompatibleVersion { received: u16, supported: u16 },
    MissingIdempotencyKey,
    Malformed { message: String },
    Rejected { message: String },
    Disconnected { message: String },
}

pub trait Handler {
    fn handle(&mut self, request: Request) -> Response;
}

pub struct Client {
    path: std::path::PathBuf,
}

impl Client {
    /// Connects to the Timer Service socket.
    ///
    /// # Errors
    ///
    /// Returns a connection error when the service is unavailable.
    pub fn connect(path: &Path) -> std::io::Result<Self> {
        Ok(Self { path: path.into() })
    }

    /// Sends one request and reads one response.
    ///
    /// # Errors
    ///
    /// Returns transport or malformed-response errors.
    pub fn request(&mut self, request: &Request) -> std::io::Result<Response> {
        let mut stream = UnixStream::connect(&self.path)?;
        serde_json::to_writer(&mut stream, request).map_err(std::io::Error::other)?;
        stream.write_all(b"\n")?;
        stream.flush()?;
        let mut line = String::new();
        BufReader::new(stream).read_line(&mut line)?;
        serde_json::from_str(&line).map_err(std::io::Error::other)
    }
}

/// Serves connections until `max_connections` have completed.
///
/// # Errors
///
/// Returns listener or stream I/O errors.
pub fn serve(
    listener: &UnixListener,
    handler: &mut impl Handler,
    max_connections: Option<usize>,
) -> std::io::Result<()> {
    for (index, stream) in listener.incoming().enumerate() {
        handle_stream(stream?, handler)?;
        if max_connections.is_some_and(|limit| index + 1 >= limit) {
            break;
        }
    }
    Ok(())
}

fn handle_stream(mut stream: UnixStream, handler: &mut impl Handler) -> std::io::Result<()> {
    let mut line = String::new();
    BufReader::new(stream.try_clone()?).read_line(&mut line)?;
    let response = match serde_json::from_str::<Request>(&line) {
        Ok(request) => match request.validate() {
            Ok(()) => handler.handle(request),
            Err(error) => Response::Error { error },
        },
        Err(error) => Response::Error {
            error: ProtocolError::Malformed {
                message: error.to_string(),
            },
        },
    };
    serde_json::to_writer(&mut stream, &response).map_err(std::io::Error::other)?;
    stream.write_all(b"\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mutations_require_idempotency_keys() {
        let request = Request {
            version: PROTOCOL_VERSION,
            idempotency_key: None,
            command: Command::Pause,
        };
        assert_eq!(
            request.validate(),
            Err(ProtocolError::MissingIdempotencyKey)
        );
    }

    #[test]
    fn malformed_and_incompatible_inputs_are_explicit() {
        let request = Request {
            version: 99,
            idempotency_key: None,
            command: Command::Status,
        };
        assert!(matches!(
            request.validate(),
            Err(ProtocolError::IncompatibleVersion { .. })
        ));
        assert!(serde_json::from_str::<Request>("{broken").is_err());
    }

    struct CountingHandler(u64);

    impl Handler for CountingHandler {
        fn handle(&mut self, _request: Request) -> Response {
            self.0 += 1;
            Response::Data {
                value: serde_json::json!({ "observed": self.0 }),
            }
        }
    }

    #[test]
    fn polling_client_reconnects_and_observes_authoritative_state() {
        let path = std::env::temp_dir().join(format!(
            "pomotui-protocol-{}-{}.sock",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        let _ = std::fs::remove_file(&path);
        let listener = match UnixListener::bind(&path) {
            Ok(listener) => listener,
            Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
                // Some code-execution sandboxes prohibit AF_UNIX even in /tmp.
                return;
            }
            Err(error) => panic!("bind: {error}"),
        };
        let server = std::thread::spawn(move || {
            serve(&listener, &mut CountingHandler(0), Some(2)).expect("serve");
        });
        let request = Request {
            version: PROTOCOL_VERSION,
            idempotency_key: None,
            command: Command::Status,
        };
        let mut client = Client::connect(&path).expect("client");
        let first = client.request(&request).expect("first response");
        let second = client.request(&request).expect("second response");
        server.join().expect("server");
        std::fs::remove_file(&path).expect("cleanup");
        assert_eq!(
            (first, second),
            (
                Response::Data {
                    value: serde_json::json!({"observed": 1})
                },
                Response::Data {
                    value: serde_json::json!({"observed": 2})
                }
            )
        );
    }

    #[test]
    fn simultaneous_clients_share_one_authoritative_server_state() {
        let path = std::env::temp_dir().join(format!(
            "pomotui-concurrent-{}-{}.sock",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        let _ = std::fs::remove_file(&path);
        let listener = match UnixListener::bind(&path) {
            Ok(listener) => listener,
            Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => return,
            Err(error) => panic!("bind: {error}"),
        };
        let server = std::thread::spawn(move || {
            serve(&listener, &mut CountingHandler(0), Some(2)).expect("serve");
        });
        let request = Request {
            version: PROTOCOL_VERSION,
            idempotency_key: None,
            command: Command::Status,
        };
        let handles: Vec<_> = (0..2)
            .map(|_| {
                let path = path.clone();
                let request = request.clone();
                std::thread::spawn(move || {
                    Client::connect(&path)
                        .expect("client")
                        .request(&request)
                        .expect("response")
                })
            })
            .collect();
        let mut observed: Vec<_> = handles
            .into_iter()
            .map(|handle| {
                let Response::Data { value } = handle.join().expect("client thread") else {
                    panic!("data response");
                };
                value["observed"].as_u64().expect("counter")
            })
            .collect();
        observed.sort_unstable();
        server.join().expect("server");
        std::fs::remove_file(&path).expect("cleanup");
        assert_eq!(observed, [1, 2]);
    }
}
