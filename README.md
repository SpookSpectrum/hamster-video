<p align="center"><img src="assets/hamster-icon.png" width="160" alt="Hamster Video icon"></p>

# Hamster Video

A friendly macOS video downloader built in Rust, powered by yt-dlp and FFmpeg.

Paste a video link, choose a format and quality, and save it for offline viewing. Hamster Video has an original desktop interface with light, dark and automatic appearance.

**Status: early beta. Current packaged build: Intel Macs running macOS 15 or later.**

## Download and install

Installers will be available on the [Releases page](https://github.com/SpookSpectrum/hamster-video/releases).

1. Download the Intel `.dmg` from a release.
2. Open it and drag **Hamster Video** into **Applications**.
3. Open the app and paste a full video URL.

The current build is ad-hoc signed, not Apple-notarized. If macOS blocks the first launch, the app-specific **Open Anyway** option is in System Settings → Privacy & Security after attempting to launch it. Do not disable Gatekeeper globally. No API key, Python installation or Homebrew is needed to run the packaged app.

## Features

- Multiple video links, one per line, processed in a download queue.
- MP4 video, or MKV for merged streams when MP4 is unchecked.
- Quality ceilings of 480p, 720p, 1080p, 1440p and 2160p (4K), depending on the source.
- MP3 audio extraction.
- Whole-playlist option and optional English/Dutch published subtitles.
- Progress, speed, ETA, pause and retry/resume.
- Remembered preferences and local download history.
- **Automatic / Light / Dark** appearance in the sidebar; Automatic follows macOS.

Single-stream downloads may retain their original container. Direct files with unknown resolution retain their original quality. Pause preserves partial files; some sources and interrupted processing stages may restart when resumed. A playlist's progress reflects its current stream, not an overall playlist percentage.

## Build from source

Requires macOS, Rust and Xcode Command Line Tools. The current packaging and bundled helper builds target Intel macOS 15+.

```sh
cargo test --release --locked
cargo build --release --locked
```

The application uses separate helper executables: `yt-dlp`, `ffmpeg`, `ffprobe`, and `deno`. For a development run, point `HAMSTER_TOOLS` at a directory containing these tools and yt-dlp's accompanying `_internal` directory:

```sh
HAMSTER_TOOLS="/Applications/Hamster Video.app/Contents/Resources/tools" cargo run --release
```

To package an application and disk image using the tools from an installed release:

```sh
bash scripts/build-macos.sh "/Applications/Hamster Video.app/Contents/Resources/tools"
```

The corresponding FFmpeg and LAME source archives are included in `third_party/`. Rebuild these helpers with:

```sh
bash scripts/build-media.sh
```

This produces `work/media-tools/`. The remaining helper versions and checksums are documented in `third_party/tool-checksums.json` and `REPORT.md`.

## Tests and current limits

Six Rust tests cover URL validation, command argument handling, progress parsing, format/playlist options, process cancellation and appearance behavior. End-to-end checks covered a local MP4, MP3 conversion, DASH audio/video merging, HTTP error reporting and a public YouTube download at 144p. Metadata exposed 2160p formats; a full 4K transfer, complete playlist and live subtitle retrieval were not tested in that session.

A deterministic local smoke runner is included:

```sh
python3 tests/smoke.py /absolute/path/to/hamster-video /absolute/path/to/tools
```

The original build session's final smoke-runner repeat was blocked by its sandbox's local-server restriction. Earlier individual end-to-end runs succeeded. See `REPORT.md` for the evidence and exact limits.

There is no built-in browser, login/cookie integration, subscription watcher, auto-updater or general-purpose transcoder. Website compatibility depends on yt-dlp and can change. Download only content you are permitted to save.

## Privacy and storage

New installations save to `~/Downloads/Hamster Video` by default. Settings and history live in `~/Library/Application Support/Hamster Video/state.json`. The first release can import settings from its earlier development name, Harbor Video. The application has no telemetry and does not read browser cookies or passwords. Submitted URLs are handled by the bundled downloader and the relevant media sites.

## Contributing

Bug reports and pull requests are welcome. Include the app version, macOS version, Intel/Apple Silicon architecture and reproducible steps. Remove private URLs, tokens and personal filesystem paths from logs before sharing them. See [CONTRIBUTING.md](CONTRIBUTING.md).

## Credits and licences

The original Rust application code is MIT-licensed; see [LICENSE](LICENSE). Bundled tools and Rust dependencies retain their own licences and notices in `third_party/`.

Built with [egui](https://github.com/emilk/egui), [yt-dlp](https://github.com/yt-dlp/yt-dlp), [FFmpeg](https://ffmpeg.org/), [LAME](https://lame.sourceforge.io/) and [Deno](https://deno.com/). The hamster artwork was generated with AI and refined through human feedback; its prompt is recorded in `assets/ICON-PROMPT.txt`.

Hamster Video is an independent application, not affiliated with 4K Download.
