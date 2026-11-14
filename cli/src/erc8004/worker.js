/**
 * Internal one-operation worker for ./index.js. It opens the identity store,
 * runs one operation, closes the store and exits. Never import the native
 * commerce binding here.
 *
 * Request on stdin and reply on stdout are `node:v8` serialized, so argument
 * values (Date, BigInt, Buffer, undefined) reach better-sqlite3 exactly as the
 * caller passed them.
 */
import { readFileSync } from 'node:fs';
import { deserialize, serialize } from 'node:v8';
import * as store from './store.js';

let reply;
try {
  const { operation, args } = deserialize(readFileSync(0));
  if (!store.STORE_OPERATIONS.includes(operation) || !Array.isArray(args)) {
    throw new Error('Invalid ERC-8004 operation');
  }
  reply = { ok: true, result: store[operation](...args) };
} catch (error) {
  reply = {
    ok: false,
    error: {
      name: error?.name,
      message: error?.message ?? String(error),
      code: error?.code,
    },
  };
}
process.stdout.write(serialize(reply));
