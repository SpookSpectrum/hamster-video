# Hamster Video 0.2.2 Beta

Universal installer for Intel and Apple silicon Macs running macOS 15 or later.

- Replaced fixed HEVC bitrates with source-aware settings and short sample quality checks before full conversion.
- Existing HEVC streams are copied without re-encoding. Audio bytes no longer inflate the estimated video bitrate.
- Shows the finished HEVC file size and its size change compared with the downloaded source.
- Keeps higher-bit-depth video in 10-bit HEVC when conversion is needed.
- Heading now reads “Bring your videos home” without a final full stop.
- Keeps the updated hamster icon, Cancel, Pause/Resume, audio languages and appearance controls.

Download **HamsterVideo-0.2.2-beta-Universal.dmg**. Quit the old app, open the DMG, and drag Hamster Video into Applications, choosing Replace.

HEVC is not guaranteed smaller or visually identical to its source. Preparation checks representative samples rather than every frame. Choose Original quality to avoid re-encoding. This beta is ad-hoc signed, not Apple-notarized. Intel runtime is tested; Apple silicon binaries are cross-compiled and inspected, but Apple silicon runtime testing is still needed.
