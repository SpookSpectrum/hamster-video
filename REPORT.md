# Evidence and implementation report

## 1. Executive summary

Delivered an original Rust macOS downloader named Hamster Video, with local settings, a queue, video quality controls, audio extraction and native application packaging. The implementation is independently written around open-source media tools. Functional parity with every feature of 4K Video Downloader+ is neither claimed nor verified.

## 2. Target profile

The installed `/Applications/4K Video Downloader+.app` identifies itself in Info.plist as version 26.3.5, bundle ID `com.openmedia.4kvideodownloaderplus`. Its main executable's dynamic-link metadata lists Qt 6.8.3 frameworks, Qt WebEngine, FFmpeg libraries and other components. This proves dependencies are linked; it does not prove the application's extraction algorithms or internal call chain.

The user's reference prompt named REA, Hopper, Ghidra and IDA. None was connected to this session. The attempt to inspect the original application's UI was interrupted. No disassembly, decompilation, private data, credentials, licence checks or private network traffic was used. No original branding or binary code is included.

The vendor's public product page describes link downloads, quality choices through 4K/8K, playlists, subtitles, remembered settings and additional features. Those are public feature claims, not measured internal behavior:
https://www.4kdownload.com/products/videodownloader-42

## 3. Implemented feature trace

`src/main.rs`: UI Add button → `Hamster::add` validates HTTP(S) links and destination → creates jobs with a snapshot of preferences → `Hamster::tick` dequeues one job → `worker` launches the bundled `yt-dlp` with an argument vector from `args` → engine downloads and optionally invokes FFmpeg → stdout/stderr readers send progress and completion messages → `tick` updates the displayed job and persists JSON.

Pause → `stop` sets a cancellation flag and terminates the worker's own Unix process group. Completion events distinguish paused, failed and complete. Resume queues the same job and its saved destination with continuation enabled. The app does not invoke a shell for downloaded URLs. It ignores external yt-dlp configuration and separates the URL from options with `--`.

Appearance selector → saved `Appearance` enum → egui theme preference. Automatic maps to the system theme. Dark and light styles are configured separately so a forced mode does not overwrite the other palette.

## 4. Evidence log and confidence

- **Proven locally:** installed target version and linked-library list, from Info.plist and `otool -L`.
- **Documented by vendor:** feature descriptions on the page above.
- **Proven for Hamster Video:** source-controlled command construction, process-group cancellation, format handling and theme behavior, plus test output in `verification/`.
- **Proven online:** Blender Foundation's Big Buck Bunny YouTube URL `https://www.youtube.com/watch?v=aqz-KE-bpKQ` was extracted and downloaded through the Rust worker at a 144p ceiling, including audio merge. Metadata listed 61 formats and a maximum height of 2160. The resulting title includes “4K”, but the test file itself was requested at 144p.
- **Not proven:** the original app's UI-to-native-function chain, exact equivalence, all supported sites, all network edge cases, full 4K transfer, playlist completion, live subtitle retrieval or third-party account access.

## 5. Reconstruction

Original interface and Rust logic; egui/eframe 0.31.1; yt-dlp 2026.10.26 unpackaged macOS distribution; Deno 2.9.8; custom FFmpeg 7.1.1 and LAME 3.100 builds. FFmpeg was compiled without GPL/nonfree features and is a separate helper executable. Its corresponding sources and rebuild script are included. A discarded FFmpeg candidate marked non-redistributable was used only as an intermediate fixture generator and is not in the delivered app.

The icon was generated with the built-in image generation tool and refined with the user's reference photo and feedback. The user selected the pale cream-peach version. The final request and tool provenance are in `assets/ICON-PROMPT.txt`.

## 6. Verification

- Rust release build: passed.
- Six tests: passed, covering URL rejection, argument-boundary safety, progress parsing, audio/playlist options, real process cancellation, theme serialization and automatic theme changes.
- Local HTTP MP4: passed after correcting unknown-height handling.
- MP3 conversion: passed.
- Separate DASH audio/video merge: passed.
- HTTP 404: failed as expected with a visible diagnostic.
- Public YouTube transfer and merge: passed at 144p.
- Native window: final Hamster Video build opened and visually inspected with the approved icon, Light appearance selected, and previously completed download history preserved. The user also reported the initial app working.

The provided standalone smoke runner could not bind its local server during a final repeat because the session network permission had expired; the prior individual end-to-end runs above succeeded. This is a test-environment limitation, not a passed rerun.

Additional final packaging checks are recorded in `verification/package-checks.txt` where available. No fabricated test or equivalence claim is intended.

## 7. Build and delivery

The disk image contains a standalone app and an Applications shortcut. Sources, tool notices and build instructions accompany the release. Copying the app is sufficient for use on the tested Intel macOS system. The app is ad-hoc signed locally; Apple Developer ID signing and notarization are not included.

## 8. Unknowns and limits

This is a first functional version. It intentionally omits account integration, browser embedding and paid-product features. Full media compatibility and ongoing site support require future maintenance of yt-dlp and its helper tools. MP4 availability depends on compatible source streams. Unknown-resolution direct files are downloaded as offered. The custom FFmpeg helper is deliberately limited to common video-container copy/merge, common audio decoding, MP3 encoding and subtitle conversion; it is not a general-purpose transcoder or livestream recorder.

Relevant upstream references:
https://github.com/yt-dlp/yt-dlp
https://github.com/emilk/egui
https://ffmpeg.org/
https://lame.sourceforge.io/
https://github.com/denoland/deno
