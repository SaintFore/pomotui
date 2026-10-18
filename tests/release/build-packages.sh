#!/usr/bin/env bash
# Build real Arch packages from recorded source trees; never publish them.
set -Eeuo pipefail
cd /work
pacman -Syu --noconfirm --needed base-devel rustup git sqlite dbus sudo
useradd -m builder
chown -R builder:builder /work
runuser -u builder -- bash <<'BUILD'
set -Eeuo pipefail
rustup toolchain install 1.96.1 --profile minimal
rustup default 1.96.1
export PATH="$HOME/.cargo/bin:$PATH"
for identity in old candidate; do
    cd "/work/$identity"
    version=$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -1)
    # Candidate package identity is explicitly experimental, not a stable release.
    if [[ "$identity" == candidate ]]; then version="$version.r$(cat /work/candidate-count).g$(cat /work/candidate-short)"; fi
    mkdir -p /work/package-$identity
    cp packaging/aur/PKGBUILD packaging/aur/pomotui.install /work/package-$identity/
    tar --exclude=target --exclude=.git --transform="s,^,pomotui-$version/," -czf "/work/package-$identity/pomotui-$version.tar.gz" .
    cd /work/package-$identity
    sed -i "s/^pkgver=.*/pkgver=$version/; s|^source=.*|source=(\"pomotui-$version.tar.gz\")|; s/^sha256sums=.*/sha256sums=('SKIP')/" PKGBUILD
    makepkg --noconfirm --nocheck
    cp pomotui-*.pkg.tar.zst /work/artifacts/
done
BUILD
