"""Governed pending-refund workflow on an owned database in a real terminal; use Buildr."""
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
root = Path(tempfile.mkdtemp(prefix='tui-refunds-pty-'))
workspace = root / 'workspace'
database = root / 'owned.db'
node = shutil.which('node')
fixture_module = json.dumps((repository / 'cli/test/fixtures/tui-orders.js').as_uri())
refund_module = json.dumps((repository / 'cli/test/fixtures/tui-refunds.js').as_uri())
fixture_script = f"""
import fs from 'node:fs';
import {{orderFixture}} from {fixture_module};
import {{refundKernel}} from {refund_module};
const fixture = await orderFixture(process.argv[1]);
const kernel = refundKernel();
fs.writeFileSync(process.argv[2],JSON.stringify(kernel.policy));
fs.writeFileSync(process.argv[3],JSON.stringify(kernel.principal));
console.log(JSON.stringify(fixture));
"""
fixture = json.loads(subprocess.check_output(
    [node, '--input-type=module', '-e', fixture_script, str(database),
     str(root / 'policy.json'), str(root / 'principal.json')], timeout=45))
payment = fixture['payment']
master, slave = pty.openpty()
original = termios.tcgetattr(slave)
child = None
buffer = b''
frames = []


def resize(columns, rows):
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', rows, columns, 0, 0))
    if child is not None:
        child.send_signal(signal.SIGWINCH)


def screen(needle, timeout=65, absent=None, required=()):
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
            if needle in plain and (absent is None or absent not in plain) and all(item in plain for item in required):
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
    send(b'5/' + payment['paymentNumber'].encode() + b'\r')
    screen(b'1 shown / 1 matches', absent=b'Refreshing',
           required=(b'Payments  / ' + payment['paymentNumber'].encode(), b'W refund request'))


def query(sql, args=()):
    with sqlite3.connect(database.as_uri() + '?mode=ro', uri=True) as connection:
        return connection.execute(sql, args).fetchall()


def stop():
    send(b'q')
    assert child.wait(timeout=20) == 0
    after = termios.tcgetattr(slave)
    assert original[3] & (termios.ECHO | termios.ICANON) == after[3] & (termios.ECHO | termios.ICANON)


def draft():
    send(b'W')
    screen(b'REFUND WORKFLOW')
    send(b'0.10\tCustomer request\r')
    frame = screen(b'Native receipt: previewed')
    assert b'USD 0.10' in frame and b'user:tui' in frame and b'store:tui' in frame


try:
    resize(140, 44)
    before = hashlib.sha256(database.read_bytes()).hexdigest()
    launch()
    draft()
    send(b'\r')
    screen(b'Preview only. Live actions require')
    assert query('SELECT count(*) FROM refunds') == [(0,)]
    assert query('SELECT count(*) FROM kernel_receipts') == [(0,)]
    send(b'\x1b')
    screen(b'1 shown / 1 matches', absent=b'REFUND WORKFLOW')
    stop()
    assert hashlib.sha256(database.read_bytes()).hexdigest() == before
    launch(apply=True)
    draft()
    send(b'\r')
    screen(b'Type APPLY to commit')
    send(b'apply\r')
    screen(b'Type APPLY exactly')
    assert query('SELECT count(*) FROM refunds') == [(0,)]
    send(b'\x1b')
    screen(b'Native receipt: previewed')
    resize(20, 8)
    screen(b'REFUND ')
    send(b'\x1b[4~')
    screen(b'operations.')
    send(b'\r')
    screen(b'Type APPLY')
    send(b'APPL')
    screen('APPL▏'.encode())
    resize(140, 44)
    frame = screen(b'Type APPLY to commit')
    assert b'APPL' in frame and b'USD 0.10' in frame
    send(b'Y\r\r')
    frame = screen(b'PENDING REFUND CREATED')
    assert b'Native receipt: succeeded' in frame
    assert b'No provider transfer' in frame
    assert query('SELECT amount,status FROM refunds') == [('0.10', 'pending')]
    assert query('SELECT amount_refunded,status FROM payments WHERE id = ?', (payment['id'],)) == [('0', 'completed')]
    assert query('SELECT count(*) FROM kernel_receipts') == [(1,)]
    history_file = next(workspace.glob('*/last-refund-action.json'))
    saved = json.loads(history_file.read_text())
    assert saved['state'] == 'finished' and saved['params']['amount'] == '0.10'
    assert saved['context'] == {'currency': 'USD'} and saved['reviewIssues'] == []
    assert query('SELECT count(*) FROM kernel_receipt_audit_log WHERE command_id = ?', (saved['receipt']['command_id'],)) == [(1,)]
    send(b'\x1b')
    screen(b'1 shown / 1 matches', absent=b'REFUND WORKFLOW')
    stop()
    before_recovery = hashlib.sha256(database.read_bytes()).hexdigest()
    launch()
    send(b'P')
    frame = screen(b'Native receipt recovered')
    assert b'PENDING REFUND CREATED' in frame
    assert saved['idempotencyKey'].encode() in frame
    send(b'\x1b')
    screen(b'1 shown / 1 matches', absent=b'REFUND WORKFLOW')
    send(b'H')
    screen(b'USD 0.10 pending refund request')
    send(b'\r\r')
    screen(b'Native receipt recovered')
    send(b'\x1b')
    screen(b'USD 0.10 pending refund request')
    send(b'\x1b')
    screen(b'1 shown / 1 matches', absent=b'REFUND WORKFLOW')
    stop()
    assert hashlib.sha256(database.read_bytes()).hexdigest() == before_recovery
    assert json.loads(history_file.read_text()) == saved
    assert query('SELECT count(*) FROM refunds') == [(1,)]
    assert query('SELECT count(*) FROM kernel_receipts') == [(1,)]
    evidence = {'previewOnlySourceUnchanged': True, 'exactMoneyAndScopeReviewed': True,
                'typedConfirmationAndCancel': True, 'compact20x8ReviewScroll': True,
                'confirmationSurvivesResize': True, 'doubleEnterOnePendingRefund': True,
                'providerSettlementNotClaimed': True, 'completedRefundBalanceUnchanged': True,
                'restartAndHistoryReadOnlyReconciliation': True, 'nativeAuditOneEntry': True,
                'terminalRestored': True, 'paymentId': payment['id'],
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
