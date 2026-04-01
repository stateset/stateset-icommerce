/**
 * PQC audit event helpers.
 *
 * Logs post-quantum cryptography operations to the audit store for
 * compliance and operational visibility. All logging is best-effort —
 * failures are silently ignored to avoid blocking crypto operations.
 */

let _auditStore = null;

/**
 * Set the audit store instance used for PQC event logging.
 * @param {Object} store - An AuditStore instance with a `log(entry)` method.
 */
export function setPqcAuditStore(store) {
  _auditStore = store;
}

function logPqcEvent(tool, params, result = 'executed', reason = null) {
  if (!_auditStore?.log) return;
  try {
    _auditStore.log({
      tool,
      params,
      result,
      reason,
      level: 'system',
    });
  } catch {
    // Audit logging is best-effort
  }
}

/**
 * Log a signing key generation event.
 * @param {string} agentId
 * @param {number} keyId
 * @param {string} profile - Security profile used ('legacy', 'hybrid', 'pqc-strict').
 * @param {string} algorithm - Key algorithm name.
 */
export function auditKeyGenerated(agentId, keyId, profile, algorithm) {
  logPqcEvent('pqc.key.generated', { agentId, keyId, profile, algorithm });
}

/**
 * Log an encryption key generation event.
 * @param {string} agentId
 * @param {number} keyId
 * @param {string} profile
 * @param {string} algorithm
 */
export function auditEncryptionKeyGenerated(agentId, keyId, profile, algorithm) {
  logPqcEvent('pqc.encryption_key.generated', { agentId, keyId, profile, algorithm });
}

/**
 * Log a key rotation event.
 * @param {string} agentId
 * @param {'signing'|'encryption'} keyType
 * @param {number} oldKeyId
 * @param {number} newKeyId
 * @param {string} reason - Rotation reason ('age_limit', 'usage_limit', 'manual').
 */
export function auditKeyRotated(agentId, keyType, oldKeyId, newKeyId, reason) {
  logPqcEvent('pqc.key.rotated', { agentId, keyType, oldKeyId, newKeyId, reason });
}

/**
 * Log a security profile change.
 * @param {string} previousProfile
 * @param {string} newProfile
 * @param {boolean} forced - Whether the change was forced (downgrade).
 */
export function auditProfileChanged(previousProfile, newProfile, forced = false) {
  const result = forced ? 'forced' : 'executed';
  logPqcEvent('pqc.profile.changed', { previousProfile, newProfile, forced }, result);
}

/**
 * Log a profile downgrade attempt that was blocked.
 * @param {string} currentProfile
 * @param {string} requestedProfile
 */
export function auditProfileDowngradeBlocked(currentProfile, requestedProfile) {
  logPqcEvent(
    'pqc.profile.downgrade_blocked',
    { currentProfile, requestedProfile },
    'denied',
    `Downgrade from ${currentProfile} to ${requestedProfile} is not allowed`,
  );
}

/**
 * Log a key expiry/validity enforcement event.
 * @param {string} agentId
 * @param {'signing'|'encryption'} keyType
 * @param {number} keyId
 * @param {string} status - 'expired', 'grace_period', 'active'.
 */
export function auditKeyValidityCheck(agentId, keyType, keyId, status) {
  logPqcEvent('pqc.key.validity_check', { agentId, keyType, keyId, status });
}
