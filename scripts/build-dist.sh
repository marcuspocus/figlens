#!/bin/sh
# Cross-build release archives into dist/. Runs inside Dockerfile.dist's toolchain
# (scripts/dist.sh, locally and in .github/workflows/release.yaml). Upload dist/ to a GitHub release; scripts/install.sh reads it from there.
set -eu

cd "$(dirname "$0")/.."
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)
targets=${FIGLENS_TARGETS:-"aarch64-apple-darwin x86_64-apple-darwin x86_64-unknown-linux-musl aarch64-unknown-linux-musl x86_64-pc-windows-gnu"}
output=${FIGLENS_DIST_DIR:-dist}
test -f skills/figlens/SKILL.md
test -f skills/figlens/references/audit.md
test -f skills/figlens/references/planning.md
test -f skills/figlens/references/planning-details.md
test -f skills/figlens/references/yolo.md
test -f skills/figlens/references/search.md
mkdir -p "$output"
set --
for target in $targets; do
    cargo zigbuild --locked --release --target "$target" --target-dir target/dist-build --bin figlens
    release="target/dist-build/$target/release"
    case "$target" in *windows*) exe=.exe ;; *) exe= ;; esac
    core=$(sha256sum "$release/figlens$exe")
    # The optional model runtime ships beside the model-free core; building it must leave the core as built.
    cargo zigbuild --locked --release --target "$target" --target-dir target/dist-build --features embed --bin figlens-embed
    test "$(sha256sum "$release/figlens$exe")" = "$core" || { echo "building figlens-embed changed figlens$exe for $target" >&2; exit 1; }
    case "$target" in
    *windows*)
        # Windows users expect a zip (Expand-Archive); the image has python3 but no zip.
        test -f "$release/figlens.exe"
        test -f "$release/figlens-embed.exe"
        archive="figlens-$version-$target.zip"
        python3 - "$output/$archive" "$release" <<'PY'
import pathlib, sys, zipfile
with zipfile.ZipFile(sys.argv[1], "w", zipfile.ZIP_DEFLATED) as z:
    for name in ["figlens.exe", "figlens-embed.exe"]:
        z.write(pathlib.Path(sys.argv[2], name), name)
    for file in sorted(pathlib.Path("skills/figlens").rglob("*")):
        if file.is_file():
            z.write(file, file.as_posix())
PY
        ;;
    *)
        test -x "$release/figlens"
        test -x "$release/figlens-embed"
        archive="figlens-$version-$target.tar.gz"
        tar -czf "$output/$archive" -C "$release" figlens figlens-embed -C "$PWD" skills/figlens
        ;;
    esac
    set -- "$@" "$archive"
    # Free each target's output but keep the host build scripts and proc macros for the next.
    rm -rf "target/dist-build/$target"
done
cd "$output"
sha256sum "$@" > SHA256SUMS
sha256sum -c SHA256SUMS
ls -lh "$@" SHA256SUMS
