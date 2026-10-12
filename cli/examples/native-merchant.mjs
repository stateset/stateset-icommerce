#!/usr/bin/env node
// Source-tree HTTP merchant. Payment and dispatch recording are opt-in host integrations.
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { configureStorage } from '../../icp-handler/src/state.mjs';
import { configureNativeMerchant } from '../../icp-handler/src/native-http.mjs';

const index = process.argv.indexOf('--config');
const configPath = index >= 0 ? process.argv[index + 1] : null;
if (!configPath || configPath.startsWith('--')) {
  throw new Error('supply --config PATH to an operator-owned native merchant module');
}
if (!process.env.ICP_MERCHANT_KEY_FILE || !process.env.ICP_MERCHANT_AID)
  throw new Error('operator-owned merchant key file and AID are required');
const { default: configure } = await import(pathToFileURL(resolve(configPath)).href);
const resources = await configure();
try {
  configureStorage(resources.store);
  configureNativeMerchant({ ...resources.native, allowApply: process.argv.includes('--apply') });
  const { server } = await import('../../icp-handler/src/server.mjs');
  server.on('close', () => resources.close?.());
  for (const signal of ['SIGINT', 'SIGTERM']) process.once(signal, () => server.close());
} catch (error) {
  await resources.close?.();
  throw error;
}
