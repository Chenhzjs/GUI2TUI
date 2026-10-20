"""Read-only inventory helper for the native browser validation session."""
import argparse
import json
from pathlib import Path
import sys
sys.path.insert(0,str(Path.home()/'Document/Jev-Anything/src'))
from jev_anything.native import NativeBridge
p=argparse.ArgumentParser();p.add_argument('container');p.add_argument('--output',type=Path,required=True);a=p.parse_args()
b=NativeBridge(['docker','exec','-i',a.container,'python3','/web-tests/session.py','--exec-agent'])
try:
 s=b.snapshot();a.output.parent.mkdir(parents=True,exist_ok=True);a.output.write_text(json.dumps(s,indent=2))
 print('nodes',len(s['nodes']),'operations',len(s['operations']),'partial',s['partial'])
 for n in s['nodes']:
  if n['role'] in ('TextInput','CheckBox','RadioButton','ComboBox','List','ListItem','Slider','Tab','Dialog') or n['label'] in ('Advanced settings','Volume','Quantity','Rich notes'):
   print(n['id'],n['role'],n['label'],n['value'],[(o['kind'],o.get('name'),o.get('selected')) for o in s['operations'] if o['target']==n['id']])
finally:b.close()
