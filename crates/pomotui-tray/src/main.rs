#[cfg(target_os = "macos")]
mod tray {
    use muda::{Menu, MenuItem, PredefinedMenuItem};
    use pomotui_protocol::{Client, Command, Request, SessionKind, Snapshot};
    use std::path::PathBuf;
    use std::time::Duration;
    use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

    const POLL_INTERVAL: Duration = Duration::from_secs(1);

    pub fn run() -> Result<(), Box<dyn std::error::Error>> {
        let socket = socket_path();
        let mut client = Client::connect(&socket)?;

        let icon = create_icon();
        let tray = TrayIconBuilder::new()
            .with_icon(icon)
            .with_tooltip("Pomotui")
            .build()?;

        let menu = Menu::new();
        let status_item = MenuItem::new("Connecting...", false, None);
        let sep = PredefinedMenuItem::separator();
        let start_item = MenuItem::new("Start Focus", true, None);
        let pause_item = MenuItem::new("Pause", true, None);
        let stop_item = MenuItem::new("Stop", true, None);
        let sep2 = PredefinedMenuItem::separator();
        let quit_item = MenuItem::new("Quit Tray", true, None);
        menu.append(&status_item)?;
        menu.append(&sep)?;
        menu.append(&start_item)?;
        menu.append(&pause_item)?;
        menu.append(&stop_item)?;
        menu.append(&sep2)?;
        menu.append(&quit_item)?;
        tray.set_menu(Some(menu));

        let mut last_text = String::new();

        loop {
            // Poll service
            if let Ok(snapshot) = fetch_snapshot(&mut client) {
                let text = pomotui_tray::format_tray_text(&snapshot);
                let status = pomotui_tray::format_status_line(&snapshot);
                if text != last_text {
                    let _ = tray.set_tooltip(Some(&text));
                    last_text = text;
                }
                status_item.set_text(&status);

                let running = snapshot.state == "running";
                let paused = snapshot.state == "paused";
                start_item.set_enabled(!running);
                pause_item.set_enabled(running || paused);
                stop_item.set_enabled(running || paused);
            }

            // Handle menu events
            if let Ok(event) = muda::menu_event_receiver().try_recv() {
                match event.id {
                    id if id == start_item.id() => {
                        let _ = send_command(&socket, Command::Start {
                            kind: SessionKind::Focus,
                            task_id: None,
                        });
                    }
                    id if id == pause_item.id() => {
                        let _ = send_command(&socket, Command::Pause);
                    }
                    id if id == stop_item.id() => {
                        let _ = send_command(&socket, Command::Stop { review: false });
                    }
                    id if id == quit_item.id() => {
                        break;
                    }
                    _ => {}
                }
            }

            std::thread::sleep(POLL_INTERVAL);
        }

        Ok(())
    }

    fn socket_path() -> PathBuf {
        std::env::var_os("POMOTUI_SOCKET")
            .map_or_else(
                || {
                    std::env::var_os("XDG_RUNTIME_DIR")
                        .map_or_else(
                            || PathBuf::from("/tmp/pomotui-runtime"),
                            PathBuf::from,
                        )
                        .join("pomotui/pomotui.sock")
                },
                PathBuf::from,
            )
    }

    fn create_icon() -> Icon {
        // 16x16 RGBA icon — a simple red circle
        let mut rgba = Vec::with_capacity(16 * 16 * 4);
        for y in 0..16u32 {
            for x in 0..16u32 {
                let cx = x as f64 - 7.5;
                let cy = y as f64 - 7.5;
                let inside = (cx * cx + cy * cy) < 49.0; // radius 7
                if inside {
                    rgba.extend_from_slice(&[0xD6, 0x4A, 0x3C, 0xFF]); // vermillion
                } else {
                    rgba.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]); // transparent
                }
            }
        }
        Icon::from_rgba(rgba, 16, 16).expect("icon")
    }

    fn fetch_snapshot(client: &mut Client) -> Result<Snapshot, Box<dyn std::error::Error>> {
        let request = Request {
            version: pomotui_protocol::PROTOCOL_VERSION,
            idempotency_key: None,
            command: Command::Status,
        };
        match client.request(&request)? {
            pomotui_protocol::Response::Status { snapshot, .. } => Ok(snapshot),
            pomotui_protocol::Response::Error { error } => {
                Err(format!("{error:?}").into())
            }
        }
    }

    fn send_command(
        socket: &PathBuf,
        command: Command,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut client = Client::connect(socket)?;
        let request = Request {
            version: pomotui_protocol::PROTOCOL_VERSION,
            idempotency_key: Some(format!("tray-{}", std::process::id())),
            command,
        };
        client.request(&request)?;
        Ok(())
    }
}

#[cfg(target_os = "macos")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    tray::run()
}

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("pomotui-tray is only available on macOS.");
    eprintln!("On Linux, use Waybar: see README for integration instructions.");
    std::process::exit(1);
}
