#!/usr/bin/env python3
"""Browser tasks through native GUI2TUI IDs; independent read-only assertions.
Validation target labels live only in manifests, never production behavior.
"""
import argparse
from dataclasses import dataclass
import hashlib
import json
from pathlib import Path
import subprocess
import shutil
import sys
import time
import uuid

ROOT=Path(__file__).resolve().parents[3]
HERE=Path(__file__).parent

def docker(*args,check=True):
    return subprocess.run(['docker',*map(str,args)],check=check,capture_output=True,text=True,timeout=180)

def events(container):
    return json.loads(docker('exec',container,'python3','-c','import urllib.request;print(urllib.request.urlopen("http://127.0.0.1:8765/events").read().decode())').stdout)

def oracle(container):
    return json.loads(docker('exec',container,'python3','/web-tests/session.py','--oracle').stdout)

def find_operation(observation,plan):
    nodes={n['id']:n for n in observation['nodes']}
    matches=[]
    for op in observation['operations']:
        n=nodes[op['target']]
        if n['label']!=plan['label'] or op['kind']!=plan['kind']:continue
        if any(op.get(k)!=v for k,v in plan.items() if k in ('name','selected','increase')):continue
        matches.append(op)
    if len(matches)!=1:
        raise LookupError(f"Expected one advertised operation for {plan}, found {len(matches)}")
    return dict(matches[0],operation_id=matches[0]['id'],**({'parameter':plan['parameter']} if 'parameter' in plan else {}))

def asserted(task,after_events,observed):
    if 'expect_selection' in task:
        actual=sorted(n['name'] for n in observed['nodes'] if n['role']=='list item' and n['name'] in ['Alpha tag','Beta tag','Gamma tag'] and n['selected'])
        return actual==sorted(task['expect_selection'])
    if 'expect_selected' in task:
        return any(n['name']==task['expect_selected'] and n['selected'] for n in observed['nodes'])
    if 'expect_value' in task:
        e=task['expect_value'];return any(n['name']==e['name'] and n.get('value')==e['value'] for n in observed['nodes'])
    expected=task.get('expect_event')
    if expected is not None:
        return any(row.get('name')==expected['name'] and row.get('value')==expected['value'] for row in after_events)
    expected=task.get('expect_node')
    if expected is not None:
        return any(n['name']==expected['name'] and (expected['value'] is None or n.get('text')==expected['value']) for n in observed['nodes'])
    return False

def run_tasks(args,container,bridge,tasks,provider):
    from jev_anything.native import NativeBridgeError,run
    results=[]
    for task in tasks:
        start=time.monotonic();before=events(container);count=0;status='not_run';error=None
        log=args.output/(task['id']+'.jsonl')
        with log.open('x') as trace:
            def emit(event):trace.write(json.dumps(event,ensure_ascii=False)+'\n');trace.flush()
            try:
                if args.mode=='jev':
                    texts=[p['parameter'] for p in task['plans'] if isinstance(p.get('parameter'),str)]
                    values=[p['parameter'] for p in task['plans'] if isinstance(p.get('parameter'),(int,float))]
                    result=run(task['goal'],bridge,provider,texts=texts,values=values,max_steps=args.max_steps,emit=emit)
                    status=result['status'];count=len(result['history']);error=result.get('error');emit({'kind':'result','result':result})
                else:
                    for plan in task['plans']:
                        deadline=time.monotonic()+2
                        while True:
                            observation=bridge.snapshot();emit({'kind':'observe','observation':observation})
                            try:candidate=find_operation(observation,plan);break
                            except LookupError:
                                if time.monotonic()>=deadline:raise
                                time.sleep(.1)
                        receipt=bridge.execute(observation,candidate);count+=1;emit({'kind':'receipt','candidate':candidate,'receipt':receipt})
                        if receipt['accepted'] is not True:raise RuntimeError('Backend acceptance not established')
                    status='dispatched'
            except LookupError as e:status='capability_unavailable';error=str(e)
            except Exception as e:status='failed';error=str(e)
            # Waits permit observation; time itself never confirms task success.
            until=time.monotonic()+3
            while True:
                actual_events=events(container)[len(before):];observed=oracle(container)
                passed=asserted(task,actual_events,observed)
                if passed or time.monotonic()>=until:break
                time.sleep(.2)
            row={'task':task['id'],'goal':task['goal'],'mode':args.mode,'status':status,'task_completed':passed,
                 'gui_writes':count,'error':error,'elapsed_s':round(time.monotonic()-start,3),
                 'events':actual_events,'oracle':observed}
            emit({'kind':'assertion','assertion':row})
        (args.output/(task['id']+'-result.json')).write_text(json.dumps(row,ensure_ascii=False,indent=2))
        results.append({k:v for k,v in row.items() if k not in ('events','oracle')})
        print(json.dumps(results[-1],ensure_ascii=False),flush=True)
    return results

def main():
    p=argparse.ArgumentParser();p.add_argument('--binary',type=Path,default=ROOT/'target/autonomous-linux/debug/gui2tui-agent');p.add_argument('--image',default='gui2tui-task-tests:local');p.add_argument('--output',type=Path,required=True);p.add_argument('--mode',choices=['direct','jev'],default='direct');p.add_argument('--jev-root',type=Path,default=Path.home()/'Document/Jev-Anything');p.add_argument('--tasks',nargs='+');p.add_argument('--container');p.add_argument('--max-steps',type=int,default=5);a=p.parse_args()
    sys.path.insert(0,str(a.jev_root/'src'))
    from jev_anything.native import NativeBridge
    from jev_anything.jev import JevClient
    a.output.mkdir(mode=0o700,parents=True,exist_ok=False)
    frozen=a.output/'gui2tui-agent';shutil.copy2(a.binary,frozen);a.binary=frozen
    tasks=json.loads((HERE/'deep_tasks.json').read_text())
    if a.tasks:tasks=[t for t in tasks if t['id'] in a.tasks]
    container=a.container or 'g2t-web-'+uuid.uuid4().hex[:10];owned=not a.container;bridge=None
    try:
        if owned:
            docker('run','-d','--name',container,'--platform','linux/amd64','--shm-size=512m','-v',f'{a.binary.resolve()}:/gui2tui-agent:ro','-v',f'{HERE}:/web-tests:ro','--entrypoint','dbus-run-session',a.image,'--','python3','/web-tests/session.py')
        for _ in range(150):
            if docker('exec',container,'test','-f','/tmp/web-ready',check=False).returncode==0:break
            time.sleep(.2)
        bridge=NativeBridge(['docker','exec','-i',container,'python3','/web-tests/session.py','--exec-agent'])
        results=run_tasks(a,container,bridge,tasks,JevClient() if a.mode=='jev' else None)
    finally:
        if bridge:bridge.close()
        if owned:docker('rm','-f',container,check=False)
    summary={'mode':a.mode,'source_base':subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),
             'binary_sha256':hashlib.sha256(a.binary.read_bytes()).hexdigest(),'fixture_sha256':hashlib.sha256((HERE/'workbench.html').read_bytes()).hexdigest(),
             'image':docker('image','inspect',a.image,'--format','{{.Id}}').stdout.strip(),'tasks':results,
             'passed':sum(r['task_completed'] for r in results),'total':len(results)}
    (a.output/'summary.json').write_text(json.dumps(summary,ensure_ascii=False,indent=2));print(json.dumps({k:v for k,v in summary.items() if k!='tasks'}),flush=True)
    return 0 if summary['passed']==summary['total'] else 1
if __name__=='__main__':raise SystemExit(main())
