#!/usr/bin/env python3
"""Development GUI task scenarios using only public AT-SPI semantics."""
import argparse
import hashlib
import json
import os
import re
import ctypes
import signal
import math
import termios
import threading
from http.server import BaseHTTPRequestHandler, HTTPServer
import subprocess
import time
from pathlib import Path

COMMANDS = {
    'fixture': ['python3', '/fixture.py'],
    "firefox": ["firefox-esr", "--no-remote", "--profile", "/tmp/profile", "about:blank"],
    "mousepad": ["mousepad", "--disable-server"],
    "featherpad": ["featherpad", "/tmp/data/sample.txt"],
    "okular": ["okular", "/tmp/data/sample.pdf"],
}


READ_ERRORS = []


def children(root):
    queue = [root]
    while queue:
        node = queue.pop(0)
        yield node
        try:
            queue.extend(node[i] for i in range(node.childCount) if node[i] is not None)
        except Exception as error:
            READ_ERRORS.append({'field': 'children', 'path': getattr(node, 'path', None), 'error': str(error)})
            continue


def find_unique(app, role, name=None):
    matches = [n for n in children(app) if n.getRoleName() == role and (name is None or n.name == name)]
    if len(matches) != 1:
        raise RuntimeError(f"expected one {role} {name!r}, got {len(matches)}")
    return matches[0]


class Terminal:
    """All writes go to the real GUI2TUI PTY, never to the GUI provider."""
    def __init__(self, binary, app, evidence):
        import pexpect
        import pyte
        self.pexpect = pexpect
        columns = int(os.environ.get('GUI2TUI_TEST_COLUMNS', '320'))
        rows = int(os.environ.get('GUI2TUI_TEST_ROWS', '50'))
        self.screen = pyte.Screen(columns, rows)
        self.stream = pyte.Stream(self.screen)
        self.evidence = evidence
        self.transcript = ""
        started = time.monotonic()
        self.child = pexpect.spawn(binary, ["--log-level", "debug", "--session", "desktop", "--layout", os.environ.get("GUI2TUI_TEST_LAYOUT", "spatial"), "--app", app],
                                  encoding="utf-8", dimensions=(rows, columns), timeout=1)
        self.child.delaybeforesend = 0
        self.wait("? Help", 40)
        self.first_screen_ms = (time.monotonic() - started) * 1000

    def navigation_metrics(self):
        self.send('\x1bOR')
        self.wait('Command palette')
        samples = []
        for _ in range(60):
            previous = next(line for line in self.pump(.1).splitlines() if '┌ Command palette ' in line)
            started = time.monotonic()
            self.child.send('\x1b[B')
            while time.monotonic() - started < 2:
                frame = self.pump(.005)
                current = next((line for line in frame.splitlines() if '┌ Command palette ' in line), '')
                if current and current != previous:
                    break
            samples.append((time.monotonic() - started) * 1000)
        self.send('\x1b')
        return {'first_screen_ms': self.first_screen_ms, 'cached_palette_navigation_ms': samples,
                'navigation_p95_ms': sorted(samples)[math.ceil(.95 * len(samples)) - 1],
                'navigation_max_ms': max(samples), 'sample_count': len(samples),
                'columns': self.screen.columns, 'rows': self.screen.lines,
                'measurement': 'PTY send through rendered palette selection change; no pexpect send delay; includes polling'}

    def pump(self, seconds=.15):
        end = time.monotonic() + seconds
        while time.monotonic() < end:
            try:
                chunk = self.child.read_nonblocking(65536, timeout=.03)
                self.transcript += chunk
                self.stream.feed(chunk)
            except self.pexpect.TIMEOUT:
                pass
        # Debian pyte can retain an orphan wide-character continuation after
        # a differential erase. Preserve valid pairs; render orphan cells blank.
        from wcwidth import wcwidth
        for row in self.screen.buffer.values():
            for column, cell in list(row.items()):
                if not cell.data and (column == 0 or not row[column - 1].data
                                      or wcwidth(row[column - 1].data[0]) != 2):
                    row[column] = cell._replace(data=' ')
        return "\n".join(self.screen.display)

    def wait(self, text, timeout=8):
        end = time.monotonic() + timeout
        while time.monotonic() < end:
            if text in self.pump():
                return
        raise RuntimeError(f"GUI2TUI did not display {text!r}")

    def wait_closed_palette(self):
        end = time.monotonic() + 15
        while time.monotonic() < end:
            if '┌ Command palette ' not in self.pump(.1):
                return
        raise RuntimeError('GUI2TUI palette operation did not finish within 15 seconds')

    def send(self, keys):
        self.child.send(keys)
        return self.pump()

    def command(self, name):
        self.send(":" + name.strip())
        self.wait("Command palette")
        self.send("\x1bOQ")  # F2: explicitly include all application commands
        for _ in range(6):
            frame = self.pump()
            lines = frame.splitlines()
            header = next(i for i, line in enumerate(lines) if "┌ Command palette " in line)
            left = lines[header].index("┌ Command palette ")
            right = lines[header].index("┐", left)
            if "0 commands" in frame:
                raise RuntimeError(f"GUI2TUI command unavailable: {name}")
            self.evidence.append({"palette_frame": frame})
            selected = []
            for line in lines[header + 2:]:
                part = line[left + 1:right].strip()
                if "└" in line[left:left + 1]:
                    break
                if part.startswith("> "):
                    selected.append(part[2:])
                elif selected and not selected[-1].endswith(" › " + name.strip()):
                    selected[-1] += " " + part
            if any(label.split(" › ")[-1].strip() == name.strip() for label in selected):
                self.evidence.append({"command": name.strip(), "frame": frame})
                self.send("\r")
                self.wait_closed_palette()
                return
            self.send("\x1b[B")
        raise RuntimeError(f"no exact selected GUI2TUI command {name!r}")

    def focus(self, label):
        seen = set()
        for _ in range(8):
            for _ in range(12):
                for _ in range(32):
                    frame = self.pump(.03)
                    selected = [part.strip() for line in frame.splitlines() for part in line.split('│')
                                if part.strip().startswith('> ') or part.strip().startswith('[> ')]
                    if any(label in part for part in selected):
                        return
                    if frame in seen:
                        break
                    seen.add(frame)
                    self.send("\t")
                self.send('\x1b[9;5u')  # explicit TUI Ctrl+Tab: sibling pane
            self.send("\x1b[17~")
        raise RuntimeError(f"GUI2TUI focus target unavailable: {label}")

    def public(self, query, operation, invoke=True):
        self.send('\x1bOR')
        self.wait('Command palette')
        self.send(query)
        # Large modal trees (for example a file chooser) can expose more
        # public entries than the compact palette initially shows. Keep
        # walking the finite rendered list until the exact operation is
        # found; this only selects an advertised entry and never guesses an
        # action.
        for _ in range(100):
            frame = self.pump(.1)
            lines = frame.splitlines()
            header = next((i for i, line in enumerate(lines) if '┌ Command palette ' in line), None)
            if header is None:
                # A frame can arrive between clear and redraw.
                continue
            left = lines[header].index('┌ Command palette ')
            right = lines[header].index('┐', left)
            index = next((i for i in range(header + 2, len(lines)) if lines[i][left + 1:right].strip().startswith('> ')), None)
            selected = ''
            if index is not None:
                for line in lines[index:index + 4]:
                    selected += ' ' + line[left + 1:right].strip()
                    if ' › ' in selected:
                        suffix = selected.rsplit(' › ', 1)[1].strip()
                        if suffix == operation or not operation.startswith(suffix):
                            break
            if ' › ' in selected and selected.rsplit(' › ', 1)[1].strip() == operation:
                self.evidence.append({'public_query': query, 'operation': operation, 'frame': frame})
                if not invoke:
                    return
                self.send('\r')
                self.wait_closed_palette()
                return
            self.send('\x1b[B')
        raise RuntimeError('Public operation unavailable: ' + query + ' / ' + operation)

    def close(self, output):
        output.mkdir(parents=True, exist_ok=True)
        final_frame = '\n'.join(self.screen.display)
        try:
            self.child.send('\x03')
            self.child.expect(self.pexpect.EOF, timeout=10)
            self.transcript += self.child.before
            import termios
            flags = termios.tcgetattr(self.child.child_fd)[3]
            self.evidence.append({'terminal_exit': 'graceful', 'canonical_restored': bool(flags & termios.ICANON), 'echo_restored': bool(flags & termios.ECHO)})
        except (self.pexpect.TIMEOUT, self.pexpect.EOF, OSError):
            self.evidence.append({'terminal_exit': 'forced_or_unobserved'})
        (output / "terminal.ansi").write_text(self.transcript)
        (output / "terminal.txt").write_text(final_frame)
        self.child.close(force=True)
        for log in Path("/tmp/runtime").rglob("product.log"):
            (output / "product.log").write_bytes(log.read_bytes())


def observe(app):
    """Discard a stale read snapshot, without replaying any GUI operation."""
    for attempt in range(3):
        try:
            return _observe_once(app)
        except Exception as error:
            if 'does not exist' not in str(error) or attempt == 2:
                raise
            READ_ERRORS.append({'field': 'snapshot', 'attempt': attempt + 1,
                                'error': str(error), 'discarded_snapshot': True})
            app.clearCache()


def _observe_once(app):
    """Test-side read-only oracle. No Action/EditableText/Component mutation."""
    rows = []
    for n in children(app):
        n.clearCache()
        role = n.getRoleName()
        row = {"role": role, "name": n.name, "states": [str(x) for x in n.getState().getStates()], "path": n.path, "interfaces": list(n.get_interfaces()), "child_count": n.childCount}
        row["ancestors"] = []
        parent = n.parent
        for _ in range(12):
            if parent is None:
                break
            row["ancestors"].append({"role": parent.getRoleName(), "name": parent.name})
            parent = parent.parent
        try:
            action = n.queryAction()
            row["actions"] = [action.getName(i) for i in range(action.nActions)]
        except NotImplementedError:
            row["actions"] = []
        if role == "frame":
            try:
                import pyatspi
                rect = n.queryComponent().getExtents(pyatspi.DESKTOP_COORDS)
                row["bounds"] = [rect.x, rect.y, rect.width, rect.height]
            except NotImplementedError:
                pass
        if "password" not in role:
            try:
                t = n.queryText()
                row["text"] = t.getText(0, min(t.characterCount, 4096))
                row['text_selections'] = [list(t.getSelection(i)) for i in range(t.getNSelections())]
                if any("MULTI_LINE" in st for st in row["states"]):
                    row["default_attributes"] = t.getDefaultAttributes()
                    if t.characterCount:
                        row["first_attributes"] = t.getAttributes(0)
            except NotImplementedError:
                pass
        rows.append(row)
    return rows


SCENARIOS = {
    "firefox": {
        "address_navigate": ("Search or enter address", "combo box", "activate"),
        "new_tab": ("Open a new tab (Ctrl+T)", "push button", "press"),
        "reload": ("Reload", "push button", "press"),
        "list_tabs": ("List all tabs", "push button", "press"),
        "firefox_view": ("Firefox View", "toggle button", "press"),
    },
    "mousepad": {
        "find": ("Find", "push button", "click"),
        "new": ("New", "push button", "click"),
        "fullscreen": ("Fullscreen", "push button", "click"),
        "line_numbers": ("Line Numbers      ", "menu item", "click"),
        "word_wrap": ("Word Wrap      ", "menu item", "click"),
    },
    "featherpad": {
        "select_text": ("Select Text", "check box", "Toggle"),
        "new": ("New", "push button", "Press"),
        "reload": ("Reload", "push button", "Press"),
        "find": ("Find", "push button", "Press"),
        "side_pane": ("Side-Pane", "push button", "Press"),
    },
    "okular": {
        "sidebar": ("Show Sidebar", "check box", "Toggle"),
        "browse": ("Browse", "check box", "Toggle"),
        "highlighter": ("Highlighter", "check box", "Toggle"),
        "underline": ("Underline", "check box", "Toggle"),
        "zoom_in": ("Zoom In", "push button", "Press"),
    },
}

WORKFLOWS = {
    'okular': {'document_workflow': ('Open...', 'push button', 'Press')},
    'fixture': {'interface_workflow': ('Named operation', 'push button', 'click')},
    'mousepad': {'text_workflow': ('New', 'push button', 'click')},
    'firefox': {'browser_workflow': ('Search or enter address', 'combo box', 'activate')},
}


def workflow(terminal, app, scenario, result, provider_pid):
    """One application and PTY for every step; observations never deliver input."""
    def checkpoint(name, predicate, allow_unconfirmed=False):
        terminal.pump(2)
        rows = observe(app)
        passed = predicate(rows)
        result['steps'].append({'checkpoint': name, 'passed': passed, 'observation': rows,
                                'frame': terminal.pump(.1)})
        if not passed:
            if allow_unconfirmed:
                result.setdefault('unconfirmed_steps', []).append(name)
            else:
                raise RuntimeError('workflow checkpoint failed: ' + name)
        return rows

    def edit(query, text):
        terminal.public(query, 'Edit text')
        terminal.wait('Editing "')
        terminal.send('\x1b[F')  # End, then clear the bounded current field through TUI
        terminal.send('\x7f' * 128)
        terminal.send(text)
        terminal.send('\x13')
        terminal.pump(2)

    if scenario == 'interface_workflow':
        edit('Input B', 'Only B changed')
        checkpoint('Exact duplicate single-line edit', lambda rows: any(r.get('text') == 'Only B changed' and any(a['name'] == 'Input B' for a in r['ancestors']) for r in rows) and any(r.get('text') == 'Input A original' for r in rows))
        terminal.public('Text A', 'Edit whole text')
        checkpoint('Exact duplicate text object edit', lambda rows: any(r.get('text') == '你好 GUI2TUI\n第二行 Alpha\n' and any(a['name'] == 'Text A' for a in r['ancestors']) for r in rows) and any(r.get('text') == 'Text B\noriginal' for r in rows))
        Path('/tmp/task-editor.py').write_text("import pathlib,sys\npathlib.Path(sys.argv[1]).write_text('Only B changed\\n')\n")
        terminal.public('Text B', 'Edit whole text')
        checkpoint('Non-model-root text edit preserves A', lambda rows: any(r.get('text') == 'Only B changed\n' and any(a['name'] == 'Text B' for a in r['ancestors']) for r in rows) and any(r.get('text') == '你好 GUI2TUI\n第二行 Alpha\n' for r in rows))
        terminal.public('Text B', 'Edit range (insert/delete/replace)')
        terminal.send('\x1b[F' + '\x7f' * 64 + '0:4:新')
        terminal.send('\x13')
        checkpoint('Unicode range replacement', lambda rows: any(r.get('text') == '新 B changed\n' for r in rows))
        terminal.public('Text B', 'Edit range (insert/delete/replace)')
        terminal.send('\x1b[F' + '\x7f' * 64 + '1:1:插入')
        terminal.send('\x13')
        checkpoint('Unicode range insertion', lambda rows: any(r.get('text') == '新插入 B changed\n' for r in rows))
        terminal.public('Text B', 'Edit range (insert/delete/replace)')
        terminal.send('\x1b[F' + '\x7f' * 64 + '1:3:')
        terminal.send('\x13')
        checkpoint('Unicode range deletion', lambda rows: any(r.get('text') == '新 B changed\n' for r in rows))
        terminal.public('Select first character', 'Action: click')
        checkpoint('Nonempty public selection', lambda rows: any(r.get('text') == '新 B changed\n' and r.get('text_selections') == [[0, 1]] for r in rows))
        terminal.public('Text B › Read text selection', 'Read text selection')
        terminal.wait('[0,1): 新')
        terminal.public('Text B › Edit range', 'Edit range (insert/delete/replace)')
        terminal.wait('0:1:')
        terminal.send('替换')
        terminal.send('\x13')
        checkpoint('Replace default selected range', lambda rows: any(r.get('text') == '替换 B changed\n' for r in rows))
        terminal.public('Alpha', 'Add to selection')
        terminal.pump(1)
        terminal.public('Beta', 'Add to selection')
        checkpoint('Multiple selection', lambda rows: sorted(r['name'] for r in rows if r['name'] in ['Alpha', 'Beta', 'Gamma'] and 'SELECTED' in ' '.join(r['states'])) == ['Alpha', 'Beta'])
        terminal.public('Alpha', 'Remove from selection')
        checkpoint('Remove one selection member', lambda rows: sorted(r['name'] for r in rows if r['name'] in ['Alpha', 'Beta', 'Gamma'] and 'SELECTED' in ' '.join(r['states'])) == ['Beta'])
        terminal.public('Public amount', 'Set value')
        terminal.wait('Set value')
        terminal.send('\x1b[F' + '\x7f' * 16 + '37')
        terminal.send('\x13')
        checkpoint('Exact Value parameter', lambda rows: find_unique(app, 'scroll bar', 'Public amount').queryValue().currentValue == 37)
        terminal.public('Public amount', 'Set value')
        terminal.wait('Set value')
        terminal.send('\x1b[F' + '\x7f' * 16 + '101')
        terminal.send('\x13')
        checkpoint('Out-of-range Value unchanged', lambda rows: find_unique(app, 'scroll bar', 'Public amount').queryValue().currentValue == 37)
        terminal.public('Public amount', 'Value increase')
        checkpoint('Value adjustment', lambda rows: find_unique(app, 'scroll bar', 'Public amount').queryValue().currentValue == 42)
        terminal.public('Toggle controls', 'Action: click')
        checkpoint('Dynamic child appears', lambda rows: any(r['name'] == 'Dynamic operation' for r in rows))
        terminal.public('Dynamic operation', 'Action: click')
        checkpoint('New child callable', lambda rows: any(r['name'] == 'Dynamic invoked' for r in rows))
        terminal.public('Toggle controls', 'Action: click')
        checkpoint('Dynamic child removed', lambda rows: not any(r['name'] == 'Dynamic operation' for r in rows))
        terminal.send('\x1bOR')
        terminal.wait('Command palette')
        terminal.send('Dynamic operation')
        terminal.wait('Command palette 0/0')
        terminal.send('\x1b')
        terminal.public('Named operation', 'Action: click')
        checkpoint('Continue after removal', lambda rows: any(r['name'] == 'Invoked through public action' for r in rows))
        other = Terminal('/gui2tui', app.name, [])
        try:
            terminal.public('Public amount', 'Set value')
            other.public('Change amount independently', 'Action: click')
            terminal.send('\x1b[F' + '\x7f' * 16 + '40')
            terminal.send('\x13')
            checkpoint('Value conflict refuses overwrite', lambda rows: find_unique(app, 'scroll bar', 'Public amount').queryValue().currentValue == 55)
            terminal.send('\x1b')
            terminal.public('Text B › Edit range', 'Edit range (insert/delete/replace)')
            other.public('Change text independently', 'Action: click')
            terminal.send('\x1b[F' + '\x7f' * 64 + '0:1:forbidden')
            terminal.send('\x13')
            checkpoint('Range conflict refuses overwrite', lambda rows: any(r.get('text') == 'External text' for r in rows))
            terminal.send('\x1b')
            terminal.public('Toggle controls', 'Action: click')
            terminal.public('Dynamic operation', 'Action: click', invoke=False)
            other.public('Toggle controls', 'Action: click')
            other.public('Toggle controls', 'Action: click')
            terminal.send('\r')
            checkpoint('Old F3 entry cannot invoke replacement', lambda rows: not any(r['name'] == 'Dynamic invoked' for r in rows))
            terminal.public('Dynamic operation', 'Action: click')
            checkpoint('Fresh replacement callable', lambda rows: any(r['name'] == 'Dynamic invoked' for r in rows))
        finally:
            other.child.send('\x03')
            other.child.expect(other.pexpect.EOF, timeout=10)
            other.child.close()
        terminal.public('Public amount', 'Set value')
        terminal.send('\x1b[F' + '\x7f' * 16 + '42')
        terminal.send('\x13')
        terminal.public('Rounded amount', 'Set value')
        terminal.send('\x1b[F' + '\x7f' * 16 + '37')
        terminal.send('\x13')
        terminal.wait('normalized by application')
        checkpoint('Value provider normalization', lambda rows: find_unique(app, 'slider', 'Rounded amount').queryValue().currentValue == 35)
        terminal.public('Reject inserted text', 'Action: click')
        terminal.public('Text B › Edit range', 'Edit range (insert/delete/replace)')
        terminal.send('\x1b[F' + '\x7f' * 64 + '0:1:X')
        terminal.send('\x13')
        terminal.wait('Range edit not confirmed')
        checkpoint('Partial range failure reported', lambda rows: any(r.get('text') == 'xternal text' for r in rows))
        terminal.public('Reject inserted text', 'Action: click')
        terminal.public('Reveal details', 'Action: activate')
        checkpoint('Expand exposes child', lambda rows: any(r['name'] == 'Revealed operation' and 'SHOWING' in ' '.join(r['states']) for r in rows))
        terminal.public('Reveal details', 'Action: activate')
        checkpoint('Collapse hides child', lambda rows: not any(r['name'] == 'Revealed operation' and 'SHOWING' in ' '.join(r['states']) for r in rows))
        terminal.focus('Open confirmation')
        terminal.send('\r')
        checkpoint('Enter opens modal', lambda rows: any(r['name'] == 'Scoped confirmation' and 'MODAL' in ' '.join(r['states']) for r in rows))
        terminal.command('Return to task')
        checkpoint('Command closes modal', lambda rows: not any(r['name'] == 'Scoped confirmation' for r in rows))
        terminal.send('\r')
        checkpoint('Modal return restores invoking focus', lambda rows: any(r['name'] == 'Scoped confirmation' for r in rows))
        terminal.public('Return to task', 'Action: click')
        checkpoint('F3 closes modal consistently', lambda rows: not any(r['name'] == 'Scoped confirmation' for r in rows))
        result['initial_size_responsiveness'] = terminal.navigation_metrics()
        # Freeze only the validation provider: this creates a real blocked
        # public read without inventing a product capability or GUI input.
        terminal.pump(1)
        os.kill(provider_pid, signal.SIGSTOP)
        try:
            terminal.send('r')
            terminal.wait('Reading application', timeout=5)
            started = time.monotonic()
            terminal.send('\x1b')
            terminal.wait('Refresh cancelled', timeout=3)
            result['steps'].append({'checkpoint': 'Cancel blocked read', 'passed': True,
                                    'cancel_ms': (time.monotonic() - started) * 1000})
        finally:
            os.kill(provider_pid, signal.SIGCONT)
        terminal.public('Public amount', 'Value decrease')
        checkpoint('Continue after cancelled read', lambda rows: find_unique(app, 'scroll bar', 'Public amount').queryValue().currentValue == 37)
        os.kill(terminal.child.pid, signal.SIGTSTP)
        deadline = time.monotonic() + 5
        while time.monotonic() < deadline:
            state = Path(f'/proc/{terminal.child.pid}/status').read_text().split('State:', 1)[1].lstrip()
            if state.startswith('T'):
                break
            terminal.pump(.05)
        else:
            raise RuntimeError('TUI did not suspend')
        flags = termios.tcgetattr(terminal.child.child_fd)[3]
        restored = bool(flags & termios.ICANON and flags & termios.ECHO)
        os.kill(terminal.child.pid, signal.SIGCONT)
        if not restored:
            raise RuntimeError('Suspend failed to restore terminal')
        terminal.wait('Resumed;', timeout=10)
        terminal.child.setwinsize(24, 80)
        terminal.screen.resize(24, 80)
        terminal.public('Public amount', 'Value increase')
        checkpoint('Suspend/resume and resize continuation', lambda rows: find_unique(app, 'scroll bar', 'Public amount').queryValue().currentValue == 42)
        result['steps'].append({'checkpoint': 'Suspend terminal restored', 'passed': restored})
        result.update(status='passed', task_completed=True, responsiveness=terminal.navigation_metrics())
        return
    if scenario == 'document_workflow':
        terminal.command('File')
        terminal.pump(2)
        terminal.public('Open...', 'Action: Press')
        checkpoint('PDF open dialog', lambda rows: any(r['role'] in ('dialog', 'file chooser') for r in rows))
        edit('File name', '/tmp/data/sample.pdf')
        terminal.command('Open')
        checkpoint('PDF opened', lambda rows: any('sample.pdf' in r['name'] and r['role'] == 'frame' for r in rows))
        terminal.command('Edit')
        terminal.pump(1)
        terminal.public('Find...', 'Action: Press')
        checkpoint('PDF search field', lambda rows: any('FOCUSED' in ' '.join(r['states']) and 'EDITABLE' in ' '.join(r['states']) for r in rows))
        edit('Edit text', 'Alpha')
        terminal.command('Next')
        checkpoint('PDF search result', lambda rows: any('Alpha' in r.get('text', '') and 'EditableText' not in r['interfaces'] for r in rows), allow_unconfirmed=True)
        page_fields = {r['path'] for r in observe(app) if r.get('text') == '1'
                       and any(a['role'] == 'tool bar' and a['name'] == 'Main Toolbar' for a in r['ancestors'])}
        if not page_fields:
            raise RuntimeError('No public toolbar page field at page 1')
        terminal.command('Go')
        terminal.pump(1)
        terminal.public('Next Page', 'Action: Press')
        checkpoint('PDF next page', lambda rows: any(r['path'] in page_fields and r.get('text') == '2' for r in rows))
        edit('Fit Width', '100%')
        checkpoint('PDF numeric zoom baseline', lambda rows: any(r['role'] == 'combo box' and r['name'] == '100%' for r in rows))
        terminal.command('Zoom In')
        checkpoint('PDF zoom increased', lambda rows: any(r['role'] == 'combo box' and re.fullmatch(r'[0-9,.]+%', r['name']) and float(r['name'].strip('%').replace(',', '')) > 100 for r in rows))
        def sidebar(rows):
            return [any('CHECKED' in state for state in r['states']) for r in rows if r['role'] == 'check box' and r['name'] == 'Show Sidebar']
        previous_sidebar = sidebar(observe(app))
        terminal.command('Show Sidebar')
        checkpoint('PDF sidebar toggled', lambda rows: len(previous_sidebar) == len(sidebar(rows)) == 1 and previous_sidebar != sidebar(rows))
        checkpoint('PDF continued reading', lambda rows: any('Beta' in r.get('text', '') for r in rows), allow_unconfirmed=True)
        result['public_content_evidence'] = [{'path': r['path'], 'role': r['role'], 'name': r['name'], 'text': r.get('text'), 'interfaces': r['interfaces']} for r in observe(app) if any(i in r['interfaces'] for i in ('Text', 'Document', 'Hypertext'))]
        result['final_public_tree'] = observe(app)
        result['final_public_tree_repeat'] = observe(app)
        result['stationary_reads_equal'] = result['final_public_tree'] == result['final_public_tree_repeat']
        result['public_read_errors'] = list(READ_ERRORS)
        result.update(status='unconfirmed_public_content' if result.get('unconfirmed_steps') else 'passed',
                      task_completed=not result.get('unconfirmed_steps'), responsiveness=terminal.navigation_metrics())
        return
    if scenario == 'text_workflow':
        terminal.command('New')
        terminal.pump(2)
        terminal.public('Untitled 2 › Edit whole text', 'Edit whole text')
        checkpoint('Chinese multiline edit', lambda rows: any(r.get('text') == '你好 GUI2TUI\n第二行 Alpha\n' for r in rows))
        terminal.command('Find')
        checkpoint('Find opens editable search', lambda rows: any('EDITABLE' in ' '.join(r['states']) and 'FOCUSED' in ' '.join(r['states']) and 'MULTI_LINE' not in ' '.join(r['states']) for r in rows))
        edit('Edit text', 'Alpha')
        terminal.command('Down')
        checkpoint('Find selects matching text', lambda rows: any(r.get('text') == '你好 GUI2TUI\n第二行 Alpha\n' and r.get('text_selections') for r in rows))
        terminal.public('Untitled 2 › Read text selection', 'Read text selection')
        terminal.wait('Text selection (Unicode characters):')
        selection_rows = observe(app)
        result['steps'].append({'checkpoint': 'Public selection read', 'passed': True,
            'frame': terminal.pump(.1), 'selection_ranges': [r.get('text_selections') for r in selection_rows if r.get('text') == '你好 GUI2TUI\n第二行 Alpha\n']})
        terminal.command('Save As...')
        checkpoint('Save dialog opens', lambda rows: any(r['role'] in ('dialog', 'file chooser') for r in rows))
        edit('Name:', '/tmp/data/workflow-saved.txt')
        deadline = time.monotonic() + 10
        while time.monotonic() < deadline:
            rows = observe(app)
            if any(r['role'] == 'push button' and r['name'] == 'Save'
                   and any(a['name'] == 'Save As' for a in r['ancestors'])
                   and 'SENSITIVE' in ' '.join(r['states']) for r in rows):
                break
            terminal.pump(.2)
        else:
            raise RuntimeError('Save dialog did not enable Save after filename edit')
        terminal.command('Save')
        checkpoint('GUI saved text', lambda rows: Path('/tmp/data/workflow-saved.txt').exists() and Path('/tmp/data/workflow-saved.txt').read_text() == '你好 GUI2TUI\n第二行 Alpha\n')
        # Only the external editor's temporary-copy payload changes here.
        Path('/tmp/task-editor.py').write_text("import pathlib,sys\npathlib.Path(sys.argv[1]).write_text('未保存的修改\\n')\n")
        terminal.public('workflow-saved.txt › Edit whole text', 'Edit whole text')
        checkpoint('Second edit', lambda rows: any(r.get('text') == '未保存的修改\n' for r in rows))
        terminal.command('Close Tab')
        checkpoint('Unsaved close dialog', lambda rows: any(r['name'] == 'Cancel' for r in rows))
        terminal.command('Cancel')
        checkpoint('Cancel retains edited document', lambda rows: any(r.get('text') == '未保存的修改\n' for r in rows))
        terminal.command('Close Tab')
        terminal.pump(2)
        terminal.command("Don't Save")
        checkpoint('Discard closes edited document', lambda rows: not any(r.get('text') == '未保存的修改\n' for r in rows))
        terminal.command('Open...')
        checkpoint('Open dialog', lambda rows: any(r['role'] in ('dialog', 'file chooser') for r in rows))
        terminal.public('File Chooser Widget [Dialog]', 'Action: show_location')
        terminal.pump(2)
        edit('Location', '/tmp/data/workflow-saved.txt')
        terminal.command('Open')
        checkpoint('Reopened saved text', lambda rows: any(r.get('text') == '你好 GUI2TUI\n第二行 Alpha\n' for r in rows))
        result.update(status='passed', task_completed=True, responsiveness=terminal.navigation_metrics())
        return
    else:
        terminal.focus('Search or enter address')
        terminal.send('\r')
        terminal.wait('Editing "')
        terminal.send('http://127.0.0.1:8765/')
        terminal.send('\x13')
        terminal.pump(2)
        terminal.send('\x1b\r')
        checkpoint('Local page navigation', lambda rows: any(r['role'] == 'document web' and r['name'] == 'Workflow form' for r in rows))
        terminal.focus('Message')
        terminal.send('\r')
        terminal.wait('Editing "')
        terminal.send('GUI2TUI browser')
        terminal.send('\x13')
        checkpoint('Form field edit', lambda rows: any(r.get('text') == 'GUI2TUI browser' for r in rows))
        terminal.command('Send message')
        checkpoint('Form submitted', lambda rows: any(r['role'] == 'document web' and r['name'] == 'Workflow result' for r in rows))
        original_tabs = sum(r['role'] == 'page tab' for r in observe(app))
        terminal.public('Open a new tab', 'Action: press')
        checkpoint('New tab in same session', lambda rows: sum(r['role'] == 'page tab' for r in rows) == original_tabs + 1)
        terminal.public('Workflow result', 'Action: switch')
        checkpoint('Switch back to result tab', lambda rows: any(r['role'] == 'page tab' and r['name'] == 'Workflow result' and 'SELECTED' in ' '.join(r['states']) for r in rows))
        terminal.public('New Tab', 'Action: switch')
        checkpoint('Switch to new tab', lambda rows: any(r['role'] == 'page tab' and r['name'] == 'New Tab' and 'SELECTED' in ' '.join(r['states']) for r in rows))
        terminal.public('New Tab › Action: click', 'Action: click')
        checkpoint('Close tab returns to original', lambda rows: sum(r['role'] == 'page tab' for r in rows) == original_tabs and any(r['role'] == 'document web' and r['name'] == 'Workflow result' for r in rows))
        terminal.public('Back [Button]', 'Action: press')
        checkpoint('Return to form', lambda rows: any(r['role'] == 'document web' and r['name'] == 'Workflow form' for r in rows))
        edit('Message', 'continued')
        terminal.command('Send message')
        checkpoint('Continue submitting in original tab', lambda rows: any(r['role'] == 'document web' and r['name'] == 'Workflow result' for r in rows))
        result.update(status='passed', task_completed=True, responsiveness=terminal.navigation_metrics())
        return
    # Remaining required steps must not be reported as completed during discovery.
    result.update(status='incomplete', task_completed=False, remaining='continuous workflow discovery in progress')


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("application", choices=COMMANDS)
    parser.add_argument("--scenario")
    parser.add_argument("--url", default="data:text/html,<title>GUI2TUI destination</title><h1>GUI2TUI destination</h1>")
    parser.add_argument("--binary", required=True)
    parser.add_argument("--layout", choices=["flat", "spatial"], default="flat")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--text", default="GUI2TUI semantic editing\n")
    parser.add_argument('--columns', type=int, default=320)
    parser.add_argument('--rows', type=int, default=50)
    args = parser.parse_args()
    os.environ["GUI2TUI_TEST_LAYOUT"] = args.layout
    os.environ['GUI2TUI_TEST_COLUMNS'] = str(args.columns)
    os.environ['GUI2TUI_TEST_ROWS'] = str(args.rows)
    if args.scenario and args.scenario not in SCENARIOS.get(args.application, {}) and args.scenario not in WORKFLOWS.get(args.application, {}):
        parser.error("unknown scenario for application")
    for path in ("/tmp/profile", "/tmp/runtime"):
        Path(path).mkdir(mode=0o700, exist_ok=True)
    if args.application == "firefox":
        Path("/tmp/profile/user.js").write_text('user_pref("browser.startup.homepage_override.mstone", "ignore");\nuser_pref("datareporting.policy.firstRunURL", "");\nuser_pref("browser.shell.checkDefaultBrowser", false);\n')
    Path("/tmp/data").mkdir(mode=0o700, exist_ok=True)
    Path("/tmp/data/sample.txt").write_text("Alpha paragraph.\nBeta paragraph.\n")
    objects = [b"<< /Type /Catalog /Pages 2 0 R >>", b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
               b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 300] /Contents 4 0 R >>",
               b"<< /Length 0 >>\nstream\n\nendstream"]
    if args.scenario == 'document_workflow':
        COMMANDS['okular'] = ['okular']
        streams = [b'BT /F1 18 Tf 30 240 Td (Alpha first page) Tj ET', b'BT /F1 18 Tf 30 240 Td (Beta second page) Tj ET']
        objects = [b'<< /Type /Catalog /Pages 2 0 R >>', b'<< /Type /Pages /Kids [3 0 R 5 0 R] /Count 2 >>']
        for i, stream in enumerate(streams):
            objects.extend([f'<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 300] /Resources << /Font << /F1 7 0 R >> >> /Contents {4 + 2*i} 0 R >>'.encode(), f'<< /Length {len(stream)} >>\nstream\n'.encode() + stream + b'\nendstream'])
        objects.append(b'<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>')
    pdf = b"%PDF-1.4\n"
    offsets = [0]
    for i, obj in enumerate(objects, 1):
        offsets.append(len(pdf))
        pdf += f"{i} 0 obj\n".encode() + obj + b"\nendobj\n"
    xref = len(pdf)
    pdf += f"xref\n0 {len(objects) + 1}\n0000000000 65535 f \n".encode()
    pdf += b"".join(f"{n:010} 00000 n \n".encode() for n in offsets[1:])
    pdf += f"trailer\n<< /Root 1 0 R /Size {len(objects) + 1} >>\nstartxref\n{xref}\n%%EOF\n".encode()
    Path("/tmp/data/sample.pdf").write_bytes(pdf)
    os.environ.update(DISPLAY=":97", XDG_RUNTIME_DIR="/tmp/runtime", XDG_SESSION_TYPE="x11",
                      NO_AT_BRIDGE="0", GTK_A11Y="atspi", QT_LINUX_ACCESSIBILITY_ALWAYS_ON="1",
                      TERM="xterm-256color", MOZ_ACCESSIBILITY_ATSPI_ENABLED="1", LANG="C.UTF-8", LC_ALL="C.UTF-8")
    subprocess.Popen(["Xvfb", ":97", "-screen", "0", "1440x1000x24"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    xlib = ctypes.CDLL("libX11.so.6")
    xlib.XOpenDisplay.restype = ctypes.c_void_p
    xlib.XCloseDisplay.argtypes = [ctypes.c_void_p]
    for _ in range(50):
        display = xlib.XOpenDisplay(None)
        if display:
            xlib.XCloseDisplay(display)
            break
        time.sleep(.1)
    subprocess.Popen(["openbox"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    time.sleep(0.5)
    subprocess.run(["dbus-update-activation-environment", "DISPLAY", "XDG_RUNTIME_DIR", "NO_AT_BRIDGE", "QT_LINUX_ACCESSIBILITY_ALWAYS_ON", "GTK_A11Y"], check=True)
    for prop in ("IsEnabled", "ScreenReaderEnabled"):
        subprocess.run(["gdbus", "call", "--session", "--dest", "org.a11y.Bus",
                        "--object-path", "/org/a11y/bus", "--method",
                        "org.freedesktop.DBus.Properties.Set", "org.a11y.Status", prop, "<true>"],
                       capture_output=True, check=True)
    if args.application == 'fixture' or (args.application == "mousepad" and args.scenario in (None, "new", "text_workflow")) or (args.application == "featherpad" and args.scenario == "reload"):
        if args.scenario in ('text_workflow', 'interface_workflow'):
            args.text = '你好 GUI2TUI\n第二行 Alpha\n'
        config = Path("/tmp/config/gui2tui")
        config.mkdir(parents=True, exist_ok=True)
        editor = Path("/tmp/task-editor.py")
        editor.write_text("import pathlib,sys\npathlib.Path(sys.argv[1]).write_text(" + repr(args.text) + ")\n")
        (config / "config.toml").write_text('[interaction.complex_text]\nprogram="/usr/bin/python3"\nargs=["/tmp/task-editor.py", "{file}"]\n')
        os.environ["XDG_CONFIG_HOME"] = "/tmp/config"
    reload_requests = []
    if args.application == "firefox" and args.scenario in ('reload', 'browser_workflow'):
        class Page(BaseHTTPRequestHandler):
            def do_GET(self):
                reload_requests.append(self.path)
                body = ("<title>Reload fixture</title><h1>Load " + str(len(reload_requests)) + "</h1>").encode()
                if args.scenario == 'browser_workflow':
                    body = ('<meta charset="utf-8"><title>Workflow result</title><h1>Submitted message</h1>' if self.path.startswith('/submit') else '<meta charset="utf-8"><title>Workflow form</title><form action="/submit"><label>Message<input name="message"></label><button>Send message</button></form>').encode()
                self.send_response(200)
                self.send_header('Content-Type', 'text/html; charset=utf-8')
                self.send_header("Cache-Control", "no-store")
                self.end_headers()
                self.wfile.write(body)
            def log_message(self, *args):
                pass
        server = HTTPServer(("127.0.0.1", 8765), Page)
        threading.Thread(target=server.serve_forever, daemon=True).start()
        if args.scenario == 'reload':
            COMMANDS["firefox"][-1] = "http://127.0.0.1:8765/"
    import pyatspi
    proc = subprocess.Popen(COMMANDS[args.application], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    desktop = pyatspi.Registry.getDesktop(0)
    started = time.monotonic()
    app = None
    while time.monotonic() - started < 30:
        apps = [a for a in desktop if a is not None and a.childCount > 0]
        if len(apps) == 1:
            app = apps[0]
            break
        time.sleep(0.25)
    if app is None:
        raise RuntimeError("expected one accessible application")
    scenario = args.scenario or next(iter(SCENARIOS[args.application]))
    steps = []
    terminal = None
    result = {"application": args.application, "scenario": scenario,
              "driver": "gui2tui_pty", "layout": args.layout, "task_completed": False, "steps": steps,
              "binary_sha256": hashlib.sha256(Path(args.binary).read_bytes()).hexdigest()}
    try:
        time.sleep(.5)
        before = observe(app)
        result["before"] = before
        terminal = Terminal.__new__(Terminal)
        terminal.__init__(args.binary, app.name, steps)
        terminal.pump(3)
        if scenario in WORKFLOWS.get(args.application, {}):
            workflow(terminal, app, scenario, result, proc.pid)
            return
        request_count_before = len(reload_requests)
        name = SCENARIOS[args.application][scenario][0].strip()
        if scenario == "address_navigate":
            terminal.focus(name)
            terminal.send("\r")
            terminal.wait("[editing]")
            terminal.send(args.url)
            terminal.send("\x13")
            terminal.pump(2)
            terminal.send("\x1b\r")  # explicit GUI2TUI Raw Enter, not inferred Submit
        else:
            if args.application == "featherpad" and scenario == "reload":
                terminal.focus("Document:")
                terminal.send("e")
                terminal.pump(3)
                edited = observe(app)
                if not any(r.get("text") == args.text for r in edited):
                    raise RuntimeError("reload precondition: GUI2TUI text edit not confirmed")
            if scenario == "zoom_in":
                terminal.focus("Fit Width")
                terminal.send("\r")
                terminal.wait("[editing]")
                terminal.send("\x7f" * len("Fit Width"))
                terminal.send("100%")
                terminal.send("\x13")
                terminal.pump(3)
                before = observe(app)
                result["before"] = before
            if args.application == "mousepad" and scenario in ("line_numbers", "word_wrap", "fullscreen"):
                terminal.command("Document" if scenario == "word_wrap" else "View")
                terminal.pump(5)
            try:
                terminal.command(name)
            except RuntimeError as error:
                steps.append({"command_unavailable": str(error), "fallback": "find named control in GUI2TUI scene"})
                terminal.send("\x1b")
                terminal.focus(name)
                terminal.send("\r")
            if args.application == "mousepad" and scenario == "new":
                terminal.pump(.8)
                terminal.focus("Document:")
                terminal.send("e")
                terminal.pump(2)
        terminal.pump(8)
        if args.application == "featherpad" and scenario == "reload":
            pending = observe(app)
            if any(r["name"] == "Discard changes" and r["role"] == "push button" for r in pending):
                terminal.command("Discard changes")
                terminal.pump(4)
        after = observe(app)
        result.update(before=before, after=after, operation_dispatched=True)
        # A changed tree alone does not prove task completion.
        result["status"] = "dispatched_requires_task_assertion"
        if scenario == "address_navigate":
            result["task_completed"] = any(r["role"] == "document web" and r["name"] == "GUI2TUI destination" for r in after)
            result["assertion"] = "destination document title exposed by public Accessibility"
        elif args.application == "mousepad" and scenario == "new":
            result["task_completed"] = any(r.get("text") == args.text for r in after) and sum(r["role"] == "page tab" for r in after) > sum(r["role"] == "page tab" for r in before)
            result["assertion"] = "new tab and exact document text"
        elif scenario in ("line_numbers", "word_wrap", "select_text", "sidebar", "browse", "highlighter", "underline"):
            def checked(rows):
                return [any("CHECKED" in state for state in r["states"]) for r in rows if r["name"].strip() == name and r["role"] == SCENARIOS[args.application][scenario][1] and (scenario != "underline" or any(a["name"] == "Annotation Toolbar" for a in r["ancestors"]))]
            old, new = checked(before), checked(after)
            result["task_completed"] = len(old) == len(new) == 1 and old != new
            result["assertion"] = "exact named control checked state changed"
        elif scenario in ("new_tab", "new"):
            result["task_completed"] = sum(r["role"] == "page tab" for r in after) > sum(r["role"] == "page tab" for r in before)
            result["assertion"] = "new public page tab"
        elif args.application == "firefox" and scenario == "reload":
            result["task_completed"] = request_count_before > 0 and len(reload_requests) > request_count_before
            result["assertion"] = "local HTTP fixture received a fresh reload request"
            result["request_counts"] = [request_count_before, len(reload_requests)]
        elif args.application == "featherpad" and scenario == "reload":
            result["task_completed"] = any(r.get("text") == "Alpha paragraph.\nBeta paragraph.\n" for r in after) and not any(r.get("text") == args.text for r in after)
            result["assertion"] = "reload restores disk fixture after GUI2TUI unsaved edit"
        elif scenario == "fullscreen":
            old = [r.get("bounds") for r in before if r["role"] == "frame"]
            new = [r.get("bounds") for r in after if r["role"] == "frame"]
            result["task_completed"] = len(old) == len(new) == 1 and old != new and new[0] == [0, 0, 1440, 1000]
            result["assertion"] = "public window bounds changed to Xvfb screen bounds"
        elif scenario == "list_tabs":
            result["task_completed"] = any(r["role"] == "panel" and r["name"] == "List all tabs" and any("SHOWING" in s for s in r["states"]) for r in after)
            result["assertion"] = "public tab list panel is showing"
        elif scenario == "firefox_view":
            result["task_completed"] = any(r["role"] == "document web" and r["name"] == "Firefox View" for r in after)
            result["assertion"] = "Firefox View document exposed"
        elif scenario == "side_pane":
            def pane_items(rows):
                return [r for r in rows if r["role"] == "list item" and r["name"] == "sample.txt" and any(a["role"] == "split pane" for a in r["ancestors"])]
            result["task_completed"] = not pane_items(before) and len(pane_items(after)) == 1
            result["assertion"] = "document side pane exposes sample.txt list item"
        elif scenario == "zoom_in":
            def zoom(rows):
                return [float(r["name"].strip('%').replace(',', '')) for r in rows if r["role"] == "combo box" and re.fullmatch(r"[0-9,.]+%", r["name"])]
            old, new = zoom(before), zoom(after)
            result["task_completed"] = len(new) == 1 and len(old) == 1 and new[0] > old[0]
            result["assertion"] = "public zoom percentage increased"
        elif scenario == "find":
            result["task_completed"] = any(any("FOCUSED" in st for st in r["states"]) and any("EDITABLE" in st for st in r["states"]) and not any("MULTI_LINE" in st for st in r["states"]) for r in after)
            result["assertion"] = "search focuses a public editable single-line control"
        if "assertion" in result:
            result["status"] = "passed" if result["task_completed"] else "assertion_failed"
        if args.application == 'mousepad' and scenario in ('line_numbers', 'word_wrap'):
            # Preserve the historical assertion; collect fresh public evidence separately.
            terminal.command('Document' if scenario == 'word_wrap' else 'View')
            terminal.pump(2)
            details = []
            for node in children(app):
                node.clearCache()
                if node.name.strip() != name:
                    continue
                details.append({'path': node.path, 'role': node.getRoleName(),
                                'name': node.name, 'attributes': node.getAttributes(),
                                'states': [str(s) for s in node.getState().getStates()],
                                'children': [{'path': c.path, 'name': c.name, 'role': c.getRoleName()} for c in node],
                                'relations': [{'type': str(rel.getRelationType()),
                                               'targets': [{'path': rel.getTarget(i).path, 'name': rel.getTarget(i).name} for i in range(rel.getNTargets())]}
                                              for rel in node.getRelationSet()]})
            result['reopened_menu_diagnostic'] = details
            if scenario == 'word_wrap':
                # The provider exposes wrapping on Text, not CHECKED on this menu.
                # Keep the original assertion separately for historical comparison.
                result['historical_checked_assertion'] = {
                    'status': result['status'], 'task_completed': result['task_completed']}
                def wrap_modes(rows):
                    return {r['path']: dict(item.split(':', 1) for item in
                            r.get('default_attributes', '').split(';') if ':' in item).get('wrap-mode')
                            for r in rows if 'EditableText' in r['interfaces']
                            and any('MULTI_LINE' in state for state in r['states'])}
                old_modes, new_modes = wrap_modes(before), wrap_modes(after)
                changed = [path for path in old_modes.keys() & new_modes.keys()
                           if old_modes[path] == 'none' and new_modes[path] in ('word', 'char', 'word_char')]
                result['wrap_mode_evidence'] = {'before': old_modes, 'after': new_modes}
                result['task_completed'] = len(changed) == 1
                result['assertion'] = 'same public editable text wrap-mode changed from none to wrapping'
                result['status'] = 'passed' if result['task_completed'] else 'assertion_failed'
    except Exception as error:
        import traceback
        result.update(status="failed", error=str(error), error_type=type(error).__name__, traceback=traceback.format_exc())
        try:
            result["after"] = observe(app)
        except Exception as observation_error:
            result['observation_error'] = str(observation_error)
    finally:
        if terminal and hasattr(terminal, "child"):
            terminal.close(args.output)
        args.output.mkdir(parents=True, exist_ok=True)
        log = args.output / "product.log"
        if log.exists():
            result["backend_action_deliveries"] = [
                {"locator": match[0], "action_index": int(match[1]), "accepted": match[2] == "true"}
                for match in re.findall(r"public action delivery returned locator=(\S+) action_index=(\d+) accepted=(true|false)", log.read_text())
            ]
        result['public_read_errors'] = list(READ_ERRORS)
        (args.output / "result.json").write_text(json.dumps(result, indent=2))
    print(json.dumps({k:v for k,v in result.items() if k not in ("before", "after", "steps")}))


if __name__ == "__main__":
    main()
