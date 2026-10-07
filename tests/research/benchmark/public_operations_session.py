"""Real GUI2TUI PTY operations; Accessibility is used only for read-only assertions."""
import json
import sys
from pathlib import Path
import scenario_session as session

original_observe = session.observe
observed_app = None
checks = {}

def observe(app):
    global observed_app
    observed_app = app
    rows = original_observe(app)
    values = {}
    for node in session.children(app):
        try:
            values[node.path] = node.queryValue().currentValue
        except NotImplementedError:
            pass
    for row in rows:
        if row['path'] in values:
            row['current_value'] = values[row['path']]
    return rows

def public_command(terminal, name, operation):
    terminal.send('\x1bOR')  # F3
    terminal.wait('Command palette')
    terminal.send(name)
    frame = terminal.pump(.5)
    # Move through the rendered menu, requiring the exact operation before Enter.
    for _ in range(20):
        if any('> ' in line and name in line and operation in line and '[search:' not in line for line in frame.splitlines()):
            break
        terminal.send('\x1b[B')
        frame = terminal.pump(.1)
    else:
        raise RuntimeError('Public operation not shown: ' + name + ' / ' + operation)
    terminal.evidence.append({'public_operation': name, 'operation': operation, 'frame': frame})
    terminal.send('\r')
    terminal.pump(2)

def selected():
    return sorted(x['name'] for x in observe(observed_app) if x['name'] in ['Alpha', 'Beta', 'Gamma'] and any('SELECTED' in st for st in x['states']))

def exercise(terminal, name):
    public_command(terminal, 'Named operation', 'Action: click')
    checks['named_action'] = any(x['name'] == 'Invoked through public action' for x in observe(observed_app))
    public_command(terminal, 'Alpha', 'Add to selection')
    public_command(terminal, 'Beta', 'Add to selection')
    checks['multiple_selection'] = selected() == ['Alpha', 'Beta']
    public_command(terminal, 'Alpha', 'Remove from selection')
    checks['remove_preserves_other_members'] = selected() == ['Beta']
    public_command(terminal, 'Public amount', 'Value increase')
    checks['scrollbar_value'] = any(x['name'] == 'Public amount' and x.get('current_value') == 25 for x in observe(observed_app))

session.observe = observe
session.Terminal.command = exercise
session.COMMANDS['fixture'] = ['python3', '/fixture.py']
session.SCENARIOS['fixture'] = {'exercise': ('Named operation', 'push button', 'click')}
session.main()
out = Path(sys.argv[sys.argv.index('--output') + 1])
r = json.loads((out / 'result.json').read_text())
r['checks'] = checks
r['task_completed'] = len(checks) == 4 and all(checks.values()) and 'error' not in r
if 'error' not in r:
    r['status'] = 'passed' if r['task_completed'] else 'assertion_failed'
(out / 'result.json').write_text(json.dumps(r, indent=2))
print(json.dumps(checks))
