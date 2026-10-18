#!/usr/bin/env bash
# Invoked only inside the disposable Arch build container by candidate.py.
set -Eeuo pipefail
# Preserve host ownership even when the build fails (hosted runners need not use UID 1000).
trap 'chown -R "$CANDIDATE_UID:$CANDIDATE_GID" /candidate' EXIT
pacman -Syu --noconfirm --needed base-devel rustup git python sqlite dbus
useradd --create-home builder
chown -R builder:builder /candidate
runuser -u builder -- bash <<'BUILD'
set -Eeuo pipefail
export PATH="$HOME/.cargo/bin:$PATH"
cd /candidate
python - <<'PY'
import json
import subprocess
m = json.load(open('candidate.json'))
subprocess.run(['rustup', 'toolchain', 'install', m['toolchain'], '--profile', 'minimal'], check=True)
subprocess.run(['rustup', 'target', 'add', '--toolchain', m['toolchain'], m['target']], check=True)
PY
for channel in aur-stable aur-git; do
    [[ -d "/candidate/$channel" ]] || continue
    cd "/candidate/$channel"
    # Freeze only this validation build; the distributed Git recipe remains moving HEAD.
    if [[ "$channel" == aur-git ]]; then
        [[ -d pomotui ]] || git clone --bare /candidate/source.bundle pomotui
        commit=$(python -c 'import json; print(json.load(open("/candidate/candidate.json"))["source_commit"])')
        git -C pomotui update-ref refs/heads/candidate "$commit"
        git -C pomotui symbolic-ref HEAD refs/heads/candidate
        git -C pomotui remote set-url origin https://github.com/SaintFore/pomotui.git
    else
        archive=$(python -c 'import json; print(json.load(open("/candidate/candidate.json"))["source_archive"])')
        version=$(python -c 'import json; print(json.load(open("/candidate/candidate.json"))["version"])')
        cp "/candidate/$archive" "pomotui-$version.tar.gz"
    fi
    makepkg --printsrcinfo > generated.SRCINFO
    diff -u .SRCINFO generated.SRCINFO
    makepkg --verifysource --holdver
    # Build the real generated recipe, including check(), with the selected toolchain.
    makepkg --noconfirm --holdver --force
    diff -u .SRCINFO <(makepkg --printsrcinfo)
    if [[ "$channel" == aur-git ]]; then
        [[ "$(git -C src/pomotui rev-parse HEAD)" == "$commit" ]]
    fi
    mkdir -p /candidate/built/"$channel"
    cp -- *.pkg.tar.zst /candidate/built/"$channel"/
done
BUILD
