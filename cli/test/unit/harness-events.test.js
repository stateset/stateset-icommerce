/**
 * Unit tests for harness events + context transform + provider overrides.
 */

import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { existsSync, unlinkSync } from 'node:fs';
import { runAgentLoop } from '../../src/claude-harness.js';
import { ModelProvider, resetProviderRegistry, getProviderRegistry } from '../../src/providers/base.js';

function newDbPath() {
  return join(
    tmpdir(),
    `stateset-harness-${process.pid}-${Date.now()}-${Math.random().toString(16).slice(2)}.db`,
  );
}

function cleanupDb(dbPath) {
  if (!dbPath) return;
  for (const suffix of ['', '-wal', '-shm']) {
    const path = `${dbPath}${suffix}`;
    if (existsSync(path)) {
      try {
        unlinkSync(path);
      } catch {
        // Ignore cleanup errors
      }
    }
  }
}

class MockProvider extends ModelProvider {
  constructor() {
    super('mock', { envKey: null, models: { default: 'mock-model' }, default: 'mock-model' });
    this.lastOptions = null;
    this.lastMessages = null;
  }

  async isAvailable() {
    return true;
  }

  async chat(messages, options = {}) {
    this.lastMessages = messages;
    this.lastOptions = options;
    return {
      text: 'mock response',
      model: options.model || 'mock-model',
      provider: 'mock',
      cost: 0,
      usage: { inputTokens: 0, outputTokens: 0 },
    };
  }
}

describe('runAgentLoop (non-Claude) enhancements', () => {
  let provider;

  beforeEach(() => {
    resetProviderRegistry();
    provider = new MockProvider();
    const registry = getProviderRegistry();
    registry.register(provider);
  });

  it('applies transformContext and emits lifecycle events', async () => {
    const events = [];
    const controller = new AbortController();
    const dbPath = newDbPath();

    const result = await runAgentLoop({
      request: 'What is the status?',
      provider: 'mock',
      model: 'mock-model',
      dbPath,
      enableSync: false,
      enableMemory: false,
      conversationHistory: [{ role: 'user', content: 'OLD' }],
      transformContext: async (history) => {
        return [{ role: 'user', content: 'TRANSFORMED' }];
      },
      getApiKey: async () => 'override-key',
      signal: controller.signal,
      onEvent: (event) => events.push(event.type),
    });

    assert.strictEqual(result.response, 'mock response');
    assert.ok(provider.lastOptions, 'provider chat options should be captured');
    assert.strictEqual(provider.lastOptions.apiKey, 'override-key');
    assert.ok(provider.lastOptions.signal, 'provider signal should be set');
    controller.abort('stop');
    assert.strictEqual(provider.lastOptions.signal.aborted, true);

    const userMessage = provider.lastMessages?.find((m) => m.role === 'user');
    assert.ok(userMessage, 'user message should be provided');
    assert.ok(userMessage.content.includes('TRANSFORMED'));
    assert.ok(!userMessage.content.includes('OLD'));

    const expected = [
      'agent_start',
      'turn_start',
      'message_start',
      'message_end',
      'message_start',
      'message_end',
      'turn_end',
      'agent_end',
    ];

    for (const type of expected) {
      assert.ok(events.includes(type), `expected event: ${type}`);
    }

    cleanupDb(dbPath);
  });
});
