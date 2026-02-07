/**
 * State Machine for Complex Business Workflows
 *
 * Enables AI agents to manage multi-step processes:
 * - Order fulfillment pipelines
 * - Return processing workflows
 * - Subscription lifecycle management
 * - Approval chains
 */

import { EventEmitter } from 'events';
import { randomUUID } from 'crypto';
import fs from 'fs';
import path from 'path';

/**
 * State definition
 */
export class State {
  constructor({
    name,
    description = '',
    onEnter = null, // Action to execute when entering state
    onExit = null, // Action to execute when exiting state
    timeout = null, // Auto-transition after timeout (ms)
    timeoutTransition = null, // State to transition to on timeout
    metadata = {},
  }) {
    this.name = name;
    this.description = description;
    this.onEnter = onEnter;
    this.onExit = onExit;
    this.timeout = timeout;
    this.timeoutTransition = timeoutTransition;
    this.metadata = metadata;
  }
}

/**
 * Transition definition
 */
export class Transition {
  constructor({
    name,
    from, // State name or array of state names
    to, // Target state name
    condition = null, // Guard condition (returns boolean)
    action = null, // Action to execute during transition
    priority = 0, // Higher priority transitions are evaluated first
    metadata = {},
  }) {
    this.name = name;
    this.from = Array.isArray(from) ? from : [from];
    this.to = to;
    this.condition = condition;
    this.action = action;
    this.priority = priority;
    this.metadata = metadata;
  }
}

/**
 * Workflow instance - a running state machine
 */
export class WorkflowInstance {
  constructor({
    id = randomUUID(),
    workflowId,
    workflowName,
    currentState,
    context = {},
    history = [],
    status = 'running', // running, completed, failed, paused, cancelled
    createdAt = new Date().toISOString(),
    updatedAt = new Date().toISOString(),
    completedAt = null,
    error = null,
    metadata = {},
  }) {
    this.id = id;
    this.workflowId = workflowId;
    this.workflowName = workflowName;
    this.currentState = currentState;
    this.context = context;
    this.history = history;
    this.status = status;
    this.createdAt = createdAt;
    this.updatedAt = updatedAt;
    this.completedAt = completedAt;
    this.error = error;
    this.metadata = metadata;
    this.timeoutTimer = null;
  }

  toJSON() {
    return {
      id: this.id,
      workflowId: this.workflowId,
      workflowName: this.workflowName,
      currentState: this.currentState,
      context: this.context,
      history: this.history,
      status: this.status,
      createdAt: this.createdAt,
      updatedAt: this.updatedAt,
      completedAt: this.completedAt,
      error: this.error,
      metadata: this.metadata,
    };
  }
}

/**
 * State Machine Definition
 */
export class StateMachine extends EventEmitter {
  constructor({
    id = randomUUID(),
    name,
    description = '',
    initialState,
    states = [],
    transitions = [],
    finalStates = [], // States that complete the workflow
    metadata = {},
  }) {
    super();

    this.id = id;
    this.name = name;
    this.description = description;
    this.initialState = initialState;
    this.finalStates = finalStates;
    this.metadata = metadata;

    // Index states and transitions for fast lookup
    this.states = new Map();
    for (const state of states) {
      const s = state instanceof State ? state : new State(state);
      this.states.set(s.name, s);
    }

    this.transitions = new Map();
    for (const transition of transitions) {
      const t = transition instanceof Transition ? transition : new Transition(transition);
      for (const fromState of t.from) {
        if (!this.transitions.has(fromState)) {
          this.transitions.set(fromState, []);
        }
        this.transitions.get(fromState).push(t);
      }
    }

    // Sort transitions by priority
    for (const trans of this.transitions.values()) {
      trans.sort((a, b) => b.priority - a.priority);
    }

    this.validate();
  }

  /**
   * Validate the state machine definition
   */
  validate() {
    // Check initial state exists
    if (!this.states.has(this.initialState)) {
      throw new Error(`Initial state '${this.initialState}' not found`);
    }

    // Check all transition targets exist
    for (const trans of this.transitions.values()) {
      for (const t of trans) {
        if (!this.states.has(t.to)) {
          throw new Error(`Transition '${t.name}' targets unknown state '${t.to}'`);
        }
      }
    }

    // Check final states exist
    for (const finalState of this.finalStates) {
      if (!this.states.has(finalState)) {
        throw new Error(`Final state '${finalState}' not found`);
      }
    }

    // Check timeout transitions
    for (const [name, state] of this.states) {
      if (state.timeout && state.timeoutTransition) {
        if (!this.states.has(state.timeoutTransition)) {
          throw new Error(
            `State '${name}' timeout targets unknown state '${state.timeoutTransition}'`,
          );
        }
      }
    }
  }

  /**
   * Get available transitions from a state
   */
  getTransitions(stateName) {
    return this.transitions.get(stateName) || [];
  }

  /**
   * Get state definition
   */
  getState(stateName) {
    return this.states.get(stateName);
  }

  /**
   * Check if state is final
   */
  isFinalState(stateName) {
    return this.finalStates.includes(stateName);
  }

  toJSON() {
    return {
      id: this.id,
      name: this.name,
      description: this.description,
      initialState: this.initialState,
      states: Array.from(this.states.values()).map((s) => ({
        name: s.name,
        description: s.description,
        timeout: s.timeout,
        timeoutTransition: s.timeoutTransition,
        metadata: s.metadata,
      })),
      transitions: Array.from(this.transitions.values())
        .flat()
        .filter((t, i, arr) => arr.findIndex((x) => x.name === t.name) === i)
        .map((t) => ({
          name: t.name,
          from: t.from,
          to: t.to,
          priority: t.priority,
          metadata: t.metadata,
        })),
      finalStates: this.finalStates,
      metadata: this.metadata,
    };
  }
}

/**
 * Workflow Engine - manages workflow instances
 */
export class WorkflowEngine extends EventEmitter {
  constructor({
    storePath = null,
    executor = null, // Function to execute actions
    conditionEvaluator = null, // Function to evaluate conditions
  }) {
    super();

    this.storePath = storePath;
    this.executor = executor;
    this.conditionEvaluator = conditionEvaluator;

    this.workflows = new Map(); // Workflow definitions
    this.instances = new Map(); // Running instances
  }

  /**
   * Load persisted data
   */
  async load() {
    if (!this.storePath) return;

    const instancesFile = path.join(this.storePath, 'workflow-instances.json');

    try {
      if (fs.existsSync(instancesFile)) {
        const data = JSON.parse(fs.readFileSync(instancesFile, 'utf-8'));
        for (const instanceData of data) {
          const instance = new WorkflowInstance(instanceData);
          if (instance.status === 'running') {
            this.instances.set(instance.id, instance);
            // Restart timeout timers
            this.setupStateTimeout(instance);
          }
        }
        this.emit('loaded', { instanceCount: this.instances.size });
      }
    } catch (error) {
      this.emit('error', { type: 'load', error });
    }
  }

  /**
   * Save instances to persistent storage
   */
  async save() {
    if (!this.storePath) return;

    try {
      fs.mkdirSync(this.storePath, { recursive: true });

      const instancesFile = path.join(this.storePath, 'workflow-instances.json');
      const instancesData = Array.from(this.instances.values()).map((i) => i.toJSON());
      fs.writeFileSync(instancesFile, JSON.stringify(instancesData, null, 2));

      this.emit('saved', { instanceCount: this.instances.size });
    } catch (error) {
      this.emit('error', { type: 'save', error });
    }
  }

  /**
   * Register a workflow definition
   */
  registerWorkflow(definition) {
    const workflow = definition instanceof StateMachine ? definition : new StateMachine(definition);

    this.workflows.set(workflow.id, workflow);
    this.emit('workflow:registered', { workflow: workflow.toJSON() });

    return workflow;
  }

  /**
   * Get a workflow definition
   */
  getWorkflow(workflowId) {
    return this.workflows.get(workflowId);
  }

  /**
   * List all workflow definitions
   */
  listWorkflows() {
    return Array.from(this.workflows.values()).map((w) => w.toJSON());
  }

  /**
   * Start a new workflow instance
   */
  async startWorkflow(workflowId, { context = {}, metadata = {} } = {}) {
    const workflow = this.workflows.get(workflowId);
    if (!workflow) {
      throw new Error(`Workflow not found: ${workflowId}`);
    }

    const instance = new WorkflowInstance({
      workflowId,
      workflowName: workflow.name,
      currentState: workflow.initialState,
      context,
      metadata,
      history: [
        {
          timestamp: new Date().toISOString(),
          event: 'started',
          state: workflow.initialState,
          context: { ...context },
        },
      ],
    });

    this.instances.set(instance.id, instance);

    // Execute onEnter for initial state
    const initialState = workflow.getState(workflow.initialState);
    if (initialState?.onEnter) {
      await this.executeAction(initialState.onEnter, instance);
    }

    // Set up timeout if configured
    this.setupStateTimeout(instance);

    this.emit('instance:started', { instance: instance.toJSON() });
    await this.save();

    return instance;
  }

  /**
   * Set up timeout timer for current state
   */
  setupStateTimeout(instance) {
    // Clear existing timer
    if (instance.timeoutTimer) {
      clearTimeout(instance.timeoutTimer);
      instance.timeoutTimer = null;
    }

    const workflow = this.workflows.get(instance.workflowId);
    if (!workflow) return;

    const state = workflow.getState(instance.currentState);
    if (!state?.timeout || !state?.timeoutTransition) return;

    instance.timeoutTimer = setTimeout(async () => {
      try {
        await this.transition(instance.id, state.timeoutTransition, {
          reason: 'timeout',
          timeoutMs: state.timeout,
        });
      } catch (error) {
        this.emit('error', { type: 'timeout-transition', instanceId: instance.id, error });
      }
    }, state.timeout);
  }

  /**
   * Transition a workflow instance to a new state
   */
  async transition(instanceId, targetState, transitionContext = {}) {
    const instance = this.instances.get(instanceId);
    if (!instance) {
      throw new Error(`Instance not found: ${instanceId}`);
    }

    if (instance.status !== 'running') {
      throw new Error(`Instance is not running: ${instance.status}`);
    }

    const workflow = this.workflows.get(instance.workflowId);
    if (!workflow) {
      throw new Error(`Workflow not found: ${instance.workflowId}`);
    }

    // Find valid transition
    const transitions = workflow.getTransitions(instance.currentState);
    const transition = transitions.find((t) => t.to === targetState);

    if (!transition) {
      throw new Error(`No transition from '${instance.currentState}' to '${targetState}'`);
    }

    // Check guard condition
    if (transition.condition) {
      const allowed = await this.evaluateCondition(
        transition.condition,
        instance,
        transitionContext,
      );
      if (!allowed) {
        throw new Error(`Transition condition not met for '${transition.name}'`);
      }
    }

    // Clear timeout
    if (instance.timeoutTimer) {
      clearTimeout(instance.timeoutTimer);
      instance.timeoutTimer = null;
    }

    const previousState = instance.currentState;
    const fromStateObj = workflow.getState(previousState);
    const toStateObj = workflow.getState(targetState);

    // Execute onExit
    if (fromStateObj?.onExit) {
      await this.executeAction(fromStateObj.onExit, instance, transitionContext);
    }

    // Execute transition action
    if (transition.action) {
      await this.executeAction(transition.action, instance, transitionContext);
    }

    // Update state
    instance.currentState = targetState;
    instance.updatedAt = new Date().toISOString();
    instance.context = { ...instance.context, ...transitionContext };
    instance.history.push({
      timestamp: instance.updatedAt,
      event: 'transition',
      from: previousState,
      to: targetState,
      transition: transition.name,
      context: { ...transitionContext },
    });

    // Execute onEnter
    if (toStateObj?.onEnter) {
      await this.executeAction(toStateObj.onEnter, instance, transitionContext);
    }

    // Check if final state
    if (workflow.isFinalState(targetState)) {
      instance.status = 'completed';
      instance.completedAt = new Date().toISOString();
      instance.history.push({
        timestamp: instance.completedAt,
        event: 'completed',
        state: targetState,
      });
      this.emit('instance:completed', { instance: instance.toJSON() });
    } else {
      // Set up new timeout
      this.setupStateTimeout(instance);
    }

    this.emit('instance:transitioned', {
      instanceId,
      from: previousState,
      to: targetState,
      transition: transition.name,
    });

    await this.save();
    return instance;
  }

  /**
   * Trigger an event on a workflow instance
   * Finds matching transition and executes it
   */
  async trigger(instanceId, eventName, eventContext = {}) {
    const instance = this.instances.get(instanceId);
    if (!instance) {
      throw new Error(`Instance not found: ${instanceId}`);
    }

    const workflow = this.workflows.get(instance.workflowId);
    if (!workflow) {
      throw new Error(`Workflow not found: ${instance.workflowId}`);
    }

    const transitions = workflow.getTransitions(instance.currentState);
    const matchingTransition = transitions.find((t) => t.name === eventName);

    if (!matchingTransition) {
      throw new Error(`No transition '${eventName}' from state '${instance.currentState}'`);
    }

    return this.transition(instanceId, matchingTransition.to, eventContext);
  }

  /**
   * Execute an action (agent request or function)
   */
  async executeAction(action, instance, context = {}) {
    if (!this.executor) {
      this.emit('warning', { message: 'No executor configured, skipping action', action });
      return null;
    }

    return this.executor(action, {
      instanceId: instance.id,
      workflowId: instance.workflowId,
      currentState: instance.currentState,
      instanceContext: instance.context,
      transitionContext: context,
    });
  }

  /**
   * Evaluate a condition
   */
  async evaluateCondition(condition, instance, context = {}) {
    if (!this.conditionEvaluator) {
      // Default: treat condition as a context key check
      if (typeof condition === 'string') {
        return !!instance.context[condition];
      }
      return true;
    }

    return this.conditionEvaluator(condition, {
      instanceId: instance.id,
      currentState: instance.currentState,
      instanceContext: instance.context,
      transitionContext: context,
    });
  }

  /**
   * Pause a workflow instance
   */
  async pauseInstance(instanceId) {
    const instance = this.instances.get(instanceId);
    if (!instance) {
      throw new Error(`Instance not found: ${instanceId}`);
    }

    if (instance.timeoutTimer) {
      clearTimeout(instance.timeoutTimer);
      instance.timeoutTimer = null;
    }

    instance.status = 'paused';
    instance.updatedAt = new Date().toISOString();
    instance.history.push({
      timestamp: instance.updatedAt,
      event: 'paused',
      state: instance.currentState,
    });

    this.emit('instance:paused', { instance: instance.toJSON() });
    await this.save();

    return instance;
  }

  /**
   * Resume a paused workflow instance
   */
  async resumeInstance(instanceId) {
    const instance = this.instances.get(instanceId);
    if (!instance) {
      throw new Error(`Instance not found: ${instanceId}`);
    }

    if (instance.status !== 'paused') {
      throw new Error(`Instance is not paused: ${instance.status}`);
    }

    instance.status = 'running';
    instance.updatedAt = new Date().toISOString();
    instance.history.push({
      timestamp: instance.updatedAt,
      event: 'resumed',
      state: instance.currentState,
    });

    // Restart timeout
    this.setupStateTimeout(instance);

    this.emit('instance:resumed', { instance: instance.toJSON() });
    await this.save();

    return instance;
  }

  /**
   * Cancel a workflow instance
   */
  async cancelInstance(instanceId, reason = null) {
    const instance = this.instances.get(instanceId);
    if (!instance) {
      throw new Error(`Instance not found: ${instanceId}`);
    }

    if (instance.timeoutTimer) {
      clearTimeout(instance.timeoutTimer);
      instance.timeoutTimer = null;
    }

    instance.status = 'cancelled';
    instance.updatedAt = new Date().toISOString();
    instance.completedAt = instance.updatedAt;
    instance.history.push({
      timestamp: instance.updatedAt,
      event: 'cancelled',
      state: instance.currentState,
      reason,
    });

    this.emit('instance:cancelled', { instance: instance.toJSON(), reason });
    await this.save();

    return instance;
  }

  /**
   * Fail a workflow instance
   */
  async failInstance(instanceId, error) {
    const instance = this.instances.get(instanceId);
    if (!instance) {
      throw new Error(`Instance not found: ${instanceId}`);
    }

    if (instance.timeoutTimer) {
      clearTimeout(instance.timeoutTimer);
      instance.timeoutTimer = null;
    }

    instance.status = 'failed';
    instance.error = error.message || String(error);
    instance.updatedAt = new Date().toISOString();
    instance.completedAt = instance.updatedAt;
    instance.history.push({
      timestamp: instance.updatedAt,
      event: 'failed',
      state: instance.currentState,
      error: instance.error,
    });

    this.emit('instance:failed', { instance: instance.toJSON(), error: instance.error });
    await this.save();

    return instance;
  }

  /**
   * Get a workflow instance
   */
  getInstance(instanceId) {
    return this.instances.get(instanceId);
  }

  /**
   * List workflow instances
   */
  listInstances({ workflowId = null, status = null, limit = 100 } = {}) {
    let instances = Array.from(this.instances.values());

    if (workflowId) {
      instances = instances.filter((i) => i.workflowId === workflowId);
    }

    if (status) {
      instances = instances.filter((i) => i.status === status);
    }

    return instances
      .sort((a, b) => new Date(b.updatedAt) - new Date(a.updatedAt))
      .slice(0, limit)
      .map((i) => i.toJSON());
  }

  /**
   * Get workflow instance status
   */
  getStatus() {
    const instances = Array.from(this.instances.values());
    const byStatus = {};

    for (const instance of instances) {
      byStatus[instance.status] = (byStatus[instance.status] || 0) + 1;
    }

    return {
      totalWorkflows: this.workflows.size,
      totalInstances: instances.length,
      byStatus,
      recentInstances: instances
        .sort((a, b) => new Date(b.updatedAt) - new Date(a.updatedAt))
        .slice(0, 5)
        .map((i) => ({
          id: i.id,
          workflowName: i.workflowName,
          currentState: i.currentState,
          status: i.status,
          updatedAt: i.updatedAt,
        })),
    };
  }
}

/**
 * Pre-defined workflow templates for common commerce operations
 */
export const WorkflowTemplates = {
  // Order fulfillment workflow
  orderFulfillment: {
    name: 'Order Fulfillment',
    description: 'Standard order fulfillment pipeline',
    initialState: 'pending',
    states: [
      { name: 'pending', description: 'Order received, awaiting processing' },
      {
        name: 'processing',
        description: 'Order being prepared',
        onEnter: { agent: 'inventory', request: 'Reserve inventory for order {orderId}' },
      },
      {
        name: 'awaiting_payment',
        description: 'Waiting for payment confirmation',
        timeout: 3600000, // 1 hour
        timeoutTransition: 'cancelled',
      },
      {
        name: 'paid',
        description: 'Payment received',
        onEnter: {
          agent: 'inventory',
          request: 'Confirm inventory reservation for order {orderId}',
        },
      },
      {
        name: 'shipped',
        description: 'Order shipped to customer',
        onEnter: { agent: 'orders', request: 'Update order {orderId} status to shipped' },
      },
      {
        name: 'delivered',
        description: 'Order delivered to customer',
        onEnter: {
          agent: 'analytics',
          request: 'Record successful fulfillment for order {orderId}',
        },
      },
      { name: 'cancelled', description: 'Order cancelled' },
      { name: 'refunded', description: 'Order refunded' },
    ],
    transitions: [
      { name: 'process', from: 'pending', to: 'processing' },
      { name: 'await_payment', from: 'processing', to: 'awaiting_payment' },
      { name: 'payment_received', from: 'awaiting_payment', to: 'paid' },
      { name: 'ship', from: 'paid', to: 'shipped' },
      { name: 'deliver', from: 'shipped', to: 'delivered' },
      { name: 'cancel', from: ['pending', 'processing', 'awaiting_payment'], to: 'cancelled' },
      { name: 'refund', from: ['paid', 'shipped', 'delivered'], to: 'refunded' },
    ],
    finalStates: ['delivered', 'cancelled', 'refunded'],
  },

  // Return processing workflow
  returnProcessing: {
    name: 'Return Processing',
    description: 'Customer return and refund workflow',
    initialState: 'requested',
    states: [
      { name: 'requested', description: 'Return request received' },
      {
        name: 'pending_approval',
        description: 'Awaiting approval decision',
        timeout: 86400000, // 24 hours
        timeoutTransition: 'auto_approved',
      },
      { name: 'auto_approved', description: 'Auto-approved due to timeout' },
      { name: 'approved', description: 'Return approved' },
      { name: 'rejected', description: 'Return rejected' },
      {
        name: 'awaiting_item',
        description: 'Waiting for item to be returned',
        timeout: 604800000, // 7 days
        timeoutTransition: 'expired',
      },
      {
        name: 'item_received',
        description: 'Returned item received',
        onEnter: { agent: 'returns', request: 'Inspect returned item for return {returnId}' },
      },
      {
        name: 'processing_refund',
        description: 'Processing refund',
        onEnter: { agent: 'payments', request: 'Process refund for return {returnId}' },
      },
      { name: 'refunded', description: 'Refund completed' },
      { name: 'expired', description: 'Return window expired' },
    ],
    transitions: [
      { name: 'submit_for_approval', from: 'requested', to: 'pending_approval' },
      { name: 'approve', from: ['pending_approval', 'auto_approved'], to: 'approved' },
      { name: 'reject', from: 'pending_approval', to: 'rejected' },
      { name: 'await_item', from: 'approved', to: 'awaiting_item' },
      { name: 'receive_item', from: 'awaiting_item', to: 'item_received' },
      { name: 'process_refund', from: 'item_received', to: 'processing_refund' },
      { name: 'complete_refund', from: 'processing_refund', to: 'refunded' },
    ],
    finalStates: ['refunded', 'rejected', 'expired'],
  },

  // Subscription lifecycle workflow
  subscriptionLifecycle: {
    name: 'Subscription Lifecycle',
    description: 'Subscription billing and renewal workflow',
    initialState: 'trial',
    states: [
      {
        name: 'trial',
        description: 'Trial period',
        timeout: 1209600000, // 14 days
        timeoutTransition: 'trial_ending',
      },
      { name: 'trial_ending', description: 'Trial ending soon' },
      { name: 'active', description: 'Active subscription' },
      {
        name: 'past_due',
        description: 'Payment past due',
        timeout: 604800000, // 7 days
        timeoutTransition: 'suspended',
      },
      {
        name: 'suspended',
        description: 'Subscription suspended',
        timeout: 2592000000, // 30 days
        timeoutTransition: 'cancelled',
      },
      { name: 'cancelled', description: 'Subscription cancelled' },
      { name: 'expired', description: 'Subscription expired' },
    ],
    transitions: [
      { name: 'convert', from: ['trial', 'trial_ending'], to: 'active' },
      { name: 'payment_failed', from: 'active', to: 'past_due' },
      { name: 'payment_received', from: 'past_due', to: 'active' },
      { name: 'suspend', from: 'past_due', to: 'suspended' },
      { name: 'reactivate', from: 'suspended', to: 'active' },
      {
        name: 'cancel',
        from: ['trial', 'trial_ending', 'active', 'past_due', 'suspended'],
        to: 'cancelled',
      },
      { name: 'expire', from: 'active', to: 'expired' },
    ],
    finalStates: ['cancelled', 'expired'],
  },

  // Purchase order workflow
  purchaseOrderApproval: {
    name: 'Purchase Order Approval',
    description: 'Multi-tier PO approval workflow',
    initialState: 'draft',
    states: [
      { name: 'draft', description: 'PO being created' },
      {
        name: 'pending_review',
        description: 'Awaiting initial review',
        timeout: 172800000, // 48 hours
        timeoutTransition: 'escalated',
      },
      { name: 'escalated', description: 'Escalated due to timeout' },
      {
        name: 'pending_approval',
        description: 'Awaiting manager approval',
        timeout: 172800000,
        timeoutTransition: 'escalated',
      },
      { name: 'approved', description: 'PO approved' },
      { name: 'rejected', description: 'PO rejected' },
      {
        name: 'sent',
        description: 'PO sent to supplier',
        onEnter: { agent: 'suppliers', request: 'Send PO {purchaseOrderId} to supplier' },
      },
      { name: 'acknowledged', description: 'Supplier acknowledged receipt' },
      { name: 'partially_received', description: 'Partial shipment received' },
      { name: 'received', description: 'All items received' },
      { name: 'cancelled', description: 'PO cancelled' },
    ],
    transitions: [
      { name: 'submit', from: 'draft', to: 'pending_review' },
      { name: 'review', from: ['pending_review', 'escalated'], to: 'pending_approval' },
      { name: 'approve', from: 'pending_approval', to: 'approved' },
      { name: 'reject', from: ['pending_review', 'pending_approval'], to: 'rejected' },
      { name: 'send', from: 'approved', to: 'sent' },
      { name: 'acknowledge', from: 'sent', to: 'acknowledged' },
      {
        name: 'partial_receive',
        from: ['acknowledged', 'partially_received'],
        to: 'partially_received',
      },
      { name: 'receive', from: ['acknowledged', 'partially_received'], to: 'received' },
      {
        name: 'cancel',
        from: ['draft', 'pending_review', 'pending_approval', 'approved'],
        to: 'cancelled',
      },
    ],
    finalStates: ['received', 'rejected', 'cancelled'],
  },
};

export default WorkflowEngine;
