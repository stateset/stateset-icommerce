import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);

let commerceCtor = null;

export function getCommerceCtor() {
  if (commerceCtor) return commerceCtor;

  let mod;
  try {
    mod = require('@stateset/embedded');
  } catch (error) {
    const message = error && typeof error.message === 'string' ? error.message : String(error);
    throw new Error(`Failed to load @stateset/embedded. ${message}`);
  }

  const resolvedCtor = mod.Commerce || mod.default?.Commerce || mod.default;
  if (!resolvedCtor) {
    throw new Error('Failed to resolve Commerce export from @stateset/embedded.');
  }

  commerceCtor = resolvedCtor;
  return commerceCtor;
}

export function createCommerce(...args) {
  const Commerce = getCommerceCtor();
  return new Commerce(...args);
}

export const Commerce = getCommerceCtor();

export default Commerce;
