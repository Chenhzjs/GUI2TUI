#!/usr/bin/env python3
"""Film actual GUI / live GUI2TUI / Jev decision output, with independent checks."""
import argparse,json,shutil,sys,textwrap,time,uuid
from pathlib import Path
from run import HERE,ROOT,docker,oracle,events,asserted

def main():
 p=argparse.ArgumentParser();p.add_argument('--output',type=Path,required=True);p.add_argument('--image',default='gui2tui-recording:local');p.add_argument('--jev-root',type=Path,default=Path.home()/'Document/Jev-Anything');a=p.parse_args()
 sys.path.insert(0,str(a.jev_root/'src'))
 from jev_anything.native import NativeBridge,candidates_for,choose
 from jev_anything.jev import JevClient
 a.output.mkdir(mode=0o700,parents=True,exist_ok=False)
 for name in ['gui2tui','gui2tui-agent']:shutil.copy2(ROOT/'target/autonomous-linux/debug'/name,a.output/name)
 right=a.output/'jev-live.ansi';right.touch();timeline=[];container='g2t-film-'+uuid.uuid4().hex[:8];bridge=None;recording=False
 def screen(phase,goal='',**details):
  lines=['JEV / miyang/jev-1.13','','PHASE: '+phase,'']
  if goal:lines+=['USER GOAL:',*textwrap.wrap(goal,72),'']
  for key,value in details.items():lines += [key+':',*textwrap.wrap(str(value),72),'']
  lines+=['observe -> choose -> validate','-> execute -> authoritative readback','','Model output is a candidate ID.','No DOM, coordinates, or generated commands.']
  with right.open('a') as stream:stream.write('\x1b[2J\x1b[H\x1b[36m'+'\n'.join(lines)+'\x1b[0m\n');stream.flush()
  timeline.append({'time':time.time(),'phase':phase,'goal':goal,**details})
 try:
  docker('run','-d','--name',container,'--platform','linux/amd64','--shm-size=512m','-v',f'{(a.output/"gui2tui-agent").resolve()}:/gui2tui-agent:ro','-v',f'{(a.output/"gui2tui").resolve()}:/gui2tui:ro','-v',f'{HERE}:/web-tests:ro','-v',f'{a.output.resolve()}:/evidence','--entrypoint','dbus-run-session',a.image,'--','python3','/web-tests/session.py','--url','http://127.0.0.1:8765/demo','--screen','1920x1080x24')
  for _ in range(150):
   if docker('exec',container,'test','-f','/tmp/web-ready',check=False).returncode==0:break
   time.sleep(.2)
  screen('INTRO',purpose='Real GUI left; actual GUI2TUI PTY center; live Jev decisions right.')
  docker('exec','-d',container,'python3','/web-tests/record_inside.py')
  for _ in range(200):
   if (a.output/'capture-start.json').exists():break
   time.sleep(.2)
  else:raise RuntimeError('Recording startup failed')
  recording=True;time.sleep(4)
  bridge=NativeBridge(['docker','exec','-i',container,'python3','/web-tests/session.py','--exec-agent']);provider=JevClient()
  all_tasks=json.loads((HERE/'deep_tasks.json').read_text());ids=['name-ascii','email','notifications-on','counter-increase','volume-set']
  tasks=[next(t for t in all_tasks if t['id']==id) for id in ids]
  tasks.insert(2,dict(id='demo-save',goal='Press Save profile to save Ada Lovelace with ada@example.test.',plans=[dict(parameter=None)],expect_event={'name':'profile','value':{'name':'Ada Lovelace','email':'ada@example.test'}}))
  tasks.append(dict(id='demo-clear',goal='Clear all text from Full name. Leave it empty.',plans=[dict(parameter='')],expect_node={'name':'Full name','value':''}))
  results=[]
  with (a.output/'decisions.jsonl').open('x') as trace:
   for index,task in enumerate(tasks,1):
    before=events(container);screen('OBSERVING',task['goal'],step=f'{index}/{len(tasks)}')
    observation=bridge.snapshot();trace.write(json.dumps({'kind':'observe','observation':observation},ensure_ascii=False)+'\n');trace.flush()
    texts=[plan['parameter'] for plan in task['plans'] if isinstance(plan.get('parameter'),str)];values=[plan['parameter'] for plan in task['plans'] if isinstance(plan.get('parameter'),(int,float))]
    candidates=candidates_for(observation,texts,values)
    screen('JEV IS DECIDING',task['goal'],observation_id=observation['observation_id'],scope=observation['interaction_scope'],legal_candidates=len(candidates),text_candidates=texts)
    started=time.monotonic();answer=choose(provider,task['goal'],observation,candidates,[]);latency=round((time.monotonic()-started)*1000)
    trace.write(json.dumps({'kind':'decision','answer':answer,'latency_ms':latency},ensure_ascii=False)+'\n');trace.flush()
    if answer['choice'] in ('done','abstain','wait'):raise RuntimeError('Demo decision did not choose an operation: '+answer['choice'])
    candidate=next(c for c in candidates if c['id']==answer['choice'])
    screen('SELECTED / VALIDATING',task['goal'],choice=answer['choice'],confidence=answer.get('confidence'),operation=candidate['label'],parameter=candidate.get('parameter'),jev_latency_ms=latency);time.sleep(2)
    receipt=bridge.execute(observation,candidate);trace.write(json.dumps({'kind':'receipt','candidate':candidate,'receipt':receipt},ensure_ascii=False)+'\n');trace.flush()
    screen('READING BACK',task['goal'],operation=candidate['label'],accepted=receipt['accepted'],primitive_effect=receipt['effect']);after=bridge.snapshot()
    until=time.monotonic()+3
    while True:
     observed=oracle(container);actual=events(container)[len(before):];passed=asserted(task,actual,observed)
     if passed or time.monotonic()>=until:break
     time.sleep(.2)
    row={'task':task['id'],'passed':passed,'receipt':receipt,'events':actual};results.append(row);trace.write(json.dumps({'kind':'readback','observation':after,'assertion':row},ensure_ascii=False)+'\n');trace.flush()
    screen('VERIFIED' if passed else 'NOT CONFIRMED',task['goal'],operation=candidate['label'],backend_acceptance=receipt['accepted'],independent_task_assertion=passed,verified_steps=sum(r['passed'] for r in results));time.sleep(4)
    if not passed:raise RuntimeError('Independent demo assertion failed: '+task['id'])
  screen('COMPLETE',result=f'{len(results)}/{len(results)} independent task assertions passed.',boundary='Jev chooses. GUI2TUI enforces authority. Public Accessibility is the source of truth.');time.sleep(5)
  (a.output/'result.json').write_text(json.dumps(results,indent=2));return 0
 finally:
  if bridge:bridge.close()
  if recording:
   (a.output/'stop').touch()
   for _ in range(200):
    if (a.output/'capture-done').exists():break
    time.sleep(.2)
  (a.output/'timeline.json').write_text(json.dumps(timeline,ensure_ascii=False,indent=2))
  docker('rm','-f',container,check=False)
if __name__=='__main__':raise SystemExit(main())
