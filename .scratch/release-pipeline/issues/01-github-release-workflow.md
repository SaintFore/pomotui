# 01 — GitHub Release workflow

**What to build:** A `release.yml` GitHub Actions workflow that triggers on `v*` tags, builds release binaries for Linux (x86_64) and macOS (aarch64 + x86_64), and creates a GitHub Release with the binaries and checksums attached.

**Blocked by:** None — can start immediately.

**Status:** done

- [x] Workflow builds `pomotui`, `pomotui-tui`, `pomotui-service` on `ubuntu-latest` and `macos-latest`
- [x] macOS aarch64 + x86_64 separate builds included
- [x] GitHub Release auto-created with `tar.gz` archives per platform and SHA256 checksums
- [x] Existing `ci.yml` still runs on push/PR
