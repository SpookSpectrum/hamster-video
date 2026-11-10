#!/bin/bash
set -euo pipefail
project_dir="$(cd "$(dirname "$0")/.." && pwd)"
tools_dir="${1:?Pass a UNIVERSAL Resources/tools directory as the first argument}"
tools_dir="$(cd "$tools_dir" && pwd)"
for tool in yt-dlp ffmpeg ffprobe deno; do test -x "$tools_dir/$tool"; lipo "$tools_dir/$tool" -verify_arch x86_64 arm64; done
export MACOSX_DEPLOYMENT_TARGET=15.0
export RUSTFLAGS="${RUSTFLAGS:-} --remap-path-prefix=$project_dir=/build/hamster-video"
cd "$project_dir"
rustup target add x86_64-apple-darwin aarch64-apple-darwin
cargo test --release --locked
cargo build --release --locked --target x86_64-apple-darwin --target aarch64-apple-darwin
target_dir="${CARGO_TARGET_DIR:-$project_dir/target}"
case "$target_dir" in /*) ;; *) target_dir="$project_dir/$target_dir";; esac
stage="$(mktemp -d "$project_dir/work-stage.XXXXXX")"
app="$stage/Hamster Video.app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources" "$project_dir/dist"
lipo -create "$target_dir/x86_64-apple-darwin/release/hamster-video" "$target_dir/aarch64-apple-darwin/release/hamster-video" -output "$app/Contents/MacOS/hamster-video"
python3 scripts/build-icon.py
cp assets/Info.plist "$app/Contents/"
cp assets/Hamster.icns "$app/Contents/Resources/Hamster-0.2.2.icns"
cp -R "$tools_dir" "$app/Contents/Resources/tools"
cp -R third_party/Licenses "$app/Contents/Resources/"
cp third_party/Rust-licenses.txt "$app/Contents/Resources/Licenses/"
cp README.md "$stage/Read Me.md"
# Keep the corresponding source and notices with redistributed media helpers.
python3 - "$project_dir" "$stage/HamsterVideo-source.zip" <<'PYTHON'
import pathlib,sys,zipfile
root=pathlib.Path(sys.argv[1])
with zipfile.ZipFile(sys.argv[2],'w',zipfile.ZIP_DEFLATED) as archive:
 for directory in ['src','assets','scripts','tests','third_party']:
  for path in sorted((root/directory).rglob('*')):
   if path.is_file() and '__pycache__' not in path.parts:archive.write(path,'hamster-video/'+str(path.relative_to(root)))
 for name in ['Cargo.toml','Cargo.lock','README.md','LICENSE','REPORT.md','RELEASE-NOTES.md','CONTRIBUTING.md','.gitignore']:
  archive.write(root/name,'hamster-video/'+name)
PYTHON
ln -s /Applications "$stage/Applications"
python3 scripts/verify-universal.py "$app" > dist/architecture-verification.json
codesign --force --sign - "$app"
codesign --verify --deep --strict "$app"
# A folder image preserves signed bundle metadata; makehybrid injects FinderInfo.
hdiutil create -fs HFS+ -srcfolder "$stage" -volname 'Hamster Video' -format UDZO -ov -o dist/HamsterVideo-0.2.2-beta-Universal.dmg
hdiutil verify dist/HamsterVideo-0.2.2-beta-Universal.dmg
(cd dist && shasum -a 256 HamsterVideo-0.2.2-beta-Universal.dmg > SHA256SUMS.txt)
