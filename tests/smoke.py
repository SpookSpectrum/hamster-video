"""Exercise the same Rust worker as the GUI against deterministic local media.
Usage: python3 tests/smoke.py /absolute/path/to/hamster-video /absolute/path/to/tools
"""
import functools, http.server, os, pathlib, subprocess, sys, tempfile, threading, json
binary, tools = map(lambda p: str(pathlib.Path(p).resolve()), sys.argv[1:3])
fixtures=pathlib.Path(__file__).parent/'fixtures'
handler=functools.partial(http.server.SimpleHTTPRequestHandler,directory=str(fixtures))
server=http.server.ThreadingHTTPServer(('127.0.0.1',0),handler)
threading.Thread(target=server.serve_forever,daemon=True).start()
base=f'http://127.0.0.1:{server.server_port}'
env=dict(os.environ,HAMSTER_TOOLS=tools)
with tempfile.TemporaryDirectory(prefix='hamster-test-') as tmp:
 for name,resource,extra,expected in [('video','sample.mp4',[],0),('audio','sample.mp4',['--audio'],0),('merge','manifest.mpd',[],0),('missing','missing.mp4',[],1)]:
  folder=pathlib.Path(tmp)/name
  run=subprocess.run([binary,'--check-download',base+'/'+resource,str(folder)]+extra,env=env,capture_output=True,text=True,timeout=120)
  assert run.returncode==expected,(name,run.stdout,run.stderr)
  if expected==0:
   media=[p for p in folder.iterdir() if p.suffix in ['.mp4','.mp3']]
   assert media,(name,run.stdout)
   info=json.loads(subprocess.check_output([tools+'/ffprobe','-v','error','-show_streams','-of','json',str(media[0])]))
   kinds={s['codec_type'] for s in info['streams']}
   assert 'audio' in kinds,(name,info)
   assert ('video' in kinds)==(name!='audio'),(name,info)
  print(name+': PASS')
server.shutdown()
