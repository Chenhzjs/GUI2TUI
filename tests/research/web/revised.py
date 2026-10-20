#!/usr/bin/env python3
"""Opt-in validation revision; never modifies the frozen manifest or runner.

Selectors describe public target evidence, not backend instructions. Every GUI
write still uses an exact observation-local operation through G2T authority.
"""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys
import time
import uuid

import run as frozen

HERE = Path(__file__).resolve().parent
FROZEN_ASSERTED = frozen.asserted


def resolve_plan(observation, plan):
    nodes = {n['id']: n for n in observation['nodes']}

    def has_ancestor(node, expected):
        seen = {node['id']}
        parent = node.get('parent')
        while parent in nodes and parent not in seen:
            seen.add(parent)
            owner = nodes[parent]
            if all(owner.get(k) == v for k, v in expected.items()):
                return True
            parent = owner.get('parent')
        return False

    matches = []
    for op in observation['operations']:
        node = nodes[op['target']]
        if node.get('redacted') or node['label'] != plan['label'] or op['kind'] != plan['kind']:
            continue
        if any(op.get(k) != plan[k] for k in ('name', 'selected', 'increase') if k in plan):
            continue
        if 'target_role' in plan and node['role'] != plan['target_role']:
            continue
        if not set(plan.get('target_states', [])).issubset(node['states']):
            continue
        if 'ancestor' in plan and not has_ancestor(node, plan['ancestor']):
            continue
        matches.append(op)
    if len(matches) != 1:
        raise LookupError(f'Expected one exact advertised operation for {plan}, found {len(matches)}')
    return dict(matches[0], operation_id=matches[0]['id'],
                **({'parameter': plan['parameter']} if 'parameter' in plan else {}))


def asserted_v2(task, after_events, observed):
    if any(e.get('name') in task.get('forbid_events', []) for e in after_events):
        return False
    if 'expect_public_text_literal' in task:
        expected = task['expect_public_text_literal']
        evidence = observed.get('public_text_evidence', {})
        return (evidence.get('verified') is True and evidence.get('label') == expected['label']
                and evidence.get('literal') == expected['value'])
    if 'expect_unique_selected' in task:
        expected = task['expect_unique_selected']
        matches = [n for n in observed['nodes']
                   if all(n.get(k) == v for k, v in expected.items())]
        return len(matches) == 1 and matches[0]['selected']
    return FROZEN_ASSERTED(task, after_events, observed)


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--binary', type=Path, required=True)
    p.add_argument('--image', required=True)
    p.add_argument('--output', type=Path, required=True)
    p.add_argument('--jev-root', type=Path, default=Path.home()/'Document/Jev-Anything')
    p.add_argument('--revision', type=Path, default=HERE/'residual-plans-v2.json')
    a = p.parse_args()
    a.output.mkdir(mode=0o700, parents=True, exist_ok=False)
    binary = a.output/'gui2tui-agent'
    shutil.copy2(a.binary, binary)
    revision = json.loads(a.revision.read_text())
    original = json.loads((HERE/'deep_tasks.json').read_text())
    tasks = [{**t, **revision['overrides'].get(t['id'], {})} for t in original]
    (a.output/'effective-tasks.json').write_text(json.dumps(tasks, ensure_ascii=False, indent=2))
    sys.path.insert(0, str(a.jev_root/'src'))
    from jev_anything.native import NativeBridge
    identity = {
        'version': revision['version'], 'mode': 'direct',
        'source_base': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=frozen.ROOT, text=True).strip(),
        'binary_sha256': hashlib.sha256(binary.read_bytes()).hexdigest(),
        'fixture_sha256': hashlib.sha256((HERE/'workbench.html').read_bytes()).hexdigest(),
        'image_id': frozen.docker('image', 'inspect', a.image, '--format', '{{.Id}}').stdout.strip(),
        'input_sha256': {name: hashlib.sha256((HERE/name).read_bytes()).hexdigest()
                         for name in ['deep_tasks.json', 'run.py', 'residual-plans-v2.json', 'revised.py']},
        'changes': revision['overrides'],
        'model': None, 'provider': 'not called; direct validation only',
        'comparison': 'Separate revised batch; never a replacement for frozen results',
        'revision_file': str(a.revision.resolve()),
        'revision_sha256': hashlib.sha256(a.revision.read_bytes()).hexdigest(),
    }
    if 'public_text_oracle' in revision:
        identity['input_sha256']['text_oracle.py'] = hashlib.sha256((HERE/'text_oracle.py').read_bytes()).hexdigest()
    (a.output/'identity.json').write_text(json.dumps(identity, ensure_ascii=False, indent=2))
    container = 'g2t-web-revised-'+uuid.uuid4().hex[:10]
    bridge = None
    with (a.output/'protocol.jsonl').open('x') as wire:
        class AuditedBridge(NativeBridge):
            def request(self, command, **kwargs):
                wire.write(json.dumps({'request': {'command': command, **kwargs}}, ensure_ascii=False)+'\n')
                wire.flush()
                try:
                    response = super().request(command, **kwargs)
                except Exception as error:
                    wire.write(json.dumps({'error': str(error), 'delivery': getattr(error, 'delivery', 'unknown')})+'\n')
                    wire.flush()
                    raise
                wire.write(json.dumps({'response': response}, ensure_ascii=False)+'\n')
                wire.flush()
                return response

            def execute(self, observation, candidate):
                try:
                    return super().execute(observation, candidate)
                finally:
                    # Read-only evidence after every attempt; never replay a write.
                    self.snapshot()

        try:
            frozen.docker('run', '-d', '--name', container, '--platform', 'linux/amd64', '--shm-size=512m',
                          '-v', f'{binary.resolve()}:/gui2tui-agent:ro',
                          '-v', f'{HERE}:/web-tests:ro', '--entrypoint', 'dbus-run-session',
                          a.image, '--', 'python3', '/web-tests/session.py')
            for _ in range(150):
                if frozen.docker('exec', container, 'test', '-f', '/tmp/web-ready', check=False).returncode == 0:
                    break
                time.sleep(.2)
            else:
                raise RuntimeError('Browser did not become ready')
            bridge = AuditedBridge(['docker', 'exec', '-i', container, 'python3', '/web-tests/session.py', '--exec-agent'])
            if 'public_text_oracle' in revision:
                bridge.snapshot()  # Read-only page initialization before the independent subscriber.
                label = revision['public_text_oracle']['label']
                frozen.docker('exec', '-d', container, 'python3', '/web-tests/text_oracle.py', '--record', '--label', label)
                for _ in range(150):
                    if frozen.docker('exec', container, 'test', '-f', '/tmp/public-text-oracle.ready', check=False).returncode == 0:
                        break
                    time.sleep(.1)
                else:
                    raise RuntimeError('Independent public text oracle did not become ready')
                original_oracle = frozen.oracle
                def with_text_evidence(current_container):
                    observed = original_oracle(current_container)
                    observed['public_text_evidence'] = json.loads(frozen.docker(
                        'exec', current_container, 'python3', '/web-tests/text_oracle.py', '--label', label).stdout)
                    return observed
                frozen.oracle = with_text_evidence
            frozen.find_operation = resolve_plan
            frozen.asserted = asserted_v2
            a.mode = 'direct'
            results = frozen.run_tasks(a, container, bridge, tasks, None)
        finally:
            if bridge:
                bridge.close()
            frozen.docker('rm', '-f', container, check=False)
    summary = {**identity, 'tasks': results, 'passed': sum(t['task_completed'] for t in results), 'total': len(results)}
    (a.output/'summary.json').write_text(json.dumps(summary, ensure_ascii=False, indent=2))
    print(json.dumps({k: summary[k] for k in ('version', 'passed', 'total')}))
    return 0 if summary['passed'] == summary['total'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
