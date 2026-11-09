"""Exercise the GUI's Rust worker with deterministic local media.
Usage: python3 tests/smoke.py /absolute/path/to/hamster-video /absolute/path/to/tools
Set HAMSTER_TEST_ENCODERS=1 to additionally test macOS VideoToolbox HEVC encoding.
"""
import functools, http.server, os, pathlib, subprocess, sys, tempfile, threading, json
binary, tools = map(lambda p: str(pathlib.Path(p).resolve()), sys.argv[1:3])
fixtures=pathlib.Path(__file__).parent/'fixtures'
class Handler(http.server.SimpleHTTPRequestHandler):
    requested=[]
    def do_GET(self):
        self.requested.append(self.path)
        return super().do_GET()
    def log_message(self,*args): pass
server=http.server.ThreadingHTTPServer(('127.0.0.1',0),functools.partial(Handler,directory=str(fixtures)))
threading.Thread(target=server.serve_forever,daemon=True).start()
base=f'http://127.0.0.1:{server.server_port}'
env=dict(os.environ,HAMSTER_TOOLS=tools)
try:
    metadata=subprocess.run([binary,'--check-tracks',base+'/multilingual.mpd'],env=env,capture_output=True,text=True,timeout=100)
    assert metadata.returncode==0,metadata.stderr
    assert {t['code'] for t in json.loads(metadata.stdout)['tracks']}=={'de','es'}
    print('Audio-language discovery: PASS')
    cases=[('video','sample.mp4',[],0),('audio','sample.mp4',['--audio'],0),
           ('merge','manifest.mpd',[],0),('original','manifest.mpd',['--compatibility=original'],0),
           ('spanish','multilingual.mpd',['--language=es'],0),
           ('spanish-audio','multilingual.mpd',['--language=es','--audio'],0),
           ('unavailable-language','multilingual.mpd',['--language=fr'],1),
           ('missing','missing.mp4',[],1)]
    if os.environ.get('HAMSTER_TEST_ENCODERS')=='1':cases.extend([('hevc','sample.mp4',['--compatibility=hevc'],0),('h264-conversion','sample-hevc.mp4',[],0)])
    with tempfile.TemporaryDirectory(prefix='hamster-test-') as tmp:
        for name,resource,extra,expected in cases:
            Handler.requested.clear()
            folder=pathlib.Path(tmp)/name
            run=subprocess.run([binary,'--check-download',base+'/'+resource,str(folder)]+extra,env=env,capture_output=True,text=True,timeout=180)
            assert run.returncode==expected,(name,run.stdout,run.stderr)
            if expected==0:
                media=[p for p in folder.iterdir() if p.suffix in ['.mp4','.mp3','.mkv']]
                assert len(media)==1,(name,run.stdout,media)
                assert '.source.' not in media[0].name,(name,media)
                info=json.loads(subprocess.check_output([tools+'/ffprobe','-v','error','-show_streams','-of','json',str(media[0])]))
                streams=info['streams']; audio=next(s for s in streams if s['codec_type']=='audio')
                video=next((s for s in streams if s['codec_type']=='video'),None)
                if '--audio' in extra:assert video is None and audio['codec_name']=='mp3',(name,info)
                else:
                    assert video['codec_name']==('hevc' if name=='hevc' else 'h264'),(name,info)
                    assert audio['codec_name']=='aac',(name,info)
                if name=='original':assert media[0].suffix=='.mkv'
                if name.startswith('spanish'):
                    assert '/init-stream2.m4s' in Handler.requested,Handler.requested
                    assert '/init-stream1.m4s' not in Handler.requested,Handler.requested
                    if name=='spanish':assert audio['tags']['language']=='spa',audio
            print(name+': PASS')
finally:server.shutdown()
