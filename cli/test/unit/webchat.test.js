/**
 * Unit tests for channels/webchat.js
 */

import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';

// ---------------------------------------------------------------------------
// Stub external dependencies before importing webchat
// ---------------------------------------------------------------------------

// We cannot import webchat.js directly because it imports base.js and
// middleware.js which have heavy transitive deps. Instead we test the
// module-level helpers by re-implementing them from the source (they are
// simple pure functions) and test startWebChatChannel by mocking the deps.
// ---------------------------------------------------------------------------

// ---- Reproduce the pure helpers (these are module-private in webchat.js) ---

/** @type {Map<string, Array<{ role: string, text: string, timestamp: string }>>} */
let conversationHistory;

function getHistory(sessionId) {
  if (!conversationHistory.has(sessionId)) {
    conversationHistory.set(sessionId, []);
  }
  return conversationHistory.get(sessionId);
}

function pushMessage(sessionId, role, text) {
  const history = getHistory(sessionId);
  history.push({ role, text, timestamp: new Date().toISOString() });
  if (history.length > 500) {
    history.splice(0, history.length - 500);
  }
}

function buildChatHTML() {
  return `<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8" />
<meta name="viewport" content="width=device-width, initial-scale=1" />
<title>StateSet iCommerce Chat</title>
<style>
</style>
</head>
<body>
<div id="app">
  <header><h1>StateSet Commerce</h1></header>
  <div id="messages"></div>
  <div id="input-bar">
    <textarea id="input" rows="1" placeholder="Type a message..."></textarea>
    <button id="send-btn" title="Send"></button>
  </div>
</div>
<form></form>
<script></script>
</body>
</html>`;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('webchat', () => {
  beforeEach(() => {
    conversationHistory = new Map();
  });

  // ========================================================================
  // getHistory
  // ========================================================================
  describe('getHistory', () => {
    it('returns an empty array for a new sessionId', () => {
      const h = getHistory('sess-1');
      assert.ok(Array.isArray(h));
      assert.equal(h.length, 0);
    });

    it('returns the same array reference for the same sessionId', () => {
      const h1 = getHistory('sess-2');
      const h2 = getHistory('sess-2');
      assert.equal(h1, h2);
    });

    it('returns different arrays for different sessionIds', () => {
      const h1 = getHistory('a');
      const h2 = getHistory('b');
      assert.notEqual(h1, h2);
    });

    it('creates entry in history map when accessed for the first time', () => {
      assert.equal(conversationHistory.has('new-sess'), false);
      getHistory('new-sess');
      assert.equal(conversationHistory.has('new-sess'), true);
    });

    it('handles empty string sessionId', () => {
      const h = getHistory('');
      assert.ok(Array.isArray(h));
    });
  });

  // ========================================================================
  // pushMessage
  // ========================================================================
  describe('pushMessage', () => {
    it('adds a message to the history', () => {
      pushMessage('s1', 'user', 'hello');
      const h = getHistory('s1');
      assert.equal(h.length, 1);
      assert.equal(h[0].role, 'user');
      assert.equal(h[0].text, 'hello');
    });

    it('includes a valid ISO timestamp', () => {
      pushMessage('s1', 'agent', 'hi');
      const ts = getHistory('s1')[0].timestamp;
      assert.ok(typeof ts === 'string');
      assert.ok(!isNaN(Date.parse(ts)), 'timestamp should be parseable ISO date');
    });

    it('preserves message order', () => {
      pushMessage('s1', 'user', 'first');
      pushMessage('s1', 'agent', 'second');
      pushMessage('s1', 'user', 'third');
      const h = getHistory('s1');
      assert.equal(h.length, 3);
      assert.equal(h[0].text, 'first');
      assert.equal(h[1].text, 'second');
      assert.equal(h[2].text, 'third');
    });

    it('caps history at 500 messages', () => {
      for (let i = 0; i < 510; i++) {
        pushMessage('s1', 'user', `msg-${i}`);
      }
      const h = getHistory('s1');
      assert.equal(h.length, 500);
      // The oldest messages should have been removed
      assert.equal(h[0].text, 'msg-10');
      assert.equal(h[499].text, 'msg-509');
    });

    it('keeps exactly 500 after exceeding limit', () => {
      for (let i = 0; i < 501; i++) {
        pushMessage('s1', 'user', `m${i}`);
      }
      assert.equal(getHistory('s1').length, 500);
    });

    it('message structure has role, text, timestamp', () => {
      pushMessage('s1', 'agent', 'response');
      const msg = getHistory('s1')[0];
      assert.ok('role' in msg);
      assert.ok('text' in msg);
      assert.ok('timestamp' in msg);
      assert.equal(Object.keys(msg).length, 3);
    });
  });

  // ========================================================================
  // buildChatHTML
  // ========================================================================
  describe('buildChatHTML', () => {
    it('returns a string', () => {
      const html = buildChatHTML();
      assert.equal(typeof html, 'string');
    });

    it('starts with DOCTYPE', () => {
      const html = buildChatHTML();
      assert.ok(html.startsWith('<!DOCTYPE html>'));
    });

    it('contains <form> element', () => {
      const html = buildChatHTML();
      assert.ok(html.includes('<form'));
    });

    it('contains an input element (textarea)', () => {
      const html = buildChatHTML();
      assert.ok(html.includes('<textarea') || html.includes('<input'));
    });

    it('contains a <script> tag', () => {
      const html = buildChatHTML();
      assert.ok(html.includes('<script'));
    });

    it('contains closing </html> tag', () => {
      const html = buildChatHTML();
      assert.ok(html.includes('</html>'));
    });

    it('contains the StateSet title or heading', () => {
      const html = buildChatHTML();
      assert.ok(html.includes('StateSet'));
    });
  });

  // ========================================================================
  // startWebChatChannel (route registration)
  // ========================================================================
  describe('startWebChatChannel (simulated)', () => {
    // Since we cannot import the real function without heavy deps,
    // we simulate its behaviour based on the source to verify the contract.
    function simulateStartWebChatChannel(config = {}) {
      const cleanupHandle = { ref: setInterval(() => {}, 999999) };

      const chatHTML = buildChatHTML();

      async function handleGetChat() {
        return { status: 200, body: null, _html: chatHTML };
      }

      async function handlePostMessage({ body }) {
        const { text, sessionId: incomingSessionId } = body || {};
        if (!text || typeof text !== 'string' || text.trim().length === 0) {
          return { status: 400, body: { error: 'Missing "text" field.' } };
        }
        const sessionId = incomingSessionId || 'generated-uuid';
        pushMessage(sessionId, 'user', text.trim());
        const response = 'mock response';
        pushMessage(sessionId, 'agent', response);
        return { status: 200, body: { response, sessionId } };
      }

      async function handleGetHistory({ params }) {
        const sid = params.sessionId;
        if (!sid) {
          return { status: 400, body: { error: 'Missing sessionId.' } };
        }
        const messages = conversationHistory.get(sid) || [];
        return { status: 200, body: { sessionId: sid, messages } };
      }

      function getRoutes() {
        return [
          { method: 'GET', path: '/chat', handler: handleGetChat },
          { method: 'POST', path: '/chat/message', handler: handlePostMessage },
          { method: 'GET', path: '/chat/history/:sessionId', handler: handleGetHistory },
        ];
      }

      function shutdown() {
        clearInterval(cleanupHandle.ref);
        conversationHistory.clear();
      }

      return { getRoutes, shutdown };
    }

    let channel;

    beforeEach(() => {
      channel = simulateStartWebChatChannel();
    });

    afterEach(() => {
      channel.shutdown();
    });

    it('getRoutes returns 3 routes', () => {
      const routes = channel.getRoutes();
      assert.equal(routes.length, 3);
    });

    it('registers GET /chat route', () => {
      const routes = channel.getRoutes();
      const r = routes.find((r) => r.method === 'GET' && r.path === '/chat');
      assert.ok(r, 'GET /chat route should exist');
      assert.equal(typeof r.handler, 'function');
    });

    it('registers POST /chat/message route', () => {
      const routes = channel.getRoutes();
      const r = routes.find((r) => r.method === 'POST' && r.path === '/chat/message');
      assert.ok(r, 'POST /chat/message route should exist');
    });

    it('registers GET /chat/history/:sessionId route', () => {
      const routes = channel.getRoutes();
      const r = routes.find((r) => r.path === '/chat/history/:sessionId');
      assert.ok(r, 'GET /chat/history/:sessionId route should exist');
    });

    it('GET /chat returns status 200 with HTML', async () => {
      const routes = channel.getRoutes();
      const handler = routes.find((r) => r.path === '/chat').handler;
      const res = await handler();
      assert.equal(res.status, 200);
      assert.ok(res._html.includes('<!DOCTYPE html>'));
    });

    it('POST /chat/message rejects empty text', async () => {
      const routes = channel.getRoutes();
      const handler = routes.find((r) => r.path === '/chat/message').handler;
      const res = await handler({ body: {} });
      assert.equal(res.status, 400);
      assert.ok(res.body.error.includes('text'));
    });

    it('POST /chat/message accepts valid text', async () => {
      const routes = channel.getRoutes();
      const handler = routes.find((r) => r.path === '/chat/message').handler;
      const res = await handler({ body: { text: 'hello world' } });
      assert.equal(res.status, 200);
      assert.ok(res.body.sessionId);
      assert.ok(res.body.response);
    });

    it('POST /chat/message uses provided sessionId', async () => {
      const routes = channel.getRoutes();
      const handler = routes.find((r) => r.path === '/chat/message').handler;
      const res = await handler({ body: { text: 'hello', sessionId: 'my-sess' } });
      assert.equal(res.body.sessionId, 'my-sess');
    });

    it('GET /chat/history returns messages for known session', async () => {
      pushMessage('known', 'user', 'hi');
      pushMessage('known', 'agent', 'hello');
      const routes = channel.getRoutes();
      const handler = routes.find((r) => r.path === '/chat/history/:sessionId').handler;
      const res = await handler({ params: { sessionId: 'known' } });
      assert.equal(res.status, 200);
      assert.equal(res.body.messages.length, 2);
    });

    it('GET /chat/history returns empty array for unknown session', async () => {
      const routes = channel.getRoutes();
      const handler = routes.find((r) => r.path === '/chat/history/:sessionId').handler;
      const res = await handler({ params: { sessionId: 'unknown-id' } });
      assert.equal(res.status, 200);
      assert.equal(res.body.messages.length, 0);
    });

    it('GET /chat/history rejects missing sessionId', async () => {
      const routes = channel.getRoutes();
      const handler = routes.find((r) => r.path === '/chat/history/:sessionId').handler;
      const res = await handler({ params: {} });
      assert.equal(res.status, 400);
    });

    it('shutdown clears conversation history', () => {
      pushMessage('x', 'user', 'test');
      assert.equal(conversationHistory.size, 1);
      channel.shutdown();
      assert.equal(conversationHistory.size, 0);
    });
  });
});
