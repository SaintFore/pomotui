//! Versioned newline-delimited JSON protocol shared by Timer Frontends.

use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::time::Duration;

pub const PROTOCOL_VERSION: u16 = 3;
pub const MAX_REQUEST_FRAME_BYTES: usize = 64 * 1024;
const REQUEST_READ_TIMEOUT: Duration = Duration::from_secs(2);
const CONNECTION_WORKERS: usize = 8;
const PENDING_CONNECTIONS: usize = 16;

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
    ReviewSuccess {
        reflection: Option<String>,
    },
    ActionChainCurrent,
}

impl Command {
    #[must_use]
    pub const fn mutates(&self) -> bool {
        !matches!(
            self,
            Self::Status
                | Self::TaskList
                | Self::History
                | Self::Summary
                | Self::ActionChainCurrent
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
    pub durable_health: DurableHealth,
    pub reminder_delivery: ReminderDelivery,
    pub tasks: Vec<TaskSummary>,
    pub today: Box<TodaySummary>,
    pub recent_history: Vec<RecentSessionSummary>,
    pub action_chain: ActionChainSummary,
    pub pending_review: Option<PendingReviewSummary>,
    pub recent_chain_links: Vec<ChainLinkSummary>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct ActionChainSummary {
    pub id: u64,
    pub length: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PendingReviewSummary {
    pub session_id: u64,
    pub actual_seconds: u64,
    pub task_id: Option<u64>,
    pub task_title: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ChainLinkSummary {
    pub id: u64,
    pub task_title: String,
    pub actual_seconds: u64,
    pub reflection: Option<String>,
    pub chain_entry_title: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct ReminderDelivery {
    pub pending: u32,
    pub retrying: u32,
    pub delivered: u32,
    pub exhausted: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DurableHealth {
    pub state: DurableHealthState,
    pub last_successful_commit: Option<i64>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DurableHealthState {
    Healthy,
    Degraded,
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
#[allow(clippy::large_enum_variant)]
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
    RequestTooLarge { max_bytes: usize },
    RequestTimeout,
    ServerBusy,
    InvalidTaskTitle { rule: TaskTitleRule },
    DurableWriteUnavailable { message: String },
    Malformed { message: String },
    Rejected { message: String },
    Disconnected { message: String },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskTitleRule {
    Empty,
    UnsafeCharacter,
    TooLong,
    TooWide,
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
    handler: &mut (impl Handler + Send),
    max_connections: Option<usize>,
) -> std::io::Result<()> {
    serve_with_capacity(
        listener,
        handler,
        max_connections,
        CONNECTION_WORKERS,
        PENDING_CONNECTIONS,
    )
}

fn serve_with_capacity(
    listener: &UnixListener,
    handler: &mut (impl Handler + Send),
    max_connections: Option<usize>,
    connection_workers: usize,
    pending_connections: usize,
) -> std::io::Result<()> {
    let handler = std::sync::Mutex::new(handler);
    std::thread::scope(|scope| {
        let (sender, receiver) = std::sync::mpsc::sync_channel::<UnixStream>(pending_connections);
        let receiver = std::sync::Arc::new(std::sync::Mutex::new(receiver));

        for _ in 0..connection_workers {
            let receiver = std::sync::Arc::clone(&receiver);
            let handler = &handler;
            scope.spawn(move || {
                loop {
                    let stream = {
                        let Ok(receiver) = receiver.lock() else {
                            return;
                        };
                        receiver.recv()
                    };
                    let Ok(stream) = stream else {
                        return;
                    };
                    let _ = handle_stream(stream, handler);
                }
            });
        }

        for (index, stream) in listener.incoming().enumerate() {
            let stream = stream?;
            match sender.try_send(stream) {
                Ok(()) => {}
                Err(std::sync::mpsc::TrySendError::Full(mut stream)) => {
                    write_response(
                        &mut stream,
                        &Response::Error {
                            error: ProtocolError::ServerBusy,
                        },
                    )?;
                }
                Err(std::sync::mpsc::TrySendError::Disconnected(_)) => {
                    return Err(std::io::Error::other(
                        "Timer Service connection workers stopped",
                    ));
                }
            }
            if max_connections.is_some_and(|limit| index + 1 >= limit) {
                break;
            }
        }
        drop(sender);
        Ok(())
    })
}

fn handle_stream(
    mut stream: UnixStream,
    handler: &std::sync::Mutex<&mut (impl Handler + Send)>,
) -> std::io::Result<()> {
    stream.set_read_timeout(Some(REQUEST_READ_TIMEOUT))?;
    let frame = match read_request_frame(&stream) {
        Ok(frame) => frame,
        Err(error) => return write_response(&mut stream, &Response::Error { error }),
    };
    let response = match serde_json::from_slice::<Request>(&frame) {
        Ok(request) => match request.validate() {
            Ok(()) => match handler.lock() {
                Ok(mut handler) => handler.handle(request),
                Err(error) => Response::Error {
                    error: ProtocolError::Rejected {
                        message: format!("Timer Service state poisoned: {error}"),
                    },
                },
            },
            Err(error) => Response::Error { error },
        },
        Err(error) => Response::Error {
            error: ProtocolError::Malformed {
                message: error.to_string(),
            },
        },
    };
    write_response(&mut stream, &response)
}

fn read_request_frame(stream: &UnixStream) -> Result<Vec<u8>, ProtocolError> {
    let mut frame = Vec::with_capacity(MAX_REQUEST_FRAME_BYTES.min(4096));
    let mut reader = BufReader::new(stream);
    match reader
        .by_ref()
        .take((MAX_REQUEST_FRAME_BYTES + 1) as u64)
        .read_until(b'\n', &mut frame)
    {
        Ok(_) if frame.len() > MAX_REQUEST_FRAME_BYTES => Err(ProtocolError::RequestTooLarge {
            max_bytes: MAX_REQUEST_FRAME_BYTES,
        }),
        Ok(0) => Err(ProtocolError::Malformed {
            message: "empty request".into(),
        }),
        Ok(_) if frame.last() != Some(&b'\n') => Err(ProtocolError::Malformed {
            message: "request must end with a newline".into(),
        }),
        Ok(_) => Ok(frame),
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
            ) =>
        {
            Err(ProtocolError::RequestTimeout)
        }
        Err(error) => Err(ProtocolError::Malformed {
            message: error.to_string(),
        }),
    }
}

fn write_response(stream: &mut UnixStream, response: &Response) -> std::io::Result<()> {
    serde_json::to_writer(&mut *stream, response).map_err(std::io::Error::other)?;
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

    fn connected_streams(label: &str) -> Option<(UnixStream, UnixStream)> {
        static NEXT_PAIR: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(1);
        let sequence = NEXT_PAIR.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "pomotui-{label}-{}-{sequence}.sock",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        let listener = match UnixListener::bind(&path) {
            Ok(listener) => listener,
            Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => return None,
            Err(error) => panic!("bind: {error}"),
        };
        let client = UnixStream::connect(&path).expect("connect stream pair");
        let (server, _) = listener.accept().expect("accept stream pair");
        std::fs::remove_file(path).expect("cleanup stream pair");
        Some((client, server))
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

    #[test]
    fn partial_client_does_not_block_a_valid_request() {
        let path = std::env::temp_dir().join(format!(
            "pomotui-partial-{}-{}.sock",
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
            let _ = serve(&listener, &mut CountingHandler(0), Some(2));
        });

        let mut partial = UnixStream::connect(&path).expect("partial client");
        partial
            .write_all(br#"{"version":"#)
            .expect("write partial request");
        std::thread::sleep(std::time::Duration::from_millis(20));

        let mut valid = UnixStream::connect(&path).expect("valid client");
        valid
            .set_read_timeout(Some(std::time::Duration::from_millis(250)))
            .expect("read timeout");
        serde_json::to_writer(
            &mut valid,
            &Request {
                version: PROTOCOL_VERSION,
                idempotency_key: None,
                command: Command::Status,
            },
        )
        .expect("valid request");
        valid.write_all(b"\n").expect("request terminator");
        let mut response = String::new();
        let result = BufReader::new(valid).read_line(&mut response);

        drop(partial);
        server.join().expect("server");
        std::fs::remove_file(&path).expect("cleanup");
        assert!(
            result.is_ok(),
            "a partial client blocked a valid request: {result:?}"
        );
    }

    #[test]
    fn request_frames_are_bounded_and_require_a_terminator() {
        let exact = vec![b'x'; MAX_REQUEST_FRAME_BYTES - 1];
        let mut exact_with_newline = exact;
        exact_with_newline.push(b'\n');
        let Some((mut writer, reader)) = connected_streams("exact-frame") else {
            return;
        };
        let write = std::thread::spawn(move || {
            writer.write_all(&exact_with_newline).expect("exact frame");
        });
        assert_eq!(
            read_request_frame(&reader).expect("frame at limit").len(),
            MAX_REQUEST_FRAME_BYTES
        );
        write.join().expect("writer");

        let oversized = vec![b'x'; MAX_REQUEST_FRAME_BYTES + 1];
        let Some((mut writer, reader)) = connected_streams("oversized-frame") else {
            return;
        };
        let write = std::thread::spawn(move || {
            writer.write_all(&oversized).expect("oversized frame");
        });
        assert_eq!(
            read_request_frame(&reader),
            Err(ProtocolError::RequestTooLarge {
                max_bytes: MAX_REQUEST_FRAME_BYTES
            })
        );
        write.join().expect("writer");

        let Some((mut writer, reader)) = connected_streams("unterminated-frame") else {
            return;
        };
        writer.write_all(b"{}").expect("unterminated frame");
        writer
            .shutdown(std::net::Shutdown::Write)
            .expect("close writer");
        assert!(matches!(
            read_request_frame(&reader),
            Err(ProtocolError::Malformed { .. })
        ));
    }

    #[test]
    fn stalled_frame_hits_the_configured_read_deadline() {
        let Some((_writer, reader)) = connected_streams("read-deadline") else {
            return;
        };
        reader
            .set_read_timeout(Some(Duration::from_millis(20)))
            .expect("read timeout");
        assert_eq!(
            read_request_frame(&reader),
            Err(ProtocolError::RequestTimeout)
        );
    }

    #[test]
    fn connection_pressure_has_a_deterministic_busy_response() {
        let path = std::env::temp_dir().join(format!(
            "pomotui-overload-{}-{}.sock",
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
            let _ = serve_with_capacity(&listener, &mut CountingHandler(0), Some(3), 1, 1);
        });

        let mut active = UnixStream::connect(&path).expect("active client");
        active.write_all(b"{").expect("partial active frame");
        std::thread::sleep(Duration::from_millis(20));
        let mut queued = UnixStream::connect(&path).expect("queued client");
        queued.write_all(b"{").expect("partial queued frame");
        std::thread::sleep(Duration::from_millis(20));
        let busy = UnixStream::connect(&path).expect("busy client");
        busy.set_read_timeout(Some(Duration::from_millis(250)))
            .expect("busy read timeout");
        let response: Response =
            serde_json::from_reader(BufReader::new(busy)).expect("busy response");

        drop(active);
        drop(queued);
        server.join().expect("server");
        std::fs::remove_file(&path).expect("cleanup");
        assert_eq!(
            response,
            Response::Error {
                error: ProtocolError::ServerBusy
            }
        );
    }
}
