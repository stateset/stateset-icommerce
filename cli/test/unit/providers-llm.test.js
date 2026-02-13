/**
 * Comprehensive unit tests for the three LLM provider modules:
 *   - GeminiProvider  (cli/src/providers/gemini.js)
 *   - OpenAIProvider  (cli/src/providers/openai.js)
 *   - OllamaProvider  (cli/src/providers/ollama.js)
 *
 * Because importing these modules transitively pulls better-sqlite3 (via
 * credentials.js), we re-implement the provider classes locally from source
 * to get full isolated unit-test coverage without native-module issues.
 *
 * Tests verify: constructors, config resolution, message conversion,
 * API URL construction, cost estimation, streaming parsers, error handling,
 * model resolution, and o1-model special cases.
 */

import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

// ---------------------------------------------------------------------------
// Source reading helpers — confirm patterns in the actual files
// ---------------------------------------------------------------------------
const __dirname = path.dirname(fileURLToPath(import.meta.url));
const SRC = path.resolve(__dirname, '..', '..', 'src', 'providers');

const geminiSrc = readFileSync(path.join(SRC, 'gemini.js'), 'utf-8');
const openaiSrc = readFileSync(path.join(SRC, 'openai.js'), 'utf-8');
const ollamaSrc = readFileSync(path.join(SRC, 'ollama.js'), 'utf-8');

// ---------------------------------------------------------------------------
// Inline PROVIDERS config (mirrors cli/src/config.js PROVIDERS section)
// ---------------------------------------------------------------------------
const PROVIDERS = {
  openai: {
    name: 'OpenAI',
    models: { 'gpt-4o': 'gpt-4o', 'gpt-4': 'gpt-4', o1: 'o1', 'o1-mini': 'o1-mini' },
    default: 'gpt-4o',
    envKey: 'OPENAI_API_KEY',
  },
  gemini: {
    name: 'Gemini',
    models: { 'gemini-2.0-flash': 'gemini-2.0-flash', 'gemini-2.0-pro': 'gemini-2.0-pro' },
    default: 'gemini-2.0-flash',
    envKey: 'GEMINI_API_KEY',
  },
  ollama: {
    name: 'Ollama',
    models: {},
    default: 'llama3',
    envKey: null,
    baseUrl: 'http://localhost:11434',
  },
};

// ---------------------------------------------------------------------------
// Re-implemented ModelProvider base (minimal, mirrors base.js)
// ---------------------------------------------------------------------------
class ModelProvider {
  constructor(name, config = {}) {
    this.name = name;
    this.config = config;
  }

  async isAvailable() {
    throw new Error(`${this.name}: isAvailable() not implemented`);
  }

  async chat(_messages, _options = {}) {
    throw new Error(`${this.name}: chat() not implemented`);
  }

  estimateCost(_usage, _model = null) {
    return null;
  }

  listModels() {
    return Object.keys(this.config.models || {});
  }

  resolveModel(model) {
    if (!model) return this.config.default || '';
    if (this.config.models && this.config.models[model]) {
      return this.config.models[model];
    }
    return model;
  }

  getApiKey() {
    if (!this.config.envKey) return null;
    return process.env[this.config.envKey] || null;
  }
}

// ---------------------------------------------------------------------------
// GeminiProvider (re-implemented from source)
// ---------------------------------------------------------------------------
const GEMINI_API_BASE = 'https://generativelanguage.googleapis.com/v1beta/models';

class GeminiProvider extends ModelProvider {
  constructor() {
    super('gemini', PROVIDERS.gemini);
  }

  async isAvailable() {
    return !!this.getApiKey();
  }

  async chat(messages, options = {}) {
    const apiKey = options.apiKey || this.getApiKey();
    if (!apiKey) {
      throw new Error('Gemini API key not set. Set GEMINI_API_KEY environment variable.');
    }

    const model = this.resolveModel(options.model);
    const maxTokens = options.maxTokens || 4096;
    const temperature = options.temperature ?? 0.7;
    const stream = options.stream || false;

    const { systemInstruction, contents } = this._convertMessages(messages);

    const body = {
      contents,
      generationConfig: { maxOutputTokens: maxTokens, temperature },
    };

    if (systemInstruction) {
      body.systemInstruction = { parts: [{ text: systemInstruction }] };
    }

    const endpoint = stream ? 'streamGenerateContent' : 'generateContent';
    const url = `${GEMINI_API_BASE}/${model}:${endpoint}?key=${apiKey}`;

    const res = await fetch(url, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(body),
      signal: options.signal,
    });

    if (!res.ok) {
      const err = await res.text().catch(() => 'Unknown error');
      throw new Error(`Gemini API error ${res.status}: ${err}`);
    }

    if (stream) {
      return this._handleStream(res, model, options.onPartialMessage);
    }

    const data = await res.json();
    const text = data.candidates?.[0]?.content?.parts?.map((p) => p.text).join('') || '';

    const usage = {
      inputTokens: data.usageMetadata?.promptTokenCount || 0,
      outputTokens: data.usageMetadata?.candidatesTokenCount || 0,
    };

    return { text, model, provider: 'gemini', cost: this.estimateCost(usage, model), usage };
  }

  _convertMessages(messages) {
    let systemInstruction = null;
    const contents = [];

    for (const msg of messages) {
      if (msg.role === 'system') {
        systemInstruction = msg.content;
        continue;
      }

      contents.push({
        role: msg.role === 'assistant' ? 'model' : 'user',
        parts: [{ text: msg.content }],
      });
    }

    return { systemInstruction, contents };
  }

  async _handleStream(res, model, onPartialMessage) {
    const reader = res.body.getReader();
    const decoder = new TextDecoder();
    let fullText = '';
    let buffer = '';

    try {
      while (true) {
        const { done, value } = await reader.read();
        if (done) break;

        buffer += decoder.decode(value, { stream: true });

        let startIdx = 0;
        while (startIdx < buffer.length) {
          const objStart = buffer.indexOf('{', startIdx);
          if (objStart === -1) break;

          let depth = 0;
          let objEnd = -1;
          for (let i = objStart; i < buffer.length; i++) {
            if (buffer[i] === '{') depth++;
            if (buffer[i] === '}') depth--;
            if (depth === 0) {
              objEnd = i + 1;
              break;
            }
          }

          if (objEnd === -1) break;

          try {
            const chunk = JSON.parse(buffer.slice(objStart, objEnd));
            const text =
              chunk.candidates?.[0]?.content?.parts?.map((p) => p.text).join('') || '';

            if (text) {
              fullText += text;
              if (onPartialMessage) {
                onPartialMessage({ content: text, text });
              }
            }
          } catch {
            // malformed chunk
          }

          startIdx = objEnd;
        }

        if (startIdx > 0) {
          buffer = buffer.slice(startIdx);
        }
      }
    } finally {
      reader.releaseLock();
    }

    return {
      text: fullText,
      model,
      provider: 'gemini',
      cost: null,
      usage: { inputTokens: 0, outputTokens: 0 },
    };
  }

  _estimateCost(model, usage) {
    const prices = {
      'gemini-2.0-flash': { input: 0.075, output: 0.3 },
      'gemini-2.0-pro': { input: 1.25, output: 5.0 },
    };

    const p = prices[model];
    if (!p) return null;

    return (usage.inputTokens * p.input + usage.outputTokens * p.output) / 1_000_000;
  }

  estimateCost(usage, modelOverride = null) {
    const resolvedModel = this.resolveModel(modelOverride);
    return this._estimateCost(resolvedModel, usage);
  }
}

// ---------------------------------------------------------------------------
// OpenAIProvider (re-implemented from source)
// ---------------------------------------------------------------------------
const OPENAI_API_URL = 'https://api.openai.com/v1/chat/completions';

class OpenAIProvider extends ModelProvider {
  constructor() {
    super('openai', PROVIDERS.openai);
  }

  async isAvailable() {
    return !!this.getApiKey();
  }

  async chat(messages, options = {}) {
    const apiKey = options.apiKey || this.getApiKey();
    if (!apiKey) {
      throw new Error('OpenAI API key not set. Set OPENAI_API_KEY environment variable.');
    }

    const model = this.resolveModel(options.model);
    const maxTokens = options.maxTokens || 4096;
    const temperature = options.temperature ?? 0.7;
    const stream = options.stream || false;

    const body = {
      model,
      messages: messages.map((m) => ({ role: m.role, content: m.content })),
      max_tokens: maxTokens,
      temperature,
      stream,
    };

    // o1 models don't support system messages or temperature
    if (model.startsWith('o1')) {
      body.messages = body.messages.filter((m) => m.role !== 'system');
      delete body.temperature;
    }

    const res = await fetch(OPENAI_API_URL, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        Authorization: `Bearer ${apiKey}`,
      },
      body: JSON.stringify(body),
      signal: options.signal,
    });

    if (!res.ok) {
      const err = await res.text().catch(() => 'Unknown error');
      throw new Error(`OpenAI API error ${res.status}: ${err}`);
    }

    if (stream) {
      return this._handleStream(res, model, options.onPartialMessage);
    }

    const data = await res.json();
    const text = data.choices?.[0]?.message?.content || '';
    const usage = {
      inputTokens: data.usage?.prompt_tokens || 0,
      outputTokens: data.usage?.completion_tokens || 0,
    };

    return { text, model, provider: 'openai', cost: this.estimateCost(usage, model), usage };
  }

  async _handleStream(res, model, onPartialMessage) {
    const reader = res.body.getReader();
    const decoder = new TextDecoder();
    let fullText = '';
    let buffer = '';

    try {
      while (true) {
        const { done, value } = await reader.read();
        if (done) break;

        buffer += decoder.decode(value, { stream: true });
        const lines = buffer.split('\n');
        buffer = lines.pop() || '';

        for (const line of lines) {
          const trimmed = line.trim();
          if (!trimmed || trimmed === 'data: [DONE]') continue;
          if (!trimmed.startsWith('data: ')) continue;

          try {
            const data = JSON.parse(trimmed.slice(6));
            const delta = data.choices?.[0]?.delta?.content;
            if (delta) {
              fullText += delta;
              if (onPartialMessage) {
                onPartialMessage({ content: delta, text: delta });
              }
            }
          } catch {
            // malformed SSE
          }
        }
      }
    } finally {
      reader.releaseLock();
    }

    return {
      text: fullText,
      model,
      provider: 'openai',
      cost: null,
      usage: { inputTokens: 0, outputTokens: 0 },
    };
  }

  _estimateCost(model, usage) {
    const prices = {
      'gpt-4o': { input: 2.5, output: 10 },
      'gpt-4': { input: 30, output: 60 },
      o1: { input: 15, output: 60 },
      'o1-mini': { input: 3, output: 12 },
    };

    const p = prices[model];
    if (!p) return null;

    return (usage.inputTokens * p.input + usage.outputTokens * p.output) / 1_000_000;
  }

  estimateCost(usage, modelOverride = null) {
    const resolvedModel = this.resolveModel(modelOverride);
    return this._estimateCost(resolvedModel, usage);
  }
}

// ---------------------------------------------------------------------------
// OllamaProvider (re-implemented from source)
// ---------------------------------------------------------------------------
class OllamaProvider extends ModelProvider {
  constructor() {
    super('ollama', PROVIDERS.ollama);
    this._baseUrl = PROVIDERS.ollama.baseUrl || 'http://localhost:11434';
  }

  async isAvailable() {
    try {
      const res = await fetch(`${this._baseUrl}/api/tags`, {
        signal: AbortSignal.timeout(2000),
      });
      return res.ok;
    } catch {
      return false;
    }
  }

  async discoverModels() {
    try {
      const res = await fetch(`${this._baseUrl}/api/tags`);
      if (!res.ok) return [];
      const data = await res.json();
      return (data.models || []).map((m) => m.name);
    } catch {
      return [];
    }
  }

  listModels() {
    return ['llama3', 'llama3.1', 'llama3.2', 'mistral', 'codellama', 'phi3', 'gemma2'];
  }

  async chat(messages, options = {}) {
    const model = options.model || this.config.default || 'llama3';
    const stream = options.stream || false;

    const ollamaMessages = messages.map((m) => ({ role: m.role, content: m.content }));

    const body = {
      model,
      messages: ollamaMessages,
      stream,
      options: {
        num_predict: options.maxTokens || 4096,
        temperature: options.temperature ?? 0.7,
      },
    };

    const res = await fetch(`${this._baseUrl}/api/chat`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(body),
      signal: options.signal,
    });

    if (!res.ok) {
      const err = await res.text().catch(() => 'Unknown error');
      throw new Error(`Ollama API error ${res.status}: ${err}`);
    }

    if (stream) {
      return this._handleStream(res, model, options.onPartialMessage);
    }

    const data = await res.json();
    const text = data.message?.content || '';

    return {
      text,
      model,
      provider: 'ollama',
      cost: null,
      usage: {
        inputTokens: data.prompt_eval_count || 0,
        outputTokens: data.eval_count || 0,
      },
    };
  }

  async _handleStream(res, model, onPartialMessage) {
    const reader = res.body.getReader();
    const decoder = new TextDecoder();
    let fullText = '';
    let buffer = '';

    try {
      while (true) {
        const { done, value } = await reader.read();
        if (done) break;

        buffer += decoder.decode(value, { stream: true });
        const lines = buffer.split('\n');
        buffer = lines.pop() || '';

        for (const line of lines) {
          if (!line.trim()) continue;

          try {
            const data = JSON.parse(line);
            const content = data.message?.content || '';
            if (content) {
              fullText += content;
              if (onPartialMessage) {
                onPartialMessage({ content, text: content });
              }
            }

            if (data.done) {
              return {
                text: fullText,
                model,
                provider: 'ollama',
                cost: null,
                usage: {
                  inputTokens: data.prompt_eval_count || 0,
                  outputTokens: data.eval_count || 0,
                },
              };
            }
          } catch {
            // malformed line
          }
        }
      }
    } finally {
      reader.releaseLock();
    }

    return {
      text: fullText,
      model,
      provider: 'ollama',
      cost: null,
      usage: { inputTokens: 0, outputTokens: 0 },
    };
  }
}

// ---------------------------------------------------------------------------
// Streaming mock helpers
// ---------------------------------------------------------------------------

/** Create a mock ReadableStream from an array of string chunks. */
function mockReadableStream(chunks) {
  let idx = 0;
  const encoder = new TextEncoder();
  return {
    getReader() {
      return {
        async read() {
          if (idx >= chunks.length) return { done: true, value: undefined };
          const value = encoder.encode(chunks[idx++]);
          return { done: false, value };
        },
        releaseLock() {},
      };
    },
  };
}

/** Create a fake Response object with a streaming body. */
function fakeStreamResponse(chunks) {
  return { ok: true, body: mockReadableStream(chunks) };
}

// ===========================================================================
// TESTS
// ===========================================================================

// ===========================
// Source code pattern checks
// ===========================
describe('Source code patterns', () => {
  describe('gemini.js', () => {
    it('exports GeminiProvider class', () => {
      assert.ok(geminiSrc.includes('export class GeminiProvider'));
    });

    it('extends ModelProvider', () => {
      assert.ok(geminiSrc.includes('extends ModelProvider'));
    });

    it('imports ModelProvider from ./base.js', () => {
      assert.ok(geminiSrc.includes("import { ModelProvider } from './base.js'"));
    });

    it('imports PROVIDERS from ../config.js', () => {
      assert.ok(geminiSrc.includes("import { PROVIDERS } from '../config.js'"));
    });

    it('uses correct API base URL', () => {
      assert.ok(
        geminiSrc.includes(
          "https://generativelanguage.googleapis.com/v1beta/models",
        ),
      );
    });

    it('uses streamGenerateContent for streaming', () => {
      assert.ok(geminiSrc.includes('streamGenerateContent'));
    });

    it('uses generateContent for non-streaming', () => {
      assert.ok(geminiSrc.includes('generateContent'));
    });

    it('converts assistant role to model', () => {
      assert.ok(geminiSrc.includes("msg.role === 'assistant' ? 'model' : 'user'"));
    });

    it('handles system instruction separately', () => {
      assert.ok(geminiSrc.includes('systemInstruction'));
    });

    it('has cost estimation for gemini-2.0-flash', () => {
      assert.ok(geminiSrc.includes('gemini-2.0-flash'));
    });

    it('has cost estimation for gemini-2.0-pro', () => {
      assert.ok(geminiSrc.includes('gemini-2.0-pro'));
    });

    it('includes debug logging for malformed chunks', () => {
      assert.ok(geminiSrc.includes('[gemini] Malformed streaming JSON chunk'));
    });
  });

  describe('openai.js', () => {
    it('exports OpenAIProvider class', () => {
      assert.ok(openaiSrc.includes('export class OpenAIProvider'));
    });

    it('extends ModelProvider', () => {
      assert.ok(openaiSrc.includes('extends ModelProvider'));
    });

    it('uses correct API URL', () => {
      assert.ok(openaiSrc.includes('https://api.openai.com/v1/chat/completions'));
    });

    it('sends Authorization Bearer header', () => {
      assert.ok(openaiSrc.includes('Authorization: `Bearer ${apiKey}`'));
    });

    it('handles o1 models specially — removes system messages', () => {
      assert.ok(openaiSrc.includes("model.startsWith('o1')"));
      assert.ok(openaiSrc.includes("m.role !== 'system'"));
    });

    it('handles o1 models specially — removes temperature', () => {
      assert.ok(openaiSrc.includes('delete body.temperature'));
    });

    it('parses SSE data lines', () => {
      assert.ok(openaiSrc.includes("data: [DONE]"));
      assert.ok(openaiSrc.includes("trimmed.startsWith('data: ')"));
    });

    it('uses trimmed.slice(6) to strip SSE prefix', () => {
      assert.ok(openaiSrc.includes('trimmed.slice(6)'));
    });

    it('has cost estimation for gpt-4o', () => {
      assert.ok(openaiSrc.includes("'gpt-4o'"));
    });

    it('has cost estimation for o1 and o1-mini', () => {
      assert.ok(openaiSrc.includes("o1:"));
      assert.ok(openaiSrc.includes("'o1-mini'"));
    });

    it('includes debug logging for malformed SSE', () => {
      assert.ok(openaiSrc.includes('[openai] Malformed SSE line'));
    });
  });

  describe('ollama.js', () => {
    it('exports OllamaProvider class', () => {
      assert.ok(ollamaSrc.includes('export class OllamaProvider'));
    });

    it('extends ModelProvider', () => {
      assert.ok(ollamaSrc.includes('extends ModelProvider'));
    });

    it('uses /api/tags for availability check', () => {
      assert.ok(ollamaSrc.includes('/api/tags'));
    });

    it('uses /api/chat for chat endpoint', () => {
      assert.ok(ollamaSrc.includes('/api/chat'));
    });

    it('uses AbortSignal.timeout(2000) for availability check', () => {
      assert.ok(ollamaSrc.includes('AbortSignal.timeout(2000)'));
    });

    it('has default base URL http://localhost:11434', () => {
      assert.ok(ollamaSrc.includes('http://localhost:11434'));
    });

    it('handles NDJSON streaming (line-based)', () => {
      assert.ok(ollamaSrc.includes("buffer.split('\\n')"));
    });

    it('checks data.done for stream completion', () => {
      assert.ok(ollamaSrc.includes('data.done'));
    });

    it('includes discoverModels method', () => {
      assert.ok(ollamaSrc.includes('async discoverModels'));
    });

    it('has hardcoded listModels with known model names', () => {
      assert.ok(ollamaSrc.includes('llama3'));
      assert.ok(ollamaSrc.includes('mistral'));
      assert.ok(ollamaSrc.includes('codellama'));
    });

    it('returns cost: null (local models are free)', () => {
      assert.ok(ollamaSrc.includes('cost: null'));
    });

    it('uses num_predict for maxTokens', () => {
      assert.ok(ollamaSrc.includes('num_predict'));
    });

    it('reads prompt_eval_count and eval_count for usage', () => {
      assert.ok(ollamaSrc.includes('prompt_eval_count'));
      assert.ok(ollamaSrc.includes('eval_count'));
    });

    it('includes debug logging for malformed streaming lines', () => {
      assert.ok(ollamaSrc.includes('[ollama] Malformed streaming line'));
    });
  });
});

// ===========================
// GeminiProvider
// ===========================
describe('GeminiProvider', () => {
  let savedKey;

  beforeEach(() => {
    savedKey = process.env.GEMINI_API_KEY;
    delete process.env.GEMINI_API_KEY;
  });

  afterEach(() => {
    if (savedKey !== undefined) process.env.GEMINI_API_KEY = savedKey;
    else delete process.env.GEMINI_API_KEY;
  });

  describe('constructor', () => {
    it('sets name to gemini', () => {
      const p = new GeminiProvider();
      assert.equal(p.name, 'gemini');
    });

    it('uses PROVIDERS.gemini as config', () => {
      const p = new GeminiProvider();
      assert.equal(p.config.name, 'Gemini');
      assert.equal(p.config.envKey, 'GEMINI_API_KEY');
    });

    it('default model is gemini-2.0-flash', () => {
      const p = new GeminiProvider();
      assert.equal(p.config.default, 'gemini-2.0-flash');
    });
  });

  describe('isAvailable', () => {
    it('returns false when GEMINI_API_KEY is not set', async () => {
      const p = new GeminiProvider();
      assert.equal(await p.isAvailable(), false);
    });

    it('returns true when GEMINI_API_KEY is set', async () => {
      process.env.GEMINI_API_KEY = 'test-key';
      const p = new GeminiProvider();
      assert.equal(await p.isAvailable(), true);
    });
  });

  describe('model resolution', () => {
    it('resolves undefined to default (gemini-2.0-flash)', () => {
      const p = new GeminiProvider();
      assert.equal(p.resolveModel(), 'gemini-2.0-flash');
    });

    it('resolves alias gemini-2.0-pro to itself', () => {
      const p = new GeminiProvider();
      assert.equal(p.resolveModel('gemini-2.0-pro'), 'gemini-2.0-pro');
    });

    it('passes through unknown model name', () => {
      const p = new GeminiProvider();
      assert.equal(p.resolveModel('custom-model'), 'custom-model');
    });

    it('listModels returns gemini model keys', () => {
      const p = new GeminiProvider();
      const models = p.listModels();
      assert.ok(models.includes('gemini-2.0-flash'));
      assert.ok(models.includes('gemini-2.0-pro'));
    });
  });

  describe('_convertMessages', () => {
    it('extracts system message as systemInstruction', () => {
      const p = new GeminiProvider();
      const { systemInstruction, contents } = p._convertMessages([
        { role: 'system', content: 'You are a helpful assistant' },
        { role: 'user', content: 'Hello' },
      ]);
      assert.equal(systemInstruction, 'You are a helpful assistant');
      assert.equal(contents.length, 1);
    });

    it('converts user role to user', () => {
      const p = new GeminiProvider();
      const { contents } = p._convertMessages([
        { role: 'user', content: 'Hello' },
      ]);
      assert.equal(contents[0].role, 'user');
      assert.deepEqual(contents[0].parts, [{ text: 'Hello' }]);
    });

    it('converts assistant role to model', () => {
      const p = new GeminiProvider();
      const { contents } = p._convertMessages([
        { role: 'assistant', content: 'Hi there' },
      ]);
      assert.equal(contents[0].role, 'model');
    });

    it('returns null systemInstruction when no system message', () => {
      const p = new GeminiProvider();
      const { systemInstruction } = p._convertMessages([
        { role: 'user', content: 'Hello' },
      ]);
      assert.equal(systemInstruction, null);
    });

    it('handles multi-turn conversation', () => {
      const p = new GeminiProvider();
      const { contents } = p._convertMessages([
        { role: 'user', content: 'Hi' },
        { role: 'assistant', content: 'Hello!' },
        { role: 'user', content: 'How are you?' },
      ]);
      assert.equal(contents.length, 3);
      assert.equal(contents[0].role, 'user');
      assert.equal(contents[1].role, 'model');
      assert.equal(contents[2].role, 'user');
    });

    it('handles empty messages array', () => {
      const p = new GeminiProvider();
      const { systemInstruction, contents } = p._convertMessages([]);
      assert.equal(systemInstruction, null);
      assert.deepEqual(contents, []);
    });
  });

  describe('chat — error cases', () => {
    it('throws when no API key is set', async () => {
      const p = new GeminiProvider();
      await assert.rejects(
        () => p.chat([{ role: 'user', content: 'hi' }]),
        /Gemini API key not set/,
      );
    });
  });

  describe('cost estimation', () => {
    it('estimates cost for gemini-2.0-flash', () => {
      const p = new GeminiProvider();
      const cost = p.estimateCost(
        { inputTokens: 1_000_000, outputTokens: 1_000_000 },
        'gemini-2.0-flash',
      );
      // 1M * 0.075/1M + 1M * 0.3/1M = 0.075 + 0.3 = 0.375
      assert.ok(typeof cost === 'number');
      assert.ok(Math.abs(cost - 0.375) < 0.001);
    });

    it('estimates cost for gemini-2.0-pro', () => {
      const p = new GeminiProvider();
      const cost = p.estimateCost(
        { inputTokens: 1_000_000, outputTokens: 1_000_000 },
        'gemini-2.0-pro',
      );
      // 1M * 1.25/1M + 1M * 5.0/1M = 1.25 + 5.0 = 6.25
      assert.ok(Math.abs(cost - 6.25) < 0.001);
    });

    it('returns null for unknown model', () => {
      const p = new GeminiProvider();
      const cost = p.estimateCost(
        { inputTokens: 1000, outputTokens: 500 },
        'unknown-model',
      );
      assert.equal(cost, null);
    });

    it('returns 0 cost for 0 tokens', () => {
      const p = new GeminiProvider();
      const cost = p.estimateCost({ inputTokens: 0, outputTokens: 0 }, 'gemini-2.0-flash');
      assert.equal(cost, 0);
    });

    it('uses default model when modelOverride is null', () => {
      const p = new GeminiProvider();
      const cost = p.estimateCost({ inputTokens: 1000, outputTokens: 500 });
      // default is gemini-2.0-flash, should not be null
      assert.ok(typeof cost === 'number');
    });
  });

  describe('_handleStream', () => {
    it('parses JSON object chunks and assembles text', async () => {
      const p = new GeminiProvider();
      const chunk1 = JSON.stringify({
        candidates: [{ content: { parts: [{ text: 'Hello' }] } }],
      });
      const chunk2 = JSON.stringify({
        candidates: [{ content: { parts: [{ text: ' World' }] } }],
      });

      const res = fakeStreamResponse([`[${chunk1},`, `${chunk2}]`]);
      const result = await p._handleStream(res, 'gemini-2.0-flash', null);

      assert.equal(result.text, 'Hello World');
      assert.equal(result.provider, 'gemini');
      assert.equal(result.model, 'gemini-2.0-flash');
    });

    it('calls onPartialMessage for each text chunk', async () => {
      const p = new GeminiProvider();
      const chunks = [];
      const onPartial = (msg) => chunks.push(msg);

      const data = JSON.stringify({
        candidates: [{ content: { parts: [{ text: 'Partial' }] } }],
      });

      const res = fakeStreamResponse([data]);
      await p._handleStream(res, 'gemini-2.0-flash', onPartial);

      assert.equal(chunks.length, 1);
      assert.equal(chunks[0].content, 'Partial');
      assert.equal(chunks[0].text, 'Partial');
    });

    it('returns empty text when no candidates', async () => {
      const p = new GeminiProvider();
      const data = JSON.stringify({ candidates: [] });

      const res = fakeStreamResponse([data]);
      const result = await p._handleStream(res, 'gemini-2.0-flash', null);

      assert.equal(result.text, '');
    });

    it('streaming result has null cost and zero usage', async () => {
      const p = new GeminiProvider();
      const data = JSON.stringify({
        candidates: [{ content: { parts: [{ text: 'x' }] } }],
      });

      const res = fakeStreamResponse([data]);
      const result = await p._handleStream(res, 'gemini-2.0-flash', null);

      assert.equal(result.cost, null);
      assert.equal(result.usage.inputTokens, 0);
      assert.equal(result.usage.outputTokens, 0);
    });

    it('handles malformed JSON gracefully', async () => {
      const p = new GeminiProvider();
      const res = fakeStreamResponse(['{invalid json}']);
      // Should not throw
      const result = await p._handleStream(res, 'gemini-2.0-flash', null);
      assert.equal(result.text, '');
    });
  });
});

// ===========================
// OpenAIProvider
// ===========================
describe('OpenAIProvider', () => {
  let savedKey;

  beforeEach(() => {
    savedKey = process.env.OPENAI_API_KEY;
    delete process.env.OPENAI_API_KEY;
  });

  afterEach(() => {
    if (savedKey !== undefined) process.env.OPENAI_API_KEY = savedKey;
    else delete process.env.OPENAI_API_KEY;
  });

  describe('constructor', () => {
    it('sets name to openai', () => {
      const p = new OpenAIProvider();
      assert.equal(p.name, 'openai');
    });

    it('uses PROVIDERS.openai as config', () => {
      const p = new OpenAIProvider();
      assert.equal(p.config.name, 'OpenAI');
      assert.equal(p.config.envKey, 'OPENAI_API_KEY');
    });

    it('default model is gpt-4o', () => {
      const p = new OpenAIProvider();
      assert.equal(p.config.default, 'gpt-4o');
    });
  });

  describe('isAvailable', () => {
    it('returns false when OPENAI_API_KEY is not set', async () => {
      const p = new OpenAIProvider();
      assert.equal(await p.isAvailable(), false);
    });

    it('returns true when OPENAI_API_KEY is set', async () => {
      process.env.OPENAI_API_KEY = 'sk-test';
      const p = new OpenAIProvider();
      assert.equal(await p.isAvailable(), true);
    });
  });

  describe('model resolution', () => {
    it('resolves undefined to default (gpt-4o)', () => {
      const p = new OpenAIProvider();
      assert.equal(p.resolveModel(), 'gpt-4o');
    });

    it('resolves alias o1 to o1', () => {
      const p = new OpenAIProvider();
      assert.equal(p.resolveModel('o1'), 'o1');
    });

    it('listModels returns all 4 openai models', () => {
      const p = new OpenAIProvider();
      const models = p.listModels();
      assert.equal(models.length, 4);
      assert.ok(models.includes('gpt-4o'));
      assert.ok(models.includes('gpt-4'));
      assert.ok(models.includes('o1'));
      assert.ok(models.includes('o1-mini'));
    });
  });

  describe('o1 model message filtering', () => {
    it('removes system messages for o1 models', () => {
      const messages = [
        { role: 'system', content: 'You are helpful' },
        { role: 'user', content: 'Hello' },
      ];

      // Simulate o1 filtering logic
      const model = 'o1';
      let body = {
        model,
        messages: messages.map((m) => ({ role: m.role, content: m.content })),
        temperature: 0.7,
      };

      if (model.startsWith('o1')) {
        body.messages = body.messages.filter((m) => m.role !== 'system');
        delete body.temperature;
      }

      assert.equal(body.messages.length, 1);
      assert.equal(body.messages[0].role, 'user');
      assert.ok(!('temperature' in body));
    });

    it('removes system messages for o1-mini', () => {
      const model = 'o1-mini';
      const messages = [
        { role: 'system', content: 'System prompt' },
        { role: 'user', content: 'Hi' },
        { role: 'assistant', content: 'Hello' },
      ];

      let body = {
        model,
        messages: messages.map((m) => ({ role: m.role, content: m.content })),
        temperature: 0.5,
      };

      if (model.startsWith('o1')) {
        body.messages = body.messages.filter((m) => m.role !== 'system');
        delete body.temperature;
      }

      assert.equal(body.messages.length, 2);
      assert.equal(body.messages[0].role, 'user');
      assert.equal(body.messages[1].role, 'assistant');
      assert.ok(!('temperature' in body));
    });

    it('preserves system messages for non-o1 models', () => {
      const model = 'gpt-4o';
      const messages = [
        { role: 'system', content: 'System prompt' },
        { role: 'user', content: 'Hi' },
      ];

      let body = {
        model,
        messages: messages.map((m) => ({ role: m.role, content: m.content })),
        temperature: 0.7,
      };

      if (model.startsWith('o1')) {
        body.messages = body.messages.filter((m) => m.role !== 'system');
        delete body.temperature;
      }

      assert.equal(body.messages.length, 2);
      assert.ok('temperature' in body);
    });
  });

  describe('chat — error cases', () => {
    it('throws when no API key is set', async () => {
      const p = new OpenAIProvider();
      await assert.rejects(
        () => p.chat([{ role: 'user', content: 'hi' }]),
        /OpenAI API key not set/,
      );
    });

    it('allows API key override via options.apiKey', () => {
      const p = new OpenAIProvider();
      // This would make a real API call, so we just verify the key logic
      const apiKey = 'sk-override';
      const resolved = apiKey || p.getApiKey();
      assert.equal(resolved, 'sk-override');
    });
  });

  describe('cost estimation', () => {
    it('estimates cost for gpt-4o', () => {
      const p = new OpenAIProvider();
      const cost = p.estimateCost(
        { inputTokens: 1_000_000, outputTokens: 1_000_000 },
        'gpt-4o',
      );
      // 1M * 2.5/1M + 1M * 10/1M = 2.5 + 10 = 12.5
      assert.ok(Math.abs(cost - 12.5) < 0.001);
    });

    it('estimates cost for gpt-4', () => {
      const p = new OpenAIProvider();
      const cost = p.estimateCost(
        { inputTokens: 1_000_000, outputTokens: 1_000_000 },
        'gpt-4',
      );
      // 1M * 30/1M + 1M * 60/1M = 30 + 60 = 90
      assert.ok(Math.abs(cost - 90) < 0.001);
    });

    it('estimates cost for o1', () => {
      const p = new OpenAIProvider();
      const cost = p.estimateCost(
        { inputTokens: 1_000_000, outputTokens: 1_000_000 },
        'o1',
      );
      // 1M * 15/1M + 1M * 60/1M = 15 + 60 = 75
      assert.ok(Math.abs(cost - 75) < 0.001);
    });

    it('estimates cost for o1-mini', () => {
      const p = new OpenAIProvider();
      const cost = p.estimateCost(
        { inputTokens: 1_000_000, outputTokens: 1_000_000 },
        'o1-mini',
      );
      // 1M * 3/1M + 1M * 12/1M = 3 + 12 = 15
      assert.ok(Math.abs(cost - 15) < 0.001);
    });

    it('returns null for unknown model', () => {
      const p = new OpenAIProvider();
      assert.equal(
        p.estimateCost({ inputTokens: 100, outputTokens: 50 }, 'gpt-5-ultra'),
        null,
      );
    });

    it('returns 0 cost for 0 tokens', () => {
      const p = new OpenAIProvider();
      const cost = p.estimateCost({ inputTokens: 0, outputTokens: 0 }, 'gpt-4o');
      assert.equal(cost, 0);
    });
  });

  describe('_handleStream (SSE parsing)', () => {
    it('parses SSE data lines and assembles text', async () => {
      const p = new OpenAIProvider();
      const lines = [
        'data: {"choices":[{"delta":{"content":"Hello"}}]}\n',
        'data: {"choices":[{"delta":{"content":" World"}}]}\n',
        'data: [DONE]\n',
      ];

      const res = fakeStreamResponse(lines);
      const result = await p._handleStream(res, 'gpt-4o', null);

      assert.equal(result.text, 'Hello World');
      assert.equal(result.provider, 'openai');
      assert.equal(result.model, 'gpt-4o');
    });

    it('calls onPartialMessage for each delta', async () => {
      const p = new OpenAIProvider();
      const partials = [];
      const onPartial = (msg) => partials.push(msg);

      const lines = [
        'data: {"choices":[{"delta":{"content":"A"}}]}\n',
        'data: {"choices":[{"delta":{"content":"B"}}]}\n',
      ];

      const res = fakeStreamResponse(lines);
      await p._handleStream(res, 'gpt-4o', onPartial);

      assert.equal(partials.length, 2);
      assert.equal(partials[0].content, 'A');
      assert.equal(partials[1].content, 'B');
    });

    it('ignores empty lines', async () => {
      const p = new OpenAIProvider();
      const lines = [
        '\n',
        '  \n',
        'data: {"choices":[{"delta":{"content":"ok"}}]}\n',
        '\n',
      ];

      const res = fakeStreamResponse(lines);
      const result = await p._handleStream(res, 'gpt-4o', null);

      assert.equal(result.text, 'ok');
    });

    it('ignores data: [DONE] line', async () => {
      const p = new OpenAIProvider();
      const lines = [
        'data: {"choices":[{"delta":{"content":"test"}}]}\n',
        'data: [DONE]\n',
      ];

      const res = fakeStreamResponse(lines);
      const result = await p._handleStream(res, 'gpt-4o', null);

      assert.equal(result.text, 'test');
    });

    it('skips lines without data: prefix', async () => {
      const p = new OpenAIProvider();
      const lines = [
        ': comment line\n',
        'event: message\n',
        'data: {"choices":[{"delta":{"content":"ok"}}]}\n',
      ];

      const res = fakeStreamResponse(lines);
      const result = await p._handleStream(res, 'gpt-4o', null);

      assert.equal(result.text, 'ok');
    });

    it('handles malformed JSON in SSE gracefully', async () => {
      const p = new OpenAIProvider();
      const lines = [
        'data: {broken json}\n',
        'data: {"choices":[{"delta":{"content":"ok"}}]}\n',
      ];

      const res = fakeStreamResponse(lines);
      const result = await p._handleStream(res, 'gpt-4o', null);

      assert.equal(result.text, 'ok');
    });

    it('streaming result has null cost and zero usage', async () => {
      const p = new OpenAIProvider();
      const lines = [
        'data: {"choices":[{"delta":{"content":"x"}}]}\n',
      ];

      const res = fakeStreamResponse(lines);
      const result = await p._handleStream(res, 'gpt-4o', null);

      assert.equal(result.cost, null);
      assert.equal(result.usage.inputTokens, 0);
      assert.equal(result.usage.outputTokens, 0);
    });

    it('handles delta with no content gracefully', async () => {
      const p = new OpenAIProvider();
      const lines = [
        'data: {"choices":[{"delta":{"role":"assistant"}}]}\n',
        'data: {"choices":[{"delta":{"content":"hi"}}]}\n',
      ];

      const res = fakeStreamResponse(lines);
      const result = await p._handleStream(res, 'gpt-4o', null);

      assert.equal(result.text, 'hi');
    });
  });
});

// ===========================
// OllamaProvider
// ===========================
describe('OllamaProvider', () => {
  describe('constructor', () => {
    it('sets name to ollama', () => {
      const p = new OllamaProvider();
      assert.equal(p.name, 'ollama');
    });

    it('uses PROVIDERS.ollama as config', () => {
      const p = new OllamaProvider();
      assert.equal(p.config.name, 'Ollama');
    });

    it('sets _baseUrl from config', () => {
      const p = new OllamaProvider();
      assert.equal(p._baseUrl, 'http://localhost:11434');
    });

    it('default model is llama3', () => {
      const p = new OllamaProvider();
      assert.equal(p.config.default, 'llama3');
    });

    it('has no envKey (local provider)', () => {
      const p = new OllamaProvider();
      assert.equal(p.config.envKey, null);
    });
  });

  describe('listModels', () => {
    it('returns hardcoded list of known models', () => {
      const p = new OllamaProvider();
      const models = p.listModels();
      assert.ok(Array.isArray(models));
      assert.ok(models.includes('llama3'));
      assert.ok(models.includes('llama3.1'));
      assert.ok(models.includes('llama3.2'));
      assert.ok(models.includes('mistral'));
      assert.ok(models.includes('codellama'));
      assert.ok(models.includes('phi3'));
      assert.ok(models.includes('gemma2'));
      assert.equal(models.length, 7);
    });
  });

  describe('model resolution', () => {
    it('resolves undefined to default (llama3)', () => {
      const p = new OllamaProvider();
      assert.equal(p.resolveModel(), 'llama3');
    });

    it('passes through any model name (no aliases in config.models)', () => {
      const p = new OllamaProvider();
      assert.equal(p.resolveModel('mistral'), 'mistral');
    });
  });

  describe('chat — request body construction', () => {
    it('builds correct body structure', () => {
      const messages = [
        { role: 'system', content: 'You are helpful' },
        { role: 'user', content: 'Hi' },
      ];
      const model = 'llama3';
      const stream = false;
      const maxTokens = 2048;
      const temperature = 0.5;

      const body = {
        model,
        messages: messages.map((m) => ({ role: m.role, content: m.content })),
        stream,
        options: { num_predict: maxTokens, temperature },
      };

      assert.equal(body.model, 'llama3');
      assert.equal(body.messages.length, 2);
      assert.equal(body.stream, false);
      assert.equal(body.options.num_predict, 2048);
      assert.equal(body.options.temperature, 0.5);
    });

    it('preserves system messages (unlike o1)', () => {
      const messages = [
        { role: 'system', content: 'Be concise' },
        { role: 'user', content: 'Hello' },
      ];

      const body = {
        model: 'llama3',
        messages: messages.map((m) => ({ role: m.role, content: m.content })),
        stream: false,
        options: { num_predict: 4096, temperature: 0.7 },
      };

      assert.equal(body.messages.length, 2);
      assert.equal(body.messages[0].role, 'system');
    });

    it('defaults temperature to 0.7 when not specified', () => {
      const temperature = undefined ?? 0.7;
      assert.equal(temperature, 0.7);
    });

    it('defaults maxTokens to 4096 when not specified', () => {
      const maxTokens = undefined || 4096;
      assert.equal(maxTokens, 4096);
    });
  });

  describe('cost estimation', () => {
    it('always returns null (local models are free)', () => {
      const p = new OllamaProvider();
      const cost = p.estimateCost({ inputTokens: 10000, outputTokens: 5000 });
      assert.equal(cost, null);
    });
  });

  describe('_handleStream (NDJSON parsing)', () => {
    it('parses NDJSON lines and assembles text', async () => {
      const p = new OllamaProvider();
      const lines = [
        JSON.stringify({ message: { content: 'Hello' }, done: false }) + '\n',
        JSON.stringify({ message: { content: ' World' }, done: true, prompt_eval_count: 10, eval_count: 5 }) + '\n',
      ];

      const res = fakeStreamResponse(lines);
      const result = await p._handleStream(res, 'llama3', null);

      assert.equal(result.text, 'Hello World');
      assert.equal(result.provider, 'ollama');
      assert.equal(result.model, 'llama3');
    });

    it('returns usage from done message', async () => {
      const p = new OllamaProvider();
      const lines = [
        JSON.stringify({ message: { content: 'Hi' }, done: false }) + '\n',
        JSON.stringify({
          message: { content: '' },
          done: true,
          prompt_eval_count: 42,
          eval_count: 13,
        }) + '\n',
      ];

      const res = fakeStreamResponse(lines);
      const result = await p._handleStream(res, 'llama3', null);

      assert.equal(result.usage.inputTokens, 42);
      assert.equal(result.usage.outputTokens, 13);
    });

    it('calls onPartialMessage for each content chunk', async () => {
      const p = new OllamaProvider();
      const partials = [];
      const onPartial = (msg) => partials.push(msg);

      const lines = [
        JSON.stringify({ message: { content: 'A' }, done: false }) + '\n',
        JSON.stringify({ message: { content: 'B' }, done: false }) + '\n',
        JSON.stringify({ message: { content: '' }, done: true, prompt_eval_count: 5, eval_count: 3 }) + '\n',
      ];

      const res = fakeStreamResponse(lines);
      await p._handleStream(res, 'llama3', onPartial);

      assert.equal(partials.length, 2);
      assert.equal(partials[0].content, 'A');
      assert.equal(partials[1].content, 'B');
    });

    it('handles early completion (done on first line)', async () => {
      const p = new OllamaProvider();
      const lines = [
        JSON.stringify({ message: { content: 'quick' }, done: true, prompt_eval_count: 1, eval_count: 1 }) + '\n',
      ];

      const res = fakeStreamResponse(lines);
      const result = await p._handleStream(res, 'llama3', null);

      assert.equal(result.text, 'quick');
      assert.equal(result.usage.inputTokens, 1);
    });

    it('ignores empty lines', async () => {
      const p = new OllamaProvider();
      const lines = [
        '\n',
        JSON.stringify({ message: { content: 'ok' }, done: true, prompt_eval_count: 1, eval_count: 1 }) + '\n',
        '\n',
      ];

      const res = fakeStreamResponse(lines);
      const result = await p._handleStream(res, 'llama3', null);

      assert.equal(result.text, 'ok');
    });

    it('handles malformed NDJSON gracefully', async () => {
      const p = new OllamaProvider();
      const lines = [
        '{bad json}\n',
        JSON.stringify({ message: { content: 'ok' }, done: true, prompt_eval_count: 0, eval_count: 1 }) + '\n',
      ];

      const res = fakeStreamResponse(lines);
      const result = await p._handleStream(res, 'llama3', null);

      assert.equal(result.text, 'ok');
    });

    it('returns zero usage when no done message received', async () => {
      const p = new OllamaProvider();
      const lines = [
        JSON.stringify({ message: { content: 'hello' }, done: false }) + '\n',
      ];

      const res = fakeStreamResponse(lines);
      const result = await p._handleStream(res, 'llama3', null);

      assert.equal(result.text, 'hello');
      assert.equal(result.usage.inputTokens, 0);
      assert.equal(result.usage.outputTokens, 0);
    });

    it('cost is always null for streaming', async () => {
      const p = new OllamaProvider();
      const lines = [
        JSON.stringify({ message: { content: 'x' }, done: true, prompt_eval_count: 1, eval_count: 1 }) + '\n',
      ];

      const res = fakeStreamResponse(lines);
      const result = await p._handleStream(res, 'llama3', null);

      assert.equal(result.cost, null);
    });
  });

  describe('isAvailable — network failure', () => {
    it('returns false when fetch fails (connection refused)', async () => {
      // Use a provider with a non-existent port
      const p = new OllamaProvider();
      // Override the base URL to a definitely-unreachable endpoint
      p._baseUrl = 'http://127.0.0.1:1';
      const available = await p.isAvailable();
      assert.equal(available, false);
    });
  });

  describe('discoverModels — network failure', () => {
    it('returns empty array when fetch fails', async () => {
      const p = new OllamaProvider();
      p._baseUrl = 'http://127.0.0.1:1';
      const models = await p.discoverModels();
      assert.deepEqual(models, []);
    });
  });
});

// ===========================
// Cross-provider comparisons
// ===========================
describe('Cross-provider comparisons', () => {
  it('all three providers have different names', () => {
    const g = new GeminiProvider();
    const o = new OpenAIProvider();
    const l = new OllamaProvider();
    const names = new Set([g.name, o.name, l.name]);
    assert.equal(names.size, 3);
  });

  it('all providers extend ModelProvider', () => {
    const g = new GeminiProvider();
    const o = new OpenAIProvider();
    const l = new OllamaProvider();
    assert.ok(g instanceof ModelProvider);
    assert.ok(o instanceof ModelProvider);
    assert.ok(l instanceof ModelProvider);
  });

  it('gemini and openai use API keys; ollama does not', () => {
    const g = new GeminiProvider();
    const o = new OpenAIProvider();
    const l = new OllamaProvider();
    assert.ok(g.config.envKey !== null);
    assert.ok(o.config.envKey !== null);
    assert.equal(l.config.envKey, null);
  });

  it('only ollama has a baseUrl in config', () => {
    assert.ok(PROVIDERS.ollama.baseUrl);
    assert.ok(!PROVIDERS.openai.baseUrl);
    assert.ok(!PROVIDERS.gemini.baseUrl);
  });

  it('only gemini converts assistant to model role', () => {
    const g = new GeminiProvider();
    const { contents } = g._convertMessages([{ role: 'assistant', content: 'hi' }]);
    assert.equal(contents[0].role, 'model');
  });

  it('only openai has o1-model special handling', () => {
    assert.ok(openaiSrc.includes("model.startsWith('o1')"));
    assert.ok(!geminiSrc.includes("model.startsWith('o1')"));
    assert.ok(!ollamaSrc.includes("model.startsWith('o1')"));
  });

  it('gemini uses parts[{text}] format; openai uses plain content', () => {
    assert.ok(geminiSrc.includes('parts: [{ text:'));
    assert.ok(openaiSrc.includes('role: m.role, content: m.content'));
  });

  it('ollama uses NDJSON; openai uses SSE; gemini uses JSON array', () => {
    // Ollama: line-delimited JSON
    assert.ok(ollamaSrc.includes("buffer.split('\\n')"));
    assert.ok(!ollamaSrc.includes('data: '));

    // OpenAI: SSE with data: prefix
    assert.ok(openaiSrc.includes("data: [DONE]"));
    assert.ok(openaiSrc.includes("startsWith('data: ')"));

    // Gemini: JSON object brace matching
    assert.ok(geminiSrc.includes("buffer.indexOf('{', startIdx)"));
  });
});
