/**
 * Treasury task pricing registry
 */

import fs from 'node:fs/promises';
import { existsSync, mkdirSync } from 'node:fs';
import { join, dirname, resolve } from 'node:path';
import { defaultTreasuryDir } from './store.js';

export function defaultPricingPath(cwd = process.cwd()) {
  return join(defaultTreasuryDir(cwd), 'pricing.json');
}

function ensureDirForFile(filePath) {
  const dir = dirname(resolve(filePath));
  mkdirSync(dir, { recursive: true });
}

export async function loadPricing(pricingPath = defaultPricingPath()) {
  if (!existsSync(pricingPath)) {
    return { rules: [] };
  }
  const raw = await fs.readFile(pricingPath, 'utf-8');
  try {
    const parsed = JSON.parse(raw);
    if (!parsed || !Array.isArray(parsed.rules)) {
      return { rules: [] };
    }
    return { rules: parsed.rules };
  } catch {
    return { rules: [] };
  }
}

export async function savePricing(pricingPath, pricing) {
  ensureDirForFile(pricingPath);
  const payload = JSON.stringify({ rules: pricing.rules || [] }, null, 2);
  await fs.writeFile(pricingPath, payload);
}

export function upsertPricingRule(pricing, rule) {
  const rules = pricing.rules || [];
  const keyTool = rule.tool.trim();
  const chainId = rule.chainId;
  const idx = rules.findIndex(r => r.tool === keyTool && r.chainId === chainId);

  const entry = {
    ...rule,
    tool: keyTool,
    chainId,
    tokenSymbol: rule.tokenSymbol.toUpperCase()
  };

  if (idx >= 0) {
    rules[idx] = { ...rules[idx], ...entry };
  } else {
    rules.push(entry);
  }

  return { rules };
}

export function removePricingRule(pricing, tool, chainId) {
  const keyTool = tool.trim();
  const rules = (pricing.rules || []).filter(r => !(r.tool === keyTool && r.chainId === chainId));
  return { rules };
}

export function getPricingRule(pricing, tool) {
  const keyTool = tool.trim();
  return (pricing.rules || []).find(r => r.tool === keyTool) || null;
}

export default {
  defaultPricingPath,
  loadPricing,
  savePricing,
  upsertPricingRule,
  removePricingRule,
  getPricingRule
};
