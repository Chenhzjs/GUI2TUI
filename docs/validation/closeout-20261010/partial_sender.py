import socket,json,struct,hashlib,time
s=socket.socket(socket.AF_UNIX);s.settimeout(15);s.connect('/tmp/handoff/broker.sock')
d={'request':'Artifact','descriptor':{'id':123,'kind':'Document','mime':'application/pdf','size':67108864,'hash':list(hashlib.sha256(bytes(67108864)).digest()),'display_name':'partial.pdf','lifetime':'Session'}}
b=json.dumps(d).encode();s.sendall(struct.pack('!I',len(b))+b)
def exact(n):
 out=b''
 while len(out)<n:
  part=s.recv(n-len(out))
  if not part:raise EOFError()
  out+=part
 return out
assert json.loads(exact(struct.unpack('!I',exact(4))[0]))['status']=='Approved'
s.sendall(bytes(1048576));print('PREFIX_SENT',flush=True)
try: print('PEER_END',s.recv(1),flush=True)
except OSError as e: print(type(e).__name__,flush=True)
s.close()
