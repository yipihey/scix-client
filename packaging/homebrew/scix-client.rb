# Homebrew formula for scix-client.
#
# This file is a template for the tap repo at github.com/yipihey/homebrew-scix.
# After the first release, copy it to that tap repo as `Formula/scix-client.rb`
# and fill in the SHA-256 sums from the GitHub release artifacts.
#
# After the tap repo exists, releases will bump this formula automatically via
# the `homebrew` job in .github/workflows/release.yml.

class ScixClient < Formula
  desc "Client for the SciX / NASA ADS astronomy database — CLI, MCP server, library"
  homepage "https://github.com/yipihey/scix-client"
  version "0.3.1"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/yipihey/scix-client/releases/download/v#{version}/scix-v#{version}-aarch64-apple-darwin.tar.gz"
      sha256 "REPLACE_WITH_AARCH64_DARWIN_SHA256"
    end
    on_intel do
      url "https://github.com/yipihey/scix-client/releases/download/v#{version}/scix-v#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "REPLACE_WITH_X86_64_DARWIN_SHA256"
    end
  end

  on_linux do
    on_intel do
      url "https://github.com/yipihey/scix-client/releases/download/v#{version}/scix-v#{version}-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "REPLACE_WITH_X86_64_LINUX_SHA256"
    end
  end

  def install
    bin.install "scix"
  end

  test do
    assert_match "scix", shell_output("#{bin}/scix --version")
  end
end
