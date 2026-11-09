#!/bin/bash
set -euo pipefail
project_dir="$(cd "$(dirname "$0")/.." && pwd)"
tools_dir="${1:?Pass the bundled Resources/tools directory as the first argument}"
for tool in yt-dlp ffmpeg ffprobe deno; do test -x "$tools_dir/$tool"; done
cd "$project_dir"
cargo test --release --locked
cargo build --release --locked
stage="$(mktemp -d "$project_dir/work-stage.XXXXXX")"
app="$stage/Hamster Video.app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp target/release/hamster-video "$app/Contents/MacOS/"
cp assets/Info.plist "$app/Contents/"
cp assets/Hamster.icns "$app/Contents/Resources/"
cp -R "$tools_dir" "$app/Contents/Resources/tools"
cp -R third_party/Licenses "$app/Contents/Resources/"
cp third_party/Rust-licenses.txt "$app/Contents/Resources/Licenses/"
cp README.md "$stage/Read Me.md"
ln -s /Applications "$stage/Applications"
codesign --force --sign - "$app"
codesign --verify --deep --strict "$app"
mkdir -p dist
hdiutil makehybrid -hfs -hfs-volume-name 'Hamster Video' -o "$stage-image" "$stage"
hdiutil convert "$stage-image.dmg" -format UDZO -ov -o 'dist/HamsterVideo-0.1.0-Intel.dmg'
hdiutil verify 'dist/HamsterVideo-0.1.0-Intel.dmg'
