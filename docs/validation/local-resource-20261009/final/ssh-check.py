import pathlib,tempfile,subprocess,time,json,signal,hashlib
root=pathlib.Path.cwd();out=root/'docs/validation/local-resource-20261009/final';results=[]
def run(args,**kw):return subprocess.run(args,check=True,capture_output=True,text=True,**kw)
with tempfile.TemporaryDirectory(prefix='g2t-ssh-',dir='/tmp') as td:
 p=pathlib.Path(td);key=p/'key';run(['ssh-keygen','-q','-t','ed25519','-N','','-f',str(key)])
 # Dedicated generated host key is pinned; no user SSH configuration is changed.
 host=p/'host';run(['ssh-keygen','-q','-t','ed25519','-N','','-f',str(host)])
 container='gui2tui-resource-ssh-check';broker=None;tunnel=None
 try:
  run(['docker','run','-d','--rm','--name',container,'--platform','linux/amd64','-p','127.0.0.1::22','-v',str(root)+':/work','-v',str(p)+':/keys:ro','-v',str(root/'target/autonomous-linux/debug/gui2tui-local')+':/sender:ro','-v',str(root/'tests/fixtures/modality/sample.pdf')+':/sample.pdf:ro','--entrypoint','sh','gui2tui-resource-ssh:local','-c','mkdir -p /root/.ssh /tmp/handoff && chmod 700 /root/.ssh /tmp/handoff && cp /keys/key.pub /root/.ssh/authorized_keys && chmod 600 /root/.ssh/authorized_keys && exec /usr/sbin/sshd -D -e -h /keys/host -o PasswordAuthentication=no -o PermitRootLogin=prohibit-password'])
  port=run(['docker','port',container,'22']).stdout.strip().rsplit(':',1)[1]
  pub=host.with_suffix('.pub').read_text().split();known=p/'known_hosts';known.write_text('[127.0.0.1]:'+port+' '+pub[0]+' '+pub[1]+chr(10))
  ssh=['ssh','-F','/dev/null','-i',str(key),'-p',port,'-o','BatchMode=yes','-o','StrictHostKeyChecking=yes','-o','UserKnownHostsFile='+str(known),'-o','ConnectTimeout=5']
  for _ in range(50):
   r=subprocess.run(ssh+['root@127.0.0.1','true'],capture_output=True)
   if r.returncode==0:break
   time.sleep(.1)
  sock=p/'broker.sock';log=(out/'ssh-broker.log').open('w')
  broker=subprocess.Popen([str(root/'target/debug/gui2tui'),'endpoint','serve','--socket',str(sock),'--mime','application/pdf','--handler-program','/usr/bin/open','--authorization','once'],stdout=log,stderr=log)
  for _ in range(100):
   if sock.exists():break
   time.sleep(.05)
  tunnel=subprocess.Popen(ssh+['-N','-o','ExitOnForwardFailure=yes','-R','/tmp/handoff/broker.sock:'+str(sock),'root@127.0.0.1'],stdout=subprocess.DEVNULL,stderr=(out/'ssh-tunnel.log').open('w'))
  for _ in range(100):
   r=subprocess.run(ssh+['root@127.0.0.1','test -S /tmp/handoff/broker.sock'],capture_output=True)
   if r.returncode==0:break
   if tunnel.poll() is not None:raise RuntimeError('tunnel exited')
   time.sleep(.05)
  for extra in ['', ' --cancel-before-transfer']:
   r=subprocess.run(ssh+['root@127.0.0.1','/sender send-artifact --socket /tmp/handoff/broker.sock --input /sample.pdf --mime application/pdf --kind document'+extra],timeout=30,capture_output=True,text=True)
   results.append({'case':'cancel' if extra else 'open','returncode':r.returncode,'stdout':r.stdout,'stderr':r.stderr})
  assert results[0]['returncode'] == 0 and 'artifact_bytes: 333' in results[0]['stdout']
  assert results[1]['returncode'] != 0 and 'cancel' in results[1]['stderr']
  r=run(ssh+['root@127.0.0.1','dbus-run-session -- python3 /work/docs/validation/local-resource-20261009/final/f4_remote.py mousepad --scenario line_numbers --binary /work/target/autonomous-linux/debug/gui2tui --layout spatial --output /work/docs/validation/local-resource-20261009/final'],timeout=90)
  d=json.loads((out/'result.json').read_text());assert d['status']=='passed',d.get('error')
  results.append({'case':'remote_f4_to_local_handler','passed':True})
  broker.send_signal(signal.SIGTERM);broker.wait(timeout=10)
  broker=subprocess.Popen([str(root/'target/debug/gui2tui'),'endpoint','serve','--socket',str(sock),'--mime','application/pdf','--recording-handler','--authorization','deny'],stdout=log,stderr=log)
  for _ in range(100):
   if sock.exists():break
   time.sleep(.05)
  r=subprocess.run(ssh+['root@127.0.0.1','/sender send-artifact --socket /tmp/handoff/broker.sock --input /sample.pdf --mime application/pdf --kind document'],timeout=20,capture_output=True,text=True)
  print('DENY', r.returncode, r.stdout, r.stderr, flush=True)
  assert r.returncode != 0 and 'payload_sent=0' in r.stdout
  results.append({'case':'deny','returncode':r.returncode,'stdout':r.stdout,'stderr':r.stderr})
  broker.send_signal(signal.SIGTERM);broker.wait(timeout=10)
  runtime=p/'runtime';runtime.mkdir(mode=0o700)
  import os
  broker=subprocess.Popen([str(root/'target/debug/gui2tui'),'endpoint','serve','--socket',str(sock),'--mime','application/pdf','--recording-handler','--authorization','once','--timeout-secs','10'],env=dict(os.environ,XDG_RUNTIME_DIR=str(runtime)),stdout=log,stderr=log)
  for _ in range(100):
   if sock.exists():break
   time.sleep(.05)
  run(ssh+['root@127.0.0.1','dd if=/dev/zero of=/tmp/large.pdf bs=1048576 count=64 status=none'])
  r=run(ssh+['root@127.0.0.1','/sender send-artifact --socket /tmp/handoff/broker.sock --input /tmp/large.pdf --mime application/pdf --kind document'],timeout=60)
  assert 'artifact_bytes: 67108864' in r.stdout
  results.append({'case':'64MiB_integrity_transfer_recording_handler','stdout':r.stdout})
  baseline={str(f):f.stat().st_size for f in runtime.rglob('*.pdf')}
  assert len(baseline)==1
  # Independent protocol producer pauses after an accepted 1 MiB prefix.
  run(['docker','cp',str(out/'partial_sender.py'),container+':/tmp/partial_sender.py'])
  partial=subprocess.Popen(ssh+['root@127.0.0.1','python3 /tmp/partial_sender.py'],stdout=(out/'partial.log').open('w'),stderr=subprocess.STDOUT)
  for _ in range(100):
   if 'PREFIX_SENT' in (out/'partial.log').read_text():break
   time.sleep(.05)
  assert 'PREFIX_SENT' in (out/'partial.log').read_text()
  tunnel.terminate();tunnel.wait(timeout=10);tunnel=None
  partial.wait(timeout=20)
  for _ in range(100):
   current={str(f):f.stat().st_size for f in runtime.rglob('*.pdf')}
   if current==baseline:break
   time.sleep(.1)
  assert current==baseline,current
  results.append({'case':'mid_payload_disconnect_partial_cleanup','passed':True})
  run(ssh+['root@127.0.0.1','rm /tmp/handoff/broker.sock'])
  tunnel=subprocess.Popen(ssh+['-N','-o','ExitOnForwardFailure=yes','-R','/tmp/handoff/broker.sock:'+str(sock),'root@127.0.0.1'],stdout=subprocess.DEVNULL,stderr=(out/'reconnect.log').open('w'))
  for _ in range(100):
   r=subprocess.run(ssh+['root@127.0.0.1','test -S /tmp/handoff/broker.sock'],capture_output=True)
   if r.returncode==0:break
   time.sleep(.05)
  r=run(ssh+['root@127.0.0.1','/sender send-artifact --socket /tmp/handoff/broker.sock --input /sample.pdf --mime application/pdf --kind document'],timeout=20)
  assert 'Opened' in r.stdout
  results.append({'case':'fresh_send_after_reconnect','passed':True})
  tunnel.terminate();tunnel.wait(timeout=10);tunnel=None
  r=subprocess.run(ssh+['root@127.0.0.1','/sender send-artifact --socket /tmp/handoff/broker.sock --input /sample.pdf --mime application/pdf --kind document'],timeout=20,capture_output=True,text=True)
  assert r.returncode != 0
  results.append({'case':'disconnected_before_send','returncode':r.returncode,'stdout':r.stdout,'stderr':r.stderr})

 finally:
  if tunnel:tunnel.terminate();tunnel.wait(timeout=10)
  if broker:broker.send_signal(signal.SIGTERM);broker.wait(timeout=10)
  subprocess.run(['docker','stop',container],capture_output=True)
 (out/'ssh-results.json').write_text(json.dumps({'topology':'macOS broker, Linux amd64 container producer, authenticated OpenSSH remote Unix socket forwarding','results':results},indent=2))
 print(results)
