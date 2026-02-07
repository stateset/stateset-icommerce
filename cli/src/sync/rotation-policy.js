/**
 * Key Rotation Policy Manager for VES v1.0
 *
 * Manages:
 * - Rotation policies (time-based, usage-based)
 * - Key expiration warnings
 * - Grace periods for old keys after rotation
 * - Scheduled key rotations
 */

import fs from 'fs/promises';
import path from 'path';

/**
 * @typedef {Object} RotationPolicy
 * @property {number} [maxAgeHours] - Rotate after this many hours
 * @property {number} [maxUsageCount] - Rotate after this many uses
 * @property {number} warningThresholdHours - Warn this many hours before expiry
 * @property {number} gracePeriodHours - Allow old key for this long after rotation
 * @property {boolean} enforceExpiry - Reject expired keys
 * @property {boolean} autoRotate - Automatically rotate when policy triggers
 */

/**
 * @typedef {Object} KeyExpiryWarning
 * @property {string} agentId
 * @property {number} keyId
 * @property {string} keyType - 'signing' | 'encryption'
 * @property {string} expiresAt
 * @property {number} hoursRemaining
 * @property {string} severity - 'info' | 'warning' | 'critical'
 */

/**
 * @typedef {Object} ScheduledRotation
 * @property {string} id
 * @property {string} agentId
 * @property {string} keyType
 * @property {number} currentKeyId
 * @property {string} scheduledAt
 * @property {string} reason - 'age_limit' | 'usage_limit' | 'manual' | 'expiry'
 * @property {string} status - 'pending' | 'completed' | 'failed' | 'cancelled'
 * @property {string} [completedAt]
 * @property {number} [newKeyId]
 * @property {string} [errorMessage]
 */

/**
 * @typedef {Object} KeyUsage
 * @property {string} agentId
 * @property {number} keyId
 * @property {string} keyType
 * @property {number} usageCount
 * @property {string} [lastUsedAt]
 */

/**
 * Rotation Policy Manager
 */
export class RotationPolicyManager {
  /**
   * @param {string} configDir - Base config directory (default: .stateset)
   */
  constructor(configDir = '.stateset') {
    this.configDir = configDir;
    this.policiesPath = path.join(configDir, 'rotation-policies.json');
    this.scheduledPath = path.join(configDir, 'scheduled-rotations.json');
    this.usagePath = path.join(configDir, 'key-usage.json');
  }

  /**
   * Get default rotation policy
   * @returns {RotationPolicy}
   */
  getDefaultPolicy() {
    return {
      maxAgeHours: 720, // 30 days
      maxUsageCount: null, // No usage limit by default
      warningThresholdHours: 24, // Warn 24 hours before expiry
      gracePeriodHours: 72, // 3 days grace period
      enforceExpiry: true, // Reject expired keys
      autoRotate: false, // Manual rotation by default
    };
  }

  /**
   * Load all policies from storage
   * @returns {Promise<Object>}
   */
  async _loadPolicies() {
    try {
      const data = await fs.readFile(this.policiesPath, 'utf8');
      return JSON.parse(data);
    } catch (e) {
      if (e.code === 'ENOENT') return {};
      throw e;
    }
  }

  /**
   * Save policies to storage
   * @param {Object} policies
   */
  async _savePolicies(policies) {
    await fs.mkdir(this.configDir, { recursive: true });
    await fs.writeFile(this.policiesPath, JSON.stringify(policies, null, 2));
  }

  /**
   * Get policy key for storage
   * @param {string} agentId
   * @param {string} keyType
   * @returns {string}
   */
  _policyKey(agentId, keyType) {
    return `${agentId}:${keyType}`;
  }

  /**
   * Set rotation policy for an agent/key type
   * @param {string} agentId
   * @param {'signing'|'encryption'} keyType
   * @param {Partial<RotationPolicy>} policy
   * @returns {Promise<RotationPolicy>}
   */
  async setPolicy(agentId, keyType, policy) {
    const policies = await this._loadPolicies();
    const key = this._policyKey(agentId, keyType);

    // Merge with defaults
    const fullPolicy = {
      ...this.getDefaultPolicy(),
      ...policies[key],
      ...policy,
    };

    policies[key] = fullPolicy;
    await this._savePolicies(policies);

    return fullPolicy;
  }

  /**
   * Get rotation policy for an agent/key type
   * @param {string} agentId
   * @param {'signing'|'encryption'} keyType
   * @returns {Promise<RotationPolicy>}
   */
  async getPolicy(agentId, keyType) {
    const policies = await this._loadPolicies();
    const key = this._policyKey(agentId, keyType);

    return {
      ...this.getDefaultPolicy(),
      ...policies[key],
    };
  }

  /**
   * Remove policy (revert to defaults)
   * @param {string} agentId
   * @param {'signing'|'encryption'} keyType
   */
  async removePolicy(agentId, keyType) {
    const policies = await this._loadPolicies();
    const key = this._policyKey(agentId, keyType);
    delete policies[key];
    await this._savePolicies(policies);
  }

  /**
   * List all configured policies
   * @returns {Promise<Array<{agentId: string, keyType: string, policy: RotationPolicy}>>}
   */
  async listPolicies() {
    const policies = await this._loadPolicies();
    return Object.entries(policies).map(([key, policy]) => {
      const [agentId, keyType] = key.split(':');
      return { agentId, keyType, policy };
    });
  }

  // ===========================================================================
  // Key Usage Tracking
  // ===========================================================================

  /**
   * Load key usage data
   * @returns {Promise<Object>}
   */
  async _loadUsage() {
    try {
      const data = await fs.readFile(this.usagePath, 'utf8');
      return JSON.parse(data);
    } catch (e) {
      if (e.code === 'ENOENT') return {};
      throw e;
    }
  }

  /**
   * Save key usage data
   * @param {Object} usage
   */
  async _saveUsage(usage) {
    await fs.mkdir(this.configDir, { recursive: true });
    await fs.writeFile(this.usagePath, JSON.stringify(usage, null, 2));
  }

  /**
   * Get usage key for storage
   * @param {string} agentId
   * @param {string} keyType
   * @param {number} keyId
   * @returns {string}
   */
  _usageKey(agentId, keyType, keyId) {
    return `${agentId}:${keyType}:${keyId}`;
  }

  /**
   * Record a key usage
   * @param {string} agentId
   * @param {'signing'|'encryption'} keyType
   * @param {number} keyId
   * @returns {Promise<KeyUsage>}
   */
  async recordUsage(agentId, keyType, keyId) {
    const usage = await this._loadUsage();
    const key = this._usageKey(agentId, keyType, keyId);

    if (!usage[key]) {
      usage[key] = {
        agentId,
        keyId,
        keyType,
        usageCount: 0,
      };
    }

    usage[key].usageCount++;
    usage[key].lastUsedAt = new Date().toISOString();

    await this._saveUsage(usage);
    return usage[key];
  }

  /**
   * Get key usage stats
   * @param {string} agentId
   * @param {'signing'|'encryption'} keyType
   * @param {number} keyId
   * @returns {Promise<KeyUsage>}
   */
  async getUsage(agentId, keyType, keyId) {
    const usage = await this._loadUsage();
    const key = this._usageKey(agentId, keyType, keyId);

    return (
      usage[key] || {
        agentId,
        keyId,
        keyType,
        usageCount: 0,
        lastUsedAt: null,
      }
    );
  }

  /**
   * Reset usage counter for a key
   * @param {string} agentId
   * @param {'signing'|'encryption'} keyType
   * @param {number} keyId
   */
  async resetUsage(agentId, keyType, keyId) {
    const usage = await this._loadUsage();
    const key = this._usageKey(agentId, keyType, keyId);
    delete usage[key];
    await this._saveUsage(usage);
  }

  // ===========================================================================
  // Rotation Checks
  // ===========================================================================

  /**
   * Check if a key should be rotated based on policy
   * @param {string} agentId
   * @param {number} keyId
   * @param {'signing'|'encryption'} keyType
   * @param {Object} keyData - Key metadata with createdAt
   * @returns {Promise<{shouldRotate: boolean, reason: string|null}>}
   */
  async shouldRotate(agentId, keyId, keyType, keyData) {
    const policy = await this.getPolicy(agentId, keyType);

    // Check age-based rotation
    if (policy.maxAgeHours) {
      const createdAt = new Date(keyData.createdAt);
      const ageHours = (Date.now() - createdAt.getTime()) / (1000 * 60 * 60);

      if (ageHours >= policy.maxAgeHours) {
        return { shouldRotate: true, reason: 'age_limit' };
      }
    }

    // Check usage-based rotation
    if (policy.maxUsageCount) {
      const usage = await this.getUsage(agentId, keyType, keyId);

      if (usage.usageCount >= policy.maxUsageCount) {
        return { shouldRotate: true, reason: 'usage_limit' };
      }
    }

    return { shouldRotate: false, reason: null };
  }

  /**
   * Calculate when a key will expire based on policy
   * @param {string} agentId
   * @param {'signing'|'encryption'} keyType
   * @param {Object} keyData - Key metadata with createdAt
   * @returns {Promise<Date|null>}
   */
  async getExpiryDate(agentId, keyType, keyData) {
    const policy = await this.getPolicy(agentId, keyType);

    if (!policy.maxAgeHours) return null;

    const createdAt = new Date(keyData.createdAt);
    return new Date(createdAt.getTime() + policy.maxAgeHours * 60 * 60 * 1000);
  }

  /**
   * Get expiry warnings for keys
   * @param {Object} keyManager - AgentKeyManager instance
   * @param {string} [agentId] - Specific agent, or all agents if not specified
   * @returns {Promise<Array<KeyExpiryWarning>>}
   */
  async getExpiryWarnings(keyManager, agentId = null) {
    const warnings = [];
    const policies = await this._loadPolicies();

    // Get list of agents to check
    const agentIds = agentId
      ? [agentId]
      : [...new Set(Object.keys(policies).map((k) => k.split(':')[0]))];

    for (const aid of agentIds) {
      for (const keyType of ['signing', 'encryption']) {
        const policy = await this.getPolicy(aid, keyType);
        if (!policy.maxAgeHours) continue;

        const keys =
          keyType === 'signing'
            ? await keyManager.listSigningKeys(aid)
            : await keyManager.listEncryptionKeys(aid);

        for (const key of keys) {
          if (key.revokedAt) continue;

          const expiresAt = await this.getExpiryDate(aid, keyType, key);
          if (!expiresAt) continue;

          const hoursRemaining = (expiresAt.getTime() - Date.now()) / (1000 * 60 * 60);

          // Only warn if within threshold
          if (hoursRemaining <= policy.warningThresholdHours) {
            let severity = 'info';
            if (hoursRemaining <= 0) {
              severity = 'critical';
            } else if (hoursRemaining <= policy.warningThresholdHours / 2) {
              severity = 'warning';
            }

            warnings.push({
              agentId: aid,
              keyId: key.keyId,
              keyType,
              expiresAt: expiresAt.toISOString(),
              hoursRemaining: Math.max(0, hoursRemaining),
              severity,
            });
          }
        }
      }
    }

    return warnings.sort((a, b) => a.hoursRemaining - b.hoursRemaining);
  }

  // ===========================================================================
  // Scheduled Rotations
  // ===========================================================================

  /**
   * Load scheduled rotations
   * @returns {Promise<Array<ScheduledRotation>>}
   */
  async _loadScheduled() {
    try {
      const data = await fs.readFile(this.scheduledPath, 'utf8');
      return JSON.parse(data);
    } catch (e) {
      if (e.code === 'ENOENT') return [];
      throw e;
    }
  }

  /**
   * Save scheduled rotations
   * @param {Array<ScheduledRotation>} rotations
   */
  async _saveScheduled(rotations) {
    await fs.mkdir(this.configDir, { recursive: true });
    await fs.writeFile(this.scheduledPath, JSON.stringify(rotations, null, 2));
  }

  /**
   * Schedule a key rotation
   * @param {string} agentId
   * @param {'signing'|'encryption'} keyType
   * @param {number} currentKeyId
   * @param {string} reason
   * @param {Date} [scheduledAt] - When to rotate (default: now)
   * @returns {Promise<ScheduledRotation>}
   */
  async scheduleRotation(agentId, keyType, currentKeyId, reason, scheduledAt = new Date()) {
    const rotations = await this._loadScheduled();

    // Check for existing pending rotation
    const existing = rotations.find(
      (r) => r.agentId === agentId && r.keyType === keyType && r.status === 'pending',
    );

    if (existing) {
      throw new Error(`Pending rotation already exists for ${agentId} ${keyType} key`);
    }

    const rotation = {
      id: crypto.randomUUID(),
      agentId,
      keyType,
      currentKeyId,
      scheduledAt: scheduledAt.toISOString(),
      reason,
      status: 'pending',
      createdAt: new Date().toISOString(),
    };

    rotations.push(rotation);
    await this._saveScheduled(rotations);

    return rotation;
  }

  /**
   * Mark a rotation as completed
   * @param {string} rotationId
   * @param {number} newKeyId
   */
  async completeRotation(rotationId, newKeyId) {
    const rotations = await this._loadScheduled();
    const rotation = rotations.find((r) => r.id === rotationId);

    if (!rotation) throw new Error(`Rotation ${rotationId} not found`);
    if (rotation.status !== 'pending') {
      throw new Error(`Rotation ${rotationId} is not pending`);
    }

    rotation.status = 'completed';
    rotation.completedAt = new Date().toISOString();
    rotation.newKeyId = newKeyId;

    await this._saveScheduled(rotations);
  }

  /**
   * Mark a rotation as failed
   * @param {string} rotationId
   * @param {string} errorMessage
   */
  async failRotation(rotationId, errorMessage) {
    const rotations = await this._loadScheduled();
    const rotation = rotations.find((r) => r.id === rotationId);

    if (!rotation) throw new Error(`Rotation ${rotationId} not found`);

    rotation.status = 'failed';
    rotation.completedAt = new Date().toISOString();
    rotation.errorMessage = errorMessage;

    await this._saveScheduled(rotations);
  }

  /**
   * Cancel a scheduled rotation
   * @param {string} rotationId
   */
  async cancelRotation(rotationId) {
    const rotations = await this._loadScheduled();
    const rotation = rotations.find((r) => r.id === rotationId);

    if (!rotation) throw new Error(`Rotation ${rotationId} not found`);
    if (rotation.status !== 'pending') {
      throw new Error(`Cannot cancel ${rotation.status} rotation`);
    }

    rotation.status = 'cancelled';
    rotation.completedAt = new Date().toISOString();

    await this._saveScheduled(rotations);
  }

  /**
   * Get pending rotations
   * @param {string} [agentId] - Filter by agent
   * @returns {Promise<Array<ScheduledRotation>>}
   */
  async getPendingRotations(agentId = null) {
    const rotations = await this._loadScheduled();
    return rotations.filter((r) => r.status === 'pending' && (!agentId || r.agentId === agentId));
  }

  /**
   * Get due rotations (scheduled time has passed)
   * @returns {Promise<Array<ScheduledRotation>>}
   */
  async getDueRotations() {
    const rotations = await this._loadScheduled();
    const now = new Date();

    return rotations.filter((r) => r.status === 'pending' && new Date(r.scheduledAt) <= now);
  }

  /**
   * List all scheduled rotations
   * @param {Object} [options]
   * @param {string} [options.agentId] - Filter by agent
   * @param {string} [options.status] - Filter by status
   * @param {number} [options.limit] - Max results
   * @returns {Promise<Array<ScheduledRotation>>}
   */
  async listRotations(options = {}) {
    let rotations = await this._loadScheduled();

    if (options.agentId) {
      rotations = rotations.filter((r) => r.agentId === options.agentId);
    }

    if (options.status) {
      rotations = rotations.filter((r) => r.status === options.status);
    }

    // Sort by scheduled date (newest first)
    rotations.sort((a, b) => new Date(b.scheduledAt) - new Date(a.scheduledAt));

    if (options.limit) {
      rotations = rotations.slice(0, options.limit);
    }

    return rotations;
  }

  /**
   * Clean up old completed/failed/cancelled rotations
   * @param {number} [maxAgeDays=30] - Remove rotations older than this
   */
  async cleanupRotations(maxAgeDays = 30) {
    const rotations = await this._loadScheduled();
    const cutoff = new Date(Date.now() - maxAgeDays * 24 * 60 * 60 * 1000);

    const filtered = rotations.filter(
      (r) => r.status === 'pending' || new Date(r.completedAt || r.scheduledAt) > cutoff,
    );

    await this._saveScheduled(filtered);
    return rotations.length - filtered.length;
  }
}

// =============================================================================
// Singleton instance for convenience
// =============================================================================

let _defaultPolicyManager = null;

/**
 * Get or create default rotation policy manager instance
 * @param {string} [configDir]
 * @returns {RotationPolicyManager}
 */
export function getRotationPolicyManager(configDir = '.stateset') {
  if (!_defaultPolicyManager || _defaultPolicyManager.configDir !== configDir) {
    _defaultPolicyManager = new RotationPolicyManager(configDir);
  }
  return _defaultPolicyManager;
}
