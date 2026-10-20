#!/usr/bin/env python3
"""Record a public website using the unchanged production Jev native loop.
The read-only AT-SPI oracle checks completion; it never drives GUI operations.
"""
import argparse,json,shutil,sys,textwrap,time,uuid
from pathlib import Path
from run import HERE,ROOT,docker,oracle


def main():
 p=argparse.ArgumentParser();p.add_argument('--output',type=Path,required=True);p.add_argument('--task',default='wikipedia-search');p.add_argument('--goal');p.add_argument('--image',default='gui2tui-recording:local');p.add_argument('--focus-label',default='Search Wikipedia');p.add_argument('--jev-root',type=Path,default=Path.home()/'Document/Jev-Anything');a=p.parse_args()
 sys.path.insert(0,str(a.jev_root/'src'))
 from jev_anything.native import NativeBridge,candidates_for,run
 from jev_anything.jev import JevClient
 from public import check
 task=next(t for t in json.loads((HERE/'public_tasks.json').read_text()) if t['id']==a.task)
 if a.goal:task['goal']=a.goal
 a.output=a.output.resolve();a.output.mkdir(mode=0o700,parents=True,exist_ok=False)
 for name in ['gui2tui','gui2tui-agent']:shutil.copy2(ROOT/'target/autonomous-linux/debug'/name,a.output/name)
 (a.output/'task.json').write_text(json.dumps(task,ensure_ascii=False,indent=2))
 right=a.output/'jev-live.ansi';right.touch();timeline=[];container='g2t-public-film-'+uuid.uuid4().hex[:8];bridge=None;recording=False
 def screen(phase,**details):
  lines=['JEV / miyang/jev-1.13','','PHASE: '+phase,'','PUBLIC WEBSITE:',task['url'],'','USER GOAL:',*textwrap.wrap(task['goal'],72),'']
  for key,value in details.items():lines += [key+':',*textwrap.wrap(str(value),72),'']
  lines+=['observe -> choose -> validate','-> execute -> public readback','','Model chooses among public operation IDs.','Independent assertion checks task completion.']
  with right.open('a') as stream:stream.write('\x1b[2J\x1b[H\x1b[36m'+'\n'.join(lines)+'\x1b[0m\n');stream.flush()
  timeline.append({'time':time.time(),'phase':phase,'goal':task['goal'],**details})
 try:
  docker('run','-d','--name',container,'--platform','linux/amd64','--shm-size=512m','-v',f'{a.output/"gui2tui-agent"}:/gui2tui-agent:ro','-v',f'{a.output/"gui2tui"}:/gui2tui:ro','-v',f'{HERE}:/web-tests:ro','-v',f'{a.output}:/evidence','--entrypoint','dbus-run-session',a.image,'--','python3','/web-tests/session.py','--url',task['url'],'--screen','1920x1080x24')
  for _ in range(150):
   if docker('exec',container,'test','-f','/tmp/web-ready',check=False).returncode==0:break
   time.sleep(.2)
  # Public read-only observation verifies initial loading before recording.
  initial=oracle(container);(a.output/'initial-oracle.json').write_text(json.dumps(initial,ensure_ascii=False,indent=2))
  screen('INTRO',purpose='Live public website; actual GUI2TUI PTY; production Jev native loop.')
  docker('exec','-d',container,'sh','-c','exec python3 /web-tests/record_inside.py --focus-label "$1" --scroll-steps 0 > /evidence/recorder.log 2>&1','recorder',a.focus_label)
  for _ in range(250):
   if (a.output/'capture-start.json').exists():break
   time.sleep(.2)
  else:raise RuntimeError('Recording startup failed')
  recording=True;time.sleep(4)
  bridge=NativeBridge(['docker','exec','-i',container,'python3','/web-tests/session.py','--exec-agent']);current=[]
  with (a.output/'decisions.jsonl').open('x') as trace:
   def emit(event):
    nonlocal current
    trace.write(json.dumps(event,ensure_ascii=False)+'\n');trace.flush()
    if event['kind']=='observe':
     observation=event['observation'];current=candidates_for(observation,task.get('texts',[]),task.get('values',[]))
     screen('JEV IS DECIDING',decision_step=event['step']+1,observation_id=observation['observation_id'],scope=observation['interaction_scope'],legal_candidates=len(current),input_values=task.get('texts',[]))
    elif event['kind']=='decision':
     answer=event['answer'];choice=answer['choice'];selected=next((c for c in current if c['id']==choice),None)
     phase='WAITING' if choice=='wait' else 'MODEL CLAIMS DONE' if choice=='done' else 'NOT CONFIRMED' if choice=='abstain' else 'SELECTED / VALIDATING'
     screen(phase,choice=choice,confidence=answer.get('confidence'),operation=selected['label'] if selected else choice,parameter=selected.get('parameter') if selected else None,jev_latency_ms=event['latency_ms']);time.sleep(3)
    elif event['kind']=='receipt':
     screen('READING BACK',operation=event['candidate']['label'],backend_acceptance=event['receipt']['accepted'],primitive_effect=event['receipt']['effect']);time.sleep(2)
    elif event['kind']=='not_dispatched':screen('NOT CONFIRMED',reason=event['reason'])
   result=run(task['goal'],bridge,JevClient(),texts=task.get('texts',[]),values=task.get('values',[]),max_steps=8,emit=emit)
   trace.write(json.dumps({'kind':'result','result':result},ensure_ascii=False)+'\n')
  observed=oracle(container);(a.output/'final-oracle.json').write_text(json.dumps(observed,ensure_ascii=False,indent=2));writes=len(result['history'])
  passed=check(task['expect'],observed,writes) and writes>0
  # This task specifically promises the English article, not just a matching title.
  address_confirmed=None
  if task['id']=='wikipedia-search':
   address_confirmed=any('en.wikipedia.org/wiki/Accessibility' in (n.get('text') or '') for n in observed['nodes'] if n['role'] in ('entry','combo box') and n['name']=='Search or enter address')
   passed=passed and address_confirmed
  row={'task':task['id'],'url':task['url'],'passed':passed,'model_status':result['status'],'gui_writes':writes,'expected':task['expect'],'english_article_url_confirmed':address_confirmed,'receipts':[h['receipt'] for h in result['history']]}
  (a.output/'result.json').write_text(json.dumps([row],ensure_ascii=False,indent=2))
  screen('VERIFIED' if passed else 'NOT CONFIRMED',model_status=result['status'],independent_task_assertion=passed,gui_writes=writes,english_article_url_confirmed=address_confirmed);time.sleep(6)
  screen('COMPLETE' if passed else 'NOT CONFIRMED',result=f'{int(passed)}/1 task independently verified.',boundary='Original public website. No site-specific backend, DOM, coordinates, or raw key replay.');time.sleep(5)
  return 0 if passed else 1
 finally:
  if bridge:bridge.close()
  if recording:
   (a.output/'stop').touch()
   for _ in range(200):
    if (a.output/'capture-done').exists():break
    time.sleep(.2)
  (a.output/'timeline.json').write_text(json.dumps(timeline,ensure_ascii=False,indent=2));docker('rm','-f',container,check=False)
if __name__=='__main__':raise SystemExit(main())
