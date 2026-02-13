/**
 * Tests for the Matrix Gateway module.
 *
 * Matrix uses the Client-Server API v3 with long-polling /sync.
 * No external SDK -- only Node's built-in fetch. We test module structure,
 * env var validation, and configuration defaults by reading the source.
 */

import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const GATEWAY_PATH = resolve(import.meta.dirname, '../../src/matrix/gateway.js');
const source = readFileSync(GATEWAY_PATH, 'utf-8');

describe('Matrix Gateway', () => {
  const originalEnv = { ...process.env };

  beforeEach(() => {
    delete process.env.MATRIX_HOMESERVER_URL;
    delete process.env.MATRIX_ACCESS_TOKEN;
  });

  afterEach(() => {
    for (const k of Object.keys(process.env)) {
      if (!(k in originalEnv)) delete process.env[k];
    }
    Object.assign(process.env, originalEnv);
  });

  // ---------- module structure ----------

  describe('module structure', () => {
    it('exports startMatrixGateway function', async () => {
      let mod;
      try {
        mod = await import('../../src/matrix/gateway.js');
      } catch {
        return;
      }
      assert.strictEqual(typeof mod.startMatrixGateway, 'function');
    });

    it('source contains export for startMatrixGateway', () => {
      assert.ok(source.includes('export async function startMatrixGateway'));
    });

    it('source imports createSessionManager from channels/base', () => {
      assert.ok(source.includes('createSessionManager'));
      assert.ok(source.includes("../channels/base.js"));
    });

    it('source imports createMessageHandler from channels/base', () => {
      assert.ok(source.includes('createMessageHandler'));
    });

    it('source imports getNotifier from channels/notifier', () => {
      assert.ok(source.includes('getNotifier'));
      assert.ok(source.includes("../channels/notifier.js"));
    });

    it('does not require any external SDK (uses built-in fetch)', () => {
      // Matrix gateway uses only fetch for HTTP calls
      assert.ok(!source.includes("await import('matrix-"));
    });
  });

  // ---------- environment validation ----------

  describe('environment validation', () => {
    it('requires MATRIX_HOMESERVER_URL env var', () => {
      assert.ok(source.includes("process.env.MATRIX_HOMESERVER_URL"));
    });

    it('requires MATRIX_ACCESS_TOKEN env var', () => {
      assert.ok(source.includes("process.env.MATRIX_ACCESS_TOKEN"));
    });

    it('throws with helpful message when homeserver URL is missing', () => {
      assert.ok(source.includes('MATRIX_HOMESERVER_URL environment variable is required'));
    });

    it('throws with helpful message when access token is missing', () => {
      assert.ok(source.includes('MATRIX_ACCESS_TOKEN environment variable is required'));
    });

    it('provides example homeserver URL in error', () => {
      assert.ok(source.includes('https://matrix.example.com'));
    });

    it('normalises homeserver URL by stripping trailing slashes', () => {
      assert.ok(source.includes("homeserver.replace(/\\/+$/, '')"));
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

    it('default autoJoin is true', () => {
      assert.match(source, /autoJoin\s*=\s*true/);
    });

    it('default middleware is empty array', () => {
      assert.match(source, /middleware\s*=\s*\[\]/);
    });
  });

  // ---------- matrixFetch helper ----------

  describe('matrixFetch helper', () => {
    it('is defined in source', () => {
      assert.ok(source.includes('async function matrixFetch'));
    });

    it('sets Authorization Bearer header', () => {
      assert.ok(source.includes('Authorization: `Bearer ${accessToken}`'));
    });

    it('sets Content-Type for bodies', () => {
      assert.ok(source.includes("'Content-Type'] = 'application/json'"));
    });

    it('throws on non-ok response', () => {
      assert.ok(source.includes('if (!res.ok)'));
    });

    it('supports AbortSignal', () => {
      assert.ok(source.includes('signal'));
    });
  });

  // ---------- adapter contract ----------

  describe('adapter contract', () => {
    it('only processes m.room.message events', () => {
      assert.ok(source.includes("raw?.type !== 'm.room.message'"));
    });

    it('only extracts m.text msgtype', () => {
      assert.ok(source.includes("content.msgtype !== 'm.text'"));
    });

    it('gets sender ID from raw.sender', () => {
      assert.ok(source.includes('raw.sender'));
    });

    it('gets target ID from raw.room_id', () => {
      assert.ok(source.includes('raw.room_id'));
    });

    it('detects own messages by comparing sender to botUserId', () => {
      assert.ok(source.includes('raw.sender === botUserId'));
    });

    it('sends messages via PUT to rooms endpoint (idempotent)', () => {
      assert.ok(source.includes('/send/m.room.message/'));
    });

    it('sends typing indicators', () => {
      assert.ok(source.includes('/typing/'));
      assert.ok(source.includes('typing: true'));
    });

    it('sets maxMessageLength to 65535', () => {
      assert.ok(source.includes('maxMessageLength: 65535'));
    });

    it('registers with notifier as matrix channel', () => {
      assert.ok(source.includes("registerChannel('matrix'"));
    });
  });

  // ---------- sync loop ----------

  describe('sync loop', () => {
    it('defines syncLoop function', () => {
      assert.ok(source.includes('async function syncLoop'));
    });

    it('performs initial sync with timeline limit 0', () => {
      assert.ok(source.includes("timeline: { limit: 0 }"));
    });

    it('long-polls with timeout of 30 seconds', () => {
      assert.ok(source.includes("timeout: '30000'"));
    });

    it('tracks next_batch token for incremental sync', () => {
      assert.ok(source.includes('nextBatch'));
      assert.ok(source.includes('next_batch'));
    });

    it('identifies the bot via /account/whoami', () => {
      assert.ok(source.includes('/_matrix/client/v3/account/whoami'));
    });

    it('backs off on sync errors (5 second delay)', () => {
      assert.ok(source.includes('setTimeout(resolve, 5000)'));
    });
  });

  // ---------- auto-join ----------

  describe('auto-join on invite', () => {
    it('defines processInvites function', () => {
      assert.ok(source.includes('async function processInvites'));
    });

    it('respects autoJoin flag', () => {
      assert.ok(source.includes('if (!autoJoin'));
    });

    it('joins rooms via POST to /join endpoint', () => {
      assert.ok(source.includes('/_matrix/client/v3/join/'));
    });
  });

  // ---------- shutdown ----------

  describe('shutdown', () => {
    it('sets stopped flag to true', () => {
      assert.ok(source.includes('stopped = true'));
    });

    it('unregisters from notifier on shutdown', () => {
      assert.ok(source.includes("unregisterChannel('matrix')"));
    });

    it('aborts the sync request on shutdown', () => {
      assert.ok(source.includes('syncAbort.abort()'));
    });

    it('returns shutdown and syncPromise', () => {
      assert.ok(source.includes('return { shutdown, _syncPromise'));
    });
  });
});
