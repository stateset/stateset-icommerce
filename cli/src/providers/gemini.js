/**
 * Google Gemini Provider for StateSet iCommerce
 *
 * Uses the Gemini generateContent API via fetch().
 * Supports streaming via streamGenerateContent.
 *
 * Models: gemini-2.0-flash, gemini-2.0-pro
 * Requires: GEMINI_API_KEY environment variable
 */

import { ModelProvider } from './base.js';
import { PROVIDERS } from '../config.js';

const API_BASE = 'https://generativelanguage.googleapis.com/v1beta/models';

export class GeminiProvider extends ModelProvider {
  constructor() {
    super('gemini', PROVIDERS.gemini);
  }

  async isAvailable() {
    return !!this.getApiKey();
  }

  /**
   * @param {import('./base.js').ChatMessage[]} messages
   * @param {import('./base.js').ChatOptions} [options]
   * @returns {Promise<import('./base.js').ChatResult>}
   */
  async chat(messages, options = {}) {
    const apiKey = this.getApiKey();
    if (!apiKey) {
      throw new Error('Gemini API key not set. Set GEMINI_API_KEY environment variable.');
    }

    const model = this.resolveModel(options.model);
    const maxTokens = options.maxTokens || 4096;
    const temperature = options.temperature ?? 0.7;
    const stream = options.stream || false;

    // Convert messages to Gemini format
    const { systemInstruction, contents } = this._convertMessages(messages);

    const body = {
      contents,
      generationConfig: {
        maxOutputTokens: maxTokens,
        temperature,
      },
    };

    if (systemInstruction) {
      body.systemInstruction = { parts: [{ text: systemInstruction }] };
    }

    const endpoint = stream ? 'streamGenerateContent' : 'generateContent';
    const url = `${API_BASE}/${model}:${endpoint}?key=${apiKey}`;

    const res = await fetch(url, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(body),
    });

    if (!res.ok) {
      const err = await res.text().catch(() => 'Unknown error');
      throw new Error(`Gemini API error ${res.status}: ${err}`);
    }

    if (stream) {
      return this._handleStream(res, model, options.onPartialMessage);
    }

    const data = await res.json();
    const text = data.candidates?.[0]?.content?.parts
      ?.map(p => p.text)
      .join('') || '';

    const usage = {
      inputTokens: data.usageMetadata?.promptTokenCount || 0,
      outputTokens: data.usageMetadata?.candidatesTokenCount || 0,
    };

    return {
      text,
      model,
      provider: 'gemini',
      cost: this._estimateCost(model, usage),
      usage,
    };
  }

  /**
   * Convert standard messages to Gemini format.
   * @private
   */
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

  /**
   * Handle streaming response.
   * @private
   */
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

        // Gemini streams JSON array chunks
        // Try to parse complete JSON objects from buffer
        let startIdx = 0;
        while (startIdx < buffer.length) {
          // Find start of JSON object
          const objStart = buffer.indexOf('{', startIdx);
          if (objStart === -1) break;

          // Try to find matching end brace
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

          if (objEnd === -1) break; // Incomplete object

          try {
            const chunk = JSON.parse(buffer.slice(objStart, objEnd));
            const text = chunk.candidates?.[0]?.content?.parts
              ?.map(p => p.text)
              .join('') || '';

            if (text) {
              fullText += text;
              if (onPartialMessage) {
                onPartialMessage({ content: text, text });
              }
            }
          } catch {
            // Malformed JSON, skip
          }

          startIdx = objEnd;
        }

        // Keep unprocessed buffer
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

  /**
   * Rough cost estimation.
   * @private
   */
  _estimateCost(model, usage) {
    const prices = {
      'gemini-2.0-flash': { input: 0.075, output: 0.30 },
      'gemini-2.0-pro': { input: 1.25, output: 5.0 },
    };

    const p = prices[model];
    if (!p) return null;

    return (usage.inputTokens * p.input + usage.outputTokens * p.output) / 1_000_000;
  }
}
