# 01 — Prefactor: platform-conditional Clock and Reminder selection

**What to build:** Introduce `cfg`-gated type aliases `PlatformClock` and `PlatformDesktopReminder` in `pomotui-platform`, then replace every hardcoded `LinuxClock` usage in `pomotui-service` with `PlatformClock`. This is a pure refactor — no behavior change on Linux, no new macOS code yet. The aliases will later select `DarwinClock` / `MacDesktopReminder` when `target_os = "macos"`.

**Blocked by:** None — can start immediately.

**Status:** done

- [x] `pomotui-platform/src/lib.rs` exports `PlatformClock` and `PlatformDesktopReminder` type aliases gated on `cfg(target_os)`
- [x] `pomotui-service/src/lib.rs` imports `PlatformClock` instead of `LinuxClock` in all 4 production call sites (lines ~209, ~551, ~1639, ~1709)
- [x] The test at `pomotui-service/src/lib.rs` line ~3418 continues to use `LinuxClock` directly (it is a Linux-specific unit test)
- [x] `cargo test --workspace --all-targets --all-features` passes on Linux with zero regressions
- [x] `cargo clippy --workspace --all-targets --all-features -- -D warnings` passes
