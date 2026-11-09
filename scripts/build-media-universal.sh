#!/bin/bash
set -euo pipefail
project_dir="$(cd "$(dirname "$0")/.." && pwd)"
media_work="${HAMSTER_MEDIA_WORK:-$project_dir/work/media-universal}"
mkdir -p "$media_work"
media_work="$(cd "$media_work" && pwd)"
export MACOSX_DEPLOYMENT_TARGET=15.0
host_arch="$(uname -m)"
case "$host_arch" in arm64) host_triplet=aarch64-apple-darwin;; x86_64) host_triplet=x86_64-apple-darwin;; *) exit 2;; esac
architectures="${*:-x86_64 arm64}"
for arch in $architectures; do
 case "$arch" in arm64) triplet=aarch64-apple-darwin; ffarch=aarch64;; x86_64) triplet=x86_64-apple-darwin; ffarch=x86_64;; *) echo "Unsupported architecture: $arch" >&2; exit 2;; esac
 arch_dir="$media_work/$arch"
 mkdir -p "$arch_dir" "$media_work/bin/$arch"
 tar -xf "$project_dir/third_party/lame-3.100.tar.gz" -C "$arch_dir"
 tar -xf "$project_dir/third_party/ffmpeg-7.1.1.tar.xz" -C "$arch_dir"
 cd "$arch_dir/lame-3.100"
 CC="clang -arch $arch" CFLAGS="-O2 -arch $arch" LDFLAGS="-arch $arch" ./configure --build="$host_triplet" --host="$triplet" --prefix=/usr/local --disable-shared --enable-static --disable-frontend
 make -j6
 make install DESTDIR="$arch_dir/stage"
 tar -xf "$project_dir/third_party/dav1d-1.5.4.tar.xz" -C "$arch_dir"
 cat > "$arch_dir/dav1d-cross.ini" <<EOF
[binaries]
c = ['clang', '-arch', '$arch']
ar = 'ar'
strip = 'strip'
[host_machine]
system = 'darwin'
cpu_family = '$ffarch'
cpu = '$ffarch'
endian = 'little'
[properties]
needs_exe_wrapper = true
EOF
 # Portable C avoids a separate NASM dependency; the two architectures use the same decoder.
 meson setup "$arch_dir/dav1d-build" "$arch_dir/dav1d-1.5.4" --cross-file "$arch_dir/dav1d-cross.ini" --prefix=/usr/local --libdir=lib --default-library=static --buildtype=release -Denable_asm=false -Denable_tools=false -Denable_tests=false
 meson compile -C "$arch_dir/dav1d-build" -j 6
 DESTDIR="$arch_dir/stage" meson install -C "$arch_dir/dav1d-build"
 export PKG_CONFIG_LIBDIR="$arch_dir/stage/usr/local/lib/pkgconfig"
 export PKG_CONFIG_SYSROOT_DIR="$arch_dir/stage"
 cd "$arch_dir/ffmpeg-7.1.1"
 ./configure --prefix=/usr/local --cc="clang -arch $arch" --arch="$ffarch" --target-os=darwin --enable-cross-compile --extra-cflags=-I../stage/usr/local/include --extra-ldflags=-L../stage/usr/local/lib --enable-libmp3lame --enable-libdav1d --enable-videotoolbox --disable-everything --disable-autodetect --disable-x86asm --disable-doc --disable-debug --enable-ffmpeg --enable-ffprobe --enable-static --disable-shared --enable-protocol=file,pipe --enable-demuxer=ffmetadata,mov,matroska,mp3,aac,wav,ogg,flac,mpegts,srt,webvtt,ass --enable-muxer=matroska,mp4,mp3,ipod,mov,webm,srt --enable-decoder=aac,mp3,opus,vorbis,flac,pcm_s16le,alac,h264,hevc,vp8,vp9,mpeg4,mpeg2video,prores,mjpeg,libdav1d --enable-encoder=libmp3lame,aac,h264_videotoolbox,hevc_videotoolbox --enable-parser=aac,mpegaudio,opus,vorbis,h264,hevc,vp9,av1 --enable-bsf=aac_adtstoasc,extract_extradata,h264_mp4toannexb,hevc_mp4toannexb --enable-filter=aresample,anull,scale,format,fps,transpose
 make -j6
 cp ffmpeg ffprobe "$media_work/bin/$arch/"
 lipo "$media_work/bin/$arch/ffmpeg" -verify_arch "$arch"
 lipo "$media_work/bin/$arch/ffprobe" -verify_arch "$arch"
done
if [[ -f "$media_work/bin/x86_64/ffmpeg" && -f "$media_work/bin/arm64/ffmpeg" ]]; then
 mkdir -p "$media_work/bin/universal"
 for tool in ffmpeg ffprobe; do
  lipo -create "$media_work/bin/x86_64/$tool" "$media_work/bin/arm64/$tool" -output "$media_work/bin/universal/$tool"
  codesign --force --sign - "$media_work/bin/universal/$tool"
 done
fi
