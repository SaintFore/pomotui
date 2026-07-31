class Pomotui < Formula
  desc "Terminal Pomodoro timer with TUI, CLI, and Waybar frontends"
  homepage "https://github.com/SaintFore/pomotui"
  license "MIT"
  head "https://github.com/SaintFore/pomotui.git", branch: "main"

  depends_on "rust" => :build

  def install
    system "cargo", "install", "--locked", "--root", prefix, "--path", "crates/pomotui-cli"
    system "cargo", "install", "--locked", "--root", prefix, "--path", "crates/pomotui-tui"
    system "cargo", "install", "--locked", "--root", prefix, "--path", "crates/pomotui-service"

    # Config example and animation
    (pkgshare/"config.example.toml").install "packaging/defaults/config.toml"
    (pkgshare/"building-collapse.animation").install "packaging/defaults/building-collapse.animation"
  end

  def caveats
    <<~EOS
      To start the Pomotui Timer Service:
        brew services start pomotui

      Then run: pomotui-tui
    EOS
  end

  service do
    run [opt_bin/"pomotui-service"]
    keep_alive true
    log_path var/"log/pomotui/service.log"
    error_path var/"log/pomotui/service.log"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/pomotui --version")
  end
end
