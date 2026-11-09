# Hamster Video 0.2.0 beta: implementation and verification

## Scope and evidence

This is original Rust code around open-source downloader and media tools. It is not a decompilation of 4K Video Downloader+ and does not claim exact feature parity. No REA, Hopper, Ghidra or IDA analysis was available. No copied product code, branding, credentials or private traffic is included.

## Feature trace

- `Hamster::add` in `src/main.rs` validates links and destination, snapshots the selected compatibility, language and quality, then queues jobs. `tick` starts one worker at a time.
- `engine::download_args` constructs an argument vector for yt-dlp. URLs are separate arguments after `--`; external yt-dlp configuration is ignored. Explicit audio-language filters apply to every fallback branch, preventing substitution of another language.
- `engine::probe` reads site metadata asynchronously. `parse_info` groups available audio formats by language and identifies original tracks when reported. Generation IDs reject stale responses after the input changes. No translation is performed.
- `engine::worker` downloads media and finalizes each fetched item. Original quality retains codecs. Apple/HEVC modes inspect streams with ffprobe, copy compatible streams or encode through VideoToolbox, use AAC audio and MP4 tags `avc1`/`hvc1`, then verify the video codec before moving the final file into place. Failed conversions retain the source. Temporary conversion files are not presented as completed downloads.
- Pause terminates the worker's own process group, including helper subprocesses. Retry resumes partial downloads when the site allows it; conversion restarts.
- The top-right monitor/sun/moon button cycles Automatic → Light → Dark → Automatic. Preferences persist as JSON and Automatic follows the OS theme. The moon uses separate open arcs to avoid a degenerate closed-path rendering spike.
- Subtitles are absent from UI and downloader arguments. Old saved subtitle settings are ignored.

## Build

Version 0.2.0, minimum macOS 15, Universal `x86_64` + `arm64`. Both Rust targets were compiled on an Intel Mac. Every bundled Mach-O is checked for both architectures, a minimum OS no newer than 15, and no non-system absolute shared-library dependency.

Dependencies: egui/eframe 0.31.1, yt-dlp 2026.10.26 unpackaged macOS distribution with its universal Python libraries, Deno 2.9.8, custom FFmpeg 7.1.1, LAME 3.100, dav1d 1.5.4. FFmpeg uses VideoToolbox encoders and the portable C dav1d decoder; no GPL/nonfree libraries are enabled. FFmetadata input support is included for YouTube chapter metadata. Corresponding media-library sources, rebuild scripts and notices accompany the app. The app and altered helper binaries are ad-hoc signed.

## Verified

- Eight Rust tests passed: theme cycle/system following, old settings migration, strict language selection, track grouping/original identification, codec-copy/transcode decisions, URL argument safety, real process-group cancellation and progress parsing.
- Local integration runner passed audio-language discovery and eight download cases: H.264 MP4, MP3, DASH merge, original-codec MKV, Spanish video, Spanish MP3, unavailable-language rejection and HTTP 404 handling.
- The multilingual fixture has separate German-labelled and Spanish-labelled audio representations. The Spanish audio is a distinct 880 Hz generated tone. Tests check requests for the Spanish representation and absence of German audio requests, as well as output stream metadata.
- Live YouTube metadata identified 25 audio-language entries on `https://www.youtube.com/watch?v=0e3GPea1Tyg`, including original English, German and Spanish. The Rust worker downloaded the Spanish track with video at a 144p ceiling and produced H.264/AAC MP4. This test exposed missing FFmetadata support; the rebuilt helper fixed the chapter-metadata failure and the retry succeeded.
- Native app tests on the Intel iMac successfully converted H.264 → HEVC and HEVC → H.264. ffprobe confirmed HEVC with `hvc1` and H.264 with `avc1`, both with AAC audio. The restricted command-line environment could not create a VideoToolbox compression session; the same conversions succeeded when performed through the normal desktop app.
- The requested layout and all three appearance modes were visually checked in the native app. The user confirmed the layout. The approved icon was edited only to remove its pale cheek patches; the subtitle reads “Paste a link. Choose your quality. Keep watching offline.”

Machine-readable architecture checks and concise test results are in `verification/`. The final disk image is verified using `hdiutil verify` and has a SHA-256 checksum alongside it.

## Limits

No Apple silicon Mac was available for runtime testing. Cross-compilation and binary inspection establish included architectures, not execution on every Mac. Full 4K transfer, complete playlists, HDR tone-mapping and every possible media codec/site were not tested. AV1 decoder availability was built and inspected; this release's conversion fixtures use H.264 and HEVC. HEVC encoding depends on the Mac and content, and its output is not guaranteed smaller than an already highly compressed source. Conversion can change quality.

The app has no DRM bypass, login/cookie integration, auto-updater, subscription watcher or translation service. Site extractors may require future yt-dlp updates. The beta is not Developer ID signed or Apple-notarized.

## Beta 2 icon packaging correction

The first 0.2.0 DMG accidentally retained the earlier ICNS because iconutil rejected its input and the one-off command sequence continued. The in-app PNG was correct. The corrected build packages validated PNG representations directly into ICNS. The decoded ICNS was visually checked. Build number is 3 and the icon resource has a new name to avoid reusing the old Finder cache entry.
