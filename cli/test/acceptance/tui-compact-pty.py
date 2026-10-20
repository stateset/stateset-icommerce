"""Real compact terminal workflow; run through Buildr.

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
root = Path(tempfile.mkdtemp(prefix='tui-compact-pty-'))
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
    demand:{min:0,max:0},reorderPoint:0,targetStock:3,leadTimeDays:1,
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
    last = b''
    print('Waiting ' + repr(needle), flush=True)
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        start = buffer.find(b'\x1b[H')
        stop = buffer.find(b'\x1b[J', max(0, start))
        if start >= 0 and stop >= 0:
            frame, buffer = buffer[start:stop + 3], buffer[stop + 3:]
            plain = re.sub(rb'\x1b\[[0-9;?]*[A-Za-z]', b'', frame)
            last = plain
            if needle in plain and (absent is None or absent not in plain):
                frames.append(plain.decode(errors='replace'))
                return plain
            continue
        if select.select([master], [], [], 0.2)[0]:
            buffer = (buffer + os.read(master, 131072))[-1000000:]
        if child.poll() is not None:
            raise AssertionError('Exited: ' + buffer[-3000:].decode(errors='replace'))
    raise AssertionError('Missing ' + repr(needle) + ': ' + (last + buffer[-3000:]).decode(errors='replace'))


def send(value):
    os.write(master, value)


try:
    resize(20, 8)
    child = subprocess.Popen(
        [node, str(repository / 'cli/bin/stateset-tui.js'), '--demo', '--workspace', str(workspace)],
        stdin=slave, stdout=slave, stderr=slave, start_new_session=True,
        env={**os.environ, 'TERM': 'xterm-256color'},
    )
    screen(b'Home', absent=b'loading')
    screen(b'ready')
    send(b'7')
    screen(b'DOMAINS')
    send(b'/inventory\r')
    screen('› inventory'.encode())
    send(b'6l')
    screen(b'Purchasing')
    send(b'\r')
    screen(b'Loaded Purchasing')
    send(b'p\r')
    screen(b'SKU 1/12')
    send(b'\t' * 10)
    screen(b'Review interval')
    send(b'\x153')
    screen('3▏'.encode())
    resize(140, 44)
    frame = screen(b'SKU TERMS')
    assert b'review on day zero' in frame
    resize(20, 8)
    screen(b'SKU 11/12')
    send(b'\t\x152')
    frame = screen('2▏'.encode())
    assert b'Position cap' in frame
    send(b'\r')
    screen(b'SKU PRIORITY')
    send(b'\x1b')
    screen(b'Seed 77')
    send(b'g')
    screen(b'SET SIMULATION')
    send(b'\x15999999\r')
    screen(b'Seed 999999')
    send(b'e')
    screen(b'PLAN 1/3')
    send(b'\t\x153.25')
    frame = screen('3.25▏'.encode())
    assert b'Opening cash' in frame
    resize(140, 44)
    frame = screen(b'PLANNING ASSUMPTIONS')
    assert b'3.25' in frame
    resize(20, 8)
    screen(b'PLAN 2/3')
    send(b'\t\x150.50')
    frame = screen('0.50▏'.encode())
    assert b'Cash reserve' in frame
    resize(140, 44)
    frame = screen(b'PLANNING ASSUMPTIONS')
    assert b'0.50' in frame and b'withheld from purchases' in frame
    resize(20, 8)
    screen(b'PLAN 3/3')
    send(b'\x13')
    screen(b'USD 3.25')
    send(b'w')
    screen(b'SAVE SCENARIO')
    send(b'Compact plan\r')
    screen(b'Scenario saved')
    send(b'l')
    screen('› Compact plan'.encode())
    send(b'S')
    frame = screen(b'UPDATE')
    assert b'Enter replace Esc' in frame
    send(b'\x1b[4~')
    screen(b'scenario.')
    send(b'\x1b')
    screen(b'Update canceled')
    send(b'\r')
    screen(b'Loaded Compact')
    send(b's')
    screen(b'RESULTS')
    deadline = time.monotonic() + 10
    while True:
        saved = json.loads(next(workspace.glob('*/workspace.json')).read_text())
        if saved['runs']:
            break
        assert time.monotonic() < deadline, 'native result was not persisted'
        time.sleep(0.1)
    assert len(saved['runs']) == 1
    run = saved['runs'][0]
    assert run['seed'] == 999999 and run['controls']['startingCash'] == '3.25'
    assert run['controls']['cashReserve'] == '0.50'
    assert run['result']['inputs'][0]['cashReserve'] == '0.50'
    assert run['result']['results'][0]['summary']['endingCash'] == '3.25'
    assert run['products'][0]['purchasing']['reviewIntervalDays'] == 3
    assert run['products'][0]['purchasing']['maximumInventoryPosition'] == 2
    assert run['result']['inputs'][0]['products'][0]['purchasing'] == run['products'][0]['purchasing']
    assert run['result']['results'][0]['summary']['reserveBlockedPurchases'] == 3
    assert run['result']['results'][0]['summary']['constraintBlockedPurchases'] == 1
    assert run['result']['results'][0]['summary']['deferredPurchaseReviews'] == 2
    assert run['result']['results'][0]['summary']['purchasingSpend'] == '0.00'
    assert run['result']['results'][0]['purchases'] == []
    assert all(event['required'] == '3.00' and event['cash'] == '3.25' and event['spendableCash'] == '2.75'
               for event in run['result']['results'][0]['timeline'] if event['type'] == 'purchase_reserve_blocked')
    send(b'\x1b[4~')
    screen(b'lab.')
    resize(140, 44)
    screen(b'COMPARISON')
    send(b'\x1b[1~')
    screen(b'Ending cash USD 3.25')
    resize(20, 8)
    screen(b'RESULTS')
    send(b'\x1b')
    frame = screen(b'Seed 999999')
    assert b'USD 3.25' in frame
    assert json.loads(next(workspace.glob('*/workspace.json')).read_text()) == saved
    send(b'/Daily reviews\r')
    frame = screen(b'Daily reviews')
    assert '› Daily reviews'.encode() in frame
    send(b's')
    screen(b'RESULTS')
    deadline = time.monotonic() + 15
    while True:
        compared = json.loads(next(workspace.glob('*/workspace.json')).read_text())
        if len(compared['runs']) == 2:
            break
        assert time.monotonic() < deadline, 'daily comparison was not persisted'
        time.sleep(0.1)
    comparison = compared['runs'][0]
    assert comparison['presetId'] == 'daily-review'
    assert comparison['products'][0]['purchasing']['reviewIntervalDays'] == 3
    assert comparison['result']['inputs'][0]['products'][0]['purchasing']['reviewIntervalDays'] == 3
    assert 'reviewIntervalDays' not in comparison['result']['inputs'][1]['products'][0]['purchasing']
    assert [r['summary']['deferredPurchaseReviews'] for r in comparison['result']['results']] == [2, 0]
    assert [r['summary']['constraintBlockedPurchases'] for r in comparison['result']['results']] == [1, 3]
    assert compared['runs'][1] == saved['runs'][0]
    resize(140, 44)
    send(b'\x1b[1~')
    frame = screen(b'RESULTS  Daily reviews')
    assert b'Reorder, sizing & review rules different' in frame
    assert b'Projected paths: demand same' in frame
    for attempt in range(25):
        send(b'\x1b[6~')
        frame = screen(b'COMPARISON')
        if (b'INVENTORY RISK' in frame and b'No unmet-demand stockout recorded.' in frame
                and b'No outstanding purchases recorded at horizon.' in frame):
            break
    else:
        raise AssertionError('Inventory risk summary was not reachable by scrolling')
    assert json.loads(next(workspace.glob('*/workspace.json')).read_text()) == compared
    resize(20, 8)
    screen(b'RESULTS')
    send(b'q')
    assert child.wait(timeout=15) == 0
    after = termios.tcgetattr(slave)
    assert original[3] & (termios.ECHO | termios.ICANON) == after[3] & (termios.ECHO | termios.ICANON)
    evidence = {'viewport': '20x8', 'domainNavigation': True,
                'activeCashEditorVisible': True, 'editorSurvivesResize': True,
                'scenarioSavedAndReviewCanceled': True, 'nativeSimulationCompleted': True,
                'seed': 999999, 'exactEndingCash': '3.25',
                'reserveEditedAt20x8': True, 'reserveSurvivesResizeAndSave': True,
                'nativeReserveBlocksWholeOrders': True, 'exactCashReserve': '0.50',
                'skuReviewAndCapEditedAt20x8': True, 'skuRulesSurviveResizeAndSave': True,
                'nativeCapacityAndReviewDecisions': True,
                'dailyReviewComparisonSelectedAt20x8': True,
                'dailyComparisonChangesNativeDecisions': True, 'priorRunUnchanged': True,
                'inventoryRiskSummaryVisible': True,
                'reportsScrollAndSurviveResize': True, 'terminalRestored': True}
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
