/**
 * Ollama Provider for StateSet iCommerce
 *
 * Uses the local Ollama HTTP API for running local models.
 * Supports streaming via NDJSON.
 *
 * Models: dynamically discovered via GET /api/tags
 * No API key required — runs locally.
 *
 * Default base URL: http://localhost:11434
 */

import { ModelProvider } from './base.js';
import { PROVIDERS } from '../config.js';

export class OllamaProvider extends ModelProvider {
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

  /**
   * Dynamically discover available local models.
   * @returns {string[]}
   */
  async discoverModels() {
    try {
      const res = await fetch(`${this._baseUrl}/api/tags`);
      if (!res.ok) return [];
      const data = await res.json();
      return (data.models || []).map(m => m.name);
    } catch {
      return [];
    }
  }

  /**
   * Override listModels to include dynamically discovered ones.
   * Note: This is synchronous, so it returns cached known models.
   * Use discoverModels() for the async version.
   */
  listModels() {
    return ['llama3', 'llama3.1', 'llama3.2', 'mistral', 'codellama', 'phi3', 'gemma2'];
  }

  /**
   * @param {import('./base.js').ChatMessage[]} messages
   * @param {import('./base.js').ChatOptions} [options]
   * @returns {Promise<import('./base.js').ChatResult>}
   */
  async chat(messages, options = {}) {
    const model = options.model || this.config.default || 'llama3';
    const stream = options.stream || false;

    // Convert system messages to Ollama format
    const ollamaMessages = messages.map(m => ({
      role: m.role,
      content: m.content,
    }));

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
      cost: null, // Local models are free
      usage: {
        inputTokens: data.prompt_eval_count || 0,
        outputTokens: data.eval_count || 0,
      },
    };
  }

  /**
   * Handle NDJSON streaming response.
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

            // Check for completion
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
            // Skip malformed lines
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
