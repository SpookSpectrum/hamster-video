# Hamster Video 0.2.2 beta: implementation and verification

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

Version 0.2.2, minimum macOS 15, Universal `x86_64` + `arm64`. Both Rust targets were compiled on an Intel Mac. Every bundled Mach-O is checked for both architectures, a minimum OS no newer than 15, and no non-system absolute shared-library dependency.

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

## 0.2.1 cancellation and packaging correction

The earlier DMG retained an older ICNS after iconutil rejected its input. The running app used the correct PNG. The corrected build packages validated PNG representations directly into ICNS; build 4 uses a new resource name. The makehybrid packaging method also injected FinderInfo into signed files, so the release now uses a normal folder disk image.

Cancel signals the worker process group and direct child. Nonblocking stdout/stderr readers avoid hanging when a descendant retains an inherited pipe. Each new job owns a marked temporary folder; cancellation removes only that folder, and completed outputs never overwrite existing files. Pause retains resumable downloads. FFmpeg progress reports drive conversion status.

All 12 unit tests passed, including bounded cancellation of a stalled process, cleanup ownership, pause retention, output collision handling and conversion progress parsing. Legacy partial files are intentionally left untouched because old jobs did not record file ownership. The user's original stalled HEVC conversion was not reproduced or diagnosed from its original logs.

Native Intel GUI verification: a 600-second H.264 fixture entered HEVC conversion; progress showed 11%, 67 seconds processed and 9.5x speed. Clicking Cancel immediately returned to an empty queue, and filesystem inspection confirmed no unfinished files remained. This used a separate profile; existing user history was preserved. All eight local worker integration cases and audio-track discovery passed after the cancellation changes.

## 0.2.2 HEVC quality and bitrate policy

The old fixed per-resolution HEVC target could be much higher than a downloaded H.264 stream. The new initial target uses 80% of measured H.264 video bitrate and 100% for other source codecs. This is only a starting point: it is not a claim that those rates preserve quality. Existing HEVC is remuxed without re-encoding. Missing stream rates (common in MKV/WebM) are measured by counting compressed video packets and validating that the complete packet count was read. Container/audio bitrate is never substituted for video bitrate.

Before a full HEVC transcode, the worker tests one four-second sample for short videos, or samples at the beginning, middle and end of longer videos. If any sample scores below 0.97 SSIM, it raises the bitrate by 50% and tests again, up to five passes. The first rate whose samples all pass is used; if no rate passes, the highest tested rate is used and the engine log states that the target was not reached. Cancel interrupts sampling as well as full conversion; the job-owned folder contains all sample files. The threshold is an engineering heuristic, not an Apple recommendation, a perceptual guarantee or a full-video quality analysis. Hardware encoders and content can yield different outcomes. An HEVC file may still grow when extra bitrate is needed; the UI reports actual size rather than promising savings.

FFmpeg 7.1.1's VideoToolbox constant-quality implementation is restricted to Apple silicon, so this release uses the same source-aware sampling policy on both architectures. VideoToolbox is asked to prioritize quality over encoding speed. Frame timestamps are passed through; higher-bit-depth source conversion uses p010le instead of being forced to eight bits. HDR and dynamic metadata have not been fully validated.

Implementation references: [FFmpeg n7.1.1 VideoToolbox encoder](https://github.com/FFmpeg/FFmpeg/blob/n7.1.1/libavcodec/videotoolboxenc.c), [Apple average-bitrate property](https://developer.apple.com/documentation/videotoolbox/kvtcompressionpropertykey_averagebitrate).

### 0.2.2 final verification

17 Rust unit tests passed. The eight existing worker integration cases and track discovery passed with the rebuilt helpers. Native Intel GUI conversions covered MP4 and MKV versions of a generated motion fixture and two highly compressed real-video excerpts. All four finished with HEVC/hvc1 and AAC; sample temporary folders were removed. Frame-number alignment in SSIM is normalized to exact integer-second timestamps for comparison only, avoiding MP4/MKV timestamp-rounding artifacts. The output video retains its own original frame timing.

| Fixture | Source bytes | HEVC bytes | Size change | Full-clip SSIM |
|---|---:|---:|---:|---:|
| motion.mp4 | 514419 | 481138 | -6.47% | 0.986379 |
| motion.mkv | 512482 | 476786 | -6.97% | 0.985889 |
| real-start.mp4 | 584863 | 1439148 | +146.07% | 0.987909 |
| real.mp4 | 654746 | 1158622 | +76.96% | 0.974292 |

The former fixed-rate motion output was 1,538,099 bytes versus 514,419 source bytes (about three times larger). The new motion output is 481,138 bytes. The two heavily compressed real-video excerpts grew to preserve the sample quality target; this is deliberately reported, not concealed or described as a storage saving. SSIM does not establish perceptual equivalence across all scenes. These short fixtures do not validate every resolution, frame rate, HDR format or long video. The quality-aware encoding smoke test in tests/smoke.py requires normal desktop VideoToolbox access; equivalent final worker conversions were exercised through the native GUI in this session. The heading without its final full stop was visually verified.
