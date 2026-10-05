use pomotui_protocol::{Command, Handler, Request, Response, VERSION};
use pomotui_service::Service;
fn send(service: &mut Service, key: &str, command: Command) -> Response {
    service.handle(Request { version: VERSION, idempotency_key: Some(key.into()), command })
}
#[test]
fn fresh_start_clears_business_and_keeps_replay_protection() {
    let mut service = Service::new();
    send(&mut service, "old", Command::TaskCreate { title: "retired".into() });
    assert!(matches!(send(&mut service, "reset", Command::FreshStart { confirmed: false }), Response::Error { .. }));
    assert!(matches!(send(&mut service, "reset", Command::FreshStart { confirmed: true }), Response::Snapshot { .. }));
    send(&mut service, "old", Command::TaskCreate { title: "new".into() });
    send(&mut service, "reset", Command::FreshStart { confirmed: true });
    let Response::Data { value } = send(&mut service, "list", Command::TaskList) else { panic!() };
    let text = value.to_string();
    assert!(text.contains("new"));
    assert!(!text.contains("retired"));
}
