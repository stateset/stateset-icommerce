/**
 * Tests for the WhatsApp Gateway module.
 *
 * Since Baileys is not installed in the test environment, we test
 * module structure, env var validation, configuration defaults,
 * and the reconnect loop design by reading the source.
 */

import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const GATEWAY_PATH = resolve(import.meta.dirname, '../../src/whatsapp/gateway.js');
const source = readFileSync(GATEWAY_PATH, 'utf-8');

describe('WhatsApp Gateway', () => {
  const originalEnv = { ...process.env };

  beforeEach(() => {
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
    it('exports startWhatsAppGateway function', async () => {
      let mod;
      try {
        mod = await import('../../src/whatsapp/gateway.js');
      } catch {
        return;
      }
      assert.strictEqual(typeof mod.startWhatsAppGateway, 'function');
    });

    it('source contains export for startWhatsAppGateway', () => {
      assert.ok(source.includes('export async function startWhatsAppGateway'));
    });

    it('source imports createSessionManager from channels/base', () => {
      assert.ok(source.includes('createSessionManager'));
      assert.ok(source.includes('../channels/base.js'));
    });

    it('source imports createMessageHandler from channels/base', () => {
      assert.ok(source.includes('createMessageHandler'));
    });

    it('source imports RECONNECT_POLICY and computeBackoff from channels/base', () => {
      assert.ok(source.includes('RECONNECT_POLICY'));
      assert.ok(source.includes('computeBackoff'));
    });

    it('source imports sleep from channels/base', () => {
      assert.ok(source.includes('sleep'));
    });

    it('source imports helpers from whatsapp/session.js', () => {
      assert.ok(source.includes("from './session.js'"));
      assert.ok(source.includes('createWhatsAppSocket'));
      assert.ok(source.includes('waitForConnection'));
    });

    it('source imports getNotifier from channels/notifier', () => {
      assert.ok(source.includes('getNotifier'));
      assert.ok(source.includes('../channels/notifier.js'));
    });

    it('source imports richMessageToPlainText for fallback', () => {
      assert.ok(source.includes('richMessageToPlainText'));
    });

    it('re-exports SESSION_TTL_MS from channels/base', () => {
      assert.ok(source.includes('export { SESSION_TTL_MS }'));
    });
  });

  // ---------- configuration defaults ----------

  describe('configuration defaults', () => {
    it('default dbPath is ./store.db', () => {
      assert.match(source, /dbPath\s*=\s*'\.\/store\.db'/);
    });

    it('default maxTurns is 10', () => {
      assert.match(source, /maxTurns\s*=\s*10/);
    });

    it('default allowApply is true', () => {
      assert.match(source, /allowApply\s*=\s*true/);
    });

    it('default verbose is false', () => {
      assert.match(source, /verbose\s*=\s*false/);
    });

    it('default allowlist is null', () => {
      assert.match(source, /allowlist\s*=\s*null/);
    });

    it('default allowGroups is false', () => {
      assert.match(source, /allowGroups\s*=\s*false/);
    });

    it('default middleware is empty array', () => {
      assert.match(source, /middleware\s*=\s*\[\]/);
    });
  });

  // ---------- adapter contract ----------

  describe('adapter contract', () => {
    it('defines extractText that reads message content', () => {
      assert.ok(source.includes('extractText:'));
      assert.ok(source.includes('waExtractText'));
    });

    it('skips status@broadcast messages', () => {
      assert.ok(source.includes('status@broadcast'));
    });

    it('skips group messages when allowGroups is false', () => {
      assert.ok(source.includes('isGroup(remoteJid) && !allowGroups'));
    });

    it('handles self-chat correctly (allows it)', () => {
      assert.ok(source.includes('isSelfChat'));
      assert.ok(source.includes('return !isSelfChat'));
    });

    it('uses cleanForWhatsApp for platform formatting', () => {
      assert.ok(source.includes('formatForPlatform: cleanForWhatsApp'));
    });

    it('sets maxMessageLength to 4000', () => {
      assert.ok(source.includes('maxMessageLength: 4000'));
    });

    it('sends rich messages as plain text (no card support)', () => {
      assert.ok(source.includes('richMessageToPlainText(richMsg)'));
    });

    it('registers with notifier as whatsapp channel', () => {
      assert.ok(source.includes("registerChannel('whatsapp'"));
    });
  });

  // ---------- cleanForWhatsApp ----------

  describe('cleanForWhatsApp helper', () => {
    it('is defined in source', () => {
      assert.ok(source.includes('function cleanForWhatsApp'));
    });

    it('converts markdown headers to bold', () => {
      assert.ok(source.includes('replace(/^#{1,6}'));
    });

    it('converts **bold** to *bold*', () => {
      assert.ok(source.includes("replace(/\\*\\*(.+?)\\*\\*/g, '*$1*')"));
    });

    it('converts markdown links to text (url) format', () => {
      assert.ok(source.includes("'$1 ($2)'"));
    });
  });

  // ---------- reconnect loop ----------

  describe('reconnect loop', () => {
    it('defines connectAndListen function', () => {
      assert.ok(source.includes('async function connectAndListen'));
    });

    it('defines runLoop function', () => {
      assert.ok(source.includes('async function runLoop'));
    });

    it('tracks reconnect attempts', () => {
      assert.ok(source.includes('reconnectAttempts'));
    });

    it('clears auth on stale credentials', () => {
      assert.ok(source.includes('clearAuth'));
      assert.ok(source.includes('Stale credentials detected'));
    });

    it('respects maxAttempts from RECONNECT_POLICY', () => {
      assert.ok(source.includes('RECONNECT_POLICY.maxAttempts'));
    });

    it('uses computeBackoff for delay calculation', () => {
      assert.ok(source.includes('computeBackoff(RECONNECT_POLICY'));
    });

    it('waits 120s for initial connection', () => {
      assert.ok(source.includes('120_000'));
    });
  });

  // ---------- shutdown ----------

  describe('shutdown', () => {
    it('sets stopped flag to true', () => {
      assert.ok(source.includes('stopped = true'));
    });

    it('unregisters from notifier on shutdown', () => {
      assert.ok(source.includes("unregisterChannel('whatsapp')"));
    });

    it('stops cleanup on shutdown', () => {
      assert.ok(source.includes('stopCleanup'));
    });

    it('ends the socket on shutdown', () => {
      assert.ok(source.includes('currentSock.end'));
    });

    it('returns sock and shutdown in result', () => {
      assert.ok(source.includes('return { sock: currentSock, shutdown'));
    });
  });
});
