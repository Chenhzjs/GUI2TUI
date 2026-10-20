"""Record real X11 windows; pixels are presentation evidence only."""
import argparse,codecs,fcntl,json,os,pty,struct,subprocess,termios,threading,time
import pyte
from pathlib import Path
os.environ.update(json.loads(Path('/tmp/web-session.json').read_text()))
arguments=argparse.ArgumentParser();arguments.add_argument('--focus-label',default='Full name');arguments.add_argument('--scroll-steps',type=int,default=60);args=arguments.parse_args()
out=Path('/evidence');out.mkdir(exist_ok=True)
screen=pyte.Screen(80,52);parser=pyte.Stream(screen);decoder=codecs.getincrementaldecoder('utf-8')('replace')
screen_lock=threading.Lock()
def frame():
 # pyte.display indexes char[0] even for an orphan wide-character stub.
 # This tolerant, locked view is used only for read-only TUI navigation;
 # the recorded ANSI bytes and terminal rendering remain untouched.
 with screen_lock:
  return [''.join(screen.buffer[y][x].data for x in range(screen.columns)) for y in range(screen.lines)]
ansi=out/'tui-live.ansi';ansi.write_bytes(b'');(out/'jev-live.ansi').touch()
master,slave=pty.openpty();fcntl.ioctl(slave,termios.TIOCSWINSZ,struct.pack('HHHH',52,80,0,0))
with open(out/'tui-stderr.log','wb') as errors:
 tui=subprocess.Popen(['/gui2tui','--session','desktop','--layout','flat','--app','Firefox','--no-mouse'],stdin=slave,stdout=slave,stderr=errors,close_fds=True)
os.close(slave)
def mirror():
 with ansi.open('ab',buffering=0) as stream:
  while True:
   try:data=os.read(master,65536)
   except OSError:break
   if not data:break
   stream.write(data)
   with screen_lock:parser.feed(decoder.decode(data))
threading.Thread(target=mirror,daemon=True).start()
stage=subprocess.Popen(['Xvfb',':99','-screen','0','1280x1000x24','-ac'])
for _ in range(100):
 if Path('/tmp/.X11-unix/X99').exists():break
 time.sleep(.1)
windows=[]
for title,file,x in [('GUI2TUI - actual live PTY',ansi,0),('JEV - actual structured decisions',out/'jev-live.ansi',640)]:
 windows.append(subprocess.Popen(['xterm','-title',title,'-fa','DejaVu Sans Mono','-fs','10','-geometry',f'80x52+{x}+0','-bg','#111827','-fg','#e5e7eb','-e','tail','-c','+1','-f',str(file)],env={**os.environ,'DISPLAY':':99'}))
for _ in range(100):
 listed=subprocess.check_output(['wmctrl','-l'],text=True).splitlines()
 if len(listed)>=1 and ansi.stat().st_size>100:break
 time.sleep(.2)
for line in listed:
 identity=line.split()[0]
 subprocess.run(['wmctrl','-ir',identity,'-b','remove,maximized_vert,maximized_horz'],check=True)
 subprocess.run(['wmctrl','-ir',identity,'-e','0,0,0,640,1000'],check=True)
# Read-only terminal navigation: inspect each current rendered focus before
# advancing. These bytes go only to the TUI PTY, never to the GUI backend.
for _ in range(80):
 if any(args.focus_label in line and '>' in line for line in frame()):break
 os.write(master,b'\t');time.sleep(.15)
for _ in range(args.scroll_steps):
 locations=[i for i,line in enumerate(frame()) if args.focus_label in line]
 if locations and min(locations)<=8:break
 os.write(master,b'\x1b[B');time.sleep(.08)
(out/'tui-start-frame.txt').write_text('\n'.join(frame()))
time.sleep(2)
with open(out/'ffmpeg.log','wb') as log:
 capture=subprocess.Popen(['ffmpeg','-hide_banner','-loglevel','error','-y','-f','x11grab','-framerate','12','-video_size','640x1000','-i',os.environ['DISPLAY'],'-f','x11grab','-framerate','12','-video_size','1280x1000','-i',':99','-filter_complex','[0:v][1:v]hstack=inputs=2[v]','-map','[v]','-c:v','libx264','-preset','ultrafast','-crf','25','-pix_fmt','yuv420p',str(out/'raw.mp4')],stdin=subprocess.PIPE,stdout=log,stderr=log)
(out/'capture-start.json').write_text(json.dumps({'time':time.time()}))
while not (out/'stop').exists() and capture.poll() is None:time.sleep(.2)
if capture.poll() is None:capture.communicate(b'q',timeout=40)
for window in windows:window.terminate()
tui.terminate();tui.wait(timeout=10);stage.terminate()
(out/'capture-done').touch()
