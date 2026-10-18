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
target/debug/pomotui reserve --no-task --json > "$reservation_root/reserved.json"
reservation_id=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["snapshot"]["reservation"]["id"])' "$reservation_root/reserved.json")
# An unconfirmed failure reports consequences but leaves the reservation intact.
if target/debug/pomotui reservation failure "$reservation_id" "Could not begin" --json > "$reservation_root/preview.json"; then
  printf '%s\n' 'Unconfirmed failure unexpectedly succeeded'; exit 1
fi
target/debug/pomotui status --json > "$reservation_root/status.json"
target/debug/pomotui reservation cancel failure "$reservation_id" "Could not begin" --confirm --json > "$reservation_root/failed.json"
stop_service
start_service
python3 - "$reservation_root" <<'PY_CHECK'
import json, pathlib, sys
root = pathlib.Path(sys.argv[1])
preview = json.loads((root/'preview.json').read_text())
assert preview['result'] == 'error'
assert '0 links' in preview['error']['message']
a = json.loads((root/'failed.json').read_text())['snapshot']
b = json.loads((root/'status.json').read_text())['snapshot']
assert a['reservation'] is None and b['reservation'] is None
assert a['action_chain']['length'] == b['action_chain']['length'] == 0
assert a['recent_history'] == b['recent_history'] == []
assert len(b['recent_ended_chains']) == 1
assert b['recent_ended_chains'][0]['break_source'] == 'reservation_review'
assert b['recent_ended_chains'][0]['break_actual_seconds'] is None
assert b['recent_ended_chains'][0]['break_task_title'] is None
assert b['recent_ended_chains'][0]['break_reflection'] == 'Could not begin'
PY_CHECK
if target/debug/pomotui reservation failure "$reservation_id" "Again" --confirm --json > /dev/null; then
  printf '%s\n' 'Stale failure unexpectedly succeeded'; exit 1
fi
target/debug/pomotui reserve --json > "$reservation_root/second.json"
reservation_id=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["snapshot"]["reservation"]["id"])' "$reservation_root/second.json")
target/debug/pomotui reservation cancel success "$reservation_id" --json > "$reservation_root/started.json"
python3 - "$reservation_root/started.json" <<'PY_CHECK'
import json, sys
s = json.load(open(sys.argv[1]))['snapshot']
assert s['reservation'] is None and s['state'] == 'running'
assert s['action_chain']['length'] == 0
assert len(s['recent_ended_chains']) == 1
PY_CHECK
printf '%s\n' 'Reservation failure CLI / restart acceptance passed'
