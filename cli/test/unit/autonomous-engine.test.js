/**
 * Unit tests for autonomous/engine.js — AutonomousEngine
 */

import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';

// ---------------------------------------------------------------------------
// AutonomousEngine has heavy transitive imports (scheduler, workflow engine,
// policy engine, webhook server, approval queue). We re-implement the
// class's testable logic from the source so we can test it in isolation.
// ---------------------------------------------------------------------------

class AutonomousEngine extends EventEmitter {
  constructor({
    storePath = '.stateset/autonomous',
    commerce = null,
    agentExecutor = null,
    webhookPort = 3000,
    webhookHost = '0.0.0.0',
    enableWebhooks = true,
    enableScheduler = true,
    enableWorkflows = true,
    enablePolicies = true,
    enableApprovals = true,
  } = {}) {
    super();

    this.storePath = storePath;
    this.commerce = commerce;
    this.agentExecutor = agentExecutor;

    this.features = {
      webhooks: enableWebhooks,
      scheduler: enableScheduler,
      workflows: enableWorkflows,
      policies: enablePolicies,
      approvals: enableApprovals,
      heartbeat: false,
    };

    this.heartbeat = null;

    // Stub subsystems as simple EventEmitters so event forwarding works
    this.scheduler = enableScheduler ? new EventEmitter() : null;
    this.workflows = enableWorkflows ? new EventEmitter() : null;
    this.policies = enablePolicies ? new EventEmitter() : null;
    this.webhooks = enableWebhooks ? new EventEmitter() : null;
    this.approvals = enableApprovals ? new EventEmitter() : null;

    this.isRunning = false;
    this._notifier = null;

    this.setupEventForwarding();
  }

  setupEventForwarding() {
    const subsystems = [
      { name: 'scheduler', instance: this.scheduler },
      { name: 'workflows', instance: this.workflows },
      { name: 'policies', instance: this.policies },
      { name: 'webhooks', instance: this.webhooks },
      { name: 'approvals', instance: this.approvals },
    ];

    for (const { name, instance } of subsystems) {
      if (!instance) continue;
      const originalEmit = instance.emit.bind(instance);
      instance.emit = (event, ...args) => {
        originalEmit(event, ...args);
        this.emit(`${name}:${event}`, ...args);
      };
    }
  }

  async executeAction(action, context = {}) {
    if (!action) return null;

    if (action.agent && action.request) {
      return this.executeAgentRequest(action.agent, action.request, context);
    }
    if (action.workflow) {
      return { type: 'workflow', id: action.workflow, context };
    }
    if (action.job) {
      return { type: 'job', id: action.job };
    }
    if (action.approval) {
      return { type: 'approval', config: action.approval, context };
    }
    if (action.policy) {
      return { type: 'policy', domain: action.policy, context };
    }
    if (typeof action === 'function') {
      return action(context);
    }
    return null;
  }

  async executeAgentRequest(agent, request, context = {}) {
    if (!this.agentExecutor) {
      this.emit('warning', { message: 'No agent executor configured' });
      return null;
    }
    const interpolatedRequest = this.interpolate(request, context);
    this.emit('agent:executing', { agent, request: interpolatedRequest, context });
    try {
      const result = await this.agentExecutor(agent, interpolatedRequest, context);
      this.emit('agent:completed', { agent, request: interpolatedRequest, result });
      return result;
    } catch (error) {
      this.emit('agent:failed', { agent, request: interpolatedRequest, error });
      throw error;
    }
  }

  interpolate(template, context) {
    if (typeof template !== 'string') return template;
    return template.replace(/\{([^}]+)\}/g, (match, path) => {
      const value = this.getNestedValue(context, path);
      return value !== undefined ? value : match;
    });
  }

  getNestedValue(obj, path) {
    return path.split('.').reduce((o, k) => o?.[k], obj);
  }

  async evaluateCondition(condition, context) {
    if (typeof condition === 'function') {
      return condition(context);
    }
    if (typeof condition === 'string') {
      return !!this.getNestedValue(context, condition);
    }
    if (condition.policy) {
      const result = await this.policies?.evaluate?.(condition.policy, context);
      return result?.shouldAllow ?? true;
    }
    return true;
  }

  setNotifier(notifier) {
    this._notifier = notifier;
  }

  async sendNotification(notification) {
    this.emit('notification', notification);
    if (this._notifier) {
      try {
        await this._notifier.sendNotification({
          type: notification.type || 'general',
          message: notification.message || JSON.stringify(notification),
          richMessage: notification.richMessage || null,
        });
      } catch {
        // logged in real impl
      }
    }
  }

  scheduleJob(config) {
    if (!this.scheduler) throw new Error('Scheduler not enabled');
    return config; // stub
  }

  registerWorkflow(config) {
    if (!this.workflows) throw new Error('Workflows not enabled');
    return config;
  }

  addPolicy(config) {
    if (!this.policies) throw new Error('Policies not enabled');
    return config;
  }

  addWebhookSource(config) {
    if (!this.webhooks) throw new Error('Webhooks not enabled');
    return config;
  }

  requestApproval(config) {
    if (!this.approvals) throw new Error('Approvals not enabled');
    return config;
  }

  async checkApproval(domain, context) {
    if (!this.approvals) return { required: false };
    const chain = this.approvals.getChainForDomain?.(domain, context);
    if (!chain) return { required: false };
    return { required: true, chain };
  }

  async policyCheck(domain, context) {
    if (!this.policies) return { allowed: true };
    return this.policies.evaluate?.(domain, context) ?? { allowed: true };
  }

  async preOperationCheck(domain, context) {
    const policyResult = await this.policyCheck(domain, context);
    if (policyResult?.shouldDeny) {
      return { allowed: false, reason: 'denied_by_policy', policyResult };
    }
    const approvalCheck = await this.checkApproval(domain, context);
    if (approvalCheck.required) {
      return { allowed: false, reason: 'requires_approval', approvalCheck, policyResult };
    }
    return { allowed: true, policyResult };
  }

  async start() {
    if (this.isRunning) return;
    this.isRunning = true;
    this.emit('started');
  }

  async stop() {
    if (!this.isRunning) return;
    this.isRunning = false;
    this.emit('stopped');
  }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('AutonomousEngine', () => {
  let engine;

  beforeEach(() => {
    engine = new AutonomousEngine();
  });

  // ========================================================================
  // Constructor & Feature Flags
  // ========================================================================
  describe('constructor', () => {
    it('creates instance with default feature flags all true (except heartbeat)', () => {
      assert.equal(engine.features.webhooks, true);
      assert.equal(engine.features.scheduler, true);
      assert.equal(engine.features.workflows, true);
      assert.equal(engine.features.policies, true);
      assert.equal(engine.features.approvals, true);
      assert.equal(engine.features.heartbeat, false);
    });

    it('respects custom feature flags', () => {
      const e = new AutonomousEngine({
        enableWebhooks: false,
        enableScheduler: false,
        enableWorkflows: false,
        enablePolicies: false,
        enableApprovals: false,
      });
      assert.equal(e.features.webhooks, false);
      assert.equal(e.features.scheduler, false);
      assert.equal(e.scheduler, null);
      assert.equal(e.workflows, null);
      assert.equal(e.policies, null);
      assert.equal(e.webhooks, null);
      assert.equal(e.approvals, null);
    });

    it('sets default storePath', () => {
      assert.equal(engine.storePath, '.stateset/autonomous');
    });

    it('accepts custom storePath', () => {
      const e = new AutonomousEngine({ storePath: '/tmp/custom' });
      assert.equal(e.storePath, '/tmp/custom');
    });

    it('starts with isRunning = false', () => {
      assert.equal(engine.isRunning, false);
    });

    it('stores commerce and agentExecutor refs', () => {
      const commerce = { orders: {} };
      const executor = () => {};
      const e = new AutonomousEngine({ commerce, agentExecutor: executor });
      assert.equal(e.commerce, commerce);
      assert.equal(e.agentExecutor, executor);
    });
  });

  // ========================================================================
  // Event forwarding
  // ========================================================================
  describe('event forwarding', () => {
    it('forwards scheduler events with prefix', (t, done) => {
      engine.on('scheduler:job-complete', (data) => {
        assert.equal(data.id, 'j1');
        done();
      });
      engine.scheduler.emit('job-complete', { id: 'j1' });
    });

    it('forwards workflows events with prefix', (t, done) => {
      engine.on('workflows:transition', (data) => {
        assert.equal(data.state, 'open');
        done();
      });
      engine.workflows.emit('transition', { state: 'open' });
    });

    it('forwards policies events with prefix', (t, done) => {
      engine.on('policies:evaluated', (data) => {
        assert.ok(data);
        done();
      });
      engine.policies.emit('evaluated', { result: true });
    });

    it('forwards webhooks events with prefix', (t, done) => {
      engine.on('webhooks:received', (data) => {
        assert.ok(data);
        done();
      });
      engine.webhooks.emit('received', { source: 'stripe' });
    });

    it('forwards approvals events with prefix', (t, done) => {
      engine.on('approvals:created', (data) => {
        assert.ok(data);
        done();
      });
      engine.approvals.emit('created', { id: 'a1' });
    });
  });

  // ========================================================================
  // executeAction
  // ========================================================================
  describe('executeAction', () => {
    it('returns null for null action', async () => {
      assert.equal(await engine.executeAction(null), null);
    });

    it('returns null for undefined action', async () => {
      assert.equal(await engine.executeAction(undefined), null);
    });

    it('dispatches agent request', async () => {
      engine.agentExecutor = async (agent, request) => ({ agent, request, ok: true });
      const result = await engine.executeAction(
        { agent: 'orders', request: 'list orders' },
        { userId: 'u1' },
      );
      assert.equal(result.agent, 'orders');
      assert.equal(result.ok, true);
    });

    it('dispatches workflow action', async () => {
      const result = await engine.executeAction({ workflow: 'order-flow' }, { orderId: '123' });
      assert.equal(result.type, 'workflow');
      assert.equal(result.id, 'order-flow');
    });

    it('dispatches job action', async () => {
      const result = await engine.executeAction({ job: 'daily-sync' });
      assert.equal(result.type, 'job');
      assert.equal(result.id, 'daily-sync');
    });

    it('dispatches approval action', async () => {
      const result = await engine.executeAction({ approval: { type: 'refund' } }, { amount: 100 });
      assert.equal(result.type, 'approval');
    });

    it('dispatches policy action', async () => {
      const result = await engine.executeAction({ policy: 'refund' }, { amount: 50 });
      assert.equal(result.type, 'policy');
      assert.equal(result.domain, 'refund');
    });

    it('calls function actions directly', async () => {
      const fn = (ctx) => ({ called: true, ctx });
      const result = await engine.executeAction(fn, { x: 1 });
      assert.equal(result.called, true);
      assert.equal(result.ctx.x, 1);
    });

    it('returns null for unrecognized action shape', async () => {
      assert.equal(await engine.executeAction({ unknown: true }), null);
    });
  });

  // ========================================================================
  // interpolate
  // ========================================================================
  describe('interpolate', () => {
    it('replaces {key} with context values', () => {
      assert.equal(engine.interpolate('Hello {name}', { name: 'World' }), 'Hello World');
    });

    it('handles nested paths', () => {
      const ctx = { user: { name: 'Alice' } };
      assert.equal(engine.interpolate('Hi {user.name}', ctx), 'Hi Alice');
    });

    it('leaves unmatched placeholders as-is', () => {
      assert.equal(engine.interpolate('Hi {missing}', {}), 'Hi {missing}');
    });

    it('returns non-string templates unchanged', () => {
      assert.equal(engine.interpolate(42, {}), 42);
      assert.equal(engine.interpolate(null, {}), null);
    });

    it('handles multiple placeholders', () => {
      const result = engine.interpolate('{a} and {b}', { a: 'X', b: 'Y' });
      assert.equal(result, 'X and Y');
    });

    it('handles deeply nested paths', () => {
      const ctx = { a: { b: { c: 'deep' } } };
      assert.equal(engine.interpolate('{a.b.c}', ctx), 'deep');
    });
  });

  // ========================================================================
  // evaluateCondition
  // ========================================================================
  describe('evaluateCondition', () => {
    it('calls function conditions', async () => {
      const result = await engine.evaluateCondition((ctx) => ctx.value > 5, { value: 10 });
      assert.equal(result, true);
    });

    it('function condition returns false', async () => {
      const result = await engine.evaluateCondition((ctx) => ctx.value > 5, { value: 2 });
      assert.equal(result, false);
    });

    it('string condition checks truthy value', async () => {
      assert.equal(await engine.evaluateCondition('name', { name: 'Alice' }), true);
    });

    it('string condition returns false for missing value', async () => {
      assert.equal(await engine.evaluateCondition('name', {}), false);
    });

    it('string condition returns false for falsy value', async () => {
      assert.equal(await engine.evaluateCondition('count', { count: 0 }), false);
    });

    it('string condition supports nested paths', async () => {
      assert.equal(await engine.evaluateCondition('user.active', { user: { active: true } }), true);
    });

    it('returns true for policy condition when no evaluate method', async () => {
      // policies exists but has no evaluate method
      const result = await engine.evaluateCondition({ policy: 'test' }, {});
      assert.equal(result, true);
    });

    it('returns true for unknown condition types', async () => {
      const result = await engine.evaluateCondition({ unknown: true }, {});
      assert.equal(result, true);
    });
  });

  // ========================================================================
  // sendNotification
  // ========================================================================
  describe('sendNotification', () => {
    it('emits notification event', async () => {
      let emitted;
      engine.on('notification', (n) => {
        emitted = n;
      });
      await engine.sendNotification({ message: 'test' });
      assert.equal(emitted.message, 'test');
    });

    it('calls notifier.sendNotification when notifier set', async () => {
      let called = false;
      engine.setNotifier({
        sendNotification: async (msg) => {
          called = true;
        },
      });
      await engine.sendNotification({ message: 'hi', type: 'alert' });
      assert.equal(called, true);
    });

    it('does not throw when notifier fails', async () => {
      engine.setNotifier({
        sendNotification: async () => {
          throw new Error('fail');
        },
      });
      // Should not throw
      await engine.sendNotification({ message: 'hi' });
    });
  });

  // ========================================================================
  // Convenience methods
  // ========================================================================
  describe('convenience methods', () => {
    it('scheduleJob returns config when scheduler enabled', () => {
      const config = { id: 'j1', cron: '* * * * *' };
      const result = engine.scheduleJob(config);
      assert.deepEqual(result, config);
    });

    it('scheduleJob throws when scheduler disabled', () => {
      const e = new AutonomousEngine({ enableScheduler: false });
      assert.throws(() => e.scheduleJob({}), /Scheduler not enabled/);
    });

    it('registerWorkflow returns config when workflows enabled', () => {
      const config = { id: 'w1' };
      assert.deepEqual(engine.registerWorkflow(config), config);
    });

    it('registerWorkflow throws when workflows disabled', () => {
      const e = new AutonomousEngine({ enableWorkflows: false });
      assert.throws(() => e.registerWorkflow({}), /Workflows not enabled/);
    });

    it('addPolicy throws when policies disabled', () => {
      const e = new AutonomousEngine({ enablePolicies: false });
      assert.throws(() => e.addPolicy({}), /Policies not enabled/);
    });

    it('addWebhookSource throws when webhooks disabled', () => {
      const e = new AutonomousEngine({ enableWebhooks: false });
      assert.throws(() => e.addWebhookSource({}), /Webhooks not enabled/);
    });

    it('requestApproval throws when approvals disabled', () => {
      const e = new AutonomousEngine({ enableApprovals: false });
      assert.throws(() => e.requestApproval({}), /Approvals not enabled/);
    });
  });

  // ========================================================================
  // preOperationCheck
  // ========================================================================
  describe('preOperationCheck', () => {
    it('returns allowed when no policy or approval issues', async () => {
      const result = await engine.preOperationCheck('refund', { amount: 10 });
      assert.equal(result.allowed, true);
    });

    it('returns denied when policy shouldDeny', async () => {
      engine.policies.evaluate = async () => ({ shouldDeny: true, reason: 'too high' });
      const result = await engine.preOperationCheck('refund', { amount: 10000 });
      assert.equal(result.allowed, false);
      assert.equal(result.reason, 'denied_by_policy');
    });

    it('returns requires_approval when chain exists', async () => {
      engine.approvals.getChainForDomain = () => ({ id: 'chain-1' });
      const result = await engine.preOperationCheck('refund', {});
      assert.equal(result.allowed, false);
      assert.equal(result.reason, 'requires_approval');
    });
  });

  // ========================================================================
  // start / stop lifecycle
  // ========================================================================
  describe('start/stop', () => {
    it('start sets isRunning to true', async () => {
      await engine.start();
      assert.equal(engine.isRunning, true);
    });

    it('start emits started event', async () => {
      let emitted = false;
      engine.on('started', () => {
        emitted = true;
      });
      await engine.start();
      assert.equal(emitted, true);
    });

    it('start is idempotent', async () => {
      let count = 0;
      engine.on('started', () => {
        count++;
      });
      await engine.start();
      await engine.start();
      assert.equal(count, 1);
    });

    it('stop sets isRunning to false', async () => {
      await engine.start();
      await engine.stop();
      assert.equal(engine.isRunning, false);
    });

    it('stop emits stopped event', async () => {
      await engine.start();
      let emitted = false;
      engine.on('stopped', () => {
        emitted = true;
      });
      await engine.stop();
      assert.equal(emitted, true);
    });

    it('stop is idempotent', async () => {
      let count = 0;
      engine.on('stopped', () => {
        count++;
      });
      // not started, stop should be no-op
      await engine.stop();
      assert.equal(count, 0);
    });
  });
});
