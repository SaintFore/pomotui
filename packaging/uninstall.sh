#!/bin/sh
set -eu

usage() {
  cat <<'EOF'
Usage: packaging/uninstall.sh [--purge]

Remove Pomotui from the current user.
  --purge  Also remove configuration and session history.
EOF
}

purge=false
case "${1:-}" in
  "") ;;
  --purge) purge=true ;;
  -h|--help) usage; exit 0 ;;
  *) usage >&2; exit 2 ;;
esac
if [ "$#" -gt 1 ]; then
  usage >&2
  exit 2
fi

prefix=${PREFIX:-"$HOME/.local"}
config_home=${XDG_CONFIG_HOME:-"$HOME/.config"}
data_home=${XDG_DATA_HOME:-"$HOME/.local/share"}
runtime_home=${XDG_RUNTIME_DIR:-/tmp/pomotui-runtime}

for path in "$prefix" "$config_home" "$data_home" "$runtime_home"; do
  if [ -z "$path" ] || [ "$path" = "/" ]; then
    printf '%s\n' "Refusing to uninstall with unsafe path: $path" >&2
    exit 1
  fi
done

if [ "${POMOTUI_SKIP_SYSTEMD:-0}" != "1" ] && command -v systemctl >/dev/null 2>&1; then
  systemctl --user disable --now pomotui.socket >/dev/null 2>&1 || true
  systemctl --user stop pomotui.service >/dev/null 2>&1 || true
fi

rm -f \
  "$prefix/bin/pomotui" \
  "$prefix/bin/pomotui-tui" \
  "$prefix/bin/pomotui-service" \
  "$config_home/systemd/user/pomotui.socket" \
  "$config_home/systemd/user/pomotui.service"
rm -rf "$prefix/share/pomotui" "$prefix/share/licenses/pomotui"
rm -f "$data_home/applications/pomotui.desktop"
rm -rf "$runtime_home/pomotui"

if [ "$purge" = true ]; then
  rm -rf "$config_home/pomotui" "$data_home/pomotui"
fi

if [ "${POMOTUI_SKIP_SYSTEMD:-0}" != "1" ] && command -v systemctl >/dev/null 2>&1; then
  systemctl --user daemon-reload >/dev/null 2>&1 || true
  systemctl --user reset-failed >/dev/null 2>&1 || true
fi

if [ "$purge" = true ]; then
  printf '%s\n' "Uninstalled Pomotui and removed its configuration and history."
else
  printf '%s\n' "Uninstalled Pomotui. Configuration and history were preserved."
fi
