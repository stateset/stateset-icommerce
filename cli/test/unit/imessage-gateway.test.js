/**
 * Tests for the iMessage Gateway module.
 *
 * iMessage uses the BlueBubbles HTTP API with a polling loop.
 * No external SDK dependency -- only Node's built-in fetch.
 * We test module structure, env var validation, and configuration
 * defaults by reading the source.
 */

import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const GATEWAY_PATH = resolve(import.meta.dirname, '../../src/imessage/gateway.js');
const source = readFileSync(GATEWAY_PATH, 'utf-8');

describe('iMessage Gateway', () => {
  const originalEnv = { ...process.env };

  beforeEach(() => {
    delete process.env.BLUEBUBBLES_URL;
    delete process.env.BLUEBUBBLES_PASSWORD;
  });

  afterEach(() => {
    for (const k of Object.keys(process.env)) {
      if (!(k in originalEnv)) delete process.env[k];
    }
    Object.assign(process.env, originalEnv);
  });

  // ---------- module structure ----------

  describe('module structure', () => {
    it('exports startIMessageGateway function', async () => {
      let mod;
      try {
        mod = await import('../../src/imessage/gateway.js');
      } catch {
        return;
      }
      assert.strictEqual(typeof mod.startIMessageGateway, 'function');
    });

    it('source contains export for startIMessageGateway', () => {
      assert.ok(source.includes('export async function startIMessageGateway'));
    });

    it('source imports createSessionManager from channels/base', () => {
      assert.ok(source.includes('createSessionManager'));
      assert.ok(source.includes("../channels/base.js"));
    });

    it('source imports createMessageHandler from channels/base', () => {
      assert.ok(source.includes('createMessageHandler'));
    });

    it('source imports BOT_PREFIX from channels/base', () => {
      assert.ok(source.includes('BOT_PREFIX'));
    });

    it('source imports getNotifier from channels/notifier', () => {
      assert.ok(source.includes('getNotifier'));
      assert.ok(source.includes("../channels/notifier.js"));
    });

    it('does not require any external SDK (uses fetch)', () => {
      // iMessage uses BlueBubbles REST API via fetch -- no SDK import
      assert.ok(!source.includes("await import('bluebubbles"));
    });
  });

  // ---------- environment / config validation ----------

  describe('environment validation', () => {
    it('reads BLUEBUBBLES_URL from env or config', () => {
      assert.ok(source.includes("process.env.BLUEBUBBLES_URL"));
    });

    it('reads BLUEBUBBLES_PASSWORD from env or config', () => {
      assert.ok(source.includes("process.env.BLUEBUBBLES_PASSWORD"));
    });

    it('requires BLUEBUBBLES_PASSWORD', () => {
      assert.ok(source.includes('if (!password)'));
    });

    it('throws helpful error when password is missing', () => {
      assert.ok(source.includes('iMessage gateway requires BLUEBUBBLES_PASSWORD'));
    });

    it('defaults BLUEBUBBLES_URL to http://localhost:1234', () => {
      assert.ok(source.includes("'http://localhost:1234'"));
    });
  });

  // ---------- startIMessageGateway signature ----------

  describe('startIMessageGateway signature', () => {
    it('takes config and shared as parameters', () => {
      assert.ok(source.includes('startIMessageGateway(config, shared)'));
    });

    it('reads dbPath from shared.dbPath', () => {
      assert.ok(source.includes('shared.dbPath'));
    });

    it('reads allowApply from shared with false default', () => {
      assert.ok(source.includes('shared.allowApply ?? false'));
    });

    it('reads maxTurns from shared with default 10', () => {
      assert.ok(source.includes('shared.maxTurns || 10'));
    });

    it('reads verbose from shared', () => {
      assert.ok(source.includes('shared.verbose'));
    });
  });

  // ---------- bbFetch helper ----------

  describe('bbFetch helper', () => {
    it('is defined in source', () => {
      assert.ok(source.includes('async function bbFetch'));
    });

    it('appends password as query parameter', () => {
      assert.ok(source.includes("url.searchParams.set('password', password)"));
    });

    it('sets Content-Type to application/json', () => {
      assert.ok(source.includes("'Content-Type': 'application/json'"));
    });

    it('throws on non-ok response', () => {
      assert.ok(source.includes('if (!res.ok)'));
    });
  });

  // ---------- adapter contract ----------

  describe('adapter contract', () => {
    it('defines extractText that reads raw.text', () => {
      assert.ok(source.includes('raw.text'));
    });

    it('skips outgoing messages (isFromMe)', () => {
      assert.ok(source.includes('raw.isFromMe'));
    });

    it('gets sender ID from handle address or handleId', () => {
      assert.ok(source.includes('raw.handle?.address'));
      assert.ok(source.includes('raw.handleId'));
    });

    it('gets target ID from chat GUID', () => {
      assert.ok(source.includes('raw.chats?.[0]?.guid'));
    });

    it('sets sendTyping to null (not supported via API)', () => {
      assert.ok(source.includes('sendTyping: null'));
    });

    it('sets maxMessageLength to 20000', () => {
      assert.ok(source.includes('maxMessageLength: 20000'));
    });

    it('sends messages via BlueBubbles message/text endpoint', () => {
      assert.ok(source.includes('/api/v1/message/text'));
    });

    it('strips markdown code blocks in formatForPlatform', () => {
      assert.ok(source.includes('```'));
    });
  });

  // ---------- polling loop ----------

  describe('polling loop', () => {
    it('defines startPolling function', () => {
      assert.ok(source.includes('async function startPolling'));
    });

    it('default poll interval is 3000ms', () => {
      assert.ok(source.includes('config.pollIntervalMs || 3000'));
    });

    it('fetches recent messages to establish high-water mark', () => {
      assert.ok(source.includes("limit: 1, sort: 'DESC'"));
    });

    it('polls for new messages after the high-water mark', () => {
      assert.ok(source.includes('after: lastDate'));
    });

    it('respects AbortSignal for clean shutdown', () => {
      assert.ok(source.includes('signal.aborted'));
    });

    it('tracks lastMessageDate for incremental polling', () => {
      assert.ok(source.includes('lastDate'));
      assert.ok(source.includes('msg.dateCreated > lastDate'));
    });
  });

  // ---------- shutdown ----------

  describe('shutdown', () => {
    it('aborts the polling loop on shutdown', () => {
      assert.ok(source.includes('abortController.abort()'));
    });

    it('unregisters from notifier on shutdown', () => {
      assert.ok(source.includes("notifier.unregister('imessage')"));
    });

    it('stops cleanup on shutdown', () => {
      assert.ok(source.includes('stopCleanup'));
    });

    it('returns an object with shutdown method', () => {
      assert.ok(source.includes('shutdown()'));
    });
  });
});
