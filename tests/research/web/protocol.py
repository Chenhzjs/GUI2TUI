#!/usr/bin/env python3
"""Native-protocol authority and pre-dispatch failure checks on a real browser."""
import argparse,json,sys
from pathlib import Path
from run import find_operation
p=argparse.ArgumentParser();p.add_argument('--container',required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args()
sys.path.insert(0,str(Path.home()/'Document/Jev-Anything/src'))
from jev_anything.native import NativeBridge,NativeBridgeError
b=NativeBridge(['docker','exec','-i',a.container,'python3','/web-tests/session.py','--exec-agent']);results=[]
def rejected(name,command,expected='not_attempted',**args):
 try:r=b.request(command,**args);results.append(dict(test=name,passed=False,response=r))
 except NativeBridgeError as e:results.append(dict(test=name,passed=e.delivery==expected,delivery=e.delivery,error=str(e)))
try:
 s=b.snapshot();op=find_operation(s,dict(label='Full name',kind='replace_text'))
 rejected('missing-text-parameter','execute',observation_id=s['observation_id'],operation_id=op['id'])
 rejected('consumed-observation','execute',observation_id=s['observation_id'],operation_id=op['id'],parameter='No replay')
 s=b.snapshot();op=find_operation(s,dict(label='Volume',kind='set_value'))
 rejected('invalid-number-parameter','execute',observation_id=s['observation_id'],operation_id=op['id'],parameter='not a number')
 s=b.snapshot();op=find_operation(s,dict(label='Increase counter',kind='action',name='press'))
 rejected('unexpected-action-parameter','execute',observation_id=s['observation_id'],operation_id=op['id'],parameter='bad')
 s=b.snapshot();rejected('unknown-operation','execute',observation_id=s['observation_id'],operation_id='invented')
 s=b.snapshot();rejected('wrong-observation','execute',observation_id=s['observation_id']+100,operation_id='op-0')
 s=b.snapshot();results.append(dict(test='secret-redaction',passed='G2T-DO-NOT-EXPORT-9371' not in json.dumps(s)))
 results.append(dict(test='readonly-not-editable',passed=not any(o['kind']=='replace_text' and next(n for n in s['nodes'] if n['id']==o['target'])['label']=='Read only reference' for o in s['operations'])))

 actor=NativeBridge(['docker','exec','-i',a.container,'python3','/web-tests/session.py','--exec-agent'])
 try:
  stale=b.snapshot();old=find_operation(stale,dict(label='Replacement action',kind='action',name='press'))
  current=actor.snapshot();replace=find_operation(current,dict(label='Replace target identity',kind='action',name='press'));actor.execute(current,replace)
  rejected('replaced-exact-target','execute',observation_id=stale['observation_id'],operation_id=old['id'])
  stale=b.snapshot();old=find_operation(stale,dict(label='Increase counter',kind='action',name='press'))
  current=actor.snapshot();opened=find_operation(current,dict(label='Open confirmation dialog',kind='action',name='press'));actor.execute(current,opened)
  rejected('old-scope-after-modal','execute',observation_id=stale['observation_id'],operation_id=old['id'])
  modal=b.snapshot();ids={n['id'] for n in modal['nodes'] if n['label']=='Increase counter'}
  results.append(dict(test='modal-operation-confinement',passed=not any(o['target'] in ids for o in modal['operations'])))
  current=actor.snapshot();cancel=find_operation(current,dict(label='Cancel confirmation',kind='action',name='press'));actor.execute(current,cancel)
 finally:actor.close()
finally:b.close()
a.output.parent.mkdir(parents=True,exist_ok=True);a.output.write_text(json.dumps(results,indent=2));print(json.dumps(results,indent=2));raise SystemExit(0 if all(r['passed'] for r in results) else 1)
