/**
 * Conversation history helpers for the Claude Agent harness.
 *
 * These utilities extract, format, and build prompts from multi-turn
 * conversation histories so that the agent can maintain context across turns.
 */

import { redactSensitive } from './privacy.js';

// ============================================================================
// Conversation History Helpers
// ============================================================================

/**
 * Extract plain text from various content types used in conversation messages.
 *
 * Handles strings, arrays of content blocks (text, tool_result), and nested
 * content objects.
 *
 * @param {string|Array|Object} content - The message content to extract text from
 * @returns {string} The extracted plain text
 */
export function extractHistoryText(content) {
  if (!content) return '';
  if (typeof content === 'string') return content;
  if (Array.isArray(content)) {
    return content
      .map((block) => {
        if (!block) return '';
        if (typeof block === 'string') return block;
        if (block.type === 'text' && typeof block.text === 'string') return block.text;
        if (block.type === 'tool_result' && typeof block.content === 'string') return block.content;
        if (block.text && typeof block.text === 'string') return block.text;
        return '';
      })
      .filter(Boolean)
      .join(' ');
  }
  if (content.text && typeof content.text === 'string') return content.text;
  if (content.content) return extractHistoryText(content.content);
  return '';
}

/**
 * Format an array of conversation history messages into a multi-line string.
 *
 * Each message is rendered as `ROLE: content` on its own line.
 *
 * @param {Array<{role?: string, type?: string, content?: any, message?: any, text?: any}>} history
 * @returns {string} Formatted history text (empty string when history is empty)
 */
export function formatConversationHistory(history) {
  if (!Array.isArray(history) || history.length === 0) return '';
  const lines = [];
  for (const msg of history) {
    if (!msg) continue;
    const role = (msg.role || msg.type || 'message').toString().toUpperCase();
    const content = extractHistoryText(
      msg.content ?? msg.message?.content ?? msg.text ?? msg.message ?? '',
    ).trim();
    if (!content) continue;
    lines.push(`${role}: ${content}`);
  }
  return lines.join('\n');
}

/**
 * Build a prompt string that includes conversation history context.
 *
 * When history is available, it is prepended to the current request so the
 * model can see prior turns.  Optional redaction can be applied to the
 * history and/or the request via the `options` parameter.
 *
 * @param {string} request - The current user request
 * @param {Array} history - Conversation history messages
 * @param {Object} [options]
 * @param {boolean} [options.redactHistory] - Redact sensitive data in history
 * @param {boolean} [options.redactRequest] - Redact sensitive data in request
 * @param {Object}  [options.redactOptions] - Options forwarded to redactSensitive
 * @returns {string} The composed prompt
 */
export function buildPromptWithHistory(request, history, options = {}) {
  const historyText = formatConversationHistory(history);
  if (!historyText) return request;
  const finalHistory = options.redactHistory
    ? redactSensitive(historyText, options.redactOptions)
    : historyText;
  const finalRequest = options.redactRequest
    ? redactSensitive(request, options.redactOptions)
    : request;
  return `Conversation history:\n${finalHistory}\n\nCurrent request:\n${finalRequest}`;
}

/**
 * Extract a compaction summary from compacted conversation history.
 *
 * After the context guard compacts history, the first user message typically
 * contains a summary.  This helper pulls that summary text out.
 *
 * @param {Array} compactedHistory - The compacted history array
 * @returns {string|null} The summary text, or null if not found
 */
export function extractCompactionSummary(compactedHistory) {
  if (!Array.isArray(compactedHistory) || compactedHistory.length === 0) return null;
  const summaryMsg = compactedHistory.find((msg) => msg?.role === 'user');
  if (!summaryMsg) return null;
  const text = extractHistoryText(summaryMsg.content ?? summaryMsg.message?.content ?? '');
  return text || null;
}

/**
 * Rough token estimation based on character count.
 *
 * Uses a simple heuristic of ~4 characters per token which is a reasonable
 * approximation for English text with the Claude tokenizer.
 *
 * @param {string} text - The text to estimate tokens for
 * @returns {number} Estimated token count (minimum 1 for non-empty text)
 */
export function estimateTokensFromText(text) {
  if (!text) return 0;
  const len = typeof text === 'string' ? text.length : String(text).length;
  return Math.max(1, Math.ceil(len / 4));
}
