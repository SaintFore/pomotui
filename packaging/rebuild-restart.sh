#!/bin/sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(dirname -- "$script_dir")

cd "$repo_root"

printf '%s\n' "Building Pomotui release binaries..."
cargo build --release --workspace

printf '%s\n' "Installing Pomotui (configuration and history are preserved)..."
"$script_dir/install.sh"

if ! command -v systemctl >/dev/null 2>&1; then
  printf '%s\n' "Installed, but systemctl is unavailable; restart pomotui-service manually." >&2
  exit 0
fi

printf '%s\n' "Reloading and restarting the Pomotui user service..."
systemctl --user daemon-reload
systemctl --user enable --now pomotui.socket
systemctl --user restart pomotui.service

printf '%s\n' "Pomotui is updated and running."
printf '%s\n' "Close any open TUI and launch it again with: pomotui-tui"
