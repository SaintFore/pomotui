#!/bin/sh
set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
test_root=$(mktemp -d "${TMPDIR:-/tmp}/pomotui-aur-install.XXXXXX")
trap 'rm -rf "$test_root"' EXIT HUP INT TERM

cat >"$test_root/loginctl" <<'MOCK'
#!/bin/sh
printf '%s\n' "$POMOTUI_TEST_USERS"
exit "${POMOTUI_TEST_DISCOVERY_EXIT:-0}"
MOCK
cat >"$test_root/runuser" <<'MOCK'
#!/bin/sh
printf '%s\n' "$*" >>"$POMOTUI_TEST_CALLS"
case "$*" in
    *"-u broken "*) exit 1 ;;
esac
MOCK
cat >"$test_root/cargo" <<'MOCK'
#!/bin/sh
exit 0
MOCK
cat >"$test_root/rustc" <<'MOCK'
#!/bin/sh
printf 'host: x86_64-unknown-linux-gnu\n'
MOCK
chmod +x "$test_root/loginctl" "$test_root/runuser" "$test_root/cargo" "$test_root/rustc"

PATH="$test_root:$PATH"
POMOTUI_TEST_CALLS="$test_root/calls"
POMOTUI_TEST_USERS='1000 tester no active'
export PATH POMOTUI_TEST_CALLS POMOTUI_TEST_USERS

# shellcheck disable=SC1091
. "$repo_root/packaging/aur/pomotui.install"

# pacman can have no SUDO_USER and no inherited desktop bus environment.
unset SUDO_USER XDG_RUNTIME_DIR DBUS_SESSION_BUS_ADDRESS
post_upgrade >"$test_root/success.out" 2>"$test_root/success.err"
grep -q 'tester -- env XDG_RUNTIME_DIR=/run/user/1000 DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/1000/bus systemctl --user daemon-reload' "$test_root/calls"
grep -q 'tester .*systemctl --user try-restart pomotui.service' "$test_root/calls"
test ! -s "$test_root/success.err"

# Do not target root or start inactive services. One user failure must not stop
# upgrades for other logged-in/lingering users, even with a wrong SUDO_USER.
: >"$test_root/calls"
POMOTUI_TEST_USERS='0 root yes active
1001 broken no active
1002 linger yes lingering'
SUDO_USER=unrelated post_upgrade >"$test_root/failure.out" 2>"$test_root/failure.err"
! grep -q -- '-u root ' "$test_root/calls"
! grep -q -- '-u unrelated ' "$test_root/calls"
grep -q 'linger .*XDG_RUNTIME_DIR=/run/user/1002 .*try-restart pomotui.service' "$test_root/calls"
grep -q 'WARNING.*Timer Service for broken.*restart' "$test_root/failure.err"
grep -q 'daemon-reload && systemctl --user restart pomotui.service' "$test_root/failure.err"
! grep -q -- 'systemctl --user restart pomotui.service' "$test_root/calls"

# No logged-in users means no running user services to restart.
: >"$test_root/calls"
POMOTUI_TEST_USERS=''
post_upgrade >"$test_root/empty.out" 2>"$test_root/empty.err"
test ! -s "$test_root/calls"
test ! -s "$test_root/empty.err"

POMOTUI_TEST_DISCOVERY_EXIT=1
export POMOTUI_TEST_DISCOVERY_EXIT
post_upgrade >"$test_root/discovery.out" 2>"$test_root/discovery.err"
grep -q 'WARNING.*user services could not be discovered' "$test_root/discovery.err"
unset POMOTUI_TEST_DISCOVERY_EXIT

# Run each actual prepare() with a stale AUR-side hook and a newer source hook.
# The install script packaged by makepkg must come from the application source.
for recipe in "$repo_root/packaging/aur/PKGBUILD" "$repo_root/packaging/aur/pomotui-git/PKGBUILD"; do
    mkdir -p "$test_root/build"
    RECIPE="$recipe" BUILD_ROOT="$test_root/build" bash -eu <<'BUILD'
source "$RECIPE"
startdir="$BUILD_ROOT/aur"
mkdir -p "$startdir"
printf 'stale hook\n' > "$startdir/pomotui.install"
case "$pkgname" in
    pomotui-git) source_dir=pomotui ;;
    *) source_dir="$pkgname-$pkgver" ;;
esac
cd "$BUILD_ROOT"
mkdir -p "$source_dir/packaging/aur"
printf 'current source hook\n' > "$source_dir/packaging/aur/pomotui.install"
prepare
cmp packaging/aur/pomotui.install "$startdir/pomotui.install"
BUILD
done

printf '%s\n' 'AUR install hook tests passed'
