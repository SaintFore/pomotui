# 05 — Homebrew formula

**What to build:** Create `packaging/homebrew/pomotui.rb` Homebrew formula that builds from source with `cargo build --release`, installs the three binaries to `bin/`, installs launchd plists to `prefix/`, and installs config example and animation to `share/`. Include a `caveats` block with `launchctl` instructions. The formula should work with `brew install --HEAD` for latest git.

**Blocked by:** 04 — launchd socket activation plist

**Status:** done

- [x] `packaging/homebrew/pomotui.rb` exists with `cargo install` build, launchd plist install, and caveats
- [ ] `brew install ./packaging/homebrew/pomotui.rb` builds and installs on macOS (needs macOS runner)
- [ ] After install, `pomotui status` runs and connects to the service (needs macOS runner)
- [x] `caveats` block prints launchctl enable instructions
