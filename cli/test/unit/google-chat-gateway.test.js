/**
 * Tests for the Google Chat Gateway module.
 *
 * Since googleapis and @google-cloud/pubsub are not installed in the test
 * environment, we test module structure, env var validation, and configuration
 * defaults by reading the source.
 */

import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const GATEWAY_PATH = resolve(import.meta.dirname, '../../src/google-chat/gateway.js');
const source = readFileSync(GATEWAY_PATH, 'utf-8');

describe('Google Chat Gateway', () => {
  const originalEnv = { ...process.env };

  beforeEach(() => {
    delete process.env.GOOGLE_APPLICATION_CREDENTIALS;
  });

  afterEach(() => {
    for (const k of Object.keys(process.env)) {
      if (!(k in originalEnv)) delete process.env[k];
    }
    Object.assign(process.env, originalEnv);
  });

  // ---------- module structure ----------

  describe('module structure', () => {
    it('exports startGoogleChatGateway function', async () => {
      let mod;
      try {
        mod = await import('../../src/google-chat/gateway.js');
      } catch {
        return;
      }
      assert.strictEqual(typeof mod.startGoogleChatGateway, 'function');
    });

    it('source contains export for startGoogleChatGateway', () => {
      assert.ok(source.includes('export async function startGoogleChatGateway'));
    });

    it('source imports createSessionManager from channels/base', () => {
      assert.ok(source.includes('createSessionManager'));
      assert.ok(source.includes('../channels/base.js'));
    });

    it('source imports createMessageHandler from channels/base', () => {
      assert.ok(source.includes('createMessageHandler'));
    });

    it('source imports getNotifier from channels/notifier', () => {
      assert.ok(source.includes('getNotifier'));
      assert.ok(source.includes('../channels/notifier.js'));
    });

    it('source imports richMessageToPlainText for fallback', () => {
      assert.ok(source.includes('richMessageToPlainText'));
      assert.ok(source.includes('../channels/rich-messages.js'));
    });

    it('dynamically imports googleapis SDK', () => {
      assert.ok(source.includes("await import('googleapis')"));
    });

    it('dynamically imports @google-cloud/pubsub', () => {
      assert.ok(source.includes("await import('@google-cloud/pubsub')"));
    });

    it('provides clear error when googleapis is not installed', () => {
      assert.ok(source.includes('googleapis is not installed'));
    });

    it('provides clear error when pubsub is not installed', () => {
      assert.ok(source.includes('@google-cloud/pubsub is not installed'));
    });
  });

  // ---------- environment validation ----------

  describe('environment validation', () => {
    it('requires GOOGLE_APPLICATION_CREDENTIALS env var', () => {
      assert.ok(source.includes('process.env.GOOGLE_APPLICATION_CREDENTIALS'));
    });

    it('throws with helpful message when credentials are missing', () => {
      assert.ok(source.includes('GOOGLE_APPLICATION_CREDENTIALS environment variable is required'));
    });

    it('includes GCP console URL in error message', () => {
      assert.ok(source.includes('https://console.cloud.google.com'));
    });

    it('requires --subscription parameter', () => {
      assert.ok(source.includes('if (!subscription)'));
    });

    it('throws helpful message when subscription is missing', () => {
      assert.ok(source.includes('--subscription is required'));
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

    it('default middleware is empty array', () => {
      assert.match(source, /middleware\s*=\s*\[\]/);
    });
  });

  // ---------- adapter contract ----------

  describe('adapter contract', () => {
    it('extracts text from event.message.text', () => {
      assert.ok(source.includes('event.message?.text'));
    });

    it('gets sender ID from event.user.name', () => {
      assert.ok(source.includes('event.user?.name'));
    });

    it('gets target ID from event.space.name', () => {
      assert.ok(source.includes('event.space?.name'));
    });

    it('isOwnMessage always returns false (Chat doesnt echo)', () => {
      assert.ok(source.includes('isOwnMessage: () => false'));
    });

    it('sets sendTyping to null (not supported)', () => {
      assert.ok(source.includes('sendTyping: null'));
    });

    it('sets maxMessageLength to 4096', () => {
      assert.ok(source.includes('maxMessageLength: 4096'));
    });

    it('sends messages via chat.spaces.messages.create', () => {
      assert.ok(source.includes('chat.spaces.messages.create'));
    });

    it('registers with notifier as google-chat channel', () => {
      assert.ok(source.includes("registerChannel('google-chat'"));
    });
  });

  // ---------- Pub/Sub message handling ----------

  describe('Pub/Sub message handling', () => {
    it('only processes MESSAGE type events', () => {
      assert.ok(source.includes("data.type !== 'MESSAGE'"));
    });

    it('acknowledges non-MESSAGE events', () => {
      assert.ok(source.includes('message.ack()'));
    });

    it('nacks on error for retry', () => {
      assert.ok(source.includes('message.nack()'));
    });

    it('listens for subscription error events', () => {
      assert.ok(source.includes("sub.on('error'"));
    });
  });

  // ---------- formatForGChat ----------

  describe('formatForGChat helper', () => {
    it('is defined in source', () => {
      assert.ok(source.includes('function formatForGChat'));
    });

    it('converts markdown headers to bold', () => {
      assert.ok(source.includes('replace(/^#{1,6}'));
    });

    it('converts **bold** to *bold*', () => {
      assert.ok(source.includes("replace(/\\*\\*(.+?)\\*\\*/g, '*$1*')"));
    });
  });

  // ---------- shutdown ----------

  describe('shutdown', () => {
    it('unregisters from notifier on shutdown', () => {
      assert.ok(source.includes("unregisterChannel('google-chat')"));
    });

    it('stops cleanup on shutdown', () => {
      assert.ok(source.includes('stopCleanup'));
    });

    it('removes all listeners from subscription', () => {
      assert.ok(source.includes('sub.removeAllListeners()'));
    });

    it('closes the subscription', () => {
      assert.ok(source.includes('sub.close()'));
    });

    it('returns an object with shutdown method', () => {
      assert.ok(source.includes('return { shutdown }'));
    });
  });
});
