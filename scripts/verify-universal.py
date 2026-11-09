#!/usr/bin/env python3
"""Fail if any bundled Mach-O file lacks Intel or Apple Silicon support."""
import json, pathlib, re, subprocess, sys
root=pathlib.Path(sys.argv[1]).resolve()
magic={b'\xcf\xfa\xed\xfe',b'\xfe\xed\xfa\xcf',b'\xca\xfe\xba\xbe',b'\xbe\xba\xfe\xca',b'\xca\xfe\xba\xbf',b'\xbf\xba\xfe\xca'}
records=[]
for path in sorted(root.rglob('*')):
 if not path.is_file() or path.is_symlink():continue
 with path.open('rb') as file:header=file.read(4)
 if header not in magic:continue
 arches=subprocess.check_output(['lipo','-archs',str(path)],text=True).split()
 if not {'x86_64','arm64'}.issubset(arches):raise SystemExit(f'Missing architecture: {path}: {arches}')
 minimums={}
 for arch in ['x86_64','arm64']:
  loads=subprocess.check_output(['otool','-arch',arch,'-l',str(path)],text=True)
  versions=re.findall(r'\bminos\s+(\d+(?:\.\d+)*)',loads)
  if not versions:
   versions=re.findall(r'LC_VERSION_MIN_MACOSX\s+cmdsize\s+\d+\s+version\s+(\d+(?:\.\d+)*)',loads)
  if not versions:raise SystemExit(f'No minimum OS for {path}: {arch}')
  version=versions[0];minimums[arch]=version
  parts=tuple(map(int,version.split('.')));parts=parts+(0,)*(3-len(parts))
  if parts>(15,0,0):raise SystemExit(f'Requires newer than macOS 15: {path}: {version}')
  dependencies=subprocess.check_output(['otool','-arch',arch,'-L',str(path)],text=True)
  for line in dependencies.splitlines():
   dep=line.strip().split(' (compatibility')[0]
   if line.startswith('\t') and dep.startswith('/') and not dep.startswith(('/usr/lib/','/System/Library/')):
    raise SystemExit(f'External runtime dependency: {path}: {dep}')
 records.append({'path':str(path.relative_to(root)),'architectures':arches,'minimum_macos':minimums})
if not records:raise SystemExit('No Mach-O files found')
print(json.dumps({'status':'passed','macho_files':len(records),'files':records},indent=2))
