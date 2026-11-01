#!/bin/sh
# Print the Homebrew formula for a release from its SHA256SUMS:
#   scripts/formula.sh <version> <SHA256SUMS> > Formula/figlens.rb
# .github/workflows/update-homebrew.yml runs it and pushes the result to runkids/homebrew-tap.
set -eu

version=$1
sums=$2
base="https://github.com/runkids/figlens/releases/download/v$version"
sum() {
    s=$(grep -- "figlens-$version-$1.tar.gz" "$sums" | cut -d' ' -f1)
    [ -n "$s" ] || { echo "formula: no figlens-$version-$1.tar.gz in $sums" >&2; exit 1; }
    echo "$s"
}
asset() {
    printf '      url "%s/figlens-%s-%s.tar.gz"\n      sha256 "%s"' "$base" "$version" "$1" "$(sum "$1")"
}

cat <<EOF
class Figlens < Formula
  desc "Figma screens for coding agents, with no Figma app, login or API limit"
  homepage "https://figlens.runkids.cc"
  version "$version"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
$(asset aarch64-apple-darwin)
    else
$(asset x86_64-apple-darwin)
    end
  end

  on_linux do
    if Hardware::CPU.arm?
$(asset aarch64-unknown-linux-musl)
    else
$(asset x86_64-unknown-linux-musl)
    end
  end

  def install
    bin.install "figlens", "figlens-embed"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/figlens --version")
    assert_match version.to_s, shell_output("#{bin}/figlens-embed --version")
  end
end
EOF
