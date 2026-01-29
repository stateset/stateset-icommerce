/**
 * Conversation Summarizer for StateSet iCommerce
 *
 * Uses Claude Haiku (cheapest model) to produce a concise summary of a
 * conversation, extracting key facts (customer names, order IDs, decisions).
 *
 * Called at session end or via the /memory save command.
 */

import { MODELS } from '../config.js';

// ============================================================================
// Prompt
// ============================================================================

const SUMMARIZE_SYSTEM = `You are a concise conversation summarizer for a commerce platform.
Given a conversation transcript, produce:
1. A 2-3 sentence summary of what was discussed and any actions taken.
2. A JSON array of key facts (customer names, emails, order IDs, SKUs, decisions, amounts).

Respond in this exact format:
SUMMARY: <your summary>
FACTS: ["fact1", "fact2", ...]`;

// ============================================================================
// Summarizer
// ============================================================================

export class ConversationSummarizer {
  /**
   * @param {Object} [opts]
   * @param {string} [opts.model] - Model for summarization (default: Haiku)
   * @param {string} [opts.apiKey] - Anthropic API key (default: env ANTHROPIC_API_KEY)
   */
  constructor(opts = {}) {
    this._model = opts.model || MODELS.HAIKU;
    this._apiKey = opts.apiKey || process.env.ANTHROPIC_API_KEY;
  }

  /**
   * Summarize a conversation transcript.
   * @param {string} transcript - The conversation text
   * @returns {Promise<{ summary: string, facts: string[], tokenCount: number }>}
   */
  async summarize(transcript) {
    if (!transcript || transcript.trim().length < 20) {
      return { summary: transcript?.trim() || '', facts: [], tokenCount: 0 };
    }

    // Truncate very long transcripts to avoid excessive cost
    const maxChars = 8000;
    const truncated = transcript.length > maxChars
      ? transcript.slice(0, maxChars) + '\n[... truncated]'
      : transcript;

    try {
      const result = await this._callClaude(truncated);
      return result;
    } catch (err) {
      // Fallback: simple truncation if API call fails
      console.warn(`[Summarizer] API call failed, using fallback: ${err.message}`);
      return {
        summary: truncated.slice(0, 500),
        facts: [],
        tokenCount: 0,
      };
    }
  }

  /**
   * Call the Anthropic Messages API directly (not the Agent SDK, for efficiency).
   * @private
   */
  async _callClaude(text) {
    if (!this._apiKey) {
      return { summary: text.slice(0, 500), facts: [], tokenCount: 0 };
    }

    const body = {
      model: this._model,
      max_tokens: 512,
      system: SUMMARIZE_SYSTEM,
      messages: [
        { role: 'user', content: `Summarize this conversation:\n\n${text}` },
      ],
    };

    const res = await fetch('https://api.anthropic.com/v1/messages', {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'x-api-key': this._apiKey,
        'anthropic-version': '2023-06-01',
      },
      body: JSON.stringify(body),
    });

    if (!res.ok) {
      throw new Error(`Anthropic API error: ${res.status} ${res.statusText}`);
    }

    const data = await res.json();
    const content = data.content?.[0]?.text || '';
    const tokenCount = (data.usage?.input_tokens || 0) + (data.usage?.output_tokens || 0);

    return this._parseResponse(content, tokenCount);
  }

  /**
   * Parse the summarizer response.
   * @private
   */
  _parseResponse(text, tokenCount) {
    let summary = '';
    let facts = [];

    const summaryMatch = text.match(/SUMMARY:\s*(.+?)(?:\nFACTS:|$)/s);
    if (summaryMatch) {
      summary = summaryMatch[1].trim();
    } else {
      summary = text.trim();
    }

    const factsMatch = text.match(/FACTS:\s*(\[.*\])/s);
    if (factsMatch) {
      try {
        facts = JSON.parse(factsMatch[1]);
      } catch {
        // Non-JSON facts line — split by comma
        facts = factsMatch[1].replace(/[\[\]"]/g, '').split(',').map(f => f.trim()).filter(Boolean);
      }
    }

    return { summary, facts, tokenCount };
  }
}

// ============================================================================
// Factory
// ============================================================================

let _instance = null;

/**
 * Get the global ConversationSummarizer singleton.
 * @param {Object} [opts]
 * @returns {ConversationSummarizer}
 */
export function getSummarizer(opts) {
  if (!_instance) {
    _instance = new ConversationSummarizer(opts);
  }
  return _instance;
}
