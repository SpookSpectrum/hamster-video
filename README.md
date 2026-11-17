<p align="center"><img src="assets/hamster-icon.png" width="160" alt="Hamster Video icon"></p>

# Hamster Video

A friendly macOS video downloader built in Rust, powered by yt-dlp and FFmpeg.

**Current stable release: 1.0.1 · macOS 15 or later.** Separate native installers for Intel and Apple Silicon Macs.

<p align="center">
  <a href="https://github.com/SpookSpectrum/hamster-video/releases/download/v1.0.1/HamsterVideo-1.0.1-AppleSilicon.dmg"><img src="assets/download-apple-silicon.svg" alt="Download for Apple Silicon — Free" width="290" height="52"></a>
  <a href="https://github.com/SpookSpectrum/hamster-video/releases/download/v1.0.1/HamsterVideo-1.0.1-Intel.dmg"><img src="assets/download-intel.svg" alt="Download for Intel Mac — Free" width="290" height="52"></a>
</p>
<p align="center">
  Hamster Video is free. A few nuts help keep its developer going.<br>
  The hamster insists on a commission.
</p>
<p align="center">
  <a href="https://ko-fi.com/kimorilabs"><img src="assets/support-kimori.svg" alt="Buy the hamster some nuts on Ko-fi" width="290" height="52"></a><br>
  <sub>via Ko-fi · Kimori Labs · Entirely optional</sub>
</p>

## Download and install

Visit the [1.0.1 release](https://github.com/SpookSpectrum/hamster-video/releases/tag/v1.0.1) and choose **HamsterVideo-1.0.1-AppleSilicon.dmg** or **HamsterVideo-1.0.1-Intel.dmg** under **Assets**. Open Apple menu → About This Mac to check whether your Mac has an Apple chip or Intel processor. The source-code ZIP is for developers.

Apple Silicon: **84.8 MB download / 163.4 MB installed**. Intel: **91.9 MB download / 181.3 MB installed** (decimal MB). The previous Universal installer remains in the [1.0.0 release](https://github.com/SpookSpectrum/hamster-video/releases/tag/v1.0.0).

1. Open the DMG.
2. Drag **Hamster Video** into **Applications**, replacing the previous version if installed.
3. Open Hamster Video from Applications.

The app is ad-hoc signed and not Apple-notarized. If macOS blocks first launch, after attempting to open it use the app-specific **Open Anyway** option in **System Settings → Privacy & Security**. No API key, Python installation or Homebrew is needed. Existing preferences and download history are retained.

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

To compile and stage an architecture-specific app, install the Rust targets and use the pinned Universal helpers from 1.0.0:

```sh
rustup target add x86_64-apple-darwin aarch64-apple-darwin
bash scripts/build-macos.sh --arch intel --tools "/Applications/Hamster Video.app/Contents/Resources/tools"
bash scripts/build-macos.sh --arch apple-silicon --tools "/Applications/Hamster Video.app/Contents/Resources/tools"
```

Universal staging remains available with `--arch universal`, or through the compatibility command `bash scripts/build-universal-macos.sh "/Applications/Hamster Video.app/Contents/Resources/tools"`. These commands now stage apps only: they do not sign, install, publish, or create a DMG. Signing and installer creation are separate commands below. The installers retain the existing app-to-Applications layout.

Outputs go to `dist/<version>/Intel`, `dist/<version>/AppleSilicon`, or `dist/<version>/Universal`. Each directory contains the staged `Hamster Video.app` and separate build, architecture, source-input and original-signing-metadata reports. Existing output directories are never replaced; pass `--output-dir /path/to/a/new/directory` for another candidate. Add `--offline` when Cargo dependencies are cached. `CARGO_TARGET_DIR` is respected.

The builder checks Cargo/plist version agreement and pinned helper checksums, copies helpers before thinning, retains resources/permissions/links, and verifies every Mach-O file for exactly the requested architectures. It rejects broken/external links, newer-than-macOS-15 requirements, external runtime search paths and unresolved library dependencies. Apple system libraries are allowed without requiring standalone files because macOS can provide them through its shared cache. The verifier resolves each image's declared search paths; it fails conservatively rather than assuming inherited search paths or searching the development machine.

The original tools directory is never thinned in place. Existing helper signing information is recorded per architecture, including Deno's entitlements, for the subsequent signing step. Architecture verification does not establish that a staged app is correctly signed or that it runs on a physical Apple Silicon Mac.

### Sign and package candidate installers

Install the pinned build-only Python packages in an isolated environment. Then, for each staged variant:

```sh
python3 -m venv .venv
.venv/bin/python -m pip install -r scripts/requirements-dmg.txt
.venv/bin/python scripts/package-macos.py prepare --stage dist/1.0.1/Intel --package-dir dist/candidates/Intel
.venv/bin/python scripts/package-macos.py create-image --package-dir dist/candidates/Intel --image dist/HamsterVideo-1.0.1-Intel.dmg
```

Repeat with `AppleSilicon` and `HamsterVideo-1.0.1-AppleSilicon.dmg`. Existing output paths are refused. The prepare command signs an independent copy from the inside out, retains Deno's recorded entitlements, verifies every native file and the outer app, and saves hashes and signing evidence outside the installer. It restores standard Python framework links only after verifying the original copies are identical. It never changes the source helpers or installs the app.

The architecture builder preserves yt-dlp's embedded PyInstaller program archive, which plain `lipo` would lose from the Intel slice. The archive is compared byte-for-byte with the pinned Universal helper and hashed again after signing and from the mounted DMG. No yt-dlp or Python version change is involved.

If `hdiutil create` is unavailable in the execution environment, use Disk Utility → File → New Image → Image from Folder, copy the candidate's `image-root` preserving links to a folder named `Hamster Video`, select that folder so the mounted volume has the correct name, and save a compressed, unencrypted DMG under the exact intended filename. Verify either creation route with:

```sh
.venv/bin/python scripts/package-macos.py verify-image --package-dir dist/candidates/Intel --image dist/HamsterVideo-1.0.1-Intel.dmg
```

If shell attachment is also restricted, mount the image in Finder and perform the same mounted-content checks, recording the image-to-mount mapping and read-only status. Image verification checks integrity, mounts read-only, checks the app/Applications layout, matches the signed candidate's file hashes, permissions and links, and reruns architecture and signature checks before detaching. The result is a candidate installer; application integration tests and physical-device acceptance remain required before publication.

The corresponding FFmpeg, LAME and dav1d sources are in `third_party/`. Install Meson, Ninja and pkg-config/pkgconf as build tools, then rebuild both media-helper architectures:

```sh
bash scripts/build-media-universal.sh
```

This produces `work/media-universal/bin/universal/`. FFmpeg uses macOS VideoToolbox for H.264/HEVC encoding and dav1d for software AV1 decoding. It does not use GPL/nonfree codec libraries. Helper versions and checksums are in `third_party/tool-checksums.json`.

## Verification

```sh
python3 tests/smoke.py /absolute/path/to/hamster-video /absolute/path/to/tools
python3 tests/packaging.py
python3 scripts/macos_bundle.py verify "/absolute/path/to/Hamster Video.app" --arch intel
python3 scripts/macos_bundle.py verify "/absolute/path/to/Hamster Video.app" --arch apple-silicon
python3 scripts/verify-universal.py "/absolute/path/to/Hamster Video.app"
```

Tests cover theme cycling and following macOS, preference migration, language discovery and strict selection, codec conversion decisions, argument safety and cancellation. Local integration checks cover MP4, MP3, DASH merging, a two-language DASH fixture, original-codec output and expected failures. Packaging tests use small compiled Mach-O fixtures to check accepted architectures and rejected packaging faults. [REPORT.md](REPORT.md) records 1.0.1 results: Intel automated and desktop conversion checks passed, and the maintainer reports all MacBook Apple Silicon tests passed. It also records the limits of the available evidence.

## Privacy and storage

Downloads default to `~/Downloads/Hamster Video`. Settings and history live in `~/Library/Application Support/Hamster Video/state.json`. The app has no telemetry and does not read browser cookies or passwords. Pasted links are sent to their media sites to inspect available formats and languages, before a download is queued. Download history can include private URLs; remove these before sharing logs.

## Contributing and credits

Bug reports and pull requests are welcome. Include the app version, macOS version, Mac architecture and steps to reproduce. See [CONTRIBUTING.md](CONTRIBUTING.md).

Original Rust code is MIT-licensed; see [LICENSE](LICENSE). Bundled tools and dependencies retain their licences in `third_party/`. Built with [egui](https://github.com/emilk/egui), [yt-dlp](https://github.com/yt-dlp/yt-dlp), [FFmpeg](https://ffmpeg.org/), [LAME](https://lame.sourceforge.io/), [dav1d](https://code.videolan.org/videolan/dav1d) and [Deno](https://deno.com/). The hamster artwork was generated with AI and refined through human feedback; its prompt is recorded in `assets/ICON-PROMPT.txt`.

Hamster Video is an independent application, not affiliated with 4K Download.
