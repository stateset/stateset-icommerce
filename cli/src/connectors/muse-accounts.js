import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { collectApiKeys } from '../mcp/http-api-keys.js';

/** Operator-owned configuration. Key files resolve relative to the accounts file. */
export function loadMuseAccounts(path) {
  let config;
  try {
    config = JSON.parse(readFileSync(path, 'utf8'));
  } catch {
    throw new Error('Cannot read Muse accounts configuration JSON.');
  }
  if (
    !config ||
    Object.keys(config).some((key) => key !== 'accounts') ||
    !Array.isArray(config.accounts) ||
    !config.accounts.length
  ) {
    throw new Error('Muse accounts configuration requires a nonempty accounts array.');
  }
  return config.accounts.map((account) => {
    if (
      !account ||
      typeof account !== 'object' ||
      Object.keys(account).some(
        (key) => !['id', 'apiKeyFile', 'tools', 'allowApply'].includes(key),
      ) ||
      typeof account.apiKeyFile !== 'string' ||
      !account.apiKeyFile.trim()
    ) {
      throw new Error('Each Muse account requires apiKeyFile and only supported fields.');
    }
    let apiKeys;
    try {
      apiKeys = collectApiKeys({ file: resolve(dirname(path), account.apiKeyFile) });
    } catch {
      throw new Error('Cannot load Muse account API keys. Check key files and minimum key length.');
    }
    return { id: account.id, apiKeys, tools: account.tools, allowApply: account.allowApply };
  });
}
