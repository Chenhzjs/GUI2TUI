"""Small live AX regression; no provider, input injection or application adapters.

Run inside an isolated dbus/X11 session with the current gui2tui binaries.
Every request/response and independent read-only AX oracle is retained.
"""
import collections
import hashlib
import json
import os
from pathlib import Path
import select
import subprocess
import sys
import time
import traceback
import pyatspi
import pexpect
import pyte

OUT = Path(sys.argv[1])
OUT.mkdir(exist_ok=False)
BINARY = Path(os.environ["GUI2TUI_AGENT"])
VERSION = "gui2tui.agent.document.v1"
LONG_TEXT = "\n".join(f"GUI2TUI real editor public line {i}" for i in range(240)) + "\nREAL-EDITOR-TAIL-8173"
manifest = {"kind": "P2 development regression, not research/model tasks", "provider": None,
            "apps": ["fixture.py", "mousepad", "thunar"], "parameters": {"editor_text": LONG_TEXT},
            "oracles": ["independent public AX text read", "exact observation consumption/rejection", "explicit coverage states"],
            "binary_sha256": hashlib.sha256(BINARY.read_bytes()).hexdigest(), "automatic_unknown_replay": False}
for source in (Path(__file__), Path(__file__).with_name("fixture.py")):
    manifest[source.name + "_sha256"] = hashlib.sha256(source.read_bytes()).hexdigest()
    (OUT / ("source-" + source.name)).write_bytes(source.read_bytes())
(OUT / "task-manifest.json").write_text(json.dumps(manifest, indent=2))
results = []

def desktop_apps():
    return list(pyatspi.Registry.getDesktop(0))

def wait_app(pid):
    for _ in range(100):
        for app in desktop_apps():
            try:
                if app.get_process_id() == pid:
                    return app
            except Exception:
                pass
        time.sleep(.1)
    raise RuntimeError(f"No public AX application for pid {pid}")

def ax_texts(app):
    texts = []
    stack = [app]
    seen = set()
    while stack:
        node = stack.pop()
        try:
            identity = str(node.path)
            if identity in seen:
                continue
            seen.add(identity)
            if node.getRole() == pyatspi.ROLE_PASSWORD_TEXT:
                continue
            try:
                text = node.queryText().getText(0, -1)
                texts.append({"path": identity, "role": node.getRoleName(), "name": node.name, "text": text})
            except NotImplementedError:
                pass
            stack.extend(list(node))
        except Exception:
            continue
    return texts

def named_paths(app, name):
    stack=[app]; paths=[]
    while stack:
        node=stack.pop()
        try:
            if node.name==name: paths.append(str(node.path))
            stack.extend(list(node))
        except Exception: pass
    return sorted(paths)

def wait_public_name(app, name):
    for _ in range(100):
        if named_paths(app, name): return
        time.sleep(.05)
    raise RuntimeError(f"Public AX name not realized: {name}")

class Bridge:
    def __init__(self, app, label):
        self.trace = (OUT / f"{label}-trace.jsonl").open("x")
        self.stderr = (OUT / f"{label}-stderr.log").open("x")
        self.p = subprocess.Popen([str(BINARY), "--session", "desktop", "--app", app.name],
                                  stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=self.stderr, text=True, bufsize=1)
    def request(self, command, **fields):
        request = {"command": command, **fields}
        self.trace.write(json.dumps({"request": request}) + "\n"); self.trace.flush()
        started = time.monotonic()
        self.p.stdin.write(json.dumps(request) + "\n"); self.p.stdin.flush()
        ready, _, _ = select.select([self.p.stdout], [], [], 120)
        if not ready:
            raise TimeoutError("G2T response timeout")
        line = self.p.stdout.readline()
        response = json.loads(line)
        self.trace.write(json.dumps({"response": response, "round_trip_ms": (time.monotonic()-started)*1000}) + "\n"); self.trace.flush()
        return response
    def observe(self):
        response = self.request("observe_document")
        assert response["ok"], response
        assert response["observation"]["version"] == VERSION
        return response["observation"]
    def execute(self, observation, operation, **fields):
        return self.request("execute", observation_id=observation["observation_id"], operation_id=operation["id"], **fields)
    def close(self):
        try:
            self.p.stdin.write('{"command":"quit"}\n'); self.p.stdin.flush(); self.p.wait(timeout=20)
        except Exception:
            self.p.terminate()
        self.trace.close(); self.stderr.close()

def operation(obs, label=None, kind=None):
    nodes = {n["id"]: n for n in obs["nodes"]}
    matches = [op for op in obs["operations"] if (label is None or nodes[op["target"]]["label"] == label)
               and (kind is None or op["kind"] == kind)]
    assert len(matches) == 1, (label, kind, matches)
    return matches[0]

def verify_inventory(obs, app, label):
    oracle = ax_texts(app)
    (OUT / f"{label}-ax-oracle.json").write_text(json.dumps(oracle, indent=2))
    expected = collections.Counter(row["text"] for row in oracle)
    actual = collections.Counter(row["read"]["text"] for row in obs["text_payloads"] if row["read"]["status"] == "complete")
    assert actual == expected, {"only_g2t": list((actual-expected).elements()), "only_oracle": list((expected-actual).elements())}
    assert obs["timings"]["human_scene_layout_performed"] is False
    assert obs["timings"]["ax_walk_read_groups"]["metadata_requests"] > 0
    return {"nodes":len(obs["nodes"]), "text_sources":sum(actual.values()), "text_bytes":sum(len(t.encode())*n for t,n in actual.items()),
            "operations":len(obs["operations"]), "completeness":obs["completeness"], "timings":obs["timings"]}

def reject(response):
    assert response["ok"] is False and response["delivery"] == "not_attempted", response

def human_surface(app, bridge, label):
    """Exercise terminal presentation only; no keys are injected into the GUI."""
    before=bridge.observe()
    log=(OUT/f"{label}-human.ansi").open("x")
    tui=pexpect.spawn(str(BINARY.with_name("gui2tui")),["--session","desktop","--app",app.name],
                      env={**os.environ,"TERM":"xterm-256color"},encoding="utf-8",dimensions=(30,110),timeout=30)
    tui.logfile_read=log
    captured=[]
    def drain(seconds=1):
        end=time.monotonic()+seconds; data=""
        while time.monotonic()<end:
            try: data+=tui.read_nonblocking(65536,timeout=.1)
            except pexpect.TIMEOUT: pass
            except pexpect.EOF: break
        captured.append(data);return data
    try:
        tui.expect("Shared document validation" if label=="fixture" else "Mousepad")
        captured.append(tui.before+tui.after);drain()
        tui.send(":");tui.expect("Command palette|Commands");captured.append(tui.before+tui.after)
        tui.send("\x1b");drain(.5)
        tui.send("\x1bOR");tui.expect("Public actions");captured.append(tui.before+tui.after)
        tui.send("\x1b");drain(.5)
        tui.setwinsize(12,40);drain()
        tui.setwinsize(30,110);drain(.5)
        tui.send("o");drain()
        screen=pyte.Screen(110,30);stream=pyte.Stream(screen)
        stream.feed("".join(captured))
        (OUT/f"{label}-human.txt").write_text("\n".join(screen.display))
        assert "G2T-P2-SECRET-" not in "".join(captured)
        after=bridge.observe()
        for key in ["nodes","relations","text_payloads","value_domains","content","regions","commands","choices","scopes","operations","sources"]:
            assert before[key]==after[key],f"Terminal-only changes altered {key}"
        reader_opened="Reader" in "\n".join(screen.display) or "Outline" in "\n".join(screen.display)
        assert reader_opened,"Explicit Document Reader/Outline did not open"
        tui.send("\x1b");drain(.3)
        if tui.isalive(): tui.send("q")
        try:tui.expect(pexpect.EOF,timeout=10)
        except pexpect.TIMEOUT:tui.terminate(force=True)
        return {"surface":True,"command_palette":True,"public_actions":True,"resize_preserves_complete_document":True,
                "reader_or_outline_opened":reader_opened}
    finally:
        if tui.isalive():tui.terminate(force=True)
        log.close()

def controlled_fixture():
    proc = subprocess.Popen(["python3", "/workspace/tests/research/document/fixture.py"], stdout=subprocess.DEVNULL, stderr=(OUT / "fixture-gui.log").open("x"))
    app = wait_app(proc.pid); b = Bridge(app, "fixture"); actor = None
    try:
        obs=b.observe(); summary=verify_inventory(obs, app, "fixture")
        assert "G2T-P2-SECRET-" not in json.dumps(obs)
        assert len(obs["content"]) >= 2
        assert any("Report A TAIL" in row["read"].get("text","") for row in obs["text_payloads"])
        assert any("Report B TAIL" in row["read"].get("text","") for row in obs["text_payloads"])
        readonly={n["id"] for n in obs["nodes"] if n["label"]=="Read only reference"}
        assert not any(op["target"] in readonly and op["kind"]=="replace_text" for op in obs["operations"])
        amount=operation(obs,"Public amount","set_value")
        assert amount["parameter_domain"]["step"]==5
        summary["human"]=human_surface(app,b,"fixture")
        obs=b.observe();amount=operation(obs,"Public amount","set_value")
        unsupported=next(op for op in obs["operations"] if op["agent_invocable"] is False)
        reject(b.execute(obs,unsupported))
        reject(b.execute(obs,amount,parameter=5))
        obs=b.observe(); amount=operation(obs,"Public amount","set_value")
        reject(b.execute(obs,amount,parameter="not numeric"))
        obs=b.observe(); old=operation(obs,"Replacement action","action")
        actor=Bridge(app,"fixture-actor"); current=actor.observe()
        before=named_paths(app,"Replacement action")
        actor.execute(current,operation(current,"Replace target identity","action"))
        after=named_paths(app,"Replacement action")
        (OUT/"replacement-identity-oracle.json").write_text(json.dumps({"before":before,"after":after},indent=2))
        assert len(before)==len(after)==1 and before!=after, (before,after)
        reject(b.execute(obs,old))
        obs=b.observe(); old=operation(obs,"Replacement action","action")
        current=actor.observe(); actor.execute(current,operation(current,"Open confirmation","action"))
        wait_public_name(app,"Cancel confirmation")
        (OUT/"modal-public-oracle.json").write_text(json.dumps([
            {"path":str(node.path),"role":node.getRoleName(),"name":node.name,
             "states":[str(state) for state in node.getState().getStates()]}
            for node in app],indent=2))
        actor.observe()
        reject(b.execute(obs,old))
        modal=b.observe()
        assert not any(n["label"]=="Replacement action" for n in modal["nodes"])
        b.execute(modal,operation(modal,"Cancel confirmation","action"))
        results.append({"scenario":"controlled full content/privacy/value/stale/modal", "passed":True, **summary})
    finally:
        b.close()
        if actor: actor.close()
        proc.terminate(); proc.wait(timeout=10)

def editor():
    proc=subprocess.Popen(["mousepad","--disable-server"], stdout=subprocess.DEVNULL, stderr=(OUT/"mousepad-gui.log").open("x"))
    app=wait_app(proc.pid);b=Bridge(app,"mousepad")
    try:
        old=b.request("observe")["observation"]
        doc=b.observe()
        reject(b.request("execute",observation_id=old["observation_id"],operation_id=old["operations"][0]["id"]))
        doc=b.observe();edit=next(op for op in doc["operations"] if op["kind"]=="replace_text" and op.get("multiline"))
        reject(b.execute(doc,edit))
        reject(b.execute(doc,edit,parameter=LONG_TEXT))
        doc=b.observe();edit=next(op for op in doc["operations"] if op["kind"]=="replace_text" and op.get("multiline"))
        receipt=b.execute(doc,edit,parameter=LONG_TEXT)
        fresh=b.observe();summary=verify_inventory(fresh,app,"mousepad")
        assert any(row["text"]==LONG_TEXT for row in ax_texts(app))
        assert any(row["read"].get("text")==LONG_TEXT for row in fresh["text_payloads"])
        results.append({"scenario":"real Mousepad long text and v1/document consumption", "passed":True,
                        "receipt":receipt, "independent_text_oracle":True, **summary})
    finally:
        b.close();proc.terminate();proc.wait(timeout=10)

def files():
    initial=OUT/"initial-files";initial.mkdir()
    for name in ("Report A.txt","Report B.txt","Reference.txt"):
        (initial/name).write_text("Initial regression fixture, before GUI launch.\n")
    proc=subprocess.Popen(["thunar",str(initial)], stdout=subprocess.DEVNULL, stderr=(OUT/"thunar-gui.log").open("x"))
    app=wait_app(proc.pid);b=Bridge(app,"thunar")
    try:
        obs=b.observe()
        initial_coverage=obs["completeness"]
        if not all(name in json.dumps(obs) for name in ("Report A.txt","Report B.txt")):
            # Change the real public view using its advertised action. No file
            # mutation or input guessing, and no replay on an unknown effect.
            receipt=b.execute(obs,operation(obs,"List View","action"))
            wait_public_name(app,"Report A.txt")
            obs=b.observe()
        else: receipt=None
        summary=verify_inventory(obs,app,"thunar")
        assert any(n["role"] in ("Table","List","Tree") for n in obs["nodes"])
        assert "Report A.txt" in json.dumps(obs) and "Report B.txt" in json.dumps(obs)
        results.append({"scenario":"real Thunar structured file content", "passed":True,
                        "initial_coverage":initial_coverage,"public_view_change_receipt":receipt, **summary})
    finally:
        b.close();proc.terminate();proc.wait(timeout=10)

for test in (controlled_fixture,editor,files):
    try: test()
    except Exception as error:
        results.append({"scenario":test.__name__,"passed":False,"error":repr(error),"traceback":traceback.format_exc()})
    (OUT/"results.json").write_text(json.dumps(results,indent=2))
print(json.dumps(results,indent=2))
raise SystemExit(0 if all(r["passed"] for r in results) else 1)
