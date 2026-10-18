#!/usr/bin/env bash
# Build the immutable previous stable source, never alter/rebuild the candidate.
set -Eeuo pipefail
pacman -Syu --noconfirm --needed base-devel rustup git sqlite dbus
useradd -m builder
trap 'chown -R "${CANDIDATE_UID:-1001}:${CANDIDATE_GID:-1001}" /previous' EXIT
chown -R builder:builder /previous
runuser -u builder -- bash <<'BUILD'
set -Eeuo pipefail
cd /previous/source
version=$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -1)
toolchain=$(sed -n 's/^channel = "\([^"]*\)"/\1/p' rust-toolchain.toml)
rustup toolchain install "$toolchain" --profile minimal
rustup default "$toolchain"
export PATH="$HOME/.cargo/bin:$PATH"
mkdir -p /previous/package /previous/artifacts
cp packaging/aur/PKGBUILD packaging/aur/pomotui.install /previous/package/
tar --exclude=target --exclude=.git --transform="s,^,pomotui-$version/," -czf "/previous/package/pomotui-$version.tar.gz" .
cd /previous/package
sed -i "s/^pkgver=.*/pkgver=$version/; s|^source=.*|source=(\"pomotui-$version.tar.gz\")|; s/^sha256sums=.*/sha256sums=('SKIP')/" PKGBUILD
makepkg --noconfirm --nocheck
cp pomotui-*.pkg.tar.zst /previous/artifacts/
BUILD
