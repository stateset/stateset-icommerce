/**
 * A2A Escrow Service
 *
 * Manages conditional fund holding between buyer and seller agents.
 * Supports multi-condition release, time-based expiry, and dispute escalation.
 *
 * State Machine:
 *   created -> funded -> active -> released (to seller)
 *                              -> refunded (to buyer)
 *                              -> disputed -> (resolved via dispute system)
 *                        -> expired (auto-refund)
 *
 * @example
 * ```javascript
 * const escrow = createEscrowService(store);
 *
 * // Create escrow for a quote
 * const result = await escrow.createEscrow({
 *   buyerAddress: '0xBuyer',
 *   sellerAddress: '0xSeller',
 *   amount: 100,
 *   conditions: [
 *     { type: 'seller_fulfilled', quoteId: 'quote-123' },
 *     { type: 'buyer_confirmed' },
 *   ],
 * });
 *
 * // Fund the escrow
 * await escrow.fundEscrow(result.escrow.id);
 *
 * // Confirm a condition
 * await escrow.confirmCondition(result.escrow.id, 1);
 *
 * // Release when all conditions met
 * await escrow.releaseEscrow(result.escrow.id);
 * ```
 */

import { randomUUID } from 'node:crypto';

// Valid escrow statuses
const ESCROW_STATUSES = [
  'created',
  'funded',
  'active',
  'released',
  'refunded',
  'disputed',
  'expired',
];

// Default configuration
const DEFAULT_EXPIRES_HOURS = 72;
const DEFAULT_ASSET = 'USDC';
const DEFAULT_NETWORK = 'set_chain';

/**
 * Valid transitions in the escrow state machine
 * @type {Record<string, string[]>}
 */
const VALID_TRANSITIONS = {
  created: ['funded', 'refunded'],
  funded: ['active', 'released', 'refunded', 'disputed', 'expired'],
  active: ['released', 'refunded', 'disputed', 'expired'],
  released: [],
  refunded: [],
  disputed: [],
  expired: [],
};

/**
 * Format an escrow record for external consumption
 *
 * @param {Object} escrow - Raw escrow record from the store
 * @returns {Object} Formatted escrow object with camelCase keys
 */
function formatEscrow(escrow) {
  if (!escrow) return null;

  let releaseConditions = [];
  if (escrow.release_conditions) {
    try {
      releaseConditions =
        typeof escrow.release_conditions === 'string'
          ? JSON.parse(escrow.release_conditions)
          : escrow.release_conditions;
    } catch {
      console.warn('Failed to parse release_conditions for escrow', escrow.id);
      releaseConditions = [];
    }
  }

  return {
    id: escrow.id,
    status: escrow.status,
    quoteId: escrow.quote_id || null,
    buyerAddress: escrow.buyer_address,
    sellerAddress: escrow.seller_address,
    amount: escrow.amount_decimal ?? escrow.amount,
    asset: escrow.asset,
    network: escrow.network,
    releaseConditions,
    fundedAt: escrow.funded_at || null,
    releasedAt: escrow.released_at || null,
    disputedAt: escrow.disputed_at || null,
    expiresAt: escrow.expires_at || null,
    createdAt: escrow.created_at,
    updatedAt: escrow.updated_at,
  };
}

/**
 * Create an A2A Escrow Service instance
 *
 * @param {Object} store - A2A store with escrow and quote methods
 * @param {Function} store.createEscrow - Persist a new escrow record
 * @param {Function} store.getEscrow - Retrieve escrow by ID
 * @param {Function} store.updateEscrow - Update escrow fields by ID
 * @param {Function} store.listEscrows - List escrows with optional filter
 * @param {Function} store.getQuote - Retrieve a quote by ID (for condition checks)
 * @returns {Object} Escrow service API
 */
export function createEscrowService(store) {
  /**
   * Validate that a status transition is allowed
   *
   * @param {string} currentStatus - Current escrow status
   * @param {string} targetStatus - Desired status
   * @throws {Error} If the transition is not allowed
   */
  function validateTransition(currentStatus, targetStatus) {
    const allowed = VALID_TRANSITIONS[currentStatus];
    if (!allowed || !allowed.includes(targetStatus)) {
      throw new Error(
        `Invalid escrow transition: ${currentStatus} -> ${targetStatus}. ` +
          `Allowed: ${(allowed || []).join(', ') || 'none'}`,
      );
    }
  }

  /**
   * Parse release conditions from a stored escrow record
   *
   * @param {Object} escrow - Escrow record
   * @returns {Array} Parsed conditions array
   */
  function parseConditions(escrow) {
    if (!escrow.release_conditions) return [];
    try {
      return typeof escrow.release_conditions === 'string'
        ? JSON.parse(escrow.release_conditions)
        : escrow.release_conditions;
    } catch {
      console.warn('Failed to parse release_conditions for escrow', escrow.id);
      return [];
    }
  }

  /**
   * Create a new escrow between buyer and seller agents
   *
   * @param {Object} params - Escrow parameters
   * @param {string} [params.quoteId] - Associated quote ID
   * @param {string} params.buyerAddress - Buyer wallet address
   * @param {string} params.sellerAddress - Seller wallet address
   * @param {number} params.amount - Amount in smallest unit
   * @param {number} [params.amountDecimal] - Human-readable amount
   * @param {string} [params.asset] - Asset type (default: USDC)
   * @param {string} [params.network] - Settlement network (default: set_chain)
   * @param {Array} [params.conditions] - Release condition objects
   * @param {number} [params.expiresInHours] - Hours until expiry (default: 72)
   * @param {number} [params.autoReleaseAfterHours] - Auto-release delay in hours
   * @param {Object} [params.metadata] - Additional metadata
   * @returns {Promise<Object>} Created escrow result
   */
  async function createEscrow(params) {
    const {
      quoteId,
      buyerAddress,
      sellerAddress,
      amount,
      amountDecimal,
      asset = DEFAULT_ASSET,
      network = DEFAULT_NETWORK,
      conditions = [],
      expiresInHours = DEFAULT_EXPIRES_HOURS,
      autoReleaseAfterHours,
      metadata,
    } = params;

    // Validate required fields
    if (!buyerAddress) {
      throw new Error('buyerAddress is required');
    }
    if (!sellerAddress) {
      throw new Error('sellerAddress is required');
    }
    if (amount === undefined || amount === null) {
      throw new Error('amount is required');
    }
    if (amount <= 0) {
      throw new Error('amount must be positive');
    }

    const now = new Date();
    const expiresAt = new Date(now.getTime() + expiresInHours * 60 * 60 * 1000);

    // Build release conditions with defaults
    const releaseConditions = conditions.map((c) => {
      switch (c.type) {
        case 'seller_fulfilled':
          return { type: 'seller_fulfilled', quoteId: c.quoteId || quoteId || null };
        case 'buyer_confirmed':
          return { type: 'buyer_confirmed', completed: false };
        case 'time_lock':
          return {
            type: 'time_lock',
            releaseAfter: c.releaseAfter || null,
          };
        case 'milestone':
          return {
            type: 'milestone',
            description: c.description || '',
            completed: false,
          };
        default:
          return { ...c, completed: false };
      }
    });

    // If autoReleaseAfterHours is set, add a time_lock condition
    if (autoReleaseAfterHours && !releaseConditions.some((c) => c.type === 'time_lock')) {
      const releaseAfter = new Date(now.getTime() + autoReleaseAfterHours * 60 * 60 * 1000);
      releaseConditions.push({
        type: 'time_lock',
        releaseAfter: releaseAfter.toISOString(),
      });
    }

    const escrowId = randomUUID();
    const escrowRecord = {
      id: escrowId,
      status: 'created',
      quote_id: quoteId || null,
      buyer_address: buyerAddress,
      seller_address: sellerAddress,
      amount,
      amount_decimal: amountDecimal ?? amount,
      asset: asset.toUpperCase(),
      network,
      release_conditions: JSON.stringify(releaseConditions),
      funded_at: null,
      released_at: null,
      disputed_at: null,
      expires_at: expiresAt.toISOString(),
      metadata: metadata ? JSON.stringify(metadata) : null,
      created_at: now.toISOString(),
      updated_at: now.toISOString(),
    };

    await store.createEscrow(escrowRecord);
    const created = await store.getEscrow(escrowId);

    return {
      success: true,
      escrow: formatEscrow(created || escrowRecord),
    };
  }

  /**
   * Fund an escrow (marks it as funded then active)
   *
   * @param {string} escrowId - Escrow ID
   * @returns {Promise<Object>} Updated escrow result
   */
  async function fundEscrow(escrowId) {
    const escrow = await store.getEscrow(escrowId);
    if (!escrow) {
      throw new Error('Escrow not found');
    }

    validateTransition(escrow.status, 'funded');

    const now = new Date().toISOString();

    // Move to funded, then immediately to active
    await store.updateEscrow(escrowId, {
      status: 'active',
      funded_at: now,
    });

    const updated = await store.getEscrow(escrowId);

    return {
      success: true,
      escrow: formatEscrow(updated),
    };
  }

  /**
   * Release escrow funds to the seller
   *
   * All release conditions must be met before funds can be released.
   *
   * @param {string} escrowId - Escrow ID
   * @returns {Promise<Object>} Release result
   */
  async function releaseEscrow(escrowId) {
    const escrow = await store.getEscrow(escrowId);
    if (!escrow) {
      throw new Error('Escrow not found');
    }

    if (escrow.status !== 'active' && escrow.status !== 'funded') {
      throw new Error(`Cannot release escrow in status: ${escrow.status}`);
    }

    // Check conditions
    const conditionResult = await checkConditions(escrowId);

    if (!conditionResult.allMet) {
      const unmet = conditionResult.conditions
        .filter((c) => !c.met)
        .map((c) => `${c.type}${c.description ? ': ' + c.description : ''}`);

      return {
        success: false,
        error: 'Not all release conditions are met',
        unmetConditions: unmet,
        conditions: conditionResult.conditions,
      };
    }

    validateTransition(escrow.status, 'released');

    await store.updateEscrow(escrowId, {
      status: 'released',
      released_at: new Date().toISOString(),
    });

    const updated = await store.getEscrow(escrowId);

    return {
      success: true,
      escrow: formatEscrow(updated),
    };
  }

  /**
   * Refund escrow funds to the buyer
   *
   * @param {string} escrowId - Escrow ID
   * @returns {Promise<Object>} Refund result
   */
  async function refundEscrow(escrowId) {
    const escrow = await store.getEscrow(escrowId);
    if (!escrow) {
      throw new Error('Escrow not found');
    }

    if (escrow.status !== 'active' && escrow.status !== 'funded' && escrow.status !== 'created') {
      throw new Error(`Cannot refund escrow in status: ${escrow.status}`);
    }

    validateTransition(escrow.status, 'refunded');

    await store.updateEscrow(escrowId, {
      status: 'refunded',
    });

    const updated = await store.getEscrow(escrowId);

    return {
      success: true,
      escrow: formatEscrow(updated),
    };
  }

  /**
   * Dispute an escrow (escalate to dispute resolution)
   *
   * @param {string} escrowId - Escrow ID
   * @param {Object} params - Dispute details
   * @param {string} params.reason - Reason for the dispute
   * @param {string} [params.category] - Dispute category
   * @returns {Promise<Object>} Dispute result (caller creates the actual dispute record)
   */
  async function disputeEscrow(escrowId, { reason, category } = {}) {
    const escrow = await store.getEscrow(escrowId);
    if (!escrow) {
      throw new Error('Escrow not found');
    }

    if (escrow.status !== 'active' && escrow.status !== 'funded') {
      throw new Error(`Cannot dispute escrow in status: ${escrow.status}`);
    }

    validateTransition(escrow.status, 'disputed');

    const now = new Date().toISOString();

    // Store dispute metadata on the escrow
    const existingMetadata = escrow.metadata ? JSON.parse(escrow.metadata) : {};
    const updatedMetadata = {
      ...existingMetadata,
      dispute: {
        reason: reason || null,
        category: category || null,
        disputedAt: now,
      },
    };

    await store.updateEscrow(escrowId, {
      status: 'disputed',
      disputed_at: now,
      metadata: JSON.stringify(updatedMetadata),
    });

    const updated = await store.getEscrow(escrowId);

    return {
      success: true,
      escrow: formatEscrow(updated),
      disputeNeeded: true,
    };
  }

  /**
   * Check whether all release conditions are met for an escrow
   *
   * Evaluates each condition:
   *   - seller_fulfilled: linked quote status is 'fulfilled'
   *   - buyer_confirmed: condition has completed === true
   *   - time_lock: current time >= releaseAfter
   *   - milestone: condition has completed === true
   *
   * @param {string} escrowId - Escrow ID
   * @returns {Promise<Object>} Condition evaluation result
   */
  async function checkConditions(escrowId) {
    const escrow = await store.getEscrow(escrowId);
    if (!escrow) {
      throw new Error('Escrow not found');
    }

    const conditions = parseConditions(escrow);

    // No conditions means all are met (unconditional release)
    if (conditions.length === 0) {
      return { allMet: true, conditions: [] };
    }

    const now = new Date();
    const evaluated = [];

    for (const condition of conditions) {
      let met = false;

      switch (condition.type) {
        case 'seller_fulfilled': {
          const quoteId = condition.quoteId || escrow.quote_id;
          if (quoteId && store.getQuote) {
            try {
              const quote = await store.getQuote(quoteId);
              met = quote?.status === 'fulfilled';
            } catch {
              console.warn('Failed to check quote status for condition', quoteId);
              met = false;
            }
          }
          break;
        }

        case 'buyer_confirmed': {
          met = condition.completed === true;
          break;
        }

        case 'time_lock': {
          if (condition.releaseAfter) {
            met = now >= new Date(condition.releaseAfter);
          }
          break;
        }

        case 'milestone': {
          met = condition.completed === true;
          break;
        }

        default: {
          // Unknown condition type: treat as not met unless explicitly completed
          met = condition.completed === true;
          break;
        }
      }

      evaluated.push({ ...condition, met });
    }

    const allMet = evaluated.every((c) => c.met);

    return { allMet, conditions: evaluated };
  }

  /**
   * Confirm (mark as completed) a specific release condition by index
   *
   * If all conditions become met after confirmation, the escrow can optionally
   * be auto-released.
   *
   * @param {string} escrowId - Escrow ID
   * @param {number} conditionIndex - Zero-based index of the condition to confirm
   * @returns {Promise<Object>} Updated escrow and condition status
   */
  async function confirmCondition(escrowId, conditionIndex) {
    const escrow = await store.getEscrow(escrowId);
    if (!escrow) {
      throw new Error('Escrow not found');
    }

    const conditions = parseConditions(escrow);

    if (conditionIndex < 0 || conditionIndex >= conditions.length) {
      throw new Error(
        `Invalid condition index: ${conditionIndex}. Escrow has ${conditions.length} conditions.`,
      );
    }

    // Mark the condition as completed
    conditions[conditionIndex].completed = true;

    await store.updateEscrow(escrowId, {
      release_conditions: JSON.stringify(conditions),
    });

    // Check if all conditions are now met
    const conditionResult = await checkConditions(escrowId);

    // Auto-release if all met and escrow is in active/funded status
    if (conditionResult.allMet && (escrow.status === 'active' || escrow.status === 'funded')) {
      const releaseResult = await releaseEscrow(escrowId);
      return {
        success: true,
        escrow: releaseResult.escrow,
        allConditionsMet: true,
      };
    }

    const updated = await store.getEscrow(escrowId);

    return {
      success: true,
      escrow: formatEscrow(updated),
      allConditionsMet: conditionResult.allMet,
    };
  }

  /**
   * Check if an escrow has expired and auto-refund if so
   *
   * @param {string} escrowId - Escrow ID
   * @returns {Promise<Object>} Expiry check result
   */
  async function checkExpired(escrowId) {
    const escrow = await store.getEscrow(escrowId);
    if (!escrow) {
      throw new Error('Escrow not found');
    }

    // Only active/funded escrows can expire
    if (escrow.status !== 'active' && escrow.status !== 'funded') {
      return {
        expired: false,
        escrow: formatEscrow(escrow),
      };
    }

    const now = new Date();
    const expiresAt = escrow.expires_at ? new Date(escrow.expires_at) : null;

    if (expiresAt && now >= expiresAt) {
      // Auto-refund on expiry
      await store.updateEscrow(escrowId, {
        status: 'expired',
      });

      const updated = await store.getEscrow(escrowId);

      return {
        expired: true,
        escrow: formatEscrow(updated),
      };
    }

    return {
      expired: false,
      escrow: formatEscrow(escrow),
    };
  }

  /**
   * Get a single escrow by ID
   *
   * @param {string} escrowId - Escrow ID
   * @returns {Promise<Object|null>} Formatted escrow or null
   */
  async function getEscrow(escrowId) {
    const escrow = await store.getEscrow(escrowId);
    if (!escrow) {
      return null;
    }
    return formatEscrow(escrow);
  }

  /**
   * List escrows with optional filtering
   *
   * @param {Object} [filter] - Filter options
   * @param {string} [filter.buyer_address] - Filter by buyer
   * @param {string} [filter.seller_address] - Filter by seller
   * @param {string} [filter.status] - Filter by status
   * @param {string} [filter.quote_id] - Filter by quote
   * @param {number} [filter.limit] - Max results
   * @param {number} [filter.offset] - Pagination offset
   * @returns {Promise<Array>} Formatted escrow list
   */
  async function listEscrows(filter = {}) {
    const escrows = await store.listEscrows(filter);
    return escrows.map(formatEscrow);
  }

  return {
    // Core escrow operations
    createEscrow,
    fundEscrow,
    releaseEscrow,
    refundEscrow,
    disputeEscrow,

    // Condition management
    checkConditions,
    confirmCondition,

    // Expiry management
    checkExpired,

    // Query operations
    getEscrow,
    listEscrows,
  };
}

export default { createEscrowService };
