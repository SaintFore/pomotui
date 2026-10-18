#!/bin/sh
# Run after cargo build -p pomotui-service -p pomotui-cli.
set -eu
reservation_root=$(mktemp -d)
service_pid=
cleanup() {
  if [ -n "$service_pid" ]; then kill "$service_pid" 2>/dev/null || true; wait "$service_pid" 2>/dev/null || true; fi
  rm -rf "$reservation_root"
}
trap cleanup EXIT INT TERM
export XDG_RUNTIME_DIR="$reservation_root/runtime"
export XDG_DATA_HOME="$reservation_root/data"
export XDG_CONFIG_HOME="$reservation_root/config"
export POMOTUI_SOCKET="$reservation_root/pomotui.sock"
mkdir -p "$XDG_RUNTIME_DIR" "$XDG_DATA_HOME" "$XDG_CONFIG_HOME/pomotui"
printf '# preserved settings\nfocus_minutes = 50\n[keybindings]\nquit = "q"\n' > "$XDG_CONFIG_HOME/pomotui/config.toml"
start_service() {
  target/debug/pomotui-service >"$reservation_root/service.log" 2>&1 &
  service_pid=$!
  attempts=0
  until target/debug/pomotui status --json >"$reservation_root/status.json" 2>/dev/null; do
    attempts=$((attempts + 1))
    if [ "$attempts" -ge 100 ]; then cat "$reservation_root/service.log"; exit 1; fi
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
target/debug/pomotui reservation minutes 7 --json > "$reservation_root/configured.json"
target/debug/pomotui task create "Reservation task" --json > "$reservation_root/task.json"
target/debug/pomotui reserve --task 1 --json > "$reservation_root/reserved.json"
target/debug/pomotui waybar > "$reservation_root/waybar-before.json"
stop_service
start_service
target/debug/pomotui waybar > "$reservation_root/waybar-recovered.json"
python3 - "$reservation_root" <<'PY'
import json, pathlib, sys
root = pathlib.Path(sys.argv[1])
a = json.loads((root/'reserved.json').read_text())['snapshot']
b = json.loads((root/'status.json').read_text())['snapshot']
assert b['reservation']['id'] == a['reservation']['id']
assert b['reservation_minutes'] == 7
assert b['reservation']['duration_seconds'] == 420
assert b['reservation']['task_title'] == 'Reservation task'
assert b['reservation']['remaining_seconds'] <= a['reservation']['remaining_seconds']
for filename in ['waybar-before.json', 'waybar-recovered.json']:
    waybar = json.loads((root/filename).read_text())
    assert waybar['class'] == ['reservation', 'countdown']
    assert waybar['text'].startswith('reservation ')
    assert 'Reservation task' in waybar['tooltip']
source = (root/'config/pomotui/config.toml').read_text()
assert source.startswith('reservation_minutes = 7\n')
assert '# preserved settings' in source and '[keybindings]' in source
PY
target/debug/pomotui start focus --json > "$reservation_root/started.json"
python3 - "$reservation_root/started.json" <<'PY'
import json, sys
s = json.load(open(sys.argv[1]))['snapshot']
assert s['reservation'] is None
assert s['state'] == 'running' and s['kind'] == 'focus'
assert s['current_task_id'] == 1
assert s['planned_seconds'] == 3000
assert s['action_chain']['length'] == 0
assert not s['recent_history']
PY
target/debug/pomotui waybar > "$reservation_root/waybar-started.json"
python3 - "$reservation_root/waybar-started.json" <<'PYBAR'
import json, sys
s = json.load(open(sys.argv[1]))
assert s['class'] == ['running', 'focus']
assert s['text'].startswith('running ')
PYBAR
target/debug/pomotui stop --no-review --json > "$reservation_root/stopped.json"
printf '%s\n' 'Reservation CLI / Waybar / restart acceptance passed'
