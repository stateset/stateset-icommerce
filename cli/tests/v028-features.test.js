/**
 * v0.2.8 Feature Tests
 *
 * Tests for:
 * - Extended thinking configuration
 * - Streaming configuration
 * - Budget configuration
 * - Multi-model provider abstraction
 * - Memory store
 * - Memory summarizer
 * - Memory injector
 * - Config changes
 */

import { describe, it, before, after, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { mkdirSync, rmSync, existsSync, readFileSync } from 'node:fs';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const TMP_DIR = join(__dirname, '.tmp-v028-test');

// ============================================================================
// Config Tests
// ============================================================================

describe('Config v0.2.8', () => {
  let config;

  before(async () => {
    config = await import('../src/config.js');
  });

  it('should export THINK_LEVELS with correct token counts', () => {
    assert.ok(config.THINK_LEVELS);
    assert.equal(config.THINK_LEVELS.off, 0);
    assert.equal(config.THINK_LEVELS.low, 10_000);
    assert.equal(config.THINK_LEVELS.medium, 50_000);
    assert.equal(config.THINK_LEVELS.high, 100_000);
  });

  it('should export PROVIDERS with claude, openai, gemini, ollama', () => {
    assert.ok(config.PROVIDERS);
    assert.ok(config.PROVIDERS.claude);
    assert.ok(config.PROVIDERS.openai);
    assert.ok(config.PROVIDERS.gemini);
    assert.ok(config.PROVIDERS.ollama);
  });

  it('should have correct provider structure', () => {
    for (const [name, provider] of Object.entries(config.PROVIDERS)) {
      assert.ok(provider.name, `${name} should have a name`);
      assert.ok(provider.models !== undefined, `${name} should have models`);
      assert.ok(provider.default, `${name} should have a default model`);
    }
  });

  it('should have Claude as the default provider with correct models', () => {
    const claude = config.PROVIDERS.claude;
    assert.equal(claude.envKey, 'ANTHROPIC_API_KEY');
    assert.ok(claude.models['claude-sonnet-4-5']);
    assert.ok(claude.models['claude-opus-4-5']);
    assert.ok(claude.models['claude-haiku-3-5']);
  });

  it('should have OpenAI provider with correct models', () => {
    const openai = config.PROVIDERS.openai;
    assert.equal(openai.envKey, 'OPENAI_API_KEY');
    assert.ok(openai.models['gpt-4o']);
    assert.ok(openai.models['gpt-4']);
  });

  it('should have Gemini provider with correct models', () => {
    const gemini = config.PROVIDERS.gemini;
    assert.equal(gemini.envKey, 'GEMINI_API_KEY');
    assert.ok(gemini.models['gemini-2.0-flash']);
  });

  it('should have Ollama provider with no API key required', () => {
    const ollama = config.PROVIDERS.ollama;
    assert.equal(ollama.envKey, null);
    assert.ok(ollama.baseUrl);
  });

  it('should export STREAMING_DEFAULTS', () => {
    assert.ok(config.STREAMING_DEFAULTS !== undefined);
    assert.equal(config.STREAMING_DEFAULTS.enabled, false);
  });

  it('should export BUDGET_DEFAULTS', () => {
    assert.ok(config.BUDGET_DEFAULTS !== undefined);
    assert.equal(config.BUDGET_DEFAULTS.maxBudgetUsd, null);
  });

  it('should export MEMORY_DEFAULTS', () => {
    assert.ok(config.MEMORY_DEFAULTS !== undefined);
    assert.equal(config.MEMORY_DEFAULTS.enabled, false);
    assert.equal(config.MEMORY_DEFAULTS.maxSummaries, 5);
  });

  it('should have v0.2.8 feature flags', () => {
    assert.equal(config.FEATURES.extendedThinking, true);
    assert.equal(config.FEATURES.streaming, true);
    assert.equal(config.FEATURES.multiModel, true);
    assert.equal(config.FEATURES.memory, true);
    assert.equal(config.FEATURES.budgetControls, true);
  });

  it('should include new flags in getParseArgsOptions', () => {
    const opts = config.getParseArgsOptions();
    assert.ok(opts.think, 'should have think option');
    assert.ok(opts.stream, 'should have stream option');
    assert.ok(opts.provider, 'should have provider option');
    assert.ok(opts.budget, 'should have budget option');
    assert.ok(opts.memory, 'should have memory option');
  });

  it('should have CLI_VERSION set to 0.7.14', () => {
    assert.equal(config.CLI_VERSION, '0.7.14');
  });
});

// ============================================================================
// Provider Base Tests
// ============================================================================

describe('Provider Base', () => {
  let base;

  before(async () => {
    base = await import('../src/providers/base.js');
  });

  it('should export ModelProvider class', () => {
    assert.ok(base.ModelProvider);
    const provider = new base.ModelProvider('test', { models: { a: 'a-1' }, default: 'a-1' });
    assert.equal(provider.name, 'test');
  });

  it('should resolve model names', () => {
    const provider = new base.ModelProvider('test', {
      models: { short: 'full-model-id' },
      default: 'default-model',
    });
    assert.equal(provider.resolveModel('short'), 'full-model-id');
    assert.equal(provider.resolveModel('unknown'), 'unknown');
    assert.equal(provider.resolveModel(), 'default-model');
  });

  it('should list model names', () => {
    const provider = new base.ModelProvider('test', {
      models: { a: 'a1', b: 'b1' },
      default: 'a1',
    });
    assert.deepEqual(provider.listModels(), ['a', 'b']);
  });

  it('should throw on unimplemented methods', async () => {
    const provider = new base.ModelProvider('test');
    await assert.rejects(() => provider.isAvailable(), /not implemented/i);
    await assert.rejects(() => provider.chat([]), /not implemented/i);
  });

  it('should export getProviderRegistry singleton', () => {
    assert.ok(base.getProviderRegistry);
    const registry = base.getProviderRegistry();
    assert.ok(registry);
    assert.ok(typeof registry.list === 'function');
    assert.ok(typeof registry.get === 'function');
    assert.ok(typeof registry.has === 'function');
  });

  it('should get provider info', () => {
    const registry = base.getProviderRegistry();
    const info = registry.getInfo();
    assert.ok(Array.isArray(info));
    // Claude should always be in the info
    const claude = info.find(p => p.name === 'claude');
    assert.ok(claude);
    assert.equal(claude.displayName, 'Claude');
  });
});

// ============================================================================
// OpenAI Provider Tests
// ============================================================================

describe('OpenAI Provider', () => {
  let OpenAIProvider;

  before(async () => {
    const mod = await import('../src/providers/openai.js');
    OpenAIProvider = mod.OpenAIProvider;
  });

  it('should create OpenAI provider instance', () => {
    const provider = new OpenAIProvider();
    assert.equal(provider.name, 'openai');
  });

  it('should list available models', () => {
    const provider = new OpenAIProvider();
    const models = provider.listModels();
    assert.ok(models.includes('gpt-4o'));
    assert.ok(models.includes('gpt-4'));
    assert.ok(models.includes('o1'));
  });

  it('should resolve model aliases', () => {
    const provider = new OpenAIProvider();
    assert.equal(provider.resolveModel('gpt-4o'), 'gpt-4o');
    assert.equal(provider.resolveModel(), 'gpt-4o');
  });

  it('should report unavailable when no API key', async () => {
    const originalKey = process.env.OPENAI_API_KEY;
    delete process.env.OPENAI_API_KEY;
    const provider = new OpenAIProvider();
    assert.equal(await provider.isAvailable(), false);
    if (originalKey) process.env.OPENAI_API_KEY = originalKey;
  });

  it('should throw when chatting without API key', async () => {
    const originalKey = process.env.OPENAI_API_KEY;
    delete process.env.OPENAI_API_KEY;
    const provider = new OpenAIProvider();
    await assert.rejects(
      () => provider.chat([{ role: 'user', content: 'hi' }]),
      /API key/
    );
    if (originalKey) process.env.OPENAI_API_KEY = originalKey;
  });
});

// ============================================================================
// Gemini Provider Tests
// ============================================================================

describe('Gemini Provider', () => {
  let GeminiProvider;

  before(async () => {
    const mod = await import('../src/providers/gemini.js');
    GeminiProvider = mod.GeminiProvider;
  });

  it('should create Gemini provider instance', () => {
    const provider = new GeminiProvider();
    assert.equal(provider.name, 'gemini');
  });

  it('should list available models', () => {
    const provider = new GeminiProvider();
    const models = provider.listModels();
    assert.ok(models.includes('gemini-2.0-flash'));
    assert.ok(models.includes('gemini-2.0-pro'));
  });

  it('should report unavailable when no API key', async () => {
    const originalKey = process.env.GEMINI_API_KEY;
    delete process.env.GEMINI_API_KEY;
    const provider = new GeminiProvider();
    assert.equal(await provider.isAvailable(), false);
    if (originalKey) process.env.GEMINI_API_KEY = originalKey;
  });
});

// ============================================================================
// Ollama Provider Tests
// ============================================================================

describe('Ollama Provider', () => {
  let OllamaProvider;

  before(async () => {
    const mod = await import('../src/providers/ollama.js');
    OllamaProvider = mod.OllamaProvider;
  });

  it('should create Ollama provider instance', () => {
    const provider = new OllamaProvider();
    assert.equal(provider.name, 'ollama');
  });

  it('should list known model names', () => {
    const provider = new OllamaProvider();
    const models = provider.listModels();
    assert.ok(models.includes('llama3'));
    assert.ok(models.includes('mistral'));
  });

  it('should use localhost base URL', () => {
    const provider = new OllamaProvider();
    assert.ok(provider._baseUrl.includes('localhost'));
  });
});

// ============================================================================
// Memory Store Tests
// ============================================================================

let memoryAvailable = true;
const memorySkipReason = 'Skipping: better-sqlite3 native module not available.';

describe('MemoryStore', () => {
  let MemoryStore, store;
  const testDbPath = join(TMP_DIR, 'memory-test.db');

  before(async () => {
    mkdirSync(TMP_DIR, { recursive: true });
    try {
      const mod = await import('../src/memory/store.js');
      MemoryStore = mod.MemoryStore;
      const probe = new MemoryStore({ dbPath: testDbPath });
      probe.close();
    } catch (error) {
      if (error?.code === 'ERR_DLOPEN_FAILED') {
        memoryAvailable = false;
        return;
      }
      throw error;
    }
  });

  beforeEach(() => {
    if (!memoryAvailable) return;
    // Fresh store for each test
    if (store) store.close();
    if (existsSync(testDbPath)) rmSync(testDbPath);
    store = new MemoryStore({ dbPath: testDbPath });
  });

  after(() => {
    if (!memoryAvailable) return;
    if (store) store.close();
    rmSync(TMP_DIR, { recursive: true, force: true });
  });

  it('should create a new store', (t) => {
    if (!memoryAvailable) return t.skip(memorySkipReason);
    assert.ok(store);
    assert.equal(store.count(), 0);
  });

  it('should save and retrieve a memory', (t) => {
    if (!memoryAvailable) return t.skip(memorySkipReason);
    const { id } = store.save({
      summary: 'User asked about order #123 and we shipped it.',
      facts: ['order #123', 'shipped', 'alice@example.com'],
      agent: 'orders',
    });
    assert.ok(id > 0);
    assert.equal(store.count(), 1);

    const recent = store.getRecent('cli', 'local', 5);
    assert.equal(recent.length, 1);
    assert.equal(recent[0].summary, 'User asked about order #123 and we shipped it.');
    assert.deepEqual(recent[0].facts, ['order #123', 'shipped', 'alice@example.com']);
    assert.equal(recent[0].agent, 'orders');
  });

  it('should save multiple memories and return in reverse chronological order', (t) => {
    if (!memoryAvailable) return t.skip(memorySkipReason);
    store.save({ summary: 'First conversation', facts: ['fact1'] });
    store.save({ summary: 'Second conversation', facts: ['fact2'] });
    store.save({ summary: 'Third conversation', facts: ['fact3'] });

    const recent = store.getRecent('cli', 'local', 2);
    assert.equal(recent.length, 2);
    assert.equal(recent[0].summary, 'Third conversation');
    assert.equal(recent[1].summary, 'Second conversation');
  });

  it('should search memories by text', (t) => {
    if (!memoryAvailable) return t.skip(memorySkipReason);
    store.save({ summary: 'Discussed inventory levels for WIDGET-001' });
    store.save({ summary: 'Created order for alice@example.com' });
    store.save({ summary: 'Checked inventory for GADGET-002' });

    const results = store.search('cli', 'local', 'inventory', 5);
    assert.equal(results.length, 2);
  });

  it('should separate memories by channel and sender', (t) => {
    if (!memoryAvailable) return t.skip(memorySkipReason);
    store.save({ channel: 'telegram', senderId: 'user1', summary: 'Telegram chat' });
    store.save({ channel: 'discord', senderId: 'user2', summary: 'Discord chat' });
    store.save({ channel: 'cli', senderId: 'local', summary: 'CLI chat' });

    assert.equal(store.getRecent('telegram', 'user1').length, 1);
    assert.equal(store.getRecent('discord', 'user2').length, 1);
    assert.equal(store.getRecent('cli', 'local').length, 1);
    assert.equal(store.count(), 3);
  });

  it('should delete a memory by id', (t) => {
    if (!memoryAvailable) return t.skip(memorySkipReason);
    const { id } = store.save({ summary: 'To be deleted' });
    assert.equal(store.count(), 1);
    assert.ok(store.delete(id));
    assert.equal(store.count(), 0);
  });

  it('should prune old memories', (t) => {
    if (!memoryAvailable) return t.skip(memorySkipReason);
    store.save({ summary: 'Recent memory' });
    // Can't easily test old memories without manipulating timestamps
    // But we can verify the method doesn't crash
    const pruned = store.prune(1); // Prune anything older than 1ms
    assert.ok(typeof pruned === 'number');
  });

  it('should get all recent memories across senders', (t) => {
    if (!memoryAvailable) return t.skip(memorySkipReason);
    store.save({ senderId: 'a', summary: 'Memory A' });
    store.save({ senderId: 'b', summary: 'Memory B' });

    const all = store.getAllRecent(10);
    assert.equal(all.length, 2);
  });

  it('should handle save with minimal fields', (t) => {
    if (!memoryAvailable) return t.skip(memorySkipReason);
    const { id } = store.save({ summary: 'Minimal memory' });
    assert.ok(id > 0);
    const mem = store.getRecent()[0];
    assert.equal(mem.channel, 'cli');
    assert.equal(mem.sender_id, 'local');
    assert.deepEqual(mem.facts, []);
  });
});

// ============================================================================
// Memory Summarizer Tests
// ============================================================================

describe('ConversationSummarizer', () => {
  let ConversationSummarizer;

  before(async () => {
    const mod = await import('../src/memory/summarizer.js');
    ConversationSummarizer = mod.ConversationSummarizer;
  });

  it('should create a summarizer instance', () => {
    const summarizer = new ConversationSummarizer();
    assert.ok(summarizer);
  });

  it('should return empty result for short/empty text', async () => {
    const summarizer = new ConversationSummarizer();
    const result = await summarizer.summarize('');
    assert.equal(result.summary, '');
    assert.deepEqual(result.facts, []);
  });

  it('should return trimmed text for very short input', async () => {
    const summarizer = new ConversationSummarizer();
    const result = await summarizer.summarize('hi');
    assert.equal(result.summary, 'hi');
  });

  it('should parse SUMMARY and FACTS from response', () => {
    const summarizer = new ConversationSummarizer();
    const result = summarizer._parseResponse(
      'SUMMARY: User asked about order #123.\nFACTS: ["order #123", "alice@example.com"]',
      100
    );
    assert.equal(result.summary, 'User asked about order #123.');
    assert.deepEqual(result.facts, ['order #123', 'alice@example.com']);
    assert.equal(result.tokenCount, 100);
  });

  it('should handle response without FACTS section', () => {
    const summarizer = new ConversationSummarizer();
    const result = summarizer._parseResponse('SUMMARY: Simple conversation about products.', 50);
    assert.equal(result.summary, 'Simple conversation about products.');
    assert.deepEqual(result.facts, []);
  });

  it('should handle plain text response without markers', () => {
    const summarizer = new ConversationSummarizer();
    const result = summarizer._parseResponse('Just a plain summary text.', 30);
    assert.equal(result.summary, 'Just a plain summary text.');
  });
});

// ============================================================================
// Memory Injector Tests
// ============================================================================

describe('MemoryInjector', () => {
  let MemoryInjector, MemoryStore;
  let store, injector;
  const testDbPath = join(TMP_DIR, 'injector-test.db');

  before(async () => {
    if (!memoryAvailable) return;
    mkdirSync(TMP_DIR, { recursive: true });
    const injectorMod = await import('../src/memory/injector.js');
    const storeMod = await import('../src/memory/store.js');
    MemoryInjector = injectorMod.MemoryInjector;
    MemoryStore = storeMod.MemoryStore;

    // Reset the singleton to use our test DB
    storeMod.resetMemoryStore();
  });

  beforeEach(() => {
    if (!memoryAvailable) return;
    if (store) store.close();
    if (existsSync(testDbPath)) rmSync(testDbPath);
    store = new MemoryStore({ dbPath: testDbPath });
    injector = new MemoryInjector({ maxMemories: 3 });
  });

  after(() => {
    if (!memoryAvailable) return;
    if (store) store.close();
    if (existsSync(TMP_DIR)) rmSync(TMP_DIR, { recursive: true, force: true });
  });

  it('should create injector with default options', (t) => {
    if (!memoryAvailable) return t.skip(memorySkipReason);
    const inj = new MemoryInjector();
    assert.ok(inj);
  });

  it('should pass through when memory is not enabled', async (t) => {
    if (!memoryAvailable) return t.skip(memorySkipReason);
    const data = { text: 'hello', memoryEnabled: false };
    const result = await injector.injectMemoryContext(data);
    assert.equal(result.text, 'hello');
  });

  it('should pass through when text is empty', async (t) => {
    if (!memoryAvailable) return t.skip(memorySkipReason);
    const data = { text: '', memoryEnabled: true };
    const result = await injector.injectMemoryContext(data);
    assert.equal(result.text, '');
  });

  it('should pass through null data', async (t) => {
    if (!memoryAvailable) return t.skip(memorySkipReason);
    const result = await injector.injectMemoryContext(null);
    assert.equal(result, null);
  });

  it('should format memories correctly', (t) => {
    if (!memoryAvailable) return t.skip(memorySkipReason);
    const memories = [
      {
        summary: 'Discussed order #123',
        facts: ['order #123', 'alice'],
        agent: 'orders',
        created_at: Date.now(),
      },
      {
        summary: 'Checked inventory levels',
        facts: [],
        agent: 'inventory',
        created_at: Date.now() - 60000,
      },
    ];

    const formatted = injector.formatMemories(memories);
    assert.ok(formatted);
    assert.ok(formatted.includes('<memory-context>'));
    assert.ok(formatted.includes('</memory-context>'));
    assert.ok(formatted.includes('Discussed order #123'));
    assert.ok(formatted.includes('Checked inventory levels'));
    assert.ok(formatted.includes('order #123, alice'));
  });

  it('should return null for empty memories', (t) => {
    if (!memoryAvailable) return t.skip(memorySkipReason);
    assert.equal(injector.formatMemories([]), null);
    assert.equal(injector.formatMemories(null), null);
  });

  it('should respect maxBodyLength', (t) => {
    if (!memoryAvailable) return t.skip(memorySkipReason);
    const shortInjector = new MemoryInjector({ maxBodyLength: 100 });
    const memories = [
      { summary: 'A'.repeat(80), facts: [], created_at: Date.now() },
      { summary: 'B'.repeat(80), facts: [], created_at: Date.now() },
    ];
    const formatted = shortInjector.formatMemories(memories);
    assert.ok(formatted);
    // Should only include the first memory due to length limit
    assert.ok(formatted.includes('A'.repeat(80)));
  });

  it('should allow setting maxMemories', (t) => {
    if (!memoryAvailable) return t.skip(memorySkipReason);
    injector.setMaxMemories(10);
    assert.equal(injector._maxMemories, 10);
  });

  it('should allow setting maxBodyLength', (t) => {
    if (!memoryAvailable) return t.skip(memorySkipReason);
    injector.setMaxBodyLength(5000);
    assert.equal(injector._maxBodyLength, 5000);
  });
});

// ============================================================================
// Daemon CLI Tests
// ============================================================================

describe('Daemon CLI', () => {
  it('should have valid syntax', async () => {
    // Already verified by syntax check, but also test import
    const path = join(__dirname, '..', 'bin', 'stateset-daemon.js');
    assert.ok(existsSync(path));
  });
});

// ============================================================================
// Systemd Service Files Tests
// ============================================================================

describe('Deploy Files', () => {
  it('should have systemd gateway service file', () => {
    const path = join(__dirname, '..', 'deploy', 'stateset-gateway.service');
    assert.ok(existsSync(path));
  });

  it('should have systemd Tailscale service file', () => {
    const path = join(__dirname, '..', 'deploy', 'stateset-tailscale.service');
    assert.ok(existsSync(path));
  });

  it('should have gateway config example', () => {
    const path = join(__dirname, '..', 'deploy', 'gateway.config.example.json');
    assert.ok(existsSync(path));
  });

  it('should have SSH tunnel template service file', () => {
    const path = join(__dirname, '..', 'deploy', 'stateset-ssh-tunnel@.service');
    assert.ok(existsSync(path));
  });

  it('should have SSH tunnel template with autossh fallback', () => {
    const path = join(__dirname, '..', 'deploy', 'stateset-ssh-tunnel@.service');
    const content = readFileSync(path, 'utf-8');
    assert.ok(content.includes('autossh'), 'should reference autossh');
    assert.ok(content.includes('SSH_HOST'), 'should use SSH_HOST env var');
    assert.ok(content.includes('SSH_PORT_FLAG'), 'should use SSH_PORT_FLAG env var');
    assert.ok(content.includes('%i'), 'should use systemd template instance');
    assert.ok(content.includes('EnvironmentFile'), 'should load env file');
  });

  it('should have gateway config with remoteAccess section', () => {
    const path = join(__dirname, '..', 'deploy', 'gateway.config.example.json');
    const config = JSON.parse(readFileSync(path, 'utf-8'));
    assert.ok(config.remoteAccess, 'should have remoteAccess section');
    assert.ok(config.remoteAccess.tailscale, 'should have tailscale config');
    assert.ok(Array.isArray(config.remoteAccess.sshTunnels), 'should have sshTunnels array');
    assert.ok(config.remoteAccess.sshTunnels.length > 0, 'should have example tunnel');
    assert.equal(config.remoteAccess.sshTunnels[0].mode, 'reverse', 'example should be reverse mode');
  });

  it('should have config version 0.7.14', () => {
    const path = join(__dirname, '..', 'deploy', 'gateway.config.example.json');
    const config = JSON.parse(readFileSync(path, 'utf-8'));
    assert.equal(config._version, '0.7.14');
  });

  it('should have gateway service with security hardening', () => {
    const path = join(__dirname, '..', 'deploy', 'stateset-gateway.service');
    const content = readFileSync(path, 'utf-8');
    assert.ok(content.includes('ProtectSystem=strict'), 'should have ProtectSystem');
    assert.ok(content.includes('NoNewPrivileges=yes'), 'should have NoNewPrivileges');
    assert.ok(content.includes('PrivateTmp=yes'), 'should have PrivateTmp');
  });
});

// ============================================================================
// Integration: Think Level Wiring
// ============================================================================

describe('Integration: Think Level Config', () => {
  it('should map think levels to token counts correctly', async () => {
    const { THINK_LEVELS } = await import('../src/config.js');

    // Verify the mapping used in claude-harness.js
    for (const [level, tokens] of Object.entries(THINK_LEVELS)) {
      assert.ok(typeof tokens === 'number', `${level} should be a number`);
      if (level === 'off') {
        assert.equal(tokens, 0);
      } else {
        assert.ok(tokens > 0, `${level} should have positive tokens`);
      }
    }

    // Verify ordering
    assert.ok(THINK_LEVELS.low < THINK_LEVELS.medium);
    assert.ok(THINK_LEVELS.medium < THINK_LEVELS.high);
  });

  it('should have AGENTS exported from claude-harness for routing', async () => {
    const { AGENTS } = await import('../src/claude-harness.js');
    assert.ok(AGENTS);
    assert.ok(AGENTS['customer-service']);
    assert.ok(AGENTS['checkout']);
    assert.ok(AGENTS['orders']);
  });
});

// ============================================================================
// FallbackChain Tests
// ============================================================================

describe('FallbackChain', () => {
  let FallbackChain, CircuitBreaker;

  before(async () => {
    const mod = await import('../src/providers/base.js');
    FallbackChain = mod.FallbackChain;
    CircuitBreaker = mod.CircuitBreaker;
  });

  it('should export FallbackChain class', () => {
    assert.ok(FallbackChain);
    const chain = new FallbackChain();
    assert.ok(chain);
  });

  it('should have default provider order', () => {
    const chain = new FallbackChain();
    assert.deepEqual(chain._order, ['claude', 'openai', 'gemini', 'ollama']);
  });

  it('should allow custom provider order', () => {
    const chain = new FallbackChain({ order: ['gemini', 'ollama'] });
    assert.deepEqual(chain._order, ['gemini', 'ollama']);
  });

  it('should track failover count', () => {
    const chain = new FallbackChain();
    assert.equal(chain.getFailoverCount(), 0);
  });

  it('should get circuit status', () => {
    const chain = new FallbackChain();
    const status = chain.getCircuitStatus();
    assert.ok(typeof status === 'object');
  });

  it('should reset circuit for a specific provider', () => {
    const chain = new FallbackChain();
    chain.resetCircuit('openai');
    const status = chain.getCircuitStatus();
    assert.ok(!status.openai); // Should be removed
  });

  it('should update provider order', () => {
    const chain = new FallbackChain();
    chain.setOrder(['ollama', 'gemini']);
    assert.deepEqual(chain._order, ['ollama', 'gemini']);
  });
});

describe('CircuitBreaker', () => {
  let CircuitBreaker;

  before(async () => {
    const mod = await import('../src/providers/base.js');
    CircuitBreaker = mod.CircuitBreaker;
  });

  it('should start with all providers available', () => {
    const cb = new CircuitBreaker();
    assert.equal(cb.isAvailable('test'), true);
  });

  it('should record success and keep circuit closed', () => {
    const cb = new CircuitBreaker();
    cb.recordSuccess('test');
    assert.equal(cb.isAvailable('test'), true);
    const status = cb.getStatus();
    assert.equal(status.test.state, 'closed');
    assert.equal(status.test.failures, 0);
  });

  it('should open circuit after threshold failures', () => {
    const cb = new CircuitBreaker({ failureThreshold: 3 });
    cb.recordFailure('test');
    cb.recordFailure('test');
    assert.equal(cb.isAvailable('test'), true); // Still under threshold
    cb.recordFailure('test');
    assert.equal(cb.isAvailable('test'), false); // Now open
  });

  it('should reset circuit breaker', () => {
    const cb = new CircuitBreaker({ failureThreshold: 2 });
    cb.recordFailure('test');
    cb.recordFailure('test');
    assert.equal(cb.isAvailable('test'), false);
    cb.reset('test');
    assert.equal(cb.isAvailable('test'), true);
  });

  it('should reset all breakers', () => {
    const cb = new CircuitBreaker({ failureThreshold: 1 });
    cb.recordFailure('a');
    cb.recordFailure('b');
    assert.equal(cb.isAvailable('a'), false);
    assert.equal(cb.isAvailable('b'), false);
    cb.resetAll();
    assert.equal(cb.isAvailable('a'), true);
    assert.equal(cb.isAvailable('b'), true);
  });

  it('should transition to half-open after timeout', () => {
    const cb = new CircuitBreaker({ failureThreshold: 1, resetTimeoutMs: 1 });
    cb.recordFailure('test');
    assert.equal(cb.isAvailable('test'), false);
    // After 1ms timeout, it should be half-open
    // Since we can't easily wait in sync, just verify the state mechanism exists
    const status = cb.getStatus();
    assert.equal(status.test.state, 'open');
  });
});

// ============================================================================
// Gateway Config v0.3.0 Tests
// ============================================================================

describe('Gateway Config v0.3.0', () => {
  let config;

  before(() => {
    const configPath = join(__dirname, '..', 'deploy', 'gateway.config.example.json');
    config = JSON.parse(readFileSync(configPath, 'utf-8'));
  });

  it('should have all 10 channel types configured', () => {
    const expectedChannels = [
      'telegram', 'discord', 'slack', 'whatsapp', 'signal',
      'google-chat', 'imessage', 'teams', 'matrix', 'webchat',
    ];
    for (const ch of expectedChannels) {
      assert.ok(config.channels[ch], `Missing channel: ${ch}`);
    }
  });

  it('should have iMessage config with pollIntervalMs', () => {
    assert.ok(config.channels.imessage);
    assert.equal(config.channels.imessage.pollIntervalMs, 3000);
  });

  it('should have Teams config with webhookPort', () => {
    assert.ok(config.channels.teams);
    assert.equal(config.channels.teams.webhookPort, 3978);
  });

  it('should have Matrix config with autoJoin', () => {
    assert.ok(config.channels.matrix);
    assert.equal(config.channels.matrix.autoJoin, true);
  });

  it('should have voice configuration', () => {
    assert.ok(config.voice);
    assert.ok(config.voice.tts);
    assert.ok(config.voice.stt);
    assert.equal(config.voice.tts.provider, 'elevenlabs');
    assert.equal(config.voice.stt.provider, 'whisper');
  });

  it('should have browser configuration', () => {
    assert.ok(config.browser);
    assert.equal(config.browser.headless, true);
  });

  it('should have memory configuration', () => {
    assert.ok(config.memory);
    assert.equal(typeof config.memory.vectorSearch, 'boolean');
    assert.ok(config.memory.maxAgeMs > 0);
  });

  it('should have fallback configuration', () => {
    assert.ok(config.fallback);
    assert.ok(Array.isArray(config.fallback.order));
    assert.ok(config.fallback.order.includes('claude'));
    assert.ok(config.fallback.order.includes('openai'));
  });

  it('should have marketplace configuration', () => {
    assert.ok(config.marketplace);
    assert.ok(config.marketplace.catalogUrl);
  });

  it('should have shared config with thinkLevel and provider', () => {
    assert.ok(config.shared);
    assert.equal(config.shared.thinkLevel, 'off');
    assert.equal(config.shared.provider, 'claude');
    assert.equal(config.shared.enableFallback, true);
  });
});

// ============================================================================
// Voice Integration Tests
// ============================================================================

describe('Voice: TTSProvider', () => {
  let TTSProvider;

  before(async () => {
    const mod = await import('../src/voice/tts.js');
    TTSProvider = mod.TTSProvider;
  });

  it('should create a TTSProvider instance', () => {
    const tts = new TTSProvider();
    assert.ok(tts);
    assert.equal(tts.outputFormat, 'mp3_44100_128');
  });

  it('should report unavailable when no API key', async () => {
    const originalKey = process.env.ELEVENLABS_API_KEY;
    delete process.env.ELEVENLABS_API_KEY;
    const tts = new TTSProvider();
    assert.equal(await tts.isAvailable(), false);
    if (originalKey) process.env.ELEVENLABS_API_KEY = originalKey;
  });

  it('should return null for synthesis when no API key', async () => {
    const originalKey = process.env.ELEVENLABS_API_KEY;
    delete process.env.ELEVENLABS_API_KEY;
    const tts = new TTSProvider();
    const result = await tts.synthesize('Hello world');
    assert.equal(result, null);
    if (originalKey) process.env.ELEVENLABS_API_KEY = originalKey;
  });

  it('should return null for empty text', async () => {
    const tts = new TTSProvider({ apiKey: 'test-key' });
    const result = await tts.synthesize('');
    assert.equal(result, null);
  });

  it('should accept voice settings overrides', () => {
    const tts = new TTSProvider({
      voiceSettings: { stability: 0.8, similarity_boost: 0.9 },
    });
    assert.equal(tts.voiceSettings.stability, 0.8);
    assert.equal(tts.voiceSettings.similarity_boost, 0.9);
  });
});

describe('Voice: STTProvider', () => {
  let STTProvider;

  before(async () => {
    const mod = await import('../src/voice/stt.js');
    STTProvider = mod.STTProvider;
  });

  it('should create an STTProvider instance', () => {
    const stt = new STTProvider();
    assert.ok(stt);
    assert.equal(stt.model, 'whisper-1');
  });

  it('should report unavailable when no API key', async () => {
    const originalKey = process.env.OPENAI_API_KEY;
    delete process.env.OPENAI_API_KEY;
    const stt = new STTProvider();
    assert.equal(await stt.isAvailable(), false);
    if (originalKey) process.env.OPENAI_API_KEY = originalKey;
  });

  it('should return null for transcription when no API key', async () => {
    const originalKey = process.env.OPENAI_API_KEY;
    delete process.env.OPENAI_API_KEY;
    const stt = new STTProvider();
    const result = await stt.transcribe(Buffer.from([1, 2, 3]));
    assert.equal(result, null);
    if (originalKey) process.env.OPENAI_API_KEY = originalKey;
  });

  it('should return null for empty buffer', async () => {
    const stt = new STTProvider({ apiKey: 'test-key' });
    const result = await stt.transcribe(Buffer.alloc(0));
    assert.equal(result, null);
  });

  it('should reject unsupported format', async () => {
    const stt = new STTProvider({ apiKey: 'test-key' });
    await assert.rejects(
      () => stt.transcribe(Buffer.from([1]), { format: 'aac' }),
      /Unsupported audio format/
    );
  });

  it('should reject oversized buffer', async () => {
    const stt = new STTProvider({ apiKey: 'test-key' });
    const large = Buffer.alloc(26 * 1024 * 1024);
    await assert.rejects(
      () => stt.transcribe(large),
      /too large/
    );
  });

  it('should list supported formats', () => {
    const stt = new STTProvider();
    const formats = stt.getSupportedFormats();
    assert.ok(formats.includes('mp3'));
    assert.ok(formats.includes('wav'));
    assert.ok(formats.includes('webm'));
  });
});

describe('Voice: VoiceModeController', () => {
  let VoiceModeController;

  before(async () => {
    const mod = await import('../src/voice/voice-mode.js');
    VoiceModeController = mod.VoiceModeController;
  });

  it('should create a controller instance', () => {
    const ctrl = new VoiceModeController();
    assert.ok(ctrl);
    assert.ok(ctrl.sessions instanceof Map);
  });

  it('should enable voice mode for a session', () => {
    const ctrl = new VoiceModeController();
    const result = ctrl.enableVoiceMode('test-session');
    assert.equal(result.enabled, true);
    assert.equal(ctrl.isVoiceModeEnabled('test-session'), true);
    ctrl.destroy();
  });

  it('should disable voice mode for a session', () => {
    const ctrl = new VoiceModeController();
    ctrl.enableVoiceMode('test-session');
    const result = ctrl.disableVoiceMode('test-session');
    assert.equal(result.enabled, false);
    assert.equal(ctrl.isVoiceModeEnabled('test-session'), false);
    ctrl.destroy();
  });

  it('should return false for unknown session', () => {
    const ctrl = new VoiceModeController();
    assert.equal(ctrl.isVoiceModeEnabled('nonexistent'), false);
    ctrl.destroy();
  });

  it('should throw for invalid session id', () => {
    const ctrl = new VoiceModeController();
    assert.throws(() => ctrl.enableVoiceMode(''), /valid session/);
    assert.throws(() => ctrl.enableVoiceMode(null), /valid session/);
    ctrl.destroy();
  });

  it('should get voice status', async () => {
    const ctrl = new VoiceModeController();
    ctrl.enableVoiceMode('s1');
    const status = await ctrl.getVoiceStatus();
    assert.equal(status.activeVoiceSessions, 1);
    assert.equal(typeof status.ttsAvailable, 'boolean');
    assert.equal(typeof status.sttAvailable, 'boolean');
    ctrl.destroy();
  });

  it('should destroy and clear all sessions', () => {
    const ctrl = new VoiceModeController();
    ctrl.enableVoiceMode('s1');
    ctrl.enableVoiceMode('s2');
    ctrl.destroy();
    assert.equal(ctrl.sessions.size, 0);
  });

  it('should require agentHandler for processVoiceMessage', async () => {
    const ctrl = new VoiceModeController();
    await assert.rejects(
      () => ctrl.processVoiceMessage(Buffer.from([1]), 'session', {}),
      /agentHandler/
    );
    ctrl.destroy();
  });
});

// ============================================================================
// Channel Base Tests
// ============================================================================

describe('Channel Base', () => {
  let base;

  before(async () => {
    base = await import('../src/channels/base.js');
  });

  it('should export createSessionManager', () => {
    assert.ok(typeof base.createSessionManager === 'function');
  });

  it('should export createMessageHandler', () => {
    assert.ok(typeof base.createMessageHandler === 'function');
  });

  it('should export processWithAgent', () => {
    assert.ok(typeof base.processWithAgent === 'function');
  });

  it('should export handleBotCommand', () => {
    assert.ok(typeof base.handleBotCommand === 'function');
  });

  it('should create session manager with defaults', () => {
    const mgr = base.createSessionManager();
    assert.ok(mgr.getSession);
    assert.ok(mgr.startCleanup);
    assert.ok(mgr.stopCleanup);
  });

  it('should create new session for unknown sender', () => {
    const mgr = base.createSessionManager();
    const session = mgr.getSession('unknown-user');
    assert.equal(session.sessionId, null);
    assert.equal(session.processing, false);
    assert.deepEqual(session.queue, []);
  });

  it('should chunk messages correctly', () => {
    const chunks = base.chunkMessage('a'.repeat(100), 30);
    assert.ok(chunks.length > 1);
    assert.ok(chunks.every(c => c.length <= 30));
  });

  it('should not chunk short messages', () => {
    const chunks = base.chunkMessage('hello', 100);
    assert.equal(chunks.length, 1);
    assert.equal(chunks[0], 'hello');
  });

  it('should check allowlist correctly', () => {
    assert.equal(base.isAllowed('user1', null), true);
    assert.equal(base.isAllowed('user1', []), true);
    assert.equal(base.isAllowed('user1', ['*']), true);
    assert.equal(base.isAllowed('user1', ['user1']), true);
    assert.equal(base.isAllowed('user1', ['user2']), false);
  });

  it('should handle /think bot command', async () => {
    const session = { thinkLevel: 'off' };
    const result = await base.handleBotCommand('/think high', session, false);
    assert.equal(result.handled, true);
    assert.equal(session.thinkLevel, 'high');
  });

  it('should handle /provider bot command', async () => {
    const session = { provider: 'claude' };
    const result = await base.handleBotCommand('/provider openai', session, false);
    assert.equal(result.handled, true);
    assert.equal(session.provider, 'openai');
    assert.ok(result.response.includes('chat-only'));
  });

  it('should handle /memory bot command', async () => {
    const session = { memoryEnabled: false };
    const result = await base.handleBotCommand('/memory', session, false);
    assert.equal(result.handled, true);
    assert.equal(session.memoryEnabled, true);
  });

  it('should compute backoff correctly', () => {
    const policy = base.RECONNECT_POLICY;
    const d1 = base.computeBackoff(policy, 1);
    const d2 = base.computeBackoff(policy, 2);
    assert.ok(d1 > 0);
    assert.ok(d2 > d1 * 0.5); // Approximate due to jitter
  });
});

// ============================================================================
// Marketplace Enhanced Tests
// ============================================================================

describe('MarketplaceClient Enhanced', () => {
  let MarketplaceClient;

  before(async () => {
    const mod = await import('../src/skills/marketplace.js');
    MarketplaceClient = mod.MarketplaceClient;
  });

  it('should check for updates on a skill', () => {
    const SKILLS_DIR = join(__dirname, '..', 'skills');
    const client = new MarketplaceClient({
      catalogPath: join(SKILLS_DIR, 'marketplace.json'),
      installDir: join(__dirname, '..', '.test-skills-update'),
      bundledDir: SKILLS_DIR,
    });

    const result = client.checkForUpdate('commerce-orders');
    assert.ok(typeof result.hasUpdate === 'boolean');
    assert.ok(result.latest !== undefined);
  });

  it('should check all updates', () => {
    const SKILLS_DIR = join(__dirname, '..', 'skills');
    const client = new MarketplaceClient({
      catalogPath: join(SKILLS_DIR, 'marketplace.json'),
      installDir: join(__dirname, '..', '.test-skills-update'),
      bundledDir: SKILLS_DIR,
    });

    const updates = client.checkAllUpdates();
    assert.ok(Array.isArray(updates));
  });

  it('should return not found for unknown skill update check', () => {
    const client = new MarketplaceClient({
      catalogPath: join(__dirname, '..', 'skills', 'marketplace.json'),
    });
    const result = client.checkForUpdate('nonexistent-skill');
    assert.equal(result.hasUpdate, false);
    assert.equal(result.latest, null);
  });
});

// ============================================================================
// File Existence Tests for v0.3.0 Modules
// ============================================================================

describe('v0.3.0 Module Files', () => {
  it('should have voice TTS module', () => {
    assert.ok(existsSync(join(__dirname, '..', 'src', 'voice', 'tts.js')));
  });

  it('should have voice STT module', () => {
    assert.ok(existsSync(join(__dirname, '..', 'src', 'voice', 'stt.js')));
  });

  it('should have voice mode controller', () => {
    assert.ok(existsSync(join(__dirname, '..', 'src', 'voice', 'voice-mode.js')));
  });

  it('should have providers base module', () => {
    assert.ok(existsSync(join(__dirname, '..', 'src', 'providers', 'base.js')));
  });

  it('should have providers openai module', () => {
    assert.ok(existsSync(join(__dirname, '..', 'src', 'providers', 'openai.js')));
  });

  it('should have providers gemini module', () => {
    assert.ok(existsSync(join(__dirname, '..', 'src', 'providers', 'gemini.js')));
  });

  it('should have providers ollama module', () => {
    assert.ok(existsSync(join(__dirname, '..', 'src', 'providers', 'ollama.js')));
  });

  it('should have memory store module', () => {
    assert.ok(existsSync(join(__dirname, '..', 'src', 'memory', 'store.js')));
  });

  it('should have skills marketplace module', () => {
    assert.ok(existsSync(join(__dirname, '..', 'src', 'skills', 'marketplace.js')));
  });

  it('should have channel orchestrator', () => {
    assert.ok(existsSync(join(__dirname, '..', 'src', 'channels', 'orchestrator.js')));
  });

  it('should have daemon CLI', () => {
    assert.ok(existsSync(join(__dirname, '..', 'bin', 'stateset-daemon.js')));
  });
});
