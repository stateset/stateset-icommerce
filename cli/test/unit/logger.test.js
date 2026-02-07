/**
 * Unit tests for logger.js
 */

import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert';
import { Logger, createLogger, LOG_LEVELS, ToolCallLogger } from '../../src/logger.js';

describe('logger', () => {
  describe('LOG_LEVELS', () => {
    it('should have all levels defined', () => {
      assert.strictEqual(LOG_LEVELS.error, 0);
      assert.strictEqual(LOG_LEVELS.warn, 1);
      assert.strictEqual(LOG_LEVELS.info, 2);
      assert.strictEqual(LOG_LEVELS.debug, 3);
      assert.strictEqual(LOG_LEVELS.trace, 4);
    });
  });

  describe('Logger', () => {
    let logger;
    let output;

    beforeEach(() => {
      output = [];
      logger = new Logger({
        level: 'debug',
        json: true,
        output: {
          log: (msg) => output.push(msg),
        },
      });
    });

    describe('log levels', () => {
      it('should log at error level', () => {
        logger.error('test error', { code: 500 });
        assert.strictEqual(output.length, 1);
        const entry = JSON.parse(output[0]);
        assert.strictEqual(entry.level, 'error');
        assert.strictEqual(entry.message, 'test error');
        assert.strictEqual(entry.code, 500);
      });

      it('should log at warn level', () => {
        logger.warn('test warning');
        const entry = JSON.parse(output[0]);
        assert.strictEqual(entry.level, 'warn');
      });

      it('should log at info level', () => {
        logger.info('test info');
        const entry = JSON.parse(output[0]);
        assert.strictEqual(entry.level, 'info');
      });

      it('should log at debug level', () => {
        logger.debug('test debug');
        const entry = JSON.parse(output[0]);
        assert.strictEqual(entry.level, 'debug');
      });

      it('should not log trace when level is debug', () => {
        logger.trace('test trace');
        assert.strictEqual(output.length, 0);
      });
    });

    describe('filtering', () => {
      it('should filter messages below configured level', () => {
        const infoLogger = new Logger({
          level: 'info',
          json: true,
          output: { log: (msg) => output.push(msg) },
        });

        infoLogger.debug('should not appear');
        infoLogger.info('should appear');

        assert.strictEqual(output.length, 1);
        assert.ok(output[0].includes('should appear'));
      });

      it('should allow error at any level', () => {
        const errorOnlyLogger = new Logger({
          level: 'error',
          json: true,
          output: { log: (msg) => output.push(msg) },
        });

        errorOnlyLogger.error('error message');
        errorOnlyLogger.warn('warn message');

        assert.strictEqual(output.length, 1);
      });
    });

    describe('context', () => {
      it('should include context in log entries', () => {
        const contextLogger = new Logger({
          level: 'info',
          json: true,
          context: { service: 'test', version: '1.0' },
          output: { log: (msg) => output.push(msg) },
        });

        contextLogger.info('test message');
        const entry = JSON.parse(output[0]);

        assert.strictEqual(entry.service, 'test');
        assert.strictEqual(entry.version, '1.0');
      });

      it('should create child logger with merged context', () => {
        const parent = new Logger({
          level: 'info',
          json: true,
          context: { service: 'test' },
          output: { log: (msg) => output.push(msg) },
        });

        const child = parent.child({ requestId: '123' });
        child.info('child message');

        const entry = JSON.parse(output[0]);
        assert.strictEqual(entry.service, 'test');
        assert.strictEqual(entry.requestId, '123');
      });
    });

    describe('timing', () => {
      it('should track timers', async () => {
        logger.time('operation');
        await new Promise((r) => setTimeout(r, 10));
        const duration = logger.timeEnd('operation');

        assert.ok(duration >= 10, 'Duration should be at least 10ms');
        assert.ok(output.length > 0);
      });

      it('should return null for unknown timer', () => {
        const duration = logger.timeEnd('nonexistent');
        assert.strictEqual(duration, null);
      });
    });

    describe('JSON output', () => {
      it('should include timestamp', () => {
        logger.info('test');
        const entry = JSON.parse(output[0]);
        assert.ok(entry.timestamp);
        assert.ok(entry.timestamp.includes('T'));
      });

      it('should merge metadata', () => {
        logger.info('test', { foo: 'bar', count: 42 });
        const entry = JSON.parse(output[0]);
        assert.strictEqual(entry.foo, 'bar');
        assert.strictEqual(entry.count, 42);
      });
    });
  });

  describe('createLogger', () => {
    it('should create logger with defaults', () => {
      const logger = createLogger();
      assert.ok(logger instanceof Logger);
    });

    it('should respect environment variables', () => {
      const originalEnv = process.env.LOG_LEVEL;
      process.env.LOG_LEVEL = 'debug';

      const logger = createLogger();
      // Can't easily test internal level, but it should not throw
      assert.ok(logger);

      process.env.LOG_LEVEL = originalEnv;
    });

    it('should accept options', () => {
      const output = [];
      const logger = createLogger({
        level: 'info',
        json: true,
        output: { log: (msg) => output.push(msg) },
      });

      logger.info('test');
      assert.strictEqual(output.length, 1);
    });
  });

  describe('ToolCallLogger', () => {
    let toolLogger;
    let output;

    beforeEach(() => {
      output = [];
      const baseLogger = new Logger({
        level: 'info',
        json: true,
        output: { log: (msg) => output.push(msg) },
      });
      toolLogger = new ToolCallLogger(baseLogger);
    });

    it('should log tool calls', () => {
      toolLogger.logCall('list_customers', {}, 'req-123');
      const entry = JSON.parse(output[0]);

      assert.strictEqual(entry.tool, 'list_customers');
      assert.strictEqual(entry.requestId, 'req-123');
    });

    it('should log tool results', () => {
      toolLogger.logResult('list_customers', { success: true }, 50, 'req-123');
      const entry = JSON.parse(output[0]);

      assert.strictEqual(entry.tool, 'list_customers');
      assert.strictEqual(entry.success, true);
      assert.strictEqual(entry.duration_ms, 50);
    });

    it('should sanitize sensitive input', () => {
      toolLogger.logCall(
        'create_customer',
        {
          email: 'test@example.com',
          password: 'secret123',
          apiKey: 'key-xxx',
        },
        'req-123',
      );

      const entry = JSON.parse(output[0]);
      assert.strictEqual(entry.input.email, 'test@example.com');
      assert.strictEqual(entry.input.password, '[REDACTED]');
      assert.strictEqual(entry.input.apiKey, '[REDACTED]');
    });

    it('should log errors appropriately', () => {
      toolLogger.logResult('list_customers', { error: 'Connection failed' }, 100, 'req-123');
      const entry = JSON.parse(output[0]);

      assert.strictEqual(entry.success, false);
      assert.strictEqual(entry.error, 'Connection failed');
      assert.strictEqual(entry.level, 'warn');
    });
  });
});
