import crypto from 'node:crypto';
import os from 'node:os';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

export function isMain(importMeta) {
  if (!process.argv[1]) {
    return false;
  }
  return pathToFileURL(path.resolve(process.argv[1])).href === importMeta.url;
}

export function printSection(title) {
  console.log(`\n=== ${title} ===`);
}

export function printKeyValue(label, value) {
  console.log(`${String(label).padEnd(22)} ${value}`);
}

export function shortHex(value, chars = 8) {
  const normalized = String(value || '');
  if (normalized.length <= chars * 2) {
    return normalized;
  }
  return `${normalized.slice(0, chars)}...${normalized.slice(-chars)}`;
}

export function toHex(value) {
  return `0x${Buffer.from(value).toString('hex')}`;
}

export function walletAddressFromPublicKey(publicKey) {
  const digest = crypto.createHash('sha256').update(Buffer.from(publicKey)).digest('hex');
  return `0x${digest.slice(-40)}`;
}

export function createTempPath(name, extension = 'json') {
  return path.join(
    os.tmpdir(),
    `${name}-${process.pid}-${Date.now()}-${crypto.randomUUID()}.${extension}`,
  );
}

export function createDemoTxHash() {
  return `0x${crypto.randomBytes(32).toString('hex')}`;
}

export async function closeServer(server) {
  if (!server?.listening) {
    return;
  }
  await new Promise((resolve, reject) => {
    server.close((error) => {
      if (error) {
        reject(error);
        return;
      }
      resolve();
    });
  });
}

export function formatUsdc(amountSmallest) {
  return `${(Number(amountSmallest) / 1_000_000).toFixed(6)} USDC`;
}

