#!/usr/bin/env python3
"""Versioned live-site validation. Site selectors belong only to test manifests.

All delivery uses G2T authority. Independent pyatspi reads judge task outcomes;
neither accepted delivery nor model done is an oracle.
"""
import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import selectors
import shutil
import subprocess
import sys
import time
import uuid

from run import docker, ROOT, HERE
from revised import resolve_plan


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def save(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2))


def descendants(nodes, root):
    index = {n['id']: n for n in nodes}
    def under(node):
        seen = set()
        while node and node['id'] not in seen:
            if node['id'] == root:
                return True
            seen.add(node['id'])
            node = index.get(node.get('parent'))
        return False
    return [n for n in nodes if under(n)]


def matches(node, query, nodes):
    if any(node.get(k) != v for k, v in query.items() if k in ('role', 'name', 'label', 'text', 'value')):
        return False
    for field in ('text', 'name', 'label', 'value'):
        if field + '_contains' in query and query[field + '_contains'] not in (node.get(field) or ''):
            return False
    if not set(query.get('states', [])).issubset(node.get('states', [])):
        return False
    if set(query.get('without_states', [])) & set(node.get('states', [])):
        return False
    if 'ancestor' in query:
        index = {n['id']: n for n in nodes}
        seen = set()
        parent = node.get('parent')
        while parent in index and parent not in seen:
            seen.add(parent)
            if matches(index[parent], query['ancestor'], nodes):
                break
            parent = index[parent].get('parent')
        else:
            return False
    return True


def doc_url(node):
    return next((s[len('DocURL:'):] for s in node.get('document', []) if s.startswith('DocURL:')), None)


def check(expect, observed, snapshot):
    """Conjunctive checks, scoped to one exact public document; no chrome matches."""
    docs = [n for n in observed['nodes'] if n['role'] == 'document web'
            and doc_url(n) == expect['url']]
    if len(docs) != 1:
        return {'passed': False, 'reason': 'expected unique exact document URL',
                'documents': [doc_url(n) for n in observed['nodes'] if n['role'] == 'document web']}
    nodes = descendants(observed['nodes'], docs[0]['id'])
    checks = []
    for query in expect.get('nodes', []):
        found = [n for n in nodes if matches(n, query, nodes)]
        passed = len(found) == query.get('count', 1) if 'count' in query else bool(found)
        checks.append({'query': query, 'passed': passed, 'matched_ids': [n['id'] for n in found]})
    if expect.get('g2t_nodes'):
        gdocs = [n for n in snapshot['nodes'] if n['role'] == 'Document'
                 and n['label'] == docs[0]['name'] and not n.get('redacted')]
        gnodes = descendants(snapshot['nodes'], gdocs[0]['id']) if len(gdocs) == 1 else []
        for query in expect['g2t_nodes']:
            found = [n for n in gnodes if not n.get('redacted') and matches(n, query, gnodes)]
            passed = len(found) == query.get('count', 1) if 'count' in query else bool(found)
            checks.append({'source': 'g2t', 'query': query, 'passed': passed,
                           'matched_ids': [n['id'] for n in found]})
    if expect.get('mirror'):
        query = expect['mirror']
        originals = [n for n in nodes if matches(n, query['oracle'], nodes)]
        # The document title associates the independently read tree with G2T.
        gdocs = [n for n in snapshot['nodes'] if n['role'] == 'Document'
                 and n['label'] == docs[0]['name'] and not n.get('redacted')]
        gnodes = descendants(snapshot['nodes'], gdocs[0]['id']) if len(gdocs) == 1 else []
        exposed = [n for n in gnodes if matches(n, query['g2t'], gnodes)]
        passed = (len(originals) == len(exposed) == 1
                  and originals[0].get('text') == exposed[0].get('value'))
        checks.append({'kind': 'full_public_text_equal', 'passed': passed,
                       'oracle_ids': [n['id'] for n in originals],
                       'g2t_ids': [n['id'] for n in exposed],
                       'oracle_lengths': [len(n.get('text') or '') for n in originals],
                       'g2t_lengths': [len(n.get('value') or '') for n in exposed],
                       'g2t_partial': [n.get('partial') for n in exposed]})
    return {'passed': all(c['passed'] for c in checks), 'checks': checks,
            'document_url': doc_url(docs[0]), 'oracle_errors': observed['errors']}


def read_oracle(container):
    return json.loads(docker('exec', container, 'python3', '/web-tests/real_session.py', '--oracle').stdout)


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--binary', type=Path, required=True)
    p.add_argument('--image', required=True)
    p.add_argument('--output', type=Path, required=True)
    p.add_argument('--manifest', type=Path, default=HERE / 'real_sites_v2.json')
    p.add_argument('--jev-root', type=Path, default=Path.home() / 'Document/Jev-Anything')
    p.add_argument('--mode', choices=['direct', 'jev'], default='direct')
    p.add_argument('--groups', nargs='+')
    a = p.parse_args()
    sys.path.insert(0, str(a.jev_root / 'src'))
    from jev_anything.native import NativeBridge, NativeBridgeError, choose, candidates_for, run
    from jev_anything.jev import JevClient
    a.output.mkdir(mode=0o700, parents=True, exist_ok=False)
    binary = a.output / 'gui2tui-agent'
    shutil.copy2(a.binary, binary)
    manifest = json.loads(a.manifest.read_text())
    save(a.output / 'manifest.json', manifest)
    identity = {'version': manifest['version'], 'created_utc': datetime.now(timezone.utc).isoformat(),
                'argv': sys.argv, 'mode': a.mode, 'binary_sha256': digest(binary),
                'image_id': docker('image', 'inspect', a.image, '--format', '{{.Id}}').stdout.strip(),
                'sources': {}, 'provider': {'called': a.mode == 'jev',
                'status': 'configured' if a.mode == 'jev' else 'blocked; direct evidence only',
                'model': None, 'request_id': None, 'usage': None},
                'fixture': None, 'content_source': 'live remote HTTPS in real browser',
                'oracle': 'independent read-only public AT-SPI; no DOM, CDP or screenshot semantics'}
    for name, repo in [('g2t', ROOT), ('jev', a.jev_root)]:
        for label, args in [('status', ['status', '--short', '--branch']),
                            ('diff', ['diff', '--binary']), ('head', ['rev-parse', 'HEAD'])]:
            (a.output / (name + '-' + label + '.txt')).write_bytes(
                subprocess.check_output(['git', *args], cwd=repo))
        paths = subprocess.check_output(['rg', '--files', 'src'], cwd=repo, text=True).splitlines()
        paths += [n for n in ['Cargo.toml', 'Cargo.lock', 'pyproject.toml'] if (repo / n).exists()]
        for namepath in paths:
            source = repo / namepath
            target = a.output / 'sources' / name / namepath
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(source, target)
            identity['sources'][str(source)] = digest(source)
    for source in [Path(__file__), HERE / 'real_session.py', HERE / 'revised.py', HERE / 'run.py', a.manifest]:
        target = a.output / 'runner-sources' / source.name
        target.parent.mkdir(exist_ok=True)
        shutil.copy2(source, target)
        identity['sources'][str(source.resolve())] = digest(source)
    save(a.output / 'identity.json', identity)
    rows = []
    for group in manifest['groups']:
        if a.groups and group['id'] not in a.groups:
            continue
        out = a.output / group['id']
        out.mkdir()
        container = 'g2t-real-' + uuid.uuid4().hex[:10]
        save(out / 'environment.json', {'container': container, 'initial_url': group['url'],
                                       'image_id': identity['image_id'],
                                       'binary_sha256': identity['binary_sha256']})
        bridge = None
        trace = (out / 'protocol.jsonl').open('x')
        def emit(event):
            trace.write(json.dumps({'utc': datetime.now(timezone.utc).isoformat(), **event},
                                   ensure_ascii=False) + '\n')
            trace.flush()
        class Bridge(NativeBridge):
            def request(self, command, **kwargs):
                request = {'command': command, **kwargs}
                emit({'kind': 'bridge_request', 'request': request})
                self.process.stdin.write(json.dumps(request, ensure_ascii=False) + '\n')
                self.process.stdin.flush()
                with selectors.DefaultSelector() as ready:
                    ready.register(self.process.stdout, selectors.EVENT_READ)
                    if not ready.select(self.timeout):
                        raise RuntimeError('Bridge timeout; delivery unknown')
                line = self.process.stdout.readline()
                if not line:
                    raise RuntimeError('Bridge closed; delivery unknown')
                response = json.loads(line)
                emit({'kind': 'bridge_response', 'response': response})
                if not response.get('ok'):
                    raise NativeBridgeError(response)
                return response
        try:
            docker('run', '-d', '--name', container, '--platform', 'linux/amd64', '--shm-size=512m',
                   '-v', f'{binary.resolve()}:/gui2tui-agent:ro', '-v', f'{HERE}:/web-tests:ro',
                   '--entrypoint', 'dbus-run-session', a.image, '--', 'python3',
                   '/web-tests/real_session.py', '--url', group['url'])
            for _ in range(150):
                if docker('exec', container, 'test', '-f', '/tmp/real-web-ready', check=False).returncode == 0:
                    break
                time.sleep(.2)
            deadline = time.monotonic() + 90
            while True:
                initial = read_oracle(container)
                emit({'kind': 'readiness', 'oracle': initial})
                docs = [n for n in initial['nodes'] if n['role'] == 'document web'
                        and doc_url(n) == group['url'] and 'busy' not in n['states']]
                if len(docs) == 1:
                    break
                if time.monotonic() >= deadline:
                    raise RuntimeError('Live page not ready at expected URL')
                time.sleep(1)
            save(out / 'initial-oracle.json', initial)
            # Browser registration and document readiness precede agent startup.
            bridge = Bridge(['docker', 'exec', '-i', container, 'python3',
                             '/web-tests/real_session.py', '--exec-agent'])
            for task in group['tasks']:
                emit({'kind': 'task', 'task': task})
                row = {'group': group['id'], 'task': task['id'], 'goal': task['goal'],
                       'mode': a.mode, 'model_done': None, 'task_completed': False,
                       'started_utc': datetime.now(timezone.utc).isoformat(),
                       'dispatch_attempts': 0, 'receipts': [], 'status': 'not_started'}
                task_valid = True
                started = time.monotonic()
                try:
                    before = read_oracle(container)
                    snapshot = bridge.snapshot()
                    save(out / (task['id'] + '-before.json'), {'oracle': before, 'snapshot': snapshot})
                    row['observation_partial'] = snapshot['partial']
                    if 'before' in task:
                        row['precondition'] = check(task['before'], before, snapshot)
                        if not row['precondition']['passed']:
                            task_valid = False
                            raise LookupError('Task initial state differs; do not act')
                    # Exercise the actual full Jev input builder without calling a provider.
                    class InputRecorder:
                        model = None
                        metadata = {}
                        def decide(self, **request):
                            save(out / (task['id'] + '-decision-input.json'), request)
                            return {'action': {'type': 'choice', 'choice': 'abstain'}}
                    if a.mode == 'direct':
                        texts = [p['parameter'] for p in task.get('plans', []) if isinstance(p.get('parameter'), str)]
                        values = [p['parameter'] for p in task.get('plans', []) if isinstance(p.get('parameter'), (int, float))]
                        choose(InputRecorder(), task['goal'], snapshot, candidates_for(snapshot, texts, values), [])
                        for step, plan in enumerate(task.get('plans', [])):
                            if step >= task['max_steps']:
                                raise RuntimeError('Direct step limit')
                            snapshot = bridge.snapshot()
                            candidate = resolve_plan(snapshot, plan)
                            emit({'kind': 'direct_choice', 'task': task['id'], 'candidate': candidate})
                            row['dispatch_attempts'] += 1
                            receipt = bridge.execute(snapshot, candidate)
                            row['receipts'].append(receipt)
                            if receipt.get('accepted') is not True:
                                row['status'] = 'unknown'
                                break
                        else:
                            row['status'] = 'read_only' if not task.get('plans') else 'dispatched'
                    else:
                        provider = JevClient()
                        identity['provider'] = {'called': True, 'model': provider.model,
                                                'endpoint': provider.config.api_url}
                        save(a.output / 'identity.json', identity)
                        result = run(task['goal'], bridge, provider, texts=task.get('texts', []),
                                     values=task.get('values', []), max_steps=task['max_steps'], emit=emit)
                        row['status'] = result['status']
                        row['model_done'] = result.get('decision', {}).get('choice') == 'done'
                        row['receipts'] = [h['receipt'] for h in result.get('history', [])]
                        emit({'kind': 'model_result', 'result': result})
                except Exception as error:
                    row.update(status='unknown' if getattr(error, 'delivery', None) == 'unknown' else 'failed',
                               error=type(error).__name__ + ': ' + str(error))
                # Fresh independent public observation judges the result, including unknown writes.
                deadline = time.monotonic() + task.get('settle_seconds', 8)
                while True:
                    observed = read_oracle(container)
                    snapshot = bridge.snapshot()
                    verdict = check(task['after'], observed, snapshot)
                    emit({'kind': 'independent_oracle', 'task': task['id'], 'oracle': observed,
                          'verdict': verdict})
                    if verdict['passed'] or time.monotonic() >= deadline:
                        break
                    time.sleep(.5)
                row.update(task_completed=task_valid and verdict['passed'], oracle=verdict,
                           finished_utc=datetime.now(timezone.utc).isoformat(),
                           elapsed_s=round(time.monotonic() - started, 3))
                save(out / (task['id'] + '-after.json'), {'oracle': observed, 'snapshot': snapshot})
                save(out / (task['id'] + '-result.json'), row)
                rows.append(row)
                print(json.dumps({k: v for k, v in row.items() if k not in ('receipts', 'oracle')}, ensure_ascii=False), flush=True)
        except Exception as error:
            emit({'kind': 'environment_failure', 'error': type(error).__name__ + ': ' + str(error)})
            for task in group['tasks']:
                if not any(r['group'] == group['id'] and r['task'] == task['id'] for r in rows):
                    rows.append({'group': group['id'], 'task': task['id'], 'task_completed': False,
                                 'status': 'environment_failure', 'error': str(error)})
        finally:
            if bridge:
                try:
                    bridge.close()
                except OSError as error:
                    emit({'kind': 'bridge_close_error', 'error': str(error)})
            logs = docker('logs', container, check=False)
            (out / 'session.log').write_text(logs.stdout + logs.stderr)
            docker('rm', '-f', container, check=False)
            trace.close()
        summary = {'version': manifest['version'], 'mode': a.mode,
                   'passed': sum(r['task_completed'] for r in rows), 'total': len(rows), 'tasks': rows,
                   'model_validation': 'not run; provider blocked' if a.mode == 'direct' else 'see decisions'}
        save(a.output / 'summary.json', summary)
    return 0 if all(r['task_completed'] for r in rows) else 1


if __name__ == '__main__':
    raise SystemExit(main())
