# Hamster Video 1.0.0

The first stable release of Hamster Video: a completely free video downloader with a simple, native Mac experience.

- Download videos up to 4K when offered by the source, or extract MP3 audio.
- Select available audio-language tracks.
- Choose H.264/MP4, Original quality, or HEVC/H.265, with resolution controls.
- Universal support for Intel and Apple Silicon Macs running macOS 15 or later.
- Queue downloads, pause, resume, cancel, retry and clear completed history.
- Automatic, Light and Dark appearance, with a refined crescent moon icon.
- Clean drag-and-drop installer; the hamster artwork and familiar app design are preserved.

Download **HamsterVideo-1.0.0-Universal.dmg**, quit the older app, open the DMG and drag Hamster Video from the left to Applications on the right. Open it from Applications.

**The app is ad-hoc signed, not Apple-notarized.** If macOS blocks first launch, attempt to open it, then use **System Settings → Privacy & Security → Open Anyway** for Hamster Video if you trust the download.

Intel launch, 17 release unit tests, eight local download tests and audio-language discovery passed. Both architectures were verified in all 111 bundled executable components. This exact build still needs physical Apple Silicon runtime testing; full 4K and live-site downloads were not exercised in this verification run. See [REPORT.md](https://github.com/SpookSpectrum/hamster-video/blob/v1.0.0/REPORT.md) for details. HEVC file size and quality depend on the source; choose Original quality to avoid re-encoding.

SHA-256:

```text
ebabd2fae2523b83c065357b834c371b5bd965ee21f68ef853951b6d31b9aee8  HamsterVideo-1.0.0-Universal.dmg
```
