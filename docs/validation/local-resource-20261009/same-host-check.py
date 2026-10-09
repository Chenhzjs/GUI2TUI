import pathlib,tempfile,subprocess,time,wave,struct,zlib,json,signal
root=pathlib.Path.cwd(); out=root/'docs/validation/local-resource-20261009'
with tempfile.TemporaryDirectory(prefix='g2t-local-',dir='/tmp') as td:
 p=pathlib.Path(td)
 def chunk(tag,data):return struct.pack('!I',len(data))+tag+data+struct.pack('!I',zlib.crc32(tag+data)&0xffffffff)
 (p/'sample.png').write_bytes(bytes([137,80,78,71,13,10,26,10])+chunk(b'IHDR',struct.pack('!2I5B',32,32,8,2,0,0,0))+chunk(b'IDAT',zlib.compress((bytes([0])+bytes([50,128,192])*32)*32))+chunk(b'IEND',b''))
 with wave.open(str(p/'sample.wav'),'wb') as w:w.setnchannels(1);w.setsampwidth(2);w.setframerate(8000);w.writeframes(bytes(1600))
 socket=p/'broker.sock';binary=str(root/'target/debug/gui2tui')
 log=(out/'same-host-broker.log').open('w')
 broker=subprocess.Popen([binary,'endpoint','serve','--socket',str(socket),'--mime','image/png','--mime','application/pdf','--mime','audio/wav','--handler-program','/usr/bin/open','--authorization','once'],stdout=log,stderr=log)
 results=[]
 try:
  for _ in range(100):
   if socket.exists():break
   if broker.poll() is not None:raise RuntimeError('broker exited')
   time.sleep(.05)
  for file,mime,kind in [(p/'sample.png','image/png','image'),(root/'tests/fixtures/modality/sample.pdf','application/pdf','document'),(p/'sample.wav','audio/wav','audio')]:
   r=subprocess.run([binary,'endpoint','send-artifact','--socket',str(socket),'--input',str(file),'--mime',mime,'--kind',kind],capture_output=True,text=True,timeout=20)
   results.append(dict(mime=mime,returncode=r.returncode,stdout=r.stdout,stderr=r.stderr))
  time.sleep(3)
 finally:
  broker.send_signal(signal.SIGTERM);broker.wait(timeout=10);log.close()
 (out/'same-host.json').write_text(json.dumps({'handler':'/usr/bin/open','meaning':'system launcher accepted; visual rendering not asserted','results':results},indent=2))
 print(json.dumps(results))
