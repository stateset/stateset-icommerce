/**
 * Structured Logging Module for StateSet CLI
 *
 * Provides structured, level-based logging with JSON output support,
 * context tracking, subsystem prefixes, and performance timing.
 */

import { PALETTE } from './theme.js';

// Log levels with numeric priorities
export const LOG_LEVELS = {
  silent: -1, // suppress all logging
  error: 0,
  warn: 1,
  info: 2,
  debug: 3,
  trace: 4,
};

// ANSI color codes — sourced from theme palette
const COLORS = {
  reset: PALETTE.reset,
  red: '\x1b[31m',
  yellow: '\x1b[33m',
  blue: '\x1b[34m',
  gray: '\x1b[90m',
  cyan: '\x1b[36m',
};

const LEVEL_COLORS = {
  error: COLORS.red,
  warn: COLORS.yellow,
  info: COLORS.blue,
  debug: COLORS.gray,
  trace: COLORS.cyan,
};

// Rotating palette for subsystem prefix coloring
const SUBSYSTEM_COLORS = [
  PALETTE.accent, // blue
  PALETTE.info, // light blue
  '\x1b[35m', // magenta
  PALETTE.accentBright, // bright blue
  '\x1b[36m', // cyan
  PALETTE.warn, // orange
];

/**
 * Logger - Structured logging with levels and context
 *
 * Usage:
 *   const log = createLogger({ level: 'info', json: false });
 *   log.info('Server started', { port: 3000 });
 *   log.error('Connection failed', { host: 'db.example.com', error: err.message });
 */
export class Logger {
  constructor(options = {}) {
    this.level = LOG_LEVELS[options.level] ?? LOG_LEVELS.info;
    this.json = options.json ?? false;
    this.color = options.color ?? (process.stdout.isTTY && !this.json);
    this.context = options.context ?? {};
    this.timers = new Map();
    this.output = options.output ?? console;
    this.subsystem = options.subsystem ?? null;
  }

  /**
   * Create a child logger with additional context
   */
  child(context) {
    return new Logger({
      level: Object.keys(LOG_LEVELS).find((k) => LOG_LEVELS[k] === this.level),
      json: this.json,
      color: this.color,
      context: { ...this.context, ...context },
      output: this.output,
      subsystem: this.subsystem,
    });
  }

  /**
   * Create a child logger scoped to a subsystem.
   *
   * Output is prefixed with `[subsystem]` in human mode, or includes
   * a `subsystem` field in JSON mode.  Nested calls produce
   * `[parent/child]` prefixes.
   *
   * @param {string} name
   * @returns {Logger}
   */
  subsystemLogger(name) {
    const fullName = this.subsystem ? `${this.subsystem}/${name}` : name;
    return new Logger({
      level: Object.keys(LOG_LEVELS).find((k) => LOG_LEVELS[k] === this.level),
      json: this.json,
      color: this.color,
      context: { ...this.context },
      output: this.output,
      subsystem: fullName,
    });
  }

  /**
   * Start a timer for performance tracking
   */
  time(label) {
    this.timers.set(label, Date.now());
  }

  /**
   * End a timer and log the duration
   */
  timeEnd(label, meta = {}) {
    const start = this.timers.get(label);
    if (start) {
      const duration = Date.now() - start;
      this.timers.delete(label);
      this.debug(`${label} completed`, { ...meta, duration_ms: duration });
      return duration;
    }
    return null;
  }

  /**
   * Log at error level
   */
  error(message, meta = {}) {
    this._log('error', message, meta);
  }

  /**
   * Log at warn level
   */
  warn(message, meta = {}) {
    this._log('warn', message, meta);
  }

  /**
   * Log at info level
   */
  info(message, meta = {}) {
    this._log('info', message, meta);
  }

  /**
   * Log at debug level
   */
  debug(message, meta = {}) {
    this._log('debug', message, meta);
  }

  /**
   * Log at trace level
   */
  trace(message, meta = {}) {
    this._log('trace', message, meta);
  }

  /**
   * Internal log method
   */
  _log(level, message, meta) {
    if (LOG_LEVELS[level] > this.level) return;

    const entry = {
      timestamp: new Date().toISOString(),
      level,
      ...(this.subsystem ? { subsystem: this.subsystem } : {}),
      message,
      ...this.context,
      ...meta,
    };

    if (this.json) {
      this.output.log(JSON.stringify(entry));
    } else {
      const levelStr = this._formatLevel(level);
      const prefix = this._formatSubsystem();
      const timestamp = this.color
        ? `${COLORS.gray}${entry.timestamp}${COLORS.reset}`
        : entry.timestamp;
      const metaStr =
        Object.keys(meta).length > 0
          ? ` ${this.color ? COLORS.gray : ''}${JSON.stringify(meta)}${this.color ? COLORS.reset : ''}`
          : '';

      this.output.log(`${timestamp} ${levelStr} ${prefix}${message}${metaStr}`);
    }
  }

  /**
   * Format subsystem prefix with deterministic color
   * @returns {string}
   */
  _formatSubsystem() {
    if (!this.subsystem) return '';
    if (!this.color) return `[${this.subsystem}] `;

    // Pick a stable color from the palette based on subsystem name hash
    let hash = 0;
    for (let i = 0; i < this.subsystem.length; i++) {
      hash = ((hash << 5) - hash + this.subsystem.charCodeAt(i)) | 0;
    }
    const colorCode = SUBSYSTEM_COLORS[Math.abs(hash) % SUBSYSTEM_COLORS.length];
    return `${colorCode}[${this.subsystem}]${COLORS.reset} `;
  }

  _formatLevel(level) {
    const upper = level.toUpperCase().padEnd(5);
    if (this.color) {
      return `${LEVEL_COLORS[level]}${upper}${COLORS.reset}`;
    }
    return upper;
  }
}

/**
 * Create a logger instance
 */
export function createLogger(options = {}) {
  // Allow environment variable override
  const level = options.level ?? process.env.LOG_LEVEL ?? 'info';
  const json = options.json ?? process.env.LOG_FORMAT === 'json';

  return new Logger({ ...options, level, json });
}

/**
 * Default logger instance
 */
export const logger = createLogger();

/**
 * Request/operation logger middleware
 * Creates a child logger with request-specific context
 */
export function createRequestLogger(requestId, operation) {
  return logger.child({
    requestId,
    operation,
  });
}

/**
 * Tool call logger
 * Specialized logging for MCP tool calls
 */
export class ToolCallLogger {
  constructor(baseLogger = logger) {
    this.logger = baseLogger;
  }

  logCall(toolName, input, requestId) {
    this.logger.info('Tool call started', {
      tool: toolName,
      input: this._sanitize(input),
      requestId,
    });
  }

  logResult(toolName, result, duration, requestId) {
    const success = !result.error;
    const level = success ? 'info' : 'warn';

    this.logger[level]('Tool call completed', {
      tool: toolName,
      success,
      duration_ms: duration,
      requestId,
      ...(result.error ? { error: result.error } : {}),
    });
  }

  _sanitize(input) {
    const sanitized = { ...input };
    const sensitiveFields = ['password', 'token', 'secret', 'key', 'apiKey', 'paymentToken'];
    for (const field of sensitiveFields) {
      if (sanitized[field]) {
        sanitized[field] = '[REDACTED]';
      }
    }
    return sanitized;
  }
}

/**
 * Create a subsystem-scoped logger (convenience wrapper).
 *
 * @param {string} subsystem - e.g. 'mcp', 'gateway', 'harness'
 * @returns {Logger}
 */
export function createSubsystemLogger(subsystem) {
  return logger.subsystemLogger(subsystem);
}

export default Logger;
