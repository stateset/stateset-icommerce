import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { createOriginGuard } from '../../src/mcp/http-origins.js';

function check(guard, origin, rawHeaders) {
  const response = {
    writeHead(status) {
      this.status = status;
    },
    end(body) {
      this.body = JSON.parse(body);
    },
  };
  const accepted = guard({ headers: { origin }, rawHeaders }, response);
  if (!accepted) {
    assert.equal(response.status, 403);
    assert.match(response.body.error.message, /Invalid Origin/);
  }
  return accepted;
}

describe('MCP HTTP browser origins', () => {
  it('pins scheme and port when an operator supplies a full origin', () => {
    const guard = createOriginGuard(['https://agent.example.com:8443']);
    assert.equal(check(guard, 'https://agent.example.com:8443'), true);
    for (const origin of [
      'http://agent.example.com:8443',
      'https://agent.example.com',
      'https://agent.example.com:8444',
      'https://agent.example.com.evil.test:8443',
    ])
      assert.equal(check(guard, origin), false, origin);
  });

  it('normalizes default ports and DNS case', () => {
    const guard = createOriginGuard(['https://AGENT.example.com:443/']);
    assert.equal(check(guard, 'https://agent.example.com'), true);
    assert.equal(check(guard, 'https://AGENT.example.com:443'), true);
  });

  it('supports exact IPv6 origins and explicit legacy bare-host rules', () => {
    const guard = createOriginGuard(['https://[::1]:8443', 'studio.example.net']);
    assert.equal(check(guard, 'https://[::1]:8443'), true);
    assert.equal(check(guard, 'https://[::1]:8444'), false);
    assert.equal(check(guard, 'http://studio.example.net:3000'), true);
    assert.equal(check(guard, 'https://studio.example.net'), true);
  });

  it('permits absent Origin for native clients, even with no browser allowlist', () => {
    assert.equal(check(createOriginGuard(), undefined), true);
    assert.equal(check(createOriginGuard(), 'http://localhost:3000'), false);
  });

  it('limits the default development policy to HTTP(S) loopback hosts', () => {
    const guard = createOriginGuard([], { allowLocalhost: true });
    for (const origin of ['http://localhost:5173', 'https://127.0.0.1', 'http://[::1]:8080']) {
      assert.equal(check(guard, origin), true);
    }
    assert.equal(check(guard, 'https://localhost.evil.test'), false);
    assert.equal(
      check(
        createOriginGuard(['https://agent.example.com'], { allowLocalhost: true }),
        'http://localhost:5173',
      ),
      false,
    );
  });

  for (const origin of [
    '',
    'null',
    'not a url',
    'ftp://agent.example.com',
    'file://agent.example.com',
    'https://user@agent.example.com',
    'https://agent.example.com/path',
    'https://agent.example.com/a/..',
    'https://agent.example.com/..',
    'https://agent.example.com\\',
    'https://agent.example.com?x=1',
    'https://agent.example.com#fragment',
    'https://agent.example.com,https://evil.test',
    'https://agent.example.com https://evil.test',
    ' https://agent.example.com',
    'https://agent.example.com/',
    ['https://agent.example.com'],
  ]) {
    it(`rejects malformed or opaque request origins: ${JSON.stringify(origin)}`, () => {
      assert.equal(check(createOriginGuard(['https://agent.example.com']), origin), false);
    });
  }

  it('rejects duplicate Origin headers even if Node retained only the first', () => {
    const guard = createOriginGuard(['https://agent.example.com']);
    assert.equal(
      check(guard, 'https://agent.example.com', [
        'Origin',
        'https://agent.example.com',
        'oRiGiN',
        'https://evil.test',
      ]),
      false,
    );
  });

  for (const value of [
    '',
    '*',
    'null',
    'ftp://agent.example.com',
    'agent.example.com:3000',
    'https://user@agent.example.com',
    'https://agent.example.com/path',
    'https://[',
  ]) {
    it(`rejects invalid configuration before serving: ${JSON.stringify(value)}`, () => {
      assert.throws(() => createOriginGuard([value]), /--allowed-origin/);
    });
  }
});
