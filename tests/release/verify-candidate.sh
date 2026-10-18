#!/usr/bin/env bash
# GitHub Ubuntu host; builds outside guest, boots a real disposable Arch manager.
set -Eeuo pipefail
candidate_dir=$(realpath "${1:?built candidate directory required}")
out=${2:?output directory required}
fixture=${3:-}
[[ -z "$fixture" || "$fixture" == --allow-nonpublishing-fixture-predecessor ]]
python3 packaging/release/candidate.py verify --output "$candidate_dir" --require-built
mkdir -p "$out"
out=$(realpath "$out")
root=$(mktemp -d)
container=
cleanup() {
    sudo journalctl -M pomotui-acceptance --no-pager > "$out/guest-journal.log" 2>&1 || true
    sudo machinectl status pomotui-acceptance > "$out/machine-status.txt" 2>&1 || true
    sudo machinectl terminate pomotui-acceptance || true
    if [[ -n "$container" ]]; then docker rm -f "$container" || true; fi
    sudo rm -rf "$root"
}
trap cleanup EXIT
{ uname -a; systemd --version; ls -l /dev/kvm || true; lscpu; free -m; df -h; } > "$out/host.txt" 2>&1
sudo apt-get update
sudo apt-get install -y systemd-container
# Explicit capability probe, failure is a failed gate, never a skipped scenario.
sudo systemd-nspawn --version | tee "$out/nspawn-version.txt"
docker pull archlinux:base-devel
docker image inspect archlinux:base-devel --format '{{json .RepoDigests}}' > "$out/arch-image.json"
mkdir -p "$out/previous/source" "$out/packages"
# Select the newest published stable release below a stable candidate, or the
# latest published stable for a Git candidate. Tags alone are not delivery. No candidate source/version mutation is permitted.
previous_args=()
if [[ -n "$fixture" ]]; then previous_args+=("$fixture"); fi
previous=$(python3 packaging/release/release.py previous --candidate "$candidate_dir" --repo . --output "$out/previous-release.json" "${previous_args[@]}")
git archive "$previous" | tar -x -C "$out/previous/source"
{ git rev-parse "$previous^{commit}"; python3 -c 'import json,sys;print(json.load(open(sys.argv[1]))["source_commit"])' "$candidate_dir/candidate.json"; } > "$out/sources.txt"
docker run --rm -v "$out/previous:/previous" -v "$PWD/tests/release/build-previous.sh:/build.sh:ro" -e CANDIDATE_UID="$(id -u)" -e CANDIDATE_GID="$(id -g)" archlinux:base-devel bash /build.sh 2>&1 | tee "$out/previous-build.log"
# Copy exactly manifest-declared package bytes into the guest's read-only inputs.
python3 - "$candidate_dir" "$out" <<'PYCODE'
import hashlib,json,pathlib,shutil,sys
src,out=map(pathlib.Path,sys.argv[1:]);m=json.loads((src/'candidate.json').read_text())
for channel,name in m['packages'].items():
    destination=out/'packages'/pathlib.Path(name).name
    shutil.copyfile(src/name,destination)
    if hashlib.sha256(destination.read_bytes()).hexdigest()!=m['artifacts'][name]: raise SystemExit('Transferred package checksum mismatch: '+channel)
(out/'candidate.json').write_bytes((src/'candidate.json').read_bytes())
PYCODE
# Keep build tools/caches out of the booted guest; install only runtime prerequisites.
container=$(docker create archlinux:base-devel bash -ec 'pacman -Syu --noconfirm systemd dbus sqlite sudo python; systemctl enable systemd-logind.service')
docker start -a "$container" | tee "$out/rootfs.log"
[[ "$(docker inspect -f '{{.State.ExitCode}}' "$container")" == 0 ]]
docker export "$container" | sudo tar -x -C "$root"
sudo systemd-nspawn --directory="$root" --machine=pomotui-acceptance --boot --register=yes --bind="$out:/acceptance" > "$out/boot.log" 2>&1 &
boot_pid=$!
for attempt in $(seq 1 60); do
    if sudo systemd-run --machine=pomotui-acceptance --wait --pipe /usr/bin/systemctl is-active systemd-logind.service; then break; fi
    kill -0 "$boot_pid"
    sleep 2
done
old=$(find "$out/previous/artifacts" -name 'pomotui-*.pkg.tar.zst' ! -name 'pomotui-debug-*' -printf '%f\n')
[[ -n "$old" && "$old" != *$'\n'* ]]
cp tests/release/acceptance.sh "$out/acceptance.sh"
cp -r tests/release/scenarios "$out/scenarios"
mkdir -p "$out/scenarios/fixtures"
cp crates/pomotui-service/tests/fixtures/v0.*-production.sqlite3 "$out/scenarios/fixtures/"
cp tests/release/lifecycle.sh "$out/lifecycle.sh"
while IFS=$'\t' read -r channel package; do
    # Fresh guest user and full state/lifecycle matrix for every distribution.
    sudo timeout 360 systemd-run --machine=pomotui-acceptance --wait --pipe /usr/bin/bash /acceptance/acceptance.sh "/acceptance/previous/artifacts/$old" "/acceptance/packages/$package" "/acceptance/result/$channel" 2>&1 | tee "$out/$channel-scenario.log"
    sudo systemd-run --machine=pomotui-acceptance --wait --pipe /usr/bin/bash -ec 'loginctl disable-linger pomotui-acceptance; systemctl stop user@$(id -u pomotui-acceptance).service; userdel -r pomotui-acceptance; pacman -R --noconfirm "$(pacman -Qq | sed -n "/^pomotui\(-git\)\?$/p")"'
done < <(python3 - "$candidate_dir/candidate.json" <<'PYCODE'
import json,pathlib,sys
for channel,name in json.load(open(sys.argv[1]))['packages'].items(): print(channel+'\t'+pathlib.Path(name).name)
PYCODE
)
python3 - "$candidate_dir" "$out" <<'PYCODE'
import hashlib,json,pathlib,sys
src,out=map(pathlib.Path,sys.argv[1:]);m=json.loads((src/'candidate.json').read_text())
report={'schema':1,'passed':True,'source_commit':m['source_commit'],'candidate_sha256':hashlib.sha256((src/'candidate.json').read_bytes()).hexdigest(),'packages':{channel:{'sha256':m['artifacts'][name],'passed':True} for channel,name in m['packages'].items()}}
(out/'acceptance.json').write_text(json.dumps(report,indent=2)+'\n')
PYCODE
