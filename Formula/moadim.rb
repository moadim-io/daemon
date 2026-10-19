class Moadim < Formula
  desc "Loop engine for AI agents"
  homepage "https://moadim.io"
  url "https://github.com/moadim-io/daemon.git", tag: "v3.2.5", revision: "28e3f67994d78e0d94ec6e74216f59051d0ccb5b"
  license "MIT"
  head "https://github.com/moadim-io/daemon.git", branch: "main"

  depends_on "rust" => :build
  depends_on "tmux"

  def install
    system "cargo", "install", "--locked", "--path", ".", "--root", prefix
    man1.install "docs/moadim.1"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/moadim --version")
  end
end
