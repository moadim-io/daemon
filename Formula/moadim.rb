class Moadim < Formula
  desc "Loop engine for AI agents"
  homepage "https://moadim.io"
  url "https://github.com/moadim-io/daemon.git", tag: "v3.6.0", revision: "f867b3eef21684f5616b5441af9693dceb94b006"
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
