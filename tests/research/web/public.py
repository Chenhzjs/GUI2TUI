#!/usr/bin/env python3
"""Cross-site live Jev tasks. Public AT-SPI assertions never drive GUI writes."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import sys
import time
import uuid
from run import HERE,ROOT,docker,oracle

def check(expected,observed,writes):
    nodes=[n for n in observed['nodes'] if n['showing'] and not n['secret']]
    if 'document' in expected:return any(n['role']=='document web' and expected['document'].lower() in (n['name'] or '').lower() for n in nodes)
    if 'text' in expected:return any(expected['text'].lower() in (n.get('text') or '').lower() for n in nodes)
    if 'name' in expected:return any(n['name']==expected['name'] and n['role']==expected['role'] for n in nodes)
    if 'absent_name' in expected:return writes>0 and not any(n['name']==expected['absent_name'] for n in nodes)
    if 'checked_count' in expected:return sum(n['role']=='check box' and n['checked'] for n in nodes)==expected['checked_count']
    if 'selected_name' in expected:return any(n['name']==expected['selected_name'] and n['selected'] for n in nodes)
    if 'slider_value' in expected:return any(n['role']=='slider' and n.get('value')==expected['slider_value'] for n in nodes)
    return False

def main():
    p=argparse.ArgumentParser();p.add_argument('--binary',type=Path,default=ROOT/'target/autonomous-linux/debug/gui2tui-agent');p.add_argument('--jev-root',type=Path,default=Path.home()/'Document/Jev-Anything');p.add_argument('--image',default='gui2tui-task-tests:local');p.add_argument('--output',type=Path,required=True);p.add_argument('--tasks',nargs='+');p.add_argument('--max-steps',type=int,default=6);a=p.parse_args()
    sys.path.insert(0,str(a.jev_root/'src'))
    from jev_anything.native import NativeBridge,run
    from jev_anything.jev import JevClient
    a.output.mkdir(mode=0o700,parents=True,exist_ok=False);frozen=a.output/'gui2tui-agent';shutil.copy2(a.binary,frozen)
    tasks=json.loads((HERE/'public_tasks.json').read_text());results=[]
    if a.tasks:tasks=[t for t in tasks if t['id'] in a.tasks]
    for task in tasks:
        container='g2t-public-'+uuid.uuid4().hex[:10];bridge=None;start=time.monotonic();row={'task':task['id'],'url':task['url'],'goal':task['goal'],'task_completed':False,'gui_writes':0}
        try:
            docker('run','-d','--name',container,'--platform','linux/amd64','--shm-size=512m','-v',f'{frozen.resolve()}:/gui2tui-agent:ro','-v',f'{HERE}:/web-tests:ro','--entrypoint','dbus-run-session',a.image,'--','python3','/web-tests/session.py','--url',task['url'])
            for _ in range(150):
                if docker('exec',container,'test','-f','/tmp/web-ready',check=False).returncode==0:break
                time.sleep(.2)
            bridge=NativeBridge(['docker','exec','-i',container,'python3','/web-tests/session.py','--exec-agent'])
            with (a.output/(task['id']+'.jsonl')).open('x') as log:
                def emit(event):log.write(json.dumps(event,ensure_ascii=False)+'\n');log.flush();print(task['id'],event['kind'],event.get('step',''),flush=True)
                result=run(task['goal'],bridge,JevClient(),texts=task.get('texts',[]),values=task.get('values',[]),max_steps=a.max_steps,emit=emit);emit({'kind':'result','result':result})
            observed=oracle(container);row.update(status=result['status'],gui_writes=len(result['history']),error=result.get('error'))
            row['task_completed']=check(task['expect'],observed,row['gui_writes']);(a.output/(task['id']+'-oracle.json')).write_text(json.dumps(observed,ensure_ascii=False,indent=2))
        except Exception as e:row.update(status='infrastructure_failure',error=str(e))
        finally:
            if bridge:bridge.close()
            docker('rm','-f',container,check=False)
        row['elapsed_s']=round(time.monotonic()-start,3);results.append(row);print(json.dumps(row,ensure_ascii=False),flush=True)
        summary={'binary_sha256':hashlib.sha256(frozen.read_bytes()).hexdigest(),'passed':sum(r['task_completed'] for r in results),'total':len(results),'planned':len(tasks),'tasks':results}
        (a.output/'summary.json').write_text(json.dumps(summary,ensure_ascii=False,indent=2))
    return 0 if summary['passed']==summary['total'] else 1
if __name__=='__main__':raise SystemExit(main())
