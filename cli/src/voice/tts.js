/**
 * Text-to-Speech Provider for StateSet iCommerce
 *
 * Integrates with the ElevenLabs API for high-quality voice synthesis.
 * Falls back gracefully (returns null) when no API key is configured,
 * allowing callers to degrade to text-only mode.
 *
 * Requires: ELEVENLABS_API_KEY environment variable
 * Optional: ELEVENLABS_VOICE_ID for default voice selection
 */

import { createLogger } from "../logger.js";

// ============================================================================
// Constants
// ============================================================================

const ELEVENLABS_BASE_URL = "https://api.elevenlabs.io/v1";

/** Default voice ID used when ELEVENLABS_VOICE_ID is not set (Rachel). */
const DEFAULT_VOICE_ID = "21m00Tcm4TlvDq8ikWAM";

/** Default model for synthesis. */
const DEFAULT_MODEL_ID = "eleven_monolingual_v1";

/** Supported output formats and their content types. */
const OUTPUT_FORMATS = {
  mp3_44100_128: "audio/mpeg",
  mp3_44100_64: "audio/mpeg",
  mp3_44100_96: "audio/mpeg",
  mp3_44100_192: "audio/mpeg",
  pcm_16000: "audio/pcm",
  pcm_22050: "audio/pcm",
  pcm_24000: "audio/pcm",
  pcm_44100: "audio/pcm",
  ulaw_8000: "audio/basic",
};

/** Default voice settings. */
const DEFAULT_VOICE_SETTINGS = {
  stability: 0.5,
  similarity_boost: 0.75,
  style: 0.0,
  use_speaker_boost: true,
};

// ============================================================================
// TTSProvider
// ============================================================================

/**
 * TTSProvider - ElevenLabs Text-to-Speech integration.
 *
 * Usage:
 *   const tts = new TTSProvider();
 *   if (await tts.isAvailable()) {
 *     const audioBuffer = await tts.synthesize("Hello, world!");
 *   }
 */
export class TTSProvider {
  /**
   * @param {Object} [options]
   * @param {string} [options.apiKey]        - ElevenLabs API key (defaults to env)
   * @param {string} [options.voiceId]       - Default voice ID (defaults to env or built-in)
   * @param {string} [options.modelId]       - Synthesis model ID
   * @param {Object} [options.voiceSettings] - Default voice settings overrides
   * @param {string} [options.outputFormat]  - Audio output format
   */
  constructor(options = {}) {
    this.apiKey = options.apiKey || process.env.ELEVENLABS_API_KEY || null;
    this.voiceId = options.voiceId || process.env.ELEVENLABS_VOICE_ID || DEFAULT_VOICE_ID;
    this.modelId = options.modelId || DEFAULT_MODEL_ID;
    this.voiceSettings = { ...DEFAULT_VOICE_SETTINGS, ...options.voiceSettings };
    this.outputFormat = options.outputFormat || "mp3_44100_128";
    this.log = createLogger({ level: process.env.LOG_LEVEL || "info" }).child({ module: "tts" });
  }

  // --------------------------------------------------------------------------
  // Public API
  // --------------------------------------------------------------------------

  /**
   * Check whether the TTS provider is available (API key is configured).
   * @returns {Promise<boolean>}
   */
  async isAvailable() {
    return !!this.apiKey;
  }

  /**
   * Synthesize text into audio.
   *
   * @param {string} text - The text to speak.
   * @param {Object}  [opts]
   * @param {string}  [opts.voiceId]       - Override voice ID for this call.
   * @param {string}  [opts.modelId]       - Override model ID for this call.
   * @param {Object}  [opts.voiceSettings] - Override voice settings for this call.
   * @param {string}  [opts.outputFormat]  - Override output format for this call.
   * @returns {Promise<Buffer|null>} Audio data as a Buffer (mp3 by default), or null if unavailable.
   */
  async synthesize(text, opts = {}) {
    if (!this.apiKey) {
      this.log.debug("TTS unavailable: no ELEVENLABS_API_KEY configured");
      return null;
    }

    if (!text || typeof text !== "string" || text.trim().length === 0) {
      this.log.warn("TTS synthesize called with empty text");
      return null;
    }

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

    this.log.debug("Synthesizing speech", {
      voiceId,
      modelId,
      textLength: text.length,
      outputFormat,
    });

    try {
      const res = await fetch(url, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          "xi-api-key": this.apiKey,
          Accept: OUTPUT_FORMATS[outputFormat] || "audio/mpeg",
        },
        body: JSON.stringify(body),
      });

      if (!res.ok) {
        const errBody = await res.text().catch(() => "Unknown error");
        this.log.error("ElevenLabs API error", {
          status: res.status,
          statusText: res.statusText,
          body: errBody.slice(0, 500),
        });
        throw new Error(`ElevenLabs API error ${res.status}: ${errBody.slice(0, 200)}`);
      }

      const arrayBuffer = await res.arrayBuffer();
      const buffer = Buffer.from(arrayBuffer);

      this.log.info("Speech synthesized", {
        voiceId,
        textLength: text.length,
        audioBytes: buffer.length,
        outputFormat,
      });

      return buffer;
    } catch (err) {
      if (err.message.startsWith("ElevenLabs API error")) {
        throw err;
      }
      this.log.error("TTS synthesis failed", { error: err.message });
      throw new Error(`TTS synthesis failed: ${err.message}`);
    }
  }

  /**
   * List available voices from ElevenLabs.
   *
   * @returns {Promise<Array<{ voice_id: string, name: string, category: string, labels: Object }> | null>}
   *   Array of voice objects, or null if unavailable.
   */
  async listVoices() {
    if (!this.apiKey) {
      this.log.debug("TTS unavailable: no ELEVENLABS_API_KEY configured");
      return null;
    }

    const url = `${ELEVENLABS_BASE_URL}/voices`;

    this.log.debug("Fetching available voices");

    try {
      const res = await fetch(url, {
        method: "GET",
        headers: {
          "xi-api-key": this.apiKey,
          Accept: "application/json",
        },
      });

      if (!res.ok) {
        const errBody = await res.text().catch(() => "Unknown error");
        this.log.error("ElevenLabs voices API error", {
          status: res.status,
          body: errBody.slice(0, 500),
        });
        throw new Error(`ElevenLabs voices API error ${res.status}: ${errBody.slice(0, 200)}`);
      }

      const data = await res.json();
      const voices = (data.voices || []).map((v) => ({
        voice_id: v.voice_id,
        name: v.name,
        category: v.category || "unknown",
        labels: v.labels || {},
        preview_url: v.preview_url || null,
      }));

      this.log.info("Voices fetched", { count: voices.length });
      return voices;
    } catch (err) {
      if (err.message.startsWith("ElevenLabs voices API error")) {
        throw err;
      }
      this.log.error("Failed to fetch voices", { error: err.message });
      throw new Error(`Failed to fetch voices: ${err.message}`);
    }
  }
}

// ============================================================================
// Singleton
// ============================================================================

/** @type {TTSProvider | null} */
let _singleton = null;

/**
 * Get the singleton TTSProvider instance.
 * @param {Object} [options] - Passed to constructor on first call only.
 * @returns {TTSProvider}
 */
export function getTTSProvider(options) {
  if (!_singleton) {
    _singleton = new TTSProvider(options);
  }
  return _singleton;
}

export default TTSProvider;
