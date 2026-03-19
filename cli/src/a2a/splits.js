/**
 * A2A Split Payment Service
 *
 * Manages multi-party payment splitting for agent-to-agent commerce.
 * Supports percentage-based and fixed-amount splits with optional platform fees.
 *
 * Split Types:
 *   - percentage: Recipients receive a percentage of the total (after platform fee)
 *   - fixed: Recipients receive fixed amounts (must sum to total minus platform fee)
 *
 * State Machine:
 *   pending -> processing -> completed | partial | failed
 *
 * @example
 * ```javascript
 * const splits = createSplitPaymentService(store);
 *
 * // Create a 3-way percentage split with 2.5% platform fee
 * const result = await splits.createSplitPayment({
 *   senderAddress: '0xSender',
 *   totalAmount: 100,
 *   asset: 'USDC',
 *   network: 'set_chain',
 *   splitType: 'percentage',
 *   platformFeePercent: 2.5,
 *   platformFeeAddress: '0xPlatform',
 *   recipients: [
 *     { address: '0xAlice', percent: 50 },
 *     { address: '0xBob', percent: 30 },
 *     { address: '0xCharlie', percent: 20 },
 *   ],
 * });
 *
 * // Execute the split (sends actual payments)
 * await splits.executeSplitPayment(result.splitPayment.id, async (to, amount, asset, network, memo) => {
 *   return paymentService.send(to, amount, asset, network, memo);
 * });
 * ```
 */

import { randomUUID } from 'node:crypto';
import {
  DEFAULT_NETWORK,
  fromSmallestUnit,
  getAssetDecimals,
  getDefaultAssetForNetwork,
  toSmallestUnit,
} from './assets.js';

// Default configuration
const VALID_SPLIT_TYPES = ['percentage', 'fixed'];

/**
 * Format a split recipient record from snake_case to camelCase
 *
 * @param {Object} row - Raw recipient record from the store
 * @returns {Object} Formatted recipient with camelCase keys
 */
function formatRecipient(row) {
  if (!row) return null;

  return {
    id: row.id,
    splitPaymentId: row.split_payment_id,
    recipientAddress: row.recipient_address,
    sharePercent: row.share_percent,
    shareAmount: row.share_amount,
    shareAmountDecimal: row.share_amount_decimal,
    paymentId: row.payment_id,
    status: row.status,
    createdAt: row.created_at,
    updatedAt: row.updated_at,
  };
}

/**
 * Format a split payment record from snake_case to camelCase
 *
 * @param {Object} row - Raw split payment record from the store (with recipients array)
 * @returns {Object} Formatted split payment with camelCase keys
 */
function formatSplitPayment(row) {
  if (!row) return null;

  return {
    id: row.id,
    status: row.status,
    senderAddress: row.sender_address,
    totalAmount: row.total_amount,
    totalAmountDecimal: row.total_amount_decimal,
    asset: row.asset,
    network: row.network,
    splitType: row.split_type,
    platformFeePercent: row.platform_fee_percent,
    platformFeeAmount: row.platform_fee_amount,
    platformFeeAddress: row.platform_fee_address,
    memo: row.memo,
    referenceType: row.reference_type,
    referenceId: row.reference_id,
    metadata: row.metadata,
    completedAt: row.completed_at,
    createdAt: row.created_at,
    updatedAt: row.updated_at,
    recipients: Array.isArray(row.recipients) ? row.recipients.map(formatRecipient) : [],
  };
}

/**
 * Create an A2A Split Payment Service instance
 *
 * @param {Object} store - A2A store with split payment and recipient methods
 * @param {Function} store.createSplitPayment - Persist a new split payment record
 * @param {Function} store.getSplitPayment - Retrieve split payment by ID (includes recipients)
 * @param {Function} store.updateSplitPayment - Update split payment fields by ID
 * @param {Function} store.listSplitPayments - List split payments with optional filter
 * @param {Function} store.createSplitRecipient - Persist a new split recipient record
 * @param {Function} store.getSplitRecipient - Retrieve split recipient by ID
 * @param {Function} store.updateSplitRecipient - Update split recipient fields by ID
 * @param {Function} store.listSplitRecipients - List recipients for a split payment
 * @returns {Object} Split payment service API
 */
export function createSplitPaymentService(store) {
  /**
   * Create a new multi-party split payment
   *
   * @param {Object} params - Split payment parameters
   * @param {string} params.senderAddress - Sender wallet address
   * @param {number} params.totalAmount - Total amount in decimal (e.g. 100.50)
   * @param {string} [params.asset] - Asset type (default: USDC)
   * @param {string} [params.network] - Settlement network (default: set_chain)
   * @param {string} [params.splitType] - 'percentage' or 'fixed' (default: percentage)
   * @param {Array<Object>} params.recipients - Recipient list (min 2)
   * @param {string} params.recipients[].address - Recipient wallet address
   * @param {number} [params.recipients[].percent] - Share percentage (for percentage splits)
   * @param {number} [params.recipients[].amount] - Share amount (for fixed splits)
   * @param {number} [params.platformFeePercent] - Platform fee percentage (0-100)
   * @param {string} [params.platformFeeAddress] - Platform fee recipient address
   * @param {string} [params.memo] - Payment memo
   * @param {string} [params.referenceType] - Reference entity type (e.g. 'order')
   * @param {string} [params.referenceId] - Reference entity ID
   * @param {Object} [params.metadata] - Additional metadata
   * @returns {Promise<Object>} Created split payment result
   */
  async function createSplitPayment(params) {
    const {
      senderAddress,
      totalAmount,
      network = DEFAULT_NETWORK,
      asset: requestedAsset = null,
      splitType = 'percentage',
      recipients,
      platformFeePercent = 0,
      platformFeeAddress,
      memo,
      referenceType,
      referenceId,
      metadata,
    } = params;

    const asset = requestedAsset || getDefaultAssetForNetwork(network);
    const assetDecimals = getAssetDecimals(asset);

    // --- Validate required fields ---
    if (!senderAddress) {
      throw new Error('senderAddress is required');
    }
    if (totalAmount === undefined || totalAmount === null) {
      throw new Error('totalAmount is required');
    }
    if (totalAmount <= 0) {
      throw new Error('totalAmount must be greater than 0');
    }
    if (!Array.isArray(recipients) || recipients.length < 2) {
      throw new Error('recipients must be an array with at least 2 entries');
    }
    if (!VALID_SPLIT_TYPES.includes(splitType)) {
      throw new Error(`splitType must be one of: ${VALID_SPLIT_TYPES.join(', ')}`);
    }

    // --- Validate each recipient ---
    for (let i = 0; i < recipients.length; i++) {
      const r = recipients[i];
      if (!r.address) {
        throw new Error(`recipients[${i}].address is required`);
      }
      if (splitType === 'percentage' && (r.percent === undefined || r.percent === null)) {
        throw new Error(`recipients[${i}].percent is required for percentage splits`);
      }
      if (splitType === 'fixed' && (r.amount === undefined || r.amount === null)) {
        throw new Error(`recipients[${i}].amount is required for fixed splits`);
      }
    }

    // --- Convert total to smallest unit ---
    const totalAmountSmallest = toSmallestUnit(totalAmount, assetDecimals);

    // --- Platform fee calculation ---
    let platformFeeAmountSmallest = 0;
    if (platformFeePercent > 0 && platformFeeAddress) {
      platformFeeAmountSmallest = Math.round((totalAmountSmallest * platformFeePercent) / 100);
    }
    const remainingSmallest = totalAmountSmallest - platformFeeAmountSmallest;

    // --- Compute recipient shares ---
    /** @type {Array<{ address: string, percent: number|null, amountSmallest: number }>} */
    const computedRecipients = [];

    if (splitType === 'percentage') {
      // Validate percentages sum to 100
      const percentSum = recipients.reduce((sum, r) => sum + r.percent, 0);
      if (Math.abs(percentSum - 100) > 0.001) {
        throw new Error(`Recipient percentages must sum to 100, got ${percentSum}`);
      }

      // Compute each share from the remaining amount (after platform fee)
      let allocated = 0;
      for (let i = 0; i < recipients.length; i++) {
        const r = recipients[i];
        let shareSmallest;

        if (i === recipients.length - 1) {
          // Last recipient gets the remainder to avoid rounding drift
          shareSmallest = remainingSmallest - allocated;
        } else {
          shareSmallest = Math.round((remainingSmallest * r.percent) / 100);
          allocated += shareSmallest;
        }

        computedRecipients.push({
          address: r.address,
          percent: r.percent,
          amountSmallest: shareSmallest,
        });
      }
    } else {
      // Fixed splits — validate amounts sum to remaining
      const fixedSumSmallest = recipients.reduce(
        (sum, r) => sum + toSmallestUnit(r.amount, assetDecimals),
        0,
      );

      if (Math.abs(fixedSumSmallest - remainingSmallest) > 1) {
        const expectedDecimal = fromSmallestUnit(remainingSmallest, assetDecimals);
        const actualDecimal = fromSmallestUnit(fixedSumSmallest, assetDecimals);
        throw new Error(
          `Fixed recipient amounts must sum to ${expectedDecimal} ` +
            `(total minus platform fee), got ${actualDecimal}`,
        );
      }

      for (const r of recipients) {
        computedRecipients.push({
          address: r.address,
          percent: null,
          amountSmallest: toSmallestUnit(r.amount, assetDecimals),
        });
      }
    }

    // --- Persist parent split payment record ---
    const splitId = randomUUID();
    const now = new Date().toISOString();

    await store.createSplitPayment({
      id: splitId,
      status: 'pending',
      sender_address: senderAddress,
      total_amount: totalAmountSmallest,
      total_amount_decimal: totalAmount,
      asset: asset.toUpperCase(),
      network,
      split_type: splitType,
      platform_fee_percent: platformFeePercent || null,
      platform_fee_amount: platformFeeAmountSmallest || null,
      platform_fee_address: platformFeeAddress || null,
      memo: memo || null,
      reference_type: referenceType || null,
      reference_id: referenceId || null,
      metadata: metadata ? JSON.stringify(metadata) : null,
      created_at: now,
      updated_at: now,
    });

    // --- Persist recipient records ---
    for (const cr of computedRecipients) {
      await store.createSplitRecipient({
        id: randomUUID(),
        split_payment_id: splitId,
        recipient_address: cr.address,
        share_percent: cr.percent,
        share_amount: cr.amountSmallest,
        share_amount_decimal: fromSmallestUnit(cr.amountSmallest, assetDecimals),
        status: 'pending',
      });
    }

    // --- Platform fee recipient ---
    if (platformFeeAmountSmallest > 0 && platformFeeAddress) {
      await store.createSplitRecipient({
        id: randomUUID(),
        split_payment_id: splitId,
        recipient_address: platformFeeAddress,
        share_percent: platformFeePercent,
        share_amount: platformFeeAmountSmallest,
        share_amount_decimal: fromSmallestUnit(platformFeeAmountSmallest, assetDecimals),
        status: 'pending',
      });
    }

    // --- Return formatted result ---
    const stored = await store.getSplitPayment(splitId);

    return {
      success: true,
      splitPayment: formatSplitPayment(stored),
    };
  }

  /**
   * Execute actual payments for a split payment
   *
   * Iterates over each recipient and calls the provided payFn to send funds.
   * Updates each recipient's status and the parent's overall status based on results.
   *
   * @param {string} splitPaymentId - Split payment ID
   * @param {Function} payFn - Async payment function: (to, amount, asset, network, memo) => paymentResult
   * @returns {Promise<Object>} Updated split payment with execution results
   */
  async function executeSplitPayment(splitPaymentId, payFn) {
    const split = await store.getSplitPayment(splitPaymentId);
    if (!split) {
      throw new Error('Split payment not found');
    }

    if (split.status !== 'pending') {
      throw new Error(`Cannot execute split payment in status: ${split.status}. Expected: pending`);
    }

    // Move to processing
    await store.updateSplitPayment(splitPaymentId, {
      status: 'processing',
    });

    let completedCount = 0;
    let failedCount = 0;

    // Process each recipient
    for (const recipient of split.recipients) {
      try {
        const result = await payFn(
          recipient.recipient_address,
          recipient.share_amount_decimal,
          split.asset,
          split.network,
          split.memo,
        );

        await store.updateSplitRecipient(recipient.id, {
          status: 'completed',
          payment_id: result?.id || result?.paymentId || null,
        });

        completedCount++;
      } catch (err) {
        console.warn(
          `Split payment ${splitPaymentId}: failed to pay recipient ${recipient.recipient_address}:`,
          err.message || err,
        );

        await store.updateSplitRecipient(recipient.id, {
          status: 'failed',
        });

        failedCount++;
      }
    }

    // Determine final parent status
    let finalStatus;
    if (failedCount === 0) {
      finalStatus = 'completed';
    } else if (completedCount === 0) {
      finalStatus = 'failed';
    } else {
      finalStatus = 'partial';
    }

    const updateFields = { status: finalStatus };
    if (finalStatus === 'completed') {
      updateFields.completed_at = new Date().toISOString();
    }

    await store.updateSplitPayment(splitPaymentId, updateFields);

    const updated = await store.getSplitPayment(splitPaymentId);
    return {
      success: finalStatus === 'completed',
      splitPayment: formatSplitPayment(updated),
    };
  }

  /**
   * Get a single split payment by ID
   *
   * @param {string} splitPaymentId - Split payment ID
   * @returns {Promise<Object|null>} Formatted split payment or null
   */
  async function getSplitPayment(splitPaymentId) {
    const split = await store.getSplitPayment(splitPaymentId);
    if (!split) {
      return null;
    }
    return formatSplitPayment(split);
  }

  /**
   * List split payments with optional filtering
   *
   * @param {Object} [filter] - Filter options
   * @param {string} [filter.senderAddress] - Filter by sender
   * @param {string} [filter.status] - Filter by status
   * @param {number} [filter.limit] - Max results
   * @param {number} [filter.offset] - Pagination offset
   * @returns {Promise<Array>} Formatted split payment list
   */
  async function listSplitPayments(filter = {}) {
    // Convert camelCase filter keys to snake_case for the store
    const storeFilter = {};
    if (filter.senderAddress) {
      storeFilter.sender_address = filter.senderAddress;
    }
    if (filter.status) {
      storeFilter.status = filter.status;
    }
    if (filter.limit) {
      storeFilter.limit = filter.limit;
    }
    if (filter.offset) {
      storeFilter.offset = filter.offset;
    }

    const splits = await store.listSplitPayments(storeFilter);
    return splits.map(formatSplitPayment);
  }

  return {
    createSplitPayment,
    executeSplitPayment,
    getSplitPayment,
    listSplitPayments,
  };
}

export default { createSplitPaymentService };
