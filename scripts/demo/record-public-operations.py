#!/usr/bin/env python3
"""Record the GUI beside a live mirror of the actual GUI2TUI PTY.

Run in the isolated recording image under dbus-run-session. All interactions
reuse the PTY task driver; xterm only displays its unmodified output bytes.
"""
import os
from pathlib import Path
import runpy
import subprocess
import sys
import time

root = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(root / 'tests/research/benchmark'))
import scenario_session as session

out = Path(sys.argv[sys.argv.index('--output') + 1])
out.mkdir(parents=True, exist_ok=True)
original_init = session.Terminal.__init__
original_pump = session.Terminal.pump
original_close = session.Terminal.close
capture = None
mirror = None
stream_file = out / 'live-terminal.ansi'

def pump(self, seconds=.15):
    before = len(self.transcript)
    frame = original_pump(self, seconds)
    with stream_file.open('a') as stream:
        stream.write(self.transcript[before:])
    return frame

def initialize(self, *args):
    global capture, mirror
    stream_file.write_text('')
    original_init(self, *args)
    self.child.setwinsize(42, 100)
    self.screen.resize(42, 100)
    self.pump(2)
    windows = subprocess.check_output(['wmctrl', '-l'], text=True).splitlines()
    for window in windows:
        subprocess.run(['wmctrl', '-ir', window.split()[0], '-b', 'remove,maximized_vert,maximized_horz'], check=True)
        subprocess.run(['wmctrl', '-ir', window.split()[0], '-e', '0,0,30,440,900'], check=True)
    mirror = subprocess.Popen(['xterm', '-title', 'GUI2TUI — live PTY output', '-fa', 'DejaVu Sans Mono', '-fs', '11', '-geometry', '100x42+520+30', '-bg', '#111827', '-fg', '#e5e7eb', '-e', 'tail', '-c', '+1', '-f', str(stream_file)])
    time.sleep(2)
    capture = subprocess.Popen(['ffmpeg', '-hide_banner', '-loglevel', 'error', '-y', '-f', 'x11grab', '-framerate', '15', '-video_size', '1440x1000', '-i', os.environ['DISPLAY'], '-c:v', 'libx264', '-preset', 'ultrafast', '-crf', '23', '-pix_fmt', 'yuv420p', str(out / 'operations.mp4')], stdin=subprocess.PIPE)
    self.pump(3)

def close(self, output):
    self.pump(3)
    if capture:
        capture.communicate(b'q', timeout=30)
    original_close(self, output)
    if mirror:
        mirror.terminate()

session.Terminal.__init__ = initialize
session.Terminal.pump = pump
session.Terminal.close = close
if sys.argv[1] == 'fixture':
    runpy.run_path(str(root / 'tests/research/benchmark/public_operations_session.py'), run_name='__main__')
else:
    session.main()
