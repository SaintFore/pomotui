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
install -Dm644 favicon_io/pomotui-16x16.png \
  "$data_home/icons/hicolor/16x16/apps/pomotui.png"
install -Dm644 favicon_io/pomotui-32x32.png \
  "$data_home/icons/hicolor/32x32/apps/pomotui.png"
install -Dm644 favicon_io/pomotui-192x192.png \
  "$data_home/icons/hicolor/192x192/apps/pomotui.png"
install -Dm644 favicon_io/pomotui-512x512.png \
  "$data_home/icons/hicolor/512x512/apps/pomotui.png"
desktop_exec=$(printf '%s' "$prefix/bin/pomotui-tui" | sed 's/\\/\\\\/g; s/&/\\&/g; s/|/\\|/g')
sed \
  -e "s|@EXEC@|$desktop_exec|" \
  -e "s|@TRY_EXEC@|$desktop_exec|" \
  packaging/pomotui.desktop.in >"$data_home/applications/pomotui.desktop.tmp"
chmod 0644 "$data_home/applications/pomotui.desktop.tmp"
mv "$data_home/applications/pomotui.desktop.tmp" \
  "$data_home/applications/pomotui.desktop"

if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database "$data_home/applications" >/dev/null 2>&1 || true
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
  gtk-update-icon-cache -f -t "$data_home/icons/hicolor" >/dev/null 2>&1 || true
fi

printf '%s\n' "Installed Pomotui. Run: systemctl --user enable --now pomotui.socket"
