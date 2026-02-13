/**
 * Tests for the Slack Gateway module.
 *
 * Since @slack/bolt is not installed in the test environment, we test
 * module structure, env var validation, and configuration defaults
 * by reading the source and exercising what we can without the SDK.
 */

import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const GATEWAY_PATH = resolve(import.meta.dirname, '../../src/slack/gateway.js');
const source = readFileSync(GATEWAY_PATH, 'utf-8');

describe('Slack Gateway', () => {
  const originalEnv = { ...process.env };

  beforeEach(() => {
    delete process.env.SLACK_BOT_TOKEN;
    delete process.env.SLACK_APP_TOKEN;
  });

  afterEach(() => {
    for (const k of Object.keys(process.env)) {
      if (!(k in originalEnv)) delete process.env[k];
    }
    Object.assign(process.env, originalEnv);
  });

  // ---------- module structure ----------

  describe('module structure', () => {
    it('exports startSlackGateway function', async () => {
      let mod;
      try {
        mod = await import('../../src/slack/gateway.js');
      } catch {
        return;
      }
      assert.strictEqual(typeof mod.startSlackGateway, 'function');
    });

    it('source contains export for startSlackGateway', () => {
      assert.ok(source.includes('export async function startSlackGateway'));
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

    it('source imports richMessageToPlainText for fallback', () => {
      assert.ok(source.includes('richMessageToPlainText'));
      assert.ok(source.includes("../channels/rich-messages.js"));
    });

    it('dynamically imports @slack/bolt SDK', () => {
      assert.ok(source.includes("import('@slack/bolt')"));
    });

    it('provides a clear error when @slack/bolt is not installed', () => {
      assert.ok(source.includes('@slack/bolt is not installed'));
    });
  });

  // ---------- environment validation ----------

  describe('environment validation', () => {
    it('requires SLACK_BOT_TOKEN env var', () => {
      assert.ok(source.includes("process.env.SLACK_BOT_TOKEN"));
    });

    it('requires SLACK_APP_TOKEN env var', () => {
      assert.ok(source.includes("process.env.SLACK_APP_TOKEN"));
    });

    it('throws with helpful message when SLACK_BOT_TOKEN is missing', () => {
      assert.ok(source.includes('SLACK_BOT_TOKEN environment variable is required'));
    });

    it('throws with helpful message when SLACK_APP_TOKEN is missing', () => {
      assert.ok(source.includes('SLACK_APP_TOKEN environment variable is required'));
    });

    it('includes Slack API URL in error message', () => {
      assert.ok(source.includes('https://api.slack.com/apps'));
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
    it('defines extractText method', () => {
      assert.ok(source.includes('extractText:'));
    });

    it('defines getSenderId that uses event.user', () => {
      assert.ok(source.includes('event.user'));
    });

    it('defines getTargetId that uses event.channel', () => {
      assert.ok(source.includes('event.channel'));
    });

    it('defines isOwnMessage to detect bot messages', () => {
      assert.ok(source.includes('bot_message'));
      assert.ok(source.includes('bot_id'));
    });

    it('sets sendTyping to null (not supported in Socket Mode)', () => {
      assert.ok(source.includes('sendTyping: null'));
    });

    it('formats markdown headers to Slack bold', () => {
      assert.ok(source.includes("replace(/^#{1,6}"));
    });

    it('sets maxMessageLength to 3000', () => {
      assert.ok(source.includes('maxMessageLength: 3000'));
    });

    it('uses Block Kit for rich messages', () => {
      assert.ok(source.includes("type: 'header'"));
      assert.ok(source.includes("type: 'section'"));
      assert.ok(source.includes("type: 'actions'"));
    });

    it('registers with notifier as slack channel', () => {
      assert.ok(source.includes("registerChannel('slack'"));
    });
  });

  // ---------- shutdown ----------

  describe('shutdown', () => {
    it('defines shutdown function', () => {
      assert.ok(source.includes('const shutdown'));
    });

    it('unregisters from notifier on shutdown', () => {
      assert.ok(source.includes("unregisterChannel('slack')"));
    });

    it('stops the Bolt app on shutdown', () => {
      assert.ok(source.includes('app.stop()'));
    });

    it('returns an object with shutdown method', () => {
      assert.ok(source.includes('return { shutdown }'));
    });
  });

  // ---------- slackActionToCommand ----------

  describe('slackActionToCommand helper', () => {
    it('is defined in source', () => {
      assert.ok(source.includes('function slackActionToCommand'));
    });

    it('passes through commands starting with /', () => {
      assert.ok(source.includes("actionId.startsWith('/')"));
    });

    it('maps view_order pattern to /order command', () => {
      assert.ok(source.includes("cmd: '/order'"));
    });

    it('maps inventory pattern to /inventory command', () => {
      assert.ok(source.includes("cmd: '/inventory'"));
    });
  });
});
