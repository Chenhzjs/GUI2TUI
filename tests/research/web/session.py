"""Linux validation session. GUI writes happen only through /gui2tui-agent."""
import argparse
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import subprocess
import shutil
import threading
import time
from urllib.parse import urlparse

EVENTS=[]
ROOT=Path(__file__).parent
class Handler(BaseHTTPRequestHandler):
    def do_POST(self):
        if self.path!='/events': self.send_error(404); return
        EVENTS.append(json.loads(self.rfile.read(int(self.headers['Content-Length']))))
        self.send_response(204); self.end_headers()
    def do_GET(self):
        route=urlparse(self.path).path
        if route=='/events': body=json.dumps(EVENTS).encode(); kind='application/json'
        elif route=='/embedded': body=b'<title>Embedded panel</title><button onclick="parent.send(\'embedded\',true);this.textContent=\'Embedded action completed\'">Run embedded action</button>';kind='text/html'
        elif route in ('/workbench','/demo'):
            text=(ROOT/'workbench.html').read_text()
            if route=='/demo':
                text=text.replace('</style>', 'section:nth-of-type(n+4){display:none}label:has(#notes),[aria-label="Rich notes"],label:has(input[readonly]),label:has(input[disabled]),label:has(input[type=password]){display:none}section h2{font-size:17px}form label{display:block}body{font-size:15px;margin:12px}h1{font-size:23px}section{padding:6px}label:has(#tags),label:has(#delivery){display:none}</style>')
            body=text.encode();kind='text/html; charset=utf-8'
        else: body=b'<h1>Not found</h1>';kind='text/html'
        self.send_response(200); self.send_header('Content-Type',kind);self.send_header('Content-Length',str(len(body)));self.end_headers();self.wfile.write(body)
    def log_message(self,*_): pass

def oracle():
    import pyatspi
    rows=[]; errors=[]
    queue=[(app,False) for app in pyatspi.Registry.getDesktop(0) if 'firefox' in app.name.lower()]
    while queue:
        node,secret=queue.pop(0)
        try:
            role=node.getRoleName();secret=secret or role=='password text'
            states=node.getState()
            row={'role':role,'name':None if secret else node.name,'path':node.path,
                 'checked':states.contains(pyatspi.STATE_CHECKED),'selected':states.contains(pyatspi.STATE_SELECTED),
                 'enabled':states.contains(pyatspi.STATE_ENABLED),'showing':states.contains(pyatspi.STATE_SHOWING),'secret':secret}
            if not secret:
                try: row['text']=node.queryText().getText(0,-1)
                except NotImplementedError: pass
                try: row['value']=node.queryValue().currentValue
                except NotImplementedError: pass
            rows.append(row)
            queue.extend((node[i],secret) for i in range(node.childCount) if node[i] is not None)
        except Exception as e: errors.append(str(e))
    return {'nodes':rows,'errors':errors}

def main():
    p=argparse.ArgumentParser();p.add_argument('--url',default='http://127.0.0.1:8765/workbench');p.add_argument('--exec-agent',action='store_true');p.add_argument('--oracle',action='store_true');p.add_argument('--max-nodes',default='10000');p.add_argument('--screen',default='1280x1000x24');args=p.parse_args()
    env_file=Path('/tmp/web-session.json')
    if args.exec_agent or args.oracle:
        os.environ.update(json.loads(env_file.read_text()))
        if args.oracle: print(json.dumps(oracle()));return
        os.execv('/gui2tui-agent',['/gui2tui-agent','--session','desktop','--app','Firefox','--max-nodes',args.max_nodes])
    os.environ.update(DISPLAY=':98',GTK_A11Y='atspi',NO_AT_BRIDGE='0',MOZ_ACCESSIBILITY_ATSPI_ENABLED='1',XDG_RUNTIME_DIR='/tmp/web-runtime')
    Path('/tmp/web-runtime').mkdir(mode=0o700,exist_ok=True)
    env_file.write_text(json.dumps({k:os.environ[k] for k in ['DISPLAY','GTK_A11Y','NO_AT_BRIDGE','MOZ_ACCESSIBILITY_ATSPI_ENABLED','XDG_RUNTIME_DIR','DBUS_SESSION_BUS_ADDRESS']}));env_file.chmod(0o600)
    server=ThreadingHTTPServer(('0.0.0.0',8765),Handler);threading.Thread(target=server.serve_forever,daemon=True).start()
    with open('/tmp/xvfb.log','w') as log: display=subprocess.Popen(['Xvfb',':98','-screen','0',args.screen,'-ac'],stdout=log,stderr=log)
    for _ in range(100):
        if Path('/tmp/.X11-unix/X98').exists(): break
        time.sleep(.1)
    if shutil.which('openbox'):
        with open('/tmp/openbox.log','w') as log: subprocess.Popen(['openbox'],stdout=log,stderr=log)
    profile=Path('/tmp/web-profile');profile.mkdir(exist_ok=True)
    (profile/'user.js').write_text('user_pref("accessibility.force_disabled", -1);\nuser_pref("browser.shell.checkDefaultBrowser", false);\nuser_pref("browser.startup.homepage_override.mstone", "ignore");\nuser_pref("browser.aboutwelcome.enabled", false);\nuser_pref("browser.newtabpage.enabled", false);\nuser_pref("datareporting.policy.dataSubmissionPolicyBypassNotification", true);\n')
    with open('/tmp/firefox.log','w') as log: browser=subprocess.Popen(['firefox-esr','--no-remote','--profile',str(profile),args.url],stdout=log,stderr=log)
    import pyatspi
    for _ in range(150):
        if any('firefox' in a.name.lower() for a in pyatspi.Registry.getDesktop(0)):
            Path('/tmp/web-ready').touch();break
        time.sleep(.2)
    try: browser.wait()
    finally: display.terminate();server.shutdown()
if __name__=='__main__':main()
