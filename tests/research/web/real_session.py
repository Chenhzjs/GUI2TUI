"""External-site validation session; no fixture server or browser automation API."""
import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import time


def oracle():
    import pyatspi
    rows, errors = [], []
    queue = [(app, None, False) for app in pyatspi.Registry.getDesktop(0)
             if 'firefox' in app.name.lower()]
    while queue:
        node, parent, secret = queue.pop(0)
        try:
            role = node.getRoleName()
            secret = secret or role == 'password text'
            if secret:
                continue
            state = node.getState()
            row_id = len(rows)
            row = {'id': row_id, 'path': node.path, 'parent': parent, 'role': role, 'name': node.name,
                   'states': [re.search(r'STATE_([A-Z_]+)', str(s)).group(1).lower()
                              for s in state.getStates()]}
            for name, query in [('text', lambda: node.queryText().getText(0, -1)),
                                ('value', lambda: node.queryValue().currentValue),
                                ('document', lambda: node.queryDocument().getAttributes())]:
                try:
                    row[name] = query()
                except NotImplementedError:
                    pass
                except Exception as error:
                    row[name + '_error'] = type(error).__name__
            rows.append(row)
            queue.extend((child, row_id, False) for child in node if child is not None)
        except Exception as error:
            errors.append(type(error).__name__ + ': ' + str(error))
    return {'source': 'independent read-only pyatspi', 'nodes': rows, 'errors': errors}


def main():
    p = argparse.ArgumentParser()
    p.add_argument('--url')
    p.add_argument('--oracle', action='store_true')
    p.add_argument('--exec-agent', action='store_true')
    a = p.parse_args()
    env_file = Path('/tmp/real-web-session.json')
    if a.oracle or a.exec_agent:
        os.environ.update(json.loads(env_file.read_text()))
        if a.oracle:
            print(json.dumps(oracle(), ensure_ascii=False))
            return
        os.execv('/gui2tui-agent', ['/gui2tui-agent', '--session', 'desktop', '--app',
                                  'Firefox', '--max-nodes', '100000'])
    if not a.url or not a.url.startswith('https://'):
        p.error('An external HTTPS URL is required')
    os.environ.update(DISPLAY=':98', GTK_A11Y='atspi', NO_AT_BRIDGE='0',
                      MOZ_ACCESSIBILITY_ATSPI_ENABLED='1', XDG_RUNTIME_DIR='/tmp/real-web-runtime')
    Path(os.environ['XDG_RUNTIME_DIR']).mkdir(mode=0o700)
    env_file.write_text(json.dumps({k: os.environ[k] for k in [
        'DISPLAY', 'GTK_A11Y', 'NO_AT_BRIDGE', 'MOZ_ACCESSIBILITY_ATSPI_ENABLED',
        'XDG_RUNTIME_DIR', 'DBUS_SESSION_BUS_ADDRESS']}))
    env_file.chmod(0o600)
    processes = []
    for name, command in [('xvfb', ['Xvfb', ':98', '-screen', '0', '1440x1100x24', '-ac'])]:
        processes.append(subprocess.Popen(command, stdout=open('/tmp/' + name + '.log', 'w'),
                                          stderr=subprocess.STDOUT))
    for _ in range(100):
        if Path('/tmp/.X11-unix/X98').exists():
            break
        time.sleep(.1)
    processes.append(subprocess.Popen(['openbox'], stdout=open('/tmp/openbox.log', 'w'),
                                      stderr=subprocess.STDOUT))
    profile = Path('/tmp/real-web-profile')
    profile.mkdir()
    (profile / 'user.js').write_text('\n'.join([
        'user_pref("accessibility.force_disabled", -1);',
        'user_pref("browser.shell.checkDefaultBrowser", false);',
        'user_pref("browser.startup.homepage_override.mstone", "ignore");',
        'user_pref("browser.aboutwelcome.enabled", false);',
        'user_pref("datareporting.policy.dataSubmissionPolicyBypassNotification", true);']))
    browser = subprocess.Popen(['firefox-esr', '--no-remote', '--profile', str(profile), a.url],
                               stdout=open('/tmp/firefox.log', 'w'), stderr=subprocess.STDOUT)
    Path('/tmp/real-web-ready').touch()
    try:
        browser.wait()
    finally:
        for process in processes:
            process.terminate()


if __name__ == '__main__':
    main()
