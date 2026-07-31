# 01 — GitHub Release workflow

**What to build:** A `release.yml` GitHub Actions workflow that triggers on `v*` tags, builds release binaries for Linux (x86_64) and macOS (aarch64 + x86_64), and creates a GitHub Release with the binaries and checksums attached.

**Blocked by:** None — can start immediately.

**Status:** ready-for-agent

- [ ] Workflow builds `pomotui`, `pomotui-tui`, `pomotui-service` on `ubuntu-latest` and `macos-latest`
- [ ] macOS universal binary (or separate aarch64/x86_64) included
- [ ] GitHub Release auto-created with `tar.gz` archives per platform and SHA256 checksums
- [ ] Existing `ci.yml` still runs on push/PR
