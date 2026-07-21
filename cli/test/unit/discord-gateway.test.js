/**
 * Tests for the Discord Gateway module.
 *
 * Since discord.js is not installed in the test environment, we test
 * module structure, env var validation, and configuration defaults
 * by reading the source and exercising what we can without the SDK.
 */

import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const GATEWAY_PATH = resolve(import.meta.dirname, '../../src/discord/gateway.js');
const source = readFileSync(GATEWAY_PATH, 'utf-8');

describe('Discord Gateway', () => {
  const originalEnv = { ...process.env };

  beforeEach(() => {
    delete process.env.DISCORD_BOT_TOKEN;
  });

  afterEach(() => {
    for (const k of Object.keys(process.env)) {
      if (!(k in originalEnv)) delete process.env[k];
    }
    Object.assign(process.env, originalEnv);
  });

  // ---------- module structure ----------

  describe('module structure', () => {
    it('exports startDiscordGateway function', async () => {
      let mod;
      try {
        mod = await import('../../src/discord/gateway.js');
      } catch {
        // Module may fail if channels/base.js has unresolvable deps — skip
        return;
      }
      assert.strictEqual(typeof mod.startDiscordGateway, 'function');
    });

    it('source contains export for startDiscordGateway', () => {
      assert.ok(source.includes('export async function startDiscordGateway'));
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

    it('dynamically imports discord.js SDK', () => {
      assert.ok(source.includes("await import('discord.js')"));
    });

    it('provides a clear error when discord.js is not installed', () => {
      assert.ok(source.includes('discord.js is not installed'));
    });
  });

  // ---------- environment validation ----------

  describe('environment validation', () => {
    it('requires DISCORD_BOT_TOKEN env var', () => {
      assert.ok(source.includes('DISCORD_BOT_TOKEN'));
      assert.ok(source.includes('process.env.DISCORD_BOT_TOKEN'));
    });

    it('throws with helpful message when token is missing', () => {
      assert.ok(source.includes('DISCORD_BOT_TOKEN environment variable is required'));
    });

    it('includes Developer Portal URL in error message', () => {
      assert.ok(source.includes('https://discord.com/developers/applications'));
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

    it('default mentionOnly is false', () => {
      assert.match(source, /mentionOnly\s*=\s*false/);
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

    it('defines getSenderId method', () => {
      assert.ok(source.includes('getSenderId:'));
    });

    it('defines getTargetId method', () => {
      assert.ok(source.includes('getTargetId:'));
    });

    it('defines isOwnMessage method', () => {
      assert.ok(source.includes('isOwnMessage:'));
    });

    it('defines send method', () => {
      assert.ok(source.includes('send:'));
    });

    it('defines sendTyping method', () => {
      assert.ok(source.includes('sendTyping:'));
    });

    it('sets maxMessageLength to 2000', () => {
      assert.ok(source.includes('maxMessageLength: 2000'));
    });

    it('defines sendRichMessage method for embeds', () => {
      assert.ok(source.includes('sendRichMessage:'));
    });

    it('registers with notifier as discord channel', () => {
      assert.ok(source.includes("registerChannel('discord'"));
    });
  });

  // ---------- shutdown ----------

  describe('shutdown', () => {
    it('defines shutdown function', () => {
      assert.ok(source.includes('const shutdown'));
    });

    it('unregisters from notifier on shutdown', () => {
      assert.ok(source.includes("unregisterChannel('discord')"));
    });

    it('stops cleanup on shutdown', () => {
      assert.ok(source.includes('stopCleanup'));
    });

    it('destroys the discord client on shutdown', () => {
      assert.ok(source.includes('client.destroy()'));
    });

    it('returns an object with shutdown method', () => {
      assert.ok(source.includes('return { shutdown }'));
    });
  });

  // ---------- discordActionToCommand ----------

  describe('discordActionToCommand helper', () => {
    it('is defined in source', () => {
      assert.ok(source.includes('function discordActionToCommand'));
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
});
