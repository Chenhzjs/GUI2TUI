#!/usr/bin/env python3
"""Each task gets a fresh container and drives the built GUI2TUI via PTY."""
import argparse
import concurrent.futures
import json
from pathlib import Path
import subprocess
import uuid
from scenario_session import SCENARIOS
import hashlib


def main():
    p = argparse.ArgumentParser()
    p.add_argument('--image', required=True)
    p.add_argument('--layout', choices=['flat','spatial'], default='flat')
    p.add_argument('--binary', type=Path, required=True)
    p.add_argument('--output', type=Path, required=True)
    p.add_argument('--apps', nargs='+', choices=list(SCENARIOS), default=list(SCENARIOS))
    p.add_argument('--tasks', nargs='+', help='Optional scenario names to run')
    args = p.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    script = args.output.resolve() / 'scenario_session.py'
    script.write_bytes(Path(__file__).with_name('scenario_session.py').read_bytes())
    script_hash = hashlib.sha256(script.read_bytes()).hexdigest()
    image = subprocess.check_output(['docker', 'image', 'inspect', args.image, '--format', '{{.Id}}'], text=True).strip()
    def run(task):
        app, scenario = task
        out = args.output.resolve() / (app + '-' + scenario)
        out.mkdir()
        name = 'gui2tui-task-' + uuid.uuid4().hex
        cmd = ['docker', 'run', '--rm', '--name', name, '--platform', 'linux/amd64',
               '--network', 'none', '--shm-size=512m',
               '-v', f'{script}:/scenario.py:ro', '-v', f'{args.binary.resolve()}:/gui2tui:ro',
               '-v', f'{out}:/evidence', '--entrypoint', 'dbus-run-session', image,
               '--', 'python3', '/scenario.py', app, '--scenario', scenario,
               '--binary', '/gui2tui', '--layout', args.layout, '--output', '/evidence']
        try:
            result = subprocess.run(cmd, capture_output=True, text=True, timeout=90)
            (out / 'stdout.txt').write_text(result.stdout)
            (out / 'stderr.txt').write_text(result.stderr)
            if not (out / 'result.json').exists():
                (out / 'result.json').write_text(json.dumps(dict(application=app, scenario=scenario,
                    status='infrastructure_failure', task_completed=False, returncode=result.returncode)))
        except subprocess.TimeoutExpired:
            (out / 'result.json').write_text(json.dumps(dict(application=app, scenario=scenario,
                status='timeout', task_completed=False)))
        finally:
            subprocess.run(['docker', 'rm', '-f', name], capture_output=True)
        row = json.loads((out / 'result.json').read_text())
        row = {k:v for k,v in row.items() if k not in ('steps', 'before', 'after')}
        print(json.dumps(row), flush=True)
        return row
    tasks = [(a,s) for a in args.apps for s in SCENARIOS[a] if not args.tasks or s in args.tasks]
    with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:
        results = list(pool.map(run, tasks))
    (args.output / 'summary.json').write_text(json.dumps(dict(image=image, script_sha256=script_hash, tasks=results), indent=2))


if __name__ == '__main__':
    main()
