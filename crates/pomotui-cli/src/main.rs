use pomotui_cli::{parse, render, request};
use pomotui_protocol::Client;

fn main() {
    if let Err(error) = run() {
        if !error.is_empty() {
            eprintln!("{error}");
        }
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args == ["reset", "--all-data", "--confirm"] {
        let outcome = pomotui_cli::reset_all_data(&database_path(), &socket_path())?;
        if let Some(backup) = outcome.backup {
            println!(
                "local domain data reset; database backup: {}",
                backup.display()
            );
        } else {
            println!("local domain data already absent; nothing to reset");
        }
        return Ok(());
    }
    let (command, json, waybar) = parse(&args)?;
    let response = match Client::connect(&socket_path())
        .and_then(|mut client| client.request(&request(command)))
    {
        Ok(response) => response,
        Err(error) if json => {
            let response = pomotui_protocol::Response::Error {
                error: pomotui_protocol::ProtocolError::Disconnected {
                    message: format!("Timer Service unavailable: {error}"),
                },
            };
            println!("{}", render(&response, true, waybar)?);
            return Err(String::new());
        }
        Err(error) => return Err(format!("Timer Service unavailable: {error}")),
    };
    let rejected = matches!(response, pomotui_protocol::Response::Error { .. });
    println!("{}", render(&response, json, waybar)?);
    if rejected && json {
        Err(String::new())
    } else {
        Ok(())
    }
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

fn database_path() -> std::path::PathBuf {
    std::env::var_os("XDG_DATA_HOME")
        .map_or_else(
            || {
                std::env::var_os("HOME").map_or_else(
                    || std::path::PathBuf::from(".local/share"),
                    |home| std::path::PathBuf::from(home).join(".local/share"),
                )
            },
            std::path::PathBuf::from,
        )
        .join("pomotui/pomotui.sqlite3")
}
