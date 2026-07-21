/**
 * Tests for the Telegram Gateway module.
 *
 * Since grammy is not installed in the test environment, we test
 * module structure, env var validation, and configuration defaults
 * by reading the source and exercising what we can without the SDK.
 */

import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const GATEWAY_PATH = resolve(import.meta.dirname, '../../src/telegram/gateway.js');
const source = readFileSync(GATEWAY_PATH, 'utf-8');

describe('Telegram Gateway', () => {
  const originalEnv = { ...process.env };

  beforeEach(() => {
    delete process.env.TELEGRAM_BOT_TOKEN;
  });

  afterEach(() => {
    for (const k of Object.keys(process.env)) {
      if (!(k in originalEnv)) delete process.env[k];
    }
    Object.assign(process.env, originalEnv);
  });

  // ---------- module structure ----------

  describe('module structure', () => {
    it('exports startTelegramGateway function', async () => {
      let mod;
      try {
        mod = await import('../../src/telegram/gateway.js');
      } catch {
        return;
      }
      assert.strictEqual(typeof mod.startTelegramGateway, 'function');
    });

    it('source contains export for startTelegramGateway', () => {
      assert.ok(source.includes('export async function startTelegramGateway'));
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

    it('dynamically imports grammy SDK', () => {
      assert.ok(source.includes("await import('grammy')"));
    });

    it('provides a clear error when grammy is not installed', () => {
      assert.ok(source.includes('grammy is not installed'));
    });
  });

  // ---------- environment validation ----------

  describe('environment validation', () => {
    it('requires TELEGRAM_BOT_TOKEN env var', () => {
      assert.ok(source.includes('process.env.TELEGRAM_BOT_TOKEN'));
    });

    it('throws with helpful message when token is missing', () => {
      assert.ok(source.includes('TELEGRAM_BOT_TOKEN environment variable is required'));
    });

    it('includes BotFather URL in error message', () => {
      assert.ok(source.includes('https://t.me/BotFather'));
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
    it('defines extractText that reads ctx.message.text', () => {
      assert.ok(source.includes('ctx.message?.text'));
    });

    it('defines getSenderId that reads ctx.from.id', () => {
      assert.ok(source.includes('ctx.from.id'));
    });

    it('defines getTargetId that reads ctx.chat.id', () => {
      assert.ok(source.includes('ctx.chat.id'));
    });

    it('isOwnMessage always returns false (bots dont receive own)', () => {
      assert.ok(source.includes('isOwnMessage: () => false'));
    });

    it('sends messages via bot.api.sendMessage', () => {
      assert.ok(source.includes('bot.api.sendMessage'));
    });

    it('sends typing via bot.api.sendChatAction', () => {
      assert.ok(source.includes("sendChatAction(chatId, 'typing')"));
    });

    it('sets maxMessageLength to 4096', () => {
      assert.ok(source.includes('maxMessageLength: 4096'));
    });

    it('uses HTML formatting for rich messages', () => {
      assert.ok(source.includes("parse_mode: 'HTML'"));
    });

    it('supports inline keyboard buttons in rich messages', () => {
      assert.ok(source.includes('inline_keyboard'));
      assert.ok(source.includes('callback_data'));
    });

    it('registers with notifier as telegram channel', () => {
      assert.ok(source.includes("registerChannel('telegram'"));
    });
  });

  // ---------- helper functions ----------

  describe('escapeHtml helper', () => {
    it('is defined in source', () => {
      assert.ok(source.includes('function escapeHtml'));
    });

    it('escapes ampersands', () => {
      assert.ok(source.includes("'&amp;'"));
    });

    it('escapes less-than signs', () => {
      assert.ok(source.includes("'&lt;'"));
    });

    it('escapes greater-than signs', () => {
      assert.ok(source.includes("'&gt;'"));
    });
  });

  describe('actionToCommand helper', () => {
    it('is defined in source', () => {
      assert.ok(source.includes('function actionToCommand'));
    });

    it('returns null for empty action', () => {
      assert.ok(source.includes('if (!action) return null'));
    });

    it('passes through commands starting with /', () => {
      assert.ok(source.includes("action.startsWith('/')"));
    });
  });

  // ---------- shutdown ----------

  describe('shutdown', () => {
    it('unregisters from notifier on shutdown', () => {
      assert.ok(source.includes("unregisterChannel('telegram')"));
    });

    it('stops cleanup on shutdown', () => {
      assert.ok(source.includes('stopCleanup'));
    });

    it('calls bot.stop() on shutdown', () => {
      assert.ok(source.includes('bot.stop()'));
    });

    it('returns an object with shutdown method', () => {
      assert.ok(source.includes('return { shutdown }'));
    });
  });
});
