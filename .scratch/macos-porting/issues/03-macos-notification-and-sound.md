# 03 — macOS notification and sound adapters

**What to build:** Implement `MacDesktopReminder` in `pomotui-platform` that satisfies `ReminderPort` on macOS. Notifications use `osascript -e 'display notification ...'`, sound playback uses `afplay` with volume mapped from percentage. Gated behind `cfg(target_os = "macos")` so Linux builds are unaffected.

**Blocked by:** 01 — Prefactor: platform-conditional Clock and Reminder selection

**Status:** done

- [x] `MacDesktopReminder` implements `ReminderPort` with `type Error = std::io::Error`
- [x] `notify()` shells out to `osascript` with title "Pomotui" and body "Session complete"
- [x] `play_sound()` shells out to `afplay` with volume scaling (0–100 → 0.0–1.0)
- [x] Handles missing `osascript` / `afplay` gracefully (returns error, does not panic)
- [ ] `cargo check --target aarch64-apple-darwin` succeeds (needs macOS SDK)
