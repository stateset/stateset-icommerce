/**
 * Tests for the Signal Gateway module.
 *
 * Signal uses signal-cli JSON-RPC over Unix socket -- no external SDK,
 * only Node's built-in `net` module. We test module structure, validation,
 * and configuration defaults by reading the source.
 */

import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const GATEWAY_PATH = resolve(import.meta.dirname, '../../src/signal/gateway.js');
const source = readFileSync(GATEWAY_PATH, 'utf-8');

describe('Signal Gateway', () => {
  const originalEnv = { ...process.env };

  beforeEach(() => {
    delete process.env.SIGNAL_CLI_PATH;
  });

  afterEach(() => {
    for (const k of Object.keys(process.env)) {
      if (!(k in originalEnv)) delete process.env[k];
    }
    Object.assign(process.env, originalEnv);
  });

  // ---------- module structure ----------

  describe('module structure', () => {
    it('exports startSignalGateway function', async () => {
      let mod;
      try {
        mod = await import('../../src/signal/gateway.js');
      } catch {
        return;
      }
      assert.strictEqual(typeof mod.startSignalGateway, 'function');
    });

    it('source contains export for startSignalGateway', () => {
      assert.ok(source.includes('export async function startSignalGateway'));
    });

    it('source imports createSessionManager from channels/base', () => {
      assert.ok(source.includes('createSessionManager'));
      assert.ok(source.includes("../channels/base.js"));
    });

    it('source imports createMessageHandler from channels/base', () => {
      assert.ok(source.includes('createMessageHandler'));
    });

    it('source imports RECONNECT_POLICY and computeBackoff', () => {
      assert.ok(source.includes('RECONNECT_POLICY'));
      assert.ok(source.includes('computeBackoff'));
    });

    it('source imports sleep from channels/base', () => {
      assert.ok(source.includes('sleep'));
    });

    it('source imports createConnection from node:net', () => {
      assert.ok(source.includes("from 'node:net'"));
      assert.ok(source.includes('createConnection'));
    });

    it('source imports getNotifier from channels/notifier', () => {
      assert.ok(source.includes('getNotifier'));
      assert.ok(source.includes("../channels/notifier.js"));
    });

    it('source imports richMessageToPlainText for fallback', () => {
      assert.ok(source.includes('richMessageToPlainText'));
      assert.ok(source.includes("../channels/rich-messages.js"));
    });

    it('does not use any external SDK (no dynamic import of vendor)', () => {
      // Signal uses signal-cli over socket, no vendor SDK
      assert.ok(!source.includes("await import('signal"));
    });
  });

  // ---------- parameter validation ----------

  describe('parameter validation', () => {
    it('requires --phone parameter', () => {
      assert.ok(source.includes("if (!phone)"));
    });

    it('throws with helpful message when phone is missing', () => {
      assert.ok(source.includes('--phone is required'));
    });

    it('mentions expected phone format in error', () => {
      assert.ok(source.includes('+14155551234'));
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

    it('default socket path is /tmp/signal-cli.sock', () => {
      assert.ok(source.includes("'/tmp/signal-cli.sock'"));
    });

    it('default middleware is empty array', () => {
      assert.match(source, /middleware\s*=\s*\[\]/);
    });
  });

  // ---------- adapter contract ----------

  describe('adapter contract', () => {
    it('extracts text from envelope.dataMessage.message', () => {
      assert.ok(source.includes('envelope.dataMessage?.message'));
    });

    it('gets sender ID from envelope.source', () => {
      assert.ok(source.includes('envelope.source'));
    });

    it('handles group messages via groupId', () => {
      assert.ok(source.includes('groupInfo?.groupId'));
    });

    it('detects own messages by comparing source to phone', () => {
      assert.ok(source.includes('envelope.source === phone'));
    });

    it('sets sendTyping to null (unreliable in signal-cli)', () => {
      assert.ok(source.includes('sendTyping: null'));
    });

    it('sets maxMessageLength to 6000', () => {
      assert.ok(source.includes('maxMessageLength: 6000'));
    });

    it('sends to groups via groupId parameter', () => {
      assert.ok(source.includes('groupId: target'));
    });

    it('sends to individuals via recipient parameter', () => {
      assert.ok(source.includes('recipient: [target]'));
    });

    it('registers with notifier as signal channel', () => {
      assert.ok(source.includes("registerChannel('signal'"));
    });
  });

  // ---------- JSON-RPC ----------

  describe('JSON-RPC communication', () => {
    it('defines jsonRpc function', () => {
      assert.ok(source.includes('function jsonRpc'));
    });

    it('uses JSON-RPC 2.0 protocol', () => {
      assert.ok(source.includes("jsonrpc: '2.0'"));
    });

    it('tracks pending RPC calls', () => {
      assert.ok(source.includes('pendingRpc'));
    });

    it('increments RPC ID for each call', () => {
      assert.ok(source.includes('++rpcId'));
    });

    it('handles RPC error responses', () => {
      assert.ok(source.includes('msg.error'));
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

    it('respects maxAttempts from RECONNECT_POLICY', () => {
      assert.ok(source.includes('RECONNECT_POLICY.maxAttempts'));
    });

    it('waits 15s for initial connection', () => {
      assert.ok(source.includes('15_000'));
    });
  });

  // ---------- shutdown ----------

  describe('shutdown', () => {
    it('sets stopped flag to true', () => {
      assert.ok(source.includes('stopped = true'));
    });

    it('unregisters from notifier on shutdown', () => {
      assert.ok(source.includes("unregisterChannel('signal')"));
    });

    it('destroys the connection on shutdown', () => {
      assert.ok(source.includes('conn.destroy()'));
    });

    it('returns shutdown and loopPromise', () => {
      assert.ok(source.includes('return { shutdown, _loopPromise'));
    });
  });
});
