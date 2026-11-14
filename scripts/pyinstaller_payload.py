"""Preserve the pinned yt-dlp PyInstaller archive when thinning its launcher.

Universal PyInstaller executables store the shared archive in their final CPU
slice. lipo alone therefore loses it when extracting the Intel slice. Keep the
archive bytes identical and extend the unsigned thin launcher's Mach-O string
table and __LINKEDIT coverage before the subsequent code-signing step.

Format references: PyInstaller/archive/readers.py and utils/osx.py in the
upstream PyInstaller repository. No Python runtime or build dependency added.
"""
import hashlib
from pathlib import Path
import struct
import subprocess

MAGIC = b'MEI\014\013\012\013\016'
COOKIE = struct.Struct('!8sIIII64s')


def archive(data):
    cookie = data.rfind(MAGIC)
    if cookie < 0:
        raise ValueError('Missing embedded PyInstaller archive')
    if cookie + COOKIE.size > len(data):
        raise ValueError('Truncated PyInstaller cookie')
    _, size, toc, toc_size, python, library = COOKIE.unpack_from(data, cookie)
    end = cookie + COOKIE.size
    start = end - size
    if start < 0 or size <= COOKIE.size or toc + toc_size != size - COOKIE.size:
        raise ValueError('Invalid PyInstaller archive bounds')
    if python != 314 or library.rstrip(b'\0') != b'Python':
        raise ValueError('Unexpected pinned yt-dlp Python runtime')
    position = start + toc
    while position < cookie:
        if position + 18 > cookie:
            raise ValueError('Truncated archive table')
        length, offset, compressed, _, flag, kind = struct.unpack_from('!IIIIBc', data, position)
        if length < 18 or position + length > cookie or offset + compressed > toc or flag not in (0, 1):
            raise ValueError('Invalid archive table entry')
        position += length
    if position != cookie:
        raise ValueError('Invalid archive table length')
    return data[start:end]


def archive_hash(path):
    return hashlib.sha256(archive(Path(path).read_bytes())).hexdigest()


def preserve_archive(source, thin):
    payload = archive(source.read_bytes())
    data = thin.read_bytes()
    if MAGIC in data:
        if archive(data) != payload:
            raise ValueError('Thinned yt-dlp archive differs from the pinned original')
        return
    subprocess.run(['codesign', '--remove-signature', str(thin)], check=True)
    data = bytearray(thin.read_bytes())
    if data[:4] != bytes.fromhex('cffaedfe'):
        raise ValueError('Expected little-endian 64-bit Mach-O launcher')
    cpu, = struct.unpack_from('<I', data, 4)
    if cpu not in (0x1000007, 0x100000c):
        raise ValueError('Unsupported launcher architecture')
    count, command_size = struct.unpack_from('<II', data, 16)
    offset = 32
    linkedit = symtab = None
    for _ in range(count):
        command, length = struct.unpack_from('<II', data, offset)
        if length < 8 or offset + length > 32 + command_size:
            raise ValueError('Invalid Mach-O load command')
        if command == 0x1d:
            raise ValueError('Signature must be removed before adding archive')
        if command == 0x19 and data[offset + 8:offset + 24].rstrip(b'\0') == b'__LINKEDIT':
            if linkedit is not None or length < 72:
                raise ValueError('Unexpected __LINKEDIT layout')
            linkedit = offset
        if command == 2:
            if symtab is not None or length != 24:
                raise ValueError('Unexpected symbol table layout')
            symtab = offset
        offset += length
    if linkedit is None or symtab is None or offset != 32 + command_size:
        raise ValueError('Incomplete Mach-O layout')
    string_start, string_size = struct.unpack_from('<II', data, symtab + 16)
    segment_start, = struct.unpack_from('<Q', data, linkedit + 40)
    if string_start + string_size != len(data) or not segment_start <= string_start:
        raise ValueError('String table must end at unsigned launcher EOF')
    data.extend(payload)
    segment_size = len(data) - segment_start
    page = 16384 if cpu == 0x100000c else 4096
    struct.pack_into('<I', data, symtab + 20, len(data) - string_start)
    struct.pack_into('<Q', data, linkedit + 48, segment_size)
    struct.pack_into('<Q', data, linkedit + 32, ((segment_size + page - 1) // page) * page)
    if archive(data) != payload:
        raise ValueError('Archive reconstruction mismatch')
    thin.write_bytes(data)
