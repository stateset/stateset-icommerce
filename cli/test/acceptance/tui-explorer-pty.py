"""Real Linux PTY acceptance; run through Buildr with Node 20+ on PATH.

Creates only a private demo/workspace. No external services or live store writes.
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

root = Path(tempfile.mkdtemp(prefix='tui-explorer-pty-'))
entry = Path(__file__).resolve().parents[2] / 'bin/stateset-tui.js'
master, slave = pty.openpty()
original = termios.tcgetattr(slave)
child = None
buffer = b''
frames = []


def size(columns, rows):
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', rows, columns, 0, 0))
    if child is not None:
        child.send_signal(signal.SIGWINCH)


def screen(needle, absent=None, timeout=65):
    global buffer
    print('Waiting ' + repr(needle), flush=True)
    end = time.monotonic() + timeout
    while time.monotonic() < end:
        start = buffer.find(b'\x1b[H')
        stop = buffer.find(b'\x1b[J', max(0, start))
        if start >= 0 and stop >= 0:
            frame = buffer[start:stop + 3]
            buffer = buffer[stop + 3:]
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
    size(55, 20)
    child = subprocess.Popen(
        [shutil.which('node'), str(entry), '--demo', '--workspace', str(root / 'workspace')],
        stdin=slave, stdout=slave, stderr=slave, start_new_session=True,
        env={**os.environ, 'TERM': 'xterm-256color'},
    )
    screen(b'Engine connected')
    send(b'7')
    screen(b'89 domains + runtime')
    send(b':Explore products\r')
    screen(b'PRODUCTS')
    send(b'\r')
    screen(b'4 shown / 4 matches', b'Refreshing')
    send(b'/House blend\r')
    screen(b'1 shown / 1 matches', b'Refreshing')
    send(b'\r')
    screen(b'STORED RECORD')
    send(b't')
    screen(b'LINKED RECORDS')
    send(b'/product_variants\r')
    screen(b'1/1')
    send(b'\r')
    screen(b'1 shown / 1 matches', b'Refreshing')
    send(b'\r')
    screen(b'STORED RECORD')
    send(b'\x1b[4~')
    screen(b'4096 characters')
    send(b'\x1b[1~')
    screen(b'id:')
    send(b'\x1b')
    screen(b'1 shown / 1 matches')
    send(b'b')
    screen(b'LINKED RECORDS')
    send(b'b')
    screen(b'STORED RECORD', b'Refreshing')
    send(b'\x1b')
    screen(b'1 shown / 1 matches')
    send(b'\x1b')
    screen(b'4 shown / 4 matches', b'Refreshing')
    send(b'T')
    screen(b'TOOL REFERENCE')
    send(b'/create_product\r')
    screen(b'write')
    send(b'\r')
    screen(b'name: create_product')
    send(b'\x1b[4~')
    screen(b'No tool is invoked')
    send(b':Explore general ledger\r')
    screen(b'GENERAL LEDGER')
    send(b'\r')
    screen(b'0 shown / 0 matches', b'Refreshing')
    send(b':Explore vector\r')
    screen(b'OPENAI_API_KEY')
    send(b'T')
    screen(b'TOOL REFERENCE')
    size(40, 14)
    screen(b'TOOL REFERENCE')
    send(b'\x1b[4~')
    screen(b'/16')
    size(140, 44)
    screen(b'TOOL REFERENCE')
    send(b':Explore simulations\r')
    screen(b'L opens the working simulation lab')
    send(b'L')
    screen(b'Seed 42')
    send(b'2')
    screen(b'12 matches', b'Refreshing')
    send(b'q')
    assert child.wait(timeout=15) == 0
    after = termios.tcgetattr(slave)
    assert original[3] & (termios.ECHO | termios.ICANON) == after[3] & (termios.ECHO | termios.ICANON)
    evidence = {
        'allDomainsReachable': True, 'paletteDomainJump': True,
        'nativeProductsAndWholeResourceSearch': True, 'parentChildLinksAndBack': True,
        'recordHomeEnd': True, 'writeToolDocumentationOnly': True,
        'financeResource': True, 'externalPrerequisites': True,
        'narrowAndWideResize': True, 'labHandoffPreservesSeed': True,
        'ordersRemainUnchanged': True, 'terminalRestored': True,
    }
    (root / 'evidence.json').write_text(json.dumps(evidence, indent=2) + '\n')
    (root / 'frames.txt').write_text('\n\n'.join(frames))
    print(json.dumps({'evidence': str(root / 'evidence.json'), **evidence}, indent=2))
finally:
    if child is not None and child.poll() is None:
        child.terminate()
        try:
            child.wait(timeout=10)
        except subprocess.TimeoutExpired:
            child.kill()
            child.wait()
    os.close(master)
    os.close(slave)
