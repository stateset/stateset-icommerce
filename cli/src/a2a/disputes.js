/**
 * A2A Dispute Resolution Service
 *
 * Manages dispute workflows for A2A commerce escrows.
 * When an escrow is disputed, this module handles filing, evidence collection,
 * review, and resolution.
 *
 * @example
 * ```javascript
 * const disputes = createDisputeService(store);
 *
 * // File a dispute
 * const result = await disputes.fileDispute({
 *   escrowId: 'escrow-123',
 *   filedBy: '0xBuyer',
 *   filedAgainst: '0xSeller',
 *   reason: 'Product never delivered',
 *   category: 'non_delivery',
 * });
 *
 * // Submit evidence
 * await disputes.submitEvidence(result.dispute.id, {
 *   submittedBy: '0xBuyer',
 *   evidenceType: 'screenshot',
 *   title: 'Order confirmation showing expected delivery date',
 *   description: 'Screenshot of the delivery promise',
 *   content: 'base64-encoded-image-data',
 * });
 *
 * // Resolve the dispute
 * await disputes.resolveDispute(result.dispute.id, {
 *   resolutionType: 'full_refund',
 *   resolvedBy: 'arbitrator-agent',
 * });
 * ```
 */

import { randomUUID, createHash } from 'node:crypto';

// Dispute categories
const DISPUTE_CATEGORIES = [
  'non_delivery',
  'poor_quality',
  'not_as_described',
  'overcharged',
  'unauthorized',
  'other',
];

// Evidence types
const EVIDENCE_TYPES = [
  'screenshot',
  'transaction_log',
  'communication',
  'delivery_proof',
  'other',
];

// Resolution types
const RESOLUTION_TYPES = [
  'full_refund',
  'partial_refund',
  'release_to_seller',
  'split',
  'escalated',
];

/**
 * Valid dispute status transitions
 *
 * filed -> evidence_period -> under_review -> resolved
 *                                          -> escalated
 */
const VALID_TRANSITIONS = {
  filed: ['evidence_period'],
  evidence_period: ['under_review'],
  under_review: ['resolved', 'escalated'],
};

// Statuses that allow evidence submission
const EVIDENCE_ALLOWED_STATUSES = ['filed', 'evidence_period'];

// Statuses that allow resolution
const RESOLUTION_ALLOWED_STATUSES = ['filed', 'evidence_period', 'under_review'];

// Deadline constants (ms)
const EVIDENCE_DEADLINE_MS = 72 * 60 * 60 * 1000; // 72 hours
const REVIEW_DEADLINE_MS = 7 * 24 * 60 * 60 * 1000; // 7 days

/**
 * Create a dispute resolution service
 *
 * @param {Object} store - Store with dispute/evidence/escrow CRUD methods
 * @param {Function} store.createDispute - Create a dispute record
 * @param {Function} store.getDispute - Get dispute by ID
 * @param {Function} store.updateDispute - Update dispute fields
 * @param {Function} store.listDisputes - List disputes with filters
 * @param {Function} store.createEvidence - Create an evidence record
 * @param {Function} store.listEvidenceByDispute - List evidence for a dispute
 * @param {Function} store.getEscrow - Get escrow by ID
 * @returns {Object} Dispute service methods
 */
export function createDisputeService(store) {
  /**
   * File a new dispute against an escrow
   *
   * @param {Object} params - Dispute parameters
   * @param {string} params.escrowId - Escrow ID being disputed
   * @param {string} params.filedBy - Address of the party filing
   * @param {string} params.filedAgainst - Address of the party being filed against
   * @param {string} params.reason - Human-readable reason for the dispute
   * @param {string} params.category - Dispute category
   * @param {Array} [params.evidence] - Initial evidence items
   * @returns {Promise<Object>} Filed dispute
   */
  async function fileDispute(params) {
    const { escrowId, filedBy, filedAgainst, reason, category, evidence } = params;

    if (!escrowId) {
      throw new Error('escrowId is required');
    }
    if (!filedBy) {
      throw new Error('filedBy is required');
    }
    if (!filedAgainst) {
      throw new Error('filedAgainst is required');
    }
    if (!reason) {
      throw new Error('reason is required');
    }
    if (!category || !DISPUTE_CATEGORIES.includes(category)) {
      throw new Error(`category must be one of: ${DISPUTE_CATEGORIES.join(', ')}`);
    }

    // Get escrow to populate amount/asset fields
    const escrow = await store.getEscrow(escrowId);
    if (!escrow) {
      throw new Error(`Escrow not found: ${escrowId}`);
    }

    const now = new Date();
    const evidenceDeadline = new Date(now.getTime() + EVIDENCE_DEADLINE_MS);
    const reviewDeadline = new Date(now.getTime() + REVIEW_DEADLINE_MS);
    const disputeId = randomUUID();

    const dispute = {
      id: disputeId,
      escrow_id: escrowId,
      status: 'filed',
      filed_by: filedBy,
      filed_against: filedAgainst,
      reason,
      category,
      amount: escrow.amount,
      amount_decimal: escrow.amount_decimal,
      asset: escrow.asset,
      evidence_deadline: evidenceDeadline.toISOString(),
      review_deadline: reviewDeadline.toISOString(),
      resolution_type: null,
      resolution_amount: null,
      resolution_note: null,
      resolved_by: null,
      resolved_at: null,
      created_at: now.toISOString(),
      updated_at: now.toISOString(),
    };

    await store.createDispute(dispute);

    // If initial evidence is provided, create evidence records
    if (evidence && Array.isArray(evidence) && evidence.length > 0) {
      for (const item of evidence) {
        await _createEvidenceRecord(disputeId, {
          submittedBy: filedBy,
          evidenceType: item.evidenceType || 'other',
          title: item.title || 'Initial evidence',
          description: item.description || null,
          content: item.content || '',
        });
      }
    }

    const created = await store.getDispute(disputeId);

    return {
      success: true,
      dispute: formatDispute(created),
    };
  }

  /**
   * Submit evidence for a dispute
   *
   * @param {string} disputeId - Dispute ID
   * @param {Object} params - Evidence parameters
   * @param {string} params.submittedBy - Address of the submitter
   * @param {string} params.evidenceType - Type of evidence
   * @param {string} params.title - Evidence title
   * @param {string} [params.description] - Evidence description
   * @param {string} params.content - Evidence content (hashed for integrity)
   * @returns {Promise<Object>} Created evidence
   */
  async function submitEvidence(disputeId, params) {
    const { submittedBy, evidenceType, title, description, content } = params;

    if (!disputeId) {
      throw new Error('disputeId is required');
    }
    if (!submittedBy) {
      throw new Error('submittedBy is required');
    }
    if (!evidenceType || !EVIDENCE_TYPES.includes(evidenceType)) {
      throw new Error(`evidenceType must be one of: ${EVIDENCE_TYPES.join(', ')}`);
    }
    if (!title) {
      throw new Error('title is required');
    }
    if (!content) {
      throw new Error('content is required');
    }

    const dispute = await store.getDispute(disputeId);
    if (!dispute) {
      throw new Error(`Dispute not found: ${disputeId}`);
    }

    if (!EVIDENCE_ALLOWED_STATUSES.includes(dispute.status)) {
      throw new Error(`Cannot submit evidence when dispute status is: ${dispute.status}`);
    }

    const evidenceRecord = await _createEvidenceRecord(disputeId, {
      submittedBy,
      evidenceType,
      title,
      description,
      content,
    });

    return {
      success: true,
      evidence: formatEvidence(evidenceRecord),
    };
  }

  /**
   * Internal helper to create an evidence record with content hash
   *
   * @param {string} disputeId - Dispute ID
   * @param {Object} params - Evidence fields
   * @returns {Promise<Object>} Created evidence record
   */
  async function _createEvidenceRecord(disputeId, params) {
    const { submittedBy, evidenceType, title, description, content } = params;

    const contentHash = createHash('sha256')
      .update(content || '')
      .digest('hex');

    const now = new Date().toISOString();
    const evidenceRecord = {
      id: randomUUID(),
      dispute_id: disputeId,
      submitted_by: submittedBy,
      evidence_type: evidenceType,
      title,
      description: description || null,
      content,
      content_hash: contentHash,
      created_at: now,
    };

    await store.createEvidence(evidenceRecord);
    return evidenceRecord;
  }

  /**
   * Resolve a dispute
   *
   * @param {string} disputeId - Dispute ID
   * @param {Object} params - Resolution parameters
   * @param {string} params.resolutionType - How to resolve
   * @param {number} [params.amount] - Amount for partial_refund or split (buyer's share)
   * @param {string} [params.note] - Resolution note
   * @param {string} params.resolvedBy - Address or ID of the resolver
   * @returns {Promise<Object>} Resolution result with escrow action
   */
  async function resolveDispute(disputeId, params) {
    const { resolutionType, amount, note, resolvedBy } = params;

    if (!disputeId) {
      throw new Error('disputeId is required');
    }
    if (!resolutionType || !RESOLUTION_TYPES.includes(resolutionType)) {
      throw new Error(`resolutionType must be one of: ${RESOLUTION_TYPES.join(', ')}`);
    }
    if (!resolvedBy) {
      throw new Error('resolvedBy is required');
    }

    const dispute = await store.getDispute(disputeId);
    if (!dispute) {
      throw new Error(`Dispute not found: ${disputeId}`);
    }

    if (!RESOLUTION_ALLOWED_STATUSES.includes(dispute.status)) {
      throw new Error(`Cannot resolve dispute in status: ${dispute.status}`);
    }

    // Validate amount for resolution types that require it
    if (resolutionType === 'partial_refund' && (amount === undefined || amount === null)) {
      throw new Error('amount is required for partial_refund resolution');
    }
    if (resolutionType === 'split' && (amount === undefined || amount === null)) {
      throw new Error('amount is required for split resolution (buyer share)');
    }

    const now = new Date().toISOString();

    await store.updateDispute(disputeId, {
      status: 'resolved',
      resolution_type: resolutionType,
      resolution_amount: amount !== undefined ? amount : null,
      resolution_note: note || null,
      resolved_by: resolvedBy,
      resolved_at: now,
      updated_at: now,
    });

    const updated = await store.getDispute(disputeId);

    // Determine escrow action based on resolution type
    const escrowAction = _buildEscrowAction(resolutionType, amount, dispute);

    return {
      success: true,
      dispute: formatDispute(updated),
      escrowAction,
    };
  }

  /**
   * Build escrow action instructions based on resolution type
   *
   * @param {string} resolutionType - Resolution type
   * @param {number} [amount] - Amount for partial/split
   * @param {Object} dispute - Original dispute record
   * @returns {Object} Escrow action descriptor
   */
  function _buildEscrowAction(resolutionType, amount, dispute) {
    switch (resolutionType) {
      case 'full_refund':
        return {
          action: 'refund',
          escrowId: dispute.escrow_id,
          amount: dispute.amount_decimal,
          asset: dispute.asset,
          to: dispute.filed_by,
        };

      case 'partial_refund':
        return {
          action: 'partial_refund',
          escrowId: dispute.escrow_id,
          refundAmount: amount,
          releaseAmount: dispute.amount_decimal - amount,
          asset: dispute.asset,
          refundTo: dispute.filed_by,
          releaseTo: dispute.filed_against,
        };

      case 'release_to_seller':
        return {
          action: 'release',
          escrowId: dispute.escrow_id,
          amount: dispute.amount_decimal,
          asset: dispute.asset,
          to: dispute.filed_against,
        };

      case 'split':
        return {
          action: 'split',
          escrowId: dispute.escrow_id,
          buyerAmount: amount,
          sellerAmount: dispute.amount_decimal - amount,
          asset: dispute.asset,
          buyer: dispute.filed_by,
          seller: dispute.filed_against,
        };

      case 'escalated':
        return {
          action: 'hold',
          escrowId: dispute.escrow_id,
          note: 'Funds held pending escalation review',
        };

      default:
        return {
          action: 'unknown',
          escrowId: dispute.escrow_id,
        };
    }
  }

  /**
   * Escalate a dispute for higher-level review
   *
   * @param {string} disputeId - Dispute ID
   * @returns {Promise<Object>} Escalation result
   */
  async function escalateDispute(disputeId) {
    if (!disputeId) {
      throw new Error('disputeId is required');
    }

    const dispute = await store.getDispute(disputeId);
    if (!dispute) {
      throw new Error(`Dispute not found: ${disputeId}`);
    }

    // Escalation is allowed from under_review status
    if (dispute.status !== 'under_review') {
      throw new Error(
        `Cannot escalate dispute in status: ${dispute.status}. Must be under_review.`,
      );
    }

    const now = new Date().toISOString();

    await store.updateDispute(disputeId, {
      status: 'escalated',
      updated_at: now,
    });

    const updated = await store.getDispute(disputeId);

    return {
      success: true,
      dispute: formatDispute(updated),
      escalated: true,
    };
  }

  /**
   * Transition dispute from filed to evidence_period
   *
   * @param {string} disputeId - Dispute ID
   * @returns {Promise<Object>} Updated dispute
   */
  async function moveToEvidencePeriod(disputeId) {
    return _transitionStatus(disputeId, 'filed', 'evidence_period');
  }

  /**
   * Transition dispute from evidence_period to under_review
   *
   * @param {string} disputeId - Dispute ID
   * @returns {Promise<Object>} Updated dispute
   */
  async function moveToReview(disputeId) {
    return _transitionStatus(disputeId, 'evidence_period', 'under_review');
  }

  /**
   * Internal status transition helper with validation
   *
   * @param {string} disputeId - Dispute ID
   * @param {string} expectedStatus - Expected current status
   * @param {string} newStatus - Target status
   * @returns {Promise<Object>} Updated dispute
   */
  async function _transitionStatus(disputeId, expectedStatus, newStatus) {
    if (!disputeId) {
      throw new Error('disputeId is required');
    }

    const dispute = await store.getDispute(disputeId);
    if (!dispute) {
      throw new Error(`Dispute not found: ${disputeId}`);
    }

    if (dispute.status !== expectedStatus) {
      throw new Error(
        `Cannot transition to ${newStatus}: dispute is in status ${dispute.status}, expected ${expectedStatus}`,
      );
    }

    const validTargets = VALID_TRANSITIONS[expectedStatus] || [];
    if (!validTargets.includes(newStatus)) {
      throw new Error(`Invalid transition: ${expectedStatus} -> ${newStatus}`);
    }

    const now = new Date().toISOString();

    await store.updateDispute(disputeId, {
      status: newStatus,
      updated_at: now,
    });

    const updated = await store.getDispute(disputeId);

    return {
      success: true,
      dispute: formatDispute(updated),
    };
  }

  /**
   * Get a dispute by ID with evidence count
   *
   * @param {string} disputeId - Dispute ID
   * @returns {Promise<Object>} Formatted dispute with evidence count
   */
  async function getDispute(disputeId) {
    if (!disputeId) {
      throw new Error('disputeId is required');
    }

    const dispute = await store.getDispute(disputeId);
    if (!dispute) {
      throw new Error(`Dispute not found: ${disputeId}`);
    }

    const evidence = await store.listEvidenceByDispute(disputeId);
    const formatted = formatDispute(dispute);
    formatted.evidenceCount = evidence ? evidence.length : 0;

    return {
      success: true,
      dispute: formatted,
    };
  }

  /**
   * List disputes with optional filters
   *
   * @param {Object} [filter] - Filter options
   * @param {string} [filter.escrow_id] - Filter by escrow ID
   * @param {string} [filter.status] - Filter by status
   * @param {string} [filter.filed_by] - Filter by filing party
   * @param {string} [filter.filed_against] - Filter by party filed against
   * @returns {Promise<Array>} Formatted disputes
   */
  async function listDisputes(filter = {}) {
    const disputes = await store.listDisputes(filter);
    return disputes.map(formatDispute);
  }

  /**
   * Get all evidence for a dispute
   *
   * @param {string} disputeId - Dispute ID
   * @returns {Promise<Array>} Formatted evidence records
   */
  async function getDisputeEvidence(disputeId) {
    if (!disputeId) {
      throw new Error('disputeId is required');
    }

    const dispute = await store.getDispute(disputeId);
    if (!dispute) {
      throw new Error(`Dispute not found: ${disputeId}`);
    }

    const evidence = await store.listEvidenceByDispute(disputeId);
    return evidence.map(formatEvidence);
  }

  /**
   * Format a dispute record for API output
   *
   * @param {Object} d - Raw dispute record
   * @returns {Object} Formatted dispute
   */
  function formatDispute(d) {
    return {
      id: d.id,
      escrowId: d.escrow_id,
      status: d.status,
      filedBy: d.filed_by,
      filedAgainst: d.filed_against,
      reason: d.reason,
      category: d.category,
      amount: d.amount_decimal,
      asset: d.asset,
      evidenceDeadline: d.evidence_deadline,
      reviewDeadline: d.review_deadline,
      resolutionType: d.resolution_type,
      resolutionAmount: d.resolution_amount,
      resolutionNote: d.resolution_note,
      resolvedBy: d.resolved_by,
      resolvedAt: d.resolved_at,
      createdAt: d.created_at,
      updatedAt: d.updated_at,
    };
  }

  /**
   * Format an evidence record for API output
   *
   * @param {Object} e - Raw evidence record
   * @returns {Object} Formatted evidence
   */
  function formatEvidence(e) {
    return {
      id: e.id,
      disputeId: e.dispute_id,
      submittedBy: e.submitted_by,
      evidenceType: e.evidence_type,
      title: e.title,
      description: e.description,
      contentHash: e.content_hash,
      createdAt: e.created_at,
    };
  }

  return {
    // Core dispute operations
    fileDispute,
    submitEvidence,
    resolveDispute,
    escalateDispute,

    // Status transitions
    moveToEvidencePeriod,
    moveToReview,

    // Query operations
    getDispute,
    listDisputes,
    getDisputeEvidence,

    // Format helpers (exposed for testing/reuse)
    formatDispute,
    formatEvidence,
  };
}

export default { createDisputeService };
