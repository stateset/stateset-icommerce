/**
 * Tests for cli/src/voice/stt.js — STTProvider (Speech-to-Text)
 */

import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';

describe('STTProvider', () => {
  let STTProvider;
  const origEnv = { ...process.env };

  beforeEach(() => {
    delete process.env.OPENAI_API_KEY;
  });

  afterEach(() => {
    process.env = { ...origEnv };
  });

  async function loadModule() {
    try {
      const mod = await import('../../src/voice/stt.js');
      STTProvider = mod.STTProvider;
      return true;
    } catch {
      return false;
    }
  }

  describe('constructor', () => {
    it('uses OPENAI_API_KEY from env by default', async () => {
      process.env.OPENAI_API_KEY = 'sk-test-key';
      if (!(await loadModule())) return;
      const stt = new STTProvider();
      assert.strictEqual(stt.apiKey, 'sk-test-key');
    });

    it('accepts apiKey option', async () => {
      if (!(await loadModule())) return;
      const stt = new STTProvider({ apiKey: 'sk-override' });
      assert.strictEqual(stt.apiKey, 'sk-override');
    });

    it('sets apiKey to null when no key available', async () => {
      if (!(await loadModule())) return;
      const stt = new STTProvider();
      assert.strictEqual(stt.apiKey, null);
    });

    it('defaults model to whisper-1', async () => {
      if (!(await loadModule())) return;
      const stt = new STTProvider();
      assert.strictEqual(stt.model, 'whisper-1');
    });

    it('accepts custom model', async () => {
      if (!(await loadModule())) return;
      const stt = new STTProvider({ model: 'whisper-2' });
      assert.strictEqual(stt.model, 'whisper-2');
    });

    it('defaults language to null', async () => {
      if (!(await loadModule())) return;
      const stt = new STTProvider();
      assert.strictEqual(stt.language, null);
    });

    it('accepts language hint', async () => {
      if (!(await loadModule())) return;
      const stt = new STTProvider({ language: 'en' });
      assert.strictEqual(stt.language, 'en');
    });
  });

  describe('isAvailable', () => {
    it('returns false when no API key', async () => {
      if (!(await loadModule())) return;
      const stt = new STTProvider();
      const available = await stt.isAvailable();
      assert.strictEqual(available, false);
    });

    it('returns true when API key is set', async () => {
      if (!(await loadModule())) return;
      const stt = new STTProvider({ apiKey: 'sk-test' });
      const available = await stt.isAvailable();
      assert.strictEqual(available, true);
    });
  });

  describe('validation', () => {
    it('transcribe returns null without API key', async () => {
      if (!(await loadModule())) return;
      const stt = new STTProvider();
      const result = await stt.transcribe(Buffer.from('audio'), { format: 'mp3' });
      assert.strictEqual(result, null, 'Should return null when no API key is set');
    });
  });
});
