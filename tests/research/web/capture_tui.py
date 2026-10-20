"""Capture actual PTY output for presentation QA; no GUI operation is sent."""
import argparse,codecs,fcntl,json,os,pty,select,struct,subprocess,termios,time
from pathlib import Path
import pyte
p=argparse.ArgumentParser();p.add_argument('--layout',default='spatial');p.add_argument('--columns',type=int,default=80);p.add_argument('--rows',type=int,default=40);p.add_argument('--name',default='scene');a=p.parse_args()
os.environ.update(json.loads(Path('/tmp/web-session.json').read_text()))
out=Path('/evidence');screen=pyte.Screen(a.columns,a.rows);parser=pyte.Stream(screen);decoder=codecs.getincrementaldecoder('utf-8')('replace')
master,slave=pty.openpty();fcntl.ioctl(slave,termios.TIOCSWINSZ,struct.pack('HHHH',a.rows,a.columns,0,0))
with (out/(a.name+'.stderr')).open('wb') as errors:process=subprocess.Popen(['/gui2tui','--session','desktop','--layout',a.layout,'--app','Firefox','--no-mouse'],stdin=slave,stdout=slave,stderr=errors,close_fds=True)
os.close(slave);end=time.monotonic()+90;last=0;data_seen=False
with (out/(a.name+'.ansi')).open('wb') as raw:
 while time.monotonic()<end and process.poll() is None:
  if select.select([master],[],[],.2)[0]:
   try:data=os.read(master,65536)
   except OSError:break
   if not data:break
   raw.write(data);parser.feed(decoder.decode(data));last=time.monotonic();data_seen=True
  ready=any('GUI2TUI' in ''.join(screen.buffer[y][x].data for x in range(screen.columns)) for y in range(screen.lines))
  if data_seen and ready and time.monotonic()-last>4:break
 frame=[''.join(screen.buffer[y][x].data for x in range(screen.columns)) for y in range(screen.lines)]
 (out/(a.name+'.txt')).write_text('\n'.join(frame)+'\n')
 # Navigate only terminal regions to confirm collapsed panes remain reachable.
 os.write(master,b'\x1b[17~') # F6, never an application/backend operation.
 until=time.monotonic()+2
 while time.monotonic()<until and process.poll() is None:
  if select.select([master],[],[],.1)[0]:
   try:data=os.read(master,65536)
   except OSError:break
   if not data:break
   raw.write(data);parser.feed(decoder.decode(data))
 (out/(a.name+'-next-region.txt')).write_text('\n'.join(''.join(screen.buffer[y][x].data for x in range(screen.columns)) for y in range(screen.lines))+'\n')
 os.write(master,b'q')
 try:process.wait(timeout=10)
 except subprocess.TimeoutExpired:process.terminate();process.wait(timeout=10)
os.close(master)
