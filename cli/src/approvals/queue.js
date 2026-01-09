/**
 * Approval Escalation System for StateSet Commerce
 *
 * Enables AI agents to escalate high-value decisions:
 * - Multi-tier approval chains
 * - Time-based auto-approval/rejection
 * - Notification system
 * - Audit trail
 */

import { EventEmitter } from 'events';
import { randomUUID } from 'crypto';
import fs from 'fs';
import path from 'path';

/**
 * Approval status enumeration
 */
export const ApprovalStatus = {
  PENDING: 'pending',
  APPROVED: 'approved',
  REJECTED: 'rejected',
  ESCALATED: 'escalated',
  EXPIRED: 'expired',
  AUTO_APPROVED: 'auto_approved',
  AUTO_REJECTED: 'auto_rejected',
  CANCELLED: 'cancelled'
};

/**
 * Approval tier configuration
 */
export class ApprovalTier {
  constructor({
    level, // 1, 2, 3, etc.
    name, // 'Manager', 'Director', 'VP'
    description = '',
    approvers = [], // List of approver IDs or roles
    requiredApprovals = 1, // How many approvals needed
    timeout = null, // Auto-escalate after timeout (ms)
    timeoutAction = 'escalate', // 'escalate', 'auto_approve', 'auto_reject'
    canApproveAmount = null, // Max amount this tier can approve
    metadata = {}
  }) {
    this.level = level;
    this.name = name;
    this.description = description;
    this.approvers = approvers;
    this.requiredApprovals = requiredApprovals;
    this.timeout = timeout;
    this.timeoutAction = timeoutAction;
    this.canApproveAmount = canApproveAmount;
    this.metadata = metadata;
  }

  toJSON() {
    return {
      level: this.level,
      name: this.name,
      description: this.description,
      approvers: this.approvers,
      requiredApprovals: this.requiredApprovals,
      timeout: this.timeout,
      timeoutAction: this.timeoutAction,
      canApproveAmount: this.canApproveAmount,
      metadata: this.metadata
    };
  }
}

/**
 * Approval chain configuration
 */
export class ApprovalChain {
  constructor({
    id = randomUUID(),
    name,
    description = '',
    domain, // 'orders', 'returns', 'purchase_orders', etc.
    tiers = [],
    conditions = null, // When to require this chain
    enabled = true,
    metadata = {}
  }) {
    this.id = id;
    this.name = name;
    this.description = description;
    this.domain = domain;
    this.tiers = tiers.map(t => t instanceof ApprovalTier ? t : new ApprovalTier(t));
    this.tiers.sort((a, b) => a.level - b.level);
    this.conditions = conditions;
    this.enabled = enabled;
    this.metadata = metadata;
  }

  /**
   * Get tier by level
   */
  getTier(level) {
    return this.tiers.find(t => t.level === level);
  }

  /**
   * Get next tier
   */
  getNextTier(currentLevel) {
    return this.tiers.find(t => t.level > currentLevel);
  }

  /**
   * Get appropriate tier for amount
   */
  getTierForAmount(amount) {
    // Find the lowest tier that can approve this amount
    for (const tier of this.tiers) {
      if (tier.canApproveAmount === null || amount <= tier.canApproveAmount) {
        return tier;
      }
    }
    // If no tier can approve, return highest
    return this.tiers[this.tiers.length - 1];
  }

  toJSON() {
    return {
      id: this.id,
      name: this.name,
      description: this.description,
      domain: this.domain,
      tiers: this.tiers.map(t => t.toJSON()),
      conditions: this.conditions,
      enabled: this.enabled,
      metadata: this.metadata
    };
  }
}

/**
 * Approval decision record
 */
export class ApprovalDecision {
  constructor({
    approverId,
    approverName = null,
    action, // 'approve', 'reject', 'escalate'
    reason = null,
    timestamp = new Date().toISOString(),
    tier,
    metadata = {}
  }) {
    this.approverId = approverId;
    this.approverName = approverName;
    this.action = action;
    this.reason = reason;
    this.timestamp = timestamp;
    this.tier = tier;
    this.metadata = metadata;
  }

  toJSON() {
    return {
      approverId: this.approverId,
      approverName: this.approverName,
      action: this.action,
      reason: this.reason,
      timestamp: this.timestamp,
      tier: this.tier,
      metadata: this.metadata
    };
  }
}

/**
 * Approval request
 */
export class ApprovalRequest {
  constructor({
    id = randomUUID(),
    chainId,
    chainName,
    domain,
    entityType, // 'order', 'return', 'purchase_order'
    entityId, // ID of the entity requiring approval
    title,
    description = '',
    amount = null, // Monetary amount (for threshold-based routing)
    requestedBy, // Agent or user ID
    requestedByName = null,
    currentTier = 1,
    status = ApprovalStatus.PENDING,
    decisions = [],
    context = {}, // Additional context for approvers
    createdAt = new Date().toISOString(),
    updatedAt = new Date().toISOString(),
    resolvedAt = null,
    expiresAt = null,
    action = null, // Action to execute on approval
    metadata = {}
  }) {
    this.id = id;
    this.chainId = chainId;
    this.chainName = chainName;
    this.domain = domain;
    this.entityType = entityType;
    this.entityId = entityId;
    this.title = title;
    this.description = description;
    this.amount = amount;
    this.requestedBy = requestedBy;
    this.requestedByName = requestedByName;
    this.currentTier = currentTier;
    this.status = status;
    this.decisions = decisions.map(d => d instanceof ApprovalDecision ? d : new ApprovalDecision(d));
    this.context = context;
    this.createdAt = createdAt;
    this.updatedAt = updatedAt;
    this.resolvedAt = resolvedAt;
    this.expiresAt = expiresAt;
    this.action = action;
    this.metadata = metadata;
    this.timeoutTimer = null;
  }

  /**
   * Get approvals at current tier
   */
  getCurrentTierApprovals() {
    return this.decisions.filter(d => d.tier === this.currentTier && d.action === 'approve');
  }

  /**
   * Check if request is resolved
   */
  isResolved() {
    return [
      ApprovalStatus.APPROVED,
      ApprovalStatus.REJECTED,
      ApprovalStatus.EXPIRED,
      ApprovalStatus.AUTO_APPROVED,
      ApprovalStatus.AUTO_REJECTED,
      ApprovalStatus.CANCELLED
    ].includes(this.status);
  }

  toJSON() {
    return {
      id: this.id,
      chainId: this.chainId,
      chainName: this.chainName,
      domain: this.domain,
      entityType: this.entityType,
      entityId: this.entityId,
      title: this.title,
      description: this.description,
      amount: this.amount,
      requestedBy: this.requestedBy,
      requestedByName: this.requestedByName,
      currentTier: this.currentTier,
      status: this.status,
      decisions: this.decisions.map(d => d.toJSON()),
      context: this.context,
      createdAt: this.createdAt,
      updatedAt: this.updatedAt,
      resolvedAt: this.resolvedAt,
      expiresAt: this.expiresAt,
      action: this.action,
      metadata: this.metadata
    };
  }
}

/**
 * Approval Queue Manager
 */
export class ApprovalQueue extends EventEmitter {
  constructor({
    storePath = null,
    executor = null, // Function to execute approved actions
    notifier = null, // Function to send notifications
    checkInterval = 60000 // Check for timeouts every minute
  }) {
    super();

    this.storePath = storePath;
    this.executor = executor;
    this.notifier = notifier;
    this.checkInterval = checkInterval;

    this.chains = new Map();
    this.requests = new Map();
    this.history = [];

    this.checkTimer = null;
    this.isRunning = false;
  }

  /**
   * Load from storage
   */
  async load() {
    if (!this.storePath) return;

    try {
      const chainsFile = path.join(this.storePath, 'approval-chains.json');
      const requestsFile = path.join(this.storePath, 'approval-requests.json');

      if (fs.existsSync(chainsFile)) {
        const data = JSON.parse(fs.readFileSync(chainsFile, 'utf-8'));
        for (const chainData of data) {
          const chain = new ApprovalChain(chainData);
          this.chains.set(chain.id, chain);
        }
      }

      if (fs.existsSync(requestsFile)) {
        const data = JSON.parse(fs.readFileSync(requestsFile, 'utf-8'));
        for (const requestData of data) {
          const request = new ApprovalRequest(requestData);
          if (!request.isResolved()) {
            this.requests.set(request.id, request);
            this.setupTimeout(request);
          } else {
            this.history.push(request);
          }
        }
      }

      this.emit('loaded', {
        chainCount: this.chains.size,
        pendingCount: this.requests.size
      });
    } catch (error) {
      this.emit('error', { type: 'load', error });
    }
  }

  /**
   * Save to storage
   */
  async save() {
    if (!this.storePath) return;

    try {
      fs.mkdirSync(this.storePath, { recursive: true });

      const chainsFile = path.join(this.storePath, 'approval-chains.json');
      const requestsFile = path.join(this.storePath, 'approval-requests.json');

      const chainsData = Array.from(this.chains.values())
        .filter(c => c instanceof ApprovalChain)
        .map(c => c.toJSON());
      fs.writeFileSync(chainsFile, JSON.stringify(chainsData, null, 2));

      const allRequests = [
        ...Array.from(this.requests.values()),
        ...this.history.slice(-500) // Keep last 500 resolved
      ];
      const requestsData = allRequests.map(r => r.toJSON());
      fs.writeFileSync(requestsFile, JSON.stringify(requestsData, null, 2));

      this.emit('saved');
    } catch (error) {
      this.emit('error', { type: 'save', error });
    }
  }

  /**
   * Register an approval chain
   */
  registerChain(config) {
    const chain = config instanceof ApprovalChain ? config : new ApprovalChain(config);
    this.chains.set(chain.id, chain);

    // Also index by domain
    const domainKey = `domain:${chain.domain}`;
    if (!this.chains.has(domainKey)) {
      this.chains.set(domainKey, []);
    }
    this.chains.get(domainKey).push(chain);

    this.emit('chain:registered', { chain: chain.toJSON() });
    this.save();
    return chain;
  }

  /**
   * Get chain for domain
   */
  getChainForDomain(domain, context = {}) {
    const chains = this.chains.get(`domain:${domain}`) || [];

    for (const chain of chains) {
      if (!chain.enabled) continue;

      // Check conditions
      if (chain.conditions) {
        let matches = true;
        for (const [field, expected] of Object.entries(chain.conditions)) {
          if (context[field] !== expected) {
            matches = false;
            break;
          }
        }
        if (!matches) continue;
      }

      return chain;
    }

    return null;
  }

  /**
   * Create an approval request
   */
  async createRequest({
    domain,
    entityType,
    entityId,
    title,
    description = '',
    amount = null,
    requestedBy,
    requestedByName = null,
    context = {},
    action = null
  }) {
    // Find appropriate chain
    const chain = this.getChainForDomain(domain, { amount, ...context });
    if (!chain) {
      // No approval needed
      return { required: false, chain: null };
    }

    // Determine starting tier based on amount
    const startingTier = amount !== null
      ? chain.getTierForAmount(amount)
      : chain.getTier(1);

    const request = new ApprovalRequest({
      chainId: chain.id,
      chainName: chain.name,
      domain,
      entityType,
      entityId,
      title,
      description,
      amount,
      requestedBy,
      requestedByName,
      currentTier: startingTier.level,
      context,
      action
    });

    // Set expiration
    if (startingTier.timeout) {
      request.expiresAt = new Date(Date.now() + startingTier.timeout).toISOString();
    }

    this.requests.set(request.id, request);
    this.setupTimeout(request);

    // Send notifications
    await this.notifyApprovers(request, startingTier);

    this.emit('request:created', { request: request.toJSON() });
    await this.save();

    return { required: true, request, chain };
  }

  /**
   * Set up timeout for request
   */
  setupTimeout(request) {
    if (request.timeoutTimer) {
      clearTimeout(request.timeoutTimer);
      request.timeoutTimer = null;
    }

    const chain = this.chains.get(request.chainId);
    if (!chain) return;

    const tier = chain.getTier(request.currentTier);
    if (!tier?.timeout) return;

    const timeRemaining = request.expiresAt
      ? new Date(request.expiresAt) - Date.now()
      : tier.timeout;

    if (timeRemaining <= 0) {
      this.handleTimeout(request);
      return;
    }

    request.timeoutTimer = setTimeout(() => {
      this.handleTimeout(request);
    }, timeRemaining);
  }

  /**
   * Handle request timeout
   */
  async handleTimeout(request) {
    if (request.isResolved()) return;

    const chain = this.chains.get(request.chainId);
    if (!chain) return;

    const tier = chain.getTier(request.currentTier);
    if (!tier) return;

    this.emit('request:timeout', { request: request.toJSON(), tier: tier.toJSON() });

    switch (tier.timeoutAction) {
      case 'escalate':
        await this.escalate(request.id, 'system', 'Escalated due to timeout');
        break;

      case 'auto_approve':
        await this.autoApprove(request.id, 'Approved due to timeout');
        break;

      case 'auto_reject':
        await this.autoReject(request.id, 'Rejected due to timeout');
        break;

      default:
        // Mark as expired
        request.status = ApprovalStatus.EXPIRED;
        request.resolvedAt = new Date().toISOString();
        request.updatedAt = request.resolvedAt;
        this.moveToHistory(request);
        this.emit('request:expired', { request: request.toJSON() });
    }

    await this.save();
  }

  /**
   * Record an approval decision
   */
  async approve(requestId, approverId, { approverName = null, reason = null } = {}) {
    const request = this.requests.get(requestId);
    if (!request) {
      throw new Error(`Request not found: ${requestId}`);
    }

    if (request.isResolved()) {
      throw new Error(`Request already resolved: ${request.status}`);
    }

    const chain = this.chains.get(request.chainId);
    if (!chain) {
      throw new Error(`Chain not found: ${request.chainId}`);
    }

    const tier = chain.getTier(request.currentTier);

    // Record decision
    const decision = new ApprovalDecision({
      approverId,
      approverName,
      action: 'approve',
      reason,
      tier: request.currentTier
    });
    request.decisions.push(decision);
    request.updatedAt = new Date().toISOString();

    // Check if enough approvals at current tier
    const tierApprovals = request.getCurrentTierApprovals();

    if (tierApprovals.length >= tier.requiredApprovals) {
      // Check if there's a next tier
      const nextTier = chain.getNextTier(request.currentTier);

      if (nextTier && request.amount > (tier.canApproveAmount || Infinity)) {
        // Escalate to next tier
        await this.escalateToTier(request, nextTier, 'Amount exceeds tier limit');
      } else {
        // Fully approved
        request.status = ApprovalStatus.APPROVED;
        request.resolvedAt = new Date().toISOString();
        this.moveToHistory(request);

        // Execute action if configured
        if (request.action && this.executor) {
          try {
            await this.executor(request.action, {
              requestId: request.id,
              entityType: request.entityType,
              entityId: request.entityId,
              context: request.context
            });
            this.emit('action:executed', { request: request.toJSON() });
          } catch (error) {
            this.emit('action:failed', { request: request.toJSON(), error });
          }
        }

        this.emit('request:approved', { request: request.toJSON() });
      }
    }

    await this.save();
    return request;
  }

  /**
   * Record a rejection decision
   */
  async reject(requestId, approverId, { approverName = null, reason = null } = {}) {
    const request = this.requests.get(requestId);
    if (!request) {
      throw new Error(`Request not found: ${requestId}`);
    }

    if (request.isResolved()) {
      throw new Error(`Request already resolved: ${request.status}`);
    }

    const decision = new ApprovalDecision({
      approverId,
      approverName,
      action: 'reject',
      reason,
      tier: request.currentTier
    });
    request.decisions.push(decision);

    request.status = ApprovalStatus.REJECTED;
    request.resolvedAt = new Date().toISOString();
    request.updatedAt = request.resolvedAt;

    this.moveToHistory(request);
    this.emit('request:rejected', { request: request.toJSON(), reason });
    await this.save();

    return request;
  }

  /**
   * Manually escalate to next tier
   */
  async escalate(requestId, approverId, reason = null) {
    const request = this.requests.get(requestId);
    if (!request) {
      throw new Error(`Request not found: ${requestId}`);
    }

    if (request.isResolved()) {
      throw new Error(`Request already resolved: ${request.status}`);
    }

    const chain = this.chains.get(request.chainId);
    if (!chain) {
      throw new Error(`Chain not found: ${request.chainId}`);
    }

    const nextTier = chain.getNextTier(request.currentTier);
    if (!nextTier) {
      throw new Error('No higher tier to escalate to');
    }

    const decision = new ApprovalDecision({
      approverId,
      action: 'escalate',
      reason,
      tier: request.currentTier
    });
    request.decisions.push(decision);

    await this.escalateToTier(request, nextTier, reason);
    await this.save();

    return request;
  }

  /**
   * Internal: escalate to specific tier
   */
  async escalateToTier(request, tier, reason) {
    if (request.timeoutTimer) {
      clearTimeout(request.timeoutTimer);
      request.timeoutTimer = null;
    }

    request.currentTier = tier.level;
    request.status = ApprovalStatus.ESCALATED;
    request.updatedAt = new Date().toISOString();

    if (tier.timeout) {
      request.expiresAt = new Date(Date.now() + tier.timeout).toISOString();
    }

    this.setupTimeout(request);
    await this.notifyApprovers(request, tier);

    this.emit('request:escalated', {
      request: request.toJSON(),
      tier: tier.toJSON(),
      reason
    });
  }

  /**
   * Auto-approve (system action)
   */
  async autoApprove(requestId, reason) {
    const request = this.requests.get(requestId);
    if (!request || request.isResolved()) return;

    request.status = ApprovalStatus.AUTO_APPROVED;
    request.resolvedAt = new Date().toISOString();
    request.updatedAt = request.resolvedAt;

    const decision = new ApprovalDecision({
      approverId: 'system',
      action: 'approve',
      reason,
      tier: request.currentTier
    });
    request.decisions.push(decision);

    this.moveToHistory(request);

    // Execute action
    if (request.action && this.executor) {
      try {
        await this.executor(request.action, {
          requestId: request.id,
          entityType: request.entityType,
          entityId: request.entityId,
          context: request.context
        });
      } catch (error) {
        this.emit('action:failed', { request: request.toJSON(), error });
      }
    }

    this.emit('request:auto_approved', { request: request.toJSON(), reason });
    await this.save();

    return request;
  }

  /**
   * Auto-reject (system action)
   */
  async autoReject(requestId, reason) {
    const request = this.requests.get(requestId);
    if (!request || request.isResolved()) return;

    request.status = ApprovalStatus.AUTO_REJECTED;
    request.resolvedAt = new Date().toISOString();
    request.updatedAt = request.resolvedAt;

    const decision = new ApprovalDecision({
      approverId: 'system',
      action: 'reject',
      reason,
      tier: request.currentTier
    });
    request.decisions.push(decision);

    this.moveToHistory(request);
    this.emit('request:auto_rejected', { request: request.toJSON(), reason });
    await this.save();

    return request;
  }

  /**
   * Cancel a request
   */
  async cancel(requestId, reason = null) {
    const request = this.requests.get(requestId);
    if (!request) {
      throw new Error(`Request not found: ${requestId}`);
    }

    if (request.isResolved()) {
      throw new Error(`Request already resolved: ${request.status}`);
    }

    if (request.timeoutTimer) {
      clearTimeout(request.timeoutTimer);
      request.timeoutTimer = null;
    }

    request.status = ApprovalStatus.CANCELLED;
    request.resolvedAt = new Date().toISOString();
    request.updatedAt = request.resolvedAt;

    this.moveToHistory(request);
    this.emit('request:cancelled', { request: request.toJSON(), reason });
    await this.save();

    return request;
  }

  /**
   * Move request to history
   */
  moveToHistory(request) {
    if (request.timeoutTimer) {
      clearTimeout(request.timeoutTimer);
      request.timeoutTimer = null;
    }

    this.requests.delete(request.id);
    this.history.push(request);

    // Keep history size manageable
    if (this.history.length > 1000) {
      this.history = this.history.slice(-1000);
    }
  }

  /**
   * Notify approvers
   */
  async notifyApprovers(request, tier) {
    if (!this.notifier) return;

    try {
      await this.notifier({
        type: 'approval_required',
        request: request.toJSON(),
        tier: tier.toJSON(),
        approvers: tier.approvers
      });
    } catch (error) {
      this.emit('notification:failed', { request: request.toJSON(), error });
    }
  }

  /**
   * Get a request
   */
  getRequest(requestId) {
    return this.requests.get(requestId) ||
      this.history.find(r => r.id === requestId);
  }

  /**
   * List pending requests
   */
  listPending({ domain = null, approverId = null } = {}) {
    let requests = Array.from(this.requests.values());

    if (domain) {
      requests = requests.filter(r => r.domain === domain);
    }

    if (approverId) {
      requests = requests.filter(r => {
        const chain = this.chains.get(r.chainId);
        if (!chain) return false;
        const tier = chain.getTier(r.currentTier);
        if (!tier) return false;
        return tier.approvers.includes(approverId) || tier.approvers.includes('*');
      });
    }

    return requests.map(r => r.toJSON());
  }

  /**
   * Get history
   */
  getHistory({ domain = null, status = null, limit = 100 } = {}) {
    let history = this.history;

    if (domain) {
      history = history.filter(r => r.domain === domain);
    }

    if (status) {
      history = history.filter(r => r.status === status);
    }

    return history.slice(-limit).map(r => r.toJSON());
  }

  /**
   * Get queue status
   */
  getStatus() {
    const pending = Array.from(this.requests.values());
    const byDomain = {};
    const byStatus = {};

    for (const request of pending) {
      byDomain[request.domain] = (byDomain[request.domain] || 0) + 1;
    }

    for (const request of this.history) {
      byStatus[request.status] = (byStatus[request.status] || 0) + 1;
    }

    return {
      chainCount: Array.from(this.chains.values()).filter(c => c instanceof ApprovalChain).length,
      pendingCount: pending.length,
      byDomain,
      historyByStatus: byStatus,
      recentHistory: this.history.slice(-5).map(r => r.toJSON())
    };
  }

  /**
   * Start background checking
   */
  start() {
    if (this.isRunning) return;

    this.isRunning = true;

    this.checkTimer = setInterval(() => {
      // Re-check timeouts
      for (const request of this.requests.values()) {
        if (request.expiresAt && new Date(request.expiresAt) <= new Date()) {
          this.handleTimeout(request);
        }
      }
    }, this.checkInterval);

    this.emit('started');
  }

  /**
   * Stop background checking
   */
  stop() {
    if (!this.isRunning) return;

    this.isRunning = false;

    if (this.checkTimer) {
      clearInterval(this.checkTimer);
      this.checkTimer = null;
    }

    // Clear all timeout timers
    for (const request of this.requests.values()) {
      if (request.timeoutTimer) {
        clearTimeout(request.timeoutTimer);
        request.timeoutTimer = null;
      }
    }

    this.emit('stopped');
  }
}

/**
 * Pre-configured approval chain templates
 */
export const ApprovalChainTemplates = {
  // Order approval based on value
  orderApproval: {
    name: 'Order Approval Chain',
    description: 'Multi-tier approval for high-value orders',
    domain: 'orders',
    tiers: [
      {
        level: 1,
        name: 'Auto-Approve',
        description: 'Orders under $1000 auto-approved',
        approvers: ['system'],
        requiredApprovals: 1,
        canApproveAmount: 1000,
        timeout: 0,
        timeoutAction: 'auto_approve'
      },
      {
        level: 2,
        name: 'Manager Review',
        description: 'Manager approval for orders $1000-$5000',
        approvers: ['manager', 'sales_lead'],
        requiredApprovals: 1,
        canApproveAmount: 5000,
        timeout: 3600000, // 1 hour
        timeoutAction: 'escalate'
      },
      {
        level: 3,
        name: 'Director Review',
        description: 'Director approval for orders over $5000',
        approvers: ['director', 'vp_sales'],
        requiredApprovals: 1,
        canApproveAmount: null, // Unlimited
        timeout: 86400000, // 24 hours
        timeoutAction: 'escalate'
      },
      {
        level: 4,
        name: 'Executive Review',
        description: 'Executive approval for escalated orders',
        approvers: ['cfo', 'ceo'],
        requiredApprovals: 1,
        timeout: 172800000, // 48 hours
        timeoutAction: 'auto_reject'
      }
    ]
  },

  // Return approval
  returnApproval: {
    name: 'Return Approval Chain',
    description: 'Approval chain for customer returns',
    domain: 'returns',
    tiers: [
      {
        level: 1,
        name: 'Auto-Approve Small Returns',
        approvers: ['system'],
        requiredApprovals: 1,
        canApproveAmount: 100,
        timeout: 0,
        timeoutAction: 'auto_approve'
      },
      {
        level: 2,
        name: 'Customer Service Review',
        approvers: ['cs_agent', 'cs_lead'],
        requiredApprovals: 1,
        canApproveAmount: 500,
        timeout: 14400000, // 4 hours
        timeoutAction: 'auto_approve'
      },
      {
        level: 3,
        name: 'Manager Review',
        approvers: ['cs_manager'],
        requiredApprovals: 1,
        timeout: 86400000, // 24 hours
        timeoutAction: 'escalate'
      }
    ]
  },

  // Purchase order approval
  purchaseOrderApproval: {
    name: 'Purchase Order Approval Chain',
    description: 'Approval chain for procurement',
    domain: 'purchase_orders',
    tiers: [
      {
        level: 1,
        name: 'Buyer Approval',
        approvers: ['buyer', 'procurement_agent'],
        requiredApprovals: 1,
        canApproveAmount: 5000,
        timeout: 86400000,
        timeoutAction: 'escalate'
      },
      {
        level: 2,
        name: 'Procurement Manager',
        approvers: ['procurement_manager'],
        requiredApprovals: 1,
        canApproveAmount: 25000,
        timeout: 172800000,
        timeoutAction: 'escalate'
      },
      {
        level: 3,
        name: 'Finance Approval',
        approvers: ['finance_manager', 'controller'],
        requiredApprovals: 1,
        canApproveAmount: 100000,
        timeout: 259200000, // 3 days
        timeoutAction: 'escalate'
      },
      {
        level: 4,
        name: 'Executive Approval',
        approvers: ['cfo'],
        requiredApprovals: 1,
        timeout: 604800000, // 7 days
        timeoutAction: 'auto_reject'
      }
    ]
  },

  // Refund approval
  refundApproval: {
    name: 'Refund Approval Chain',
    description: 'Approval chain for refunds',
    domain: 'refunds',
    tiers: [
      {
        level: 1,
        name: 'Auto-Approve',
        approvers: ['system'],
        requiredApprovals: 1,
        canApproveAmount: 50,
        timeout: 0,
        timeoutAction: 'auto_approve'
      },
      {
        level: 2,
        name: 'Agent Review',
        approvers: ['cs_agent'],
        requiredApprovals: 1,
        canApproveAmount: 200,
        timeout: 7200000, // 2 hours
        timeoutAction: 'auto_approve'
      },
      {
        level: 3,
        name: 'Supervisor Review',
        approvers: ['cs_supervisor', 'finance_analyst'],
        requiredApprovals: 1,
        timeout: 86400000,
        timeoutAction: 'escalate'
      }
    ]
  }
};

export default ApprovalQueue;
