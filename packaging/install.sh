#!/bin/sh
set -eu

prefix=${PREFIX:-"$HOME/.local"}
config_home=${XDG_CONFIG_HOME:-"$HOME/.config"}
data_home=${XDG_DATA_HOME:-"$HOME/.local/share"}

install -d "$data_home/applications"

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
desktop_exec=$(printf '%s' "$prefix/bin/pomotui-tui" | sed 's/\\/\\\\/g; s/&/\\&/g; s/|/\\|/g')
sed \
  -e "s|@EXEC@|$desktop_exec|" \
  -e "s|@TRY_EXEC@|$desktop_exec|" \
  packaging/pomotui.desktop.in >"$data_home/applications/pomotui.desktop.tmp"
chmod 0644 "$data_home/applications/pomotui.desktop.tmp"
mv "$data_home/applications/pomotui.desktop.tmp" \
  "$data_home/applications/pomotui.desktop"

printf '%s\n' "Installed Pomotui. Run: systemctl --user enable --now pomotui.socket"
