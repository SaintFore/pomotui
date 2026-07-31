class Pomotui < Formula
  desc "Terminal Pomodoro timer with TUI, CLI, and Waybar frontends"
  homepage "https://github.com/pomotui/pomotui"
  license "MIT"
  head "https://github.com/pomotui/pomotui.git", branch: "main"

  depends_on "rust" => :build

  def install
    system "cargo", "install", "--locked", "--root", prefix, "--path", "crates/pomotui-cli"
    system "cargo", "install", "--locked", "--root", prefix, "--path", "crates/pomotui-tui"
    system "cargo", "install", "--locked", "--root", prefix, "--path", "crates/pomotui-service"

    # launchd plists
    (prefix/"launchd").install "packaging/launchd/com.pomotui.socket.plist"
    (prefix/"launchd").install "packaging/launchd/com.pomotui.service.plist"

    # Patch binary path in service plist
    inreplace prefix/"launchd/com.pomotui.service.plist",
              "/usr/local/bin/pomotui-service",
              "#{bin}/pomotui-service"

    # Config example and animation
    (pkgshare/"config.example.toml").install "packaging/defaults/config.toml"
    (pkgshare/"building-collapse.animation").install "packaging/defaults/building-collapse.animation"
  end

  def caveats
    <<~EOS
      To start the Pomotui Timer Service with launchd:
        cp #{prefix}/launchd/com.pomotui.socket.plist ~/Library/LaunchAgents/
        cp #{prefix}/launchd/com.pomotui.service.plist ~/Library/LaunchAgents/
        launchctl load ~/Library/LaunchAgents/com.pomotui.socket.plist
        launchctl load ~/Library/LaunchAgents/com.pomotui.service.plist

      Then run: pomotui-tui
    EOS
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/pomotui --version")
  end
end
