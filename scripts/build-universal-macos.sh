#!/bin/bash
set -euo pipefail
project_dir="$(cd "$(dirname "$0")/.." && pwd)"
tools_dir="${1:?Pass a UNIVERSAL Resources/tools directory as the first argument}"
tools_dir="$(cd "$tools_dir" && pwd)"
for tool in yt-dlp ffmpeg ffprobe deno; do test -x "$tools_dir/$tool"; lipo "$tools_dir/$tool" -verify_arch x86_64 arm64; done
export MACOSX_DEPLOYMENT_TARGET=15.0
export RUSTFLAGS="${RUSTFLAGS:-} --remap-path-prefix=$project_dir=/build/hamster-video"
cd "$project_dir"
python3 -c "import ds_store" # See requirements-dmg.txt for build-only dependencies.
rustup target add x86_64-apple-darwin aarch64-apple-darwin
cargo test --release --locked
cargo build --release --locked --target x86_64-apple-darwin --target aarch64-apple-darwin
target_dir="${CARGO_TARGET_DIR:-$project_dir/target}"
case "$target_dir" in /*) ;; *) target_dir="$project_dir/$target_dir";; esac
stage="$(mktemp -d "$project_dir/work-stage.XXXXXX")"
app="$stage/Hamster Video.app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources" "$project_dir/dist"
lipo -create "$target_dir/x86_64-apple-darwin/release/hamster-video" "$target_dir/aarch64-apple-darwin/release/hamster-video" -output "$app/Contents/MacOS/hamster-video"
cp assets/Info.plist "$app/Contents/"
cp assets/Hamster.icns "$app/Contents/Resources/Hamster-1.0.0.icns"
cp -R "$tools_dir" "$app/Contents/Resources/tools"
cp -R third_party/Licenses "$app/Contents/Resources/"
cp third_party/Rust-licenses.txt "$app/Contents/Resources/Licenses/"
ln -s /Applications "$stage/Applications"
python3 scripts/dmg-layout.py "$stage/.DS_Store"
python3 scripts/verify-universal.py "$app" > dist/architecture-verification.json
codesign --force --sign - "$app"
codesign --verify --deep --strict "$app"
# Keep only the app, Applications link and hidden Finder layout in the image.
# A folder image preserves signed bundle metadata; avoid makehybrid.
hdiutil create -fs HFS+ -srcfolder "$stage" -volname 'Hamster Video' -format UDZO -ov -o dist/HamsterVideo-1.0.0-Universal.dmg
hdiutil verify dist/HamsterVideo-1.0.0-Universal.dmg
(cd dist && shasum -a 256 HamsterVideo-1.0.0-Universal.dmg > SHA256SUMS.txt)
