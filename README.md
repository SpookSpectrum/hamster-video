<p align="center"><img src="assets/hamster-icon.png" width="160" alt="Hamster Video icon"></p>

# Hamster Video

A friendly macOS video downloader built in Rust, powered by yt-dlp and FFmpeg.

**0.2.2 beta · Universal Mac · macOS 15 or later.** One installer contains native code for both Intel and Apple silicon Macs.

## Download and install

Visit the [Releases page](https://github.com/SpookSpectrum/hamster-video/releases), open **Hamster Video 0.2.2 Beta**, and download **HamsterVideo-0.2.2-beta-Universal.dmg** under **Assets**. The source-code ZIP is for developers.

1. Open the DMG.
2. Drag **Hamster Video** into **Applications**, replacing the previous version if installed.
3. Open Hamster Video from Applications.

The beta is ad-hoc signed and not Apple-notarized. If macOS blocks first launch, after attempting to open it use the app-specific **Open Anyway** option in **System Settings → Privacy & Security**. No API key, Python installation or Homebrew is needed. Preferences and download history from 0.1 are retained.

## Download a video

Paste a full video URL, select quality and a destination, then click **Add to downloads**. Several URLs can be entered on separate lines. Enable **Whole playlist** only when you want the entire playlist.

**Video compatibility** offers three choices:

- **Apple compatible (H.264 / MP4)** — recommended. Produces MP4 with H.264 video and AAC audio. Compatible streams are copied; other codecs are converted.
- **Original quality** — preserves the downloaded video and audio codecs. Separate streams are merged into MKV; a single stream keeps its source container. The selected resolution ceiling still applies.
- **High efficiency (HEVC / H.265)** — produces MP4 with HEVC video, the `hvc1` tag and AAC audio. Conversion uses macOS VideoToolbox when the source is not already HEVC. Encoding support depends on the Mac. File size depends on the source and content; it is not always smaller than an already efficient source.

Conversion takes additional time and may change quality. The download shows **Converting** during this step. If conversion fails, its `.source` file is retained and the job reports an error. You can select Original quality and add the URL again.

### Audio language

After a link is pasted, Hamster checks the languages the site exposes. Choose **Original / site default**, or an available language such as **Spanish (es)**. Original is selected explicitly when the site identifies the original track; otherwise the site's default is used.

The chosen track is included with the video, or used for **MP3 audio** extraction. Hamster does not translate or generate speech. A language must actually be offered by the site. If a selected language is unavailable, the download fails rather than silently switching languages. For multiple URLs and playlists, choices shown come from the first video and apply to every item. Some sites do not expose language metadata. Use **Check tracks** to retry discovery.

Subtitles are no longer offered or downloaded.

### HEVC quality and file size

Existing HEVC video is kept without re-encoding. For other sources, Hamster measures the video bitrate separately from audio, then tests up to three short samples before converting the full video. It increases the bitrate if those samples lose too much detail, instead of using the old fixed bitrate for each resolution. Preparation is shown in the progress area and can be cancelled.

A sample similarity check is a practical safeguard, not a guarantee of visually identical output. Very compressed sources may need more space when converted to HEVC. The completed download shows its actual size and whether it is larger or smaller than the downloaded source. Choose **Original quality** to avoid re-encoding loss altogether.

## Appearance

Click the small icon at the top right to cycle **Automatic (monitor) → Light (sun) → Dark (moon) → Automatic**. Automatic follows your Mac. Your choice is remembered.

## Other features

- Quality ceilings of 480p, 720p, 1080p, 1440p and 2160p, when offered by the source.
- MP3 audio extraction, download queue, progress, speed and ETA.
- Pause and retry/resume; partial downloads are retained where supported.
- Cancel stops an active download or conversion, removes that job from the queue, and deletes its unfinished files. Completed downloads are kept.
- Conversion shows processed video time, progress and speed. HEVC encoding can take longer than downloading.

Downloads started in this version use a private temporary folder per job. Partial files left by older versions are not automatically deleted because they cannot be safely identified as belonging to a particular job.
- Local preferences and download history.

A playlist's progress reflects its current stream, not the whole playlist. Interrupted conversions restart; some sites cannot resume a partial transfer. Files with unknown resolution retain their original resolution. Website support depends on yt-dlp and may change. There is no login/cookie integration, auto-updater or livestream recorder. Download only content you are permitted to save.

## Build from source

Requires macOS, Rust and Xcode Command Line Tools:

```sh
cargo test --release --locked
cargo build --release --locked
```

For development, use the helpers from an installed Universal release, including yt-dlp's `_internal` folder:

```sh
HAMSTER_TOOLS="/Applications/Hamster Video.app/Contents/Resources/tools" cargo run --release
```

To build the Universal app and DMG:

```sh
bash scripts/build-universal-macos.sh "/Applications/Hamster Video.app/Contents/Resources/tools"
```

The script builds both Rust targets and checks every bundled Mach-O for both architectures and macOS 15 compatibility. It creates an ad-hoc signed build, not a Developer ID notarized release.

The corresponding FFmpeg, LAME and dav1d sources are in `third_party/`. Install Meson, Ninja and pkg-config/pkgconf as build tools, then rebuild both media-helper architectures:

```sh
bash scripts/build-media-universal.sh
```

This produces `work/media-universal/bin/universal/`. FFmpeg uses macOS VideoToolbox for H.264/HEVC encoding and dav1d for software AV1 decoding. It does not use GPL/nonfree codec libraries. Helper versions and checksums are in `third_party/tool-checksums.json`.

## Verification

```sh
python3 tests/smoke.py /absolute/path/to/hamster-video /absolute/path/to/tools
python3 scripts/verify-universal.py "/absolute/path/to/Hamster Video.app"
```

Tests cover theme cycling and following macOS, preference migration, language discovery and strict selection, codec conversion decisions, argument safety and cancellation. Local integration checks cover MP4, MP3, DASH merging, a two-language DASH fixture, original-codec output and expected failures. See `REPORT.md` for release-specific results and limits. Apple silicon binaries are cross-compiled and inspected; runtime testing on an Apple silicon Mac is still needed.

## Privacy and storage

Downloads default to `~/Downloads/Hamster Video`. Settings and history live in `~/Library/Application Support/Hamster Video/state.json`. The app has no telemetry and does not read browser cookies or passwords. Pasted links are sent to their media sites to inspect available formats and languages, before a download is queued. Download history can include private URLs; remove these before sharing logs.

## Contributing and credits

Bug reports and pull requests are welcome. Include the app version, macOS version, Mac architecture and steps to reproduce. See [CONTRIBUTING.md](CONTRIBUTING.md).

Original Rust code is MIT-licensed; see [LICENSE](LICENSE). Bundled tools and dependencies retain their licences in `third_party/`. Built with [egui](https://github.com/emilk/egui), [yt-dlp](https://github.com/yt-dlp/yt-dlp), [FFmpeg](https://ffmpeg.org/), [LAME](https://lame.sourceforge.io/), [dav1d](https://code.videolan.org/videolan/dav1d) and [Deno](https://deno.com/). The hamster artwork was generated with AI and refined through human feedback; its prompt is recorded in `assets/ICON-PROMPT.txt`.

Hamster Video is an independent application, not affiliated with 4K Download.
