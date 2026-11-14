#!/usr/bin/env python3
"""Sign an independent staged app, create a candidate DMG, and verify its contents.

Requires the build-only packages in requirements-dmg.txt. Never installs or
publishes an app. Disk Utility's Image from Folder is a supported fallback for
image creation; verify-image applies the same checks to either creation route.
"""
import argparse
import json
import os
from pathlib import Path
import plistlib
import shutil
import subprocess
import sys

from ds_store import DSStore
from macos_bundle import inventory, macho, sha256, signing_metadata, verify_bundle
from pyinstaller_payload import archive_hash


def run(*args):
    result = subprocess.run([str(a) for a in args], capture_output=True, text=True)
    if result.stdout.strip():
        print(result.stdout.strip(), flush=True)
    if result.stderr.strip():
        print(result.stderr.strip(), flush=True)
    result.check_returncode()
    return result.stdout


def save(path, data):
    path.write_text(json.dumps(data, indent=2) + '\n')


def check_inputs(stage, project):
    inputs = json.loads((stage / 'build-inputs.json').read_text())
    for name, digest in inputs.items():
        if sha256(project / name) != digest:
            raise ValueError(f'Compiled build input changed: {name}; rebuild first')
    return inputs


def check_signatures(app, source_signatures, arch):
    records = []
    tools = app / 'Contents/Resources/tools'
    for path in sorted(app.rglob('*')):
        if not macho(path):
            continue
        run('codesign', '--verify', '--strict', '--all-architectures', path)
        actual = signing_metadata(path, arch)
        name = str(path.relative_to(app))
        expected = None
        if path.is_relative_to(tools):
            expected = source_signatures[str(path.relative_to(tools))][arch]['entitlements']
        if actual['inspection_exit_code'] or actual['entitlements'] != expected:
            raise ValueError(f'Entitlements changed or signature inspection failed: {name}')
        records.append({'path': name, 'verification': 'passed', **actual})
    for framework in app.rglob('*.framework'):
        for version in (framework / 'Versions').iterdir():
            if version.is_dir() and not version.is_symlink():
                run('codesign', '--verify', '--strict', '--bundle-version', version.name, framework)
        run('codesign', '--verify', '--strict', framework)
    run('codesign', '--verify', '--deep', '--strict', '--all-architectures', app)
    return records


def normalize_python_framework(app):
    """Restore framework links only after proving the copied trees identical."""
    frame = app / 'Contents/Resources/tools/_internal/Python.framework'
    versions = frame / 'Versions'
    if (versions / 'Current').is_symlink():
        return []
    if inventory(versions / 'Current') != inventory(versions / '3.14'):
        raise ValueError('Python framework versions differ; cannot normalize safely')
    if inventory(frame / 'Resources') != inventory(versions / '3.14/Resources'):
        raise ValueError('Python framework resource copies differ')
    if sha256(frame / 'Python') != sha256(versions / '3.14/Python'):
        raise ValueError('Python framework executable copies differ')
    links = [(versions / 'Current', '3.14'),
             (frame / 'Python', 'Versions/Current/Python'),
             (frame / 'Resources', 'Versions/Current/Resources')]
    result = []
    for path, target in links:
        if path.is_dir():
            shutil.rmtree(path)
        else:
            path.unlink()
        path.symlink_to(target)
        result.append({'path': str(path.relative_to(app)), 'target': target})
    return result


def prepare(stage, package):
    project = Path(__file__).resolve().parent.parent
    inputs = check_inputs(stage, project)
    metadata = json.loads((stage / 'build-report.json').read_text())
    if metadata['variant'] not in ('intel', 'apple-silicon'):
        raise ValueError('This release packages Intel and Apple Silicon separately')
    arch, = metadata['expected_architectures']
    source_app = stage / metadata['app']
    if archive_hash(source_app / 'Contents/Resources/tools/yt-dlp') != metadata['yt_dlp_archive_sha256']:
        raise ValueError('Staged yt-dlp archive differs from build evidence')
    verify_bundle(source_app, metadata['variant'])
    original = inventory(source_app)
    signatures = json.loads((stage / 'source-signing-metadata.json').read_text())
    if any(record['inspection_exit_code'] for item in signatures.values() for record in item.values()):
        raise ValueError('Source signing metadata is incomplete')
    package.mkdir(parents=True, exist_ok=False)
    image_root = package / 'image-root'
    image_root.mkdir()
    app = image_root / metadata['app']
    shutil.copytree(source_app, app, symlinks=True)
    # Remove downloaded-source and Finder metadata only from this manufacturing
    # copy. The installed app and original staging tree are never modified.
    run('xattr', '-cr', app)
    normalized_links = normalize_python_framework(app)
    entitlement_dir = package / 'entitlements'
    entitlement_dir.mkdir()
    tools = app / 'Contents/Resources/tools'
    natives = sorted((p for p in app.rglob('*') if macho(p)), key=lambda p: (-len(p.parts), str(p)))
    for index, path in enumerate(natives):
        args = ['codesign', '--force', '--sign', '-', '--timestamp=none']
        if path.is_relative_to(tools):
            record = signatures[str(path.relative_to(tools))][arch]
            identifiers = [v.partition('=')[2] for v in record['source_signature'] if v.startswith('Identifier=')]
            if identifiers:
                args += ['--identifier', identifiers[0]]
            if record['entitlements']:
                ent = entitlement_dir / f'{index}.plist'
                ent.write_bytes(plistlib.dumps(record['entitlements']))
                args += ['--entitlements', ent, '--generate-entitlement-der']
        run(*args, path)
    # Seal real framework versions after their nested native code.
    frameworks = sorted(app.rglob('*.framework'), key=lambda p: -len(p.parts))
    for framework in frameworks:
        for version in sorted((framework / 'Versions').iterdir()):
            if version.is_dir() and not version.is_symlink():
                run('codesign', '--force', '--sign', '-', '--timestamp=none', '--bundle-version', version.name, framework)
        run('codesign', '--force', '--sign', '-', '--timestamp=none', framework)
    run('codesign', '--force', '--sign', '-', '--timestamp=none', app)
    verified_signatures = check_signatures(app, signatures, arch)
    architecture = verify_bundle(app, metadata['variant'])
    if archive_hash(app / 'Contents/Resources/tools/yt-dlp') != metadata['yt_dlp_archive_sha256']:
        raise ValueError('Signing changed the yt-dlp archive')
    signed = inventory(app)
    for name, record in original.items():
        path = app / name
        if not path.exists():
            raise ValueError(f'Packaging removed a file: {name}')
        if macho(source_app / name):
            if record['mode'] != (path.stat().st_mode & 0o7777):
                raise ValueError(f'Executable permission changed: {name}')
        elif 'link' in record:
            if not path.is_symlink() or os.readlink(path) != record['link']:
                raise ValueError(f'Packaging changed a link: {name}')
        elif sha256(path) != record['sha256'] or (path.stat().st_mode & 0o7777) != record['mode']:
            raise ValueError(f'Packaging changed a resource: {name}')
    added = set(signed) - set(original)
    normalized_names = {record['path'] for record in normalized_links}
    if any('/_CodeSignature/' not in '/' + name and name not in normalized_names for name in added):
        raise ValueError(f'Unexpected new bundle resources: {sorted(added)}')
    (image_root / 'Applications').symlink_to('/Applications')
    run(sys.executable, project / 'scripts/dmg-layout.py', image_root / '.DS_Store')
    if inventory(source_app) != original or check_inputs(stage, project) != inputs:
        raise ValueError('Source changed during packaging')
    for name in ('build-inputs.json', 'build-report.json', 'source-helper-manifest.json', 'source-signing-metadata.json'):
        shutil.copy2(stage / name, package / name)
    save(package / 'signed-app-manifest.json', signed)
    save(package / 'signature-verification.json', verified_signatures)
    save(package / 'framework-layout-normalization.json', normalized_links)
    save(package / 'architecture-verification.json', architecture)
    save(package / 'packaging-inputs.json', {str(p.relative_to(project)): sha256(p) for p in
         (project / 'scripts/package-macos.py', project / 'scripts/pyinstaller_payload.py', project / 'scripts/dmg-layout.py', project / 'scripts/requirements-dmg.txt')})
    save(package / 'package-report.json', {'status': 'signed; DMG verification pending',
         'version': metadata['version'], 'variant': metadata['variant'], 'dmg_filename': metadata['dmg_filename'],
         'installed_bytes': sum(v.get('bytes', 0) for v in signed.values()),
         'native_components': len(verified_signatures), 'signing': 'ad-hoc', 'notarized': False})
    print(f'Signed package prepared: {package}', flush=True)


def check_image(package, image):
    report = json.loads((package / 'package-report.json').read_text())
    metadata = json.loads((package / 'build-report.json').read_text())
    if image.name != report['dmg_filename']:
        raise ValueError('Unexpected image filename')
    run('hdiutil', 'verify', image)
    attach = plistlib.loads(subprocess.check_output(['hdiutil', 'attach', '-readonly', '-nobrowse', '-plist', str(image)]))
    mounted = [Path(e['mount-point']) for e in attach['system-entities'] if 'mount-point' in e]
    if len(mounted) != 1:
        for mount in mounted:
            run('hdiutil', 'detach', mount)
        raise ValueError(f'Expected one mounted filesystem: {mounted}')
    mount, = mounted
    try:
        visible = sorted(p.name for p in mount.iterdir() if not p.name.startswith('.'))
        if visible != ['Applications', 'Hamster Video.app'] or os.readlink(mount / 'Applications') != '/Applications':
            raise ValueError('Unexpected installer contents or Applications shortcut')
        with DSStore.open(str(mount / '.DS_Store'), 'r') as store:
            positions = {n: store[n]['Iloc'] for n in visible}
            if positions['Hamster Video.app'] != (160, 170) or positions['Applications'] != (480, 170):
                raise ValueError('Installer layout differs from the approved layout')
        app = mount / metadata['app']
        if archive_hash(app / 'Contents/Resources/tools/yt-dlp') != metadata['yt_dlp_archive_sha256']:
            raise ValueError('Packaged yt-dlp archive differs from pinned original')
        if inventory(app) != json.loads((package / 'signed-app-manifest.json').read_text()):
            raise ValueError('Mounted app differs from signed candidate')
        info = plistlib.loads((app / 'Contents/Info.plist').read_bytes())
        if info['CFBundleShortVersionString'] != metadata['version'] or info['CFBundleVersion'] != metadata['build'] or 'LSEnvironment' in info:
            raise ValueError('Unexpected version/build or test environment override')
        architecture = verify_bundle(app, metadata['variant'])
        signatures = json.loads((package / 'source-signing-metadata.json').read_text())
        arch, = metadata['expected_architectures']
        checks = check_signatures(app, signatures, arch)
        save(package / 'mounted-architecture-verification.json', architecture)
        save(package / 'mounted-signature-verification.json', checks)
        report.update({'status': 'candidate DMG verified', 'image': str(image), 'dmg_bytes': image.stat().st_size,
                       'sha256': sha256(image), 'version': info['CFBundleShortVersionString'], 'build': info['CFBundleVersion'],
                       'visible_contents': visible, 'hidden_contents': sorted(p.name for p in mount.iterdir() if p.name.startswith('.')),
                       'positions': positions, 'mounted_app_matches_signed_candidate': True})
        save(package / 'package-report.json', report)
    finally:
        run('hdiutil', 'detach', mount)
    print(json.dumps(report, indent=2))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest='command', required=True)
    p = sub.add_parser('prepare')
    p.add_argument('--stage', type=Path, required=True)
    p.add_argument('--package-dir', type=Path, required=True)
    for name in ('create-image', 'verify-image'):
        p = sub.add_parser(name)
        p.add_argument('--package-dir', type=Path, required=True)
        p.add_argument('--image', type=Path, required=True)
    args = parser.parse_args()
    package = args.package_dir.resolve()
    if args.command == 'prepare':
        prepare(args.stage.resolve(), package)
    elif args.command == 'verify-image':
        check_image(package, args.image.resolve())
    else:
        if args.image.exists():
            raise ValueError('Refusing to replace an existing image')
        report = json.loads((package / 'package-report.json').read_text())
        if args.image.name != report['dmg_filename']:
            raise ValueError('Unexpected image filename')
        run('hdiutil', 'create', '-fs', 'HFS+', '-srcfolder', package / 'image-root',
            '-volname', 'Hamster Video', '-format', 'UDZO', '-o', args.image.resolve())
        check_image(package, args.image.resolve())


if __name__ == '__main__':
    main()
