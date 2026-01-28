/**
 * Autonomous Business Engine for StateSet Commerce
 *
 * Unified orchestrator that combines:
 * - Scheduled Jobs (cron, intervals)
 * - State Machine Workflows
 * - Declarative Policies
 * - Webhook Event Handling
 * - Approval Escalation
 *
 * This enables AI agents to run a complete business autonomously.
 */

import { EventEmitter } from 'events';
import fs from 'fs';
import path from 'path';

import { Scheduler, JobTemplates } from '../workflows/scheduler.js';
import { WorkflowEngine, WorkflowTemplates } from '../workflows/state-machine.js';
import { PolicyEngine, PolicyTemplates } from '../policies/engine.js';
import { WebhookServer, WebhookSourceTemplates, WebhookHandlerTemplates } from '../webhooks/server.js';
import { ApprovalQueue, ApprovalChainTemplates } from '../approvals/queue.js';

/**
 * Autonomous Business Engine
 */
export class AutonomousEngine extends EventEmitter {
  constructor({
    storePath = '.stateset/autonomous',
    commerce = null, // StateSet Commerce instance
    agentExecutor = null, // Function to execute agent requests
    webhookPort = 3000,
    webhookHost = '0.0.0.0',
    enableWebhooks = true,
    enableScheduler = true,
    enableWorkflows = true,
    enablePolicies = true,
    enableApprovals = true
  }) {
    super();

    this.storePath = storePath;
    this.commerce = commerce;
    this.agentExecutor = agentExecutor;

    // Feature flags
    this.features = {
      webhooks: enableWebhooks,
      scheduler: enableScheduler,
      workflows: enableWorkflows,
      policies: enablePolicies,
      approvals: enableApprovals
    };

    // Initialize subsystems
    this.scheduler = enableScheduler ? new Scheduler({
      storePath: path.join(storePath, 'scheduler'),
      executor: this.executeAction.bind(this)
    }) : null;

    this.workflows = enableWorkflows ? new WorkflowEngine({
      storePath: path.join(storePath, 'workflows'),
      executor: this.executeAction.bind(this),
      conditionEvaluator: this.evaluateCondition.bind(this)
    }) : null;

    this.policies = enablePolicies ? new PolicyEngine({
      storePath: storePath,
      executor: this.executeAction.bind(this)
    }) : null;

    this.webhooks = enableWebhooks ? new WebhookServer({
      port: webhookPort,
      host: webhookHost,
      storePath: path.join(storePath, 'webhooks'),
      executor: this.executeAction.bind(this)
    }) : null;

    this.approvals = enableApprovals ? new ApprovalQueue({
      storePath: path.join(storePath, 'approvals'),
      executor: this.executeAction.bind(this),
      notifier: this.sendNotification.bind(this)
    }) : null;

    this.isRunning = false;
    this._notifier = null;

    // Wire up event forwarding
    this.setupEventForwarding();
  }

  /**
   * Set up event forwarding from subsystems
   */
  setupEventForwarding() {
    const subsystems = [
      { name: 'scheduler', instance: this.scheduler },
      { name: 'workflows', instance: this.workflows },
      { name: 'policies', instance: this.policies },
      { name: 'webhooks', instance: this.webhooks },
      { name: 'approvals', instance: this.approvals }
    ];

    for (const { name, instance } of subsystems) {
      if (!instance) continue;

      // Forward all events with subsystem prefix
      const originalEmit = instance.emit.bind(instance);
      instance.emit = (event, ...args) => {
        originalEmit(event, ...args);
        this.emit(`${name}:${event}`, ...args);
      };
    }
  }

  /**
   * Execute an action (used by all subsystems)
   */
  async executeAction(action, context = {}) {
    if (!action) return null;

    // Handle different action types
    if (action.agent && action.request) {
      // Agent request
      return this.executeAgentRequest(action.agent, action.request, context);
    }

    if (action.workflow) {
      // Start a workflow
      return this.startWorkflow(action.workflow, context);
    }

    if (action.job) {
      // Trigger a job
      return this.triggerJob(action.job);
    }

    if (action.approval) {
      // Create an approval request
      return this.createApproval(action.approval, context);
    }

    if (action.policy) {
      // Evaluate a policy
      return this.evaluatePolicy(action.policy, context);
    }

    if (typeof action === 'function') {
      // Direct function call
      return action(context);
    }

    return null;
  }

  /**
   * Execute an agent request
   */
  async executeAgentRequest(agent, request, context = {}) {
    if (!this.agentExecutor) {
      this.emit('warning', { message: 'No agent executor configured' });
      return null;
    }

    // Interpolate context values into request
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

  /**
   * Interpolate values into a string
   */
  interpolate(template, context) {
    if (typeof template !== 'string') return template;

    return template.replace(/\{([^}]+)\}/g, (match, path) => {
      const value = this.getNestedValue(context, path);
      return value !== undefined ? value : match;
    });
  }

  /**
   * Get nested value from object
   */
  getNestedValue(obj, path) {
    return path.split('.').reduce((o, k) => o?.[k], obj);
  }

  /**
   * Evaluate a condition (used by workflows)
   */
  async evaluateCondition(condition, context) {
    if (typeof condition === 'function') {
      return condition(context);
    }

    if (typeof condition === 'string') {
      // Simple field check
      return !!this.getNestedValue(context, condition);
    }

    if (condition.policy) {
      // Evaluate via policy engine
      const result = await this.policies?.evaluate(condition.policy, context);
      return result?.shouldAllow ?? true;
    }

    return true;
  }

  /**
   * Set the channel notifier for proactive notifications.
   *
   * @param {import('../channels/notifier.js').ChannelNotifier} notifier
   */
  setNotifier(notifier) {
    this._notifier = notifier;
  }

  /**
   * Send a notification (used by approvals and events).
   * Routes through channel notifier if configured, otherwise logs to console.
   */
  async sendNotification(notification) {
    this.emit('notification', notification);

    if (this._notifier) {
      try {
        await this._notifier.sendNotification({
          type: notification.type || 'general',
          message: notification.message || JSON.stringify(notification),
          richMessage: notification.richMessage || null,
        });
      } catch (err) {
        console.error('[Notification] Failed to send via notifier:', err.message);
        console.log('[Notification]', JSON.stringify(notification, null, 2));
      }
    } else {
      console.log('[Notification]', JSON.stringify(notification, null, 2));
    }
  }

  /**
   * Start a workflow
   */
  async startWorkflow(workflowId, context = {}) {
    if (!this.workflows) {
      throw new Error('Workflows not enabled');
    }

    return this.workflows.startWorkflow(workflowId, { context });
  }

  /**
   * Trigger a job
   */
  async triggerJob(jobId) {
    if (!this.scheduler) {
      throw new Error('Scheduler not enabled');
    }

    return this.scheduler.runNow(jobId);
  }

  /**
   * Create an approval request
   */
  async createApproval(config, context = {}) {
    if (!this.approvals) {
      throw new Error('Approvals not enabled');
    }

    return this.approvals.createRequest({
      ...config,
      context: { ...config.context, ...context }
    });
  }

  /**
   * Evaluate a policy
   */
  async evaluatePolicy(domain, context = {}) {
    if (!this.policies) {
      throw new Error('Policies not enabled');
    }

    return this.policies.evaluateAndExecute(domain, context);
  }

  /**
   * Load all subsystems
   */
  async load() {
    fs.mkdirSync(this.storePath, { recursive: true });

    const loadPromises = [];

    if (this.scheduler) loadPromises.push(this.scheduler.load());
    if (this.workflows) loadPromises.push(this.workflows.load());
    if (this.policies) loadPromises.push(this.policies.load());
    if (this.webhooks) loadPromises.push(this.webhooks.load());
    if (this.approvals) loadPromises.push(this.approvals.load());

    await Promise.all(loadPromises);
    this.emit('loaded');
  }

  /**
   * Save all subsystems
   */
  async save() {
    const savePromises = [];

    if (this.scheduler) savePromises.push(this.scheduler.save());
    if (this.workflows) savePromises.push(this.workflows.save());
    if (this.policies) savePromises.push(this.policies.save());
    if (this.webhooks) savePromises.push(this.webhooks.save());
    if (this.approvals) savePromises.push(this.approvals.save());

    await Promise.all(savePromises);
    this.emit('saved');
  }

  /**
   * Initialize with default templates
   */
  async initializeDefaults() {
    // Register default job templates
    if (this.scheduler) {
      for (const [key, template] of Object.entries(JobTemplates)) {
        this.scheduler.addJob({
          ...template,
          enabled: false // Disabled by default
        });
      }
    }

    // Register default workflow templates
    if (this.workflows) {
      for (const [key, template] of Object.entries(WorkflowTemplates)) {
        this.workflows.registerWorkflow(template);
      }
    }

    // Register default policy templates
    if (this.policies) {
      for (const [key, template] of Object.entries(PolicyTemplates)) {
        this.policies.registerPolicySet(template);
      }
    }

    // Register default webhook sources
    if (this.webhooks) {
      for (const [key, template] of Object.entries(WebhookSourceTemplates)) {
        this.webhooks.registerSource({
          ...template,
          enabled: false // Disabled by default
        });
      }
    }

    // Register default approval chains
    if (this.approvals) {
      for (const [key, template] of Object.entries(ApprovalChainTemplates)) {
        this.approvals.registerChain(template);
      }
    }

    await this.save();
    this.emit('initialized');
  }

  /**
   * Start all subsystems
   */
  async start() {
    if (this.isRunning) return;

    await this.load();

    if (this.scheduler) this.scheduler.start();
    if (this.webhooks) this.webhooks.start();
    if (this.approvals) this.approvals.start();

    this.isRunning = true;
    this.emit('started');
  }

  /**
   * Stop all subsystems
   */
  async stop() {
    if (!this.isRunning) return;

    if (this.scheduler) this.scheduler.stop();
    if (this.webhooks) await this.webhooks.stop();
    if (this.approvals) this.approvals.stop();

    await this.save();

    this.isRunning = false;
    this.emit('stopped');
  }

  /**
   * Get comprehensive status
   */
  getStatus() {
    return {
      isRunning: this.isRunning,
      features: this.features,
      scheduler: this.scheduler?.getStatus() || null,
      workflows: this.workflows?.getStatus() || null,
      policies: this.policies?.getStatus() || null,
      webhooks: this.webhooks?.getStatus() || null,
      approvals: this.approvals?.getStatus() || null
    };
  }

  // ============================================================================
  // Convenience Methods for Common Operations
  // ============================================================================

  /**
   * Schedule a recurring job
   */
  scheduleJob(config) {
    if (!this.scheduler) throw new Error('Scheduler not enabled');
    return this.scheduler.addJob(config);
  }

  /**
   * Register a workflow
   */
  registerWorkflow(config) {
    if (!this.workflows) throw new Error('Workflows not enabled');
    return this.workflows.registerWorkflow(config);
  }

  /**
   * Register a policy set
   */
  registerPolicy(config) {
    if (!this.policies) throw new Error('Policies not enabled');
    return this.policies.registerPolicySet(config);
  }

  /**
   * Register a webhook source
   */
  registerWebhook(config) {
    if (!this.webhooks) throw new Error('Webhooks not enabled');
    return this.webhooks.registerSource(config);
  }

  /**
   * Register a webhook handler
   */
  registerWebhookHandler(config) {
    if (!this.webhooks) throw new Error('Webhooks not enabled');
    return this.webhooks.registerHandler(config);
  }

  /**
   * Register an approval chain
   */
  registerApprovalChain(config) {
    if (!this.approvals) throw new Error('Approvals not enabled');
    return this.approvals.registerChain(config);
  }

  /**
   * Check if operation needs approval
   */
  async checkApproval(domain, context) {
    if (!this.approvals) return { required: false };

    const chain = this.approvals.getChainForDomain(domain, context);
    if (!chain) return { required: false };

    return { required: true, chain };
  }

  /**
   * Run policy check before operation
   */
  async policyCheck(domain, context) {
    if (!this.policies) return { allowed: true };

    return this.policies.evaluate(domain, context);
  }

  /**
   * Combined pre-operation check (policy + approval)
   */
  async preOperationCheck(domain, context) {
    // Check policy first
    const policyResult = await this.policyCheck(domain, context);

    if (policyResult.shouldDeny) {
      return {
        allowed: false,
        reason: 'denied_by_policy',
        policyResult
      };
    }

    // Check if approval needed
    const approvalCheck = await this.checkApproval(domain, context);

    if (approvalCheck.required) {
      return {
        allowed: false,
        reason: 'requires_approval',
        approvalCheck,
        policyResult
      };
    }

    return {
      allowed: true,
      policyResult
    };
  }
}

/**
 * Create a pre-configured autonomous engine
 */
export function createAutonomousEngine(options = {}) {
  return new AutonomousEngine(options);
}

export default AutonomousEngine;
