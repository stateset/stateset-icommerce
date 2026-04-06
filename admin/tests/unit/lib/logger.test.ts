/**
 * Tests for Structured Logger
 *
 * @module tests/unit/lib/logger
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

// Mock request-context before importing logger
vi.mock('@/lib/shared/request-context', () => ({
  getRequestContext: vi.fn(() => undefined),
  getRequestId: vi.fn(() => 'unknown'),
}));

import { logger } from '@/lib/shared/logger';
import { getRequestContext } from '@/lib/shared/request-context';

const mockedGetRequestContext = vi.mocked(getRequestContext);

describe('logger', () => {
  let consoleSpy: {
    log: ReturnType<typeof vi.spyOn>;
    error: ReturnType<typeof vi.spyOn>;
    warn: ReturnType<typeof vi.spyOn>;
    debug: ReturnType<typeof vi.spyOn>;
  };

  beforeEach(() => {
    consoleSpy = {
      log: vi.spyOn(console, 'log').mockImplementation(() => {}),
      error: vi.spyOn(console, 'error').mockImplementation(() => {}),
      warn: vi.spyOn(console, 'warn').mockImplementation(() => {}),
      debug: vi.spyOn(console, 'debug').mockImplementation(() => {}),
    };
    mockedGetRequestContext.mockReturnValue(undefined);
  });

  afterEach(() => {
    vi.restoreAllMocks();
    vi.unstubAllEnvs();
  });

  describe('info', () => {
    it('logs to console.log', () => {
      logger.info('Test info message');

      expect(consoleSpy.log).toHaveBeenCalledOnce();
    });

    it('outputs valid JSON', () => {
      logger.info('JSON test');

      const output = consoleSpy.log.mock.calls[0][0] as string;
      const parsed = JSON.parse(output);

      expect(parsed).toBeDefined();
      expect(typeof parsed).toBe('object');
    });

    it('includes level, message, and timestamp fields', () => {
      logger.info('Fields test');

      const output = consoleSpy.log.mock.calls[0][0] as string;
      const parsed = JSON.parse(output);

      expect(parsed.level).toBe('info');
      expect(parsed.message).toBe('Fields test');
      expect(parsed.timestamp).toBeDefined();
      expect(typeof parsed.timestamp).toBe('string');
    });

    it('includes additional metadata', () => {
      logger.info('With meta', { userId: 'u-1', action: 'login' });

      const output = consoleSpy.log.mock.calls[0][0] as string;
      const parsed = JSON.parse(output);

      expect(parsed.userId).toBe('u-1');
      expect(parsed.action).toBe('login');
    });
  });

  describe('error', () => {
    it('logs to console.error', () => {
      logger.error('Test error message');

      expect(consoleSpy.error).toHaveBeenCalledOnce();
    });

    it('outputs valid JSON with level error', () => {
      logger.error('Error JSON test');

      const output = consoleSpy.error.mock.calls[0][0] as string;
      const parsed = JSON.parse(output);

      expect(parsed.level).toBe('error');
      expect(parsed.message).toBe('Error JSON test');
    });

    it('includes additional metadata', () => {
      logger.error('DB failed', { dbHost: 'localhost', code: 'ECONNREFUSED' });

      const output = consoleSpy.error.mock.calls[0][0] as string;
      const parsed = JSON.parse(output);

      expect(parsed.dbHost).toBe('localhost');
      expect(parsed.code).toBe('ECONNREFUSED');
    });
  });

  describe('warn', () => {
    it('logs to console.warn', () => {
      logger.warn('Test warning');

      expect(consoleSpy.warn).toHaveBeenCalledOnce();
    });

    it('outputs valid JSON with level warn', () => {
      logger.warn('Warn JSON test');

      const output = consoleSpy.warn.mock.calls[0][0] as string;
      const parsed = JSON.parse(output);

      expect(parsed.level).toBe('warn');
      expect(parsed.message).toBe('Warn JSON test');
    });
  });

  describe('debug', () => {
    it('logs to console.debug in non-production', () => {
      vi.stubEnv('NODE_ENV', 'test');

      logger.debug('Debug message');

      expect(consoleSpy.debug).toHaveBeenCalledOnce();
    });

    it('outputs valid JSON with level debug', () => {
      vi.stubEnv('NODE_ENV', 'development');

      logger.debug('Debug JSON test');

      const output = consoleSpy.debug.mock.calls[0][0] as string;
      const parsed = JSON.parse(output);

      expect(parsed.level).toBe('debug');
      expect(parsed.message).toBe('Debug JSON test');
    });
  });

  describe('with request context', () => {
    it('includes requestId when context is available', () => {
      mockedGetRequestContext.mockReturnValue({
        requestId: 'req_abc123',
        startTime: Date.now(),
      });

      logger.info('With context');

      const output = consoleSpy.log.mock.calls[0][0] as string;
      const parsed = JSON.parse(output);

      expect(parsed.requestId).toBe('req_abc123');
    });

    it('includes orgId when present in context', () => {
      mockedGetRequestContext.mockReturnValue({
        requestId: 'req_abc123',
        startTime: Date.now(),
        orgId: 'org-42',
      });

      logger.info('With org');

      const output = consoleSpy.log.mock.calls[0][0] as string;
      const parsed = JSON.parse(output);

      expect(parsed.orgId).toBe('org-42');
    });

    it('includes path and method when present in context', () => {
      mockedGetRequestContext.mockReturnValue({
        requestId: 'req_abc123',
        startTime: Date.now(),
        path: '/api/sessions',
        method: 'GET',
      });

      logger.info('With path');

      const output = consoleSpy.log.mock.calls[0][0] as string;
      const parsed = JSON.parse(output);

      expect(parsed.path).toBe('/api/sessions');
      expect(parsed.method).toBe('GET');
    });

    it('omits context fields when context is undefined', () => {
      mockedGetRequestContext.mockReturnValue(undefined);

      logger.info('No context');

      const output = consoleSpy.log.mock.calls[0][0] as string;
      const parsed = JSON.parse(output);

      expect(parsed.requestId).toBeUndefined();
      expect(parsed.orgId).toBeUndefined();
    });
  });

  describe('JSON output format', () => {
    it('produces single-line JSON for each log call', () => {
      logger.info('Single line test');

      const output = consoleSpy.log.mock.calls[0][0] as string;
      expect(output).not.toContain('\n');
      expect(() => JSON.parse(output)).not.toThrow();
    });

    it('timestamp is in ISO format', () => {
      logger.info('Timestamp test');

      const output = consoleSpy.log.mock.calls[0][0] as string;
      const parsed = JSON.parse(output);
      const date = new Date(parsed.timestamp);

      expect(date.toISOString()).toBe(parsed.timestamp);
    });
  });
});
