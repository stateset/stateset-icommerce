/**
 * Enhanced Error Handling System for StateSet CLI
 *
 * Provides structured error classes with recovery suggestions,
 * context preservation, and consistent error codes.
 */

// Exit codes for consistent CLI behavior
export const EXIT_CODES = {
  SUCCESS: 0,
  USER_ERROR: 1,      // Bad arguments, permission denied, validation failed
  OPERATIONAL: 2,     // Database, API, file system errors
  INTERNAL: 3,        // Unexpected exceptions, bugs
  TIMEOUT: 4,         // Operation timed out
  CANCELLED: 5        // User cancelled operation
};

/**
 * Base error class for all StateSet CLI errors
 */
export class StateSetError extends Error {
  constructor(message, options = {}) {
    super(message);
    this.name = 'StateSetError';
    this.code = options.code || 'UNKNOWN_ERROR';
    this.exitCode = options.exitCode || EXIT_CODES.INTERNAL;
    this.retryable = options.retryable || false;
    this.context = options.context || {};
    this.cause = options.cause || null;
    this.timestamp = new Date().toISOString();

    // Capture stack trace
    if (Error.captureStackTrace) {
      Error.captureStackTrace(this, this.constructor);
    }
  }

  /**
   * Get user-friendly error message
   */
  get userMessage() {
    return this.message;
  }

  /**
   * Get recovery suggestions
   */
  getSuggestions() {
    return [];
  }

  /**
   * Format error for display
   */
  format(options = {}) {
    const { color = true, verbose = false } = options;
    const red = color ? '\x1b[31m' : '';
    const yellow = color ? '\x1b[33m' : '';
    const gray = color ? '\x1b[90m' : '';
    const reset = color ? '\x1b[0m' : '';

    let output = `${red}Error: ${this.userMessage}${reset}`;

    const suggestions = this.getSuggestions();
    if (suggestions.length > 0) {
      output += `\n\n${yellow}Suggestions:${reset}`;
      suggestions.forEach(s => {
        output += `\n  ${gray}•${reset} ${s}`;
      });
    }

    if (verbose && this.context && Object.keys(this.context).length > 0) {
      output += `\n\n${gray}Context: ${JSON.stringify(this.context, null, 2)}${reset}`;
    }

    if (verbose && this.stack) {
      output += `\n\n${gray}Stack trace:\n${this.stack}${reset}`;
    }

    return output;
  }

  /**
   * Convert to JSON for logging/API
   */
  toJSON() {
    return {
      name: this.name,
      code: this.code,
      message: this.message,
      exitCode: this.exitCode,
      retryable: this.retryable,
      context: this.context,
      suggestions: this.getSuggestions(),
      timestamp: this.timestamp,
      stack: this.stack
    };
  }
}

/**
 * Validation errors - invalid input, schema violations
 */
export class ValidationError extends StateSetError {
  constructor(message, options = {}) {
    super(message, {
      ...options,
      code: options.code || 'VALIDATION_ERROR',
      exitCode: EXIT_CODES.USER_ERROR,
      retryable: false
    });
    this.name = 'ValidationError';
    this.field = options.field || null;
    this.expected = options.expected || null;
    this.received = options.received || null;
  }

  getSuggestions() {
    const suggestions = [];

    if (this.field) {
      suggestions.push(`Check the '${this.field}' parameter`);
    }
    if (this.expected) {
      suggestions.push(`Expected: ${this.expected}`);
    }
    if (this.received !== null && this.received !== undefined) {
      suggestions.push(`Received: ${JSON.stringify(this.received)}`);
    }

    suggestions.push("Run with --help to see usage examples");
    return suggestions;
  }
}

/**
 * Permission errors - --apply not set, unauthorized
 */
export class PermissionError extends StateSetError {
  constructor(message, options = {}) {
    super(message, {
      ...options,
      code: options.code || 'PERMISSION_DENIED',
      exitCode: EXIT_CODES.USER_ERROR,
      retryable: false
    });
    this.name = 'PermissionError';
    this.requiredLevel = options.requiredLevel || 'write';
    this.currentLevel = options.currentLevel || 'preview';
    this.operation = options.operation || null;
  }

  get userMessage() {
    if (this.operation) {
      return `Permission denied: '${this.operation}' requires --apply flag`;
    }
    return this.message;
  }

  getSuggestions() {
    return [
      "Add --apply flag to enable write operations",
      "Example: stateset --apply \"your command here\"",
      "Run without --apply first to preview what would happen"
    ];
  }
}

/**
 * API errors - Claude API issues
 */
export class ApiError extends StateSetError {
  constructor(message, options = {}) {
    super(message, {
      ...options,
      code: options.code || 'API_ERROR',
      exitCode: EXIT_CODES.OPERATIONAL,
      retryable: options.retryable ?? true
    });
    this.name = 'ApiError';
    this.statusCode = options.statusCode || null;
    this.endpoint = options.endpoint || null;
  }

  getSuggestions() {
    const suggestions = [];

    if (this.statusCode === 401) {
      suggestions.push("Check your ANTHROPIC_API_KEY environment variable");
      suggestions.push("Run: stateset-doctor --checks api");
    } else if (this.statusCode === 429) {
      suggestions.push("Rate limit exceeded - wait a moment and try again");
      suggestions.push("Consider using a less frequent polling interval");
    } else if (this.statusCode >= 500) {
      suggestions.push("Claude API is experiencing issues");
      suggestions.push("Try again in a few minutes");
      suggestions.push("Use stateset-direct for non-AI operations");
    } else if (!this.statusCode) {
      suggestions.push("Check your internet connection");
      suggestions.push("Try: stateset-doctor --checks api");
    }

    if (this.retryable) {
      suggestions.push("This error may be temporary - retry the operation");
    }

    return suggestions;
  }
}

/**
 * Database errors - connection, query, schema issues
 */
export class DatabaseError extends StateSetError {
  constructor(message, options = {}) {
    super(message, {
      ...options,
      code: options.code || 'DATABASE_ERROR',
      exitCode: EXIT_CODES.OPERATIONAL,
      retryable: options.retryable ?? false
    });
    this.name = 'DatabaseError';
    this.dbPath = options.dbPath || null;
    this.query = options.query || null;
  }

  getSuggestions() {
    const suggestions = [];

    if (this.message.includes('SQLITE_BUSY') || this.message.includes('locked')) {
      suggestions.push("Database is locked by another process");
      suggestions.push("Close other applications using this database");
      suggestions.push("Try again in a moment");
      this.retryable = true;
    } else if (this.message.includes('no such table')) {
      suggestions.push("Database schema may be outdated");
      suggestions.push("Run: stateset-doctor --checks db");
    } else if (this.message.includes('ENOENT') || this.message.includes('not found')) {
      suggestions.push(`Database file not found: ${this.dbPath || 'unknown'}`);
      suggestions.push("Check the --db path argument");
      suggestions.push("The database will be created on first write operation");
    } else {
      suggestions.push("Run: stateset-doctor --checks db,permissions");
    }

    return suggestions;
  }
}

/**
 * Tool execution errors - MCP tool failures
 */
export class ToolError extends StateSetError {
  constructor(message, options = {}) {
    super(message, {
      ...options,
      code: options.code || 'TOOL_ERROR',
      exitCode: EXIT_CODES.OPERATIONAL,
      retryable: options.retryable ?? true
    });
    this.name = 'ToolError';
    this.toolName = options.toolName || null;
    this.input = options.input || null;
  }

  get userMessage() {
    if (this.toolName) {
      return `Tool '${this.toolName}' failed: ${this.message}`;
    }
    return this.message;
  }

  getSuggestions() {
    const suggestions = [];

    if (this.toolName) {
      suggestions.push(`Check parameters for '${this.toolName}'`);
    }

    if (this.message.includes('not found')) {
      suggestions.push("The requested resource may not exist");
      suggestions.push("Try listing available items first");
    }

    if (this.retryable) {
      suggestions.push("This error may be temporary - retry the operation");
    }

    return suggestions;
  }
}

/**
 * Configuration errors - missing/invalid config
 */
export class ConfigError extends StateSetError {
  constructor(message, options = {}) {
    super(message, {
      ...options,
      code: options.code || 'CONFIG_ERROR',
      exitCode: EXIT_CODES.USER_ERROR,
      retryable: false
    });
    this.name = 'ConfigError';
    this.configKey = options.configKey || null;
    this.configPath = options.configPath || null;
  }

  getSuggestions() {
    const suggestions = [];

    if (this.configKey) {
      suggestions.push(`Set the '${this.configKey}' configuration`);
      suggestions.push(`Run: stateset-config set ${this.configKey} <value>`);
    }

    if (this.configPath) {
      suggestions.push(`Check configuration file: ${this.configPath}`);
    }

    suggestions.push("Run: stateset-doctor --checks config");
    return suggestions;
  }
}

/**
 * Timeout errors - operation took too long
 */
export class TimeoutError extends StateSetError {
  constructor(message, options = {}) {
    super(message || 'Operation timed out', {
      ...options,
      code: options.code || 'TIMEOUT',
      exitCode: EXIT_CODES.TIMEOUT,
      retryable: true
    });
    this.name = 'TimeoutError';
    this.timeout = options.timeout || null;
    this.operation = options.operation || null;
  }

  getSuggestions() {
    return [
      "The operation took too long to complete",
      "Try a simpler query or break into smaller operations",
      "Check your network connection",
      "Retry the operation"
    ];
  }
}

/**
 * Not found errors - resource doesn't exist
 */
export class NotFoundError extends StateSetError {
  constructor(message, options = {}) {
    super(message, {
      ...options,
      code: options.code || 'NOT_FOUND',
      exitCode: EXIT_CODES.USER_ERROR,
      retryable: false
    });
    this.name = 'NotFoundError';
    this.resourceType = options.resourceType || null;
    this.resourceId = options.resourceId || null;
  }

  get userMessage() {
    if (this.resourceType && this.resourceId) {
      return `${this.resourceType} '${this.resourceId}' not found`;
    }
    return this.message;
  }

  getSuggestions() {
    const suggestions = [];

    if (this.resourceType) {
      suggestions.push(`Check that the ${this.resourceType} ID is correct`);
      suggestions.push(`List available ${this.resourceType}s: stateset-direct ${this.resourceType}s list`);
    }

    suggestions.push("Use a partial ID (like git) - only a few characters needed if unique");
    return suggestions;
  }
}

// ============================================================================
// Error Handler
// ============================================================================

/**
 * Global error handler for CLI
 */
export class ErrorHandler {
  constructor(options = {}) {
    this.verbose = options.verbose || false;
    this.json = options.json || false;
    this.logger = options.logger || console;
    this.onError = options.onError || null;
  }

  /**
   * Handle an error and exit appropriately
   */
  handle(error) {
    // Convert to StateSetError if needed
    const statesetError = this.normalize(error);

    // Call error callback if provided
    if (this.onError) {
      this.onError(statesetError);
    }

    // Output error
    if (this.json) {
      this.logger.error(JSON.stringify(statesetError.toJSON(), null, 2));
    } else {
      this.logger.error(statesetError.format({
        color: process.stdout.isTTY,
        verbose: this.verbose
      }));
    }

    return statesetError.exitCode;
  }

  /**
   * Normalize any error to a StateSetError
   */
  normalize(error) {
    if (error instanceof StateSetError) {
      return error;
    }

    // Detect error type from message/properties
    const message = error.message || String(error);

    if (message.includes('ANTHROPIC_API') || message.includes('API key')) {
      return new ApiError(message, { cause: error, statusCode: 401 });
    }

    if (message.includes('SQLITE') || message.includes('database')) {
      return new DatabaseError(message, { cause: error });
    }

    if (message.includes('permission') || message.includes('--apply')) {
      return new PermissionError(message, { cause: error });
    }

    if (message.includes('not found') || message.includes('No .* found')) {
      return new NotFoundError(message, { cause: error });
    }

    if (message.includes('timeout') || message.includes('ETIMEDOUT')) {
      return new TimeoutError(message, { cause: error });
    }

    if (message.includes('validation') || message.includes('invalid') || message.includes('required')) {
      return new ValidationError(message, { cause: error });
    }

    // Generic error
    return new StateSetError(message, {
      cause: error,
      exitCode: EXIT_CODES.INTERNAL
    });
  }

  /**
   * Wrap an async function with error handling
   */
  wrap(fn) {
    return async (...args) => {
      try {
        return await fn(...args);
      } catch (error) {
        const exitCode = this.handle(error);
        process.exit(exitCode);
      }
    };
  }
}

/**
 * Create an error handler
 */
export function createErrorHandler(options = {}) {
  return new ErrorHandler(options);
}

/**
 * Retry helper with exponential backoff
 */
export async function withRetry(fn, options = {}) {
  const maxRetries = options.maxRetries || 3;
  const baseDelay = options.baseDelay || 1000;
  const maxDelay = options.maxDelay || 30000;
  const shouldRetry = options.shouldRetry || ((error) => error.retryable);

  let lastError;

  for (let attempt = 0; attempt <= maxRetries; attempt++) {
    try {
      return await fn();
    } catch (error) {
      lastError = error;

      const statesetError = error instanceof StateSetError
        ? error
        : new StateSetError(error.message, { cause: error });

      if (!shouldRetry(statesetError) || attempt === maxRetries) {
        throw statesetError;
      }

      // Exponential backoff
      const delay = Math.min(baseDelay * Math.pow(2, attempt), maxDelay);
      await new Promise(resolve => setTimeout(resolve, delay));
    }
  }

  throw lastError;
}

export default {
  EXIT_CODES,
  StateSetError,
  ValidationError,
  PermissionError,
  ApiError,
  DatabaseError,
  ToolError,
  ConfigError,
  TimeoutError,
  NotFoundError,
  ErrorHandler,
  createErrorHandler,
  withRetry
};
