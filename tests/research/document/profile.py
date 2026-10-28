"""Complete-mode AX profile/paired timing, with full material and read-only oracle.

Development performance only. Does not make model requests or select task done.
"""
import argparse, collections, hashlib, json, os, select, subprocess, time, traceback
from pathlib import Path
import pyatspi

parser=argparse.ArgumentParser()
parser.add_argument('--config',type=Path,required=True)
parser.add_argument('--output',type=Path,required=True)
parser.add_argument('--baseline',type=Path,required=True)
parser.add_argument('--candidate',type=Path)
parser.add_argument('--candidate-concurrency',type=int,default=4)
parser.add_argument('--check-bounds',action='store_true')
args=parser.parse_args();config=json.loads(args.config.read_text());out=args.output;out.mkdir(exist_ok=False)
manifest={'protocol':config,'driver_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
          'binary_sha256':{label:hashlib.sha256(path.read_bytes()).hexdigest() for label,path in [('A',args.baseline),('B',args.candidate)] if path},
          'candidate_concurrency':args.candidate_concurrency if args.candidate else None,'started_at':time.time()}
(out/'manifest.json').write_text(json.dumps(manifest,indent=2))
(out/'driver.py').write_bytes(Path(__file__).read_bytes())
samples=[];summaries=[];ticks=os.sysconf('SC_CLK_TCK')

def cpu_memory(pid):
    stat=Path(f'/proc/{pid}/stat').read_text().split(') ',1)[1].split()
    status=dict(line.split(':',1) for line in Path(f'/proc/{pid}/status').read_text().splitlines() if ':' in line)
    return {'cpu_seconds':(int(stat[11])+int(stat[12]))/ticks,'rss_kib':int(status['VmRSS'].split()[0]),'peak_rss_kib':int(status['VmHWM'].split()[0])}

def wait_app(pid):
    for _ in range(100):
        for app in pyatspi.Registry.getDesktop(0):
            try:
                if app.get_process_id()==pid:return app
            except Exception:pass
        time.sleep(.1)
    raise RuntimeError(f'AX application not exposed for pid {pid}')

def exposed(app):
    texts=[];names=[];stack=[app];seen=set()
    while stack:
        node=stack.pop()
        try:
            path=str(node.path)
            if path in seen:continue
            seen.add(path)
            if node.getRole()==pyatspi.ROLE_PASSWORD_TEXT:continue
            names.append(node.name)
            try:texts.append(node.queryText().getText(0,-1))
            except NotImplementedError:pass
            stack.extend(list(node))
        except Exception:continue
    return texts,names

def wait_name(app,name):
    for _ in range(100):
        if name in exposed(app)[1]:return
        time.sleep(.05)
    raise RuntimeError(f'AX initial state not realized: {name}')

def facts(doc):
    # Keep every semantic/presentation/source field and its order. Only protocol
    # observation identities, acquisition interval and timing differ by design.
    return {key:value for key,value in doc.items() if key not in ['session','observation_id','collection','timings']}

def digest(value):return hashlib.sha256(json.dumps(value,ensure_ascii=False,sort_keys=True,separators=(',',':')).encode()).hexdigest()

class Bridge:
    def __init__(self,app,label,binary,extra=()):
        self.trace=(out/f'{app.get_process_id()}-{label}-trace.jsonl').open('x')
        self.stderr=(out/f'{app.get_process_id()}-{label}-stderr.log').open('x')
        self.p=subprocess.Popen([str(binary),'--session','desktop','--app',app.name,*extra],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=self.stderr,text=True,bufsize=1)
    def request(self,command,**fields):
        request={'command':command,**fields};self.trace.write(json.dumps({'request':request})+'\n');self.trace.flush()
        before=cpu_memory(self.p.pid);start=time.perf_counter()
        self.p.stdin.write(json.dumps(request)+'\n');self.p.stdin.flush()
        ready,_,_=select.select([self.p.stdout],[],[],120)
        if not ready:raise TimeoutError('G2T complete observation timed out')
        line=self.p.stdout.readline();elapsed=(time.perf_counter()-start)*1000
        decode=time.perf_counter();response=json.loads(line);decode_ms=(time.perf_counter()-decode)*1000
        after=cpu_memory(self.p.pid)
        timing={'round_trip_ms':elapsed,'client_decode_ms':decode_ms,'response_bytes':len(line.encode()),
                'g2t_cpu_ms':(after['cpu_seconds']-before['cpu_seconds'])*1000,'rss_kib':after['rss_kib'],'peak_rss_kib':after['peak_rss_kib']}
        write=time.perf_counter();self.trace.write(json.dumps({'response':response,'timing':timing})+'\n');self.trace.flush()
        timing['evidence_write_ms']=(time.perf_counter()-write)*1000
        return response,timing
    def observe(self):
        response,timing=self.request('observe_document')
        if not response.get('ok'):raise RuntimeError(response)
        return response['observation'],timing
    def execute(self,doc,op,**fields):
        response,_=self.request('execute',observation_id=doc['observation_id'],operation_id=op['id'],**fields)
        if not response.get('ok'):raise RuntimeError(response)
        return response
    def close(self):
        try:self.p.stdin.write('{"command":"quit"}\n');self.p.stdin.flush();self.p.wait(timeout=20)
        except Exception:self.p.terminate()
        self.trace.close();self.stderr.close()

def op_named(doc,name):
    nodes={n['id']:n for n in doc['nodes']}
    matches=[o for o in doc['operations'] if o['kind']=='action' and nodes[o['target']]['label']==name]
    assert len(matches)==1,(name,matches)
    return matches[0]

def run_scene(scene):
    if scene=='fixture':cmd=['python3','/workspace/tests/research/document/fixture.py']
    elif scene=='mousepad':cmd=['mousepad','--disable-server']
    else:
        directory=out/'initial-files';directory.mkdir()
        for name in ['Report A.txt','Report B.txt','Reference.txt']:(directory/name).write_text('Initial fixture before GUI launch.\n')
        cmd=['thunar',str(directory)]
    proc=subprocess.Popen(cmd,stdout=subprocess.DEVNULL,stderr=(out/f'{scene}-gui.log').open('x'));app=wait_app(proc.pid)
    setup=Bridge(app,'setup',args.baseline);bridges={}
    try:
        doc,_=setup.observe()
        if scene=='mousepad':
            edit=next(o for o in doc['operations'] if o['kind']=='replace_text' and o.get('multiline'))
            setup.execute(doc,edit,parameter=config['text'])
            assert config['text'] in exposed(app)[0]
        if scene=='thunar':
            setup.execute(doc,op_named(doc,'List View'));wait_name(app,'Report A.txt')
        setup.close();setup=None
        oracle=collections.Counter(exposed(app)[0]);(out/f'{scene}-text-oracle.json').write_text(json.dumps(list(oracle.elements()),indent=2))
        bridges['A']=Bridge(app,'A',args.baseline)
        if args.candidate:bridges['B']=Bridge(app,'B',args.candidate,['--ax-read-concurrency',str(args.candidate_concurrency)])
        warmups=config['paired_warmups_per_arm'] if args.candidate else config['profile_warmups_per_scene']
        reference=None
        def observe(label,phase,index):
            nonlocal reference
            gui_before=cpu_memory(proc.pid);doc,timing=bridges[label].observe();gui_after=cpu_memory(proc.pid)
            actual=collections.Counter(p['read']['text'] for p in doc['text_payloads'] if p['read']['status']=='complete')
            material=facts(doc);fingerprint=digest(material)
            if reference is None:reference=material
            equivalent=material==reference
            row={'scene':scene,'arm':label,'phase':phase,'index':index,**timing,'gui_cpu_ms':(gui_after['cpu_seconds']-gui_before['cpu_seconds'])*1000,
                 'timings':doc['timings'],'fact_sha256':fingerprint,'equivalent':equivalent,'text_oracle_equal':actual==oracle,
                 'nodes':len(doc['nodes']),'text_sources':len(doc['text_payloads']),'text_bytes':sum(len(t.encode())*n for t,n in actual.items()),
                 'operations':len(doc['operations']),'completeness':doc['completeness']}
            samples.append(row);(out/'samples.json').write_text(json.dumps(samples,indent=2))
            if not equivalent:
                (out/f'{scene}-{phase}-{index}-{label}-facts.json').write_text(json.dumps(material,indent=2))
                (out/f'{scene}-reference-facts.json').write_text(json.dumps(reference,indent=2))
            assert equivalent,'Full document changed in a declared static scene'
            assert actual==oracle,'Independent public Text inventory differs'
            assert 'G2T-P2-SECRET-' not in json.dumps(doc)
        for index in range(warmups):
            for label in bridges:observe(label,'warmup',index)
        orders=config['pair_order'] if args.candidate else ['A']*config['profile_samples_per_scene']
        for index,order in enumerate(orders):
            for label in order:observe(label,'measured',index)
        summaries.append({'scene':scene,'passed':True,'measured_samples':len(orders)*len(bridges),'oracle_sources':sum(oracle.values())})
    finally:
        if setup:setup.close()
        for bridge in bridges.values():bridge.close()
        proc.terminate();proc.wait(timeout=10)

def check_bounds():
    assert args.candidate,'Bounds comparison needs the candidate binary'
    proc=subprocess.Popen(['python3','/workspace/tests/research/document/fixture.py'],stdout=subprocess.DEVNULL,stderr=(out/'fixture-gui.log').open('x'))
    app=wait_app(proc.pid)
    try:
        oracle=collections.Counter(exposed(app)[0])
        for case,flags in [('node-budget',['--max-nodes','12']),('depth-budget',['--max-depth','3'])]:
            bridges=[];docs=[]
            try:
                for label,binary,extra in [('A',args.baseline,[]),('B',args.candidate,['--ax-read-concurrency',str(args.candidate_concurrency)])]:
                    bridge=Bridge(app,case+label,binary,extra+flags);bridges.append(bridge)
                    doc,timing=bridge.observe();docs.append(doc)
                    actual=collections.Counter(p['read']['text'] for p in doc['text_payloads'] if p['read']['status']=='complete')
                    assert not (actual-oracle),'A partial observation contains text absent from independent AX'
                    assert doc['completeness']['observed_coverage']=='partial'
                    assert 'G2T-P2-SECRET-' not in json.dumps(doc)
                    samples.append({'case':case,'arm':label,'timings':doc['timings'],**timing})
                assert facts(docs[0])==facts(docs[1]),'Read concurrency changed the declared traversal prefix or references'
                if case=='node-budget':assert len(docs[1]['nodes'])<=12
                summaries.append({'case':case,'passed':True,'nodes':len(docs[1]['nodes'])})
            finally:
                for bridge in bridges:bridge.close()
    finally:proc.terminate();proc.wait(timeout=10)
    (out/'samples.json').write_text(json.dumps(samples,indent=2))

for scene in (['bounds'] if args.check_bounds else ['fixture','mousepad','thunar']):
    try:check_bounds() if args.check_bounds else run_scene(scene)
    except Exception as error:summaries.append({'scene':scene,'passed':False,'error':repr(error),'traceback':traceback.format_exc()})
    (out/'results.json').write_text(json.dumps(summaries,indent=2))
print(json.dumps(summaries,indent=2))
raise SystemExit(0 if all(row['passed'] for row in summaries) else 1)
