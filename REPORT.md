# Hamster Video 1.0.0 verification

Verified November 13, 2026 on an Intel Mac running macOS 15.8.3.

## Final artifact

- `HamsterVideo-1.0.0-Universal.dmg`, 172934686 bytes.
- Version 1.0.0, build 6, minimum macOS 15.0, production identifier `local.hamster.video`.
- Universal x86_64 and arm64 app and helpers: all 111 Mach-O components passed architecture, deployment-target and system-library dependency checks.
- Ad-hoc signature: deep/strict verification passed. No Apple notarization.
- Compressed APFS DMG passed disk-image integrity verification and opened in Finder.
- Only the app, Applications symlink and hidden Finder layout metadata appear at the volume root. The original hamster icon is byte-identical.
- Drag-to-Applications installation passed. All 148 installed files/links match the mounted image, including file contents and permissions.
- Installed production GUI launched and displayed Hamster Video 1.0.0 and its ready state.

```text
ebabd2fae2523b83c065357b834c371b5bd965ee21f68ef853951b6d31b9aee8  HamsterVideo-1.0.0-Universal.dmg
```

## Automated checks on the final source and installed executable

- 17 optimized release unit tests passed; no failures.
- Eight local integration cases passed: MP4, MP3, DASH merge, original quality, Spanish video, Spanish audio, expected unavailable-language rejection and expected missing-file failure.
- Audio-language discovery passed for the German/Spanish DASH fixture.
- Checksum generated and independently verified. Principal helper hashes match `third_party/tool-checksums.json`.
- Release records are in `verification/1.0.0/`. Older records elsewhere in `verification/` describe earlier beta builds.

## Earlier release GUI checks

An isolated release test bundle verified H.264-to-HEVC conversion, HEVC-to-H.264 conversion, Spanish Original-quality output, MP3 retry, queue progression, active pause/resume and cancellation, all appearance modes, restored preferences/history and Clear Finished preserving completed files. All five output files passed stream inspection and full decoding. These checks preceded final production packaging; they are not claimed as new tests of the installed final bundle.

## Coverage limits

- No physical Apple Silicon Mac was available for this exact build. Cross-compilation and static inspection do not substitute for runtime testing. Earlier successful testing reported by the maintainer is acknowledged separately.
- Local deterministic fixtures are 640×360. Full 4K, long-form, HDR, playlists, sustained performance and live external-site extraction were not tested in this run.
- Optional command-line VideoToolbox conversion testing could not create a compression session in the restricted execution environment. Equivalent conversions succeeded through the earlier desktop release test bundle.
- The local installer launch did not reproduce browser-download quarantine behaviour. Gatekeeper may block a downloaded copy because this app is not notarized; see INSTALLATION.md.
- HEVC size and visual similarity are source-dependent, not guaranteed improvements.

## Packaging route

The packaging script completed release tests, both target builds, bundling, architecture verification and ad-hoc signing. The restricted shell blocked disk-image creation with `Device not configured`, so Disk Utility created the delivered compressed APFS image from that same completed staging folder. The script retains its HFS+ route for unrestricted local builds. Both formats support the macOS 15 minimum.

No release-blocking product defect was found in the executed checks.
