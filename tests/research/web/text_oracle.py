"""Independent, read-only public AT-SPI text witness for a versioned test.

This runs in the validation container. It never uses G2T receipts or drives GUI
operations, and refuses ambiguous initial targets and changed source identity.
"""
import argparse
import json
import os
from pathlib import Path
import time


def main():
    p = argparse.ArgumentParser()
    p.add_argument('--record', action='store_true')
    p.add_argument('--label', default='Full name')
    p.add_argument('--state', type=Path, default=Path('/tmp/public-text-oracle.json'))
    a = p.parse_args()
    os.environ.update(json.loads(Path('/tmp/web-session.json').read_text()))
    import pyatspi

    def targets():
        queue = list(pyatspi.Registry.getDesktop(0))
        found = []
        while queue:
            node = queue.pop(0)
            try:
                if node.getRoleName() == 'password text':
                    continue
                if node.name == a.label and node.getRoleName() == 'entry':
                    found.append(node)
                queue.extend(node[i] for i in range(node.childCount) if node[i] is not None)
            except (RuntimeError, AttributeError):
                continue
        return found

    nodes = targets()
    if a.record:
        # Browser registration precedes loading its public page tree. Wait for
        # the exact target instead of treating application startup as page ready.
        deadline = time.monotonic() + 30
        while not nodes and time.monotonic() < deadline:
            time.sleep(.1)
            nodes = targets()
    if len(nodes) != 1:
        if not a.record:
            # A modal may remove the background entry from the public tree.
            # Unavailable evidence must fail a Unicode assertion, while an
            # unrelated task can still use its own independent oracle.
            print(json.dumps({'label': a.label, 'verified': False,
                              'error': 'Target unavailable or ambiguous in current public scope'}))
            return
        raise RuntimeError(f'Independent text oracle requires one public entry, found {len(nodes)}')
    node = nodes[0]
    source = {'bus': node.app.bus_name, 'path': node.path}

    if not a.record:
        before = json.loads(a.state.read_text())
        raw = node.queryText().getText(0, -1)
        after = json.loads(a.state.read_text())
        verified = (before == after and after['source'] == source and after['valid']
                    and after['literal'] is not None and after['events']
                    and after['wire'] == raw)
        print(json.dumps({**after, 'label': a.label, 'fresh_text': raw,
                          'verified': bool(verified)}, ensure_ascii=False))
        return

    raw = node.queryText().getText(0, -1)
    ambiguous = any(ord(c) > 0xffff and following == '\ufeff' for c, following in zip(raw, raw[1:]))
    state = {'source': source, 'wire': raw, 'literal': None if ambiguous else raw,
             'cells': None if ambiguous else list(raw),
             'valid': True, 'events': []}

    def save():
        pending = a.state.with_suffix('.pending')
        pending.write_text(json.dumps(state, ensure_ascii=False))
        pending.replace(a.state)

    def changed(event):
        if event.source.app.bus_name != source['bus'] or event.source.path != source['path']:
            return
        try:
            if event.source.getRoleName() == 'password text':
                raise ValueError('Target became secret')
            start, length, text = event.detail1, event.detail2, event.any_data
            if not isinstance(text, str) or start < 0 or length < 0 or start > len(state['wire']):
                raise ValueError('Invalid public text change')
            if not state['valid']:
                return
            if len(text) == length:
                fragment = text
                inserted = list(text)
            elif len(text.encode('utf-16-le')) // 2 == length:
                fragment = ''.join(c + ('\ufeff' if ord(c) > 0xffff else '') for c in text)
                inserted = [cell for c in text for cell in ([c, None] if ord(c) > 0xffff else [c])]
            else:
                raise ValueError('Event span and literal disagree')
            old = state['wire']
            cells = state['cells']
            def boundary(offset):
                return offset == len(old) or cells[offset] is not None
            if event.type.startswith('object:text-changed:insert'):
                if cells is not None:
                    if boundary(start):
                        cells[start:start] = inserted
                    else:
                        cells = None
                state['wire'] = old[:start] + fragment + old[start:]
            elif event.type.startswith('object:text-changed:delete'):
                if start + length > len(old) or (text and old[start:start + length] != fragment):
                    raise ValueError('Deletion disagrees with current public text')
                if start == 0 and length == len(old):
                    cells = []
                elif cells is not None:
                    if boundary(start) and boundary(start + length):
                        del cells[start:start + length]
                    else:
                        cells = None
                state['wire'] = old[:start] + old[start + length:]
            else:
                raise ValueError('Unknown text change')
            state['cells'] = cells
            state['literal'] = None if cells is None else ''.join(c for c in cells if c is not None)
            state['events'].append({'type': event.type, 'start': start, 'length': length, 'text': text})
        except Exception as error:
            state.update(valid=False, literal=None, error=str(error))
        save()

    pyatspi.Registry.registerEventListener(changed, 'object:text-changed')
    save()
    a.state.with_suffix('.ready').touch()
    pyatspi.Registry.start()


if __name__ == '__main__':
    main()
