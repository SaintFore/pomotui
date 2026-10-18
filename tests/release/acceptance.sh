#!/usr/bin/env bash
# Run as root inside a disposable, booted Arch system. Packages are inputs.
set -Eeuo pipefail
old=$(realpath "$1")
candidate=$(realpath "$2")
output=$(realpath "$3")
mkdir -p "$output"
exec > >(tee "$output/acceptance.log") 2>&1
started=$SECONDS
user=pomotui-acceptance
useradd -m "$user"
uid=$(id -u "$user")
observe() {
    journalctl --no-pager > "$output/journal.log" || true
    cp /var/log/pacman.log "$output/pacman.log" || true
    systemctl status "user@$uid.service" --no-pager > "$output/user-manager.log" || true
    free -m > "$output/memory.txt"
    df -h > "$output/disk.txt"
    printf 'elapsed_seconds=%s\n' "$((SECONDS-started))" > "$output/runtime.txt"
}
trap observe EXIT
as_user() { runuser -u "$user" -- env XDG_RUNTIME_DIR="/run/user/$uid" DBUS_SESSION_BUS_ADDRESS="unix:path=/run/user/$uid/bus" "$@"; }
loginctl enable-linger "$user"
systemctl start "user@$uid.service"
as_user systemctl --user status --no-pager
sha256sum "$old" "$candidate" | tee "$output/packages.sha256"
old_version=$(pacman -Qp "$old")
candidate_version=$(pacman -Qp "$candidate")
[[ "$old_version" != "$candidate_version" ]]
printf 'old=%s\ncandidate=%s\n' "$old_version" "$candidate_version" > "$output/identity.txt"
# Clean candidate install uses the documented startup interface in fresh user state.
pacman -U --noconfirm --ask=4 "$candidate"
as_user systemctl --user daemon-reload
as_user systemctl --user enable --now pomotui.socket
as_user pomotui status --json | tee "$output/clean-status.json"
as_user systemctl --user is-active pomotui.service
as_user systemctl --user disable --now pomotui.socket
as_user systemctl --user stop pomotui.service
pacman -R --noconfirm "${candidate_version%% *}"
rm -rf "/home/$user/.local/share/pomotui" "/home/$user/.config/pomotui"
pacman -U --noconfirm --ask=4 "$old"
old_hash=$(sha256sum /usr/bin/pomotui-service | cut -d' ' -f1)
as_user systemctl --user daemon-reload
as_user systemctl --user enable --now pomotui.socket
as_user pomotui status --json | tee "$output/old-status.json"
old_pid=$(as_user systemctl --user show pomotui.service -p MainPID --value)
[[ "$old_pid" -gt 0 ]]
# Genuine pacman post_upgrade must recover the already configured service.
pacman -U --noconfirm --ask=4 "$candidate"
new_pid=$(as_user systemctl --user show pomotui.service -p MainPID --value)
[[ "$new_pid" -gt 0 && "$new_pid" != "$old_pid" ]]
[[ "$(pacman -Q "${candidate_version%% *}")" == "$candidate_version" ]]
new_hash=$(sha256sum /usr/bin/pomotui-service | cut -d' ' -f1)
# Packaging-only candidates may contain identical executable bytes. Identity is
# the exact installed candidate package plus replacement PID and matching bytes.
expected_hash=$(tar --zstd -xOf "$candidate" usr/bin/pomotui-service | sha256sum | cut -d' ' -f1)
[[ "$new_hash" == "$expected_hash" ]]
[[ "$(sha256sum "/proc/$new_pid/exe" | cut -d' ' -f1)" == "$new_hash" ]]
[[ "$(as_user systemctl --user show pomotui.service -p FragmentPath --value)" == /usr/lib/systemd/user/pomotui.service ]]
as_user systemctl --user is-active pomotui.service
as_user pomotui status --json | tee "$output/candidate-status.json"
as_user systemctl --user show pomotui.service > "$output/candidate-unit.txt"
printf 'old_pid=%s\ncandidate_pid=%s\nold_service_sha256=%s\ncandidate_service_sha256=%s\n' "$old_pid" "$new_pid" "$old_hash" "$new_hash" >> "$output/identity.txt"
# A Git package also has a same-name post_upgrade path after replacement.
# Reinstalling exact candidate bytes proves packaging-only upgrades need no
# invented executable/version difference while still replacing the running PID.
if [[ "${candidate_version%% *}" == pomotui-git ]]; then
    previous_pid=$new_pid
    pacman -U --noconfirm --ask=4 "$candidate"
    new_pid=$(as_user systemctl --user show pomotui.service -p MainPID --value)
    [[ "$new_pid" -gt 0 && "$new_pid" != "$previous_pid" ]]
    [[ "$(sha256sum "/proc/$new_pid/exe" | cut -d' ' -f1)" == "$expected_hash" ]]
    as_user pomotui status --json > "$output/same-name-status.json"
fi
echo 'PASS: clean install and real package upgrade served by candidate'

python "$(dirname "${BASH_SOURCE[0]}")/scenarios/domain-state.py" "$old" "$candidate" "$output" "$user" "$uid"

source "$(dirname "$0")/lifecycle.sh"
