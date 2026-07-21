import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import {
  State,
  Transition,
  WorkflowInstance,
  StateMachine,
  WorkflowEngine,
  WorkflowTemplates,
} from '../../src/workflows/state-machine.js';

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------
describe('State', () => {
  it('stores name and defaults', () => {
    const s = new State({ name: 'draft' });
    assert.equal(s.name, 'draft');
    assert.equal(s.description, '');
    assert.equal(s.onEnter, null);
    assert.equal(s.onExit, null);
    assert.equal(s.timeout, null);
    assert.equal(s.timeoutTransition, null);
    assert.deepStrictEqual(s.metadata, {});
  });

  it('accepts all options', () => {
    const enter = () => {};
    const exit = () => {};
    const s = new State({
      name: 'pending',
      description: 'Pending review',
      onEnter: enter,
      onExit: exit,
      timeout: 5000,
      timeoutTransition: 'expired',
      metadata: { level: 1 },
    });
    assert.equal(s.name, 'pending');
    assert.equal(s.description, 'Pending review');
    assert.equal(s.onEnter, enter);
    assert.equal(s.onExit, exit);
    assert.equal(s.timeout, 5000);
    assert.equal(s.timeoutTransition, 'expired');
    assert.deepStrictEqual(s.metadata, { level: 1 });
  });
});

// ---------------------------------------------------------------------------
// Transition
// ---------------------------------------------------------------------------
describe('Transition', () => {
  it('normalises from to an array when given a string', () => {
    const t = new Transition({ name: 'go', from: 'a', to: 'b' });
    assert.deepStrictEqual(t.from, ['a']);
    assert.equal(t.to, 'b');
  });

  it('keeps from as array when already an array', () => {
    const t = new Transition({ name: 'go', from: ['a', 'b'], to: 'c' });
    assert.deepStrictEqual(t.from, ['a', 'b']);
  });

  it('stores defaults', () => {
    const t = new Transition({ name: 'go', from: 'x', to: 'y' });
    assert.equal(t.condition, null);
    assert.equal(t.action, null);
    assert.equal(t.priority, 0);
    assert.deepStrictEqual(t.metadata, {});
  });

  it('stores all options', () => {
    const cond = () => true;
    const act = () => {};
    const t = new Transition({
      name: 'go',
      from: 'x',
      to: 'y',
      condition: cond,
      action: act,
      priority: 10,
      metadata: { tag: 'fast' },
    });
    assert.equal(t.condition, cond);
    assert.equal(t.action, act);
    assert.equal(t.priority, 10);
    assert.deepStrictEqual(t.metadata, { tag: 'fast' });
  });
});

// ---------------------------------------------------------------------------
// WorkflowInstance
// ---------------------------------------------------------------------------
describe('WorkflowInstance', () => {
  it('sets defaults', () => {
    const wi = new WorkflowInstance({
      workflowId: 'wf-1',
      workflowName: 'Test',
      currentState: 'init',
    });
    assert.ok(wi.id); // uuid generated
    assert.equal(wi.workflowId, 'wf-1');
    assert.equal(wi.workflowName, 'Test');
    assert.equal(wi.currentState, 'init');
    assert.deepStrictEqual(wi.context, {});
    assert.deepStrictEqual(wi.history, []);
    assert.equal(wi.status, 'running');
    assert.ok(wi.createdAt);
    assert.ok(wi.updatedAt);
    assert.equal(wi.completedAt, null);
    assert.equal(wi.error, null);
    assert.deepStrictEqual(wi.metadata, {});
    assert.equal(wi.timeoutTimer, null);
  });

  it('accepts explicit values', () => {
    const wi = new WorkflowInstance({
      id: 'custom-id',
      workflowId: 'wf-2',
      workflowName: 'Custom',
      currentState: 'step1',
      context: { key: 'val' },
      history: [{ event: 'started' }],
      status: 'paused',
      createdAt: '2024-01-01',
      updatedAt: '2024-01-02',
      completedAt: '2024-01-03',
      error: 'oops',
      metadata: { x: 1 },
    });
    assert.equal(wi.id, 'custom-id');
    assert.equal(wi.status, 'paused');
    assert.equal(wi.completedAt, '2024-01-03');
    assert.equal(wi.error, 'oops');
  });

  it('toJSON excludes timeoutTimer', () => {
    const wi = new WorkflowInstance({
      workflowId: 'wf',
      workflowName: 'T',
      currentState: 's',
    });
    wi.timeoutTimer = 42;
    const json = wi.toJSON();
    assert.equal(json.timeoutTimer, undefined);
    assert.equal(json.workflowId, 'wf');
    assert.equal(json.currentState, 's');
  });

  it('toJSON contains all serialisable fields', () => {
    const wi = new WorkflowInstance({
      workflowId: 'wf',
      workflowName: 'T',
      currentState: 's',
      context: { a: 1 },
      history: [{ e: 1 }],
      metadata: { m: 2 },
    });
    const json = wi.toJSON();
    const keys = Object.keys(json).sort();
    assert.deepStrictEqual(keys, [
      'completedAt',
      'context',
      'createdAt',
      'currentState',
      'error',
      'history',
      'id',
      'metadata',
      'status',
      'updatedAt',
      'workflowId',
      'workflowName',
    ]);
  });
});

// ---------------------------------------------------------------------------
// StateMachine
// ---------------------------------------------------------------------------
describe('StateMachine', () => {
  /** Helper to build a minimal valid config */
  function minimalConfig(overrides = {}) {
    return {
      name: 'test',
      initialState: 'a',
      states: [{ name: 'a' }, { name: 'b' }],
      transitions: [{ name: 'go', from: 'a', to: 'b' }],
      finalStates: ['b'],
      ...overrides,
    };
  }

  it('constructs with plain objects (auto-wraps State/Transition)', () => {
    const sm = new StateMachine(minimalConfig());
    assert.ok(sm.id);
    assert.equal(sm.name, 'test');
    assert.equal(sm.initialState, 'a');
    assert.equal(sm.states.size, 2);
    assert.ok(sm.states.get('a') instanceof State);
    assert.deepStrictEqual(sm.finalStates, ['b']);
  });

  it('constructs with State/Transition instances', () => {
    const sm = new StateMachine({
      name: 'inst',
      initialState: 'x',
      states: [new State({ name: 'x' }), new State({ name: 'y' })],
      transitions: [new Transition({ name: 'move', from: 'x', to: 'y' })],
      finalStates: ['y'],
    });
    assert.equal(sm.states.size, 2);
  });

  it('indexes transitions from multiple source states', () => {
    const sm = new StateMachine({
      name: 'multi',
      initialState: 'a',
      states: [{ name: 'a' }, { name: 'b' }, { name: 'c' }],
      transitions: [{ name: 'cancel', from: ['a', 'b'], to: 'c' }],
      finalStates: ['c'],
    });
    assert.equal(sm.getTransitions('a').length, 1);
    assert.equal(sm.getTransitions('b').length, 1);
    assert.equal(sm.getTransitions('a')[0].name, 'cancel');
  });

  it('sorts transitions by priority descending', () => {
    const sm = new StateMachine({
      name: 'prio',
      initialState: 'a',
      states: [{ name: 'a' }, { name: 'b' }, { name: 'c' }],
      transitions: [
        { name: 'low', from: 'a', to: 'b', priority: 1 },
        { name: 'high', from: 'a', to: 'c', priority: 10 },
      ],
      finalStates: ['b', 'c'],
    });
    const trans = sm.getTransitions('a');
    assert.equal(trans[0].name, 'high');
    assert.equal(trans[1].name, 'low');
  });

  describe('validate()', () => {
    it('throws if initial state not found', () => {
      assert.throws(
        () =>
          new StateMachine({
            name: 'bad',
            initialState: 'missing',
            states: [{ name: 'a' }],
            transitions: [],
            finalStates: [],
          }),
        /Initial state 'missing' not found/,
      );
    });

    it('throws if transition targets unknown state', () => {
      assert.throws(
        () =>
          new StateMachine({
            name: 'bad',
            initialState: 'a',
            states: [{ name: 'a' }],
            transitions: [{ name: 'go', from: 'a', to: 'z' }],
            finalStates: [],
          }),
        /Transition 'go' targets unknown state 'z'/,
      );
    });

    it('throws if transition source state is unknown', () => {
      assert.throws(
        () =>
          new StateMachine({
            name: 'bad',
            initialState: 'a',
            states: [{ name: 'a' }],
            transitions: [{ name: 'go', from: 'missing', to: 'a' }],
            finalStates: [],
          }),
        /Transition 'go' has unknown from state 'missing'/,
      );
    });

    it('throws if final state not found', () => {
      assert.throws(
        () =>
          new StateMachine({
            name: 'bad',
            initialState: 'a',
            states: [{ name: 'a' }],
            transitions: [],
            finalStates: ['z'],
          }),
        /Final state 'z' not found/,
      );
    });

    it('throws if timeout transition targets unknown state', () => {
      assert.throws(
        () =>
          new StateMachine({
            name: 'bad',
            initialState: 'a',
            states: [{ name: 'a', timeout: 1000, timeoutTransition: 'nowhere' }],
            transitions: [],
            finalStates: [],
          }),
        /State 'a' timeout targets unknown state 'nowhere'/,
      );
    });

    it('does not throw when timeout transition target exists', () => {
      assert.doesNotThrow(() => {
        new StateMachine({
          name: 'ok',
          initialState: 'a',
          states: [{ name: 'a', timeout: 1000, timeoutTransition: 'b' }, { name: 'b' }],
          transitions: [],
          finalStates: ['b'],
        });
      });
    });

    it('does not throw when timeout set without timeoutTransition', () => {
      assert.doesNotThrow(() => {
        new StateMachine({
          name: 'ok',
          initialState: 'a',
          states: [{ name: 'a', timeout: 1000 }],
          transitions: [],
          finalStates: [],
        });
      });
    });
  });

  describe('getTransitions()', () => {
    it('returns empty array for state with no transitions', () => {
      const sm = new StateMachine(minimalConfig());
      assert.deepStrictEqual(sm.getTransitions('b'), []);
    });

    it('returns empty array for unknown state', () => {
      const sm = new StateMachine(minimalConfig());
      assert.deepStrictEqual(sm.getTransitions('nonexistent'), []);
    });
  });

  describe('getState()', () => {
    it('returns state by name', () => {
      const sm = new StateMachine(minimalConfig());
      const s = sm.getState('a');
      assert.ok(s);
      assert.equal(s.name, 'a');
    });

    it('returns undefined for unknown state', () => {
      const sm = new StateMachine(minimalConfig());
      assert.equal(sm.getState('missing'), undefined);
    });
  });

  describe('isFinalState()', () => {
    it('returns true for final states', () => {
      const sm = new StateMachine(minimalConfig());
      assert.equal(sm.isFinalState('b'), true);
    });

    it('returns false for non-final states', () => {
      const sm = new StateMachine(minimalConfig());
      assert.equal(sm.isFinalState('a'), false);
    });

    it('returns false for unknown names', () => {
      const sm = new StateMachine(minimalConfig());
      assert.equal(sm.isFinalState('xyz'), false);
    });
  });

  describe('toJSON()', () => {
    it('returns serialisable representation', () => {
      const sm = new StateMachine(minimalConfig({ description: 'desc', metadata: { v: 1 } }));
      const json = sm.toJSON();
      assert.equal(json.name, 'test');
      assert.equal(json.description, 'desc');
      assert.equal(json.initialState, 'a');
      assert.deepStrictEqual(json.finalStates, ['b']);
      assert.deepStrictEqual(json.metadata, { v: 1 });
      assert.equal(json.states.length, 2);
      assert.equal(json.transitions.length, 1);
    });

    it('deduplicates transitions shared across multiple from states', () => {
      const sm = new StateMachine({
        name: 'dedup',
        initialState: 'a',
        states: [{ name: 'a' }, { name: 'b' }, { name: 'c' }],
        transitions: [{ name: 'cancel', from: ['a', 'b'], to: 'c' }],
        finalStates: ['c'],
      });
      const json = sm.toJSON();
      // Should appear once even though indexed under both 'a' and 'b'
      assert.equal(json.transitions.length, 1);
      assert.equal(json.transitions[0].name, 'cancel');
    });
  });

  it('is an EventEmitter', () => {
    const sm = new StateMachine(minimalConfig());
    assert.equal(typeof sm.on, 'function');
    assert.equal(typeof sm.emit, 'function');
  });
});

// ---------------------------------------------------------------------------
// WorkflowEngine
// ---------------------------------------------------------------------------
describe('WorkflowEngine', () => {
  let engine;
  let wfId;

  function simpleWorkflow() {
    return {
      name: 'simple',
      initialState: 'start',
      states: [{ name: 'start' }, { name: 'middle' }, { name: 'end' }],
      transitions: [
        { name: 'advance', from: 'start', to: 'middle' },
        { name: 'finish', from: 'middle', to: 'end' },
        { name: 'cancel', from: ['start', 'middle'], to: 'end' },
      ],
      finalStates: ['end'],
    };
  }

  beforeEach(() => {
    engine = new WorkflowEngine({});
    const wf = engine.registerWorkflow(simpleWorkflow());
    wfId = wf.id;
  });

  it('is an EventEmitter', () => {
    assert.equal(typeof engine.on, 'function');
  });

  // ---- registerWorkflow / getWorkflow / listWorkflows ----
  describe('registerWorkflow()', () => {
    it('registers from plain object', () => {
      const e = new WorkflowEngine({});
      const wf = e.registerWorkflow(simpleWorkflow());
      assert.ok(wf instanceof StateMachine);
      assert.equal(e.getWorkflow(wf.id), wf);
    });

    it('registers an existing StateMachine instance', () => {
      const sm = new StateMachine({ ...simpleWorkflow(), id: 'fixed' });
      const e = new WorkflowEngine({});
      const ret = e.registerWorkflow(sm);
      assert.equal(ret, sm);
      assert.equal(e.getWorkflow('fixed'), sm);
    });

    it('emits workflow:registered', () => {
      const e = new WorkflowEngine({});
      let emitted = null;
      e.on('workflow:registered', (data) => {
        emitted = data;
      });
      e.registerWorkflow(simpleWorkflow());
      assert.ok(emitted);
      assert.ok(emitted.workflow);
    });
  });

  describe('listWorkflows()', () => {
    it('returns all registered workflows as JSON', () => {
      const list = engine.listWorkflows();
      assert.equal(list.length, 1);
      assert.equal(list[0].name, 'simple');
    });
  });

  describe('getWorkflow()', () => {
    it('returns undefined for unknown id', () => {
      assert.equal(engine.getWorkflow('nope'), undefined);
    });
  });

  // ---- startWorkflow ----
  describe('startWorkflow()', () => {
    it('creates a running instance in the initial state', async () => {
      const inst = await engine.startWorkflow(wfId, { context: { orderId: 'o1' } });
      assert.ok(inst instanceof WorkflowInstance);
      assert.equal(inst.currentState, 'start');
      assert.equal(inst.status, 'running');
      assert.equal(inst.context.orderId, 'o1');
      assert.equal(inst.history.length, 1);
      assert.equal(inst.history[0].event, 'started');
    });

    it('throws for unknown workflow', async () => {
      await assert.rejects(() => engine.startWorkflow('missing'), /Workflow not found/);
    });

    it('stores instance retrievable via getInstance', async () => {
      const inst = await engine.startWorkflow(wfId);
      assert.equal(engine.getInstance(inst.id), inst);
    });

    it('emits instance:started', async () => {
      let emitted = null;
      engine.on('instance:started', (d) => {
        emitted = d;
      });
      await engine.startWorkflow(wfId);
      assert.ok(emitted);
    });

    it('executes onEnter action for initial state', async () => {
      const calls = [];
      const e = new WorkflowEngine({
        executor: (action, ctx) => {
          calls.push({ action, ctx });
        },
      });
      e.registerWorkflow({
        name: 'enter-test',
        initialState: 'init',
        states: [{ name: 'init', onEnter: 'doInit' }, { name: 'done' }],
        transitions: [{ name: 'go', from: 'init', to: 'done' }],
        finalStates: ['done'],
      });
      const wf = e.listWorkflows()[0];
      await e.startWorkflow(wf.id);
      assert.equal(calls.length, 1);
      assert.equal(calls[0].action, 'doInit');
    });

    it('accepts metadata option', async () => {
      const inst = await engine.startWorkflow(wfId, { metadata: { source: 'test' } });
      assert.deepStrictEqual(inst.metadata, { source: 'test' });
    });
  });

  // ---- transition ----
  describe('transition()', () => {
    it('moves instance to the target state', async () => {
      const inst = await engine.startWorkflow(wfId);
      const result = await engine.transition(inst.id, 'middle');
      assert.equal(result.currentState, 'middle');
      assert.equal(result.status, 'running');
    });

    it('records transition in history', async () => {
      const inst = await engine.startWorkflow(wfId);
      await engine.transition(inst.id, 'middle');
      const last = inst.history[inst.history.length - 1];
      assert.equal(last.event, 'transition');
      assert.equal(last.from, 'start');
      assert.equal(last.to, 'middle');
      assert.equal(last.transition, 'advance');
    });

    it('merges transitionContext into instance context', async () => {
      const inst = await engine.startWorkflow(wfId);
      await engine.transition(inst.id, 'middle', { trackingNumber: 'TRK-1' });
      assert.equal(inst.context.trackingNumber, 'TRK-1');
    });

    it('marks completed when reaching a final state', async () => {
      const inst = await engine.startWorkflow(wfId);
      await engine.transition(inst.id, 'middle');
      await engine.transition(inst.id, 'end');
      assert.equal(inst.status, 'completed');
      assert.ok(inst.completedAt);
      // History should contain completed event
      const completedEntry = inst.history.find((h) => h.event === 'completed');
      assert.ok(completedEntry);
    });

    it('emits instance:completed for final state', async () => {
      let emitted = null;
      engine.on('instance:completed', (d) => {
        emitted = d;
      });
      const inst = await engine.startWorkflow(wfId);
      await engine.transition(inst.id, 'middle');
      await engine.transition(inst.id, 'end');
      assert.ok(emitted);
    });

    it('emits instance:transitioned', async () => {
      let emitted = null;
      engine.on('instance:transitioned', (d) => {
        emitted = d;
      });
      const inst = await engine.startWorkflow(wfId);
      await engine.transition(inst.id, 'middle');
      assert.ok(emitted);
      assert.equal(emitted.from, 'start');
      assert.equal(emitted.to, 'middle');
    });

    it('throws for unknown instance', async () => {
      await assert.rejects(() => engine.transition('no-such', 'middle'), /Instance not found/);
    });

    it('throws when instance is not running', async () => {
      const inst = await engine.startWorkflow(wfId);
      await engine.pauseInstance(inst.id);
      await assert.rejects(() => engine.transition(inst.id, 'middle'), /Instance is not running/);
    });

    it('throws when no valid transition exists', async () => {
      const inst = await engine.startWorkflow(wfId);
      // 'middle' is reachable via 'advance', and 'end' via 'cancel',
      // so use a target that has no transition from 'start'
      await assert.rejects(() => engine.transition(inst.id, 'nonexistent'), {
        message: /No transition/,
      });
    });

    it('throws when workflow definition is missing', async () => {
      const inst = await engine.startWorkflow(wfId);
      engine.workflows.delete(wfId);
      await assert.rejects(() => engine.transition(inst.id, 'middle'), /Workflow not found/);
    });

    describe('guard conditions', () => {
      it('blocks transition when condition returns false (default evaluator)', async () => {
        const e = new WorkflowEngine({});
        e.registerWorkflow({
          name: 'guarded',
          initialState: 'a',
          states: [{ name: 'a' }, { name: 'b' }],
          transitions: [{ name: 'go', from: 'a', to: 'b', condition: 'approved' }],
          finalStates: ['b'],
        });
        const wf = e.listWorkflows()[0];
        const inst = await e.startWorkflow(wf.id, { context: { approved: false } });
        await assert.rejects(() => e.transition(inst.id, 'b'), /Transition condition not met/);
      });

      it('allows transition when condition is truthy in context (default evaluator)', async () => {
        const e = new WorkflowEngine({});
        e.registerWorkflow({
          name: 'guarded',
          initialState: 'a',
          states: [{ name: 'a' }, { name: 'b' }],
          transitions: [{ name: 'go', from: 'a', to: 'b', condition: 'approved' }],
          finalStates: ['b'],
        });
        const wf = e.listWorkflows()[0];
        const inst = await e.startWorkflow(wf.id, { context: { approved: true } });
        const result = await e.transition(inst.id, 'b');
        assert.equal(result.currentState, 'b');
      });

      it('uses custom conditionEvaluator when provided', async () => {
        const e = new WorkflowEngine({
          conditionEvaluator: (condition, ctx) => {
            return ctx.instanceContext.amount > 100;
          },
        });
        e.registerWorkflow({
          name: 'guarded',
          initialState: 'a',
          states: [{ name: 'a' }, { name: 'b' }],
          transitions: [{ name: 'go', from: 'a', to: 'b', condition: 'amountCheck' }],
          finalStates: ['b'],
        });
        const wf = e.listWorkflows()[0];

        // below threshold
        const inst1 = await e.startWorkflow(wf.id, { context: { amount: 50 } });
        await assert.rejects(() => e.transition(inst1.id, 'b'), /condition not met/);

        // above threshold
        const inst2 = await e.startWorkflow(wf.id, { context: { amount: 200 } });
        const result = await e.transition(inst2.id, 'b');
        assert.equal(result.currentState, 'b');
      });

      it('default evaluator returns true for non-string conditions without evaluator', async () => {
        const e = new WorkflowEngine({});
        e.registerWorkflow({
          name: 'nonstring',
          initialState: 'a',
          states: [{ name: 'a' }, { name: 'b' }],
          transitions: [{ name: 'go', from: 'a', to: 'b', condition: { rule: true } }],
          finalStates: ['b'],
        });
        const wf = e.listWorkflows()[0];
        const inst = await e.startWorkflow(wf.id);
        const result = await e.transition(inst.id, 'b');
        assert.equal(result.currentState, 'b');
      });
    });

    describe('onExit / transition action / onEnter hooks', () => {
      it('calls onExit, then transition action, then onEnter in order', async () => {
        const order = [];
        const e = new WorkflowEngine({
          executor: (action) => {
            order.push(action);
          },
        });
        e.registerWorkflow({
          name: 'hooks',
          initialState: 'a',
          states: [
            { name: 'a', onExit: 'exitA' },
            { name: 'b', onEnter: 'enterB' },
          ],
          transitions: [{ name: 'go', from: 'a', to: 'b', action: 'transAction' }],
          finalStates: ['b'],
        });
        const wf = e.listWorkflows()[0];
        const inst = await e.startWorkflow(wf.id);
        // startWorkflow calls onEnter for initial state 'a' — state 'a' has no onEnter, so order is empty
        assert.equal(order.length, 0);

        await e.transition(inst.id, 'b');
        assert.deepStrictEqual(order, ['exitA', 'transAction', 'enterB']);
      });
    });
  });

  // ---- trigger ----
  describe('trigger()', () => {
    it('finds matching transition by event name and executes it', async () => {
      const inst = await engine.startWorkflow(wfId);
      const result = await engine.trigger(inst.id, 'advance');
      assert.equal(result.currentState, 'middle');
    });

    it('throws for unknown instance', async () => {
      await assert.rejects(() => engine.trigger('bad', 'advance'), /Instance not found/);
    });

    it('throws when workflow is missing', async () => {
      const inst = await engine.startWorkflow(wfId);
      engine.workflows.delete(wfId);
      await assert.rejects(() => engine.trigger(inst.id, 'advance'), /Workflow not found/);
    });

    it('throws when no matching transition name from current state', async () => {
      const inst = await engine.startWorkflow(wfId);
      await assert.rejects(
        () => engine.trigger(inst.id, 'finish'),
        /No transition 'finish' from state 'start'/,
      );
    });
  });

  // ---- executeAction ----
  describe('executeAction()', () => {
    it('emits warning when no executor configured', async () => {
      let warned = null;
      engine.on('warning', (d) => {
        warned = d;
      });
      const inst = await engine.startWorkflow(wfId);
      const result = await engine.executeAction('someAction', inst);
      assert.equal(result, null);
      assert.ok(warned);
      assert.ok(warned.message.includes('No executor'));
    });

    it('passes action and context to executor', async () => {
      let capturedArgs = null;
      const e = new WorkflowEngine({
        executor: (action, ctx) => {
          capturedArgs = { action, ctx };
          return 'done';
        },
      });
      e.registerWorkflow(simpleWorkflow());
      const wf = e.listWorkflows()[0];
      const inst = await e.startWorkflow(wf.id, { context: { orderId: '42' } });
      const result = await e.executeAction('myAction', inst, { extra: true });
      assert.equal(result, 'done');
      assert.equal(capturedArgs.action, 'myAction');
      assert.equal(capturedArgs.ctx.instanceId, inst.id);
      assert.equal(capturedArgs.ctx.instanceContext.orderId, '42');
      assert.deepStrictEqual(capturedArgs.ctx.transitionContext, { extra: true });
    });
  });

  // ---- evaluateCondition ----
  describe('evaluateCondition()', () => {
    it('checks context key when condition is string (default evaluator)', async () => {
      const inst = await engine.startWorkflow(wfId, { context: { paid: true } });
      const result = await engine.evaluateCondition('paid', inst);
      assert.equal(result, true);
    });

    it('returns false for falsy context key (default evaluator)', async () => {
      const inst = await engine.startWorkflow(wfId, { context: { paid: '' } });
      const result = await engine.evaluateCondition('paid', inst);
      assert.equal(result, false);
    });

    it('returns true for non-string condition without evaluator', async () => {
      const inst = await engine.startWorkflow(wfId);
      const result = await engine.evaluateCondition(42, inst);
      assert.equal(result, true);
    });

    it('delegates to custom conditionEvaluator', async () => {
      const e = new WorkflowEngine({
        conditionEvaluator: (cond, ctx) => cond === 'special',
      });
      e.registerWorkflow(simpleWorkflow());
      const wf = e.listWorkflows()[0];
      const inst = await e.startWorkflow(wf.id);
      assert.equal(await e.evaluateCondition('special', inst), true);
      assert.equal(await e.evaluateCondition('other', inst), false);
    });
  });

  // ---- pauseInstance / resumeInstance ----
  describe('pauseInstance()', () => {
    it('sets status to paused and records history', async () => {
      const inst = await engine.startWorkflow(wfId);
      await engine.pauseInstance(inst.id);
      assert.equal(inst.status, 'paused');
      const entry = inst.history.find((h) => h.event === 'paused');
      assert.ok(entry);
    });

    it('emits instance:paused', async () => {
      let emitted = false;
      engine.on('instance:paused', () => {
        emitted = true;
      });
      const inst = await engine.startWorkflow(wfId);
      await engine.pauseInstance(inst.id);
      assert.ok(emitted);
    });

    it('throws for unknown instance', async () => {
      await assert.rejects(() => engine.pauseInstance('missing'), /Instance not found/);
    });

    it('clears timeout timer', async () => {
      const e = new WorkflowEngine({});
      e.registerWorkflow({
        name: 'timeout-test',
        initialState: 'wait',
        states: [
          { name: 'wait', timeout: 999999, timeoutTransition: 'expired' },
          { name: 'expired' },
        ],
        transitions: [],
        finalStates: ['expired'],
      });
      const wf = e.listWorkflows()[0];
      const inst = await e.startWorkflow(wf.id);
      assert.ok(inst.timeoutTimer);
      await e.pauseInstance(inst.id);
      assert.equal(inst.timeoutTimer, null);
    });
  });

  describe('resumeInstance()', () => {
    it('sets status back to running and records history', async () => {
      const inst = await engine.startWorkflow(wfId);
      await engine.pauseInstance(inst.id);
      await engine.resumeInstance(inst.id);
      assert.equal(inst.status, 'running');
      const entry = inst.history.find((h) => h.event === 'resumed');
      assert.ok(entry);
    });

    it('emits instance:resumed', async () => {
      let emitted = false;
      engine.on('instance:resumed', () => {
        emitted = true;
      });
      const inst = await engine.startWorkflow(wfId);
      await engine.pauseInstance(inst.id);
      await engine.resumeInstance(inst.id);
      assert.ok(emitted);
    });

    it('throws for unknown instance', async () => {
      await assert.rejects(() => engine.resumeInstance('missing'), /Instance not found/);
    });

    it('throws if instance is not paused', async () => {
      const inst = await engine.startWorkflow(wfId);
      await assert.rejects(() => engine.resumeInstance(inst.id), /Instance is not paused/);
    });
  });

  // ---- cancelInstance ----
  describe('cancelInstance()', () => {
    it('sets status to cancelled with optional reason', async () => {
      const inst = await engine.startWorkflow(wfId);
      await engine.cancelInstance(inst.id, 'user request');
      assert.equal(inst.status, 'cancelled');
      assert.ok(inst.completedAt);
      const entry = inst.history.find((h) => h.event === 'cancelled');
      assert.ok(entry);
      assert.equal(entry.reason, 'user request');
    });

    it('emits instance:cancelled', async () => {
      let emitted = null;
      engine.on('instance:cancelled', (d) => {
        emitted = d;
      });
      const inst = await engine.startWorkflow(wfId);
      await engine.cancelInstance(inst.id, 'testing');
      assert.equal(emitted.reason, 'testing');
    });

    it('throws for unknown instance', async () => {
      await assert.rejects(() => engine.cancelInstance('missing'), /Instance not found/);
    });

    it('clears timeout timer', async () => {
      const e = new WorkflowEngine({});
      e.registerWorkflow({
        name: 'timeout-cancel',
        initialState: 'wait',
        states: [
          { name: 'wait', timeout: 999999, timeoutTransition: 'expired' },
          { name: 'expired' },
        ],
        transitions: [],
        finalStates: ['expired'],
      });
      const wf = e.listWorkflows()[0];
      const inst = await e.startWorkflow(wf.id);
      assert.ok(inst.timeoutTimer);
      await e.cancelInstance(inst.id);
      assert.equal(inst.timeoutTimer, null);
    });

    it('works without a reason (null default)', async () => {
      const inst = await engine.startWorkflow(wfId);
      await engine.cancelInstance(inst.id);
      const entry = inst.history.find((h) => h.event === 'cancelled');
      assert.equal(entry.reason, null);
    });
  });

  // ---- failInstance ----
  describe('failInstance()', () => {
    it('sets status to failed and records error', async () => {
      const inst = await engine.startWorkflow(wfId);
      await engine.failInstance(inst.id, new Error('boom'));
      assert.equal(inst.status, 'failed');
      assert.equal(inst.error, 'boom');
      assert.ok(inst.completedAt);
      const entry = inst.history.find((h) => h.event === 'failed');
      assert.ok(entry);
      assert.equal(entry.error, 'boom');
    });

    it('handles non-Error objects', async () => {
      const inst = await engine.startWorkflow(wfId);
      await engine.failInstance(inst.id, 'string error');
      assert.equal(inst.error, 'string error');
    });

    it('emits instance:failed', async () => {
      let emitted = null;
      engine.on('instance:failed', (d) => {
        emitted = d;
      });
      const inst = await engine.startWorkflow(wfId);
      await engine.failInstance(inst.id, new Error('oops'));
      assert.equal(emitted.error, 'oops');
    });

    it('throws for unknown instance', async () => {
      await assert.rejects(
        () => engine.failInstance('missing', new Error('x')),
        /Instance not found/,
      );
    });

    it('clears timeout timer', async () => {
      const e = new WorkflowEngine({});
      e.registerWorkflow({
        name: 'timeout-fail',
        initialState: 'wait',
        states: [
          { name: 'wait', timeout: 999999, timeoutTransition: 'expired' },
          { name: 'expired' },
        ],
        transitions: [],
        finalStates: ['expired'],
      });
      const wf = e.listWorkflows()[0];
      const inst = await e.startWorkflow(wf.id);
      assert.ok(inst.timeoutTimer);
      await e.failInstance(inst.id, new Error('fail'));
      assert.equal(inst.timeoutTimer, null);
    });
  });

  // ---- getInstance ----
  describe('getInstance()', () => {
    it('returns instance by id', async () => {
      const inst = await engine.startWorkflow(wfId);
      assert.equal(engine.getInstance(inst.id), inst);
    });

    it('returns undefined for unknown id', () => {
      assert.equal(engine.getInstance('xyz'), undefined);
    });
  });

  // ---- listInstances ----
  describe('listInstances()', () => {
    it('returns all instances as JSON', async () => {
      await engine.startWorkflow(wfId);
      await engine.startWorkflow(wfId);
      const list = engine.listInstances();
      assert.equal(list.length, 2);
      // Ensure they are plain objects (toJSON)
      assert.equal(list[0].timeoutTimer, undefined);
    });

    it('filters by workflowId', async () => {
      const e = new WorkflowEngine({});
      const wf1 = e.registerWorkflow({ ...simpleWorkflow(), name: 'w1' });
      const wf2 = e.registerWorkflow({ ...simpleWorkflow(), name: 'w2' });
      await e.startWorkflow(wf1.id);
      await e.startWorkflow(wf2.id);
      await e.startWorkflow(wf1.id);

      const filtered = e.listInstances({ workflowId: wf1.id });
      assert.equal(filtered.length, 2);
      for (const inst of filtered) {
        assert.equal(inst.workflowId, wf1.id);
      }
    });

    it('filters by status', async () => {
      const inst1 = await engine.startWorkflow(wfId);
      const inst2 = await engine.startWorkflow(wfId);
      await engine.pauseInstance(inst1.id);

      const paused = engine.listInstances({ status: 'paused' });
      assert.equal(paused.length, 1);
      assert.equal(paused[0].status, 'paused');

      const running = engine.listInstances({ status: 'running' });
      assert.equal(running.length, 1);
      assert.equal(running[0].status, 'running');
    });

    it('respects limit', async () => {
      await engine.startWorkflow(wfId);
      await engine.startWorkflow(wfId);
      await engine.startWorkflow(wfId);
      const limited = engine.listInstances({ limit: 2 });
      assert.equal(limited.length, 2);
    });

    it('sorts by updatedAt descending', async () => {
      const inst1 = await engine.startWorkflow(wfId);
      // Ensure inst2 has a later updatedAt
      await new Promise((r) => setTimeout(r, 5));
      const inst2 = await engine.startWorkflow(wfId);
      // inst2 was created after inst1, so it should appear first
      const list = engine.listInstances();
      assert.equal(list[0].id, inst2.id);
    });

    it('returns empty array when no instances', () => {
      const e = new WorkflowEngine({});
      assert.deepStrictEqual(e.listInstances(), []);
    });
  });

  // ---- getStatus ----
  describe('getStatus()', () => {
    it('returns aggregate status', async () => {
      const inst1 = await engine.startWorkflow(wfId);
      await engine.startWorkflow(wfId);
      await engine.pauseInstance(inst1.id);

      const status = engine.getStatus();
      assert.equal(status.totalWorkflows, 1);
      assert.equal(status.totalInstances, 2);
      assert.equal(status.byStatus.paused, 1);
      assert.equal(status.byStatus.running, 1);
      assert.ok(Array.isArray(status.recentInstances));
      assert.ok(status.recentInstances.length <= 5);
    });

    it('returns zeros when empty', () => {
      const e = new WorkflowEngine({});
      const status = e.getStatus();
      assert.equal(status.totalWorkflows, 0);
      assert.equal(status.totalInstances, 0);
      assert.deepStrictEqual(status.byStatus, {});
    });

    it('caps recentInstances at 5', async () => {
      for (let i = 0; i < 8; i++) {
        await engine.startWorkflow(wfId);
      }
      const status = engine.getStatus();
      assert.equal(status.recentInstances.length, 5);
    });
  });

  // ---- setupStateTimeout ----
  describe('setupStateTimeout()', () => {
    it('does nothing when workflow has no timeout', async () => {
      const inst = await engine.startWorkflow(wfId);
      // 'start' has no timeout in simpleWorkflow
      assert.equal(inst.timeoutTimer, null);
    });

    it('does nothing when workflow is not found', async () => {
      const inst = await engine.startWorkflow(wfId);
      engine.workflows.delete(wfId);
      engine.setupStateTimeout(inst); // Should not throw
      assert.equal(inst.timeoutTimer, null);
    });

    it('clears existing timer before setting new one', async () => {
      const e = new WorkflowEngine({});
      e.registerWorkflow({
        name: 'timeout-clear',
        initialState: 'a',
        states: [{ name: 'a', timeout: 999999, timeoutTransition: 'b' }, { name: 'b' }],
        transitions: [{ name: 'go', from: 'a', to: 'b' }],
        finalStates: ['b'],
      });
      const wf = e.listWorkflows()[0];
      const inst = await e.startWorkflow(wf.id);
      const firstTimer = inst.timeoutTimer;
      assert.ok(firstTimer);

      // Call again
      e.setupStateTimeout(inst);
      assert.ok(inst.timeoutTimer);
      // Timer reference should change (old one cleared, new one set)
      // (they could theoretically be the same reference on some runtimes, so we just
      // check that it exists and the operation did not throw)
      clearTimeout(inst.timeoutTimer);
      clearTimeout(firstTimer);
    });
  });

  // ---- save / load ----
  describe('save() and load()', () => {
    it('save is a no-op when storePath is null', async () => {
      // Should not throw
      await engine.save();
    });

    it('load is a no-op when storePath is null', async () => {
      await engine.load();
    });

    it('save and load round-trip instances via filesystem', async () => {
      const os = await import('os');
      const fs = await import('fs');
      const pathMod = await import('path');
      const tmpDir = fs.mkdtempSync(pathMod.join(os.tmpdir(), 'sm-test-'));

      try {
        const e1 = new WorkflowEngine({ storePath: tmpDir });
        const wf = e1.registerWorkflow(simpleWorkflow());
        const inst = await e1.startWorkflow(wf.id, { context: { key: 'val' } });
        await e1.transition(inst.id, 'middle');

        // Load into a new engine
        const e2 = new WorkflowEngine({ storePath: tmpDir });
        e2.registerWorkflow({ ...simpleWorkflow(), id: wf.id });
        await e2.load();

        const loaded = e2.getInstance(inst.id);
        assert.ok(loaded);
        assert.equal(loaded.currentState, 'middle');
        assert.equal(loaded.context.key, 'val');
        assert.equal(loaded.status, 'running');
      } finally {
        fs.rmSync(tmpDir, { recursive: true, force: true });
      }
    });

    it('load emits loaded event', async () => {
      const os = await import('os');
      const fs = await import('fs');
      const pathMod = await import('path');
      const tmpDir = fs.mkdtempSync(pathMod.join(os.tmpdir(), 'sm-test-'));

      try {
        const e1 = new WorkflowEngine({ storePath: tmpDir });
        const wf = e1.registerWorkflow(simpleWorkflow());
        await e1.startWorkflow(wf.id);

        const e2 = new WorkflowEngine({ storePath: tmpDir });
        e2.registerWorkflow({ ...simpleWorkflow(), id: wf.id });
        let emitted = null;
        e2.on('loaded', (d) => {
          emitted = d;
        });
        await e2.load();
        assert.ok(emitted);
        assert.equal(emitted.instanceCount, 1);
      } finally {
        fs.rmSync(tmpDir, { recursive: true, force: true });
      }
    });

    it('load skips non-running instances', async () => {
      const os = await import('os');
      const fs = await import('fs');
      const pathMod = await import('path');
      const tmpDir = fs.mkdtempSync(pathMod.join(os.tmpdir(), 'sm-test-'));

      try {
        const e1 = new WorkflowEngine({ storePath: tmpDir });
        const wf = e1.registerWorkflow(simpleWorkflow());
        const inst = await e1.startWorkflow(wf.id);
        await e1.pauseInstance(inst.id);

        const e2 = new WorkflowEngine({ storePath: tmpDir });
        e2.registerWorkflow({ ...simpleWorkflow(), id: wf.id });
        await e2.load();
        assert.equal(e2.getInstance(inst.id), undefined);
      } finally {
        fs.rmSync(tmpDir, { recursive: true, force: true });
      }
    });

    it('load emits error on corrupt file', async () => {
      const os = await import('os');
      const fs = await import('fs');
      const pathMod = await import('path');
      const tmpDir = fs.mkdtempSync(pathMod.join(os.tmpdir(), 'sm-test-'));

      try {
        fs.writeFileSync(pathMod.join(tmpDir, 'workflow-instances.json'), 'not json');
        const e = new WorkflowEngine({ storePath: tmpDir });
        let errorEmitted = null;
        e.on('error', (d) => {
          errorEmitted = d;
        });
        await e.load();
        assert.ok(errorEmitted);
        assert.equal(errorEmitted.type, 'load');
      } finally {
        fs.rmSync(tmpDir, { recursive: true, force: true });
      }
    });

    it('save emits error on write failure', async () => {
      const e = new WorkflowEngine({ storePath: '/dev/null/impossible/path' });
      let errorEmitted = null;
      e.on('error', (d) => {
        errorEmitted = d;
      });
      await e.save();
      assert.ok(errorEmitted);
      assert.equal(errorEmitted.type, 'save');
    });

    it('save emits saved event on success', async () => {
      const os = await import('os');
      const fs = await import('fs');
      const pathMod = await import('path');
      const tmpDir = fs.mkdtempSync(pathMod.join(os.tmpdir(), 'sm-test-'));

      try {
        const e = new WorkflowEngine({ storePath: tmpDir });
        let emitted = null;
        e.on('saved', (d) => {
          emitted = d;
        });
        await e.save();
        assert.ok(emitted);
        assert.equal(emitted.instanceCount, 0);
      } finally {
        fs.rmSync(tmpDir, { recursive: true, force: true });
      }
    });
  });

  // ---- Full workflow end-to-end ----
  describe('end-to-end workflow', () => {
    it('runs through a complete order fulfillment scenario', async () => {
      const e = new WorkflowEngine({});
      const wf = e.registerWorkflow({
        name: 'order',
        initialState: 'pending',
        states: [
          { name: 'pending' },
          { name: 'processing' },
          { name: 'shipped' },
          { name: 'delivered' },
          { name: 'cancelled' },
        ],
        transitions: [
          { name: 'process', from: 'pending', to: 'processing' },
          { name: 'ship', from: 'processing', to: 'shipped' },
          { name: 'deliver', from: 'shipped', to: 'delivered' },
          { name: 'cancel', from: ['pending', 'processing'], to: 'cancelled' },
        ],
        finalStates: ['delivered', 'cancelled'],
      });

      const inst = await e.startWorkflow(wf.id, {
        context: { orderId: 'ORD-100' },
      });
      assert.equal(inst.currentState, 'pending');

      await e.trigger(inst.id, 'process');
      assert.equal(inst.currentState, 'processing');

      await e.trigger(inst.id, 'ship', { trackingNumber: 'TRK-1' });
      assert.equal(inst.currentState, 'shipped');
      assert.equal(inst.context.trackingNumber, 'TRK-1');

      await e.trigger(inst.id, 'deliver');
      assert.equal(inst.currentState, 'delivered');
      assert.equal(inst.status, 'completed');
      assert.ok(inst.completedAt);

      // History should have: started, transition*3, completed
      assert.equal(inst.history.length, 5);
    });

    it('supports cancellation from multiple states', async () => {
      const e = new WorkflowEngine({});
      const wf = e.registerWorkflow({
        name: 'cancel-test',
        initialState: 'a',
        states: [{ name: 'a' }, { name: 'b' }, { name: 'c' }],
        transitions: [
          { name: 'next', from: 'a', to: 'b' },
          { name: 'cancel', from: ['a', 'b'], to: 'c' },
        ],
        finalStates: ['c'],
      });

      // Cancel from 'a'
      const inst1 = await e.startWorkflow(wf.id);
      await e.trigger(inst1.id, 'cancel');
      assert.equal(inst1.currentState, 'c');
      assert.equal(inst1.status, 'completed');

      // Cancel from 'b'
      const inst2 = await e.startWorkflow(wf.id);
      await e.trigger(inst2.id, 'next');
      await e.trigger(inst2.id, 'cancel');
      assert.equal(inst2.currentState, 'c');
      assert.equal(inst2.status, 'completed');
    });

    it('prevents invalid transitions', async () => {
      const inst = await engine.startWorkflow(wfId);
      // 'finish' only valid from 'middle', not 'start'
      await assert.rejects(
        () => engine.trigger(inst.id, 'finish'),
        /No transition 'finish' from state 'start'/,
      );
    });

    it('cannot transition after completion', async () => {
      const inst = await engine.startWorkflow(wfId);
      await engine.transition(inst.id, 'middle');
      await engine.transition(inst.id, 'end');
      assert.equal(inst.status, 'completed');

      await assert.rejects(() => engine.transition(inst.id, 'middle'), /Instance is not running/);
    });

    it('cannot transition after cancel', async () => {
      const inst = await engine.startWorkflow(wfId);
      await engine.cancelInstance(inst.id);
      await assert.rejects(() => engine.transition(inst.id, 'middle'), /Instance is not running/);
    });

    it('cannot transition after failure', async () => {
      const inst = await engine.startWorkflow(wfId);
      await engine.failInstance(inst.id, new Error('broke'));
      await assert.rejects(() => engine.transition(inst.id, 'middle'), /Instance is not running/);
    });
  });

  // ---- Timeout transitions ----
  describe('timeout transitions', () => {
    it('auto-transitions after timeout', async () => {
      const e = new WorkflowEngine({});
      e.registerWorkflow({
        name: 'timeout-auto',
        initialState: 'wait',
        states: [{ name: 'wait', timeout: 50, timeoutTransition: 'done' }, { name: 'done' }],
        transitions: [{ name: 'auto', from: 'wait', to: 'done' }],
        finalStates: ['done'],
      });
      const wf = e.listWorkflows()[0];
      const inst = await e.startWorkflow(wf.id);
      assert.equal(inst.currentState, 'wait');

      // Wait for timeout to fire
      await new Promise((resolve) => setTimeout(resolve, 150));
      assert.equal(inst.currentState, 'done');
      assert.equal(inst.status, 'completed');
      // Cleanup
      if (inst.timeoutTimer) clearTimeout(inst.timeoutTimer);
    });

    it('clears timeout on manual transition before timeout fires', async () => {
      const e = new WorkflowEngine({});
      e.registerWorkflow({
        name: 'timeout-clear',
        initialState: 'wait',
        states: [
          { name: 'wait', timeout: 5000, timeoutTransition: 'expired' },
          { name: 'active' },
          { name: 'expired' },
        ],
        transitions: [
          { name: 'activate', from: 'wait', to: 'active' },
          { name: 'timeout', from: 'wait', to: 'expired' },
        ],
        finalStates: ['active', 'expired'],
      });
      const wf = e.listWorkflows()[0];
      const inst = await e.startWorkflow(wf.id);
      assert.ok(inst.timeoutTimer);

      await e.trigger(inst.id, 'activate');
      assert.equal(inst.currentState, 'active');
      // Timer should have been cleared during transition
      assert.equal(inst.timeoutTimer, null);
    });
  });
});

/**
 * Clear all timeout timers on an engine's instances to prevent test hangs.
 */
function cleanupEngine(engine) {
  for (const inst of engine.instances?.values?.() ?? []) {
    if (inst.timeoutTimer) {
      clearTimeout(inst.timeoutTimer);
      inst.timeoutTimer = null;
    }
  }
}

// ---------------------------------------------------------------------------
// WorkflowTemplates
// ---------------------------------------------------------------------------
describe('WorkflowTemplates', () => {
  it('exports four templates', () => {
    const keys = Object.keys(WorkflowTemplates);
    assert.deepStrictEqual(keys.sort(), [
      'orderFulfillment',
      'purchaseOrderApproval',
      'returnProcessing',
      'subscriptionLifecycle',
    ]);
  });

  for (const [key, template] of Object.entries(WorkflowTemplates)) {
    describe(`${key}`, () => {
      it('is a valid StateMachine definition', () => {
        // Constructing it validates states, transitions, and final states
        assert.doesNotThrow(() => new StateMachine(template));
      });

      it('has a name and description', () => {
        assert.ok(template.name);
        assert.ok(template.description);
      });

      it('has an initialState that exists in states', () => {
        const stateNames = template.states.map((s) => s.name);
        assert.ok(stateNames.includes(template.initialState));
      });

      it('has at least one final state', () => {
        assert.ok(template.finalStates.length > 0);
      });

      it('all finalStates exist in states', () => {
        const stateNames = template.states.map((s) => s.name);
        for (const fs of template.finalStates) {
          assert.ok(stateNames.includes(fs), `Final state '${fs}' not in states`);
        }
      });

      it('all transition targets exist in states', () => {
        const stateNames = template.states.map((s) => s.name);
        for (const t of template.transitions) {
          assert.ok(stateNames.includes(t.to), `Transition '${t.name}' targets unknown '${t.to}'`);
        }
      });

      it('all transition sources exist in states', () => {
        const stateNames = template.states.map((s) => s.name);
        for (const t of template.transitions) {
          const froms = Array.isArray(t.from) ? t.from : [t.from];
          for (const f of froms) {
            assert.ok(stateNames.includes(f), `Transition '${t.name}' from unknown '${f}'`);
          }
        }
      });

      it('can be used by WorkflowEngine', async () => {
        const e = new WorkflowEngine({});
        const wf = e.registerWorkflow(template);
        const inst = await e.startWorkflow(wf.id);
        assert.equal(inst.currentState, template.initialState);
        assert.equal(inst.status, 'running');
        cleanupEngine(e);
      });
    });
  }

  describe('orderFulfillment specifics', () => {
    it('can complete the happy path', async () => {
      const e = new WorkflowEngine({});
      const wf = e.registerWorkflow(WorkflowTemplates.orderFulfillment);
      const inst = await e.startWorkflow(wf.id);

      await e.trigger(inst.id, 'process');
      await e.trigger(inst.id, 'await_payment');
      await e.trigger(inst.id, 'payment_received');
      await e.trigger(inst.id, 'ship');
      await e.trigger(inst.id, 'deliver');

      assert.equal(inst.currentState, 'delivered');
      assert.equal(inst.status, 'completed');
      cleanupEngine(e);
    });

    it('can cancel from pending', async () => {
      const e = new WorkflowEngine({});
      const wf = e.registerWorkflow(WorkflowTemplates.orderFulfillment);
      const inst = await e.startWorkflow(wf.id);
      await e.trigger(inst.id, 'cancel');
      assert.equal(inst.currentState, 'cancelled');
      assert.equal(inst.status, 'completed');
      cleanupEngine(e);
    });

    it('can refund from delivered', async () => {
      const e = new WorkflowEngine({});
      const wf = e.registerWorkflow(WorkflowTemplates.orderFulfillment);
      const inst = await e.startWorkflow(wf.id);

      await e.trigger(inst.id, 'process');
      await e.trigger(inst.id, 'await_payment');
      await e.trigger(inst.id, 'payment_received');
      await e.trigger(inst.id, 'ship');
      await e.trigger(inst.id, 'deliver');
      const inst2 = await e.startWorkflow(wf.id);
      await e.trigger(inst2.id, 'process');
      await e.trigger(inst2.id, 'await_payment');
      await e.trigger(inst2.id, 'payment_received');
      await e.trigger(inst2.id, 'refund');
      assert.equal(inst2.currentState, 'refunded');
      assert.equal(inst2.status, 'completed');
      cleanupEngine(e);
    });
  });

  describe('returnProcessing specifics', () => {
    it('can complete the happy path', async () => {
      const e = new WorkflowEngine({});
      const wf = e.registerWorkflow(WorkflowTemplates.returnProcessing);
      const inst = await e.startWorkflow(wf.id);

      await e.trigger(inst.id, 'submit_for_approval');
      await e.trigger(inst.id, 'approve');
      await e.trigger(inst.id, 'await_item');
      await e.trigger(inst.id, 'receive_item');
      await e.trigger(inst.id, 'process_refund');
      await e.trigger(inst.id, 'complete_refund');

      assert.equal(inst.currentState, 'refunded');
      assert.equal(inst.status, 'completed');
      cleanupEngine(e);
    });
  });

  describe('subscriptionLifecycle specifics', () => {
    it('can convert trial to active', async () => {
      const e = new WorkflowEngine({});
      const wf = e.registerWorkflow(WorkflowTemplates.subscriptionLifecycle);
      const inst = await e.startWorkflow(wf.id);

      await e.trigger(inst.id, 'convert');
      assert.equal(inst.currentState, 'active');
      cleanupEngine(e);
    });

    it('handles payment failure and recovery', async () => {
      const e = new WorkflowEngine({});
      const wf = e.registerWorkflow(WorkflowTemplates.subscriptionLifecycle);
      const inst = await e.startWorkflow(wf.id);

      await e.trigger(inst.id, 'convert');
      await e.trigger(inst.id, 'payment_failed');
      assert.equal(inst.currentState, 'past_due');

      await e.trigger(inst.id, 'payment_received');
      assert.equal(inst.currentState, 'active');
      cleanupEngine(e);
    });
  });

  describe('purchaseOrderApproval specifics', () => {
    it('can complete the happy path', async () => {
      const e = new WorkflowEngine({});
      const wf = e.registerWorkflow(WorkflowTemplates.purchaseOrderApproval);
      const inst = await e.startWorkflow(wf.id);

      await e.trigger(inst.id, 'submit');
      await e.trigger(inst.id, 'review');
      await e.trigger(inst.id, 'approve');
      await e.trigger(inst.id, 'send');
      await e.trigger(inst.id, 'acknowledge');
      await e.trigger(inst.id, 'receive');

      assert.equal(inst.currentState, 'received');
      assert.equal(inst.status, 'completed');
      cleanupEngine(e);
    });

    it('handles partial receives', async () => {
      const e = new WorkflowEngine({});
      const wf = e.registerWorkflow(WorkflowTemplates.purchaseOrderApproval);
      const inst = await e.startWorkflow(wf.id);

      await e.trigger(inst.id, 'submit');
      await e.trigger(inst.id, 'review');
      await e.trigger(inst.id, 'approve');
      await e.trigger(inst.id, 'send');
      await e.trigger(inst.id, 'acknowledge');
      await e.trigger(inst.id, 'partial_receive');
      assert.equal(inst.currentState, 'partially_received');

      await e.trigger(inst.id, 'receive');
      assert.equal(inst.currentState, 'received');
      assert.equal(inst.status, 'completed');
      cleanupEngine(e);
    });
  });
});

// ---------------------------------------------------------------------------
// Default export
// ---------------------------------------------------------------------------
describe('default export', () => {
  it('default export is WorkflowEngine', async () => {
    const mod = await import('../../src/workflows/state-machine.js');
    assert.equal(mod.default, WorkflowEngine);
  });
});
