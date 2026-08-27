import { randomUUID } from 'node:crypto';

export const KERNEL_CAPABILITY_BY_TOOL = Object.freeze({
  a2a_create_escrow: 'a2a.escrow.create',
  a2a_dispute_escrow: 'a2a.escrow.dispute',
  a2a_file_dispute: 'a2a.dispute.file',
  a2a_fund_escrow: 'a2a.escrow.fund',
  a2a_refund_escrow: 'a2a.escrow.refund',
  a2a_resolve_dispute: 'a2a.dispute.resolve',
  a2a_submit_evidence: 'a2a.dispute.evidence.submit',
  create_payment: 'payments.create',
  create_refund: 'payments.create_refund',
  create_inventory_item: 'inventory.item.create',
  reserve_inventory: 'inventory.reserve',
  confirm_reservation: 'inventory.reservation.confirm',
  release_reservation: 'inventory.reservation.release',
  update_order_status: 'orders.transition',
  ship_order: 'orders.ship',
  approve_return: 'returns.transition',
  reject_return: 'returns.transition',
  mark_return_received: 'returns.transition',
  complete_return: 'returns.transition',
  cancel_return: 'returns.transition',
  post_journal_entry: 'ledger.post',
  create_product: 'products.create',
  x402_mark_settled: 'x402.settle',
  complete_checkout: 'checkout.commit',
  a2a_release_escrow: 'a2a.escrow.release',
});

const RETURN_STATUS_BY_TOOL = Object.freeze({
  approve_return: 'approved',
  reject_return: 'rejected',
  mark_return_received: 'received',
  complete_return: 'completed',
  cancel_return: 'cancelled',
});

function isPlainObject(value) {
  return Boolean(value && typeof value === 'object' && !Array.isArray(value));
}

function payloadFor(toolName, params, issuedAt = new Date(), actorAddress = null) {
  switch (toolName) {
    case 'create_inventory_item':
      return {
        sku: params.sku,
        name: params.name,
        description: params.description || null,
        unit_of_measure: params.unitOfMeasure || null,
        initial_quantity:
          params.initialQuantity === null || params.initialQuantity === undefined
            ? null
            : String(params.initialQuantity),
        location_id: params.locationId ?? null,
        reorder_point:
          params.reorderPoint === null || params.reorderPoint === undefined
            ? null
            : String(params.reorderPoint),
        safety_stock:
          params.safetyStock === null || params.safetyStock === undefined
            ? null
            : String(params.safetyStock),
      };
    case 'create_product':
      return {
        name: params.name,
        slug: null,
        description: params.description || null,
        product_type: null,
        attributes: null,
        seo: null,
        variants: (params.variants || []).map((variant, index) => ({
          sku: variant.sku,
          name: variant.name || null,
          price: String(variant.price),
          compare_at_price:
            variant.compareAtPrice === null || variant.compareAtPrice === undefined
              ? null
              : String(variant.compareAtPrice),
          cost: null,
          barcode: null,
          weight: null,
          weight_unit: null,
          options: null,
          is_default: index === 0,
        })),
      };
    case 'create_payment':
      return {
        order_id: params.orderId || null,
        amount: String(params.amount),
        currency: params.currency || 'USD',
        payment_method: params.method || 'credit_card',
        idempotency_key: params.idempotencyKey || null,
      };
    case 'create_refund':
      return {
        payment_id: params.paymentId,
        amount:
          params.amount === null || params.amount === undefined ? null : String(params.amount),
        reason: params.reason || null,
        idempotency_key: params.idempotencyKey || null,
      };
    case 'reserve_inventory':
      return {
        sku: params.sku,
        location_id: params.locationId ?? null,
        quantity: String(params.quantity),
        reference_type: params.referenceType,
        reference_id: params.referenceId,
        expires_in_seconds: params.expiresInSeconds ?? null,
      };
    case 'confirm_reservation':
      return {
        reservation_id: params.reservationId,
        quantity:
          params.quantity === null || params.quantity === undefined
            ? null
            : String(params.quantity),
      };
    case 'release_reservation':
      return { reservation_id: params.reservationId };
    case 'update_order_status':
      return {
        order_id: params.orderId,
        status: params.status,
        payment_status: params.paymentStatus || null,
      };
    case 'ship_order':
      return {
        order_id: params.orderId,
        tracking_number: params.trackingNumber || null,
        lines: params.lines || null,
      };
    case 'approve_return':
    case 'reject_return':
    case 'mark_return_received':
    case 'complete_return':
    case 'cancel_return':
      return { return_id: params.returnId, status: RETURN_STATUS_BY_TOOL[toolName] };
    case 'post_journal_entry':
      return { journal_entry_id: params.journalEntryId, posted_by: params.postedBy };
    case 'x402_mark_settled':
      return {
        intent_id: params.intentId,
        tx_hash: params.txHash,
        block_number: params.blockNumber,
      };
    case 'complete_checkout':
      return { cart_id: params.cartId };
    case 'a2a_create_escrow': {
      const network = params.network || 'set_chain';
      const asset =
        params.asset ||
        (network.startsWith('bitcoin') ? 'BTC' : network.startsWith('zcash') ? 'ZEC' : 'USDC');
      const expiresInHours = params.expiresInHours ?? 72;
      const expiresAt = new Date(issuedAt.getTime() + expiresInHours * 60 * 60 * 1000);
      const releaseConditions = (params.conditions || []).map((condition) => {
        if (condition.type === 'seller_fulfilled') {
          return {
            type: condition.type,
            quoteId: condition.quoteId || params.quoteId || null,
          };
        }
        if (condition.type === 'buyer_confirmed' || condition.type === 'milestone') {
          return { ...condition, completed: false };
        }
        return { ...condition };
      });
      return {
        quote_id: params.quoteId || null,
        payment_id: params.paymentId || null,
        buyer_address: params.buyerAddress,
        seller_address: params.sellerAddress,
        amount: String(params.amount),
        asset,
        network,
        release_conditions: releaseConditions,
        expires_at: expiresAt.toISOString(),
        auto_release_after: null,
        metadata: params.metadata || null,
      };
    }
    case 'a2a_fund_escrow':
      return { escrow_id: params.escrowId };
    case 'a2a_dispute_escrow':
      return {
        escrow_id: params.escrowId,
        reason: params.reason,
        category: params.category || null,
      };
    case 'a2a_file_dispute':
      return {
        escrow_id: params.escrowId,
        claimant_address: actorAddress,
        reason: params.reason,
        category: params.category || 'other',
        evidence_deadline: new Date(issuedAt.getTime() + 7 * 24 * 60 * 60 * 1000).toISOString(),
        review_deadline: new Date(issuedAt.getTime() + 14 * 24 * 60 * 60 * 1000).toISOString(),
        metadata: null,
      };
    case 'a2a_submit_evidence':
      return {
        dispute_id: params.disputeId,
        submitted_by: actorAddress,
        evidence_type: params.evidenceType,
        title: params.title,
        description: params.description || null,
        content: params.content,
      };
    case 'a2a_resolve_dispute':
      return {
        dispute_id: params.disputeId,
        resolution_type: params.resolutionType,
        buyer_amount:
          params.buyerAmount === null || params.buyerAmount === undefined
            ? null
            : String(params.buyerAmount),
        seller_amount:
          params.sellerAmount === null || params.sellerAmount === undefined
            ? null
            : String(params.sellerAmount),
        note: params.note || null,
      };
    case 'a2a_release_escrow':
      return { escrow_id: params.escrowId };
    case 'a2a_refund_escrow':
      return { escrow_id: params.escrowId, reason: params.reason || null };
    default:
      return null;
  }
}

function normalizePrincipal(principal) {
  if (!isPlainObject(principal)) {
    throw new Error('Kernel execution requires a trusted principal configuration.');
  }
  return {
    id: principal.id,
    kind: principal.kind || 'agent',
    tenant_id: principal.tenant_id || principal.tenantId || null,
    delegated_by: principal.delegated_by || principal.delegatedBy || null,
    capabilities: Array.from(new Set((principal.capabilities || []).map(String))),
  };
}

/** Build an executor whose policy and identity are closed over host config. */
export function createKernelToolExecutor({ commerce, kernel, allowApply, agentConfig = null }) {
  const kernelConfig = isPlainObject(kernel) ? kernel : null;
  return async (toolName, params = {}, executionOptions = {}) => {
    const issuedAt = new Date();
    const commandType = KERNEL_CAPABILITY_BY_TOOL[toolName];
    if (!commandType) {
      if (
        kernelConfig &&
        kernelConfig.strict !== false &&
        allowApply &&
        executionOptions.requireGoverned
      ) {
        throw new Error(
          `Tool '${toolName}' is a write mutation outside the governed kernel catalog. ` +
            'Add a typed kernel command or explicitly set kernel.strict=false for legacy migration.',
        );
      }
      return null;
    }
    if (!kernelConfig) {
      if (!allowApply) return null;
      throw new Error(
        `Tool '${toolName}' is a governed mutation and apply mode requires trusted kernel configuration.`,
      );
    }
    const actorAddress =
      agentConfig?.walletAddress ||
      kernelConfig.actorAddress ||
      kernelConfig.actor_address ||
      kernelConfig.principal?.id ||
      null;
    const payload = payloadFor(toolName, params, issuedAt, actorAddress);
    if (!payload) {
      throw new Error(`Governed tool '${toolName}' does not define a typed command payload.`);
    }
    if (
      kernelConfig.strict !== false &&
      toolName === 'create_inventory_item' &&
      [params.initialQuantity, params.reorderPoint, params.safetyStock].some(
        (value) => value !== null && value !== undefined && typeof value !== 'string',
      )
    ) {
      throw new Error(
        'Strict kernel inventory creation requires quantities as exact decimal strings.',
      );
    }
    if (
      kernelConfig.strict !== false &&
      toolName === 'create_product' &&
      (params.variants || []).some(
        (variant) =>
          typeof variant.price !== 'string' ||
          (variant.compareAtPrice !== null &&
            variant.compareAtPrice !== undefined &&
            typeof variant.compareAtPrice !== 'string'),
      )
    ) {
      throw new Error(
        'Strict kernel product creation requires variant prices as exact decimal strings.',
      );
    }
    if (
      kernelConfig.strict !== false &&
      toolName === 'a2a_create_escrow' &&
      typeof params.amount !== 'string'
    ) {
      throw new Error('Strict kernel escrow creation requires amount as an exact decimal string.');
    }
    if (
      kernelConfig.strict !== false &&
      toolName === 'a2a_resolve_dispute' &&
      params.resolutionType === 'partial_refund'
    ) {
      throw new Error(
        'Strict kernel mode replaces partial_refund with split; provide exact buyerAmount and sellerAmount allocations.',
      );
    }
    if (
      kernelConfig.strict !== false &&
      toolName === 'a2a_resolve_dispute' &&
      params.resolutionType === 'split' &&
      (typeof params.buyerAmount !== 'string' || typeof params.sellerAmount !== 'string')
    ) {
      throw new Error(
        'Strict kernel split resolution requires buyerAmount and sellerAmount as exact decimal strings.',
      );
    }
    if (typeof commerce.executeKernelCommand !== 'function') {
      throw new Error(
        'The installed @stateset/embedded build does not expose executeKernelCommand; upgrade the native binding.',
      );
    }
    if (!isPlainObject(kernelConfig.policy)) {
      throw new Error('kernel.policy must be trusted host policy configuration.');
    }
    const storeId = kernelConfig.storeId || kernelConfig.store_id;
    if (!storeId) throw new Error('kernel.storeId is required.');

    const command = {
      contract_version: '1.0',
      command_id: randomUUID(),
      idempotency_key: executionOptions.idempotencyKey || params.idempotencyKey || randomUUID(),
      command_type: commandType,
      principal: normalizePrincipal(kernelConfig.principal),
      store_id: String(storeId),
      correlation_id: executionOptions.correlationId || null,
      causation_id: executionOptions.causationId || null,
      expected_version: executionOptions.expectedVersion ?? null,
      policy_version: kernelConfig.policy.version || null,
      approval: null,
      authority: null,
      deadline: executionOptions.deadline || null,
      trace_id: executionOptions.traceId || null,
      mode: allowApply ? 'apply' : 'preview',
      payload,
      issued_at: issuedAt.toISOString(),
    };
    const approvalSource = executionOptions.approval ?? kernelConfig.approval;
    command.approval =
      typeof approvalSource === 'function' ? await approvalSource(command) : approvalSource || null;
    const authoritySource = executionOptions.authorize ?? kernelConfig.authorize;
    command.authority =
      typeof authoritySource === 'function'
        ? await authoritySource(command)
        : executionOptions.authority || null;

    const receipt = await commerce.executeKernelCommand(command, kernelConfig.policy);
    return {
      success: receipt?.status === 'succeeded' || receipt?.status === 'previewed',
      kernel: true,
      commandType,
      preview: receipt?.status === 'previewed',
      receipt,
      result: receipt?.result ?? null,
    };
  };
}
