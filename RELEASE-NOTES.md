# Hamster Video 1.0.1

Smaller downloads for your Mac: Hamster Video now comes in separate **Intel** and **Apple Silicon** installers. The familiar design and download features remain the same.

Requires **macOS 15 or later**. Open **Apple menu → About This Mac**: “Chip: Apple …” means Apple Silicon; “Processor: Intel …” means Intel.

| Download | DMG size | Installed app size |
| --- | ---: | ---: |
| `HamsterVideo-1.0.1-AppleSilicon.dmg` | 84.8 MB | 163.4 MB |
| `HamsterVideo-1.0.1-Intel.dmg` | 91.9 MB | 181.3 MB |

Sizes use decimal MB and installed sizes are summed file sizes. Compared with the 360.9 MB Universal 1.0.0 app, this reduces installed size by approximately **55% on Apple Silicon** and **50% on Intel**. Each installer includes only the native code needed by that Mac, including the bundled download and media tools.

Quit any older copy, open the matching DMG, drag **Hamster Video** from the left to **Applications** on the right, and open it from Applications. Both builds use the existing preferences and download-history location. The source archives are for developers; choose a DMG to install the app.

**The app is ad-hoc signed, not Apple-notarized.** If macOS blocks launch and you trust the download, attempt to open it, then use the app-specific **System Settings → Privacy & Security → Open Anyway** option. See [installation instructions](https://github.com/SpookSpectrum/hamster-video/blob/v1.0.1/INSTALLATION.md).

Validation: all 109 bundled native files per build passed architecture and signing checks. Intel testing passed 17 Rust tests, 13 packaging tests, audio-language discovery, eight deterministic download cases and desktop H.264/HEVC conversions. The maintainer reports all tests passed on an Apple Silicon MacBook. Detailed MacBook logs and machine specifications were not supplied; remaining Intel manual coverage limits are recorded in the [verification report](https://github.com/SpookSpectrum/hamster-video/blob/v1.0.1/REPORT.md).

HEVC file size and quality depend on the source. Choose Original quality to avoid re-encoding. Only download content you are permitted to save.

The previous Universal 1.0.0 release and earlier releases remain available. `SHA256SUMS.txt` contains checksums for both new installers.

## Hamster Video 1.0.0 — published release history

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
