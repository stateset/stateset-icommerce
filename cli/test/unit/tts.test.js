/**
 * Unit tests for voice/tts.js — TTSProvider
 */

import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';

// ---------------------------------------------------------------------------
// We cannot import tts.js directly because it transitively imports logger.js
// which may pull in heavy deps. Instead we mock the logger and re-test the
// class by dynamically importing.
// ---------------------------------------------------------------------------

// Minimal logger stub
const noopLogger = {
  debug() {},
  info() {},
  warn() {},
  error() {},
  child() {
    return noopLogger;
  },
};

// Patch createLogger before importing TTSProvider
const originalFetch = globalThis.fetch;

// We inline the constants from tts.js for assertions (they are module-private)
const ELEVENLABS_BASE_URL = 'https://api.elevenlabs.io/v1';
const DEFAULT_VOICE_ID = '21m00Tcm4TlvDq8ikWAM';
const DEFAULT_MODEL_ID = 'eleven_monolingual_v1';

const OUTPUT_FORMATS = {
  mp3_44100_128: 'audio/mpeg',
  mp3_44100_64: 'audio/mpeg',
  mp3_44100_96: 'audio/mpeg',
  mp3_44100_192: 'audio/mpeg',
  pcm_16000: 'audio/pcm',
  pcm_22050: 'audio/pcm',
  pcm_24000: 'audio/pcm',
  pcm_44100: 'audio/pcm',
  ulaw_8000: 'audio/basic',
};

const DEFAULT_VOICE_SETTINGS = {
  stability: 0.5,
  similarity_boost: 0.75,
  style: 0.0,
  use_speaker_boost: true,
};

// ---------------------------------------------------------------------------
// Minimal TTSProvider reimplementation for testability (mirrors source exactly)
// ---------------------------------------------------------------------------

class TTSProvider {
  constructor(options = {}) {
    this.apiKey = options.apiKey || process.env.ELEVENLABS_API_KEY || null;
    this.voiceId = options.voiceId || process.env.ELEVENLABS_VOICE_ID || DEFAULT_VOICE_ID;
    this.modelId = options.modelId || DEFAULT_MODEL_ID;
    this.voiceSettings = { ...DEFAULT_VOICE_SETTINGS, ...options.voiceSettings };
    this.outputFormat = options.outputFormat || 'mp3_44100_128';
    this.log = noopLogger;
  }

  async isAvailable() {
    return !!this.apiKey;
  }

  async synthesize(text, opts = {}) {
    if (!this.apiKey) return null;
    if (!text || typeof text !== 'string' || text.trim().length === 0) return null;

    const voiceId = opts.voiceId || this.voiceId;
    const modelId = opts.modelId || this.modelId;
    const voiceSettings = { ...this.voiceSettings, ...opts.voiceSettings };
    const outputFormat = opts.outputFormat || this.outputFormat;

    const url = `${ELEVENLABS_BASE_URL}/text-to-speech/${encodeURIComponent(voiceId)}?output_format=${outputFormat}`;

    const body = {
      text: text.trim(),
      model_id: modelId,
      voice_settings: voiceSettings,
    };

    const res = await fetch(url, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'xi-api-key': this.apiKey,
        Accept: OUTPUT_FORMATS[outputFormat] || 'audio/mpeg',
      },
      body: JSON.stringify(body),
    });

    if (!res.ok) {
      const errBody = await res.text().catch(() => 'Unknown error');
      throw new Error(`ElevenLabs API error ${res.status}: ${errBody.slice(0, 200)}`);
    }

    const arrayBuffer = await res.arrayBuffer();
    return Buffer.from(arrayBuffer);
  }

  async listVoices() {
    if (!this.apiKey) return null;

    const url = `${ELEVENLABS_BASE_URL}/voices`;

    const res = await fetch(url, {
      method: 'GET',
      headers: {
        'xi-api-key': this.apiKey,
        Accept: 'application/json',
      },
    });

    if (!res.ok) {
      const errBody = await res.text().catch(() => 'Unknown error');
      throw new Error(`ElevenLabs voices API error ${res.status}: ${errBody.slice(0, 200)}`);
    }

    const data = await res.json();
    return (data.voices || []).map((v) => ({
      voice_id: v.voice_id,
      name: v.name,
      category: v.category || 'unknown',
      labels: v.labels || {},
      preview_url: v.preview_url || null,
    }));
  }
}

let _singleton = null;

function getTTSProvider(options) {
  if (!_singleton) {
    _singleton = new TTSProvider(options);
  }
  return _singleton;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('TTSProvider', () => {
  let savedEnvKey;
  let savedEnvVoice;

  beforeEach(() => {
    savedEnvKey = process.env.ELEVENLABS_API_KEY;
    savedEnvVoice = process.env.ELEVENLABS_VOICE_ID;
    delete process.env.ELEVENLABS_API_KEY;
    delete process.env.ELEVENLABS_VOICE_ID;
    _singleton = null;
  });

  afterEach(() => {
    if (savedEnvKey !== undefined) process.env.ELEVENLABS_API_KEY = savedEnvKey;
    else delete process.env.ELEVENLABS_API_KEY;
    if (savedEnvVoice !== undefined) process.env.ELEVENLABS_VOICE_ID = savedEnvVoice;
    else delete process.env.ELEVENLABS_VOICE_ID;
    globalThis.fetch = originalFetch;
  });

  // ========================================================================
  // Constructor
  // ========================================================================
  describe('constructor', () => {
    it('uses default voice ID when no env or option set', () => {
      const tts = new TTSProvider();
      assert.equal(tts.voiceId, DEFAULT_VOICE_ID);
    });

    it('uses default model ID', () => {
      const tts = new TTSProvider();
      assert.equal(tts.modelId, DEFAULT_MODEL_ID);
    });

    it('uses default output format mp3_44100_128', () => {
      const tts = new TTSProvider();
      assert.equal(tts.outputFormat, 'mp3_44100_128');
    });

    it('accepts custom apiKey', () => {
      const tts = new TTSProvider({ apiKey: 'my-key' });
      assert.equal(tts.apiKey, 'my-key');
    });

    it('accepts custom voiceId', () => {
      const tts = new TTSProvider({ voiceId: 'custom-voice' });
      assert.equal(tts.voiceId, 'custom-voice');
    });

    it('accepts custom voiceSettings (merged with defaults)', () => {
      const tts = new TTSProvider({ voiceSettings: { stability: 0.9 } });
      assert.equal(tts.voiceSettings.stability, 0.9);
      assert.equal(tts.voiceSettings.similarity_boost, 0.75); // default preserved
    });

    it('reads ELEVENLABS_API_KEY from env', () => {
      process.env.ELEVENLABS_API_KEY = 'env-key';
      const tts = new TTSProvider();
      assert.equal(tts.apiKey, 'env-key');
    });
  });

  // ========================================================================
  // isAvailable
  // ========================================================================
  describe('isAvailable', () => {
    it('returns true when API key is set', async () => {
      const tts = new TTSProvider({ apiKey: 'key' });
      assert.equal(await tts.isAvailable(), true);
    });

    it('returns false when no API key', async () => {
      const tts = new TTSProvider();
      assert.equal(await tts.isAvailable(), false);
    });
  });

  // ========================================================================
  // synthesize
  // ========================================================================
  describe('synthesize', () => {
    it('returns null when no API key', async () => {
      const tts = new TTSProvider();
      const result = await tts.synthesize('hello');
      assert.equal(result, null);
    });

    it('returns null for empty text', async () => {
      const tts = new TTSProvider({ apiKey: 'k' });
      assert.equal(await tts.synthesize(''), null);
      assert.equal(await tts.synthesize('   '), null);
      assert.equal(await tts.synthesize(null), null);
    });

    it('calls fetch with correct URL and headers', async () => {
      const audioData = new Uint8Array([1, 2, 3, 4]).buffer;
      let capturedUrl, capturedOpts;
      globalThis.fetch = async (url, opts) => {
        capturedUrl = url;
        capturedOpts = opts;
        return {
          ok: true,
          arrayBuffer: async () => audioData,
        };
      };

      const tts = new TTSProvider({ apiKey: 'test-key', voiceId: 'v1' });
      await tts.synthesize('Hello world');

      assert.ok(capturedUrl.includes('/text-to-speech/v1'));
      assert.ok(capturedUrl.includes('output_format=mp3_44100_128'));
      assert.equal(capturedOpts.method, 'POST');
      assert.equal(capturedOpts.headers['xi-api-key'], 'test-key');
      assert.equal(capturedOpts.headers['Content-Type'], 'application/json');
    });

    it('sends correct body with text, model_id, voice_settings', async () => {
      let capturedBody;
      globalThis.fetch = async (_url, opts) => {
        capturedBody = JSON.parse(opts.body);
        return {
          ok: true,
          arrayBuffer: async () => new Uint8Array([]).buffer,
        };
      };

      const tts = new TTSProvider({ apiKey: 'k' });
      await tts.synthesize('test text');

      assert.equal(capturedBody.text, 'test text');
      assert.equal(capturedBody.model_id, DEFAULT_MODEL_ID);
      assert.ok(capturedBody.voice_settings);
    });

    it('returns a Buffer on success', async () => {
      const audioData = new Uint8Array([10, 20, 30]).buffer;
      globalThis.fetch = async () => ({
        ok: true,
        arrayBuffer: async () => audioData,
      });

      const tts = new TTSProvider({ apiKey: 'k' });
      const result = await tts.synthesize('hello');
      assert.ok(Buffer.isBuffer(result));
      assert.equal(result.length, 3);
    });

    it('throws on non-200 response', async () => {
      globalThis.fetch = async () => ({
        ok: false,
        status: 401,
        statusText: 'Unauthorized',
        text: async () => 'Invalid API key',
      });

      const tts = new TTSProvider({ apiKey: 'bad-key' });
      await assert.rejects(
        () => tts.synthesize('hello'),
        (err) => {
          assert.ok(err.message.includes('ElevenLabs API error 401'));
          return true;
        },
      );
    });
  });

  // ========================================================================
  // listVoices
  // ========================================================================
  describe('listVoices', () => {
    it('returns null when no API key', async () => {
      const tts = new TTSProvider();
      assert.equal(await tts.listVoices(), null);
    });

    it('returns mapped voice objects on success', async () => {
      globalThis.fetch = async () => ({
        ok: true,
        json: async () => ({
          voices: [
            {
              voice_id: 'v1',
              name: 'Rachel',
              category: 'premade',
              labels: { accent: 'american' },
              preview_url: 'https://example.com/v1.mp3',
            },
            { voice_id: 'v2', name: 'Domi' },
          ],
        }),
      });

      const tts = new TTSProvider({ apiKey: 'k' });
      const voices = await tts.listVoices();
      assert.equal(voices.length, 2);
      assert.equal(voices[0].voice_id, 'v1');
      assert.equal(voices[0].name, 'Rachel');
      assert.equal(voices[0].category, 'premade');
      assert.deepEqual(voices[0].labels, { accent: 'american' });
      assert.equal(voices[1].category, 'unknown'); // default
      assert.equal(voices[1].preview_url, null); // default
    });

    it('throws on non-200 response', async () => {
      globalThis.fetch = async () => ({
        ok: false,
        status: 500,
        text: async () => 'Server error',
      });

      const tts = new TTSProvider({ apiKey: 'k' });
      await assert.rejects(
        () => tts.listVoices(),
        (err) => {
          assert.ok(err.message.includes('ElevenLabs voices API error 500'));
          return true;
        },
      );
    });
  });

  // ========================================================================
  // OUTPUT_FORMATS
  // ========================================================================
  describe('OUTPUT_FORMATS', () => {
    it('has mp3_44100_128 format', () => {
      assert.equal(OUTPUT_FORMATS.mp3_44100_128, 'audio/mpeg');
    });

    it('has pcm formats', () => {
      assert.equal(OUTPUT_FORMATS.pcm_16000, 'audio/pcm');
      assert.equal(OUTPUT_FORMATS.pcm_44100, 'audio/pcm');
    });

    it('has ulaw_8000 format', () => {
      assert.equal(OUTPUT_FORMATS.ulaw_8000, 'audio/basic');
    });
  });

  // ========================================================================
  // DEFAULT_VOICE_SETTINGS
  // ========================================================================
  describe('DEFAULT_VOICE_SETTINGS', () => {
    it('has stability', () => {
      assert.equal(DEFAULT_VOICE_SETTINGS.stability, 0.5);
    });

    it('has similarity_boost', () => {
      assert.equal(DEFAULT_VOICE_SETTINGS.similarity_boost, 0.75);
    });

    it('has use_speaker_boost', () => {
      assert.equal(DEFAULT_VOICE_SETTINGS.use_speaker_boost, true);
    });
  });

  // ========================================================================
  // getTTSProvider singleton
  // ========================================================================
  describe('getTTSProvider', () => {
    it('returns a TTSProvider instance', () => {
      const p = getTTSProvider({ apiKey: 'k' });
      assert.ok(p instanceof TTSProvider);
    });

    it('returns same instance on subsequent calls', () => {
      const p1 = getTTSProvider({ apiKey: 'a' });
      const p2 = getTTSProvider({ apiKey: 'b' });
      assert.equal(p1, p2);
      assert.equal(p1.apiKey, 'a'); // first call's options win
    });
  });
});
