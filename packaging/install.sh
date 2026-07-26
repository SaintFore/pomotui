#!/bin/sh
set -eu

prefix=${PREFIX:-"$HOME/.local"}
config_home=${XDG_CONFIG_HOME:-"$HOME/.config"}

install -Dm755 target/release/pomotui "$prefix/bin/pomotui"
install -Dm755 target/release/pomotui-tui "$prefix/bin/pomotui-tui"
install -Dm755 target/release/pomotui-service "$prefix/bin/pomotui-service"
install -Dm644 packaging/systemd/pomotui.socket "$config_home/systemd/user/pomotui.socket"
install -Dm644 packaging/systemd/pomotui.service "$config_home/systemd/user/pomotui.service"
install -Dm644 packaging/defaults/config.toml "$prefix/share/pomotui/config.example.toml"
if [ ! -e "$config_home/pomotui/config.toml" ]; then
  install -Dm644 packaging/defaults/config.toml "$config_home/pomotui/config.toml"
fi
install -Dm644 packaging/defaults/building-collapse.animation \
  "$prefix/share/pomotui/building-collapse.animation"
install -Dm644 LICENSE "$prefix/share/licenses/pomotui/LICENSE"

printf '%s\n' "Installed Pomotui. Run: systemctl --user enable --now pomotui.socket"
