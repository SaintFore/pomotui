use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, MouseEventKind,
    },
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use pomotui_protocol::{Client, Command};
use pomotui_tui::{Action, App, InputKey, Theme, render};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let result = run(&mut Terminal::new(CrosstermBackend::new(stdout))?);
    disable_raw_mode()?;
    execute!(std::io::stdout(), DisableMouseCapture, LeaveAlternateScreen)?;
    result
}

fn run(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let socket = std::env::var_os("POMOTUI_SOCKET").map_or_else(
        || {
            std::env::var_os("XDG_RUNTIME_DIR")
                .map_or_else(
                    || std::path::PathBuf::from("/tmp/pomotui-runtime"),
                    std::path::PathBuf::from,
                )
                .join("pomotui/pomotui.sock")
        },
        std::path::PathBuf::from,
    );
    let mut client = Client::connect(&socket).ok();
    let config = load_config()?;
    let theme = if config.theme == "Vermilion Paper Light" {
        Theme::VermilionPaperLight
    } else {
        Theme::VermilionPaperDark
    };
    let (completion_animation, warning) = config.animation.as_ref().map_or_else(
        || (pomotui_tui::animation::built_in(), None),
        |path| match std::fs::read_to_string(path) {
            Ok(source) => pomotui_tui::animation::custom_or_builtin(&source),
            Err(error) => (
                pomotui_tui::animation::built_in(),
                Some(format!(
                    "custom animation unreadable: {error}; using built-in"
                )),
            ),
        },
    );
    let mut app = App::new(read_snapshot(client.as_mut()), theme);
    app.warning = warning;
    loop {
        terminal.draw(|frame| render(frame, &mut app))?;
        if !event::poll(Duration::from_millis(250))? {
            let previous = app.snapshot.as_ref().map(|snapshot| snapshot.state.clone());
            app.snapshot = read_snapshot(client.as_mut());
            if previous.as_deref() == Some("running")
                && app
                    .snapshot
                    .as_ref()
                    .is_some_and(|snapshot| snapshot.state == "pending")
            {
                app.begin_completion(completion_animation.clone());
            }
            app.animation_tick(250);
            if client.is_none() {
                client = Client::connect(&socket).ok();
            }
            continue;
        }
        match event::read()? {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                let input = match key.code {
                    KeyCode::Char(value) => {
                        Some(InputKey::Char(canonical_key(value, &config.keybindings)))
                    }
                    KeyCode::Esc => Some(InputKey::Escape),
                    KeyCode::Enter => Some(InputKey::Enter),
                    KeyCode::Backspace => Some(InputKey::Backspace),
                    KeyCode::Up => Some(InputKey::Up),
                    KeyCode::Down => Some(InputKey::Down),
                    KeyCode::Left => Some(InputKey::Left),
                    KeyCode::Right => Some(InputKey::Right),
                    _ => None,
                };
                if let Some(action) = input.and_then(|value| app.handle_key(value)) {
                    if action == Action::Quit {
                        break;
                    }
                    send_action(&mut client, &mut app, action);
                }
            }
            Event::Mouse(mouse) if mouse.kind == MouseEventKind::Down(event::MouseButton::Left) => {
                if let Some(action) = app.mouse_click(mouse.column, mouse.row) {
                    send_action(&mut client, &mut app, action);
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn canonical_key(value: char, keys: &pomotui_tui::config::Keybindings) -> char {
    for (configured, canonical) in [
        (&keys.down, 'j'),
        (&keys.up, 'k'),
        (&keys.next_view, 'l'),
        (&keys.previous_view, 'h'),
        (&keys.skip, 'K'),
        (&keys.toggle_session, ' '),
        (&keys.stop, 'X'),
        (&keys.palette, ':'),
        (&keys.settings, 's'),
        (&keys.help, '?'),
        (&keys.quit, 'q'),
    ] {
        if configured.starts_with(value) {
            return canonical;
        }
    }
    value
}

fn load_config() -> Result<pomotui_tui::config::Config, Box<dyn std::error::Error>> {
    let config_home = std::env::var_os("XDG_CONFIG_HOME").map_or_else(
        || {
            std::env::var_os("HOME").map_or_else(
                || std::path::PathBuf::from(".config"),
                |home| std::path::PathBuf::from(home).join(".config"),
            )
        },
        std::path::PathBuf::from,
    );
    match std::fs::read_to_string(config_home.join("pomotui/config.toml")) {
        Ok(source) => pomotui_tui::config::parse(&source).map_err(Into::into),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(pomotui_tui::config::Config::default())
        }
        Err(error) => Err(error.into()),
    }
}

fn read_snapshot(client: Option<&mut Client>) -> Option<pomotui_protocol::Snapshot> {
    let response = client?
        .request(&pomotui_cli_request(Command::Status))
        .ok()?;
    if let pomotui_protocol::Response::Snapshot { snapshot } = response {
        Some(snapshot)
    } else {
        None
    }
}

fn send_action(client: &mut Option<Client>, app: &mut App, action: Action) {
    let Action::Command(command) = action else {
        return;
    };
    let Some(connection) = client else {
        app.message = Some("Timer Service unavailable; reconnecting".into());
        return;
    };
    match connection.request(&pomotui_cli_request(command)) {
        Ok(pomotui_protocol::Response::Accepted) => {
            app.message = Some("Command accepted".into());
            app.snapshot = read_snapshot(Some(connection));
        }
        Ok(pomotui_protocol::Response::Snapshot { snapshot }) => {
            app.snapshot = Some(snapshot);
            app.message = Some("State refreshed".into());
        }
        Ok(pomotui_protocol::Response::Error { error }) => {
            app.message = Some(format!("Command rejected: {error:?}"));
        }
        Ok(pomotui_protocol::Response::Data { .. }) => {
            app.message = Some("Command completed".into());
            app.snapshot = read_snapshot(Some(connection));
        }
        Err(error) => {
            app.message = Some(format!("Timer Service unavailable: {error}"));
            *client = None;
        }
    }
}

fn pomotui_cli_request(command: Command) -> pomotui_protocol::Request {
    let idempotency_key = command
        .mutates()
        .then(|| format!("tui-{}-{:?}", std::process::id(), std::time::Instant::now()));
    pomotui_protocol::Request {
        version: pomotui_protocol::PROTOCOL_VERSION,
        idempotency_key,
        command,
    }
}
