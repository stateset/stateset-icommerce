"""Real terminal purchase-priority workflow; run through Buildr.

Uses an owned demo workspace and private native simulations only.
"""
import fcntl
import json
import os
from pathlib import Path
import pty
import re
import select
import shutil
import signal
import struct
import subprocess
import tempfile
import termios
import time

repository = Path(__file__).resolve().parents[3]
root = Path(tempfile.mkdtemp(prefix='tui-priority-pty-'))
workspace = root / 'workspace'
node = shutil.which('node')
fixture = """
import { TerminalWorkspace } from __WORKSPACE__;
import { initialState } from __MODEL__;
const workspace = new TerminalWorkspace(process.argv[1], 'demo');
await workspace.saveScenario('Purchasing priority', {
  presetId:'baseline', seed:77,
  controls:{...initialState().planning,days:2,startingCash:'10.00'},
  products:['COFFEE-250G','MATCHA-50G'].map(sku=>({
    sku,initialStock:0,unitPrice:'2.00',unitCost:'1.00',
    demand:{min:0,max:0},reorderPoint:0,targetStock:10,leadTimeDays:1,
    purchasing:{minimumOrderQuantity:1,orderMultiple:1}
  }))
});
""".replace('__WORKSPACE__', json.dumps((repository / 'cli/src/tui/workspace.js').as_uri())).replace(
    '__MODEL__', json.dumps((repository / 'cli/src/tui/model.js').as_uri()))
subprocess.run([node, '--input-type=module', '-e', fixture, str(workspace)], check=True, timeout=20)
master, slave = pty.openpty()
original = termios.tcgetattr(slave)
child = None
buffer = b''
frames = []


def resize(columns, rows):
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', rows, columns, 0, 0))
    if child is not None:
        child.send_signal(signal.SIGWINCH)


def screen(needle, timeout=65, absent=None):
    global buffer
    print('Waiting ' + repr(needle), flush=True)
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        start = buffer.find(b'\x1b[H')
        stop = buffer.find(b'\x1b[J', max(0, start))
        if start >= 0 and stop >= 0:
            frame, buffer = buffer[start:stop + 3], buffer[stop + 3:]
            plain = re.sub(rb'\x1b\[[0-9;?]*[A-Za-z]', b'', frame)
            if needle in plain and (absent is None or absent not in plain):
                frames.append(plain.decode(errors='replace'))
                return plain
            continue
        if select.select([master], [], [], 0.2)[0]:
            buffer = (buffer + os.read(master, 131072))[-1000000:]
        if child.poll() is not None:
            raise AssertionError('Exited: ' + buffer[-3000:].decode(errors='replace'))
    raise AssertionError('Missing ' + repr(needle) + ': ' + buffer[-3000:].decode(errors='replace'))


def send(value):
    os.write(master, value)


try:
    resize(55, 20)
    child = subprocess.Popen(
        [node, str(repository / 'cli/bin/stateset-tui.js'), '--demo', '--workspace', str(workspace)],
        stdin=slave, stdout=slave, stderr=slave, start_new_session=True,
        env={**os.environ, 'TERM': 'xterm-256color'},
    )
    screen(b'Engine connected')
    send(b'6l')
    screen(b'Purchasing priority')
    send(b'\r')
    screen(b'Loaded Purchasing priority')
    send(b'p')
    frame = screen(b'PURCHASE PRIORITY')
    assert b'1. COFFEE-250G' in frame and b'2. MATCHA-50G' in frame
    send(b'/MATCHA\r')
    screen(b'1 / 2 entries')
    send(b'[')
    screen(b'1. MATCHA-50G')
    resize(40, 14)
    screen(b'1. MATCHA-50G')
    resize(140, 44)
    screen(b'1. MATCHA-50G')
    send(b'w')
    screen(b'SAVE SCENARIO')
    send(b'Matcha first\r')
    screen(b'Scenario saved')
    saved = json.loads(next(workspace.glob('*/workspace.json')).read_text())
    selected = next(item for item in saved['scenarios'] if item['name'] == 'Matcha first')
    assert [item['sku'] for item in selected['products']] == ['MATCHA-50G', 'COFFEE-250G']
    assert not saved['runs'], 'changing priority and saving do not launch a simulation'
    send(b'\x1b')
    screen(b'2 / 2 entries')
    send(b'\x1b')
    screen(b'Seed 77')
    send(b's')
    frame = screen(b'COMPARISON')
    assert b'Lowest cash USD 0.00' in frame
    assert b'CASH-BUDGET REFUSALS' in frame
    assert b'COFFEE-250G: 3 blocked attempt(s)' in frame
    assert b'First refusal day 0: whole order USD 10.00; cash USD 0.00' in frame
    deadline = time.monotonic() + 10
    while True:
        saved = json.loads(next(workspace.glob('*/workspace.json')).read_text())
        if saved['runs']:
            break
        assert time.monotonic() < deadline, 'completed run was not persisted'
        time.sleep(0.1)
    run = saved['runs'][0]
    assert [item['sku'] for item in run['products']] == ['MATCHA-50G', 'COFFEE-250G']
    assert run['result']['inputs'][0]['products'][0]['sku'] == 'MATCHA-50G'
    report = run['result']['results'][0]
    assert [event['sku'] for event in report['timeline'] if event['type'] == 'purchase_order_sent'] == ['MATCHA-50G']
    assert report['summary']['purchasingSpend'] == '10.00'
    assert report['summary']['endingCash'] == '0.00'
    send(b'\x1b')
    screen(b'Seed 77')
    send(b'jjjs')
    frame = screen(b'RESULTS  Cash constraint')
    assert b'COMPARISON CHECKS' in frame
    assert b'Opening cash different' in frame
    assert b'Projected paths: demand same' in frame
    send(b'\x1b[4~')
    frame = screen(b'Conditional projections', absent=b'RUNNING')
    assert b'Baseline' in frame and b'USD 10.00 opening cash' in frame
    assert b'Cash constraint' in frame and b'USD 0.50 opening cash' in frame
    deadline = time.monotonic() + 10
    while True:
        saved = json.loads(next(workspace.glob('*/workspace.json')).read_text())
        if len(saved['runs']) == 2:
            break
        assert time.monotonic() < deadline, 'comparison history was not persisted'
        time.sleep(0.1)
    original_runs = saved['runs']
    send(b'a')
    frame = screen(b'Run assumptions loaded')
    assert b'USD 10.00 cash' in frame and b'Seed 77' in frame
    send(b'e')
    screen(b'PLANNING ASSUMPTIONS')
    send(b'\t\x1520.00\x13')
    screen(b'USD 20.00 cash')
    send(b'w')
    screen(b'SAVE SCENARIO')
    send(b'Edited run\r')
    screen(b'Scenario saved')
    saved = json.loads(next(workspace.glob('*/workspace.json')).read_text())
    edited = next(item for item in saved['scenarios'] if item['name'] == 'Edited run')
    assert edited['presetId'] == 'cash' and edited['seed'] == 77
    assert edited['controls']['startingCash'] == '20.00'
    assert [item['sku'] for item in edited['products']] == ['MATCHA-50G', 'COFFEE-250G']
    send(b'h/Steady demand\r')
    screen(b'1 / 2 entries')
    send(b'a')
    frame = screen(b'Run assumptions loaded')
    assert b'USD 10.00 cash' in frame
    assert b'Assumptions changed since this result' in frame
    send(b'w')
    screen(b'SAVE SCENARIO')
    send(b'History draft\r')
    screen(b'Scenario saved')
    saved = json.loads(next(workspace.glob('*/workspace.json')).read_text())
    history = next(item for item in saved['scenarios'] if item['name'] == 'History draft')
    assert history['presetId'] == 'baseline' and history['controls']['startingCash'] == '10.00'
    assert saved['runs'] == original_runs, 'loading, editing and saving drafts never changes or adds runs'
    before_rename = saved
    send(b'l/History draft\r')
    screen(b'1 / 4 entries')
    send(b'N')
    screen(b'RENAME SCENARIO')
    send(b'\x15Canceled name\x1b')
    screen(b'SAVED SCENARIOS')
    assert json.loads(next(workspace.glob('*/workspace.json')).read_text()) == before_rename
    send(b'N')
    screen(b'RENAME SCENARIO')
    resize(40, 14)
    screen(b'RENAME SCENARIO')
    resize(140, 44)
    screen(b'RENAME SCENARIO')
    send(b'\x15Renamed planning draft\r')
    frame = screen(b'Scenario renamed')
    assert b'4 / 4 entries' in frame and '› Renamed planning draft'.encode() in frame
    expected = json.loads(json.dumps(before_rename))
    next(item for item in expected['scenarios'] if item['id'] == history['id'])['name'] = 'Renamed planning draft'
    assert json.loads(next(workspace.glob('*/workspace.json')).read_text()) == expected
    send(b'q')
    assert child.wait(timeout=15) == 0
    buffer = b''
    child = subprocess.Popen(
        [node, str(repository / 'cli/bin/stateset-tui.js'), '--demo', '--workspace', str(workspace)],
        stdin=slave, stdout=slave, stderr=slave, start_new_session=True,
        env={**os.environ, 'TERM': 'xterm-256color'},
    )
    screen(b'Engine connected')
    send(b'6l/Renamed planning draft\r')
    frame = screen(b'1 / 4 entries')
    assert '› Renamed planning draft'.encode() in frame
    send(b'\r')
    screen(b'Loaded Renamed planning draft')
    assert json.loads(next(workspace.glob('*/workspace.json')).read_text()) == expected
    send(b'q')
    assert child.wait(timeout=15) == 0
    after = termios.tcgetattr(slave)
    assert original[3] & (termios.ECHO | termios.ICANON) == after[3] & (termios.ECHO | termios.ICANON)
    evidence = {'filteredSkuReordered': True, 'narrowAndWideResize': True,
                'savedOrderPreserved': True, 'noImplicitRun': True,
                'nativePurchaseHonorsPriority': True, 'exactSpend': '10.00',
                'cashBudgetEvidenceVisible': True, 'exactMinimumCash': '0.00',
                'comparisonChecksVisible': True, 'candidateInputsVisible': True,
                'resultDraftLoadedAndEdited': True, 'filteredHistoryDraftLoaded': True,
                'originalRunsUnchanged': True,
                'filteredScenarioRenamedInPlace': True, 'renameCancellationPreservedFile': True,
                'renamedScenarioLoadedAfterRestart': True,
                'terminalRestored': True}
    (root / 'evidence.json').write_text(json.dumps(evidence, indent=2) + '\n')
    print(json.dumps({'evidence': str(root / 'evidence.json'), **evidence}, indent=2))
finally:
    (root / 'frames.txt').write_text('\n\n'.join(frames))
    if child is not None and child.poll() is None:
        child.terminate()
        try:
            child.wait(timeout=10)
        except subprocess.TimeoutExpired:
            child.kill()
            child.wait()
    os.close(master)
    os.close(slave)
