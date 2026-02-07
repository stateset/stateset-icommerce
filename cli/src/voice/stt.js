/**
 * Speech-to-Text Provider for StateSet iCommerce
 *
 * Integrates with the OpenAI Whisper API for accurate speech recognition.
 * Falls back gracefully (returns null) when no API key is configured,
 * allowing callers to degrade to text-input-only mode.
 *
 * Requires: OPENAI_API_KEY environment variable
 */

import { createLogger } from '../logger.js';

// ============================================================================
// Constants
// ============================================================================

const OPENAI_TRANSCRIPTIONS_URL = 'https://api.openai.com/v1/audio/transcriptions';

/** Default Whisper model. */
const DEFAULT_MODEL = 'whisper-1';

/** Supported audio formats and their MIME types. */
const SUPPORTED_FORMATS = {
  mp3: 'audio/mpeg',
  wav: 'audio/wav',
  m4a: 'audio/mp4',
  ogg: 'audio/ogg',
  webm: 'audio/webm',
};

/** Maximum file size for Whisper API (25 MB). */
const MAX_FILE_SIZE = 25 * 1024 * 1024;

// ============================================================================
// STTProvider
// ============================================================================

/**
 * STTProvider - OpenAI Whisper Speech-to-Text integration.
 *
 * Usage:
 *   const stt = new STTProvider();
 *   if (await stt.isAvailable()) {
 *     const result = await stt.transcribe(audioBuffer, { format: "mp3" });
 *     console.log(result.text);
 *   }
 */
export class STTProvider {
  /**
   * @param {Object} [options]
   * @param {string} [options.apiKey]   - OpenAI API key (defaults to env)
   * @param {string} [options.model]    - Whisper model name
   * @param {string} [options.language] - ISO-639-1 language hint (e.g. "en")
   */
  constructor(options = {}) {
    this.apiKey = options.apiKey || process.env.OPENAI_API_KEY || null;
    this.model = options.model || DEFAULT_MODEL;
    this.language = options.language || null;
    this.log = createLogger({ level: process.env.LOG_LEVEL || 'info' }).child({ module: 'stt' });
  }

  // --------------------------------------------------------------------------
  // Public API
  // --------------------------------------------------------------------------

  /**
   * Check whether the STT provider is available (API key is configured).
   * @returns {Promise<boolean>}
   */
  async isAvailable() {
    return !!this.apiKey;
  }

  /**
   * Get the list of supported audio formats.
   * @returns {string[]}
   */
  getSupportedFormats() {
    return Object.keys(SUPPORTED_FORMATS);
  }

  /**
   * Transcribe an audio buffer to text.
   *
   * @param {Buffer} audioBuffer - Raw audio data.
   * @param {Object} [opts]
   * @param {string} [opts.format="mp3"]    - Audio format (mp3, wav, m4a, ogg, webm).
   * @param {string} [opts.language]         - ISO-639-1 language hint (e.g. "en").
   * @param {string} [opts.prompt]           - Optional context prompt to guide transcription.
   * @param {string} [opts.model]            - Override model for this call.
   * @param {number} [opts.temperature=0]    - Sampling temperature (0 = deterministic).
   * @returns {Promise<{ text: string, language: string | null, duration: number | null } | null>}
   *   Transcription result, or null if unavailable.
   */
  async transcribe(audioBuffer, opts = {}) {
    if (!this.apiKey) {
      this.log.debug('STT unavailable: no OPENAI_API_KEY configured');
      return null;
    }

    if (!audioBuffer || !Buffer.isBuffer(audioBuffer) || audioBuffer.length === 0) {
      this.log.warn('STT transcribe called with empty or invalid audio buffer');
      return null;
    }

    const format = opts.format || 'mp3';
    if (!SUPPORTED_FORMATS[format]) {
      throw new Error(
        `Unsupported audio format: "${format}". Supported: ${Object.keys(SUPPORTED_FORMATS).join(', ')}`,
      );
    }

    if (audioBuffer.length > MAX_FILE_SIZE) {
      throw new Error(
        `Audio buffer too large (${(audioBuffer.length / 1024 / 1024).toFixed(1)} MB). Maximum: 25 MB.`,
      );
    }

    const model = opts.model || this.model;
    const language = opts.language || this.language;
    const temperature = opts.temperature ?? 0;

    this.log.debug('Transcribing audio', {
      format,
      model,
      language,
      bufferSize: audioBuffer.length,
    });

    try {
      // Build multipart/form-data manually using the Blob/FormData API
      // available in Node.js 18+ (native fetch)
      const formData = new FormData();

      const blob = new Blob([audioBuffer], { type: SUPPORTED_FORMATS[format] });
      formData.append('file', blob, `audio.${format}`);
      formData.append('model', model);
      formData.append('response_format', 'verbose_json');
      formData.append('temperature', String(temperature));

      if (language) {
        formData.append('language', language);
      }
      if (opts.prompt) {
        formData.append('prompt', opts.prompt);
      }

      const res = await fetch(OPENAI_TRANSCRIPTIONS_URL, {
        method: 'POST',
        headers: {
          Authorization: `Bearer ${this.apiKey}`,
        },
        body: formData,
      });

      if (!res.ok) {
        const errBody = await res.text().catch(() => 'Unknown error');
        this.log.error('OpenAI Whisper API error', {
          status: res.status,
          statusText: res.statusText,
          body: errBody.slice(0, 500),
        });
        throw new Error(`OpenAI Whisper API error ${res.status}: ${errBody.slice(0, 200)}`);
      }

      const data = await res.json();

      const result = {
        text: data.text || '',
        language: data.language || language || null,
        duration: data.duration ?? null,
      };

      this.log.info('Audio transcribed', {
        textLength: result.text.length,
        language: result.language,
        duration: result.duration,
        model,
      });

      return result;
    } catch (err) {
      if (err.message.startsWith('OpenAI Whisper API error')) {
        throw err;
      }
      this.log.error('STT transcription failed', { error: err.message });
      throw new Error(`STT transcription failed: ${err.message}`);
    }
  }
}

// ============================================================================
// Singleton
// ============================================================================

/** @type {STTProvider | null} */
let _singleton = null;

/**
 * Get the singleton STTProvider instance.
 * @param {Object} [options] - Passed to constructor on first call only.
 * @returns {STTProvider}
 */
export function getSTTProvider(options) {
  if (!_singleton) {
    _singleton = new STTProvider(options);
  }
  return _singleton;
}

export default STTProvider;
