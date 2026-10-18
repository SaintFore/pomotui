//! Real CLI against a protocol boundary fixture; no wall-clock expiry waits.
use pomotui_protocol::{Command, Handler, Request, Response, Snapshot};
use std::os::unix::net::UnixListener;
use std::process::Command as Process;

struct SnapshotFixture(Snapshot);
impl Handler for SnapshotFixture {
    fn handle(&mut self, request: Request) -> Response {
        assert_eq!(request.command, Command::Status);
        assert!(request.idempotency_key.is_none());
        Response::Snapshot {
            snapshot: self.0.clone(),
        }
    }
}

#[test]
fn waybar_cli_renders_pending_judgment_without_hiding_running_break() {
    let root = std::env::temp_dir().join(format!("pomotui-waybar-process-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let socket = root.join("service.sock");
    let listener = UnixListener::bind(&socket).unwrap();
    let snapshot: Snapshot = serde_json::from_value(serde_json::json!({
        "state": "running", "kind": "long_break", "remaining_seconds": 900,
        "planned_seconds": 1200, "current_task": null, "current_task_id": null,
        "completed_rounds": 4, "rounds_per_cycle": 4, "next_kind": "focus",
        "durable_health": {"state":"healthy", "last_successful_commit": null,"error":null},
        "reminder_delivery": {"pending":0,"retrying":0,"delivered":0,"exhausted":0},
        "reservation": {"id":"reservation-fixture", "state":"awaiting_review", "task_id":null,
            "task_title":null,"duration_seconds":600,"remaining_seconds":0},
        "tasks":[], "today":pomotui_protocol::TodaySummary::default(), "recent_history":[],
        "action_chain":{"id":1,"length":20}, "pending_review":null,
        "recent_chain_links":[],"recent_ended_chains":[],"next_reward":null,"current_chain_rewards":[]
    })).unwrap();
    let server = std::thread::spawn(move || {
        pomotui_protocol::serve(&listener, &mut SnapshotFixture(snapshot), Some(1)).unwrap();
    });
    let output = Process::new(env!("CARGO_BIN_EXE_pomotui"))
        .arg("waybar")
        .env("POMOTUI_SOCKET", &socket)
        .output()
        .unwrap();
    server.join().unwrap();
    std::fs::remove_dir_all(root).unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["text"], "reservation awaiting review");
    assert_eq!(
        value["class"],
        serde_json::json!(["reservation", "awaiting_review"])
    );
    assert_eq!(value["percentage"], 0);
    let tooltip = value["tooltip"].as_str().unwrap();
    assert!(tooltip.contains("Focus reservation · No task · awaiting_review"));
    assert!(tooltip.contains("LongBreak running 15:00"));
}
