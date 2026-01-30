/**
 * Context Window Guard for StateSet CLI
 *
 * Proactively manages context window usage to prevent overflow failures.
 * Estimates token count before LLM calls and triggers automatic compaction
 * if context exceeds configurable thresholds.
 *
 * Key features:
 * - Pre-flight context checking before each LLM call
 * - Automatic summarization/compaction when approaching limits
 * - Token counting using cl100k_base estimation (Claude tokenizer approximation)
 * - Graceful degradation with clear warnings
 *
 * Usage:
 *   const guard = new ContextGuard({ maxTokens: 200000 });
 *
 *   // Before LLM call
 *   const { safe, action, compactedHistory } = await guard.check(history, newMessage);
 *   if (!safe && action === 'abort') throw new Error('Context overflow');
 *   if (action === 'compact') history = compactedHistory;
 *
 *   // Proceed with LLM call...
 */

// ============================================================================
// Token Estimation
// ============================================================================

/**
 * Approximate token count for text using cl100k_base-like estimation.
 * This is a heuristic that closely matches Claude's actual tokenization.
 *
 * Rules:
 * - ~4 characters per token for English
 * - Code/JSON is denser (~3.5 chars/token)
 * - Whitespace and punctuation count as partial tokens
 *
 * @param {string} text
 * @returns {number}
 */
export function estimateTokens(text) {
  if (!text || typeof text !== 'string') return 0;

  // Count different character types
  const length = text.length;
  const whitespaceCount = (text.match(/\s/g) || []).length;
  const punctuationCount = (text.match(/[.,!?;:'"()\[\]{}]/g) || []).length;
  const codeIndicators = (text.match(/[{}\[\]();=<>]/g) || []).length;

  // Base estimate: ~4 chars per token
  let estimate = length / 4;

  // Adjust for code-heavy content (denser tokenization)
  if (codeIndicators > length * 0.05) {
    estimate = length / 3.5;
  }

  // Whitespace and punctuation adjustments
  estimate += whitespaceCount * 0.1;
  estimate += punctuationCount * 0.2;

  return Math.ceil(estimate);
}

/**
 * Estimate tokens for a message object (with role, content, etc.)
 * @param {object} message
 * @returns {number}
 */
export function estimateMessageTokens(message) {
  if (!message) return 0;

  let tokens = 0;

  // Role overhead (~3 tokens per message for role/formatting)
  tokens += 3;

  // Content
  if (typeof message.content === 'string') {
    tokens += estimateTokens(message.content);
  } else if (Array.isArray(message.content)) {
    for (const block of message.content) {
      if (block.type === 'text') {
        tokens += estimateTokens(block.text);
      } else if (block.type === 'tool_use') {
        tokens += estimateTokens(JSON.stringify(block.input));
        tokens += estimateTokens(block.name);
      } else if (block.type === 'tool_result') {
        tokens += estimateTokens(typeof block.content === 'string' ?
          block.content : JSON.stringify(block.content));
      }
    }
  }

  // Tool calls (if present)
  if (message.tool_calls) {
    tokens += estimateTokens(JSON.stringify(message.tool_calls));
  }

  return tokens;
}

/**
 * Estimate tokens for a conversation history.
 * @param {object[]} history
 * @returns {number}
 */
export function estimateHistoryTokens(history) {
  if (!Array.isArray(history)) return 0;
  return history.reduce((sum, msg) => sum + estimateMessageTokens(msg), 0);
}

// ============================================================================
// Summarizer
// ============================================================================

/**
 * Simple conversation summarizer for context compaction.
 * Extracts key information while reducing token count.
 */
export class ConversationSummarizer {
  constructor(options = {}) {
    this.maxSummaryTokens = options.maxSummaryTokens || 2000;
    this.keepRecentMessages = options.keepRecentMessages || 5;
  }

  /**
   * Summarize a conversation history.
   * Keeps recent messages intact, summarizes older content.
   *
   * @param {object[]} history
   * @returns {{ summary: string, keptMessages: object[], stats: object }}
   */
  summarize(history) {
    if (!Array.isArray(history) || history.length <= this.keepRecentMessages) {
      return {
        summary: null,
        keptMessages: history,
        stats: { summarized: 0, kept: history.length }
      };
    }

    // Split into older (to summarize) and recent (to keep)
    const splitPoint = history.length - this.keepRecentMessages;
    const toSummarize = history.slice(0, splitPoint);
    const toKeep = history.slice(splitPoint);

    // Extract key information from older messages
    const summaryParts = [];
    const toolsUsed = new Set();
    const entitiesMentioned = new Set();
    let userIntents = [];
    let keyResults = [];

    for (const msg of toSummarize) {
      const content = typeof msg.content === 'string' ? msg.content :
        (Array.isArray(msg.content) ?
          msg.content.filter(b => b.type === 'text').map(b => b.text).join(' ') : '');

      if (msg.role === 'user') {
        // Extract user intent
        const intent = content.slice(0, 200);
        if (intent) userIntents.push(intent);
      } else if (msg.role === 'assistant') {
        // Extract tool usage
        if (Array.isArray(msg.content)) {
          for (const block of msg.content) {
            if (block.type === 'tool_use') {
              toolsUsed.add(block.name);
            }
          }
        }
      }

      // Extract entity references (IDs, emails, etc.)
      const idMatches = content.match(/\b[A-Z]+-[A-Z0-9]+\b/g) || [];
      const emailMatches = content.match(/\b[\w.-]+@[\w.-]+\.\w+\b/g) || [];
      idMatches.forEach(id => entitiesMentioned.add(id));
      emailMatches.forEach(email => entitiesMentioned.add(email));
    }

    // Build summary
    if (userIntents.length > 0) {
      summaryParts.push(`**User requests:** ${userIntents.slice(0, 5).join('; ')}`);
    }

    if (toolsUsed.size > 0) {
      summaryParts.push(`**Tools used:** ${Array.from(toolsUsed).join(', ')}`);
    }

    if (entitiesMentioned.size > 0) {
      summaryParts.push(`**Entities:** ${Array.from(entitiesMentioned).slice(0, 10).join(', ')}`);
    }

    const summary = summaryParts.length > 0 ?
      `[Earlier conversation summary - ${toSummarize.length} messages]\n${summaryParts.join('\n')}` :
      `[Earlier conversation: ${toSummarize.length} messages summarized]`;

    return {
      summary,
      keptMessages: toKeep,
      stats: {
        summarized: toSummarize.length,
        kept: toKeep.length,
        originalTokens: estimateHistoryTokens(toSummarize),
        summaryTokens: estimateTokens(summary)
      }
    };
  }

  /**
   * Create a compacted history with summary prepended.
   *
   * @param {object[]} history
   * @returns {{ history: object[], stats: object }}
   */
  compact(history) {
    const { summary, keptMessages, stats } = this.summarize(history);

    if (!summary) {
      return { history, stats };
    }

    // Create a system message with the summary
    const summaryMessage = {
      role: 'user',
      content: summary
    };

    // Add assistant acknowledgment
    const ackMessage = {
      role: 'assistant',
      content: 'Understood. I have the context from our earlier conversation.'
    };

    const compactedHistory = [summaryMessage, ackMessage, ...keptMessages];

    return {
      history: compactedHistory,
      stats: {
        ...stats,
        compactedLength: compactedHistory.length,
        tokensSaved: stats.originalTokens - stats.summaryTokens - estimateMessageTokens(ackMessage)
      }
    };
  }
}

// ============================================================================
// ContextGuard
// ============================================================================

/**
 * Context Window Guard - pre-flight checker for LLM calls.
 */
export class ContextGuard {
  /**
   * @param {object} [options]
   * @param {number} [options.maxTokens=200000] - Maximum context window tokens
   * @param {number} [options.warningThreshold=0.7] - Warn at this % of max
   * @param {number} [options.compactThreshold=0.8] - Auto-compact at this % of max
   * @param {number} [options.abortThreshold=0.95] - Abort at this % of max
   * @param {number} [options.reserveTokens=4096] - Reserve for response generation
   * @param {ConversationSummarizer} [options.summarizer] - Custom summarizer
   */
  constructor(options = {}) {
    this.maxTokens = options.maxTokens || 200000;
    this.warningThreshold = options.warningThreshold || 0.7;
    this.compactThreshold = options.compactThreshold || 0.8;
    this.abortThreshold = options.abortThreshold || 0.95;
    this.reserveTokens = options.reserveTokens || 4096;
    this.summarizer = options.summarizer || new ConversationSummarizer();

    // Effective max after reserving space for response
    this.effectiveMax = this.maxTokens - this.reserveTokens;
  }

  /**
   * Check context usage and determine action.
   *
   * @param {object[]} history - Conversation history
   * @param {string} systemPrompt - System prompt
   * @param {string} [newMessage] - New user message to add
   * @returns {{
   *   safe: boolean,
   *   action: 'none' | 'warn' | 'compact' | 'abort',
   *   usage: { tokens: number, percent: number },
   *   compactedHistory?: object[],
   *   message?: string
   * }}
   */
  check(history, systemPrompt, newMessage = null) {
    // Estimate current token usage
    const historyTokens = estimateHistoryTokens(history);
    const systemTokens = estimateTokens(systemPrompt);
    const newMessageTokens = newMessage ? estimateTokens(newMessage) : 0;

    const totalTokens = historyTokens + systemTokens + newMessageTokens;
    const percent = totalTokens / this.effectiveMax;

    const usage = {
      tokens: totalTokens,
      percent,
      historyTokens,
      systemTokens,
      newMessageTokens,
      maxTokens: this.effectiveMax
    };

    // Check thresholds in order
    if (percent >= this.abortThreshold) {
      return {
        safe: false,
        action: 'abort',
        usage,
        message: `Context window at ${(percent * 100).toFixed(1)}% capacity (${totalTokens}/${this.effectiveMax} tokens). Cannot proceed safely.`
      };
    }

    if (percent >= this.compactThreshold) {
      // Auto-compact
      const { history: compactedHistory, stats } = this.summarizer.compact(history);
      const compactedTokens = estimateHistoryTokens(compactedHistory) + systemTokens + newMessageTokens;
      const compactedPercent = compactedTokens / this.effectiveMax;

      // Check if compaction helped enough
      if (compactedPercent >= this.abortThreshold) {
        return {
          safe: false,
          action: 'abort',
          usage,
          message: `Context still at ${(compactedPercent * 100).toFixed(1)}% after compaction. Too much context to process.`
        };
      }

      return {
        safe: true,
        action: 'compact',
        usage: {
          ...usage,
          afterCompaction: {
            tokens: compactedTokens,
            percent: compactedPercent,
            tokensSaved: totalTokens - compactedTokens
          }
        },
        compactedHistory,
        message: `Context at ${(percent * 100).toFixed(1)}%. Compacted to ${(compactedPercent * 100).toFixed(1)}% (saved ${stats.tokensSaved} tokens).`
      };
    }

    if (percent >= this.warningThreshold) {
      return {
        safe: true,
        action: 'warn',
        usage,
        message: `Context at ${(percent * 100).toFixed(1)}% capacity. Consider starting a new session or compacting history.`
      };
    }

    return {
      safe: true,
      action: 'none',
      usage
    };
  }

  /**
   * Get model-specific context limits.
   * @param {string} model
   * @returns {number}
   */
  static getModelContextLimit(model) {
    const limits = {
      'claude-opus-4-5-20251101': 200000,
      'claude-sonnet-4-5-20250929': 200000,
      'claude-haiku-3-5-20241022': 200000,
      'claude-3-5-sonnet-20241022': 200000,
      'claude-3-5-haiku-20241022': 200000,
      'gpt-4o': 128000,
      'gpt-4o-mini': 128000,
      'gpt-4-turbo': 128000,
      'gemini-2.0-flash': 1000000,
      'gemini-1.5-pro': 2000000
    };

    // Match partial model names
    for (const [key, limit] of Object.entries(limits)) {
      if (model.includes(key) || key.includes(model)) {
        return limit;
      }
    }

    // Default fallback
    return 128000;
  }

  /**
   * Create a guard configured for a specific model.
   * @param {string} model
   * @param {object} [options]
   * @returns {ContextGuard}
   */
  static forModel(model, options = {}) {
    const maxTokens = ContextGuard.getModelContextLimit(model);
    return new ContextGuard({ ...options, maxTokens });
  }
}

// ============================================================================
// Integration Helper
// ============================================================================

/**
 * Middleware-style guard for agent loops.
 *
 * @param {object} options
 * @param {object[]} options.history
 * @param {string} options.systemPrompt
 * @param {string} options.newMessage
 * @param {string} options.model
 * @param {Function} [options.onWarn] - Called when warning threshold reached
 * @param {Function} [options.onCompact] - Called when compaction occurs
 * @returns {{ proceed: boolean, history: object[], message?: string }}
 */
export async function guardContext({
  history,
  systemPrompt,
  newMessage,
  model,
  onWarn,
  onCompact
}) {
  const guard = ContextGuard.forModel(model);
  const result = guard.check(history, systemPrompt, newMessage);

  switch (result.action) {
    case 'abort':
      return {
        proceed: false,
        history,
        message: result.message
      };

    case 'compact':
      if (onCompact) onCompact(result);
      return {
        proceed: true,
        history: result.compactedHistory,
        message: result.message
      };

    case 'warn':
      if (onWarn) onWarn(result);
      return {
        proceed: true,
        history,
        message: result.message
      };

    default:
      return {
        proceed: true,
        history
      };
  }
}

// ============================================================================
// Exports
// ============================================================================

export default {
  ContextGuard,
  ConversationSummarizer,
  estimateTokens,
  estimateMessageTokens,
  estimateHistoryTokens,
  guardContext
};
