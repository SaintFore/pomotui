# 03 — Submit to AUR

**What to build:** Submit the `pomotui` and `pomotui-git` PKGBUILDs to the AUR so Arch users can install via `paru -S pomotui` or `paru -S pomotui-git`.

**Blocked by:** 02 — Commit and tag v0.1.0

**Status:** done

- [x] `pomotui-git` package pushed to AUR (builds from HEAD, no tag needed)
- [x] `pomotui` package pushed to AUR (builds from v0.1.0 release tarball)
- [x] `updpkgsums` run to get correct SHA256 for the release tarball
- [x] Both packages verified with `makepkg --printsrcinfo`
