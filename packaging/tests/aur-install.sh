#!/bin/sh
set -eu
repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
test_root=$(mktemp -d "${TMPDIR:-/tmp}/pomotui-aur-install.XXXXXX")
trap 'rm -rf "$test_root"' EXIT HUP INT TERM
cat > "$test_root/loginctl" <<'MOCK'
#!/bin/sh
printf '%s\n' "$POMOTUI_TEST_USERS"
exit "${POMOTUI_TEST_DISCOVERY_EXIT:-0}"
MOCK
cat > "$test_root/runuser" <<'MOCK'
#!/bin/sh
printf '%s\n' "$*" >>"$POMOTUI_TEST_CALLS"
case "$*" in
    *"-u broken "*) exit 1 ;;
    *"is-enabled --quiet pomotui.socket"*) exit "${POMOTUI_TEST_SOCKET_EXIT:-0}" ;;
    *"pomotui.service -p FragmentPath"*) printf '%s\n' "${POMOTUI_TEST_FRAGMENT:-/usr/lib/systemd/user/pomotui.service}" ;;
    *"pomotui.socket -p FragmentPath"*) echo /usr/lib/systemd/user/pomotui.socket ;;
    *"DropInPaths"*) printf '%s\n' "${POMOTUI_TEST_DROPINS:-}" ;;
    *"MainPID"*) echo $$ ;;
    *"reset-failed"*) rm -f "$POMOTUI_TEST_LIMIT" ;;
    *"restart pomotui.service"*) test ! -f "$POMOTUI_TEST_LIMIT" || exit 1; printf 'active\n' >"$POMOTUI_TEST_STATE" ;;
    *"/usr/bin/pomotui status"*) printf '{"snapshot":{"durable_health":{"state":"%s"}}}\n' "${POMOTUI_TEST_HEALTH_STATE:-healthy}"; exit "${POMOTUI_TEST_HEALTH_EXIT:-0}" ;;
esac
MOCK
cat > "$test_root/sha256sum" <<'MOCK'
#!/bin/sh
case "$1" in /proc/*) echo "${POMOTUI_TEST_RUNNING_HASH:-candidate}  $1" ;; *) echo "candidate  $1" ;; esac
MOCK
cat > "$test_root/cargo" <<'MOCK'
#!/bin/sh
exit 0
MOCK
cat > "$test_root/rustc" <<'MOCK'
#!/bin/sh
printf 'host: x86_64-unknown-linux-gnu\n'
MOCK
chmod +x "$test_root/loginctl" "$test_root/runuser" "$test_root/sha256sum" "$test_root/cargo" "$test_root/rustc"
PATH="$test_root:$PATH"
POMOTUI_TEST_CALLS="$test_root/calls"
POMOTUI_TEST_USERS='1000 tester no active'
POMOTUI_TEST_STATE="$test_root/state"
POMOTUI_TEST_LIMIT="$test_root/limit"
touch "$POMOTUI_TEST_LIMIT"
export PATH POMOTUI_TEST_CALLS POMOTUI_TEST_USERS POMOTUI_TEST_STATE POMOTUI_TEST_LIMIT
. "$repo_root/packaging/aur/pomotui.install"
unset SUDO_USER XDG_RUNTIME_DIR DBUS_SESSION_BUS_ADDRESS
post_upgrade > "$test_root/success.out" 2> "$test_root/success.err"
test "$(cat "$POMOTUI_TEST_STATE")" = active
test ! -s "$test_root/success.err"
grep -q 'XDG_RUNTIME_DIR=/run/user/1000 DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/1000/bus' "$test_root/calls"
# Disabled intent never starts or enables the service/socket.
: > "$test_root/calls"
POMOTUI_TEST_SOCKET_EXIT=1 post_upgrade 2> "$test_root/disabled.err"
! grep -q 'restart\|reset-failed\|enable --now' "$test_root/calls"
test ! -s "$test_root/disabled.err"
# User-unit conflict is precise and leaves recovery untouched.
: > "$test_root/calls"
POMOTUI_TEST_FRAGMENT=/home/tester/.config/systemd/user/pomotui.service post_upgrade 2> "$test_root/conflict.err"
grep -q '/home/tester/.config/systemd/user/pomotui.service' "$test_root/conflict.err"
! grep -q 'restart\|reset-failed' "$test_root/calls"
POMOTUI_TEST_DROPINS=/home/tester/.config/systemd/user/pomotui.service.d/source.conf post_upgrade 2> "$test_root/dropin.err"
grep -q 'DropInPaths=/home/tester/.config/systemd/user/pomotui.service.d/source.conf' "$test_root/dropin.err"
POMOTUI_TEST_HEALTH_STATE=failed post_upgrade 2> "$test_root/degraded.err"
grep -q 'public status health check failed' "$test_root/degraded.err"
POMOTUI_TEST_HEALTH_EXIT=1 post_upgrade 2> "$test_root/health.err"
grep -q 'public status health check failed' "$test_root/health.err"
POMOTUI_TEST_RUNNING_HASH=old post_upgrade 2> "$test_root/executable.err"
grep -q 'running executable conflict' "$test_root/executable.err"
POMOTUI_TEST_USERS='0 root yes active
1001 broken no active
1002 linger yes lingering'
: > "$test_root/calls"
SUDO_USER=unrelated post_upgrade 2> "$test_root/users.err"
! grep -q -- '-u root\|-u unrelated' "$test_root/calls"
grep -q 'daemon-reload failed' "$test_root/users.err"
grep -q 'linger .*restart pomotui.service' "$test_root/calls"
POMOTUI_TEST_USERS=''
: > "$test_root/calls"
post_upgrade 2> "$test_root/empty.err"
test ! -s "$test_root/calls"
POMOTUI_TEST_DISCOVERY_EXIT=1 post_upgrade 2> "$test_root/discovery.err"
grep -q 'user-manager discovery failed' "$test_root/discovery.err"
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
