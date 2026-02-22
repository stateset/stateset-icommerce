/**
 * OpenAI Provider for StateSet iCommerce
 *
 * Uses the OpenAI Chat Completions API via fetch().
 * Supports streaming via Server-Sent Events.
 *
 * Models: gpt-4o, gpt-4, o1, o1-mini
 * Requires: OPENAI_API_KEY environment variable
 */

import { ModelProvider, DEFAULT_MAX_TOKENS } from './base.js';
import { PROVIDERS } from '../config.js';

const API_URL = 'https://api.openai.com/v1/chat/completions';

export class OpenAIProvider extends ModelProvider {
  constructor() {
    super('openai', PROVIDERS.openai);
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
    const apiKey = options.apiKey || this.getApiKey();
    if (!apiKey) {
      throw new Error('OpenAI API key not set. Set OPENAI_API_KEY environment variable.');
    }

    const model = this.resolveModel(options.model);
    const maxTokens = options.maxTokens || DEFAULT_MAX_TOKENS;
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

    const res = await fetch(API_URL, {
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

    return {
      text,
      model,
      provider: 'openai',
      cost: this.estimateCost(usage, model),
      usage,
    };
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
          } catch (err) {
            console.debug('[openai] Malformed SSE line:', err.message);
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

  /**
   * Rough cost estimation.
   * @private
   */
  _estimateCost(model, usage) {
    // Approximate prices per 1M tokens (input/output)
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
