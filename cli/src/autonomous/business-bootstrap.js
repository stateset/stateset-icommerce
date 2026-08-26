import { randomUUID } from 'node:crypto';

export const AUTONOMOUS_BUSINESS_SCHEMA_VERSION = '1.0';

export const GOVERNED_AUTONOMOUS_CAPABILITIES = Object.freeze([
  'a2a.dispute.evidence.submit',
  'a2a.dispute.file',
  'a2a.dispute.resolve',
  'a2a.escrow.create',
  'a2a.escrow.dispute',
  'a2a.escrow.fund',
  'a2a.escrow.release',
  'a2a.escrow.refund',
  'checkout.commit',
  'inventory.item.create',
  'inventory.reservation.confirm',
  'inventory.reservation.release',
  'inventory.reserve',
  'ledger.post',
  'orders.ship',
  'orders.transition',
  'payments.create',
  'payments.create_refund',
  'products.create',
  'returns.transition',
  'subscriptions.charge',
  'x402.settle',
]);

export const PRODUCTION_LAUNCH_REQUIREMENTS = Object.freeze([
  'authority',
  'business_identity',
  'disputes',
  'fulfillment',
  'payouts',
  'tax',
]);

function requiredText(value, field) {
  const normalized = String(value ?? '').trim();
  if (!normalized) throw new Error(`${field} is required.`);
  return normalized;
}

function slug(value) {
  const normalized = requiredText(value, 'name')
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-|-$/g, '');
  if (!normalized) throw new Error('name must contain at least one letter or number.');
  return normalized;
}

function normalizeCurrency(value) {
  const currency = String(value || 'USD')
    .trim()
    .toUpperCase();
  if (!/^[A-Z]{3}$/.test(currency)) {
    throw new Error('baseCurrency must be a three-letter ISO 4217 currency code.');
  }
  return currency;
}

function normalizeJurisdiction(value) {
  const jurisdiction = String(value || 'UNSPECIFIED')
    .trim()
    .toUpperCase();
  if (!/^(UNSPECIFIED|[A-Z]{2})$/.test(jurisdiction)) {
    throw new Error('jurisdiction must be a two-letter country code or UNSPECIFIED.');
  }
  return jurisdiction;
}

function commandPolicy(capability, { approval = false } = {}, tenantId, storeId) {
  return {
    required_capabilities: [capability],
    requires_approval: approval,
    requires_tenant: true,
    requires_store: true,
    allowed_tenant_ids: [tenantId],
    allowed_store_ids: [storeId],
    requires_agent_delegation: true,
    requires_signed_authority: false,
  };
}

/**
 * Create the portable, provider-neutral control plane for a new agent-operated business.
 * No credentials or private keys are generated or persisted here.
 */
export function createBusinessBootstrap(options = {}) {
  const name = requiredText(options.name, 'name');
  const ownerId = requiredText(options.ownerId, 'ownerId');
  const businessSlug = slug(name);
  const businessId = String(options.businessId || `business:${businessSlug}`).trim();
  const tenantId = String(options.tenantId || `tenant:${businessSlug}`).trim();
  const storeId = String(options.storeId || `store:${businessSlug}`).trim();
  const agentId = String(options.agentId || `agent:${businessSlug}:operator`).trim();
  const jurisdiction = normalizeJurisdiction(options.jurisdiction);
  const baseCurrency = normalizeCurrency(options.baseCurrency);
  const createdAt = options.createdAt || new Date().toISOString();
  const policyVersion = String(options.policyVersion || `${businessSlug}-v1`).trim();

  const approvalRequired = new Set([
    'a2a.dispute.resolve',
    'a2a.escrow.release',
    'a2a.escrow.refund',
    'ledger.post',
    'payments.create_refund',
  ]);
  const commands = Object.fromEntries(
    GOVERNED_AUTONOMOUS_CAPABILITIES.map((capability) => [
      capability,
      commandPolicy(capability, { approval: approvalRequired.has(capability) }, tenantId, storeId),
    ]),
  );

  const manifest = {
    schema_version: AUTONOMOUS_BUSINESS_SCHEMA_VERSION,
    id: businessId,
    name,
    objective: String(
      options.objective || 'Create sustainable customer value within the delegated authority.',
    ).trim(),
    status: 'bootstrap_pending',
    jurisdiction,
    base_currency: baseCurrency,
    tenant_id: tenantId,
    store_id: storeId,
    owner_principal_id: ownerId,
    operator_agent_id: agentId,
    kernel_policy_version: policyVersion,
    operating_mode: 'preview',
    created_at: createdAt,
    launch_requirements: Object.fromEntries(
      PRODUCTION_LAUNCH_REQUIREMENTS.map((requirement) => [requirement, null]),
    ),
    metadata: {
      bootstrap_id: randomUUID(),
      credentials_embedded: false,
      private_keys_embedded: false,
    },
  };

  const principal = {
    id: agentId,
    kind: 'agent',
    tenant_id: tenantId,
    delegated_by: ownerId,
    capabilities: [...GOVERNED_AUTONOMOUS_CAPABILITIES],
  };

  const policy = {
    version: policyVersion,
    commands,
    trusted_authority_keys: {},
  };

  return {
    manifest,
    principal,
    policy,
    readiness: evaluateBusinessReadiness({ manifest, principal, policy }),
  };
}

/** Evaluate whether a business may leave preview mode. */
export function evaluateBusinessReadiness(bundle) {
  const manifest = bundle?.manifest || {};
  const principal = bundle?.principal || {};
  const policy = bundle?.policy || {};
  const requirements = manifest.launch_requirements || {};
  const checks = [];

  for (const requirement of PRODUCTION_LAUNCH_REQUIREMENTS) {
    const reference = requirements[requirement];
    checks.push({
      id: requirement,
      passed: typeof reference === 'string' && reference.trim().length > 0,
      evidence: typeof reference === 'string' && reference.trim() ? reference.trim() : null,
    });
  }

  checks.push({
    id: 'delegated_agent',
    passed: Boolean(principal.id && principal.delegated_by && principal.tenant_id),
    evidence: principal.delegated_by || null,
  });
  checks.push({
    id: 'deny_by_default_policy',
    passed:
      Boolean(policy.version) &&
      GOVERNED_AUTONOMOUS_CAPABILITIES.every((capability) => policy.commands?.[capability]),
    evidence: policy.version || null,
  });
  checks.push({
    id: 'tenant_store_scope',
    passed: Object.values(policy.commands || {}).every(
      (rule) =>
        rule.requires_tenant === true &&
        rule.requires_store === true &&
        rule.allowed_tenant_ids?.includes(manifest.tenant_id) &&
        rule.allowed_store_ids?.includes(manifest.store_id),
    ),
    evidence:
      manifest.tenant_id && manifest.store_id ? `${manifest.tenant_id}/${manifest.store_id}` : null,
  });

  const failed = checks.filter((check) => !check.passed).map((check) => check.id);
  return {
    schema_version: AUTONOMOUS_BUSINESS_SCHEMA_VERSION,
    ready_for_apply: failed.length === 0 && manifest.operating_mode === 'apply',
    ready_for_preview: checks
      .filter((check) =>
        ['delegated_agent', 'deny_by_default_policy', 'tenant_store_scope'].includes(check.id),
      )
      .every((check) => check.passed),
    operating_mode: manifest.operating_mode || 'preview',
    checks,
    blockers: failed,
  };
}
