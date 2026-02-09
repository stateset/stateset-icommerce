/**
 * Tests for cli/src/channels/command-registry.js
 *
 * Covers: CommandRegistry, getCommandRegistry, resetCommandRegistry.
 */

import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';

import { CommandRegistry, resetCommandRegistry } from '../../src/channels/command-registry.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function makeHandler() {
  return async (args, ctx) => ({ response: `handled: ${args}` });
}

function makeDef(overrides = {}) {
  return {
    name: 'test-cmd',
    description: 'A test command',
    handler: makeHandler(),
    ...overrides,
  };
}

// ---------------------------------------------------------------------------
// CommandRegistry
// ---------------------------------------------------------------------------

describe('CommandRegistry', () => {
  let reg;

  beforeEach(() => {
    reg = new CommandRegistry();
  });

  it('registers and retrieves a command', () => {
    reg.register(makeDef());
    const cmd = reg.get('test-cmd');
    assert.ok(cmd);
    assert.equal(cmd.name, 'test-cmd');
  });

  it('normalizes command names (strips /, lowercases)', () => {
    reg.register(makeDef({ name: 'MyCmd' }));
    assert.ok(reg.get('/mycmd'));
    assert.ok(reg.get('MYCMD'));
  });

  it('rejects reserved command names', () => {
    assert.throws(() => reg.register(makeDef({ name: 'help' })), /reserved/);
    assert.throws(() => reg.register(makeDef({ name: 'orders' })), /reserved/);
  });

  it('rejects invalid name format', () => {
    assert.throws(() => reg.register(makeDef({ name: '123bad' })), /Invalid command name/);
    assert.throws(() => reg.register(makeDef({ name: 'bad name' })), /Invalid command name/);
  });

  it('rejects duplicate names', () => {
    reg.register(makeDef());
    assert.throws(() => reg.register(makeDef()), /already registered/);
  });

  it('rejects non-function handler', () => {
    assert.throws(
      () => reg.register(makeDef({ handler: 'not a function' })),
      /handler must be a function/,
    );
  });

  it('rejects missing description', () => {
    assert.throws(() => reg.register(makeDef({ description: '' })), /description is required/);
  });

  it('registers and resolves aliases', () => {
    reg.register(makeDef({ name: 'foo', aliases: ['bar', 'baz'] }));
    assert.ok(reg.get('bar'));
    assert.ok(reg.get('baz'));
    assert.equal(reg.resolve('bar'), 'foo');
  });

  it('rejects reserved aliases', () => {
    assert.throws(() => reg.register(makeDef({ aliases: ['help'] })), /reserved/);
  });

  it('rejects conflicting aliases', () => {
    reg.register(makeDef({ name: 'cmd-a', aliases: ['shared'] }));
    assert.throws(() => reg.register(makeDef({ name: 'cmd-b', aliases: ['shared'] })), /conflicts/);
  });

  it('unregisters a command and its aliases', () => {
    reg.register(makeDef({ name: 'removeme', aliases: ['rmme'] }));
    assert.ok(reg.has('removeme'));
    assert.ok(reg.has('rmme'));

    assert.ok(reg.unregister('removeme'));
    assert.ok(!reg.has('removeme'));
    assert.ok(!reg.has('rmme'));
  });

  it('unregister returns false for unknown command', () => {
    assert.ok(!reg.unregister('nonexistent'));
  });

  it('unregister works via alias', () => {
    reg.register(makeDef({ name: 'cmd', aliases: ['a'] }));
    assert.ok(reg.unregister('a'));
    assert.ok(!reg.has('cmd'));
  });

  it('supports channel-specific names', () => {
    reg.register(
      makeDef({
        name: 'speak',
        channelNames: { discord: 'say', telegram: 'talk' },
      }),
    );

    assert.ok(reg.get('say', 'discord'));
    assert.ok(reg.get('talk', 'telegram'));
    assert.ok(!reg.get('say', 'slack'));
    assert.equal(reg.resolve('say', 'discord'), 'speak');
  });

  it('has() returns boolean', () => {
    reg.register(makeDef({ name: 'exists' }));
    assert.ok(reg.has('exists'));
    assert.ok(!reg.has('nope'));
  });

  it('list() returns all visible commands', () => {
    reg.register(makeDef({ name: 'visible' }));
    reg.register(makeDef({ name: 'hidden-cmd', hidden: true }));

    assert.equal(reg.list().length, 1);
    assert.equal(reg.list({ includeHidden: true }).length, 2);
  });

  it('list() filters by category', () => {
    reg.register(makeDef({ name: 'cat-a', category: 'alpha' }));
    reg.register(makeDef({ name: 'cat-b', category: 'beta' }));

    assert.equal(reg.list({ category: 'alpha' }).length, 1);
  });

  it('listBySource()', () => {
    reg.register(makeDef({ name: 'p1', source: 'plugin:foo' }));
    reg.register(makeDef({ name: 'p2', source: 'plugin:bar' }));

    assert.equal(reg.listBySource('plugin:foo').length, 1);
  });

  it('getCategories() returns sorted unique categories', () => {
    reg.register(makeDef({ name: 'c1', category: 'beta' }));
    reg.register(makeDef({ name: 'c2', category: 'alpha' }));
    reg.register(makeDef({ name: 'c3', category: 'alpha' }));

    const cats = reg.getCategories();
    assert.deepStrictEqual(cats, ['alpha', 'beta']);
  });

  it('generateHelp() returns formatted text', () => {
    reg.register(makeDef({ name: 'greet', description: 'Say hello' }));

    const help = reg.generateHelp();
    assert.ok(help.includes('/greet'));
    assert.ok(help.includes('Say hello'));
  });

  it('generateHelp() returns empty string when no commands', () => {
    assert.equal(reg.generateHelp(), '');
  });

  it('generateHelp() grouped mode', () => {
    reg.register(makeDef({ name: 'a-cmd', category: 'alpha' }));
    reg.register(makeDef({ name: 'b-cmd', category: 'beta' }));

    const help = reg.generateHelp({ grouped: true });
    assert.ok(help.includes('Alpha:'));
    assert.ok(help.includes('Beta:'));
  });

  it('lock/unlock prevents modifications', () => {
    reg.lock();
    assert.throws(() => reg.register(makeDef()), /locked/);
    assert.throws(() => reg.unregister('x'), /locked/);
    reg.unlock();
    reg.register(makeDef());
    assert.ok(reg.has('test-cmd'));
  });

  it('execute() runs handler with locking', async () => {
    reg.register(makeDef({ name: 'exec-test' }));
    const result = await reg.execute('exec-test', 'hello', { channel: 'test' });
    assert.equal(result.response, 'handled: hello');
  });

  it('execute() throws for unknown command', async () => {
    await assert.rejects(() => reg.execute('nope', '', {}), /not found/);
  });

  it('clear() removes everything', () => {
    reg.register(makeDef({ name: 'a', aliases: ['b'] }));
    reg.clear();
    assert.ok(!reg.has('a'));
    assert.ok(!reg.has('b'));
    assert.equal(reg.getStats().total, 0);
  });

  it('getStats() returns counts', () => {
    reg.register(makeDef({ name: 'x', aliases: ['y'], channelNames: { discord: 'z' } }));
    const stats = reg.getStats();
    assert.equal(stats.commands, 1);
    assert.equal(stats.aliases, 1);
    assert.ok(stats.channels.includes('discord'));
  });
});

// ---------------------------------------------------------------------------
// Singleton
// ---------------------------------------------------------------------------

describe('resetCommandRegistry', () => {
  it('resets the global instance', () => {
    resetCommandRegistry();
  });
});
