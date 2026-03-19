/**
 * Agent-to-Agent (A2A) Commerce Module
 *
 * High-level API for agent-to-agent payments, quotes, and commerce negotiations.
 * Makes it dead simple for AI agents to pay each other.
 *
 * @example
 * ```javascript
 * const a2a = createA2AService(commerce, agentConfig);
 *
 * // Direct payment
 * await a2a.pay({ to: 'agent-wallet-address', amount: 10.00, memo: 'API call' });
 *
 * // Request payment
 * const request = await a2a.requestPayment({
 *   from: 'buyer-agent-wallet',
 *   amount: 25.00,
 *   description: 'Data processing fee'
 * });
 *
 * // Quote flow
 * const quote = await a2a.requestQuote({
 *   seller: 'vendor-agent',
 *   items: [{ description: 'Widget', quantity: 2 }]
 * });
 * const payment = await a2a.acceptQuote(quote.id);
 * ```
 */

import { randomUUID } from 'node:crypto';
import {
  DEFAULT_ASSET,
  DEFAULT_NETWORK,
  getDefaultAssetForNetwork,
  getAssetDecimals,
  toSmallestUnit,
  fromSmallestUnit,
} from './assets.js';

/**
 * Create an A2A commerce service instance
 *
 * @param {Object} commerce - The Commerce instance from stateset-embedded
 * @param {Object} config - Agent configuration
 * @param {string} config.agentId - This agent's ID
 * @param {string} config.walletAddress - This agent's wallet address
 * @param {Object} config.signingKey - Ed25519 signing key { privateKey, publicKey }
 * @param {Object} [config.sequencerClient] - Sequencer client for settlement
 * @param {string} [config.tenantId] - Tenant ID for sequencer
 * @param {string} [config.storeId] - Store ID for sequencer
 */
export function createA2AService(commerce, config) {
  const {
    agentId,
    walletAddress,
    signingKey,
    sequencerClient,
    tenantId,
    storeId,
    defaultNetwork = DEFAULT_NETWORK,
    defaultAsset: requestedDefaultAsset,
    receiveAddressForNetwork,
  } = config;
  const defaultAsset = requestedDefaultAsset || getDefaultAssetForNetwork(defaultNetwork);

  if (!walletAddress) {
    throw new Error('walletAddress is required for A2A service');
  }

  function normalizeAcceptedNetworks(value) {
    if (Array.isArray(value) && value.length > 0) {
      return value;
    }
    if (typeof value === 'string' && value.length > 0) {
      try {
        const parsed = JSON.parse(value);
        if (Array.isArray(parsed) && parsed.length > 0) {
          return parsed;
        }
      } catch (_err) {
        void _err;
      }
      return [value];
    }
    return [defaultNetwork];
  }

  function parseMetadata(value) {
    if (!value) return null;
    if (typeof value === 'object') return value;
    if (typeof value !== 'string') return null;
    try {
      const parsed = JSON.parse(value);
      return parsed && typeof parsed === 'object' ? parsed : null;
    } catch {
      return null;
    }
  }

  function mergeMetadata(value, additions) {
    const metadata = { ...(parseMetadata(value) || {}) };
    for (const [key, nextValue] of Object.entries(additions || {})) {
      if (nextValue !== undefined && nextValue !== null && nextValue !== '') {
        metadata[key] = nextValue;
      }
    }
    return Object.keys(metadata).length > 0 ? JSON.stringify(metadata) : null;
  }

  function parsePaymentAddresses(value) {
    const parsed = parseMetadata(value);
    if (!parsed || Array.isArray(parsed)) return null;
    return parsed;
  }

  function selectPaymentAddress(agentRef, network = defaultNetwork) {
    const paymentAddresses = parsePaymentAddresses(
      agentRef?.payment_addresses || agentRef?.paymentAddresses,
    );
    if (paymentAddresses?.[network]) {
      return paymentAddresses[network];
    }
    return agentRef?.wallet_address || agentRef?.walletAddress || agentRef?.address || null;
  }

  async function getOwnPaymentAddress(network = defaultNetwork) {
    if (typeof receiveAddressForNetwork === 'function') {
      try {
        const resolved = await receiveAddressForNetwork(network);
        if (resolved) return resolved;
      } catch {
        // Ignore and fall back to the identity wallet.
      }
    }
    return walletAddress;
  }

  function canAccessPayment(payment) {
    if (!payment) return false;
    if (payment.sender_address === walletAddress || payment.recipient_address === walletAddress) {
      return true;
    }
    if (
      agentId &&
      (payment.sender_agent_id === agentId || payment.recipient_agent_id === agentId)
    ) {
      return true;
    }
    return false;
  }

  async function getStoredPaymentForAgent(paymentId) {
    const payment = await commerce.a2a().getPayment(paymentId);
    if (!payment || !canAccessPayment(payment)) {
      throw new Error('Payment not found');
    }
    return payment;
  }

  function getFinalityTracker() {
    return commerce?._finalityTracker || null;
  }

  function getTrackedFinality(intentId) {
    const tracker = getFinalityTracker();
    if (!tracker || !intentId) return null;
    try {
      return tracker.getSettlementStatus(intentId);
    } catch {
      return null;
    }
  }

  function syncTrackedSettlement({
    intentId,
    txHash,
    chainId,
    blockNumber = 0,
    confirmations = 0,
  }) {
    const tracker = getFinalityTracker();
    if (!tracker || !intentId || !txHash || !chainId) {
      return null;
    }

    if (!getTrackedFinality(intentId)) {
      try {
        tracker.trackSettlement(intentId, txHash, chainId, blockNumber || 0);
      } catch {
        // Ignore duplicate/invalid tracking attempts and fall through.
      }
    }

    const safeConfirmations = Math.max(0, Number(confirmations || 0));
    const latestBlock =
      blockNumber && safeConfirmations > 0 ? blockNumber + safeConfirmations - 1 : blockNumber || 0;

    try {
      tracker.updateConfirmations(intentId, safeConfirmations, latestBlock);
    } catch {
      // Ignore tracker update failures and return best-effort status below.
    }

    return getTrackedFinality(intentId);
  }

  function markTrackedSettlementFailed(intentId, reason) {
    const tracker = getFinalityTracker();
    if (!tracker || !intentId) {
      return null;
    }

    if (!getTrackedFinality(intentId)) {
      return null;
    }

    try {
      return tracker.markFailed(intentId, reason);
    } catch {
      return getTrackedFinality(intentId);
    }
  }

  function hydrateTrackedSettlementFromPayment(payment) {
    if (!payment) return null;
    const metadata = parseMetadata(payment.metadata);

    if (!payment.tx_hash || !payment.network || metadata?.simulated === true) {
      return getTrackedFinality(payment.id);
    }

    if (payment.status === 'failed') {
      syncTrackedSettlement({
        intentId: payment.id,
        txHash: payment.tx_hash,
        chainId: payment.network,
        blockNumber: payment.block_number ?? 0,
        confirmations:
          metadata?.confirmations !== undefined && metadata?.confirmations !== null
            ? Number(metadata.confirmations)
            : 0,
      });
      return markTrackedSettlementFailed(
        payment.id,
        metadata?.settlement_error || 'payment_failed',
      );
    }

    return syncTrackedSettlement({
      intentId: payment.id,
      txHash: payment.tx_hash,
      chainId: payment.network,
      blockNumber: payment.block_number ?? 0,
      confirmations:
        metadata?.confirmations !== undefined && metadata?.confirmations !== null
          ? Number(metadata.confirmations)
          : 0,
    });
  }

  /**
   * Resolve an agent reference to a wallet address
   * Accepts: agent ID, wallet address, or agent card object
   */
  async function resolveAgentAddress(agentRef, network = defaultNetwork) {
    if (!agentRef) {
      throw new Error('Agent reference is required');
    }

    if (typeof agentRef === 'string') {
      // Try to look up as agent ID (UUID format)
      if (agentRef.includes('-') && agentRef.length === 36) {
        const agent = await commerce.x402().getAgent(agentRef);
        if (agent) {
          return {
            address: agent.wallet_address,
            paymentAddress: selectPaymentAddress(agent, network),
            agentId: agent.id,
          };
        }
      }

      // Prefer a registered agent card over treating the value as a raw address.
      const agentByWallet = await commerce.x402().getAgentByWallet(agentRef);
      if (agentByWallet) {
        return {
          address: agentByWallet.wallet_address,
          paymentAddress: selectPaymentAddress(agentByWallet, network),
          agentId: agentByWallet.id,
        };
      }

      // Fall back to a raw chain address.
      return { address: agentRef, paymentAddress: agentRef, agentId: null };
    }

    // If it's an agent card object
    if (
      agentRef.wallet_address ||
      agentRef.walletAddress ||
      agentRef.address ||
      agentRef.paymentAddress
    ) {
      return {
        address: agentRef.wallet_address || agentRef.walletAddress || agentRef.address,
        paymentAddress: agentRef.paymentAddress || selectPaymentAddress(agentRef, network),
        agentId: agentRef.id || agentRef.agentId || null,
      };
    }

    throw new Error(`Cannot resolve agent reference: ${JSON.stringify(agentRef)}`);
  }

  /**
   * Pay another agent directly
   *
   * @param {Object} params - Payment parameters
   * @param {string} params.to - Recipient agent ID, wallet address, or agent card
   * @param {number} params.amount - Amount to pay (human-readable, e.g., 10.00)
   * @param {string} [params.asset] - Asset to pay with (default: runtime payment asset)
   * @param {string} [params.network] - Network for settlement
   * @param {string} [params.memo] - Payment memo/description
   * @param {string} [params.referenceType] - Type of reference (quote, order, etc.)
   * @param {string} [params.referenceId] - Reference ID
   * @param {string} [params.idempotencyKey] - Idempotency key for deduplication
   * @returns {Promise<Object>} Payment result
   */
  async function pay(params) {
    const {
      to,
      amount,
      asset = defaultAsset,
      network = defaultNetwork,
      memo,
      referenceType,
      referenceId,
      idempotencyKey,
    } = params;

    if (!to) {
      throw new Error('Recipient (to) is required');
    }
    if (amount === undefined || amount === null) {
      throw new Error('Amount is required');
    }
    if (amount <= 0) {
      throw new Error('Amount must be positive');
    }

    const recipient = await resolveAgentAddress(to, network);
    const decimals = getAssetDecimals(asset);
    const amountSmallest = toSmallestUnit(amount, decimals);
    const now = new Date().toISOString();
    const paymentId = randomUUID();

    // Create payment record
    const payment = {
      id: paymentId,
      status: 'pending',
      sender_agent_id: agentId,
      sender_address: walletAddress,
      recipient_agent_id: recipient.agentId,
      recipient_address: recipient.paymentAddress || recipient.address,
      amount: amountSmallest,
      amount_decimal: amount,
      asset: asset.toUpperCase(),
      network,
      memo: memo || null,
      reference_type: referenceType || null,
      reference_id: referenceId || null,
      idempotency_key: idempotencyKey || `a2a-pay-${paymentId}`,
      intent_id: null,
      tx_hash: null,
      block_number: null,
      metadata: null,
      created_at: now,
      updated_at: now,
      completed_at: null,
    };

    // Store the payment record
    await commerce.a2a().createPayment(payment);

    // If we have a sequencer client, create and submit the x402 payment intent
    if (sequencerClient && signingKey) {
      try {
        // Create x402 payment intent
        const intent = await commerce.x402().createIntent({
          payer_address: walletAddress,
          payee_address: recipient.paymentAddress || recipient.address,
          amount: amountSmallest,
          asset,
          network,
          description: memo,
          idempotency_key: payment.idempotency_key,
        });

        // Sign the intent
        const signedIntent = await commerce.x402().signIntent(intent.id, signingKey);

        // Submit to sequencer
        await sequencerClient.submitPaymentIntent({
          tenant_id: tenantId,
          store_id: storeId,
          agent_id: agentId,
          ...signedIntent,
        });

        // Update payment with intent info
        await commerce.a2a().updatePayment(paymentId, {
          status: 'submitted',
          intent_id: intent.id,
        });

        payment.status = 'submitted';
        payment.intent_id = intent.id;

        // Optionally wait for settlement
        // For now, we return after submission

        return {
          success: true,
          payment: formatPayment(payment),
          intent: {
            id: intent.id,
            signingHash: signedIntent.signing_hash,
          },
        };
      } catch (error) {
        // Update payment as failed
        await commerce.a2a().updatePayment(paymentId, {
          status: 'failed',
          metadata: JSON.stringify({ error: error.message }),
        });

        throw new Error(`Payment failed: ${error.message}`);
      }
    }

    // Without sequencer, just record the intent locally
    // The payment will be settled out-of-band
    return {
      success: true,
      payment: formatPayment(payment),
      note: 'Payment recorded locally. Connect sequencer for on-chain settlement.',
    };
  }

  /**
   * Request payment from another agent
   *
   * @param {Object} params - Request parameters
   * @param {string} [params.from] - Payer agent (optional - open request if not specified)
   * @param {number} params.amount - Amount to request
   * @param {string} params.description - What the payment is for
   * @param {string} [params.asset] - Asset to request
   * @param {string} [params.network] - Preferred settlement network
   * @param {Array} [params.lineItems] - Itemized breakdown
   * @param {number} [params.expiresInHours] - Hours until expiry (default: 24)
   * @param {boolean} [params.allowPartial] - Allow partial payments
   * @param {string} [params.callbackUrl] - Webhook URL for payment notifications
   * @returns {Promise<Object>} Payment request
   */
  async function requestPayment(params) {
    const {
      from,
      amount,
      description,
      asset = defaultAsset,
      network = defaultNetwork,
      lineItems,
      expiresInHours = 24,
      allowPartial = false,
      minimumAmount,
      callbackUrl,
      metadata,
    } = params;

    if (!description) {
      throw new Error('Description is required');
    }
    if (amount === undefined || amount === null) {
      throw new Error('Amount is required');
    }

    let payer = null;
    if (from) {
      payer = await resolveAgentAddress(from, network);
    }

    const requesterPaymentAddress = await getOwnPaymentAddress(network);

    const decimals = getAssetDecimals(asset);
    const amountSmallest = toSmallestUnit(amount, decimals);
    const now = new Date();
    const expiresAt = new Date(now.getTime() + expiresInHours * 60 * 60 * 1000);

    const request = {
      id: randomUUID(),
      status: 'pending',
      requester_agent_id: agentId,
      requester_address: walletAddress,
      payer_agent_id: payer?.agentId || null,
      payer_address: payer?.address || null,
      amount: amountSmallest,
      amount_decimal: amount,
      asset: asset.toUpperCase(),
      accepted_networks: [network],
      description,
      line_items: lineItems ? JSON.stringify(lineItems) : null,
      reference_type: null,
      reference_id: null,
      expires_at: expiresAt.toISOString(),
      allow_partial: allowPartial,
      minimum_amount: minimumAmount ? toSmallestUnit(minimumAmount, decimals) : null,
      amount_paid: 0,
      payment_ids: [],
      callback_url: callbackUrl || null,
      metadata: mergeMetadata(metadata, {
        requester_payment_address: requesterPaymentAddress,
      }),
      created_at: now.toISOString(),
      updated_at: now.toISOString(),
      paid_at: null,
    };

    await commerce.a2a().createPaymentRequest(request);

    return {
      success: true,
      request: formatPaymentRequest(request),
      paymentUrl: `a2a://pay/${request.id}`,
    };
  }

  /**
   * Pay a payment request
   *
   * @param {string} requestId - Payment request ID
   * @param {Object} [options] - Payment options
   * @param {number} [options.amount] - Amount to pay (for partial payments)
   * @returns {Promise<Object>} Payment result
   */
  async function payRequest(requestId, options = {}) {
    const request = await commerce.a2a().getPaymentRequest(requestId);
    if (!request) {
      throw new Error('Payment request not found');
    }

    if (request.status === 'paid') {
      throw new Error('Payment request already paid');
    }
    if (request.status === 'expired' || new Date(request.expires_at) < new Date()) {
      throw new Error('Payment request has expired');
    }
    if (request.status === 'cancelled') {
      throw new Error('Payment request was cancelled');
    }

    const decimals = getAssetDecimals(request.asset);
    const amountToPay = options.amount
      ? toSmallestUnit(options.amount, decimals)
      : request.amount - request.amount_paid;

    if (amountToPay <= 0) {
      throw new Error('Invalid payment amount');
    }

    if (!request.allow_partial && amountToPay < request.amount - request.amount_paid) {
      throw new Error('Partial payments not allowed for this request');
    }

    const requestMetadata = parseMetadata(request.metadata);
    const paymentTarget = request.requester_agent_id
      ? {
          id: request.requester_agent_id,
          wallet_address: request.requester_address,
          paymentAddress: requestMetadata?.requester_payment_address,
        }
      : requestMetadata?.requester_payment_address || request.requester_address;

    // Make the payment
    const paymentResult = await pay({
      to: paymentTarget,
      amount: fromSmallestUnit(amountToPay, decimals),
      asset: request.asset,
      network: normalizeAcceptedNetworks(request.accepted_networks)[0],
      memo: `Payment for: ${request.description}`,
      referenceType: 'payment_request',
      referenceId: requestId,
    });

    // Update the request
    const newAmountPaid = request.amount_paid + amountToPay;
    const isFullyPaid = newAmountPaid >= request.amount;

    await commerce.a2a().updatePaymentRequest(requestId, {
      status: isFullyPaid ? 'paid' : 'processing',
      amount_paid: newAmountPaid,
      payment_ids: [...(request.payment_ids || []), paymentResult.payment.id],
      paid_at: isFullyPaid ? new Date().toISOString() : null,
    });

    // Trigger callback if configured
    if (request.callback_url && isFullyPaid) {
      triggerCallback(request.callback_url, {
        event: 'payment_request.paid',
        request_id: requestId,
        payment_id: paymentResult.payment.id,
        amount: amountToPay,
        total_paid: newAmountPaid,
      }).catch((err) => {
        console.debug('callback trigger failed:', err.message);
      }); // Fire and forget
    }

    return {
      success: true,
      payment: paymentResult.payment,
      request: {
        id: requestId,
        status: isFullyPaid ? 'paid' : 'processing',
        amountPaid: fromSmallestUnit(newAmountPaid, decimals),
        amountRemaining: fromSmallestUnit(request.amount - newAmountPaid, decimals),
        fullyPaid: isFullyPaid,
      },
    };
  }

  /**
   * Request a quote from another agent
   *
   * @param {Object} params - Quote request parameters
   * @param {string} params.seller - Seller agent ID or wallet
   * @param {Array} params.items - Items to quote
   * @param {string} [params.asset] - Preferred asset
   * @param {string} [params.network] - Preferred settlement network
   * @param {string} [params.message] - Message to seller
   * @returns {Promise<Object>} Quote request
   */
  async function requestQuote(params) {
    const {
      seller,
      items,
      asset = defaultAsset,
      network = defaultNetwork,
      message,
      metadata,
    } = params;

    if (!seller) {
      throw new Error('Seller is required');
    }
    if (!items || items.length === 0) {
      throw new Error('At least one item is required');
    }

    const sellerAgent = await resolveAgentAddress(seller, network);
    const now = new Date();
    const expiresAt = new Date(now.getTime() + 24 * 60 * 60 * 1000); // 24 hours

    // Convert items to quote format
    const quoteItems = items.map((item) => ({
      description: item.description || item.name,
      sku: item.sku || null,
      quantity: item.quantity || 1,
      unit_price: item.unitPrice ? toSmallestUnit(item.unitPrice, getAssetDecimals(asset)) : 0,
      metadata: item.metadata ? JSON.stringify(item.metadata) : null,
    }));

    const subtotal = quoteItems.reduce((sum, item) => sum + item.unit_price * item.quantity, 0);

    const quote = {
      id: randomUUID(),
      status: 'requested',
      buyer_agent_id: agentId,
      buyer_address: walletAddress,
      seller_agent_id: sellerAgent.agentId,
      seller_address: sellerAgent.address,
      items: quoteItems,
      subtotal,
      fees: 0,
      tax: 0,
      total: subtotal,
      total_decimal: fromSmallestUnit(subtotal, getAssetDecimals(asset)),
      asset: asset.toUpperCase(),
      accepted_networks: [network],
      expires_at: expiresAt.toISOString(),
      terms: null,
      estimated_delivery: null,
      delivery_method: null,
      fulfillment_instructions: null,
      payment_id: null,
      payment_request_id: null,
      request_message: message || null,
      response_message: null,
      metadata: mergeMetadata(metadata, {
        seller_payment_address: sellerAgent.paymentAddress || sellerAgent.address,
      }),
      created_at: now.toISOString(),
      quoted_at: null,
      accepted_at: null,
      fulfilled_at: null,
      updated_at: now.toISOString(),
    };

    await commerce.a2a().createQuote(quote);

    // Notification: seller is notified via webhook in the MCP tool layer
    // (see tools/a2a.js a2a_request_quote handler)

    return {
      success: true,
      quote: formatQuote(quote),
    };
  }

  /**
   * Provide a quote (seller responding to quote request)
   *
   * @param {string} quoteId - Quote ID to respond to
   * @param {Object} params - Quote parameters
   * @param {number} params.total - Total amount
   * @param {number} [params.fees] - Fees
   * @param {number} [params.tax] - Tax
   * @param {number} [params.expiresInHours] - Hours until quote expires
   * @param {string} [params.terms] - Terms and conditions
   * @param {string} [params.estimatedDelivery] - Estimated delivery time
   * @param {string} [params.message] - Message to buyer
   * @returns {Promise<Object>} Updated quote
   */
  async function provideQuote(quoteId, params) {
    const quote = await commerce.a2a().getQuote(quoteId);
    if (!quote) {
      throw new Error('Quote not found');
    }

    if (quote.status !== 'requested') {
      throw new Error(`Cannot provide quote in status: ${quote.status}`);
    }

    // Verify this agent is the seller
    if (quote.seller_address !== walletAddress) {
      throw new Error('Only the seller can provide a quote');
    }

    const {
      total,
      fees = 0,
      tax = 0,
      expiresInHours = 48,
      terms,
      estimatedDelivery,
      message,
    } = params;

    const decimals = getAssetDecimals(quote.asset);
    const totalSmallest = toSmallestUnit(total, decimals);
    const feesSmallest = toSmallestUnit(fees, decimals);
    const taxSmallest = toSmallestUnit(tax, decimals);
    const now = new Date();
    const expiresAt = new Date(now.getTime() + expiresInHours * 60 * 60 * 1000);

    await commerce.a2a().updateQuote(quoteId, {
      status: 'quoted',
      total: totalSmallest,
      total_decimal: total,
      fees: feesSmallest,
      tax: taxSmallest,
      expires_at: expiresAt.toISOString(),
      terms: terms || null,
      estimated_delivery: estimatedDelivery || null,
      response_message: message || null,
      quoted_at: now.toISOString(),
      updated_at: now.toISOString(),
    });

    const updatedQuote = await commerce.a2a().getQuote(quoteId);

    return {
      success: true,
      quote: formatQuote(updatedQuote),
    };
  }

  /**
   * Accept a quote and pay
   *
   * @param {string} quoteId - Quote ID to accept
   * @returns {Promise<Object>} Payment result
   */
  async function acceptQuote(quoteId) {
    const quote = await commerce.a2a().getQuote(quoteId);
    if (!quote) {
      throw new Error('Quote not found');
    }

    if (quote.status !== 'quoted') {
      throw new Error(`Cannot accept quote in status: ${quote.status}`);
    }

    if (new Date(quote.expires_at) < new Date()) {
      throw new Error('Quote has expired');
    }

    // Verify this agent is the buyer
    if (quote.buyer_address !== walletAddress) {
      throw new Error('Only the buyer can accept a quote');
    }

    const decimals = getAssetDecimals(quote.asset);
    const quoteMetadata = parseMetadata(quote.metadata);
    const paymentTarget = quote.seller_agent_id
      ? {
          id: quote.seller_agent_id,
          wallet_address: quote.seller_address,
          paymentAddress: quoteMetadata?.seller_payment_address,
        }
      : quoteMetadata?.seller_payment_address || quote.seller_address;

    // Make the payment
    const paymentResult = await pay({
      to: paymentTarget,
      amount: fromSmallestUnit(quote.total, decimals),
      asset: quote.asset,
      network: normalizeAcceptedNetworks(quote.accepted_networks)[0],
      memo: `Payment for quote ${quoteId}`,
      referenceType: 'quote',
      referenceId: quoteId,
    });

    // Update the quote
    await commerce.a2a().updateQuote(quoteId, {
      status: 'accepted',
      payment_id: paymentResult.payment.id,
      accepted_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    });

    return {
      success: true,
      payment: paymentResult.payment,
      quote: {
        id: quoteId,
        status: 'accepted',
        total: fromSmallestUnit(quote.total, decimals),
        asset: quote.asset,
      },
    };
  }

  /**
   * Counter-offer a quote (buyer proposes a different price)
   *
   * @param {string} quoteId - Quote ID to counter
   * @param {Object} params - Counter-offer parameters
   * @param {number} params.total - Proposed total amount
   * @param {string} [params.message] - Message to seller
   * @returns {Promise<Object>} Updated quote with negotiation round
   */
  async function counterQuote(quoteId, params) {
    const { total, message } = params;

    const quote = await commerce.a2a().getQuote(quoteId);
    if (!quote) {
      throw new Error('Quote not found');
    }

    if (quote.status !== 'quoted') {
      throw new Error(`Cannot counter a quote in status: ${quote.status}. Must be 'quoted'.`);
    }

    // Verify this agent is the buyer
    if (quote.buyer_address !== walletAddress) {
      throw new Error('Only the buyer can counter a quote');
    }

    const maxRounds = quote.max_rounds || 5;
    const counterCount = quote.counter_count || 0;

    if (counterCount >= maxRounds) {
      throw new Error(
        `Maximum negotiation rounds reached (${maxRounds}). Accept, decline, or start a new quote.`,
      );
    }

    if (new Date(quote.expires_at) < new Date()) {
      throw new Error('Quote has expired');
    }

    const decimals = getAssetDecimals(quote.asset);
    const totalSmallest = toSmallestUnit(total, decimals);
    const now = new Date().toISOString();
    const newRound = counterCount + 1;

    // Build negotiation history
    const history = Array.isArray(quote.negotiation_history) ? quote.negotiation_history : [];
    history.push({
      round: newRound,
      type: 'counter',
      from: 'buyer',
      amount: total,
      message: message || null,
      timestamp: now,
    });

    await commerce.a2a().updateQuote(quoteId, {
      status: 'counter_offered',
      total: totalSmallest,
      total_decimal: total,
      counter_count: newRound,
      negotiation_history: history,
      updated_at: now,
    });

    const updated = await commerce.a2a().getQuote(quoteId);

    return {
      success: true,
      quote: formatQuote(updated),
      round: newRound,
    };
  }

  /**
   * Revise a quote after a counter-offer (seller adjusts price)
   *
   * @param {string} quoteId - Quote ID to revise
   * @param {Object} params - Revision parameters
   * @param {number} params.total - Revised total amount
   * @param {number} [params.fees] - Revised fees
   * @param {number} [params.tax] - Revised tax
   * @param {string} [params.message] - Message to buyer
   * @returns {Promise<Object>} Updated quote with negotiation round
   */
  async function reviseQuote(quoteId, params) {
    const { total, fees = 0, tax = 0, message } = params;

    const quote = await commerce.a2a().getQuote(quoteId);
    if (!quote) {
      throw new Error('Quote not found');
    }

    if (quote.status !== 'counter_offered') {
      throw new Error(
        `Cannot revise a quote in status: ${quote.status}. Must be 'counter_offered'.`,
      );
    }

    // Verify this agent is the seller
    if (quote.seller_address !== walletAddress) {
      throw new Error('Only the seller can revise a quote');
    }

    if (new Date(quote.expires_at) < new Date()) {
      throw new Error('Quote has expired');
    }

    const decimals = getAssetDecimals(quote.asset);
    const totalSmallest = toSmallestUnit(total, decimals);
    const feesSmallest = toSmallestUnit(fees, decimals);
    const taxSmallest = toSmallestUnit(tax, decimals);
    const now = new Date().toISOString();
    const counterCount = quote.counter_count || 0;
    const newRound = counterCount + 1;

    // Build negotiation history
    const history = Array.isArray(quote.negotiation_history) ? quote.negotiation_history : [];
    history.push({
      round: newRound,
      type: 'revision',
      from: 'seller',
      amount: total,
      fees,
      tax,
      message: message || null,
      timestamp: now,
    });

    await commerce.a2a().updateQuote(quoteId, {
      status: 'quoted',
      total: totalSmallest,
      total_decimal: total,
      fees: feesSmallest,
      tax: taxSmallest,
      counter_count: newRound,
      negotiation_history: history,
      quoted_at: now,
      updated_at: now,
    });

    const updated = await commerce.a2a().getQuote(quoteId);

    return {
      success: true,
      quote: formatQuote(updated),
      round: newRound,
    };
  }

  /**
   * Decline a quote
   *
   * @param {string} quoteId - Quote ID to decline
   * @param {string} [reason] - Reason for declining
   * @returns {Promise<Object>} Result
   */
  async function declineQuote(quoteId, reason) {
    const quote = await commerce.a2a().getQuote(quoteId);
    if (!quote) {
      throw new Error('Quote not found');
    }

    if (quote.buyer_address !== walletAddress) {
      throw new Error('Only the buyer can decline a quote');
    }

    await commerce.a2a().updateQuote(quoteId, {
      status: 'declined',
      metadata: reason ? JSON.stringify({ declineReason: reason }) : null,
      updated_at: new Date().toISOString(),
    });

    return {
      success: true,
      quote: {
        id: quoteId,
        status: 'declined',
      },
    };
  }

  /**
   * Mark a quote as fulfilled
   *
   * @param {string} quoteId - Quote ID
   * @returns {Promise<Object>} Result
   */
  async function fulfillQuote(quoteId) {
    const quote = await commerce.a2a().getQuote(quoteId);
    if (!quote) {
      throw new Error('Quote not found');
    }

    if (quote.status !== 'accepted') {
      throw new Error(`Cannot fulfill quote in status: ${quote.status}`);
    }

    if (quote.seller_address !== walletAddress) {
      throw new Error('Only the seller can mark a quote as fulfilled');
    }

    await commerce.a2a().updateQuote(quoteId, {
      status: 'fulfilled',
      fulfilled_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    });

    return {
      success: true,
      quote: {
        id: quoteId,
        status: 'fulfilled',
      },
    };
  }

  /**
   * Get payment history
   *
   * @param {Object} [filter] - Filter options
   * @returns {Promise<Array>} Payments
   */
  async function getPayments(filter = {}) {
    const payments = await commerce.a2a().listPayments({
      ...filter,
      sender_address: filter.sent ? walletAddress : filter.sender_address,
      recipient_address: filter.received ? walletAddress : filter.recipient_address,
    });

    if (!filter.refreshOnChain) {
      return payments.map(formatPayment);
    }

    return Promise.all(
      payments.map(async (payment) => {
        const metadata = parseMetadata(payment.metadata);
        const shouldRefresh =
          payment.tx_hash &&
          payment.network &&
          payment.status === 'submitted' &&
          metadata?.simulated !== true;

        if (!shouldRefresh) {
          return formatPayment(payment);
        }

        const result = await refreshPayment(payment.id);
        if (result.success) {
          return result.payment;
        }

        return {
          ...result.payment,
          refreshError: result.error || null,
          finality: result.finality || result.payment?.finality || null,
        };
      }),
    );
  }

  /**
   * Get a single payment by ID.
   *
   * @param {string} paymentId - Payment ID
   * @returns {Promise<Object>} Payment
   */
  async function getPayment(paymentId) {
    if (!paymentId) {
      throw new Error('paymentId is required');
    }
    const payment = await getStoredPaymentForAgent(paymentId);
    return formatPayment(payment);
  }

  /**
   * Refresh a payment's on-chain status from the underlying network.
   *
   * @param {string} paymentId - Payment ID
   * @returns {Promise<Object>} Refreshed payment result
   */
  async function refreshPayment(paymentId) {
    if (!paymentId) {
      throw new Error('paymentId is required');
    }

    const payment = await getStoredPaymentForAgent(paymentId);
    const metadata = parseMetadata(payment.metadata);

    if (!payment.tx_hash || !payment.network) {
      return {
        success: true,
        refreshed: false,
        reason: 'payment_not_submitted',
        payment: formatPayment(payment),
        onChain: null,
        finality: getTrackedFinality(payment.id),
      };
    }

    if (metadata?.simulated === true) {
      return {
        success: true,
        refreshed: false,
        reason: 'simulated_payment',
        payment: formatPayment(payment),
        onChain: {
          txHash: payment.tx_hash,
          chainId: payment.network,
          confirmed: payment.status === 'completed',
          final: payment.status === 'completed',
          confirmations:
            metadata?.confirmations !== undefined && metadata?.confirmations !== null
              ? Number(metadata.confirmations)
              : null,
          requiredConfirmations: null,
          blockNumber: payment.block_number ?? null,
        },
        finality: getTrackedFinality(payment.id),
      };
    }

    const { getChain, getExplorerTxUrl, getTransactionStatus } = await import('../chains/index.js');
    const chain = getChain(payment.network);
    const requiredConfirmations = chain?.executionConfirmations || chain?.confirmations || 1;
    let status = null;
    try {
      status = await getTransactionStatus(payment.tx_hash, payment.network);
    } catch (error) {
      if (/status lookup requires a wallet-enabled JSON-RPC endpoint/i.test(error.message || '')) {
        return {
          success: true,
          refreshed: false,
          reason: 'status_unavailable',
          error: error.message,
          payment: formatPayment(payment),
          onChain: null,
          finality: hydrateTrackedSettlementFromPayment(payment),
        };
      }

      const updatedAt = new Date().toISOString();
      await commerce.a2a().updatePayment(paymentId, {
        status: 'failed',
        metadata: mergeMetadata(payment.metadata, {
          settlement_error: error.message,
          chain_id: payment.network,
          simulated: metadata?.simulated ?? false,
        }),
        updated_at: updatedAt,
      });
      const finality = markTrackedSettlementFailed(payment.id, error.message);
      const failedPayment = await getStoredPaymentForAgent(paymentId);
      return {
        success: false,
        refreshed: true,
        error: error.message,
        payment: formatPayment(failedPayment),
        onChain: {
          txHash: payment.tx_hash,
          chainId: payment.network,
          confirmed: false,
          final: false,
          confirmations: 0,
          requiredConfirmations,
          blockNumber: payment.block_number ?? null,
        },
        finality,
      };
    }
    const confirmations = Number(status?.confirmations || 0);
    const final = Boolean(status?.confirmed && confirmations >= requiredConfirmations);
    const nextStatus =
      final || payment.status === 'completed'
        ? 'completed'
        : payment.tx_hash
          ? 'submitted'
          : payment.status;
    const updatedAt = new Date().toISOString();

    await commerce.a2a().updatePayment(paymentId, {
      status: nextStatus,
      block_number:
        status?.blockNumber !== undefined && status?.blockNumber !== null
          ? status.blockNumber
          : payment.block_number,
      completed_at: final
        ? payment.completed_at || status?.confirmedAt || updatedAt
        : payment.completed_at,
      metadata: mergeMetadata(payment.metadata, {
        explorer_url: metadata?.explorer_url || getExplorerTxUrl(payment.network, payment.tx_hash),
        confirmations,
        chain_id: payment.network,
        simulated: metadata?.simulated ?? false,
      }),
      updated_at: updatedAt,
    });

    const refreshedPayment = await getStoredPaymentForAgent(paymentId);
    const finality = syncTrackedSettlement({
      intentId: payment.id,
      txHash: payment.tx_hash,
      chainId: payment.network,
      blockNumber:
        status?.blockNumber !== undefined && status?.blockNumber !== null
          ? status.blockNumber
          : (payment.block_number ?? 0),
      confirmations,
    });

    return {
      success: true,
      refreshed: true,
      payment: formatPayment(refreshedPayment),
      onChain: {
        txHash: payment.tx_hash,
        chainId: payment.network,
        confirmed: Boolean(status?.confirmed),
        final,
        confirmations,
        requiredConfirmations,
        blockNumber:
          status?.blockNumber !== undefined && status?.blockNumber !== null
            ? status.blockNumber
            : (payment.block_number ?? null),
      },
      finality,
    };
  }

  /**
   * Get payment requests
   *
   * @param {Object} [filter] - Filter options
   * @returns {Promise<Array>} Payment requests
   */
  async function getPaymentRequests(filter = {}) {
    const requests = await commerce.a2a().listPaymentRequests({
      ...filter,
      requester_address: filter.created ? walletAddress : filter.requester_address,
      payer_address: filter.received ? walletAddress : filter.payer_address,
    });

    return requests.map(formatPaymentRequest);
  }

  /**
   * Get quotes
   *
   * @param {Object} [filter] - Filter options
   * @returns {Promise<Array>} Quotes
   */
  async function getQuotes(filter = {}) {
    const quotes = await commerce.a2a().listQuotes({
      ...filter,
      buyer_address: filter.asBuyer ? walletAddress : filter.buyer_address,
      seller_address: filter.asSeller ? walletAddress : filter.seller_address,
    });

    return quotes.map(formatQuote);
  }

  function createBalanceBucket() {
    return {
      totalSent: 0,
      totalReceived: 0,
      netFlow: 0,
      paymentCountSent: 0,
      paymentCountReceived: 0,
      paymentCount: 0,
      networks: {},
    };
  }

  function normalizeSummaryRows(rows = []) {
    return rows
      .map((row) => ({
        asset: row?.asset || null,
        network: row?.network || null,
        paymentCount: Number(row?.payment_count ?? row?.paymentCount ?? 0),
        totalAmount: Number(row?.total_amount ?? row?.totalAmount ?? row?.total ?? 0),
      }))
      .filter((row) => row.asset && row.network);
  }

  function matchesPaymentSummaryFilter(payment, filter = {}) {
    if (filter.sender_address && payment.sender_address !== filter.sender_address) return false;
    if (filter.recipient_address && payment.recipient_address !== filter.recipient_address)
      return false;
    if (filter.sender_agent_id && payment.sender_agent_id !== filter.sender_agent_id) return false;
    if (filter.recipient_agent_id && payment.recipient_agent_id !== filter.recipient_agent_id)
      return false;
    if (filter.status && payment.status !== filter.status) return false;
    if (filter.asset && payment.asset !== filter.asset) return false;
    if (filter.network && payment.network !== filter.network) return false;
    return true;
  }

  async function summarizePaymentsForFlow(filter = {}) {
    const store = commerce.a2a();
    if (typeof store.summarizePayments === 'function') {
      return normalizeSummaryRows(await store.summarizePayments(filter));
    }

    const listedPayments = await store.listPayments({
      ...filter,
      limit: filter.limit || 5000,
      offset: filter.offset || 0,
    });
    const grouped = new Map();

    for (const payment of listedPayments || []) {
      if (!matchesPaymentSummaryFilter(payment, filter)) continue;
      const asset = payment.asset || null;
      const network = payment.network || null;
      if (!asset || !network) continue;

      const key = `${asset}:${network}`;
      const current = grouped.get(key) || {
        asset,
        network,
        paymentCount: 0,
        totalAmount: 0,
      };
      current.paymentCount += 1;
      current.totalAmount += Number(payment.amount_decimal || 0);
      grouped.set(key, current);
    }

    return [...grouped.values()].sort((a, b) =>
      a.asset === b.asset ? a.network.localeCompare(b.network) : a.asset.localeCompare(b.asset),
    );
  }

  function buildBalanceBreakdown(sentRows, receivedRows) {
    const breakdownByAsset = {};

    function applyRows(rows, direction) {
      const totalKey = direction === 'sent' ? 'totalSent' : 'totalReceived';
      const countKey = direction === 'sent' ? 'paymentCountSent' : 'paymentCountReceived';

      for (const row of rows) {
        const assetBucket = breakdownByAsset[row.asset] || createBalanceBucket();
        const networkBucket = assetBucket.networks[row.network] || {
          totalSent: 0,
          totalReceived: 0,
          netFlow: 0,
          paymentCountSent: 0,
          paymentCountReceived: 0,
          paymentCount: 0,
        };

        assetBucket[totalKey] += row.totalAmount;
        assetBucket[countKey] += row.paymentCount;
        breakdownByAsset[row.asset] = assetBucket;
        assetBucket.networks[row.network] = networkBucket;

        networkBucket[totalKey] += row.totalAmount;
        networkBucket[countKey] += row.paymentCount;
      }
    }

    applyRows(sentRows, 'sent');
    applyRows(receivedRows, 'received');

    const assets = Object.keys(breakdownByAsset).sort();
    let totalSent = 0;
    let totalReceived = 0;
    let paymentCountSent = 0;
    let paymentCountReceived = 0;

    for (const asset of assets) {
      const bucket = breakdownByAsset[asset];
      bucket.netFlow = bucket.totalReceived - bucket.totalSent;
      bucket.paymentCount = bucket.paymentCountSent + bucket.paymentCountReceived;
      totalSent += bucket.totalSent;
      totalReceived += bucket.totalReceived;
      paymentCountSent += bucket.paymentCountSent;
      paymentCountReceived += bucket.paymentCountReceived;

      const orderedNetworks = Object.keys(bucket.networks).sort();
      const nextNetworks = {};
      for (const network of orderedNetworks) {
        const networkBucket = bucket.networks[network];
        networkBucket.netFlow = networkBucket.totalReceived - networkBucket.totalSent;
        networkBucket.paymentCount =
          networkBucket.paymentCountSent + networkBucket.paymentCountReceived;
        nextNetworks[network] = networkBucket;
      }
      bucket.networks = nextNetworks;
    }

    return {
      breakdownByAsset,
      assets,
      totalSent,
      totalReceived,
      paymentCountSent,
      paymentCountReceived,
      paymentCount: paymentCountSent + paymentCountReceived,
    };
  }

  /**
   * Get balance/summary for this agent
   */
  async function getBalance(filter = {}) {
    const includeBreakdown = filter.includeBreakdown !== false;
    const baseFilter = {
      status: 'completed',
      asset: filter.asset,
      network: filter.network,
    };

    let totalSent = 0;
    let totalReceived = 0;
    let paymentCountSent = null;
    let paymentCountReceived = null;
    let paymentCount = null;
    let breakdownByAsset = null;
    let assets = [];
    let summarySource = 'totals_only';

    if (includeBreakdown) {
      const [sentRows, receivedRows] = await Promise.all([
        summarizePaymentsForFlow({ ...baseFilter, sender_address: walletAddress }),
        summarizePaymentsForFlow({ ...baseFilter, recipient_address: walletAddress }),
      ]);
      const summary = buildBalanceBreakdown(sentRows, receivedRows);
      totalSent = summary.totalSent;
      totalReceived = summary.totalReceived;
      paymentCountSent = summary.paymentCountSent;
      paymentCountReceived = summary.paymentCountReceived;
      paymentCount = summary.paymentCount;
      breakdownByAsset = summary.breakdownByAsset;
      assets = summary.assets;
      summarySource =
        typeof commerce.a2a().summarizePayments === 'function'
          ? 'store_aggregate'
          : 'list_payments_fallback';
    } else {
      [totalSent, totalReceived] = await Promise.all([
        commerce.a2a().sumPayments({ ...baseFilter, sender_address: walletAddress }),
        commerce.a2a().sumPayments({ ...baseFilter, recipient_address: walletAddress }),
      ]);
    }

    const aggregateAsset = filter.asset || (assets.length === 1 ? assets[0] : null);
    const aggregateTotalsMeaningful = Boolean(aggregateAsset) || assets.length <= 1;

    return {
      walletAddress,
      agentId,
      totalSent,
      totalReceived,
      netFlow: totalReceived - totalSent,
      aggregateTotalsMeaningful,
      aggregateAsset,
      asset: filter.asset || null,
      network: filter.network || null,
      assets,
      paymentCountSent,
      paymentCountReceived,
      paymentCount,
      summarySource,
      breakdownByAsset,
    };
  }

  // Format functions for consistent output
  function formatPayment(p) {
    const decimals = getAssetDecimals(p.asset);
    const metadata = parseMetadata(p.metadata);
    const finality = hydrateTrackedSettlementFromPayment(p);
    return {
      id: p.id,
      status: p.status,
      from: p.sender_address,
      to: p.recipient_address,
      amount:
        typeof p.amount_decimal === 'number'
          ? p.amount_decimal
          : fromSmallestUnit(p.amount, decimals),
      asset: p.asset,
      network: p.network,
      memo: p.memo,
      txHash: p.tx_hash,
      blockNumber: p.block_number ?? null,
      explorerUrl: metadata?.explorer_url || null,
      confirmations:
        metadata?.confirmations !== undefined && metadata?.confirmations !== null
          ? Number(metadata.confirmations)
          : null,
      chainId: metadata?.chain_id || p.network || null,
      simulated: metadata?.simulated ?? null,
      settlementError: metadata?.settlement_error || null,
      finality,
      createdAt: p.created_at,
      completedAt: p.completed_at,
    };
  }

  function formatPaymentRequest(r) {
    const decimals = getAssetDecimals(r.asset);
    const acceptedNetworks = normalizeAcceptedNetworks(r.accepted_networks);
    const metadata = parseMetadata(r.metadata);
    return {
      id: r.id,
      status: r.status,
      from: r.requester_address,
      payer: r.payer_address,
      requesterPaymentAddress: metadata?.requester_payment_address || r.requester_address,
      amount:
        typeof r.amount_decimal === 'number'
          ? r.amount_decimal
          : fromSmallestUnit(r.amount, decimals),
      amountPaid: fromSmallestUnit(r.amount_paid, decimals),
      asset: r.asset,
      network: acceptedNetworks[0],
      acceptedNetworks,
      description: r.description,
      expiresAt: r.expires_at,
      allowPartial: r.allow_partial,
      createdAt: r.created_at,
      paidAt: r.paid_at,
    };
  }

  function formatQuote(q) {
    const decimals = getAssetDecimals(q.asset);
    const acceptedNetworks = normalizeAcceptedNetworks(q.accepted_networks);
    const metadata = parseMetadata(q.metadata);
    return {
      id: q.id,
      status: q.status,
      buyer: q.buyer_address,
      seller: q.seller_address,
      sellerPaymentAddress: metadata?.seller_payment_address || q.seller_address,
      items: q.items,
      subtotal: fromSmallestUnit(q.subtotal, decimals),
      fees: fromSmallestUnit(q.fees, decimals),
      tax: fromSmallestUnit(q.tax, decimals),
      total:
        typeof q.total_decimal === 'number' ? q.total_decimal : fromSmallestUnit(q.total, decimals),
      asset: q.asset,
      network: acceptedNetworks[0],
      acceptedNetworks,
      expiresAt: q.expires_at,
      terms: q.terms,
      estimatedDelivery: q.estimated_delivery,
      createdAt: q.created_at,
      quotedAt: q.quoted_at,
      acceptedAt: q.accepted_at,
      fulfilledAt: q.fulfilled_at,
    };
  }

  // Trigger webhook callback
  async function triggerCallback(url, payload) {
    try {
      await fetch(url, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload),
      });
    } catch (error) {
      console.warn('Callback failed:', error.message);
    }
  }

  // ===========================================================================
  // Conditional Payments (escrow + x402 intent linking)
  // ===========================================================================

  /**
   * Create a conditional payment backed by escrow with optional x402 intent.
   * Combines escrow creation, funding, and x402 intent in one step.
   *
   * @param {Object} params
   * @param {string} params.sellerAddress - Seller wallet address
   * @param {number} params.amount - Payment amount (human-readable)
   * @param {string} [params.asset] - Asset (default: USDC)
   * @param {string} [params.network] - Network (default: set_chain)
   * @param {Array} [params.conditions] - Release conditions
   * @param {number} [params.expiresInHours] - Escrow expiry (default: 72)
   * @param {string} [params.quoteId] - Optional linked quote
   * @param {string} [params.memo] - Payment memo
   * @returns {Promise<Object>} Created escrow with intent details
   */
  async function createConditionalPayment(params) {
    const {
      sellerAddress,
      amount,
      asset = defaultAsset,
      network = defaultNetwork,
      conditions = [],
      expiresInHours = 72,
      quoteId,
      memo,
    } = params;

    if (!sellerAddress) throw new Error('sellerAddress is required');
    if (!amount || amount <= 0) throw new Error('amount must be positive');

    const decimals = getAssetDecimals(asset);
    const amountSmallest = toSmallestUnit(amount, decimals);
    const now = new Date();
    const expiresAt = new Date(now.getTime() + expiresInHours * 60 * 60 * 1000);

    // Build release conditions with defaults
    const releaseConditions = [...conditions];
    if (quoteId && !releaseConditions.some((c) => c.type === 'seller_fulfilled')) {
      releaseConditions.push({ type: 'seller_fulfilled', quoteId });
    }

    // Create escrow
    const escrow = await commerce.a2a().createEscrow({
      buyer_address: walletAddress,
      seller_address: sellerAddress,
      amount: amountSmallest,
      amount_decimal: amount,
      asset: asset.toUpperCase(),
      network,
      release_conditions: releaseConditions,
      expires_at: expiresAt.toISOString(),
      quote_id: quoteId || null,
      metadata: memo ? JSON.stringify({ memo }) : null,
    });

    // Create x402 payment intent if sequencer available
    let intentId = null;
    if (sequencerClient && signingKey) {
      try {
        const intent = await commerce.x402().createIntent({
          payer_address: walletAddress,
          payee_address: sellerAddress,
          amount: amountSmallest,
          asset: asset.toUpperCase(),
          network,
          description: memo || `Conditional payment (escrow ${escrow.id})`,
        });
        intentId = intent.id;

        // Link intent to escrow
        await commerce.a2a().updateEscrow(escrow.id, { intent_id: intentId });
      } catch (err) {
        console.warn('x402 intent creation failed (escrow still created):', err.message);
      }
    }

    // Fund the escrow (transition to funded/active)
    await commerce.a2a().updateEscrow(escrow.id, {
      status: 'funded',
      funded_at: now.toISOString(),
    });

    const funded = await commerce.a2a().getEscrow(escrow.id);

    return {
      success: true,
      escrow: {
        id: funded.id,
        status: funded.status,
        buyerAddress: funded.buyer_address,
        sellerAddress: funded.seller_address,
        amount,
        asset: funded.asset,
        network: funded.network,
        conditions: funded.release_conditions,
        expiresAt: funded.expires_at,
        intentId,
        createdAt: funded.created_at,
      },
    };
  }

  /**
   * Check if all conditions for a conditional payment are met.
   *
   * @param {string} escrowId - Escrow ID to check
   * @returns {Promise<Object>} Condition status
   */
  async function checkPaymentConditions(escrowId) {
    if (!escrowId) throw new Error('escrowId is required');

    const escrow = await commerce.a2a().getEscrow(escrowId);
    if (!escrow) throw new Error('Escrow not found');

    const conditions = Array.isArray(escrow.release_conditions)
      ? escrow.release_conditions
      : JSON.parse(escrow.release_conditions || '[]');

    const evaluated = [];
    let allMet = true;

    for (const condition of conditions) {
      let met = false;

      switch (condition.type) {
        case 'seller_fulfilled': {
          if (condition.quoteId) {
            const quote = await commerce.a2a().getQuote(condition.quoteId);
            met = quote?.status === 'fulfilled';
          }
          break;
        }
        case 'buyer_confirmed':
          met = condition.completed === true;
          break;
        case 'time_lock':
          met = condition.releaseAfter ? new Date() >= new Date(condition.releaseAfter) : false;
          break;
        case 'milestone':
          met = condition.completed === true;
          break;
        default:
          met = false;
      }

      evaluated.push({ ...condition, met });
      if (!met) allMet = false;
    }

    return {
      escrowId,
      status: escrow.status,
      allMet,
      conditions: evaluated,
      intentId: escrow.intent_id || null,
    };
  }

  /**
   * Settle a conditional payment by releasing escrow when conditions are met.
   *
   * @param {string} escrowId - Escrow ID to settle
   * @returns {Promise<Object>} Settlement result
   */
  async function settleConditionalPayment(escrowId) {
    if (!escrowId) throw new Error('escrowId is required');

    const conditionStatus = await checkPaymentConditions(escrowId);

    if (!conditionStatus.allMet) {
      const unmet = conditionStatus.conditions.filter((c) => !c.met);
      throw new Error(
        `Cannot settle: ${unmet.length} condition(s) not met — ${unmet.map((c) => c.type).join(', ')}`,
      );
    }

    const escrow = await commerce.a2a().getEscrow(escrowId);

    if (!['funded', 'active'].includes(escrow.status)) {
      throw new Error(`Cannot settle escrow in status: ${escrow.status}`);
    }

    // Release the escrow
    await commerce.a2a().updateEscrow(escrowId, {
      status: 'released',
      released_at: new Date().toISOString(),
    });

    // If linked to x402 intent, mark it settled
    let intentSettled = false;
    if (escrow.intent_id && sequencerClient) {
      try {
        await commerce.x402().updateIntent(escrow.intent_id, { status: 'settled' });
        intentSettled = true;
      } catch (err) {
        console.warn('x402 intent settlement update failed:', err.message);
      }
    }

    return {
      success: true,
      escrowId,
      status: 'released',
      amount: escrow.amount_decimal,
      asset: escrow.asset,
      sellerAddress: escrow.seller_address,
      intentId: escrow.intent_id || null,
      intentSettled,
    };
  }

  return {
    // Core payment operations
    pay,
    requestPayment,
    payRequest,

    // Quote operations
    requestQuote,
    provideQuote,
    acceptQuote,
    declineQuote,
    fulfillQuote,

    // Negotiation operations
    counterQuote,
    reviseQuote,

    // Conditional payment operations
    createConditionalPayment,
    checkPaymentConditions,
    settleConditionalPayment,

    // Query operations
    getPayment,
    refreshPayment,
    getPayments,
    getPaymentRequests,
    getQuotes,
    getBalance,

    // Utilities
    resolveAgentAddress,

    // Config
    walletAddress,
    agentId,
  };
}

export default { createA2AService };
