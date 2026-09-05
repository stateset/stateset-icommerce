import { createHash, randomUUID } from 'node:crypto';
import { canonicalJson } from './codec.mjs';
import { amount, exactMoney } from './quote-money.mjs';

const hash = (value) => createHash('sha256').update(canonicalJson(value)).digest('hex');
function required(value, name) {
  if (typeof value !== 'string' || !value.trim() || value.length > 512)
    throw new Error(`${name} is required`);
  return value;
}

/** Operator-owned bridge to native checkout.commit. The kernel transaction is
 * authoritative: order, stock reservations, outbox and receipt commit together.
 * The protocol journal is a separate checkpoint, never a distributed transaction.
 */
export class NativeMerchantCheckout {
  constructor({
    store,
    commerce,
    principal,
    storeId,
    policy,
    resolveQuote,
    readReceipt,
    allowApply = false,
    stockPolicy = 'allow_backorder',
  }) {
    this.store = store;
    this.commerce = commerce;
    this.principal = structuredClone(principal);
    this.storeId = required(storeId, 'storeId');
    this.policy = structuredClone(policy);
    required(principal.id, 'principal id');
    required(principal.tenant_id, 'tenant id');
    required(policy.version, 'policy version');
    if (typeof resolveQuote !== 'function' || typeof readReceipt !== 'function')
      throw new Error('operator quote resolver and receipt lookup are required');
    this.resolveQuote = resolveQuote;
    this.readReceipt = readReceipt;
    this.allowApply = allowApply === true;
    if (!['allow_backorder', 'reject_if_insufficient'].includes(stockPolicy))
      throw new Error('invalid checkout stock policy');
    this.stockPolicy = stockPolicy;
    this.scope = hash([principal.id, principal.tenant_id, storeId]);
    this.operations = store.collection('native_checkout_operations');
    this.claims = store.collection('native_checkout_carts');
  }

  async accept(input) {
    if (!input || Object.keys(input).some((key) => !['quoteId', 'idempotencyKey'].includes(key)))
      throw new Error('unknown acceptance argument');
    const request = {
      quoteId: required(input.quoteId, 'quoteId'),
      idempotencyKey: required(input.idempotencyKey, 'idempotencyKey'),
    };
    const id = hash([this.scope, request.idempotencyKey]);
    const prior = this.operations.get(id);
    if (prior) {
      if (prior.request.quoteId !== request.quoteId)
        throw new Error('acceptance idempotency conflict');
      return this.allowApply ? this.resume(id) : { id, status: 'preview', existing: true };
    }
    await this.requireStockPolicySupport(this.stockPolicy);
    const quote = structuredClone(
      await this.resolveQuote(request.quoteId, structuredClone(this.principal)),
    );
    if (
      quote.quoteId !== request.quoteId ||
      amount(quote.amount) <= 0n ||
      !/^[A-Z]{3}$/.test(quote.currency) ||
      !Number.isFinite(Date.parse(quote.expiresAt)) ||
      Date.parse(quote.expiresAt) <= Date.now()
    ) {
      throw new Error('invalid or expired native checkout quote');
    }
    required(quote.cartId, 'quote cartId');
    const command = {
      contract_version: '1.0',
      command_id: randomUUID(),
      command_type: 'checkout.commit',
      idempotency_key: `native-checkout:${id}`,
      principal: this.principal,
      store_id: this.storeId,
      policy_version: this.policy.version,
      mode: this.allowApply ? 'apply' : 'preview',
      issued_at: new Date().toISOString(),
      deadline: quote.expiresAt,
      payload: { cart_id: quote.cartId },
      commitment: {
        budget_id: quote.budgetId ?? null,
        amount: { amount: exactMoney(amount(quote.amount)), currency: quote.currency },
        counterparty_id: null,
        quantity: null,
        evidence: [quote.quoteId],
      },
    };
    // Preserve legacy command serialization when backorders are permitted.
    if (this.stockPolicy === 'reject_if_insufficient')
      command.payload.stock_policy = this.stockPolicy;
    if (!this.allowApply) {
      command.idempotency_key = `native-checkout-preview:${randomUUID()}`;
      return {
        id,
        status: 'preview',
        receipt: await this.commerce.executeKernelCommand(command, this.policy),
      };
    }
    this.store.atomic(() => {
      const existing = this.operations.get(id);
      if (existing) {
        if (existing.request.quoteId !== request.quoteId)
          throw new Error('acceptance idempotency conflict');
        return;
      }
      const claimed = this.claims.get(quote.cartId);
      if (claimed && claimed !== id) throw new Error('cart is already bound to another acceptance');
      this.claims.set(quote.cartId, id);
      this.operations.set(id, { id, scope: this.scope, request, command });
    });
    return this.resume(id);
  }

  async resume(id) {
    if (!this.allowApply) throw new Error('apply is disabled');
    const operation = this.operations.get(id);
    if (!operation || operation.scope !== this.scope) throw new Error('acceptance not found');
    const { command } = operation;
    // Always consult the native ledger, including after a lost response or a
    // failed projection write. Null MUST mean authoritative absence.
    let receipt;
    try {
      receipt = await this.readReceipt(command.idempotency_key);
      if (receipt === null) {
        // Check the persisted policy, not current constructor defaults. A
        // restart or binary downgrade must never weaken an accepted intent.
        await this.requireStockPolicySupport(command.payload.stock_policy);
        receipt = await this.commerce.executeKernelCommand(
          structuredClone(command),
          structuredClone(this.policy),
        );
      }
      if (!receipt || typeof receipt !== 'object')
        throw new Error('invalid native receipt lookup result');
    } catch (error) {
      return { id, status: 'reconciling', error: String(error.message || error) };
    }
    if (
      receipt.idempotency_key !== command.idempotency_key ||
      receipt.command_id !== command.command_id ||
      receipt.command_type !== 'checkout.commit'
    )
      throw new Error('native receipt identity mismatch');
    if (receipt.status === 'succeeded') {
      if (
        !receipt.result?.order_id ||
        receipt.result.cart_id !== command.payload.cart_id ||
        receipt.result.currency !== command.commitment.amount.currency ||
        amount(receipt.result.total_charged) !== amount(command.commitment.amount.amount)
      ) {
        throw new Error('native receipt does not match the accepted quote');
      }
      return { id, status: 'accepted', orderId: receipt.result.order_id, receipt };
    }
    return { id, status: receipt.status === 'rejected' ? 'rejected' : 'reconciling', receipt };
  }

  async requireStockPolicySupport(policy) {
    if (policy === undefined || policy === 'allow_backorder') return;
    if (policy !== 'reject_if_insufficient') throw new Error('invalid checkout stock policy');
    const features =
      typeof this.commerce.kernelFeatures === 'function'
        ? await this.commerce.kernelFeatures()
        : null;
    if (!Array.isArray(features) || !features.includes('checkout.stock_policy.v1'))
      throw new Error(
        'native binary does not advertise checkout.stock_policy.v1; rebuild or upgrade',
      );
  }
}
