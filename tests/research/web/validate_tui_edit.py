"""Exercise an exact visible F3 text target; verify cancel and write via AT-SPI.

Validation only. Navigation keys go to GUI2TUI's PTY, never to the GUI.
The GUI write uses the selected public EditText operation and normal authority.
"""
import argparse
import codecs
import fcntl
import json
import os
from pathlib import Path
import pty
import re
import select
import struct
import subprocess
import termios
import time

import pyte
from session import oracle


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--field', required=True)
    parser.add_argument('--value', required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    assert args.value and args.value.isascii() and args.value.isprintable()
    assert args.field and args.field.isprintable()
    args.output.mkdir(parents=True, exist_ok=False)
    os.environ.update(json.loads(Path('/tmp/web-session.json').read_text()))
    screen = pyte.Screen(80, 40)
    stream = pyte.Stream(screen)
    decoder = codecs.getincrementaldecoder('utf-8')('replace')
    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', 40, 80, 0, 0))
    errors = (args.output / 'stderr.txt').open('wb')
    raw = (args.output / 'terminal.ansi').open('wb')
    proc = subprocess.Popen(
        ['/new-gui2tui', '--session', 'desktop', '--layout', 'spatial', '--app', 'Firefox', '--no-mouse'],
        stdin=slave, stdout=slave, stderr=errors, close_fds=True,
    )
    os.close(slave)

    def frame():
        return '\n'.join(''.join(screen.buffer[y][x].data for x in range(80)) for y in range(40))

    def pump(seconds):
        end = time.monotonic() + seconds
        while time.monotonic() < end:
            if proc.poll() is not None:
                raise RuntimeError('GUI2TUI exited')
            if select.select([master], [], [], min(.1, max(0, end-time.monotonic())))[0]:
                data = os.read(master, 65536)
                raw.write(data)
                raw.flush()
                stream.feed(decoder.decode(data))

    def wait_for(predicate, timeout=15):
        end = time.monotonic() + timeout
        while time.monotonic() < end:
            pump(.2)
            if predicate():
                return
        raise AssertionError('Expected TUI state did not appear:\n' + frame())

    def capture(name):
        (args.output / (name + '.txt')).write_text(frame() + '\n')

    def field_value(name):
        observed = oracle()
        (args.output / (name + '-oracle.json')).write_text(json.dumps(observed, indent=2))
        matches = [n for n in observed['nodes'] if n['showing'] and not n['secret']
                   and n['name'] == args.field and n['role'] in ('entry', 'text') and 'text' in n]
        assert len(matches) == 1, f'Expected one public text field, found {len(matches)}'
        return matches[0]['text']

    def begin_edit():
        os.write(master, b'\x1bOR')
        wait_for(lambda: 'Command palette' in frame())
        os.write(master, args.field.encode())
        wait_for(lambda: 'Search: ' + args.field in frame())
        target = re.compile(r'> #\d+\s+Edit text\s+·\s+\[TextInput\]\s+' + re.escape(args.field) + r'\s*$')
        for _ in range(40):
            selected = [line.strip(' │') for line in frame().splitlines() if '> #' in line]
            if any(target.fullmatch(line) for line in selected):
                capture('selected-exact-target')
                os.write(master, b'\r')
                wait_for(lambda: 'Edit text' in frame() and '[editing] |' in frame())
                return
            # One terminal-list step, followed by observation; no blind GUI keys.
            os.write(master, b'\x1b[B')
            pump(.3)
        raise AssertionError('Exact advertised Edit text target was not visibly selected')

    result = {'field': args.field, 'requested': args.value, 'passed': False}
    try:
        wait_for(lambda: 'GUI2TUI' in frame() and 'F6 Regions' in frame(), timeout=110)
        pump(2)
        capture('initial')
        assert field_value('before') == '', 'Test requires an initially empty field'
        begin_edit()
        os.write(master, b'cancelled-preview')
        wait_for(lambda: '[editing] cancelled-preview|' in frame())
        capture('local-edit-cancel')
        os.write(master, b'\x1b')
        wait_for(lambda: '[editing]' not in frame())
        assert field_value('cancel') == ''
        result['cancel_preserved_gui'] = True
        begin_edit()
        os.write(master, args.value.encode())
        wait_for(lambda: '[editing] ' + args.value + '|' in frame())
        capture('local-edit-commit')
        os.write(master, b'\x13')  # Ctrl+S: explicit CommitOnly, no Submit.
        wait_for(lambda: '[editing]' not in frame(), timeout=30)
        pump(2)
        capture('after')
        result['readback'] = field_value('after')
        assert result['readback'] == args.value
        result['passed'] = True
    except Exception as exc:
        result['error'] = str(exc)
        capture('failure')
        raise
    finally:
        (args.output / 'result.json').write_text(json.dumps(result, indent=2))
        proc.terminate()
        try:
            proc.wait(timeout=10)
        except subprocess.TimeoutExpired:
            proc.kill()
            proc.wait()
        os.close(master)
        raw.close()
        errors.close()
    print(json.dumps(result))


if __name__ == '__main__':
    main()
