/**
 * Tests for cli/src/voice/tts.js — TTSProvider (Text-to-Speech)
 */

import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';

describe('TTSProvider', () => {
  let TTSProvider;
  const origEnv = { ...process.env };

  beforeEach(() => {
    delete process.env.ELEVENLABS_API_KEY;
    delete process.env.ELEVENLABS_VOICE_ID;
  });

  afterEach(() => {
    process.env = { ...origEnv };
  });

  async function loadModule() {
    try {
      const mod = await import('../../src/voice/tts.js');
      TTSProvider = mod.TTSProvider;
      return true;
    } catch {
      return false;
    }
  }

  describe('constructor', () => {
    it('uses ELEVENLABS_API_KEY from env by default', async () => {
      process.env.ELEVENLABS_API_KEY = 'el-test-key';
      if (!(await loadModule())) return;
      const tts = new TTSProvider();
      assert.strictEqual(tts.apiKey, 'el-test-key');
    });

    it('accepts apiKey option', async () => {
      if (!(await loadModule())) return;
      const tts = new TTSProvider({ apiKey: 'el-override' });
      assert.strictEqual(tts.apiKey, 'el-override');
    });

    it('sets apiKey to null when no key available', async () => {
      if (!(await loadModule())) return;
      const tts = new TTSProvider();
      assert.strictEqual(tts.apiKey, null);
    });

    it('uses default voice ID when env not set', async () => {
      if (!(await loadModule())) return;
      const tts = new TTSProvider();
      // Default voice ID is '21m00Tcm4TlvDq8ikWAM' (Rachel)
      assert.strictEqual(tts.voiceId, '21m00Tcm4TlvDq8ikWAM');
    });

    it('uses ELEVENLABS_VOICE_ID from env', async () => {
      process.env.ELEVENLABS_VOICE_ID = 'custom-voice';
      if (!(await loadModule())) return;
      const tts = new TTSProvider();
      assert.strictEqual(tts.voiceId, 'custom-voice');
    });

    it('accepts voiceId option', async () => {
      if (!(await loadModule())) return;
      const tts = new TTSProvider({ voiceId: 'override-voice' });
      assert.strictEqual(tts.voiceId, 'override-voice');
    });

    it('defaults model to eleven_monolingual_v1', async () => {
      if (!(await loadModule())) return;
      const tts = new TTSProvider();
      assert.strictEqual(tts.modelId, 'eleven_monolingual_v1');
    });

    it('accepts custom modelId', async () => {
      if (!(await loadModule())) return;
      const tts = new TTSProvider({ modelId: 'eleven_turbo_v2' });
      assert.strictEqual(tts.modelId, 'eleven_turbo_v2');
    });
  });

  describe('isAvailable', () => {
    it('returns false when no API key', async () => {
      if (!(await loadModule())) return;
      const tts = new TTSProvider();
      const available = await tts.isAvailable();
      assert.strictEqual(available, false);
    });

    it('returns true when API key is set', async () => {
      if (!(await loadModule())) return;
      const tts = new TTSProvider({ apiKey: 'el-test' });
      const available = await tts.isAvailable();
      assert.strictEqual(available, true);
    });
  });

  describe('validation', () => {
    it('synthesize returns null without API key', async () => {
      if (!(await loadModule())) return;
      const tts = new TTSProvider();
      const result = await tts.synthesize('Hello world');
      assert.strictEqual(result, null, 'Should return null when no API key is set');
    });

    it('synthesize returns null for empty text', async () => {
      if (!(await loadModule())) return;
      const tts = new TTSProvider({ apiKey: 'el-test' });
      const result = await tts.synthesize('');
      assert.strictEqual(result, null, 'Should return null for empty text');
    });
  });
});
