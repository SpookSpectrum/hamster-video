#!/usr/bin/env python3
"""Build ICNS from the current PNG and fail on any invalid conversion."""
import pathlib,struct,subprocess,tempfile
root=pathlib.Path(__file__).resolve().parent.parent
chunks=[]
with tempfile.TemporaryDirectory(prefix='hamster-icon-') as folder:
 for kind,size in [('icp4',16),('icp5',32),('icp6',64),('ic07',128),('ic08',256),('ic09',512),('ic10',1024),('ic11',32),('ic12',64),('ic13',256),('ic14',512)]:
  p=pathlib.Path(folder)/(kind+'.png')
  subprocess.run(['sips','-z',str(size),str(size),str(root/'assets/hamster-icon.png'),'--out',str(p)],check=True,stdout=subprocess.DEVNULL)
  data=p.read_bytes()
  assert data[:8]==b'\x89PNG\r\n\x1a\n' and struct.unpack('>II',data[16:24])==(size,size)
  chunks.append(kind.encode()+struct.pack('>I',len(data)+8)+data)
body=b''.join(chunks);target=root/'assets/Hamster.icns';tmp=target.with_suffix('.icns.tmp')
tmp.write_bytes(b'icns'+struct.pack('>I',len(body)+8)+body);tmp.replace(target)
print('Validated and packaged 11 icon representations')
