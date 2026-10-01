#!/bin/sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
test_root=$(mktemp -d "${TMPDIR:-/tmp}/pomotui-aur-install.XXXXXX")
trap 'rm -rf "$test_root"' EXIT HUP INT TERM

cat >"$test_root/sudo" <<'EOF'
#!/bin/sh
printf '%s\n' "$*" >>"$POMOTUI_TEST_CALLS"
exit "${POMOTUI_TEST_SUDO_EXIT:-0}"
EOF
chmod +x "$test_root/sudo"

PATH="$test_root:$PATH"
POMOTUI_TEST_CALLS="$test_root/calls"
export PATH POMOTUI_TEST_CALLS

# shellcheck disable=SC1091
. "$repo_root/packaging/aur/pomotui.install"

SUDO_USER=tester post_upgrade >"$test_root/success.out" 2>"$test_root/success.err"
grep -q 'tester systemctl --user daemon-reload' "$test_root/calls"
grep -q 'tester systemctl --user try-restart pomotui.service' "$test_root/calls"

: >"$test_root/calls"
POMOTUI_TEST_SUDO_EXIT=1
export POMOTUI_TEST_SUDO_EXIT
SUDO_USER=tester post_upgrade >"$test_root/failure.out" 2>"$test_root/failure.err"
grep -q 'WARNING.*Timer Service.*restart' "$test_root/failure.err"
grep -q 'systemctl --user restart pomotui.service' "$test_root/failure.err"

unset SUDO_USER
unset POMOTUI_TEST_SUDO_EXIT
post_upgrade >"$test_root/unknown.out" 2>"$test_root/unknown.err"
grep -q 'WARNING.*installing user could not be identified' "$test_root/unknown.err"

printf '%s\n' 'AUR install hook tests passed'
