/**
 * Unit tests for tutorial.js — TUTORIALS, TutorialRunner, createTutorialRunner,
 * checkFirstRun, showWelcome
 */

import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import {
  TUTORIALS,
  TutorialRunner,
  createTutorialRunner,
  showWelcome,
} from '../../src/tutorial.js';

// ===========================================================================
// TUTORIALS structure
// ===========================================================================

describe('TUTORIALS', () => {
  it('has quickstart key', () => {
    assert.ok('quickstart' in TUTORIALS);
  });

  it('has orders key', () => {
    assert.ok('orders' in TUTORIALS);
  });

  it('has inventory key', () => {
    assert.ok('inventory' in TUTORIALS);
  });

  it('has checkout key', () => {
    assert.ok('checkout' in TUTORIALS);
  });

  it('has analytics key', () => {
    assert.ok('analytics' in TUTORIALS);
  });

  it('each tutorial has name, description, and steps', () => {
    for (const [id, tutorial] of Object.entries(TUTORIALS)) {
      assert.ok(typeof tutorial.name === 'string', `${id} missing name`);
      assert.ok(typeof tutorial.description === 'string', `${id} missing description`);
      assert.ok(Array.isArray(tutorial.steps), `${id} missing steps`);
      assert.ok(tutorial.steps.length > 0, `${id} has no steps`);
    }
  });

  it('quickstart has >= 7 steps', () => {
    assert.ok(TUTORIALS.quickstart.steps.length >= 7);
  });

  it('each step has title and content', () => {
    for (const [id, tutorial] of Object.entries(TUTORIALS)) {
      for (let i = 0; i < tutorial.steps.length; i++) {
        const step = tutorial.steps[i];
        assert.ok(typeof step.title === 'string', `${id} step ${i} missing title`);
        assert.ok(typeof step.content === 'string', `${id} step ${i} missing content`);
      }
    }
  });
});

// ===========================================================================
// TutorialRunner color helpers
// ===========================================================================

describe('TutorialRunner color helpers', () => {
  const runner = new TutorialRunner();

  it('bold wraps in ANSI bold codes', () => {
    const result = runner.bold('test');
    assert.equal(result, '\x1b[1mtest\x1b[0m');
  });

  it('dim wraps in ANSI dim codes', () => {
    const result = runner.dim('test');
    assert.equal(result, '\x1b[90mtest\x1b[0m');
  });

  it('cyan wraps in ANSI cyan codes', () => {
    const result = runner.cyan('test');
    assert.equal(result, '\x1b[36mtest\x1b[0m');
  });

  it('green wraps in ANSI green codes', () => {
    const result = runner.green('test');
    assert.equal(result, '\x1b[32mtest\x1b[0m');
  });

  it('yellow wraps in ANSI yellow codes', () => {
    const result = runner.yellow('test');
    assert.equal(result, '\x1b[33mtest\x1b[0m');
  });
});

// ===========================================================================
// TutorialRunner.run
// ===========================================================================

describe('TutorialRunner.run', () => {
  let logs;
  let errors;
  let originalLog;
  let originalError;

  beforeEach(() => {
    logs = [];
    errors = [];
    originalLog = console.info;
    originalError = console.error;
    console.info = (...args) => logs.push(args.join(' '));
    console.error = (...args) => errors.push(args.join(' '));
  });

  afterEach(() => {
    console.info = originalLog;
    console.error = originalError;
  });

  it('returns false for unknown tutorial', async () => {
    const runner = new TutorialRunner({ interactive: false });
    const result = await runner.run('nonexistent');
    assert.equal(result, false);
    assert.ok(errors.some((e) => e.includes('Unknown tutorial')));
  });

  it('runs quickstart to completion in non-interactive mode', async () => {
    const runner = new TutorialRunner({ interactive: false });
    const result = await runner.run('quickstart');
    assert.equal(result, true);
    // Should output step headers
    assert.ok(logs.some((l) => l.includes('Step 1/')));
    // Should output tutorial complete message
    assert.ok(logs.some((l) => l.includes('Tutorial complete')));
  });

  it('runs orders tutorial to completion', async () => {
    const runner = new TutorialRunner({ interactive: false });
    const result = await runner.run('orders');
    assert.equal(result, true);
  });

  it('runs inventory tutorial to completion', async () => {
    const runner = new TutorialRunner({ interactive: false });
    const result = await runner.run('inventory');
    assert.equal(result, true);
  });

  it('outputs tutorial name and description', async () => {
    const runner = new TutorialRunner({ interactive: false });
    await runner.run('quickstart');
    assert.ok(logs.some((l) => l.includes('Quick Start')));
  });

  it('outputs step content', async () => {
    const runner = new TutorialRunner({ interactive: false });
    await runner.run('quickstart');
    // First step mentions "StateSet CLI"
    assert.ok(logs.some((l) => l.includes('StateSet CLI')));
  });
});

// ===========================================================================
// TutorialRunner.listTutorials
// ===========================================================================

describe('TutorialRunner.listTutorials', () => {
  let logs;
  let originalInfo;

  beforeEach(() => {
    logs = [];
    originalInfo = console.info;
    console.info = (...args) => logs.push(args.join(' '));
  });

  afterEach(() => {
    console.info = originalInfo;
  });

  it('outputs Available Tutorials header', () => {
    const runner = new TutorialRunner();
    runner.listTutorials();
    assert.ok(logs.some((l) => l.includes('Available Tutorials')));
  });

  it('lists all tutorial IDs', () => {
    const runner = new TutorialRunner();
    runner.listTutorials();
    const allOutput = logs.join('\n');
    assert.ok(allOutput.includes('quickstart'));
    assert.ok(allOutput.includes('orders'));
    assert.ok(allOutput.includes('inventory'));
    assert.ok(allOutput.includes('checkout'));
    assert.ok(allOutput.includes('analytics'));
  });

  it('outputs run instructions', () => {
    const runner = new TutorialRunner();
    runner.listTutorials();
    assert.ok(logs.some((l) => l.includes('stateset-tutorial')));
  });
});

// ===========================================================================
// createTutorialRunner
// ===========================================================================

describe('createTutorialRunner', () => {
  it('returns a TutorialRunner instance', () => {
    const runner = createTutorialRunner();
    assert.ok(runner instanceof TutorialRunner);
  });

  it('passes options to the runner', () => {
    const runner = createTutorialRunner({ interactive: false });
    assert.equal(runner.interactive, false);
  });
});

// ===========================================================================
// showWelcome
// ===========================================================================

describe('showWelcome', () => {
  let logs;
  let originalInfo;

  beforeEach(() => {
    logs = [];
    originalInfo = console.info;
    console.info = (...args) => logs.push(args.join(' '));
  });

  afterEach(() => {
    console.info = originalInfo;
  });

  it('outputs welcome message with key phrases', () => {
    showWelcome();
    const allOutput = logs.join('\n');
    assert.ok(allOutput.includes('Welcome to StateSet CLI'));
    assert.ok(allOutput.includes('Quick Start'));
    assert.ok(allOutput.includes('stateset-tutorial'));
    assert.ok(allOutput.includes('stateset-doctor'));
  });
});
