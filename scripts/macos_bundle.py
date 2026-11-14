#!/usr/bin/env python3
"""Build/stage macOS apps and verify architecture plus bundled dependencies.

Staging deliberately does not sign or package a DMG. Sign changed nested code
before the outer app, then verify signatures in the packaging step.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import plistlib
import re
import shutil
import stat
import subprocess
import tempfile
from pyinstaller_payload import archive_hash, preserve_archive

VARIANTS = {
    'intel': ('Intel', {'x86_64': 'x86_64-apple-darwin'}),
    'apple-silicon': ('AppleSilicon', {'arm64': 'aarch64-apple-darwin'}),
    'universal': ('Universal', {'x86_64': 'x86_64-apple-darwin', 'arm64': 'aarch64-apple-darwin'}),
}
MAGIC = {bytes.fromhex(h) for h in ('cefaedfe', 'feedface', 'cffaedfe', 'feedfacf', 'cafebabe', 'bebafeca', 'cafebabf', 'bfbafeca')}
LOADS = {'LC_LOAD_DYLIB', 'LC_LOAD_WEAK_DYLIB', 'LC_REEXPORT_DYLIB', 'LC_LOAD_UPWARD_DYLIB', 'LC_LAZY_LOAD_DYLIB'}
TOOLS = ('yt-dlp', 'ffmpeg', 'ffprobe', 'deno')


def output(*args):
    return subprocess.check_output([str(a) for a in args], text=True).strip()


def sha256(path):
    digest = hashlib.sha256()
    with path.open('rb') as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b''):
            digest.update(block)
    return digest.hexdigest()


def macho(path):
    if path.is_symlink() or not path.is_file():
        return False
    with path.open('rb') as handle:
        return handle.read(4) in MAGIC


def inside(path, root):
    try:
        path.relative_to(root)
        return True
    except ValueError:
        return False


def check_links(root):
    for path in root.rglob('*'):
        if path.is_symlink():
            try:
                target = path.resolve(strict=True)
            except (OSError, RuntimeError) as error:
                raise ValueError(f'Broken or cyclic link: {path}') from error
            if not inside(target, root):
                raise ValueError(f'Link escapes bundle: {path}')


def inventory(root):
    result = {}
    for path in sorted(root.rglob('*')):
        name = str(path.relative_to(root))
        if path.is_symlink():
            result[name] = {'link': os.readlink(path)}
        elif path.is_file():
            result[name] = {'sha256': sha256(path), 'bytes': path.stat().st_size,
                            'mode': stat.S_IMODE(path.stat().st_mode)}
    return result


def build_inputs(project):
    paths = [project / name for name in ('Cargo.toml', 'Cargo.lock', 'assets/Info.plist',
             'assets/Hamster.icns', 'third_party/tool-checksums.json', 'third_party/Rust-licenses.txt')]
    paths += list((project / 'src').rglob('*.rs'))
    paths += [p for p in (project / 'scripts').iterdir() if p.suffix in ('.py', '.sh', '.txt')]
    paths += [p for p in (project / 'third_party/Licenses').rglob('*') if p.is_file()]
    return {str(p.relative_to(project)): sha256(p) for p in sorted(paths)}


def load_commands(path, arch):
    text = output('otool', '-arch', arch, '-l', path)
    minimum = None
    dependencies, rpaths = [], []
    for block in text.split('Load command '):
        cmd = re.search(r'\bcmd (LC_\S+)', block)
        if not cmd:
            continue
        cmd = cmd.group(1)
        if cmd == 'LC_BUILD_VERSION':
            platform = re.search(r'\bplatform (\S+)', block)
            if not platform or platform.group(1) not in ('1', 'MACOS', 'macos'):
                raise ValueError(f'Not a macOS binary: {path}: {arch}')
            minimum = re.search(r'\bminos (\S+)', block).group(1)
        elif cmd == 'LC_VERSION_MIN_MACOSX':
            minimum = re.search(r'\bversion (\S+)', block).group(1)
        elif cmd in LOADS:
            dependencies.append(re.search(r'\bname (.+?) \(offset', block).group(1))
        elif cmd == 'LC_RPATH':
            rpaths.append(re.search(r'\bpath (.+?) \(offset', block).group(1))
        # LC_ID_DYLIB is the library's own identity, not a dependency.
    if minimum is None:
        raise ValueError(f'Missing macOS minimum version: {path}: {arch}')
    parts = tuple(int(v) for v in minimum.split('.'))
    if parts + (0,) * (3 - len(parts)) > (15, 0, 0):
        raise ValueError(f'Requires newer than macOS 15: {path}: {minimum}')
    return minimum, dependencies, rpaths


def system_path(value):
    return os.path.normpath(value).startswith(('/usr/lib/', '/System/Library/'))


def expand_path(value, loader, executable_dir):
    for prefix, directory in (('@loader_path', loader.parent), ('@executable_path', executable_dir)):
        if value == prefix:
            return directory
        if value.startswith(prefix + '/'):
            return directory / value[len(prefix) + 1:]
    if value.startswith('/'):
        return Path(value)
    raise ValueError(f'Unsupported relative load path: {value}: {loader}')


def resolve_dependency(dep, loader, executable_dir, rpaths, root):
    if system_path(dep):
        return dep  # Apple libraries can exist only in the dyld shared cache.
    if dep.startswith('@rpath/'):
        candidates = [p / dep[len('@rpath/'):] for p in rpaths]
    else:
        candidates = [expand_path(dep, loader, executable_dir)]
    for candidate in candidates:
        candidate = candidate.resolve()
        if system_path(str(candidate)):
            return str(candidate)
        if not inside(candidate, root):
            raise ValueError(f'Dependency escapes bundle: {loader}: {dep}: {candidate}')
        if candidate.is_file():
            if not macho(candidate):
                raise ValueError(f'Dependency is not Mach-O: {candidate}')
            return str(candidate.relative_to(root))
    raise ValueError(f'Unresolved dependency: {loader}: {dep}')


def verify_bundle(root, variant):
    root = root.resolve(strict=True)
    check_links(root)
    info = plistlib.loads((root / 'Contents/Info.plist').read_bytes())
    if info.get('LSMinimumSystemVersion') != '15.0' or info.get('CFBundleIdentifier') != 'local.hamster.video':
        raise ValueError('Unexpected application identity or minimum OS')
    expected = set(VARIANTS[variant][1])
    main = root / 'Contents/MacOS' / info['CFBundleExecutable']
    tools = root / 'Contents/Resources/tools'
    for executable in [main] + [tools / name for name in TOOLS]:
        if not executable.is_file() or not os.access(executable, os.X_OK) or not macho(executable):
            raise ValueError(f'Missing executable: {executable}')
    if not (tools / '_internal').is_dir():
        raise ValueError('Missing yt-dlp runtime directory')
    if not (root / 'Contents/Resources' / info['CFBundleIconFile']).is_file():
        raise ValueError('Missing application icon')
    records = []
    for path in sorted(root.rglob('*')):
        if not macho(path):
            continue
        arches = set(output('lipo', '-archs', path).split())
        if arches != expected:
            raise ValueError(f'Wrong architecture: {path}: expected {sorted(expected)}, got {sorted(arches)}')
        details = {}
        # All supported helper entry points live in Resources/tools; the GUI
        # entry point lives in Contents/MacOS. Never search PATH or Homebrew.
        executable_dir = tools if inside(path, tools) else main.parent
        for arch in sorted(expected):
            minimum, deps, raw_rpaths = load_commands(path, arch)
            rpaths = []
            for raw in raw_rpaths:
                candidate = expand_path(raw, path, executable_dir).resolve()
                if not inside(candidate, root) and not system_path(str(candidate)):
                    raise ValueError(f'External rpath: {path}: {raw}')
                rpaths.append(candidate)
            resolved = {dep: resolve_dependency(dep, path, executable_dir, rpaths, root) for dep in deps}
            details[arch] = {'minimum_macos': minimum, 'dependencies': resolved, 'rpaths': raw_rpaths}
        records.append({'path': str(path.relative_to(root)), 'architectures': sorted(arches), 'details': details})
    return {'status': 'passed', 'variant': variant, 'version': info['CFBundleShortVersionString'],
            'build': info['CFBundleVersion'], 'macho_files': len(records), 'files': records}


def signing_metadata(path, arch):
    result = subprocess.run(['codesign', '-d', '--architecture', arch, '--verbose=4', '--entitlements', ':-', str(path)],
                            capture_output=True)
    entitlements = None
    if result.stdout.strip():
        entitlements = plistlib.loads(result.stdout)
    diagnostic = result.stderr.decode(errors='replace')
    return {'entitlements': entitlements,
            'source_signature': [line for line in diagnostic.splitlines()
                                 if line.startswith(('Identifier=', 'CodeDirectory ', 'TeamIdentifier='))],
            'inspection_exit_code': result.returncode}


def stage_app(project, tools, binaries, version, target, destination, inputs):
    expected = set(VARIANTS[target][1])
    tools = tools.resolve(strict=True)
    destination = destination.resolve()
    if destination.exists():
        raise ValueError(f'Output already exists; choose a new --output-dir: {destination}')
    if inside(destination, tools) or inside(tools, destination):
        raise ValueError('Output and helper source must not overlap')
    check_links(tools)
    trusted = json.loads((project / 'third_party/tool-checksums.json').read_text())
    before = inventory(tools)
    for name in TOOLS:
        if before[name]['sha256'] != trusted[name]['sha256']:
            raise ValueError(f'Helper differs from pinned version: {name}')
    info = plistlib.loads((project / 'assets/Info.plist').read_bytes())
    if info['CFBundleShortVersionString'] != version:
        raise ValueError('Cargo and Info.plist versions disagree')
    icon = info['CFBundleIconFile']
    if Path(icon).name != icon:
        raise ValueError('Icon filename must be a simple filename')
    signing = {str(p.relative_to(tools)): {arch: signing_metadata(p, arch) for arch in sorted(expected)}
               for p in sorted(tools.rglob('*')) if macho(p)}
    destination.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='.hamster-stage-', dir=destination.parent) as temp:
        staging = Path(temp)
        app = staging / 'Hamster Video.app'
        resources = app / 'Contents/Resources'
        (app / 'Contents/MacOS').mkdir(parents=True)
        resources.mkdir()
        main = app / 'Contents/MacOS' / info['CFBundleExecutable']
        if len(expected) == 2:
            subprocess.run(['lipo', '-create', *[str(binaries[a]) for a in sorted(expected)], '-output', str(main)], check=True)
        else:
            shutil.copy2(binaries[next(iter(expected))], main)
        shutil.copy2(project / 'assets/Info.plist', app / 'Contents/Info.plist')
        shutil.copy2(project / 'assets/Hamster.icns', resources / icon)
        shutil.copytree(tools, resources / 'tools', symlinks=True)
        shutil.copytree(project / 'third_party/Licenses', resources / 'Licenses', symlinks=True)
        shutil.copy2(project / 'third_party/Rust-licenses.txt', resources / 'Licenses/Rust-licenses.txt')
        for path in sorted((resources / 'tools').rglob('*')):
            if not macho(path):
                continue
            arches = set(output('lipo', '-archs', path).split())
            if not expected.issubset(arches):
                raise ValueError(f'Missing target architecture: {path}')
            if arches != expected:
                if len(expected) != 1:
                    raise ValueError(f'Unexpected source architectures: {path}: {arches}')
                thin = path.with_name(path.name + '.hamster-thin')
                mode = stat.S_IMODE(path.stat().st_mode)
                subprocess.run(['lipo', str(path), '-thin', next(iter(expected)), '-output', str(thin)], check=True)
                thin.chmod(mode)
                thin.replace(path)
        preserve_archive(tools / 'yt-dlp', resources / 'tools/yt-dlp')
        payload_hash = archive_hash(resources / 'tools/yt-dlp')
        after = inventory(resources / 'tools')
        if set(before) != set(after):
            raise ValueError('Helper file inventory changed during staging')
        for name, record in before.items():
            dest = resources / 'tools' / name
            if not macho(dest):
                if record != after[name]:
                    raise ValueError(f'Helper resource or link changed: {name}')
            elif record['mode'] != after[name]['mode']:
                raise ValueError(f'Helper permissions changed: {name}')
        report = verify_bundle(app, target)
        if inventory(tools) != before:
            raise ValueError('Source helpers changed during staging')
        if build_inputs(project) != inputs:
            raise ValueError('Build inputs changed during compilation/staging; rebuild from a stable source')
        (staging / 'architecture-verification.json').write_text(json.dumps(report, indent=2) + '\n')
        (staging / 'source-helper-manifest.json').write_text(json.dumps(before, indent=2) + '\n')
        (staging / 'source-signing-metadata.json').write_text(json.dumps(signing, indent=2) + '\n')
        (staging / 'build-inputs.json').write_text(json.dumps(inputs, indent=2) + '\n')
        label = VARIANTS[target][0]
        metadata = {'version': version, 'build': info['CFBundleVersion'], 'variant': target,
                    'expected_architectures': sorted(expected), 'app': 'Hamster Video.app',
                    'dmg_filename': f'HamsterVideo-{version}-{label}.dmg',
                    'installed_bytes': sum(p.stat().st_size for p in app.rglob('*') if p.is_file() and not p.is_symlink()),
                    'yt_dlp_archive_sha256': payload_hash,
                    'stage_status': 'architecture-verified; signing and DMG packaging pending',
                    'source_commit': output('git', '-C', project, 'rev-parse', 'HEAD'),
                    'source_dirty': bool(output('git', '-C', project, 'status', '--porcelain'))}
        (staging / 'build-report.json').write_text(json.dumps(metadata, indent=2) + '\n')
        staging.rename(destination)
    return metadata


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest='command', required=True)
    build = commands.add_parser('build', help='Compile and stage an unsigned app; no DMG or install')
    build.add_argument('--arch', choices=VARIANTS, required=True)
    build.add_argument('--tools', type=Path, required=True, help='Pinned Universal Resources/tools directory')
    build.add_argument('--output-dir', type=Path, help='New output directory; existing paths are never replaced')
    build.add_argument('--offline', action='store_true')
    verify = commands.add_parser('verify')
    verify.add_argument('app', type=Path)
    verify.add_argument('--arch', choices=VARIANTS, required=True)
    args = parser.parse_args()
    if args.command == 'verify':
        print(json.dumps(verify_bundle(args.app, args.arch), indent=2))
        return
    project = Path(__file__).resolve().parent.parent
    inputs = build_inputs(project)
    metadata_command = ['cargo', 'metadata', '--no-deps', '--locked', '--format-version', '1',
                        '--manifest-path', project / 'Cargo.toml']
    if args.offline:
        metadata_command.append('--offline')
    cargo = json.loads(output(*metadata_command))
    package = next(p for p in cargo['packages'] if p['name'] == 'hamster-video')
    version = package['version']
    if not re.fullmatch(r'\d+\.\d+\.\d+', version):
        raise ValueError('Expected stable numeric release version')
    label, targets = VARIANTS[args.arch]
    destination = args.output_dir or project / 'dist' / version / label
    if destination.exists():
        raise ValueError(f'Output already exists: {destination}')
    env = os.environ.copy()
    env['MACOSX_DEPLOYMENT_TARGET'] = '15.0'
    remap = f'--remap-path-prefix={project}=/build/hamster-video'
    flags = env.get('RUSTFLAGS', '')
    if remap not in flags.split():
        flags += ' ' + remap
    env['RUSTFLAGS'] = flags.strip()
    command = ['cargo', 'build', '--release', '--locked', '--manifest-path', str(project / 'Cargo.toml')]
    if args.offline:
        command.append('--offline')
    for target in targets.values():
        command += ['--target', target]
    subprocess.run(command, env=env, check=True)
    binaries = {arch: Path(cargo['target_directory']) / target / 'release/hamster-video'
                for arch, target in targets.items()}
    report = stage_app(project, args.tools, binaries, version, args.arch, destination, inputs)
    print(json.dumps({'output_dir': str(destination.resolve()), **report}, indent=2))


if __name__ == '__main__':
    try:
        main()
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        raise SystemExit(str(error))
