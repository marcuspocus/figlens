// Where the figlens engine comes from. No Herdr imports, so `node --test` can run it.
// The person never installs a CLI: a figlens on PATH wins, then the npm launcher's cache
// for VERSION (shared with `npx @runkids/figlens`), else one click downloads this
// version's release archive into that same cache after checking it against SHA256SUMS.

export const VERSION = "0.13.0"; // x-release-please-version

const OS = { macos: "apple-darwin", linux: "unknown-linux-musl" };
const ARCH = { arm64: "aarch64", aarch64: "aarch64", x64: "x86_64", x86_64: "x86_64" };

// The release archive for Herdr's platform name and the process arch, or null when
// there is no build (Windows is not supported by this mod yet).
export function asset(platform, arch, version = VERSION) {
  if (!OS[platform] || !ARCH[arch]) return null;
  return `figlens-${version}-${ARCH[arch]}-${OS[platform]}.tar.gz`;
}

export const releaseBase = (version = VERSION) =>
  `https://github.com/runkids/figlens/releases/download/v${version}`;

// The npm launcher's cache folder for a version, under the cache root.
export const cacheDir = (root, version = VERSION) => `${root}/figlens/npm/${version}`;

// sh -c INSTALL <name> <dir> <archive> <base>: download, verify, extract next to the
// cache, then rename into place so a half-written binary is never run.
export const INSTALL = `set -e
dir=$1; archive=$2; base=$3
parent=$(dirname "$dir"); mkdir -p "$parent"
tmp=$(mktemp -d "$parent/.tmp-XXXXXX")
trap 'rm -rf "$tmp"' EXIT
curl -fsSL --retry 2 -o "$tmp/$archive" "$base/$archive"
curl -fsSL --retry 2 -o "$tmp/SHA256SUMS" "$base/SHA256SUMS"
want=$(awk -v f="$archive" '$2 == f || $2 == "*" f { print $1 }' "$tmp/SHA256SUMS")
if command -v sha256sum >/dev/null 2>&1; then got=$(sha256sum "$tmp/$archive" | cut -d' ' -f1)
else got=$(shasum -a 256 "$tmp/$archive" | cut -d' ' -f1); fi
if [ -z "$want" ] || [ "$want" != "$got" ]; then
  echo "$archive does not match the release's SHA256SUMS" >&2; exit 1
fi
tar -xzf "$tmp/$archive" -C "$tmp" figlens figlens-embed
rm "$tmp/$archive" "$tmp/SHA256SUMS"
# A folder without the binary (emptied, or a deleted binary) is replaced, never nested into.
[ -x "$dir/figlens" ] || { rm -rf "$dir"; mv "$tmp" "$dir"; }
`;
