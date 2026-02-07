/**
 * Encryption Key Group Manager for VES v1.0
 *
 * Manages groups of agents that can decrypt shared encrypted payloads.
 * When encrypting to a group, each member receives a wrapped DEK,
 * allowing any member to decrypt the payload independently.
 *
 * Features:
 * - Create/delete encryption groups
 * - Add/remove group members
 * - Expand groups to recipient keys for encryption
 * - Role-based access (admin, member)
 */

import crypto from 'crypto';
import fs from 'fs/promises';
import path from 'path';
import { getKeyManager } from './keys.js';
import { bufferToHex, hexToBuffer } from './crypto.js';

/**
 * @typedef {Object} GroupMember
 * @property {string} agentId
 * @property {number} encryptionKeyId
 * @property {string} publicKey - X25519 public key (hex)
 * @property {string} role - 'admin' | 'member'
 * @property {string} addedAt - ISO timestamp
 * @property {string} addedBy - Agent ID who added this member
 */

/**
 * @typedef {Object} EncryptionGroup
 * @property {string} groupId
 * @property {string} tenantId
 * @property {string} name
 * @property {string} [description]
 * @property {GroupMember[]} members
 * @property {string} createdBy - Agent ID who created the group
 * @property {string} createdAt - ISO timestamp
 * @property {string} [updatedAt] - ISO timestamp
 */

/**
 * @typedef {Object} RecipientKey
 * @property {number} keyId - Encryption key ID
 * @property {Buffer} publicKey - X25519 public key
 * @property {string} agentId - Owner agent ID
 */

/**
 * Encryption Key Group Manager
 *
 * Stores groups in JSON files under .stateset/groups/
 */
export class GroupKeyManager {
  /**
   * @param {string} configDir - Base config directory (default: .stateset)
   */
  constructor(configDir = '.stateset') {
    this.configDir = configDir;
    this.groupsDir = path.join(configDir, 'groups');
    this.keyManager = getKeyManager(configDir);
  }

  /**
   * Ensure groups directory exists
   */
  async _ensureGroupsDir() {
    await fs.mkdir(this.groupsDir, { recursive: true });
  }

  /**
   * Get path to group file
   * @param {string} groupId
   */
  _groupFilePath(groupId) {
    return path.join(this.groupsDir, `${groupId}.json`);
  }

  /**
   * Get path to tenant index file
   * @param {string} tenantId
   */
  _tenantIndexPath(tenantId) {
    return path.join(this.groupsDir, `_index_${tenantId}.json`);
  }

  /**
   * Load group from file
   * @param {string} groupId
   * @returns {Promise<EncryptionGroup|null>}
   */
  async _loadGroup(groupId) {
    try {
      const data = await fs.readFile(this._groupFilePath(groupId), 'utf8');
      return JSON.parse(data);
    } catch (e) {
      if (e.code === 'ENOENT') return null;
      throw e;
    }
  }

  /**
   * Save group to file
   * @param {EncryptionGroup} group
   */
  async _saveGroup(group) {
    await this._ensureGroupsDir();
    await fs.writeFile(this._groupFilePath(group.groupId), JSON.stringify(group, null, 2));

    // Update tenant index
    await this._updateTenantIndex(group.tenantId, group.groupId, group.name);
  }

  /**
   * Delete group file
   * @param {string} groupId
   * @param {string} tenantId
   */
  async _deleteGroup(groupId, tenantId) {
    try {
      await fs.unlink(this._groupFilePath(groupId));
    } catch (e) {
      if (e.code !== 'ENOENT') throw e;
    }

    // Update tenant index
    await this._removeFromTenantIndex(tenantId, groupId);
  }

  /**
   * Update tenant index with group
   * @param {string} tenantId
   * @param {string} groupId
   * @param {string} name
   */
  async _updateTenantIndex(tenantId, groupId, name) {
    let index = {};
    try {
      const data = await fs.readFile(this._tenantIndexPath(tenantId), 'utf8');
      index = JSON.parse(data);
    } catch (e) {
      if (e.code !== 'ENOENT') throw e;
    }

    index[groupId] = { name, updatedAt: new Date().toISOString() };

    await fs.writeFile(this._tenantIndexPath(tenantId), JSON.stringify(index, null, 2));
  }

  /**
   * Remove group from tenant index
   * @param {string} tenantId
   * @param {string} groupId
   */
  async _removeFromTenantIndex(tenantId, groupId) {
    let index = {};
    try {
      const data = await fs.readFile(this._tenantIndexPath(tenantId), 'utf8');
      index = JSON.parse(data);
    } catch (e) {
      if (e.code !== 'ENOENT') throw e;
    }

    delete index[groupId];

    await fs.writeFile(this._tenantIndexPath(tenantId), JSON.stringify(index, null, 2));
  }

  // ===========================================================================
  // Group Management
  // ===========================================================================

  /**
   * Create a new encryption group
   * @param {string} tenantId
   * @param {string} name - Group name (unique within tenant)
   * @param {string} creatorAgentId - Agent creating the group
   * @param {Object} [options]
   * @param {string} [options.description] - Group description
   * @returns {Promise<EncryptionGroup>}
   */
  async createGroup(tenantId, name, creatorAgentId, options = {}) {
    // Check for existing group with same name
    const existing = await this.getGroupByName(tenantId, name);
    if (existing) {
      throw new Error(`Group '${name}' already exists in tenant`);
    }

    // Get creator's encryption key
    const creatorKey = await this.keyManager.getCurrentEncryptionKey(creatorAgentId);
    if (!creatorKey) {
      throw new Error('Creator must have an encryption key. Run keys:generate first.');
    }

    const groupId = crypto.randomUUID();
    const now = new Date().toISOString();

    const group = {
      groupId,
      tenantId,
      name,
      description: options.description || '',
      members: [
        {
          agentId: creatorAgentId,
          encryptionKeyId: creatorKey.keyId,
          publicKey: bufferToHex(creatorKey.publicKey),
          role: 'admin',
          addedAt: now,
          addedBy: creatorAgentId,
        },
      ],
      createdBy: creatorAgentId,
      createdAt: now,
      updatedAt: now,
    };

    await this._saveGroup(group);
    return group;
  }

  /**
   * Get a group by ID
   * @param {string} groupId
   * @returns {Promise<EncryptionGroup|null>}
   */
  async getGroup(groupId) {
    return this._loadGroup(groupId);
  }

  /**
   * Get a group by name within a tenant
   * @param {string} tenantId
   * @param {string} name
   * @returns {Promise<EncryptionGroup|null>}
   */
  async getGroupByName(tenantId, name) {
    const groups = await this.listGroups(tenantId);
    return groups.find((g) => g.name === name) || null;
  }

  /**
   * List all groups for a tenant
   * @param {string} tenantId
   * @returns {Promise<EncryptionGroup[]>}
   */
  async listGroups(tenantId) {
    try {
      const data = await fs.readFile(this._tenantIndexPath(tenantId), 'utf8');
      const index = JSON.parse(data);

      const groups = [];
      for (const groupId of Object.keys(index)) {
        const group = await this._loadGroup(groupId);
        if (group && group.tenantId === tenantId) {
          groups.push(group);
        }
      }

      return groups.sort((a, b) => a.name.localeCompare(b.name));
    } catch (e) {
      if (e.code === 'ENOENT') return [];
      throw e;
    }
  }

  /**
   * Get groups that an agent is a member of
   * @param {string} agentId
   * @param {string} tenantId
   * @returns {Promise<EncryptionGroup[]>}
   */
  async getAgentGroups(agentId, tenantId) {
    const allGroups = await this.listGroups(tenantId);
    return allGroups.filter((g) => g.members.some((m) => m.agentId === agentId));
  }

  /**
   * Update group metadata
   * @param {string} groupId
   * @param {Object} updates
   * @param {string} updaterAgentId - Agent making the update
   * @returns {Promise<EncryptionGroup>}
   */
  async updateGroup(groupId, updates, updaterAgentId) {
    const group = await this._loadGroup(groupId);
    if (!group) throw new Error(`Group ${groupId} not found`);

    // Check if updater is admin
    const updater = group.members.find((m) => m.agentId === updaterAgentId);
    if (!updater || updater.role !== 'admin') {
      throw new Error('Only admins can update group metadata');
    }

    // Apply allowed updates
    if (updates.name !== undefined) {
      // Check name uniqueness
      const existing = await this.getGroupByName(group.tenantId, updates.name);
      if (existing && existing.groupId !== groupId) {
        throw new Error(`Group '${updates.name}' already exists in tenant`);
      }
      group.name = updates.name;
    }

    if (updates.description !== undefined) {
      group.description = updates.description;
    }

    group.updatedAt = new Date().toISOString();

    await this._saveGroup(group);
    return group;
  }

  /**
   * Delete a group
   * @param {string} groupId
   * @param {string} deleterAgentId - Agent deleting the group
   */
  async deleteGroup(groupId, deleterAgentId) {
    const group = await this._loadGroup(groupId);
    if (!group) throw new Error(`Group ${groupId} not found`);

    // Only creator or admin can delete
    const deleter = group.members.find((m) => m.agentId === deleterAgentId);
    if (!deleter || (deleter.role !== 'admin' && group.createdBy !== deleterAgentId)) {
      throw new Error('Only admins or creator can delete the group');
    }

    await this._deleteGroup(groupId, group.tenantId);
  }

  // ===========================================================================
  // Member Management
  // ===========================================================================

  /**
   * Add a member to a group
   * @param {string} groupId
   * @param {string} agentId - Agent to add
   * @param {string} addedByAgentId - Agent performing the add
   * @param {Object} [options]
   * @param {string} [options.role='member'] - Role for the new member
   * @returns {Promise<EncryptionGroup>}
   */
  async addMember(groupId, agentId, addedByAgentId, options = {}) {
    const group = await this._loadGroup(groupId);
    if (!group) throw new Error(`Group ${groupId} not found`);

    // Check if adder has admin rights
    const adder = group.members.find((m) => m.agentId === addedByAgentId);
    if (!adder || adder.role !== 'admin') {
      throw new Error('Only admins can add members');
    }

    // Check if already a member
    if (group.members.find((m) => m.agentId === agentId)) {
      throw new Error(`Agent ${agentId} is already a member`);
    }

    // Get agent's encryption key
    const agentKey = await this.keyManager.getCurrentEncryptionKey(agentId);
    if (!agentKey) {
      throw new Error(`Agent ${agentId} has no encryption key. They must run keys:generate first.`);
    }

    const role = options.role === 'admin' ? 'admin' : 'member';

    group.members.push({
      agentId,
      encryptionKeyId: agentKey.keyId,
      publicKey: bufferToHex(agentKey.publicKey),
      role,
      addedAt: new Date().toISOString(),
      addedBy: addedByAgentId,
    });

    group.updatedAt = new Date().toISOString();

    await this._saveGroup(group);
    return group;
  }

  /**
   * Remove a member from a group
   * @param {string} groupId
   * @param {string} agentId - Agent to remove
   * @param {string} removedByAgentId - Agent performing the removal
   * @returns {Promise<EncryptionGroup>}
   */
  async removeMember(groupId, agentId, removedByAgentId) {
    const group = await this._loadGroup(groupId);
    if (!group) throw new Error(`Group ${groupId} not found`);

    // Check if remover has admin rights
    const remover = group.members.find((m) => m.agentId === removedByAgentId);
    if (!remover || remover.role !== 'admin') {
      throw new Error('Only admins can remove members');
    }

    // Cannot remove the creator
    if (agentId === group.createdBy) {
      throw new Error('Cannot remove the group creator');
    }

    // Cannot remove yourself if you're the last admin
    if (agentId === removedByAgentId) {
      const admins = group.members.filter((m) => m.role === 'admin');
      if (admins.length === 1) {
        throw new Error('Cannot remove yourself as the last admin');
      }
    }

    // Find and remove member
    const memberIndex = group.members.findIndex((m) => m.agentId === agentId);
    if (memberIndex === -1) {
      throw new Error(`Agent ${agentId} is not a member`);
    }

    group.members.splice(memberIndex, 1);
    group.updatedAt = new Date().toISOString();

    await this._saveGroup(group);
    return group;
  }

  /**
   * Update a member's role
   * @param {string} groupId
   * @param {string} agentId - Agent to update
   * @param {string} newRole - New role ('admin' | 'member')
   * @param {string} updaterAgentId - Agent performing the update
   * @returns {Promise<EncryptionGroup>}
   */
  async updateMemberRole(groupId, agentId, newRole, updaterAgentId) {
    const group = await this._loadGroup(groupId);
    if (!group) throw new Error(`Group ${groupId} not found`);

    // Check if updater has admin rights
    const updater = group.members.find((m) => m.agentId === updaterAgentId);
    if (!updater || updater.role !== 'admin') {
      throw new Error('Only admins can update member roles');
    }

    // Find member
    const member = group.members.find((m) => m.agentId === agentId);
    if (!member) {
      throw new Error(`Agent ${agentId} is not a member`);
    }

    // Validate role
    if (!['admin', 'member'].includes(newRole)) {
      throw new Error(`Invalid role: ${newRole}`);
    }

    // Cannot demote the last admin
    if (member.role === 'admin' && newRole === 'member') {
      const admins = group.members.filter((m) => m.role === 'admin');
      if (admins.length === 1) {
        throw new Error('Cannot demote the last admin');
      }
    }

    member.role = newRole;
    group.updatedAt = new Date().toISOString();

    await this._saveGroup(group);
    return group;
  }

  /**
   * Update a member's encryption key (after key rotation)
   * @param {string} groupId
   * @param {string} agentId - Agent whose key to update
   * @returns {Promise<EncryptionGroup>}
   */
  async refreshMemberKey(groupId, agentId) {
    const group = await this._loadGroup(groupId);
    if (!group) throw new Error(`Group ${groupId} not found`);

    // Find member
    const member = group.members.find((m) => m.agentId === agentId);
    if (!member) {
      throw new Error(`Agent ${agentId} is not a member`);
    }

    // Get current encryption key
    const key = await this.keyManager.getCurrentEncryptionKey(agentId);
    if (!key) {
      throw new Error(`Agent ${agentId} has no encryption key`);
    }

    // Update if key changed
    if (member.encryptionKeyId !== key.keyId) {
      member.encryptionKeyId = key.keyId;
      member.publicKey = bufferToHex(key.publicKey);
      group.updatedAt = new Date().toISOString();
      await this._saveGroup(group);
    }

    return group;
  }

  // ===========================================================================
  // Encryption Support
  // ===========================================================================

  /**
   * Expand a group to recipient keys for encryption
   * Returns the public key of each member for VES-ENC-1 multi-recipient encryption
   * @param {string} groupId
   * @returns {Promise<RecipientKey[]>}
   */
  async expandGroupToRecipients(groupId) {
    const group = await this._loadGroup(groupId);
    if (!group) throw new Error(`Group ${groupId} not found`);

    if (group.members.length === 0) {
      throw new Error('Group has no members');
    }

    return group.members.map((m) => ({
      keyId: m.encryptionKeyId,
      publicKey: hexToBuffer(m.publicKey),
      agentId: m.agentId,
    }));
  }

  /**
   * Get recipient keys for multiple groups
   * Useful when encrypting to multiple groups at once
   * @param {string[]} groupIds
   * @returns {Promise<RecipientKey[]>}
   */
  async expandGroupsToRecipients(groupIds) {
    const allRecipients = [];
    const seenAgents = new Set();

    for (const groupId of groupIds) {
      const recipients = await this.expandGroupToRecipients(groupId);

      for (const recipient of recipients) {
        // Deduplicate by agent ID
        if (!seenAgents.has(recipient.agentId)) {
          seenAgents.add(recipient.agentId);
          allRecipients.push(recipient);
        }
      }
    }

    return allRecipients;
  }

  /**
   * Check if an agent can decrypt a payload encrypted to a group
   * @param {string} groupId
   * @param {string} agentId
   * @returns {Promise<boolean>}
   */
  async canDecrypt(groupId, agentId) {
    const group = await this._loadGroup(groupId);
    if (!group) return false;

    return group.members.some((m) => m.agentId === agentId);
  }

  /**
   * Find the recipient entry for an agent in a group
   * Useful for locating the right wrapped DEK during decryption
   * @param {string} groupId
   * @param {string} agentId
   * @returns {Promise<RecipientKey|null>}
   */
  async findRecipientForAgent(groupId, agentId) {
    const group = await this._loadGroup(groupId);
    if (!group) return null;

    const member = group.members.find((m) => m.agentId === agentId);
    if (!member) return null;

    return {
      keyId: member.encryptionKeyId,
      publicKey: hexToBuffer(member.publicKey),
      agentId: member.agentId,
    };
  }

  // ===========================================================================
  // Statistics
  // ===========================================================================

  /**
   * Get group statistics
   * @param {string} groupId
   * @returns {Promise<Object>}
   */
  async getGroupStats(groupId) {
    const group = await this._loadGroup(groupId);
    if (!group) throw new Error(`Group ${groupId} not found`);

    const admins = group.members.filter((m) => m.role === 'admin');
    const members = group.members.filter((m) => m.role === 'member');

    return {
      groupId: group.groupId,
      name: group.name,
      tenantId: group.tenantId,
      totalMembers: group.members.length,
      adminCount: admins.length,
      memberCount: members.length,
      createdBy: group.createdBy,
      createdAt: group.createdAt,
      updatedAt: group.updatedAt,
    };
  }
}

// =============================================================================
// Singleton instance for convenience
// =============================================================================

let _defaultGroupManager = null;

/**
 * Get or create default group manager instance
 * @param {string} [configDir]
 * @returns {GroupKeyManager}
 */
export function getGroupManager(configDir = '.stateset') {
  if (!_defaultGroupManager || _defaultGroupManager.configDir !== configDir) {
    _defaultGroupManager = new GroupKeyManager(configDir);
  }
  return _defaultGroupManager;
}
