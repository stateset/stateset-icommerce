import { createHash } from 'crypto';

function normalizeForHash(value) {
  if (Array.isArray(value)) {
    return value.map((entry) => normalizeForHash(entry));
  }

  if (value && typeof value === 'object') {
    const sorted = {};
    for (const key of Object.keys(value).sort()) {
      sorted[key] = normalizeForHash(value[key]);
    }
    return sorted;
  }

  return value;
}

export function deterministicHash(value) {
  const normalized = normalizeForHash(value);
  return createHash('sha256').update(JSON.stringify(normalized)).digest('hex');
}

export function deterministicId(prefix, payload) {
  return `${prefix}_${deterministicHash(payload).slice(0, 24)}`;
}

export function clone(value) {
  return JSON.parse(JSON.stringify(value));
}

export function moneyToNumber(value) {
  const numeric = Number.parseFloat(String(value));
  if (!Number.isFinite(numeric)) {
    throw new Error(`Invalid monetary value: ${value}`);
  }
  return numeric;
}

export function normalizeMoney(value) {
  return moneyToNumber(value).toFixed(2);
}

export function roundMoney(value) {
  return Math.round(moneyToNumber(value) * 100) / 100;
}

export function nowIso() {
  return new Date().toISOString();
}

export function ensureProvider(providers, providerId, defaultProviderId) {
  const resolvedId = providerId || defaultProviderId;
  const provider = providers.find((entry) => entry.id === resolvedId);
  if (!provider) {
    throw new Error(
      `Unknown provider "${resolvedId}". Available providers: ${providers.map((p) => p.id).join(', ')}`,
    );
  }
  return provider;
}

export function filterProvidersByCapability(providers, capability) {
  if (!capability) {
    return providers;
  }
  return providers.filter((provider) => provider.capabilities.includes(capability));
}
