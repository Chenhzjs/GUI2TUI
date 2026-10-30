"""Regression: the first raw key after optional F7 ownership returns must work.

Run in an existing test desktop with a uniquely named public AX application.
The helper is a terminal-only stub: no model, GUI writes, backing-file edits,
or application-specific production behavior. This tests the native input seam.
"""
import argparse
import json
from pathlib import Path
import tempfile

import pexpect


def check(binary, app, evidence):
    with tempfile.TemporaryDirectory(prefix='g2t-workflow-handoff-') as directory:
        helper = Path(directory) / 'workflow-stub'
        helper.write_text('''#!/usr/bin/env python3
import argparse,json,os
p=argparse.ArgumentParser()
p.add_argument('--result-file',required=True)
p.add_argument('--bridge',nargs=argparse.REMAINDER)
a=p.parse_args()
print('WORKFLOW_HANDOFF_READY',flush=True)
input()
fd=os.open(a.result_file,os.O_WRONLY|os.O_CREAT|os.O_EXCL,0o600)
with os.fdopen(fd,'w') as f:
    json.dump({'version':'jev.workflow.ui-result.v1','status':'idle','task_completed':None},f)
''')
        helper.chmod(0o700)
        with evidence.open('x') as output:
            child = pexpect.spawn(str(binary), ['--session','desktop','--app',app,
                '--no-mouse','--workflow-client',str(helper)], encoding='utf-8',
                timeout=30, dimensions=(40,120))
            child.logfile = output
            try:
                child.expect_exact('F6')
                child.send('\x1b[18~')  # Documented frontend F7, never a GUI key.
                child.expect_exact('WORKFLOW_HANDOFF_READY')
                child.sendline('0')
                child.expect_exact('\x1b[?1049h')
                # A second key must not be needed to rescue a stale wake batch.
                child.delaybeforesend = 0
                child.sendcontrol('c')
                child.expect(pexpect.EOF, timeout=5)
                child.close()
                if child.exitstatus != 0:
                    raise AssertionError(f'Unexpected native exit: {child.exitstatus}/{child.signalstatus}')
            finally:
                if child.isalive(): child.terminate(force=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--app', required=True)
    parser.add_argument('--evidence', type=Path, required=True)
    args = parser.parse_args()
    check(args.binary, args.app, args.evidence)
    print(json.dumps({'first_key_after_F7':'passed','provider_calls':0,'GUI_writes':0}))
