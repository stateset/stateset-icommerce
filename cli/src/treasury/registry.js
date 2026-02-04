/**
 * Token Registry for Treasury
 */

import fs from 'node:fs/promises';
import { existsSync, mkdirSync } from 'node:fs';
import { join, dirname, resolve } from 'node:path';
import { defaultTreasuryDir } from './store.js';

export function defaultRegistryPath(cwd = process.cwd()) {
  return join(defaultTreasuryDir(cwd), 'tokens.json');
}

function ensureDirForFile(filePath) {
  const dir = dirname(resolve(filePath));
  mkdirSync(dir, { recursive: true });
}

export async function loadTokenRegistry(registryPath = defaultRegistryPath()) {
  if (!existsSync(registryPath)) {
    return { tokens: [] };
  }

  const raw = await fs.readFile(registryPath, 'utf-8');
  try {
    const parsed = JSON.parse(raw);
    if (!parsed || !Array.isArray(parsed.tokens)) {
      return { tokens: [] };
    }
    return { tokens: parsed.tokens };
  } catch {
    return { tokens: [] };
  }
}

export async function saveTokenRegistry(registryPath, registry) {
  ensureDirForFile(registryPath);
  const payload = JSON.stringify({ tokens: registry.tokens || [] }, null, 2);
  await fs.writeFile(registryPath, payload);
}

export function upsertToken(registry, token) {
  const symbol = token.symbol.toUpperCase();
  const chainId = token.chainId;
  const tokens = registry.tokens || [];
  const idx = tokens.findIndex(t => t.symbol.toUpperCase() === symbol && t.chainId === chainId);

  const entry = {
    ...token,
    symbol,
    chainId
  };

  if (idx >= 0) {
    tokens[idx] = { ...tokens[idx], ...entry };
  } else {
    tokens.push(entry);
  }

  return { tokens };
}

export function removeToken(registry, symbol, chainId) {
  const upper = symbol.toUpperCase();
  const tokens = (registry.tokens || []).filter(
    t => !(t.symbol.toUpperCase() === upper && t.chainId === chainId)
  );
  return { tokens };
}

export default {
  defaultRegistryPath,
  loadTokenRegistry,
  saveTokenRegistry,
  upsertToken,
  removeToken
};
