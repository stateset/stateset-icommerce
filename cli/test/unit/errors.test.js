import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';

import {
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
  withRetry,
} from '../../src/errors.js';

// ============================================================================
// EXIT_CODES
// ============================================================================

describe('EXIT_CODES', () => {
  it('defines all expected exit codes', () => {
    assert.equal(EXIT_CODES.SUCCESS, 0);
    assert.equal(EXIT_CODES.USER_ERROR, 1);
    assert.equal(EXIT_CODES.OPERATIONAL, 2);
    assert.equal(EXIT_CODES.INTERNAL, 3);
    assert.equal(EXIT_CODES.TIMEOUT, 4);
    assert.equal(EXIT_CODES.CANCELLED, 5);
  });
});

// ============================================================================
// StateSetError (base)
// ============================================================================

describe('StateSetError', () => {
  it('sets default properties', () => {
    const err = new StateSetError('test message');
    assert.equal(err.message, 'test message');
    assert.equal(err.name, 'StateSetError');
    assert.equal(err.code, 'UNKNOWN_ERROR');
    assert.equal(err.exitCode, EXIT_CODES.INTERNAL);
    assert.equal(err.retryable, false);
    assert.deepEqual(err.context, {});
    assert.equal(err.cause, null);
    assert.ok(err.timestamp);
    assert.ok(err.stack);
  });

  it('accepts custom options', () => {
    const cause = new Error('root');
    const err = new StateSetError('custom', {
      code: 'CUSTOM_CODE',
      exitCode: EXIT_CODES.USER_ERROR,
      retryable: true,
      context: { foo: 'bar' },
      cause,
    });
    assert.equal(err.code, 'CUSTOM_CODE');
    assert.equal(err.exitCode, EXIT_CODES.USER_ERROR);
    assert.equal(err.retryable, true);
    assert.deepEqual(err.context, { foo: 'bar' });
    assert.equal(err.cause, cause);
  });

  it('userMessage returns message by default', () => {
    const err = new StateSetError('hello');
    assert.equal(err.userMessage, 'hello');
  });

  it('getSuggestions returns empty array by default', () => {
    const err = new StateSetError('hello');
    assert.deepEqual(err.getSuggestions(), []);
  });

  it('format() produces colored output', () => {
    const err = new StateSetError('fail');
    const formatted = err.format({ color: true, verbose: false });
    assert.ok(formatted.includes('fail'));
    assert.ok(formatted.includes('\x1b[31m')); // red
  });

  it('format() produces plain output without color', () => {
    const err = new StateSetError('fail');
    const formatted = err.format({ color: false });
    assert.ok(formatted.includes('Error: fail'));
    assert.ok(!formatted.includes('\x1b['));
  });

  it('format() includes context in verbose mode', () => {
    const err = new StateSetError('fail', { context: { key: 'val' } });
    const formatted = err.format({ color: false, verbose: true });
    assert.ok(formatted.includes('Context:'));
    assert.ok(formatted.includes('"key"'));
  });

  it('format() includes stack in verbose mode', () => {
    const err = new StateSetError('fail');
    const formatted = err.format({ color: false, verbose: true });
    assert.ok(formatted.includes('Stack trace:'));
  });

  it('format() shows suggestions when available', () => {
    const err = new ValidationError('bad input', { field: 'email' });
    const formatted = err.format({ color: false });
    assert.ok(formatted.includes('Suggestions:'));
    assert.ok(formatted.includes('email'));
  });

  it('toJSON() includes all fields', () => {
    const err = new StateSetError('test', { code: 'MY_CODE' });
    const json = err.toJSON();
    assert.equal(json.name, 'StateSetError');
    assert.equal(json.code, 'MY_CODE');
    assert.equal(json.message, 'test');
    assert.equal(json.exitCode, EXIT_CODES.INTERNAL);
    assert.equal(json.retryable, false);
    assert.deepEqual(json.context, {});
    assert.deepEqual(json.suggestions, []);
    assert.ok(json.timestamp);
    assert.ok(json.stack);
  });

  it('is instanceof Error', () => {
    const err = new StateSetError('test');
    assert.ok(err instanceof Error);
    assert.ok(err instanceof StateSetError);
  });
});

// ============================================================================
// Error Subclasses
// ============================================================================

describe('ValidationError', () => {
  it('sets correct defaults', () => {
    const err = new ValidationError('bad input');
    assert.equal(err.name, 'ValidationError');
    assert.equal(err.code, 'VALIDATION_ERROR');
    assert.equal(err.exitCode, EXIT_CODES.USER_ERROR);
    assert.equal(err.retryable, false);
    assert.equal(err.field, null);
    assert.equal(err.expected, null);
    assert.equal(err.received, null);
  });

  it('includes field/expected/received in suggestions', () => {
    const err = new ValidationError('bad', {
      field: 'email',
      expected: 'string',
      received: 42,
    });
    const suggestions = err.getSuggestions();
    assert.ok(suggestions.some((s) => s.includes('email')));
    assert.ok(suggestions.some((s) => s.includes('string')));
    assert.ok(suggestions.some((s) => s.includes('42')));
    assert.ok(suggestions.some((s) => s.includes('--help')));
  });

  it('is instanceof StateSetError', () => {
    assert.ok(new ValidationError('x') instanceof StateSetError);
  });
});

describe('PermissionError', () => {
  it('sets correct defaults', () => {
    const err = new PermissionError('denied');
    assert.equal(err.name, 'PermissionError');
    assert.equal(err.code, 'PERMISSION_DENIED');
    assert.equal(err.exitCode, EXIT_CODES.USER_ERROR);
    assert.equal(err.requiredLevel, 'write');
    assert.equal(err.currentLevel, 'preview');
    assert.equal(err.operation, null);
  });

  it('userMessage includes operation when set', () => {
    const err = new PermissionError('denied', { operation: 'create_order' });
    assert.ok(err.userMessage.includes('create_order'));
    assert.ok(err.userMessage.includes('--apply'));
  });

  it('userMessage falls back to message when no operation', () => {
    const err = new PermissionError('custom denied');
    assert.equal(err.userMessage, 'custom denied');
  });

  it('suggestions include --apply guidance', () => {
    const suggestions = new PermissionError('denied').getSuggestions();
    assert.ok(suggestions.some((s) => s.includes('--apply')));
  });
});

describe('ApiError', () => {
  it('sets correct defaults with retryable=true', () => {
    const err = new ApiError('API failed');
    assert.equal(err.name, 'ApiError');
    assert.equal(err.code, 'API_ERROR');
    assert.equal(err.exitCode, EXIT_CODES.OPERATIONAL);
    assert.equal(err.retryable, true);
    assert.equal(err.statusCode, null);
    assert.equal(err.endpoint, null);
  });

  it('suggestions for 401 mention API key', () => {
    const err = new ApiError('Unauthorized', { statusCode: 401 });
    const suggestions = err.getSuggestions();
    assert.ok(suggestions.some((s) => s.includes('ANTHROPIC_API_KEY')));
  });

  it('suggestions for 429 mention rate limit', () => {
    const err = new ApiError('Too Many Requests', { statusCode: 429 });
    const suggestions = err.getSuggestions();
    assert.ok(suggestions.some((s) => s.toLowerCase().includes('rate limit')));
  });

  it('suggestions for 500+ mention API issues', () => {
    const err = new ApiError('Server Error', { statusCode: 500 });
    const suggestions = err.getSuggestions();
    assert.ok(suggestions.some((s) => s.includes('Claude API')));
  });

  it('suggestions for no statusCode mention network', () => {
    const err = new ApiError('connection failed');
    const suggestions = err.getSuggestions();
    assert.ok(
      suggestions.some(
        (s) =>
          s.toLowerCase().includes('internet') ||
          s.toLowerCase().includes('network') ||
          s.toLowerCase().includes('connection'),
      ),
    );
  });

  it('includes retry suggestion when retryable', () => {
    const err = new ApiError('fail', { retryable: true });
    const suggestions = err.getSuggestions();
    assert.ok(suggestions.some((s) => s.toLowerCase().includes('retry')));
  });
});

describe('DatabaseError', () => {
  it('sets correct defaults', () => {
    const err = new DatabaseError('db fail');
    assert.equal(err.name, 'DatabaseError');
    assert.equal(err.code, 'DATABASE_ERROR');
    assert.equal(err.exitCode, EXIT_CODES.OPERATIONAL);
    assert.equal(err.retryable, false);
    assert.equal(err.dbPath, null);
    assert.equal(err.query, null);
  });

  it('suggestions for SQLITE_BUSY mention locked', () => {
    const err = new DatabaseError('SQLITE_BUSY: database is locked');
    const suggestions = err.getSuggestions();
    assert.ok(suggestions.some((s) => s.toLowerCase().includes('locked')));
  });

  it('suggestions for no such table mention schema', () => {
    const err = new DatabaseError('no such table: orders');
    const suggestions = err.getSuggestions();
    assert.ok(suggestions.some((s) => s.includes('schema')));
  });

  it('suggestions for ENOENT mention file not found', () => {
    const err = new DatabaseError('ENOENT: no such file', { dbPath: '/tmp/store.db' });
    const suggestions = err.getSuggestions();
    assert.ok(suggestions.some((s) => s.includes('/tmp/store.db')));
  });

  it('generic db error falls back to stateset-doctor', () => {
    const err = new DatabaseError('something unknown');
    const suggestions = err.getSuggestions();
    assert.ok(suggestions.some((s) => s.includes('stateset-doctor')));
  });
});

describe('ToolError', () => {
  it('sets correct defaults', () => {
    const err = new ToolError('tool fail');
    assert.equal(err.name, 'ToolError');
    assert.equal(err.code, 'TOOL_ERROR');
    assert.equal(err.exitCode, EXIT_CODES.OPERATIONAL);
    assert.equal(err.retryable, true);
    assert.equal(err.toolName, null);
    assert.equal(err.input, null);
  });

  it('userMessage includes tool name when set', () => {
    const err = new ToolError('oops', { toolName: 'list_orders' });
    assert.ok(err.userMessage.includes('list_orders'));
  });

  it('userMessage falls back to message', () => {
    const err = new ToolError('just an error');
    assert.equal(err.userMessage, 'just an error');
  });

  it('suggestions mention checking parameters for named tool', () => {
    const err = new ToolError('fail', { toolName: 'get_order' });
    const suggestions = err.getSuggestions();
    assert.ok(suggestions.some((s) => s.includes('get_order')));
  });

  it('suggestions for not found message mention listing items', () => {
    const err = new ToolError('order not found');
    const suggestions = err.getSuggestions();
    assert.ok(suggestions.some((s) => s.toLowerCase().includes('list')));
  });
});

describe('ConfigError', () => {
  it('sets correct defaults', () => {
    const err = new ConfigError('bad config');
    assert.equal(err.name, 'ConfigError');
    assert.equal(err.code, 'CONFIG_ERROR');
    assert.equal(err.exitCode, EXIT_CODES.USER_ERROR);
    assert.equal(err.retryable, false);
    assert.equal(err.configKey, null);
    assert.equal(err.configPath, null);
  });

  it('suggestions include configKey when set', () => {
    const err = new ConfigError('missing', { configKey: 'apiKey' });
    const suggestions = err.getSuggestions();
    assert.ok(suggestions.some((s) => s.includes('apiKey')));
  });

  it('suggestions include configPath when set', () => {
    const err = new ConfigError('bad', { configPath: '/etc/stateset/config.json' });
    const suggestions = err.getSuggestions();
    assert.ok(suggestions.some((s) => s.includes('/etc/stateset/config.json')));
  });
});

describe('TimeoutError', () => {
  it('sets correct defaults', () => {
    const err = new TimeoutError();
    assert.equal(err.name, 'TimeoutError');
    assert.equal(err.code, 'TIMEOUT');
    assert.equal(err.exitCode, EXIT_CODES.TIMEOUT);
    assert.equal(err.retryable, true);
    assert.equal(err.timeout, null);
    assert.equal(err.operation, null);
    assert.equal(err.message, 'Operation timed out');
  });

  it('accepts custom message', () => {
    const err = new TimeoutError('Custom timeout', { timeout: 5000, operation: 'fetch' });
    assert.equal(err.message, 'Custom timeout');
    assert.equal(err.timeout, 5000);
    assert.equal(err.operation, 'fetch');
  });

  it('suggestions mention retry', () => {
    const suggestions = new TimeoutError().getSuggestions();
    assert.ok(suggestions.some((s) => s.toLowerCase().includes('retry')));
  });
});

describe('NotFoundError', () => {
  it('sets correct defaults', () => {
    const err = new NotFoundError('missing');
    assert.equal(err.name, 'NotFoundError');
    assert.equal(err.code, 'NOT_FOUND');
    assert.equal(err.exitCode, EXIT_CODES.USER_ERROR);
    assert.equal(err.retryable, false);
    assert.equal(err.resourceType, null);
    assert.equal(err.resourceId, null);
  });

  it('userMessage includes type and id', () => {
    const err = new NotFoundError('not found', {
      resourceType: 'Order',
      resourceId: 'ORD-123',
    });
    assert.ok(err.userMessage.includes('Order'));
    assert.ok(err.userMessage.includes('ORD-123'));
  });

  it('userMessage falls back to message', () => {
    const err = new NotFoundError('plain not found');
    assert.equal(err.userMessage, 'plain not found');
  });

  it('suggestions include resource type guidance', () => {
    const err = new NotFoundError('not found', { resourceType: 'customer' });
    const suggestions = err.getSuggestions();
    assert.ok(suggestions.some((s) => s.includes('customer')));
  });
});

// ============================================================================
// ErrorHandler
// ============================================================================

describe('ErrorHandler', () => {
  let handler;
  let logged;

  beforeEach(() => {
    logged = [];
    handler = new ErrorHandler({
      verbose: false,
      json: false,
      logger: { error: (msg) => logged.push(msg) },
    });
  });

  describe('normalize()', () => {
    it('returns StateSetError as-is', () => {
      const orig = new ValidationError('test');
      const result = handler.normalize(orig);
      assert.equal(result, orig);
    });

    // --- Property-based detection ---

    it('detects 401 statusCode → ApiError', () => {
      const err = handler.normalize({ message: 'auth fail', statusCode: 401 });
      assert.ok(err instanceof ApiError);
      assert.equal(err.statusCode, 401);
    });

    it('detects 403 statusCode → ApiError', () => {
      const err = handler.normalize({ message: 'forbidden', statusCode: 403 });
      assert.ok(err instanceof ApiError);
    });

    it('detects 404 statusCode → NotFoundError', () => {
      const err = handler.normalize({ message: 'not found', statusCode: 404 });
      assert.ok(err instanceof NotFoundError);
    });

    it('detects 422 statusCode → ValidationError', () => {
      const err = handler.normalize({ message: 'unprocessable', statusCode: 422 });
      assert.ok(err instanceof ValidationError);
    });

    it('detects 429 statusCode → ApiError (retryable)', () => {
      const err = handler.normalize({ message: 'rate limited', statusCode: 429 });
      assert.ok(err instanceof ApiError);
      assert.equal(err.retryable, true);
      assert.equal(err.statusCode, 429);
    });

    it('detects 500+ statusCode → ApiError (retryable)', () => {
      const err = handler.normalize({ message: 'server error', statusCode: 502 });
      assert.ok(err instanceof ApiError);
      assert.equal(err.retryable, true);
    });

    it('detects VALIDATION_ERROR code → ValidationError', () => {
      const err = handler.normalize({ message: 'invalid', code: 'VALIDATION_ERROR' });
      assert.ok(err instanceof ValidationError);
    });

    it('detects RATE_LIMITED code → ApiError', () => {
      const err = handler.normalize({ message: 'slow down', code: 'RATE_LIMITED' });
      assert.ok(err instanceof ApiError);
      assert.equal(err.statusCode, 429);
    });

    it('detects ANTHROPIC_AUTH_ERROR code → ApiError', () => {
      const err = handler.normalize({ message: 'bad key', code: 'ANTHROPIC_AUTH_ERROR' });
      assert.ok(err instanceof ApiError);
    });

    it('detects ECONNREFUSED → ApiError (network)', () => {
      const err = handler.normalize({ message: 'connect failed', code: 'ECONNREFUSED' });
      assert.ok(err instanceof ApiError);
      assert.equal(err.retryable, true);
    });

    it('detects ENOTFOUND → ApiError (network)', () => {
      const err = handler.normalize({ message: 'host not found', code: 'ENOTFOUND' });
      assert.ok(err instanceof ApiError);
      assert.equal(err.retryable, true);
    });

    it('detects ENETUNREACH → ApiError (network)', () => {
      const err = handler.normalize({ message: 'network unreachable', code: 'ENETUNREACH' });
      assert.ok(err instanceof ApiError);
    });

    it('detects ECONNRESET → ApiError (network)', () => {
      const err = handler.normalize({ message: 'connection reset', code: 'ECONNRESET' });
      assert.ok(err instanceof ApiError);
    });

    it('detects EACCES → PermissionError', () => {
      const err = handler.normalize({ message: 'permission denied', code: 'EACCES' });
      assert.ok(err instanceof PermissionError);
    });

    it('detects EPERM → PermissionError', () => {
      const err = handler.normalize({ message: 'operation not permitted', code: 'EPERM' });
      assert.ok(err instanceof PermissionError);
    });

    it('detects ENOENT → NotFoundError', () => {
      const err = handler.normalize({ message: 'no such file', code: 'ENOENT' });
      assert.ok(err instanceof NotFoundError);
    });

    it('detects SQLITE_ERROR → DatabaseError', () => {
      const err = handler.normalize({ message: 'sql error', code: 'SQLITE_ERROR' });
      assert.ok(err instanceof DatabaseError);
    });

    it('detects SQLITE_BUSY → DatabaseError', () => {
      const err = handler.normalize({ message: 'db locked', code: 'SQLITE_BUSY' });
      assert.ok(err instanceof DatabaseError);
    });

    it('detects SQLITE_CONSTRAINT → DatabaseError', () => {
      const err = handler.normalize({ message: 'unique constraint', code: 'SQLITE_CONSTRAINT' });
      assert.ok(err instanceof DatabaseError);
    });

    it('detects ETIMEDOUT → TimeoutError', () => {
      const err = handler.normalize({ message: 'timed out', code: 'ETIMEDOUT' });
      assert.ok(err instanceof TimeoutError);
    });

    it('detects ESOCKETTIMEDOUT → TimeoutError', () => {
      const err = handler.normalize({ message: 'socket timeout', code: 'ESOCKETTIMEDOUT' });
      assert.ok(err instanceof TimeoutError);
    });

    it('detects ABORT_ERR → TimeoutError', () => {
      const err = handler.normalize({ message: 'aborted', code: 'ABORT_ERR' });
      assert.ok(err instanceof TimeoutError);
    });

    // --- Message-based detection (case-insensitive) ---

    it('detects "anthropic_api" in message → ApiError', () => {
      const err = handler.normalize(new Error('ANTHROPIC_API returned 401'));
      assert.ok(err instanceof ApiError);
    });

    it('detects "api key" in message → ApiError', () => {
      const err = handler.normalize(new Error('Invalid API Key provided'));
      assert.ok(err instanceof ApiError);
    });

    it('detects "sqlite" in message → DatabaseError', () => {
      const err = handler.normalize(new Error('SQLITE_CANTOPEN: unable to open'));
      assert.ok(err instanceof DatabaseError);
    });

    it('detects "database" in message → DatabaseError', () => {
      const err = handler.normalize(new Error('Database connection lost'));
      assert.ok(err instanceof DatabaseError);
    });

    it('detects "permission" in message → PermissionError', () => {
      const err = handler.normalize(new Error('Permission denied for this action'));
      assert.ok(err instanceof PermissionError);
    });

    it('detects "--apply" in message → PermissionError', () => {
      const err = handler.normalize(new Error('Requires --apply flag'));
      assert.ok(err instanceof PermissionError);
    });

    it('detects "not found" in message → NotFoundError', () => {
      const err = handler.normalize(new Error('Resource not found'));
      assert.ok(err instanceof NotFoundError);
    });

    it('detects "no X found" pattern in message → NotFoundError', () => {
      const err = handler.normalize(new Error('No customer found'));
      assert.ok(err instanceof NotFoundError);
    });

    it('detects "timeout" in message → TimeoutError', () => {
      const err = handler.normalize(new Error('Connection timeout'));
      assert.ok(err instanceof TimeoutError);
    });

    it('detects "timed out" in message → TimeoutError', () => {
      const err = handler.normalize(new Error('Request timed out'));
      assert.ok(err instanceof TimeoutError);
    });

    it('detects "validation" in message → ValidationError', () => {
      const err = handler.normalize(new Error('Validation failed'));
      assert.ok(err instanceof ValidationError);
    });

    it('detects "invalid" in message → ValidationError', () => {
      const err = handler.normalize(new Error('Invalid email format'));
      assert.ok(err instanceof ValidationError);
    });

    it('detects "required field" in message → ValidationError', () => {
      const err = handler.normalize(new Error('Required field missing'));
      assert.ok(err instanceof ValidationError);
    });

    // --- Fallback ---

    it('falls back to StateSetError for unknown errors', () => {
      const err = handler.normalize(new Error('something unexpected'));
      assert.ok(err instanceof StateSetError);
      assert.equal(err.exitCode, EXIT_CODES.INTERNAL);
    });

    it('handles non-Error objects', () => {
      const err = handler.normalize({ message: 'plain object error' });
      assert.ok(err instanceof StateSetError);
    });

    it('handles string errors via String()', () => {
      const err = handler.normalize('string error');
      assert.ok(err instanceof StateSetError);
      assert.ok(err.message.includes('string error'));
    });

    // --- Priority: property-based wins over message-based ---

    it('property detection takes priority over message detection', () => {
      // message says "not found" but code says SQLITE_ERROR => DatabaseError wins
      const err = handler.normalize({ message: 'not found', code: 'SQLITE_ERROR' });
      assert.ok(err instanceof DatabaseError);
    });

    it('statusCode detection takes priority over message detection', () => {
      // message says "timeout" but statusCode is 401 => ApiError wins
      const err = handler.normalize({ message: 'timeout', statusCode: 401 });
      assert.ok(err instanceof ApiError);
    });
  });

  describe('handle()', () => {
    it('returns exit code from error', () => {
      const exitCode = handler.handle(new ValidationError('bad'));
      assert.equal(exitCode, EXIT_CODES.USER_ERROR);
    });

    it('logs formatted error message', () => {
      handler.handle(new StateSetError('test error'));
      assert.ok(logged.length > 0);
      assert.ok(logged[0].includes('test error'));
    });

    it('calls onError callback when provided', () => {
      let captured = null;
      handler.onError = (err) => {
        captured = err;
      };
      handler.handle(new ValidationError('callback test'));
      assert.ok(captured instanceof ValidationError);
    });

    it('handles JSON mode', () => {
      handler.json = true;
      handler.handle(new ApiError('json test'));
      assert.ok(logged.length > 0);
      const parsed = JSON.parse(logged[0]);
      assert.equal(parsed.name, 'ApiError');
      assert.equal(parsed.message, 'json test');
    });

    it('normalizes plain errors before handling', () => {
      const exitCode = handler.handle(new Error('SQLITE_BUSY: locked'));
      assert.equal(exitCode, EXIT_CODES.OPERATIONAL);
    });
  });
});

// ============================================================================
// createErrorHandler
// ============================================================================

describe('createErrorHandler()', () => {
  it('returns an ErrorHandler instance', () => {
    const handler = createErrorHandler({ verbose: true });
    assert.ok(handler instanceof ErrorHandler);
    assert.equal(handler.verbose, true);
  });
});

// ============================================================================
// withRetry
// ============================================================================

describe('withRetry()', () => {
  it('returns result on first success', async () => {
    const result = await withRetry(() => Promise.resolve('ok'));
    assert.equal(result, 'ok');
  });

  it('retries on retryable error then succeeds', async () => {
    let attempts = 0;
    const result = await withRetry(
      () => {
        attempts++;
        if (attempts < 3) {
          throw new ApiError('transient', { retryable: true });
        }
        return Promise.resolve('recovered');
      },
      { maxRetries: 3, baseDelay: 10 },
    );
    assert.equal(result, 'recovered');
    assert.equal(attempts, 3);
  });

  it('throws after max retries exhausted', async () => {
    await assert.rejects(
      () =>
        withRetry(
          () => {
            throw new ApiError('keep failing', { retryable: true });
          },
          { maxRetries: 2, baseDelay: 10 },
        ),
      (err) => {
        assert.ok(err instanceof StateSetError);
        return true;
      },
    );
  });

  it('does not retry non-retryable errors', async () => {
    let attempts = 0;
    await assert.rejects(
      () =>
        withRetry(
          () => {
            attempts++;
            throw new ValidationError('not retryable');
          },
          { maxRetries: 3, baseDelay: 10 },
        ),
      (err) => {
        assert.ok(err instanceof StateSetError);
        return true;
      },
    );
    assert.equal(attempts, 1);
  });

  it('wraps non-StateSetError in StateSetError before retry check', async () => {
    let attempts = 0;
    await assert.rejects(
      () =>
        withRetry(
          () => {
            attempts++;
            throw new Error('generic error');
          },
          { maxRetries: 3, baseDelay: 10 },
        ),
      (err) => {
        assert.ok(err instanceof StateSetError);
        return true;
      },
    );
    // generic Error has retryable=false by default, so should not retry
    assert.equal(attempts, 1);
  });

  it('respects custom shouldRetry function', async () => {
    let attempts = 0;
    const result = await withRetry(
      () => {
        attempts++;
        if (attempts < 2) {
          const e = new Error('custom');
          e.retryMe = true;
          throw e;
        }
        return 'done';
      },
      {
        maxRetries: 3,
        baseDelay: 10,
        shouldRetry: (err) => err.cause?.retryMe === true,
      },
    );
    assert.equal(result, 'done');
    assert.equal(attempts, 2);
  });
});

// ============================================================================
// ErrorHandler.wrap()
// ============================================================================

describe('ErrorHandler.wrap()', () => {
  it('returns a function that catches and handles errors', async () => {
    let exitCodeCaptured = null;
    const originalExit = process.exit;

    // Temporarily mock process.exit to capture instead of exiting
    process.exit = (code) => {
      exitCodeCaptured = code;
    };

    try {
      const handler = new ErrorHandler({
        logger: { error: () => {} },
      });

      const wrapped = handler.wrap(async () => {
        throw new ValidationError('wrapped error');
      });

      await wrapped();
      assert.equal(exitCodeCaptured, EXIT_CODES.USER_ERROR);
    } finally {
      process.exit = originalExit;
    }
  });

  it('passes through successful results', async () => {
    const handler = new ErrorHandler({
      logger: { error: () => {} },
    });

    const wrapped = handler.wrap(async (x) => x * 2);
    const result = await wrapped(5);
    assert.equal(result, 10);
  });
});
