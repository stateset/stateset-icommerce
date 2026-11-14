/**
 * Internal one-operation worker for ./index.js. It opens the identity store,
 * runs one operation, closes the store and exits. Never import the native
 * commerce binding here.
 *
 * The request arrives on stdin and the reply leaves on fd 3, both `node:v8`
 * serialized, so argument values (Date, BigInt, Buffer, undefined) reach
 * better-sqlite3 exactly as the caller passed them. The reply has its own pipe
 * so that stdout written by anything else in this process (a NODE_OPTIONS
 * preload, say) cannot corrupt it.
 */
import { readFileSync, writeSync } from 'node:fs';
import { format } from 'node:util';
import { deserialize, serialize } from 'node:v8';
import * as store from './store.js';

/** Keep in step with REPLY_FD in ./index.js. */
const REPLY_FD = 3;

// The store reports the JSON fallback with console.warn. Return those messages
// in the reply for the caller to re-emit; the caller shows the rest of stderr
// only when the call fails.
const warnings = [];
console.warn = (...args) => {
  warnings.push(format(...args));
};

function writeAll(fd, buffer) {
  let offset = 0;
  while (offset < buffer.length) {
    try {
      offset += writeSync(fd, buffer, offset, buffer.length - offset);
    } catch (error) {
      if (error?.code !== 'EAGAIN') throw error;
    }
  }
}

let reply;
try {
  const { operation, args } = deserialize(readFileSync(0));
  if (!store.STORE_OPERATIONS.includes(operation) || !Array.isArray(args)) {
    throw new Error('Invalid ERC-8004 operation');
  }
  reply = { ok: true, result: store[operation](...args), warnings };
} catch (error) {
  reply = {
    ok: false,
    error: {
      name: error?.name,
      message: error?.message ?? String(error),
      code: error?.code,
    },
    warnings,
  };
}
writeAll(REPLY_FD, serialize(reply));
