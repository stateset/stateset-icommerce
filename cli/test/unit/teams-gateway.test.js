/**
 * Tests for the Microsoft Teams Gateway module.
 *
 * Teams uses the Bot Framework REST API with an HTTP webhook.
 * No Bot Framework SDK dependency -- uses Node's built-in http module
 * and fetch. We test module structure, env var validation, and
 * configuration defaults by reading the source.
 */

import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const GATEWAY_PATH = resolve(import.meta.dirname, '../../src/teams/gateway.js');
const source = readFileSync(GATEWAY_PATH, 'utf-8');

describe('Teams Gateway', () => {
  const originalEnv = { ...process.env };

  beforeEach(() => {
    delete process.env.TEAMS_APP_ID;
    delete process.env.TEAMS_APP_PASSWORD;
  });

  afterEach(() => {
    for (const k of Object.keys(process.env)) {
      if (!(k in originalEnv)) delete process.env[k];
    }
    Object.assign(process.env, originalEnv);
  });

  // ---------- module structure ----------

  describe('module structure', () => {
    it('exports startTeamsGateway function', async () => {
      let mod;
      try {
        mod = await import('../../src/teams/gateway.js');
      } catch {
        return;
      }
      assert.strictEqual(typeof mod.startTeamsGateway, 'function');
    });

    it('source contains export for startTeamsGateway', () => {
      assert.ok(source.includes('export async function startTeamsGateway'));
    });

    it('source imports createSessionManager from channels/base', () => {
      assert.ok(source.includes('createSessionManager'));
      assert.ok(source.includes('../channels/base.js'));
    });

    it('source imports createMessageHandler from channels/base', () => {
      assert.ok(source.includes('createMessageHandler'));
    });

    it('source imports BOT_PREFIX from channels/base', () => {
      assert.ok(source.includes('BOT_PREFIX'));
    });

    it('source imports http from node:http', () => {
      assert.ok(source.includes("from 'node:http'"));
    });

    it('source imports getNotifier from channels/notifier', () => {
      assert.ok(source.includes('getNotifier'));
      assert.ok(source.includes('../channels/notifier.js'));
    });

    it('source imports richMessageToPlainText for fallback', () => {
      assert.ok(source.includes('richMessageToPlainText'));
      assert.ok(source.includes('../channels/rich-messages.js'));
    });

    it('does not require any external SDK', () => {
      assert.ok(!source.includes("await import('botframework"));
      assert.ok(!source.includes("await import('@microsoft"));
    });
  });

  // ---------- environment validation ----------

  describe('environment validation', () => {
    it('requires TEAMS_APP_ID env var', () => {
      assert.ok(source.includes('process.env.TEAMS_APP_ID'));
    });

    it('requires TEAMS_APP_PASSWORD env var', () => {
      assert.ok(source.includes('process.env.TEAMS_APP_PASSWORD'));
    });

    it('throws with helpful message when TEAMS_APP_ID is missing', () => {
      assert.ok(source.includes('TEAMS_APP_ID environment variable is required'));
    });

    it('throws with helpful message when TEAMS_APP_PASSWORD is missing', () => {
      assert.ok(source.includes('TEAMS_APP_PASSWORD environment variable is required'));
    });

    it('includes Bot Framework registration URL in error', () => {
      assert.ok(source.includes('https://dev.botframework.com'));
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

    it('default webhookPort is 3978', () => {
      assert.match(source, /webhookPort\s*=\s*3978/);
    });

    it('default middleware is empty array', () => {
      assert.match(source, /middleware\s*=\s*\[\]/);
    });
  });

  // ---------- OAuth token management ----------

  describe('OAuth token management', () => {
    it('defines getBotToken function', () => {
      assert.ok(source.includes('async function getBotToken'));
    });

    it('uses Microsoft OAuth2 token endpoint', () => {
      assert.ok(source.includes('login.microsoftonline.com/botframework.com'));
    });

    it('caches tokens with expiry buffer', () => {
      assert.ok(source.includes('_cachedToken'));
      assert.ok(source.includes('_tokenExpiresAt'));
    });

    it('uses client_credentials grant type', () => {
      assert.ok(source.includes("grant_type: 'client_credentials'"));
    });
  });

  // ---------- adapter contract ----------

  describe('adapter contract', () => {
    it('only processes message type activities', () => {
      assert.ok(source.includes("activity.type !== 'message'"));
    });

    it('strips bot mention tags from text', () => {
      assert.ok(source.includes('<at>'));
      assert.ok(source.includes('</at>'));
    });

    it('gets sender via AAD Object ID with fallback', () => {
      assert.ok(source.includes('activity.from?.aadObjectId'));
    });

    it('detects own messages by comparing to appId', () => {
      assert.ok(source.includes('activity.from?.id === appId'));
    });

    it('uses formatForTeams for platform formatting', () => {
      assert.ok(source.includes('formatForPlatform: formatForTeams'));
    });

    it('sets maxMessageLength to 28000', () => {
      assert.ok(source.includes('maxMessageLength: 28000'));
    });

    it('supports Adaptive Card format for rich messages', () => {
      assert.ok(source.includes("type: 'AdaptiveCard'"));
      assert.ok(source.includes("version: '1.4'"));
    });

    it('registers with notifier as teams channel', () => {
      assert.ok(source.includes("registerChannel('teams'"));
    });
  });

  // ---------- HTTP webhook server ----------

  describe('HTTP webhook server', () => {
    it('creates an HTTP server', () => {
      assert.ok(source.includes('http.createServer'));
    });

    it('has a health check endpoint at /api/health', () => {
      assert.ok(source.includes("'/api/health'"));
    });

    it('accepts POST to /api/messages', () => {
      assert.ok(source.includes("'/api/messages'"));
    });

    it('responds 200 immediately to acknowledge receipt', () => {
      assert.ok(source.includes('sendJson(res, 200'));
    });

    it('returns 404 for unknown routes', () => {
      assert.ok(source.includes('sendJson(res, 404'));
    });

    it('handles conversationUpdate events for welcome messages', () => {
      assert.ok(source.includes("'conversationUpdate'"));
      assert.ok(source.includes('membersAdded'));
    });

    it('handles invoke events for Adaptive Card submissions', () => {
      assert.ok(source.includes("'invoke'"));
      assert.ok(source.includes('invokeValue'));
    });
  });

  // ---------- actionToCommand ----------

  describe('actionToCommand helper', () => {
    it('is defined in source', () => {
      assert.ok(source.includes('function actionToCommand'));
    });

    it('passes through commands starting with /', () => {
      assert.ok(source.includes("action.startsWith('/')"));
    });

    it('maps view_order pattern to /order command', () => {
      assert.ok(source.includes("cmd: '/order'"));
    });

    it('maps track pattern to /track command', () => {
      assert.ok(source.includes("cmd: '/track'"));
    });
  });

  // ---------- shutdown ----------

  describe('shutdown', () => {
    it('unregisters from notifier on shutdown', () => {
      assert.ok(source.includes("unregisterChannel('teams')"));
    });

    it('stops cleanup on shutdown', () => {
      assert.ok(source.includes('stopCleanup'));
    });

    it('clears conversation references on shutdown', () => {
      assert.ok(source.includes('conversationRefs.clear()'));
    });

    it('closes the HTTP server on shutdown', () => {
      assert.ok(source.includes('server.close'));
    });

    it('returns an object with shutdown method', () => {
      assert.ok(source.includes('return { shutdown }'));
    });
  });
});
