/**
 * Agent Key Management for VES v1.0
 *
 * Manages:
 * - Ed25519 signing keys (agent signatures)
 * - X25519 encryption keys (payload encryption)
 * - Key storage and rotation
 * - Key registration with sequencer
 */

import crypto from 'crypto';
import fs from 'fs/promises';
import path from 'path';
import { bufferToHex, hexToBuffer } from './crypto.js';
import { getRotationPolicyManager } from './rotation-policy.js';

/**
 * @typedef {Object} SigningKeyPair
 * @property {number} keyId - Key identifier
 * @property {Buffer} publicKey - 32-byte Ed25519 public key
 * @property {Buffer} privateKey - 32-byte Ed25519 private key (seed)
 * @property {string} createdAt - ISO timestamp
 * @property {string} [revokedAt] - ISO timestamp if revoked
 */

/**
 * @typedef {Object} EncryptionKeyPair
 * @property {number} keyId - Key identifier
 * @property {Buffer} publicKey - 32-byte X25519 public key
 * @property {Buffer} privateKey - 32-byte X25519 private key
 * @property {string} createdAt - ISO timestamp
 * @property {string} [revokedAt] - ISO timestamp if revoked
 */

/**
 * Agent Key Manager
 *
 * Stores keys in JSON files under .stateset/keys/{agentId}/
 */
export class AgentKeyManager {
  /**
   * @param {string} configDir - Base config directory (default: .stateset)
   */
  constructor(configDir = '.stateset') {
    this.keysDir = path.join(configDir, 'keys');
  }

  /**
   * Ensure keys directory exists for an agent
   * @param {string} agentId
   */
  async _ensureAgentDir(agentId) {
    const agentDir = path.join(this.keysDir, agentId);
    await fs.mkdir(agentDir, { recursive: true });
    return agentDir;
  }

  /**
   * Get path to key file
   * @param {string} agentId
   * @param {'signing'|'encryption'} keyType
   */
  _keyFilePath(agentId, keyType) {
    return path.join(this.keysDir, agentId, `${keyType}-keys.json`);
  }

  /**
   * Load keys from file
   * @param {string} agentId
   * @param {'signing'|'encryption'} keyType
   * @returns {Promise<Array<Object>>}
   */
  async _loadKeys(agentId, keyType) {
    try {
      const data = await fs.readFile(this._keyFilePath(agentId, keyType), 'utf8');
      const keys = JSON.parse(data);
      // Convert hex strings back to buffers
      return keys.map((k) => ({
        ...k,
        publicKey: hexToBuffer(k.publicKey),
        privateKey: hexToBuffer(k.privateKey),
      }));
    } catch (e) {
      if (e.code === 'ENOENT') return [];
      throw e;
    }
  }

  /**
   * Save keys to file
   * @param {string} agentId
   * @param {'signing'|'encryption'} keyType
   * @param {Array<Object>} keys
   */
  async _saveKeys(agentId, keyType, keys) {
    await this._ensureAgentDir(agentId);
    // Convert buffers to hex for storage
    const serialized = keys.map((k) => ({
      ...k,
      publicKey: bufferToHex(k.publicKey),
      privateKey: bufferToHex(k.privateKey),
    }));
    await fs.writeFile(
      this._keyFilePath(agentId, keyType),
      JSON.stringify(serialized, null, 2),
      { mode: 0o600 }, // Restrict permissions
    );
  }

  /**
   * Get next available key ID
   * @param {string} agentId
   * @param {'signing'|'encryption'} keyType
   * @returns {Promise<number>}
   */
  async _nextKeyId(agentId, keyType) {
    const keys = await this._loadKeys(agentId, keyType);
    if (keys.length === 0) return 1;
    return Math.max(...keys.map((k) => k.keyId)) + 1;
  }

  // ===========================================================================
  // Ed25519 Signing Keys
  // ===========================================================================

  /**
   * Generate a new Ed25519 signing key pair
   * @param {string} agentId
   * @returns {Promise<SigningKeyPair>}
   */
  async generateSigningKey(agentId) {
    const keyId = await this._nextKeyId(agentId, 'signing');

    // Generate Ed25519 key pair
    const { publicKey, privateKey } = crypto.generateKeyPairSync('ed25519');

    // Extract raw bytes
    const pubDer = publicKey.export({ type: 'spki', format: 'der' });
    const privDer = privateKey.export({ type: 'pkcs8', format: 'der' });

    // Ed25519 public key is last 32 bytes of SPKI DER
    const pubKey32 = pubDer.subarray(-32);
    // Ed25519 private key (seed) is last 32 bytes of PKCS#8 DER
    const privKey32 = privDer.subarray(-32);

    const keyPair = {
      keyId,
      publicKey: pubKey32,
      privateKey: privKey32,
      createdAt: new Date().toISOString(),
    };

    // Save to storage
    const keys = await this._loadKeys(agentId, 'signing');
    keys.push(keyPair);
    await this._saveKeys(agentId, 'signing', keys);

    return keyPair;
  }

  /**
   * Get current (latest non-revoked) signing key
   * @param {string} agentId
   * @returns {Promise<SigningKeyPair|null>}
   */
  async getCurrentSigningKey(agentId) {
    const keys = await this._loadKeys(agentId, 'signing');
    // Find latest non-revoked key
    const activeKeys = keys.filter((k) => !k.revokedAt);
    if (activeKeys.length === 0) return null;
    return activeKeys.reduce((a, b) => (a.keyId > b.keyId ? a : b));
  }

  /**
   * Get a specific signing key by ID
   * @param {string} agentId
   * @param {number} keyId
   * @returns {Promise<SigningKeyPair|null>}
   */
  async getSigningKey(agentId, keyId) {
    const keys = await this._loadKeys(agentId, 'signing');
    return keys.find((k) => k.keyId === keyId) || null;
  }

  /**
   * List all signing keys for an agent
   * @param {string} agentId
   * @returns {Promise<Array<SigningKeyPair>>}
   */
  async listSigningKeys(agentId) {
    return this._loadKeys(agentId, 'signing');
  }

  /**
   * Revoke a signing key
   * @param {string} agentId
   * @param {number} keyId
   */
  async revokeSigningKey(agentId, keyId) {
    const keys = await this._loadKeys(agentId, 'signing');
    const key = keys.find((k) => k.keyId === keyId);
    if (!key) throw new Error(`Signing key ${keyId} not found`);
    if (key.revokedAt) throw new Error(`Signing key ${keyId} already revoked`);

    key.revokedAt = new Date().toISOString();
    await this._saveKeys(agentId, 'signing', keys);
  }

  // ===========================================================================
  // X25519 Encryption Keys
  // ===========================================================================

  /**
   * Generate a new X25519 encryption key pair
   * @param {string} agentId
   * @returns {Promise<EncryptionKeyPair>}
   */
  async generateEncryptionKey(agentId) {
    const keyId = await this._nextKeyId(agentId, 'encryption');

    // Generate X25519 key pair
    const { publicKey, privateKey } = crypto.generateKeyPairSync('x25519');

    // Extract raw bytes
    const pubDer = publicKey.export({ type: 'spki', format: 'der' });
    const privDer = privateKey.export({ type: 'pkcs8', format: 'der' });

    // X25519 keys are last 32 bytes of DER encoding
    const pubKey32 = pubDer.subarray(-32);
    const privKey32 = privDer.subarray(-32);

    const keyPair = {
      keyId,
      publicKey: pubKey32,
      privateKey: privKey32,
      createdAt: new Date().toISOString(),
    };

    // Save to storage
    const keys = await this._loadKeys(agentId, 'encryption');
    keys.push(keyPair);
    await this._saveKeys(agentId, 'encryption', keys);

    return keyPair;
  }

  /**
   * Get current (latest non-revoked) encryption key
   * @param {string} agentId
   * @returns {Promise<EncryptionKeyPair|null>}
   */
  async getCurrentEncryptionKey(agentId) {
    const keys = await this._loadKeys(agentId, 'encryption');
    const activeKeys = keys.filter((k) => !k.revokedAt);
    if (activeKeys.length === 0) return null;
    return activeKeys.reduce((a, b) => (a.keyId > b.keyId ? a : b));
  }

  /**
   * Get a specific encryption key by ID
   * @param {string} agentId
   * @param {number} keyId
   * @returns {Promise<EncryptionKeyPair|null>}
   */
  async getEncryptionKey(agentId, keyId) {
    const keys = await this._loadKeys(agentId, 'encryption');
    return keys.find((k) => k.keyId === keyId) || null;
  }

  /**
   * List all encryption keys for an agent
   * @param {string} agentId
   * @returns {Promise<Array<EncryptionKeyPair>>}
   */
  async listEncryptionKeys(agentId) {
    return this._loadKeys(agentId, 'encryption');
  }

  /**
   * Revoke an encryption key
   * @param {string} agentId
   * @param {number} keyId
   */
  async revokeEncryptionKey(agentId, keyId) {
    const keys = await this._loadKeys(agentId, 'encryption');
    const key = keys.find((k) => k.keyId === keyId);
    if (!key) throw new Error(`Encryption key ${keyId} not found`);
    if (key.revokedAt) throw new Error(`Encryption key ${keyId} already revoked`);

    key.revokedAt = new Date().toISOString();
    await this._saveKeys(agentId, 'encryption', keys);
  }

  // ===========================================================================
  // Key Initialization
  // ===========================================================================

  /**
   * Ensure agent has at least one signing and encryption key
   * Generates keys if none exist
   * @param {string} agentId
   * @returns {Promise<{signingKey: SigningKeyPair, encryptionKey: EncryptionKeyPair}>}
   */
  async ensureKeys(agentId) {
    let signingKey = await this.getCurrentSigningKey(agentId);
    if (!signingKey) {
      signingKey = await this.generateSigningKey(agentId);
    }

    let encryptionKey = await this.getCurrentEncryptionKey(agentId);
    if (!encryptionKey) {
      encryptionKey = await this.generateEncryptionKey(agentId);
    }

    return { signingKey, encryptionKey };
  }

  /**
   * Check if agent has valid keys
   * @param {string} agentId
   * @returns {Promise<{hasSigningKey: boolean, hasEncryptionKey: boolean}>}
   */
  async hasKeys(agentId) {
    const signingKey = await this.getCurrentSigningKey(agentId);
    const encryptionKey = await this.getCurrentEncryptionKey(agentId);
    return {
      hasSigningKey: signingKey !== null,
      hasEncryptionKey: encryptionKey !== null,
    };
  }

  // ===========================================================================
  // Key Export (for registration with sequencer)
  // ===========================================================================

  /**
   * Export signing public key in format for sequencer registration
   * @param {string} agentId
   * @param {number} [keyId] - Specific key ID, or latest if not specified
   * @returns {Promise<{keyId: number, publicKey: string, createdAt: string}>}
   */
  async exportSigningPublicKey(agentId, keyId = null) {
    const key = keyId
      ? await this.getSigningKey(agentId, keyId)
      : await this.getCurrentSigningKey(agentId);

    if (!key) throw new Error('No signing key found');

    return {
      keyId: key.keyId,
      publicKey: bufferToHex(key.publicKey),
      createdAt: key.createdAt,
    };
  }

  /**
   * Export encryption public key in format for sequencer registration
   * @param {string} agentId
   * @param {number} [keyId] - Specific key ID, or latest if not specified
   * @returns {Promise<{keyId: number, publicKey: string, createdAt: string}>}
   */
  async exportEncryptionPublicKey(agentId, keyId = null) {
    const key = keyId
      ? await this.getEncryptionKey(agentId, keyId)
      : await this.getCurrentEncryptionKey(agentId);

    if (!key) throw new Error('No encryption key found');

    return {
      keyId: key.keyId,
      publicKey: bufferToHex(key.publicKey),
      createdAt: key.createdAt,
    };
  }

  // ===========================================================================
  // Key Import (for testing or migration)
  // ===========================================================================

  /**
   * Import an existing signing key pair
   * @param {string} agentId
   * @param {Buffer} publicKey - 32-byte Ed25519 public key
   * @param {Buffer} privateKey - 32-byte Ed25519 private key (seed)
   * @param {number} [keyId] - Explicit key ID (auto-assigned if not specified)
   * @returns {Promise<SigningKeyPair>}
   */
  async importSigningKey(agentId, publicKey, privateKey, keyId = null) {
    if (publicKey.length !== 32) throw new Error('Public key must be 32 bytes');
    if (privateKey.length !== 32) throw new Error('Private key must be 32 bytes');

    const keys = await this._loadKeys(agentId, 'signing');
    const newKeyId = keyId ?? (keys.length === 0 ? 1 : Math.max(...keys.map((k) => k.keyId)) + 1);

    // Check for duplicate keyId
    if (keys.some((k) => k.keyId === newKeyId)) {
      throw new Error(`Signing key ${newKeyId} already exists`);
    }

    const keyPair = {
      keyId: newKeyId,
      publicKey,
      privateKey,
      createdAt: new Date().toISOString(),
    };

    keys.push(keyPair);
    await this._saveKeys(agentId, 'signing', keys);

    return keyPair;
  }

  /**
   * Import an existing encryption key pair
   * @param {string} agentId
   * @param {Buffer} publicKey - 32-byte X25519 public key
   * @param {Buffer} privateKey - 32-byte X25519 private key
   * @param {number} [keyId] - Explicit key ID (auto-assigned if not specified)
   * @returns {Promise<EncryptionKeyPair>}
   */
  async importEncryptionKey(agentId, publicKey, privateKey, keyId = null) {
    if (publicKey.length !== 32) throw new Error('Public key must be 32 bytes');
    if (privateKey.length !== 32) throw new Error('Private key must be 32 bytes');

    const keys = await this._loadKeys(agentId, 'encryption');
    const newKeyId = keyId ?? (keys.length === 0 ? 1 : Math.max(...keys.map((k) => k.keyId)) + 1);

    if (keys.some((k) => k.keyId === newKeyId)) {
      throw new Error(`Encryption key ${newKeyId} already exists`);
    }

    const keyPair = {
      keyId: newKeyId,
      publicKey,
      privateKey,
      createdAt: new Date().toISOString(),
    };

    keys.push(keyPair);
    await this._saveKeys(agentId, 'encryption', keys);

    return keyPair;
  }

  // ===========================================================================
  // Policy-Aware Key Rotation
  // ===========================================================================

  /**
   * Rotate key with policy-aware expiration and grace period
   * @param {string} agentId
   * @param {'signing'|'encryption'} keyType
   * @param {Object} [options]
   * @param {number} [options.gracePeriodHours] - Override policy grace period
   * @param {string} [options.reason] - Reason for rotation
   * @returns {Promise<{oldKey: Object, newKey: Object, graceUntil: string}>}
   */
  async rotateKeyWithPolicy(agentId, keyType, options = {}) {
    const policyManager = getRotationPolicyManager(this.keysDir.replace('/keys', ''));
    const policy = await policyManager.getPolicy(agentId, keyType);
    const gracePeriodHours = options.gracePeriodHours ?? policy.gracePeriodHours;

    // Get current key
    const currentKey =
      keyType === 'signing'
        ? await this.getCurrentSigningKey(agentId)
        : await this.getCurrentEncryptionKey(agentId);

    if (!currentKey) {
      throw new Error(`No ${keyType} key found for agent ${agentId}`);
    }

    // Generate new key
    const newKey =
      keyType === 'signing'
        ? await this.generateSigningKey(agentId)
        : await this.generateEncryptionKey(agentId);

    // Set grace period on old key instead of immediate revocation
    const graceUntil = new Date();
    graceUntil.setHours(graceUntil.getHours() + gracePeriodHours);

    await this._setKeyGracePeriod(agentId, keyType, currentKey.keyId, graceUntil);

    // Reset usage counter for new key
    await policyManager.resetUsage(agentId, keyType, newKey.keyId);

    return {
      oldKey: currentKey,
      newKey,
      graceUntil: graceUntil.toISOString(),
    };
  }

  /**
   * Set grace period on a key (instead of immediate revocation)
   * @param {string} agentId
   * @param {'signing'|'encryption'} keyType
   * @param {number} keyId
   * @param {Date} graceUntil
   */
  async _setKeyGracePeriod(agentId, keyType, keyId, graceUntil) {
    const keys = await this._loadKeys(agentId, keyType);
    const key = keys.find((k) => k.keyId === keyId);

    if (!key) throw new Error(`${keyType} key ${keyId} not found`);

    key.graceUntil = graceUntil.toISOString();
    key.expiresAt = key.expiresAt || graceUntil.toISOString();

    await this._saveKeys(agentId, keyType, keys);
  }

  /**
   * Set explicit expiration date on a key
   * @param {string} agentId
   * @param {'signing'|'encryption'} keyType
   * @param {number} keyId
   * @param {Date} expiresAt
   */
  async setKeyExpiration(agentId, keyType, keyId, expiresAt) {
    const keys = await this._loadKeys(agentId, keyType);
    const key = keys.find((k) => k.keyId === keyId);

    if (!key) throw new Error(`${keyType} key ${keyId} not found`);

    key.expiresAt = expiresAt.toISOString();

    await this._saveKeys(agentId, keyType, keys);
  }

  /**
   * Get key status considering grace period and expiration
   * @param {string} agentId
   * @param {'signing'|'encryption'} keyType
   * @param {number} keyId
   * @returns {Promise<'active'|'grace_period'|'expired'|'revoked'|'not_found'>}
   */
  async getKeyStatus(agentId, keyType, keyId) {
    const key =
      keyType === 'signing'
        ? await this.getSigningKey(agentId, keyId)
        : await this.getEncryptionKey(agentId, keyId);

    if (!key) return 'not_found';
    if (key.revokedAt) return 'revoked';

    const now = new Date();

    // Check grace period first (rotated but still valid)
    if (key.graceUntil) {
      const graceUntil = new Date(key.graceUntil);
      if (now > graceUntil) {
        return 'expired';
      }
      // Has grace period set, meaning it was rotated
      if (key.expiresAt && new Date(key.expiresAt) <= now) {
        return 'grace_period';
      }
    }

    // Check explicit expiration
    if (key.expiresAt && new Date(key.expiresAt) < now) {
      return 'expired';
    }

    return 'active';
  }

  /**
   * Get detailed key info with status
   * @param {string} agentId
   * @param {'signing'|'encryption'} keyType
   * @param {number} keyId
   * @returns {Promise<Object|null>}
   */
  async getKeyInfo(agentId, keyType, keyId) {
    const key =
      keyType === 'signing'
        ? await this.getSigningKey(agentId, keyId)
        : await this.getEncryptionKey(agentId, keyId);

    if (!key) return null;

    const status = await this.getKeyStatus(agentId, keyType, keyId);
    const policyManager = getRotationPolicyManager(this.keysDir.replace('/keys', ''));
    const usage = await policyManager.getUsage(agentId, keyType, keyId);

    return {
      ...key,
      status,
      usageCount: usage.usageCount,
      lastUsedAt: usage.lastUsedAt,
    };
  }

  /**
   * Check if a key is usable (active or in grace period)
   * @param {string} agentId
   * @param {'signing'|'encryption'} keyType
   * @param {number} keyId
   * @returns {Promise<boolean>}
   */
  async isKeyUsable(agentId, keyType, keyId) {
    const status = await this.getKeyStatus(agentId, keyType, keyId);
    return status === 'active' || status === 'grace_period';
  }

  /**
   * Batch rotate keys for multiple agents
   * @param {Array<{agentId: string, keyType: 'signing'|'encryption', options?: Object}>} rotations
   * @returns {Promise<Array<{agentId: string, keyType: string, success: boolean, oldKey?: Object, newKey?: Object, error?: string}>>}
   */
  async batchRotate(rotations) {
    const results = [];

    for (const { agentId, keyType, options } of rotations) {
      try {
        const result = await this.rotateKeyWithPolicy(agentId, keyType, options);
        results.push({
          agentId,
          keyType,
          success: true,
          oldKey: { keyId: result.oldKey.keyId },
          newKey: { keyId: result.newKey.keyId },
          graceUntil: result.graceUntil,
        });
      } catch (error) {
        results.push({
          agentId,
          keyType,
          success: false,
          error: error.message,
        });
      }
    }

    return results;
  }

  /**
   * Clean up expired keys (past grace period)
   * @param {string} agentId
   * @param {'signing'|'encryption'} keyType
   * @returns {Promise<number>} Number of keys cleaned up
   */
  async cleanupExpiredKeys(agentId, keyType) {
    const keys = await this._loadKeys(agentId, keyType);
    const now = new Date();
    let cleanedUp = 0;

    for (const key of keys) {
      if (key.revokedAt) continue; // Already revoked

      // Check if past grace period
      if (key.graceUntil && new Date(key.graceUntil) < now) {
        key.revokedAt = new Date().toISOString();
        cleanedUp++;
      }
    }

    if (cleanedUp > 0) {
      await this._saveKeys(agentId, keyType, keys);
    }

    return cleanedUp;
  }

  /**
   * Process auto-rotation based on policies
   * @param {string} agentId
   * @returns {Promise<Array<{keyType: string, rotated: boolean, reason?: string}>>}
   */
  async processAutoRotation(agentId) {
    const policyManager = getRotationPolicyManager(this.keysDir.replace('/keys', ''));
    const results = [];

    for (const keyType of ['signing', 'encryption']) {
      const policy = await policyManager.getPolicy(agentId, keyType);

      if (!policy.autoRotate) {
        results.push({ keyType, rotated: false, reason: 'auto_rotate_disabled' });
        continue;
      }

      const currentKey =
        keyType === 'signing'
          ? await this.getCurrentSigningKey(agentId)
          : await this.getCurrentEncryptionKey(agentId);

      if (!currentKey) {
        results.push({ keyType, rotated: false, reason: 'no_current_key' });
        continue;
      }

      const { shouldRotate, reason } = await policyManager.shouldRotate(
        agentId,
        currentKey.keyId,
        keyType,
        currentKey,
      );

      if (shouldRotate) {
        await this.rotateKeyWithPolicy(agentId, keyType, { reason });
        results.push({ keyType, rotated: true, reason });
      } else {
        results.push({ keyType, rotated: false, reason: 'not_needed' });
      }
    }

    return results;
  }
}

// =============================================================================
// Singleton instance for convenience
// =============================================================================

let _defaultManager = null;

/**
 * Get or create default key manager instance
 * @param {string} [configDir]
 * @returns {AgentKeyManager}
 */
export function getKeyManager(configDir = '.stateset') {
  if (!_defaultManager || _defaultManager.keysDir !== path.join(configDir, 'keys')) {
    _defaultManager = new AgentKeyManager(configDir);
  }
  return _defaultManager;
}
