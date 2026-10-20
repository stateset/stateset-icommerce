"""Governed order workflow on an owned database in a real terminal; use Buildr."""
import fcntl
import hashlib
import json
import os
from pathlib import Path
import pty
import re
import select
import shutil
import signal
import sqlite3
import struct
import subprocess
import tempfile
import termios
import time
import uuid

repository = Path(__file__).resolve().parents[3]
root = Path(tempfile.mkdtemp(prefix='tui-orders-pty-'))
workspace = root / 'workspace'
database = root / 'owned.db'
node = shutil.which('node')
fixture_module = json.dumps((repository / 'cli/test/fixtures/tui-orders.js').as_uri())
fixture_script = f"""
import fs from 'node:fs';
import {{orderFixture,operatorKernel}} from {fixture_module};
const fixture = await orderFixture(process.argv[1]);
const kernel = operatorKernel();
fs.writeFileSync(process.argv[2],JSON.stringify(kernel.policy));
fs.writeFileSync(process.argv[3],JSON.stringify(kernel.principal));
console.log(JSON.stringify(fixture));
"""
fixture = json.loads(subprocess.check_output(
    [node, '--input-type=module', '-e', fixture_script, str(database),
     str(root / 'policy.json'), str(root / 'principal.json')], timeout=45))
order = fixture['orders'][0]
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


def launch(apply=False):
    global child, buffer
    buffer = b''
    arguments = [node, str(repository / 'cli/bin/stateset-tui.js'), '--db', str(database),
                 '--workspace', str(workspace), '--no-color',
                 '--kernel-policy', str(root / 'policy.json'),
                 '--kernel-principal', str(root / 'principal.json'),
                 '--kernel-store-id', 'store:tui']
    if apply:
        arguments.append('--apply')
    child = subprocess.Popen(arguments, stdin=slave, stdout=slave, stderr=slave,
                             start_new_session=True, env={**os.environ, 'TERM': 'xterm-256color'})
    screen(b'Engine connected')
    send(b'2/' + order['orderNumber'].encode() + b'\r')
    screen(b'1 shown / 1 matches', absent=b'Refreshing')


def query(sql, args=()):
    with sqlite3.connect(database.as_uri() + '?mode=ro', uri=True) as connection:
        return connection.execute(sql, args).fetchall()


def stop():
    send(b'q')
    assert child.wait(timeout=20) == 0
    after = termios.tcgetattr(slave)
    assert original[3] & (termios.ECHO | termios.ICANON) == after[3] & (termios.ECHO | termios.ICANON)


try:
    resize(140, 44)
    before = hashlib.sha256(database.read_bytes()).hexdigest()
    launch()
    send(b'W\r')
    frame = screen(b'Native receipt: previewed')
    assert b'25.02' in frame and b'pending' in frame and b'confirmed' in frame
    assert b'user:tui' in frame and b'store:tui' in frame
    send(b'\r')
    screen(b'Preview only. Live actions require')
    assert query('SELECT status FROM orders WHERE id = ?', (order['id'],)) == [('pending',)]
    assert query('SELECT count(*) FROM kernel_receipts') == [(0,)]
    send(b'\x1b')
    screen(b'ORDER ', absent=b'ORDER WORKFLOW')
    stop()
    assert hashlib.sha256(database.read_bytes()).hexdigest() == before
    launch(apply=True)
    send(b'W\r')
    screen(b'Native receipt: previewed')
    with sqlite3.connect(database) as connection:
        connection.execute('UPDATE orders SET version = version + 1 WHERE id = ?', (order['id'],))
    send(b'\rAPPLY\r')
    frame = screen(b'kernel.version_conflict')
    assert b'Native receipt: rejected' in frame
    assert query('SELECT status FROM orders WHERE id = ?', (order['id'],)) == [('pending',)]
    conflict_record = json.loads(next(workspace.glob('*/last-order-action.json')).read_text())
    assert conflict_record['receipt']['status'] == 'rejected'
    send(b'r')
    screen(b'Confirm order')
    send(b'\r')
    frame = screen(b'Native receipt: previewed')
    assert b'Reviewed version: 2' in frame
    send(b'\r')
    screen(b'Type APPLY to commit')
    send(b'apply\r')
    screen(b'Type APPLY exactly')
    assert query('SELECT status FROM orders WHERE id = ?', (order['id'],)) == [('pending',)]
    send(b'\x1b')
    screen(b'Native receipt: previewed')
    resize(20, 8)
    screen(b'ORDER ')
    send(b'\x1b[4~')
    screen(b'live change has')
    send(b'\x1b[A')
    screen(b'confirmation.')
    send(b'\r')
    screen(b'Type APPLY')
    send(b'APPL')
    screen('APPL▏'.encode())
    resize(140, 44)
    frame = screen(b'Type APPLY to commit')
    assert b'APPL' in frame
    send(b'Y\r\r')
    frame = screen(b'Native receipt: succeeded')
    assert b'Native order status: confirmed' in frame
    assert query('SELECT status FROM orders WHERE id = ?', (order['id'],)) == [('confirmed',)]
    assert query('SELECT count(*) FROM kernel_receipts') == [(2,)]
    assert query("SELECT count(*) FROM kernel_receipts WHERE status = 'succeeded'") == [(1,)]
    deadline = time.monotonic() + 10
    while True:
        history = list(workspace.glob('*/last-order-action.json'))
        if history and json.loads(history[0].read_text())['state'] == 'finished':
            break
        assert time.monotonic() < deadline, 'native receipt was not journaled'
        time.sleep(0.1)
    saved = json.loads(history[0].read_text())
    assert saved['receipt']['status'] == 'succeeded'
    assert saved['idempotencyKey'] != conflict_record['idempotencyKey']
    assert saved['expectedVersion'] == 2
    assert saved['reviewIssues'] == []
    send(b'\x1b')
    screen(b'confirmed', absent=b'ORDER WORKFLOW')
    stop()
    launch()
    send(b'P')
    frame = screen(b'Native receipt recovered')
    assert b'Native receipt: succeeded' in frame and b'No command was repeated' in frame
    assert query('SELECT count(*) FROM kernel_receipts') == [(2,)]
    assert query("SELECT count(*) FROM kernel_receipts WHERE status = 'succeeded'") == [(1,)]
    assert json.loads(history[0].read_text()) == saved
    send(b'\x1b')
    screen(b'confirmed', absent=b'ORDER WORKFLOW')
    stop()
    # An older uncertain local intent remains reachable after the last pointer
    # moves. Its editable summary is never accepted as a native receipt.
    old_intent = next(workspace.glob('*/order-action-' + conflict_record['id'] + '.json'))
    old_record = {**conflict_record, 'state': 'pending',
                  'receipt': {**conflict_record['receipt'], 'status': 'succeeded'}}
    old_intent.write_text(json.dumps(old_record))
    os.utime(old_intent, (1700000000, 1700000000))
    # Put the real pending intent beyond the first history page. These extra
    # local records have no native receipts and are never submitted.
    for index in range(25):
        action_id = str(uuid.uuid4())
        extra = old_intent.parent / ('order-action-' + action_id + '.json')
        extra.write_text(json.dumps({**saved, 'id': action_id,
                                   'idempotencyKey': 'tui-order:' + action_id}))
        os.utime(extra, (1700100000 + index, 1700100000 + index))
    history_before = old_intent.read_bytes()
    source_before = hashlib.sha256(database.read_bytes()).hexdigest()
    launch()
    send(b'H')
    screen(b'1/25', absent=b'Loading local')
    send(b'f')
    frame = screen(b'PENDING ONLY')
    assert b'LOCAL PENDING' in frame
    assert conflict_record['idempotencyKey'].encode() in frame
    send(b'f')
    screen(b'1/25')
    send(b'nn')
    screen(b'Page 2')
    resize(20, 8)
    screen(b'HISTORY ')
    send(b'\x1b[4~')
    screen(b'LOCAL PENDING')
    send(b'\x1b[6~')
    screen(b'HISTORY ')
    send(b'\r\r')
    screen(b'Native receipt:')
    resize(140, 44)
    frame = screen(b'Native receipt: rejected')
    assert conflict_record['idempotencyKey'].encode() in frame
    assert b'kernel.version_conflict' in frame
    assert b'No command was repeated' in frame
    send(b'\x1b')
    frame = screen(b'LOCAL PENDING')
    assert b'Page 2' in frame
    send(b'p')
    screen(b'1/25')
    send(b'r')
    screen(b'1/25')
    send(b'\x1b')
    screen(b'confirmed', absent=b'ORDER WORKFLOW')
    stop()
    assert old_intent.read_bytes() == history_before
    assert hashlib.sha256(database.read_bytes()).hexdigest() == source_before
    assert query('SELECT count(*) FROM kernel_receipts') == [(2,)]
    evidence = {'previewOnlySourceUnchanged': True, 'exactMoneyAndScopeReviewed': True,
                'typedConfirmationAndCancel': True,
                'versionConflictRefuses': True, 'freshReviewUsesNewTicket': True, 'compact20x8ReviewScroll': True,
                'confirmationSurvivesResize': True, 'doubleEnterOneNativeReceipt': True,
                'orderStatusRefreshed': True, 'restartReadOnlyReconciliation': True,
                'historyPagingAndPendingFilter': True,
                'olderPendingHistoryRecovery': True, 'localReceiptNotTrusted': True,
                'historyReadOnlyAndSelectionSurvivesResize': True,
                'terminalRestored': True, 'orderId': order['id'],
                'commandId': saved['receipt']['command_id']}
    (root / 'evidence.json').write_text(json.dumps(evidence, indent=2) + '\n')
    print(json.dumps({'evidence': str(root / 'evidence.json'), **evidence}, indent=2))
finally:
    (root / 'frames.txt').write_text('\n\n'.join(frames))
    if child is not None and child.poll() is None:
        child.terminate()
        try:
            child.wait(timeout=15)
        except subprocess.TimeoutExpired:
            child.kill()
            child.wait()
    os.close(master)
    os.close(slave)
