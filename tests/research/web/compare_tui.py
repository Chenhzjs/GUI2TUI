"""Render old/new real GUI2TUI PTYs side by side, for visual QA only.
Both binaries observe the same live Accessibility application. No GUI input.
"""
import argparse,codecs,fcntl,hashlib,json,os,pty,struct,subprocess,termios,threading,time
from pathlib import Path
import pyte
p=argparse.ArgumentParser();p.add_argument('--output',type=Path,default=Path('/tmp/comparison'));p.add_argument('--columns',type=int,default=80);p.add_argument('--rows',type=int,default=40);p.add_argument('--query',default='');p.add_argument('--details-pages',type=int,default=0);p.add_argument('--background',default='#111827');p.add_argument('--foreground',default='#e5e7eb');a=p.parse_args();a.output.mkdir(parents=True,exist_ok=False)
os.environ.update(json.loads(Path('/tmp/web-session.json').read_text()))
def digest(path):
 with open(path,'rb') as source:return hashlib.file_digest(source,'sha256').hexdigest()
(a.output/'capture.json').write_text(json.dumps({'before_sha256':digest('/old-gui2tui'),'after_sha256':digest('/new-gui2tui'),'capture_script_sha256':digest(__file__),'terminal':f'{a.columns}x{a.rows}','layout':'spatial','query':a.query,'details_pages':a.details_pages,'background':a.background,'foreground':a.foreground,'capture':'Two actual PTYs mirrored to xterms and captured on an isolated X11 screen; no GUI operations.'},indent=2))
stage_size=f'{a.columns*16+8}x{a.rows*20}'
stage=subprocess.Popen(['Xvfb',':99','-screen','0',stage_size+'x24','-ac'],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
for _ in range(100):
 if Path('/tmp/.X11-unix/X99').exists():break
 time.sleep(.1)
views=[];windows=[]
for name,binary,x in [('before','/old-gui2tui',0),('after','/new-gui2tui',a.columns*8+4)]:
 master,slave=pty.openpty();fcntl.ioctl(slave,termios.TIOCSWINSZ,struct.pack('HHHH',a.rows,a.columns,0,0));screen=pyte.Screen(a.columns,a.rows)
 ansi=a.output/(name+'.ansi');ansi.touch();lock=threading.Lock()
 with (a.output/(name+'.stderr')).open('wb') as errors:
  proc=subprocess.Popen([binary,'--session','desktop','--layout','spatial','--app','Firefox','--no-mouse'],stdin=slave,stdout=slave,stderr=errors,close_fds=True)
 os.close(slave);view={'name':name,'master':master,'process':proc,'screen':screen,'lock':lock,'last':0};views.append(view)
 def mirror(view=view,ansi=ansi):
  parser=pyte.Stream(view['screen']);decoder=codecs.getincrementaldecoder('utf-8')('replace')
  with ansi.open('ab',buffering=0) as raw:
   while True:
    try:data=os.read(view['master'],65536)
    except OSError:break
    if not data:break
    raw.write(data)
    with view['lock']:parser.feed(decoder.decode(data));view['last']=time.monotonic()
 threading.Thread(target=mirror,daemon=True).start()
 windows.append(subprocess.Popen(['xterm','-title',name,'-fa','DejaVu Sans Mono','-fs','10','-geometry',f'{a.columns}x{a.rows}+{x}+0','-bg',a.background,'-fg',a.foreground,'-e','tail','-c','+1','-f',str(ansi)],env={**os.environ,'DISPLAY':':99'},stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL))
def frame(view):
 with view['lock']:
  screen=view['screen'];return [''.join(screen.buffer[y][x].data for x in range(screen.columns)) for y in range(screen.lines)]
def snapshot(name):
 time.sleep(1.2) # Let xterm's tail mirror catch up with the actual PTY bytes.
 subprocess.run(['ffmpeg','-hide_banner','-loglevel','error','-y','-f','x11grab','-video_size',stage_size,'-i',':99','-frames:v','1',str(a.output/(name+'.png'))],check=True)
try:
 deadline=time.monotonic()+100
 while time.monotonic()<deadline:
  if all(any('GUI2TUI' in line for line in frame(v)) and time.monotonic()-v['last']>3 for v in views):break
  if any(v['process'].poll() is not None for v in views):raise RuntimeError('TUI exited before capture')
  time.sleep(.2)
 else:raise RuntimeError('TUI did not become ready')
 for v in views:(a.output/(v['name']+'.txt')).write_text('\n'.join(frame(v))+'\n')
 snapshot('comparison')
 # F3 opens the canonical operation palette, without invoking a GUI operation.
 for v in views:os.write(v['master'],b'\x1bOR')
 time.sleep(2)
 for v in views:(a.output/(v['name']+'-actions.txt')).write_text('\n'.join(frame(v))+'\n')
 snapshot('actions')
 if a.query:
  for v in views:os.write(v['master'],a.query.encode())
  time.sleep(1)
  for v in views:(a.output/(v['name']+'-search.txt')).write_text('\n'.join(frame(v))+'\n')
  snapshot('search')
 if a.details_pages:
  for _ in range(a.details_pages):os.write(views[1]['master'],b'\x1b[6~')
  time.sleep(1)
  (a.output/'after-details.txt').write_text('\n'.join(frame(views[1]))+'\n')
  snapshot('details')
finally:
 for window in windows:window.terminate()
 for v in views:
  v['process'].terminate()
  try:v['process'].wait(timeout=10)
  except subprocess.TimeoutExpired:v['process'].kill();v['process'].wait()
  os.close(v['master'])
 stage.terminate();stage.wait(timeout=10)
