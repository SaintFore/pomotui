#!/bin/sh
set -eu

tmp_root=$(mktemp -d)
service_pid=
cleanup() {
  if [ -n "$service_pid" ]; then
    kill "$service_pid" 2>/dev/null || true
    wait "$service_pid" 2>/dev/null || true
  fi
  rm -rf "$tmp_root"
}
trap cleanup EXIT INT TERM

export XDG_RUNTIME_DIR="$tmp_root/runtime"
export XDG_DATA_HOME="$tmp_root/data"
export XDG_CONFIG_HOME="$tmp_root/config"
export POMOTUI_SOCKET="$XDG_RUNTIME_DIR/pomotui/pomotui.sock"
mkdir -p "$XDG_RUNTIME_DIR" "$XDG_DATA_HOME" "$XDG_CONFIG_HOME/pomotui"
cp packaging/defaults/config.toml "$XDG_CONFIG_HOME/pomotui/config.toml"

start_service() {
  target/debug/pomotui-service >"$tmp_root/service.log" 2>&1 &
  service_pid=$!
  attempts=0
  while [ ! -S "$POMOTUI_SOCKET" ]; do
    attempts=$((attempts + 1))
    if [ "$attempts" -ge 100 ]; then
      cat "$tmp_root/service.log"
      return 1
    fi
    sleep 0.02
  done
}

stop_service() {
  kill "$service_pid"
  wait "$service_pid" 2>/dev/null || true
  service_pid=
  rm -f "$POMOTUI_SOCKET"
}

start_service
target/debug/pomotui task create "Persistent Task"
target/debug/pomotui start focus --task 1
status=$(target/debug/pomotui status --json)
case "$status" in
  *'"state":"running"'*'"current_task":"Persistent Task"'*) ;;
  *) printf '%s\n' "unexpected running status: $status"; exit 1 ;;
esac
waybar=$(target/debug/pomotui waybar)
case "$waybar" in
  *'"class":["running","focus"]'*) ;;
  *) printf '%s\n' "unexpected Waybar output: $waybar"; exit 1 ;;
esac
stop_service

start_service
recovered=$(target/debug/pomotui status --json)
case "$recovered" in
  *'"state":"running"'*'"current_task":"Persistent Task"'*) ;;
  *) printf '%s\n' "state did not recover: $recovered"; exit 1 ;;
esac
waybar_after_restart=$(target/debug/pomotui waybar)
case "$waybar_after_restart" in
  *'"class":["running","focus"]'*) ;;
  *) printf '%s\n' "Waybar did not reconnect: $waybar_after_restart"; exit 1 ;;
esac
target/debug/pomotui stop
history=$(target/debug/pomotui history --json)
case "$history" in
  *'"outcome":"Stopped"'*'"task_title":"Persistent Task"'*) ;;
  *) printf '%s\n' "history did not preserve Task snapshot: $history"; exit 1 ;;
esac

stop_service

activation_socket="$XDG_RUNTIME_DIR/pomotui/activation.sock"
systemd-socket-activate \
  --listen="$activation_socket" \
  --setenv="XDG_DATA_HOME=$XDG_DATA_HOME" \
  --setenv="XDG_CONFIG_HOME=$XDG_CONFIG_HOME" \
  target/debug/pomotui-service >"$tmp_root/activation.log" 2>&1 &
service_pid=$!
attempts=0
while [ ! -S "$activation_socket" ]; do
  attempts=$((attempts + 1))
  if [ "$attempts" -ge 100 ]; then
    cat "$tmp_root/activation.log"
    exit 1
  fi
  sleep 0.02
done
activated=$(POMOTUI_SOCKET="$activation_socket" target/debug/pomotui status --json)
case "$activated" in
  *'"result":"snapshot"'*) ;;
  *) printf '%s\n' "socket activation failed: $activated"; exit 1 ;;
esac
kill "$service_pid"
wait "$service_pid" 2>/dev/null || true
service_pid=
rm -f "$activation_socket"

if disconnected=$(POMOTUI_SOCKET="$tmp_root/missing.sock" \
  target/debug/pomotui status --json 2>/dev/null); then
  printf '%s\n' "disconnected JSON command unexpectedly succeeded"
  exit 1
fi
case "$disconnected" in
  *'"code":"disconnected"'*) ;;
  *) printf '%s\n' "disconnected output was not JSON: $disconnected"; exit 1 ;;
esac
install_home="$tmp_root/home"
install_prefix="$tmp_root/prefix"
install_config="$tmp_root/install-config"
install_data="$tmp_root/install-data"
install_runtime="$tmp_root/install-runtime"
mkdir -p "$install_home" "$install_config" "$install_data/pomotui" \
  "$install_runtime/pomotui"
HOME="$install_home" PREFIX="$install_prefix" XDG_CONFIG_HOME="$install_config" \
  XDG_DATA_HOME="$install_data" XDG_RUNTIME_DIR="$install_runtime" \
  packaging/install.sh >/dev/null
test -x "$install_prefix/bin/pomotui"
test -x "$install_prefix/bin/pomotui-tui"
test -x "$install_prefix/bin/pomotui-service"
test -f "$install_config/systemd/user/pomotui.socket"
test -f "$install_prefix/share/pomotui/building-collapse.animation"
test -f "$install_data/applications/pomotui.desktop"
grep -F "Exec=$install_prefix/bin/pomotui-tui" \
  "$install_data/applications/pomotui.desktop" >/dev/null
grep -F "Terminal=true" "$install_data/applications/pomotui.desktop" >/dev/null
printf '%s\n' "user_setting = true" >"$install_config/pomotui/config.toml"
printf '%s\n' "history" >"$install_data/pomotui/pomotui.sqlite3"
printf '%s\n' "socket" >"$install_runtime/pomotui/pomotui.sock"
HOME="$install_home" PREFIX="$install_prefix" XDG_CONFIG_HOME="$install_config" \
  XDG_DATA_HOME="$install_data" XDG_RUNTIME_DIR="$install_runtime" \
  packaging/install.sh >/dev/null
test "$(cat "$install_config/pomotui/config.toml")" = "user_setting = true"

HOME="$install_home" PREFIX="$install_prefix" XDG_CONFIG_HOME="$install_config" \
  XDG_DATA_HOME="$install_data" XDG_RUNTIME_DIR="$install_runtime" \
  POMOTUI_SKIP_SYSTEMD=1 packaging/uninstall.sh >/dev/null
test ! -e "$install_prefix/bin/pomotui"
test ! -e "$install_prefix/bin/pomotui-tui"
test ! -e "$install_prefix/bin/pomotui-service"
test ! -e "$install_config/systemd/user/pomotui.socket"
test ! -e "$install_prefix/share/pomotui"
test ! -e "$install_data/applications/pomotui.desktop"
test -f "$install_config/pomotui/config.toml"
test -f "$install_data/pomotui/pomotui.sqlite3"
test ! -e "$install_runtime/pomotui"

HOME="$install_home" PREFIX="$install_prefix" XDG_CONFIG_HOME="$install_config" \
  XDG_DATA_HOME="$install_data" XDG_RUNTIME_DIR="$install_runtime" \
  POMOTUI_SKIP_SYSTEMD=1 packaging/uninstall.sh --purge >/dev/null
test ! -e "$install_config/pomotui"
test ! -e "$install_data/pomotui"

printf '%s\n' "Pomotui end-to-end smoke test passed"
