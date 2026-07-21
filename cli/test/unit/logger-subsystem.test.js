import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { Logger, createSubsystemLogger } from '../../src/logger.js';

describe('Logger subsystem support', () => {
  describe('subsystemLogger', () => {
    it('creates a logger with subsystem name', () => {
      const base = new Logger({ level: 'info', color: false });
      const sub = base.subsystemLogger('gateway');
      assert.strictEqual(sub.subsystem, 'gateway');
    });

    it('creates nested subsystem names', () => {
      const base = new Logger({ level: 'info', color: false });
      const sub = base.subsystemLogger('gateway');
      const nested = sub.subsystemLogger('discord');
      assert.strictEqual(nested.subsystem, 'gateway/discord');
    });

    it('preserves log level', () => {
      const base = new Logger({ level: 'debug', color: false });
      const sub = base.subsystemLogger('mcp');
      // debug level = 3
      assert.strictEqual(sub.level, 3);
    });

    it('preserves context', () => {
      const base = new Logger({ level: 'info', color: false, context: { app: 'test' } });
      const sub = base.subsystemLogger('harness');
      assert.deepStrictEqual(sub.context, { app: 'test' });
    });
  });

  describe('subsystem prefix in output', () => {
    it('includes [subsystem] prefix in plain text output', () => {
      const logs = [];
      const mockOutput = { log: (...args) => logs.push(args.join(' ')) };
      const logger = new Logger({
        level: 'info',
        color: false,
        output: mockOutput,
        subsystem: 'mcp',
      });

      logger.info('Server started');
      assert.strictEqual(logs.length, 1);
      assert.ok(logs[0].includes('[mcp]'), `Expected [mcp] prefix, got: ${logs[0]}`);
      assert.ok(logs[0].includes('Server started'));
    });

    it('includes nested subsystem prefix', () => {
      const logs = [];
      const mockOutput = { log: (...args) => logs.push(args.join(' ')) };
      const logger = new Logger({
        level: 'info',
        color: false,
        output: mockOutput,
        subsystem: 'gateway/slack',
      });

      logger.warn('Rate limited');
      assert.ok(logs[0].includes('[gateway/slack]'));
    });

    it('omits prefix when no subsystem', () => {
      const logs = [];
      const mockOutput = { log: (...args) => logs.push(args.join(' ')) };
      const logger = new Logger({ level: 'info', color: false, output: mockOutput });

      logger.info('No prefix');
      assert.ok(!logs[0].includes('['));
    });

    it('includes subsystem in JSON output', () => {
      const logs = [];
      const mockOutput = { log: (...args) => logs.push(args.join(' ')) };
      const logger = new Logger({
        level: 'info',
        json: true,
        output: mockOutput,
        subsystem: 'permissions',
      });

      logger.info('Access denied');
      const parsed = JSON.parse(logs[0]);
      assert.strictEqual(parsed.subsystem, 'permissions');
      assert.strictEqual(parsed.message, 'Access denied');
    });

    it('omits subsystem field from JSON when not set', () => {
      const logs = [];
      const mockOutput = { log: (...args) => logs.push(args.join(' ')) };
      const logger = new Logger({ level: 'info', json: true, output: mockOutput });

      logger.info('Plain log');
      const parsed = JSON.parse(logs[0]);
      assert.ok(!('subsystem' in parsed));
    });
  });

  describe('colored subsystem prefix', () => {
    it('wraps prefix in ANSI color codes when color enabled', () => {
      const logs = [];
      const mockOutput = { log: (...args) => logs.push(args.join(' ')) };
      const logger = new Logger({
        level: 'info',
        color: true,
        output: mockOutput,
        subsystem: 'gateway',
      });

      logger.info('Connected');
      assert.ok(logs[0].includes('[gateway]'));
      assert.ok(logs[0].includes('\x1b['));
    });

    it('uses consistent color for same subsystem name', () => {
      const logs1 = [];
      const logs2 = [];
      const out1 = { log: (...args) => logs1.push(args.join(' ')) };
      const out2 = { log: (...args) => logs2.push(args.join(' ')) };

      const l1 = new Logger({ level: 'info', color: true, output: out1, subsystem: 'harness' });
      const l2 = new Logger({ level: 'info', color: true, output: out2, subsystem: 'harness' });

      l1.info('msg1');
      l2.info('msg2');

      // Both should use the same color prefix (deterministic hash)
      const prefix1 = logs1[0].split('[harness]')[0];
      const prefix2 = logs2[0].split('[harness]')[0];
      // The ANSI color code part before [harness] should be the same
      assert.ok(prefix1.includes('\x1b['));
      assert.ok(prefix2.includes('\x1b['));
    });
  });

  describe('createSubsystemLogger', () => {
    it('creates a logger scoped to a subsystem', () => {
      const sub = createSubsystemLogger('channels');
      assert.strictEqual(sub.subsystem, 'channels');
    });

    it('inherits default logger settings', () => {
      const sub = createSubsystemLogger('test');
      // Should be able to log without error
      const logs = [];
      sub.output = { log: (...args) => logs.push(args.join(' ')) };
      sub.info('test message');
      assert.strictEqual(logs.length, 1);
    });
  });

  describe('child logger preserves subsystem', () => {
    it('child inherits subsystem from parent', () => {
      const parent = new Logger({ level: 'info', color: false, subsystem: 'mcp' });
      const child = parent.child({ requestId: '123' });
      assert.strictEqual(child.subsystem, 'mcp');
    });

    it('child includes subsystem in output', () => {
      const logs = [];
      const mockOutput = { log: (...args) => logs.push(args.join(' ')) };
      const parent = new Logger({
        level: 'info',
        color: false,
        output: mockOutput,
        subsystem: 'mcp',
      });
      const child = parent.child({ requestId: 'abc' });

      child.info('Tool called');
      assert.ok(logs[0].includes('[mcp]'));
      assert.ok(logs[0].includes('Tool called'));
    });
  });
});
