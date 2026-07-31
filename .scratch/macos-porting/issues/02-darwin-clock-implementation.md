# 02 — DarwinClock: macOS suspend-aware monotonic clock

**What to build:** Implement `DarwinClock` in `pomotui-platform` that satisfies the `Clock` trait on macOS. Monotonic seconds come from `mach_absolute_time` (converted via `mach_timebase_info`), wall seconds from `gettimeofday`, and boot identity from `kern.bootsid` (sysctl) or `IORegistryGetBootUUID`. Gated behind `cfg(target_os = "macos")` so Linux builds are unaffected.

**Blocked by:** 01 — Prefactor: platform-conditional Clock and Reminder selection

**Status:** done

- [x] `DarwinClock` struct implements `Clock` with `type Error = std::io::Error`
- [x] `monotonic_seconds()` returns suspend-aware uptime in whole seconds via `mach_absolute_time`
- [x] `wall_seconds()` returns Unix epoch seconds (i64) via `SystemTime`
- [x] `boot_id()` returns a stable UUID string per boot via `kern.bootsid` sysctl
- [ ] Unit tests for `DarwinClock` pass on macOS (needs macOS runner)
- [ ] `cargo check --target aarch64-apple-darwin` succeeds (needs macOS SDK)
