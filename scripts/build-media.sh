#!/bin/bash
set -euo pipefail
project_dir="$(cd "$(dirname "$0")/.." && pwd)"
media_work="$project_dir/work/media-build"
prefix="$media_work/prefix"
mkdir -p "$media_work" "$project_dir/work/media-tools"
tar -xf "$project_dir/third_party/lame-3.100.tar.gz" -C "$media_work"
tar -xf "$project_dir/third_party/ffmpeg-7.1.1.tar.xz" -C "$media_work"
export MACOSX_DEPLOYMENT_TARGET=15.0
cd "$media_work/lame-3.100"
./configure --prefix="$prefix" --disable-shared --enable-static --disable-frontend
make -j6
make install
cd "$media_work/ffmpeg-7.1.1"
./configure --prefix="$prefix" --extra-cflags="-I$prefix/include" --extra-ldflags="-L$prefix/lib" --enable-libmp3lame --disable-everything --disable-autodetect --disable-x86asm --disable-doc --disable-debug --enable-ffmpeg --enable-ffprobe --enable-static --disable-shared --enable-protocol=file,pipe --enable-demuxer=mov,matroska,mp3,aac,wav,ogg,flac,mpegts,srt,webvtt,ass --enable-muxer=matroska,mp4,mp3,ipod,mov,webm,srt --enable-decoder=aac,mp3,opus,vorbis,flac,pcm_s16le,alac,ass,subrip,webvtt --enable-encoder=libmp3lame,subrip --enable-parser=aac,mpegaudio,opus,vorbis,h264,hevc,vp9,av1 --enable-bsf=aac_adtstoasc,extract_extradata,h264_mp4toannexb,hevc_mp4toannexb --enable-filter=aresample,anull
make -j6
make install
cp "$prefix/bin/ffmpeg" "$prefix/bin/ffprobe" "$project_dir/work/media-tools/"
