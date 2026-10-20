import { createHash } from 'node:crypto';
import { canonicalJson } from './codec.mjs';
import { amount } from './quote-money.mjs';

const hash = (value) => createHash('sha256').update(canonicalJson(value)).digest('hex');
const uuid = (hex) =>
  `${hex.slice(0, 8)}-${hex.slice(8, 12)}-4${hex.slice(13, 16)}-8${hex.slice(17, 20)}-${hex.slice(20, 32)}`;

/** Record externally captured money using governed commands. The provider is
 * read-only. Its identity and reference are pinned before lookup; one provider
 * payment cannot satisfy two orders in this protocol store. */
export class NativeMerchantPayment {
  constructor({
    store,
    commerce,
    principal,
    policy,
    storeId,
    readReceipt,
    provider,
    resolvePayment,
    allowApply = false,
  }) {
    if (
      typeof provider?.identity !== 'string' ||
      !provider.identity ||
      typeof provider.lookup !== 'function' ||
      typeof resolvePayment !== 'function' ||
      typeof readReceipt !== 'function'
    )
      throw new Error('operator payment lookup and resolver required');
    Object.assign(this, {
      store,
      commerce,
      storeId,
      readReceipt,
      provider,
      resolvePayment,
      allowApply,
    });
    this.principal = structuredClone(principal);
    this.policy = structuredClone(policy);
    this.scope = hash([principal.id, principal.tenant_id, storeId]);
    this.operations = store.collection('native_payment_operations');
    this.claims = store.collection('native_payment_claims');
    this.commands = store.collection('native_payment_commands');
  }

  async reconcile(terms) {
    if (this.allowApply !== true) return { status: 'preview' };
    terms = structuredClone(terms);
    const id = hash([this.scope, terms.quoteId]);
    try {
      let operation = this.operations.get(id);
      if (!operation) {
        const reference = await this.resolvePayment(structuredClone(terms));
        if (reference === null) return { status: 'pending' };
        if (typeof reference !== 'string' || !reference || reference.length > 256)
          throw new Error('invalid payment reference');
        this.store.atomic(() => {
          if (this.operations.get(id)) return;
          const claim = hash([this.provider.identity, reference]);
          const owner = this.claims.get(claim);
          if (owner && owner !== id) throw new Error('provider payment already claimed');
          this.claims.set(claim, id);
          this.operations.set(id, {
            id,
            terms,
            reference,
            provider: this.provider.identity,
            createdAt: new Date().toISOString(),
          });
        });
        operation = this.operations.get(id);
      }
      if (
        canonicalJson(operation.terms) !== canonicalJson(terms) ||
        operation.provider !== this.provider.identity
      )
        throw new Error('payment binding changed');

      // A completed kernel receipt is authoritative after lost responses or a
      // protocol checkpoint failure. This is historical capture evidence, not
      // a claim that the provider can never refund/dispute it later.
      const completedCommand = this.commands.get(`${id}:complete`);
      if (completedCommand) {
        const receipt = await this.readReceipt(completedCommand.idempotency_key);
        if (receipt !== null) return this.project(receipt, completedCommand, operation);
      }
      const verified = await this.provider.lookup(operation.reference, structuredClone(terms));
      if (verified?.status === 'pending') return { status: 'pending' };
      if (verified?.status !== 'succeeded') return { status: 'needs_attention' };
      const evidence = verified.evidence;
      if (
        !evidence ||
        evidence.provider !== operation.provider ||
        evidence.reference !== operation.reference ||
        evidence.currency !== terms.currency ||
        amount(evidence.amount) !== amount(terms.amount) ||
        evidence.cartId !== terms.cartId ||
        evidence.cartFingerprint !== terms.cartFingerprint ||
        evidence.buyer !== terms.buyer ||
        typeof evidence.transactionId !== 'string' ||
        !evidence.transactionId
      )
        throw new Error('provider evidence mismatch');
      // Persist only an allowlist, never arbitrary provider data or secrets.
      const proof = {
        provider: operation.provider,
        reference: operation.reference,
        transactionId: evidence.transactionId,
        amount: terms.amount,
        currency: terms.currency,
      };
      const create = this.command(operation, 'create', 'payments.create', {
        order_id: terms.orderId,
        amount: terms.amount,
        currency: terms.currency,
        payment_method: 'credit_card',
        processor: operation.provider,
        external_id: operation.reference,
        metadata: canonicalJson(proof),
      });
      const created = await this.run(create);
      const projection = this.project(created, create, operation);
      if (projection.status !== 'pending') return projection;
      const complete = this.command(operation, 'complete', 'payments.complete', {
        payment_id: created.result.id,
      });
      return this.project(await this.run(complete), complete, operation);
    } catch {
      // Neither a provider timeout nor a lost kernel response proves failure.
      // Keep the reference claimed and avoid exposing provider errors/secrets.
      return { status: 'reconciling' };
    }
  }

  command(operation, step, type, payload) {
    const key = `${operation.id}:${step}`;
    return this.store.atomic(() => {
      const prior = this.commands.get(key);
      if (prior) {
        if (canonicalJson(prior.payload) !== canonicalJson(payload))
          throw new Error('payment command evidence changed');
        return prior;
      }
      const command = {
        contract_version: '1.0',
        command_id: uuid(hash(key)),
        command_type: type,
        idempotency_key: `native-payment:${key}`,
        principal: this.principal,
        store_id: this.storeId,
        policy_version: this.policy.version,
        mode: 'apply',
        issued_at: operation.createdAt,
        payload,
      };
      if (step === 'create')
        command.commitment = {
          amount: { amount: operation.terms.amount, currency: operation.terms.currency },
          evidence: [operation.terms.quoteId, hash(payload.metadata)],
        };
      this.commands.set(key, command);
      return command;
    });
  }

  async run(command) {
    const receipt = await this.readReceipt(command.idempotency_key);
    if (receipt === null)
      return this.commerce.executeKernelCommand(
        structuredClone(command),
        structuredClone(this.policy),
      );
    return receipt;
  }

  project(receipt, command, operation) {
    if (
      !receipt ||
      receipt.idempotency_key !== command.idempotency_key ||
      receipt.command_id !== command.command_id ||
      receipt.command_type !== command.command_type
    )
      throw new Error('payment receipt identity mismatch');
    if (receipt.status === 'rejected') return { status: 'needs_attention' };
    if (receipt.status !== 'succeeded') return { status: 'reconciling' };
    const payment = receipt.result;
    if (
      !payment?.id ||
      payment.order_id !== operation.terms.orderId ||
      payment.currency !== operation.terms.currency ||
      amount(payment.amount) !== amount(operation.terms.amount) ||
      payment.processor !== operation.provider ||
      payment.external_id !== operation.reference ||
      (command.command_type === 'payments.complete' &&
        (payment.id !== command.payload.payment_id || payment.status !== 'completed'))
    )
      throw new Error('payment receipt result mismatch');
    return command.command_type === 'payments.complete'
      ? { status: 'recorded', payment_id: payment.id }
      : { status: 'pending', payment_id: payment.id };
  }
}
