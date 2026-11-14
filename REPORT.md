# Hamster Video 1.0.1 — verification report

Release build 7; macOS 15 or later. Separate Intel and Apple Silicon installers use the same app identity and storage locations. No engine behavior or helper-version changes were introduced in this packaging release.

## Candidate identity

```text
ba9f8aaf6b4154c6d536ce98117f59536b7149f7c74b53918c8416ca90c7c0fa  HamsterVideo-1.0.1-Intel.dmg
f79b07ee643fab0014d2db9f59c806b0d9f1a9c30af56be182aed0417379f484  HamsterVideo-1.0.1-AppleSilicon.dmg
```

Each mounted DMG was verified against its signed app manifest, including file hashes, modes and symbolic links. All 109 native files contain exactly the intended architecture; signatures, required Deno entitlements, minimum macOS requirements and library dependencies were checked. yt-dlp's embedded program archive was preserved across thinning and signing. Standard Python framework links were restored only after the duplicate files were proved identical.

## Intel results — directly observed, 14 November 2026

Intel iMac, macOS 15.8.3 (24H241). Installed from the final Intel DMG, version 1.0.1/build 7. The installed app exactly matched the signed candidate manifest before and after testing.

- 17 optimized Rust unit tests passed.
- 13 packaging tests passed during candidate preparation.
- Audio-language discovery and all eight deterministic download cases passed using the installed app and bundled tools: video, MP3, DASH merging, Original, Spanish video/audio, unavailable-language rejection and missing-resource error.
- Desktop H.264-to-HEVC and HEVC-to-H.264 conversions passed; both 5-second 640×360 outputs fully decoded. HEVC SSIM was 0.986379 (threshold 0.97) and output size was 6.47% below the fixture source.
- yt-dlp 2026.10.26, Deno 2.9.8 and FFmpeg/FFprobe 7.1.1 launched from the installed bundle. Deno executed a JavaScript test.
- Existing settings were retained; two completed synthetic jobs and preferences survived restart. Clear Finished retained both completed files. Original preferences and empty history were restored afterward.

The restricted-shell HEVC test failed with VideoToolbox -12903 and did not reach H.264 conversion; both encoder paths subsequently passed through the installed desktop GUI. This is recorded as a desktop pass, not a shell encoder pass.

The old 1.0.0 app had already been removed or moved by the maintainer, and its saved history was empty. Populated old-history migration and direct replacement were therefore not exercised. Intel live YouTube/4K, manual playback/listening, extended queue/pause/resume/retry/theme checks and browser-quarantined first launch were not separately completed in this release's acceptance record. Unit tests cover some control/theme behavior but do not replace these manual scenarios.

## Apple Silicon results — maintainer reported

After receiving the test kit containing the exact Apple Silicon DMG identified above, the maintainer reported: “All the tests on my MacBook Silicon have past” and authorized Step 6 publication. This is recorded as a user-reported overall pass. MacBook model, macOS version, locally measured checksum, individual results and logs were not supplied; no independently observed per-scenario result is claimed.

The test kit supplied installation/checksum instructions, a manual checklist, local media fixtures and optional installation/automated runners. Its archived files and included DMG checksum were verified on the Intel machine before handover. Static inspection alone was not treated as Apple Silicon runtime evidence.

## Release scope and remaining limits

The maintainer authorized publication following the MacBook pass, with the Intel evidence above. No additional test results are inferred. The release retains ad-hoc signing without Apple notarization. Local installation did not show a security prompt; browser download behavior may differ. Live-site support can change with yt-dlp and source websites. HEVC size/quality depend on content.

The dependency review kept the validated Rust dependency set and pinned helpers; an optional GUI-framework upgrade and further size optimization were deferred. Candidate executable code, helper resources and installer bytes remain unchanged since validation. Release-facing documentation was finalized afterward. The website download buttons are a separate follow-up step.
