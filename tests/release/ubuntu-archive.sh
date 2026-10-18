#!/usr/bin/env bash
# Exact distributed archive, real Ubuntu 24.04 host, private Timer Service/socket.
set -Eeuo pipefail
candidate=$(realpath "${1:?candidate directory}")
output=${2:?diagnostic directory}
mkdir -p "$output"
output=$(realpath "$output")
python3 packaging/release/candidate.py verify --output "$candidate" --require-built
# Do not accidentally turn an Arch/container-only run into Ubuntu compatibility evidence.
. /etc/os-release
[[ "$ID" == ubuntu && "$VERSION_ID" == 24.04 ]]
root=$(mktemp -d /tmp/pomotui-archive-smoke.XXXXXX)
service_pid=
cleanup() {
    if [[ -n "$service_pid" ]]; then kill "$service_pid" 2>/dev/null || true; wait "$service_pid" 2>/dev/null || true; fi
    rm -rf "$root"
}
trap cleanup EXIT
mkdir -p "$root/bin" "$root/home" "$root/data" "$root/config" "$root/cache" "$root/runtime"
chmod 700 "$root/runtime"
export HOME="$root/home" XDG_DATA_HOME="$root/data" XDG_CONFIG_HOME="$root/config" XDG_CACHE_HOME="$root/cache" XDG_RUNTIME_DIR="$root/runtime" POMOTUI_SOCKET="$root/runtime/pomotui.sock"
unset LISTEN_FDS LISTEN_PID LISTEN_FDNAMES LD_LIBRARY_PATH LD_PRELOAD
tar -xzf "$candidate/pomotui-linux-x86_64.tar.gz" -C "$root/bin"
{ cat /etc/os-release; getconf GNU_LIBC_VERSION; ldd "$root/bin/pomotui-service"; } > "$output/host-runtime.txt" 2>&1
"$root/bin/pomotui-service" > "$output/service.log" 2>&1 &
service_pid=$!
healthy=false
for attempt in $(seq 1 50); do
    kill -0 "$service_pid"
    if "$root/bin/pomotui" status --json > "$output/public-status.json" 2> "$output/cli.log"; then healthy=true; break; fi
    sleep 0.1
done
[[ "$healthy" == true ]]
python3 - "$candidate" "$output" <<'PY'
import hashlib,json,pathlib,sys
candidate,output=map(pathlib.Path,sys.argv[1:])
status=json.loads((output/'public-status.json').read_text())
if status.get('type')=='error': raise SystemExit('public status is an error')
m=json.loads((candidate/'candidate.json').read_text())
receipt={'schema':1,'passed':True,'platform':'ubuntu-24.04','source_commit':m['source_commit'],'candidate_sha256':hashlib.sha256((candidate/'candidate.json').read_bytes()).hexdigest(),'archive_sha256':m['artifacts']['pomotui-linux-x86_64.tar.gz'],'binary_sha256':m['binary_sha256']}
(output/'ubuntu-archive.json').write_text(json.dumps(receipt,indent=2)+'\n')
PY
