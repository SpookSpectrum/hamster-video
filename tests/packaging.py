"""Failure-oriented packaging checks using real, tiny Mach-O fixtures.

Run on a Mac with Xcode Command Line Tools: python3 tests/packaging.py
No app launch, downloaded dependencies or installed-app writes. Signing tests
operate only on temporary synthetic fixtures.
"""
import importlib.util
import os
from pathlib import Path
import plistlib
import shutil
import subprocess
import struct
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'scripts'))
from pyinstaller_payload import archive, preserve_archive, COOKIE, MAGIC

spec = importlib.util.spec_from_file_location('macos_bundle', Path(__file__).resolve().parents[1] / 'scripts/macos_bundle.py')
bundle = importlib.util.module_from_spec(spec)
spec.loader.exec_module(bundle)


class BundleVerification(unittest.TestCase):
    def test_pyinstaller_payload_survives_thinning_and_signing(self):
        payload_data = b'x' * 32
        table = struct.pack('!IIIIBc', 32, 0, 32, 32, 0, b'z') + b'PYZ.pyz\0'.ljust(14, b'\0')
        payload = payload_data + table + COOKIE.pack(MAGIC, 64 + COOKIE.size, 32, 32, 314, b'Python')
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / 'source'
            source.write_bytes(b'prefix' + payload + b'trailing-signature')
            thin = root / 'thin'
            shutil.copy2(self.fixtures / 'x86_64', thin)
            preserve_archive(source, thin)
            subprocess.run(['codesign', '--force', '--sign', '-', '--timestamp=none', str(thin)], check=True)
            subprocess.run(['codesign', '--verify', '--strict', str(thin)], check=True)
            self.assertEqual(archive(thin.read_bytes()), payload)
            once = thin.read_bytes()
            preserve_archive(source, thin)
            self.assertEqual(thin.read_bytes(), once)

    def test_missing_pyinstaller_payload_rejected(self):
        with self.assertRaisesRegex(ValueError, 'Missing embedded'):
            archive(b'no payload')

    def test_invalid_pyinstaller_bounds_rejected(self):
        with self.assertRaisesRegex(ValueError, 'Invalid PyInstaller archive bounds'):
            archive(COOKIE.pack(MAGIC, 99999, 0, 0, 314, b'Python'))

    @classmethod
    def setUpClass(cls):
        cls.cache = tempfile.TemporaryDirectory(prefix='hamster-packaging-fixtures-')
        cls.fixtures = Path(cls.cache.name)
        (cls.fixtures / 'main.c').write_text('int main(void) { return 0; }\n')
        for arch in ('x86_64', 'arm64'):
            subprocess.run(['clang', '-arch', arch, '-mmacosx-version-min=15.0',
                            str(cls.fixtures / 'main.c'), '-o', str(cls.fixtures / arch)], check=True)
        subprocess.run(['lipo', '-create', str(cls.fixtures / 'x86_64'), str(cls.fixtures / 'arm64'),
                        '-output', str(cls.fixtures / 'universal')], check=True)

    @classmethod
    def tearDownClass(cls):
        cls.cache.cleanup()

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='hamster-packaging-test-')
        self.addCleanup(self.temp.cleanup)
        self.app = Path(self.temp.name) / 'Hamster Video.app'
        self.main = self.app / 'Contents/MacOS/hamster-video'
        self.tools = self.app / 'Contents/Resources/tools'
        self.main.parent.mkdir(parents=True)
        (self.tools / '_internal').mkdir(parents=True)
        self.write_binaries('x86_64')
        (self.app / 'Contents/Resources/icon.icns').write_bytes(b'fixture')
        (self.app / 'Contents/Info.plist').write_bytes(plistlib.dumps({
            'CFBundleExecutable': 'hamster-video', 'CFBundleIconFile': 'icon.icns',
            'CFBundleIdentifier': 'local.hamster.video', 'CFBundleShortVersionString': '1.0.1',
            'CFBundleVersion': '7', 'LSMinimumSystemVersion': '15.0'}))

    def write_binaries(self, arch):
        for path in [self.main] + [self.tools / name for name in bundle.TOOLS]:
            shutil.copy2(self.fixtures / arch, path)

    def verify(self, arch='intel'):
        return bundle.verify_bundle(self.app, arch)

    def test_accepts_each_exact_architecture_and_universal(self):
        for source, variant in [('x86_64', 'intel'), ('arm64', 'apple-silicon'), ('universal', 'universal')]:
            with self.subTest(variant=variant):
                self.write_binaries(source)
                self.assertEqual(self.verify(variant)['macho_files'], 5)

    def test_rejects_wrong_architecture_in_nested_module(self):
        shutil.copy2(self.fixtures / 'arm64', self.tools / '_internal/extension.so')
        with self.assertRaisesRegex(ValueError, 'Wrong architecture'):
            self.verify()

    def test_rejects_unthinned_universal_helper(self):
        shutil.copy2(self.fixtures / 'universal', self.tools / 'deno')
        with self.assertRaisesRegex(ValueError, 'Wrong architecture'):
            self.verify()

    def test_rejects_dangling_and_external_links(self):
        path = self.tools / '_internal/link'
        path.symlink_to('missing')
        with self.assertRaisesRegex(ValueError, 'Broken or cyclic'):
            self.verify()
        path.unlink()
        path.symlink_to(self.fixtures / 'x86_64')
        with self.assertRaisesRegex(ValueError, 'escapes bundle'):
            self.verify()

    def test_accepts_internal_link(self):
        (self.tools / '_internal/deno-link').symlink_to('../deno')
        self.assertEqual(self.verify()['status'], 'passed')

    def linked_binary(self, rpath='@loader_path/libs', minimum='15.0'):
        libraries = self.main.parent / 'libs'
        libraries.mkdir()
        c = Path(self.temp.name) / 'fixture.c'
        c.write_text('int fixture(void) { return 0; }\n')
        library = libraries / 'libfixture.dylib'
        subprocess.run(['clang', '-arch', 'x86_64', '-mmacosx-version-min=' + minimum,
                        '-dynamiclib', str(c), '-Wl,-install_name,@rpath/libfixture.dylib', '-o', str(library)], check=True)
        c.write_text('extern int fixture(void); int main(void) { return fixture(); }\n')
        subprocess.run(['clang', '-arch', 'x86_64', '-mmacosx-version-min=' + minimum, str(c),
                        '-L' + str(libraries), '-lfixture', '-Wl,-rpath,' + rpath, '-o', str(self.main)], check=True)
        return library

    def test_resolves_bundled_rpath_and_rejects_missing_library(self):
        library = self.linked_binary()
        self.assertEqual(self.verify()['macho_files'], 6)
        library.unlink()
        with self.assertRaisesRegex(ValueError, 'Unresolved dependency'):
            self.verify()

    def test_rejects_external_rpath(self):
        self.linked_binary(rpath='/usr/local/lib')
        with self.assertRaisesRegex(ValueError, 'External rpath'):
            self.verify()

    def test_rejects_newer_os_requirement(self):
        self.linked_binary(minimum='16.0')
        with self.assertRaisesRegex(ValueError, 'Requires newer than macOS 15'):
            self.verify()

    def test_rejects_lost_executable_permission(self):
        (self.tools / 'deno').chmod(0o644)
        with self.assertRaisesRegex(ValueError, 'Missing executable'):
            self.verify()

    def test_system_path_cannot_escape_using_dotdot(self):
        self.assertFalse(bundle.system_path('/usr/lib/../../tmp/libunsafe.dylib'))
        self.assertTrue(bundle.system_path('/usr/lib/libSystem.B.dylib'))


if __name__ == '__main__':
    unittest.main(verbosity=2)
