import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { listenUrl, parseListenPort, requestPath } from '../../src/mcp/http-listen.js';

describe('MCP HTTP listen ports', () => {
  it('defaults to 8090 and supports zero and the TCP port boundaries', () => {
    assert.equal(parseListenPort(), 8090);
    for (const [input, expected] of [
      ['0', 0],
      ['1', 1],
      ['00080', 80],
      ['65535', 65535],
    ]) {
      assert.equal(parseListenPort(input), expected);
    }
  });

  for (const input of [
    '',
    '-1',
    '+80',
    '1.5',
    '8090garbage',
    '65536',
    '1e3',
    '0x50',
    ' 80',
    '80 ',
    '80\n',
    'Infinity',
    '9007199254740992',
  ]) {
    it(`rejects the entire invalid port argument ${JSON.stringify(input)}`, () => {
      assert.throws(() => parseListenPort(input), /--port.*integer.*0.*65535/);
    });
  }
});

describe('MCP HTTP addresses and routing', () => {
  it('formats IPv4, hostname and IPv6 listening addresses as usable URLs', () => {
    for (const [host, expected] of [
      ['127.0.0.1', 'http://127.0.0.1:8090'],
      ['localhost', 'http://localhost:8090'],
      ['::1', 'http://[::1]:8090'],
      ['::', 'http://[::]:8090'],
    ]) {
      assert.equal(listenUrl(host, 8090), expected);
      assert.equal(new URL(listenUrl(host, 8090)).port, '8090');
    }
  });

  it('routes health and MCP requests without depending on the bind address', () => {
    assert.equal(requestPath('/health?probe=1'), '/health');
    assert.equal(requestPath('/mcp?transport=http'), '/mcp');
    assert.equal(requestPath('http://[::1]:8090/mcp'), '/mcp');
    assert.equal(requestPath('/missing'), '/missing');
    assert.equal(requestPath(undefined), '/');
  });

  it('refuses malformed absolute request targets', () => {
    assert.equal(requestPath('http://['), null);
    assert.equal(requestPath('http://[::1'), null);
  });
});
