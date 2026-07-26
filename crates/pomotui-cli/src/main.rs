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
    let (command, json, waybar) = parse(&args)?;
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
    let response =
        match Client::connect(&socket).and_then(|mut client| client.request(&request(command))) {
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
