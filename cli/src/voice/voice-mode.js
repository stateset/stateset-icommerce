/**
 * Voice Mode Controller for StateSet iCommerce
 *
 * Manages voice mode state per session, orchestrating Speech-to-Text and
 * Text-to-Speech providers to enable conversational voice interactions.
 *
 * Voice mode turns audio input into text, processes it through the agent
 * pipeline, then synthesizes the response back to audio.
 */

import { createLogger } from '../logger.js';
import { getTTSProvider } from './tts.js';
import { getSTTProvider } from './stt.js';

// ============================================================================
// Constants
// ============================================================================

/** Maximum text length to synthesize in a single TTS call. */
const MAX_TTS_TEXT_LENGTH = 5000;

/** Session voice mode TTL - auto-disable after 30 minutes of inactivity. */
const VOICE_SESSION_TTL_MS = 30 * 60 * 1000;

// ============================================================================
// VoiceModeController
// ============================================================================

/**
 * VoiceModeController - Orchestrates voice interactions per session.
 *
 * Usage:
 *   const ctrl = new VoiceModeController();
 *   ctrl.enableVoiceMode(sessionId);
 *   const result = await ctrl.processVoiceMessage(audioBuffer, sessionId, {
 *     format: "mp3",
 *     agentHandler: async (text) => agentReply,
 *   });
 *   // result.audioResponse is a Buffer (mp3), result.textResponse is the string
 */
export class VoiceModeController {
  /**
   * @param {Object} [options]
   * @param {import("./tts.js").TTSProvider} [options.ttsProvider]  - Custom TTS provider
   * @param {import("./stt.js").STTProvider} [options.sttProvider]  - Custom STT provider
   */
  constructor(options = {}) {
    this.tts = options.ttsProvider || getTTSProvider();
    this.stt = options.sttProvider || getSTTProvider();
    this.log = createLogger({ level: process.env.LOG_LEVEL || 'info' }).child({
      module: 'voice-mode',
    });

    /**
     * Active voice sessions.
     * Maps session ID to { enabled: boolean, lastActive: number, settings: Object }
     * @type {Map<string, { enabled: boolean, lastActive: number, settings: Object }>}
     */
    this.sessions = new Map();

    // Periodically clean up stale voice sessions
    this._cleanupInterval = null;
  }

  // --------------------------------------------------------------------------
  // Session Management
  // --------------------------------------------------------------------------

  /**
   * Enable voice mode for a session.
   *
   * @param {string} session - Session identifier.
   * @param {Object} [settings]
   * @param {string} [settings.voiceId]      - ElevenLabs voice ID override.
   * @param {string} [settings.language]     - Preferred STT language hint.
   * @param {string} [settings.outputFormat] - TTS output format override.
   * @returns {{ enabled: boolean, ttsAvailable: boolean, sttAvailable: boolean }}
   */
  enableVoiceMode(session, settings = {}) {
    if (!session || typeof session !== 'string') {
      throw new Error('enableVoiceMode requires a valid session identifier');
    }

    this.sessions.set(session, {
      enabled: true,
      lastActive: Date.now(),
      settings: {
        voiceId: settings.voiceId || null,
        language: settings.language || null,
        outputFormat: settings.outputFormat || null,
      },
    });

    this._ensureCleanup();

    const ttsAvailable = !!this.tts.apiKey;
    const sttAvailable = !!this.stt.apiKey;

    this.log.info('Voice mode enabled', {
      session,
      ttsAvailable,
      sttAvailable,
    });

    return { enabled: true, ttsAvailable, sttAvailable };
  }

  /**
   * Disable voice mode for a session.
   *
   * @param {string} session - Session identifier.
   * @returns {{ enabled: boolean }}
   */
  disableVoiceMode(session) {
    if (!session || typeof session !== 'string') {
      throw new Error('disableVoiceMode requires a valid session identifier');
    }

    const entry = this.sessions.get(session);
    if (entry) {
      entry.enabled = false;
      entry.lastActive = Date.now();
    }

    this.log.info('Voice mode disabled', { session });
    return { enabled: false };
  }

  /**
   * Check whether voice mode is enabled for a session.
   *
   * @param {string} session - Session identifier.
   * @returns {boolean}
   */
  isVoiceModeEnabled(session) {
    const entry = this.sessions.get(session);
    if (!entry) return false;

    // Auto-expire stale sessions
    if (Date.now() - entry.lastActive > VOICE_SESSION_TTL_MS) {
      this.sessions.delete(session);
      this.log.debug('Voice session expired', { session });
      return false;
    }

    return entry.enabled;
  }

  // --------------------------------------------------------------------------
  // Voice Processing Pipeline
  // --------------------------------------------------------------------------

  /**
   * Process a voice message: transcribe audio -> run agent -> synthesize reply.
   *
   * @param {Buffer} audioBuffer - Input audio data.
   * @param {string} session     - Session identifier.
   * @param {Object} opts
   * @param {Function} opts.agentHandler      - async (text: string) => string. Processes
   *                                            transcribed text through the agent and returns
   *                                            the text response.
   * @param {string}   [opts.format="mp3"]    - Audio input format.
   * @param {string}   [opts.language]        - Language hint for STT.
   * @param {string}   [opts.voiceId]         - Override TTS voice for this message.
   * @param {boolean}  [opts.skipTTS=false]   - If true, skip audio synthesis (return text only).
   * @returns {Promise<{
   *   transcription: { text: string, language: string | null, duration: number | null },
   *   textResponse: string,
   *   audioResponse: Buffer | null,
   *   timing: { sttMs: number, agentMs: number, ttsMs: number, totalMs: number }
   * }>}
   */
  async processVoiceMessage(audioBuffer, session, opts = {}) {
    const totalStart = Date.now();

    if (!opts.agentHandler || typeof opts.agentHandler !== 'function') {
      throw new Error('processVoiceMessage requires opts.agentHandler function');
    }

    // Update session activity
    const sessionEntry = this.sessions.get(session);
    if (sessionEntry) {
      sessionEntry.lastActive = Date.now();
    }

    // Merge session settings with per-call options
    const sessionSettings = sessionEntry?.settings || {};
    const format = opts.format || 'mp3';
    const language = opts.language || sessionSettings.language || null;
    const voiceId = opts.voiceId || sessionSettings.voiceId || null;
    const outputFormat = sessionSettings.outputFormat || null;
    const skipTTS = opts.skipTTS || false;

    this.log.info('Processing voice message', {
      session,
      format,
      language,
      bufferSize: audioBuffer?.length,
      skipTTS,
    });

    // ---- Step 1: Speech-to-Text ----
    const sttStart = Date.now();
    const transcription = await this.stt.transcribe(audioBuffer, {
      format,
      language,
    });
    const sttMs = Date.now() - sttStart;

    if (!transcription || !transcription.text) {
      this.log.warn('STT returned empty transcription', { session });
      return {
        transcription: { text: '', language: null, duration: null },
        textResponse: '',
        audioResponse: null,
        timing: { sttMs, agentMs: 0, ttsMs: 0, totalMs: Date.now() - totalStart },
      };
    }

    this.log.debug('STT complete', {
      session,
      text: transcription.text.slice(0, 100),
      sttMs,
    });

    // ---- Step 2: Agent Processing ----
    const agentStart = Date.now();
    let textResponse;
    try {
      textResponse = await opts.agentHandler(transcription.text);
    } catch (err) {
      this.log.error('Agent handler failed during voice processing', {
        session,
        error: err.message,
      });
      textResponse = 'I encountered an error processing your request. Please try again.';
    }
    const agentMs = Date.now() - agentStart;

    if (!textResponse || typeof textResponse !== 'string') {
      textResponse = '';
    }

    this.log.debug('Agent processing complete', {
      session,
      responseLength: textResponse.length,
      agentMs,
    });

    // ---- Step 3: Text-to-Speech ----
    let audioResponse = null;
    let ttsMs = 0;

    if (!skipTTS && textResponse.length > 0) {
      const ttsStart = Date.now();

      // Truncate overly long responses for TTS
      const ttsText =
        textResponse.length > MAX_TTS_TEXT_LENGTH
          ? textResponse.slice(0, MAX_TTS_TEXT_LENGTH) + '...'
          : textResponse;

      const ttsOpts = {};
      if (voiceId) ttsOpts.voiceId = voiceId;
      if (outputFormat) ttsOpts.outputFormat = outputFormat;

      audioResponse = await this.tts.synthesize(ttsText, ttsOpts);
      ttsMs = Date.now() - ttsStart;

      this.log.debug('TTS complete', {
        session,
        audioBytes: audioResponse?.length || 0,
        ttsMs,
      });
    }

    const totalMs = Date.now() - totalStart;

    this.log.info('Voice message processed', {
      session,
      sttMs,
      agentMs,
      ttsMs,
      totalMs,
      transcriptionLength: transcription.text.length,
      responseLength: textResponse.length,
      audioResponseBytes: audioResponse?.length || 0,
    });

    return {
      transcription,
      textResponse,
      audioResponse,
      timing: { sttMs, agentMs, ttsMs, totalMs },
    };
  }

  // --------------------------------------------------------------------------
  // Status
  // --------------------------------------------------------------------------

  /**
   * Get the current voice system status.
   *
   * @returns {Promise<{
   *   ttsAvailable: boolean,
   *   sttAvailable: boolean,
   *   activeVoiceSessions: number
   * }>}
   */
  async getVoiceStatus() {
    const ttsAvailable = await this.tts.isAvailable();
    const sttAvailable = await this.stt.isAvailable();

    // Count active (non-expired) voice sessions
    let activeVoiceSessions = 0;
    const now = Date.now();
    for (const entry of this.sessions.values()) {
      if (entry.enabled && now - entry.lastActive <= VOICE_SESSION_TTL_MS) {
        activeVoiceSessions++;
      }
    }

    return {
      ttsAvailable,
      sttAvailable,
      activeVoiceSessions,
    };
  }

  // --------------------------------------------------------------------------
  // Cleanup
  // --------------------------------------------------------------------------

  /**
   * Start the periodic cleanup of stale voice sessions.
   * @private
   */
  _ensureCleanup() {
    if (this._cleanupInterval) return;

    this._cleanupInterval = setInterval(
      () => {
        const now = Date.now();
        let removed = 0;
        for (const [id, entry] of this.sessions) {
          if (now - entry.lastActive > VOICE_SESSION_TTL_MS) {
            this.sessions.delete(id);
            removed++;
          }
        }
        if (removed > 0) {
          this.log.debug('Cleaned up stale voice sessions', { removed });
        }

        // Stop interval when no sessions remain
        if (this.sessions.size === 0 && this._cleanupInterval) {
          clearInterval(this._cleanupInterval);
          this._cleanupInterval = null;
        }
      },
      5 * 60 * 1000,
    ); // Check every 5 minutes

    // Ensure the interval does not prevent process exit
    if (this._cleanupInterval.unref) {
      this._cleanupInterval.unref();
    }
  }

  /**
   * Shut down the controller, clearing all sessions and stopping cleanup.
   */
  destroy() {
    if (this._cleanupInterval) {
      clearInterval(this._cleanupInterval);
      this._cleanupInterval = null;
    }
    this.sessions.clear();
    this.log.debug('VoiceModeController destroyed');
  }
}

// ============================================================================
// Singleton
// ============================================================================

/** @type {VoiceModeController | null} */
let _singleton = null;

/**
 * Get the singleton VoiceModeController instance.
 * @param {Object} [options] - Passed to constructor on first call only.
 * @returns {VoiceModeController}
 */
export function getVoiceModeController(options) {
  if (!_singleton) {
    _singleton = new VoiceModeController(options);
  }
  return _singleton;
}

export default VoiceModeController;
