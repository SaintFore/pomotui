#!/usr/bin/env bash
# GitHub Ubuntu host; builds outside guest, boots a real disposable Arch manager.
set -Eeuo pipefail
out=${1:?output directory required}
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
mkdir -p "$out/work/artifacts" "$out/work/old" "$out/work/candidate"
git archive v0.3.0 | tar -x -C "$out/work/old"
git archive HEAD | tar -x -C "$out/work/candidate"
git rev-list --count HEAD > "$out/work/candidate-count"
git rev-parse --short=12 HEAD > "$out/work/candidate-short"
{ git rev-parse 'v0.3.0^{commit}'; git rev-parse HEAD; } > "$out/sources.txt"
cp tests/release/build-packages.sh "$out/work/build-packages.sh"
docker run --rm -v "$out/work:/work" archlinux:base-devel bash /work/build-packages.sh 2>&1 | tee "$out/build.log"
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
old=$(find "$out/work/artifacts" -name 'pomotui-0.3.0-*.pkg.tar.zst' ! -name 'pomotui-debug-*' -printf '%f\n')
candidate=$(find "$out/work/artifacts" -name 'pomotui-*.r*.pkg.tar.zst' ! -name 'pomotui-debug-*' -printf '%f\n')
[[ -n "$old" && -n "$candidate" ]]
cp tests/release/acceptance.sh "$out/acceptance.sh"
cp -r tests/release/scenarios "$out/scenarios"
sudo timeout 300 systemd-run --machine=pomotui-acceptance --wait --pipe /usr/bin/bash /acceptance/acceptance.sh "/acceptance/work/artifacts/$old" "/acceptance/work/artifacts/$candidate" /acceptance/result 2>&1 | tee "$out/scenario.log"
