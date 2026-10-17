#!/usr/bin/env node
// Run after a full CLI dependency install. These checks intentionally require
// the optional SDKs: an omitted integration supplies no compatibility evidence.
import assert from 'node:assert/strict';
import { readFileSync, existsSync } from 'node:fs';
import { createRequire } from 'node:module';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const require = createRequire(import.meta.url);
const manifest = JSON.parse(readFileSync(new URL('../package.json', import.meta.url), 'utf8'));

function packageVersion(entry, name) {
  let directory = path.dirname(entry);
  for (;;) {
    const candidate = path.join(directory, 'package.json');
    if (existsSync(candidate)) {
      const metadata = JSON.parse(readFileSync(candidate, 'utf8'));
      if (metadata.name === name) return metadata.version;
    }
    const parent = path.dirname(directory);
    assert.notEqual(parent, directory, `Cannot locate installed ${name} manifest`);
    directory = parent;
  }
}

// Resolve through Solana's actual dependency, then through Jayson's resolver.
// A patched top-level uuid does not protect an older nested copy.
const solanaRequire = createRequire(require.resolve('@solana/web3.js'));
const jaysonEntry = solanaRequire.resolve('jayson/lib/client/browser');
const jaysonRequire = createRequire(jaysonEntry);
const uuidVersion = packageVersion(jaysonRequire.resolve('uuid'), 'uuid');
assert.equal(
  uuidVersion,
  manifest.overrides.jayson.uuid,
  'Jayson must use the declared uuid override',
);
const Client = solanaRequire('jayson/lib/client/browser');
let request;
const client = new Client((message, callback) => {
  request = JSON.parse(message);
  callback(null, JSON.stringify({ jsonrpc: '2.0', id: request.id, result: 42 }));
});
const response = await new Promise((resolve, reject) => {
  client.request('getBalance', ['offline-fixture'], (error, result) =>
    error ? reject(error) : resolve(result),
  );
});
assert.match(request.id, /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i);
assert.equal(response.id, request.id);
assert.equal(response.result, 42);
assert.equal(request.method, 'getBalance');
console.log(`Jayson request/response and nested uuid ${uuidVersion}: passed`);

// Baileys uses parseBuffer for audio duration. Exercise its resolved parser
// against a complete PCM WAV fixture without connecting to WhatsApp.
const baileysRequire = createRequire(require.resolve('@whiskeysockets/baileys'));
const metadataEntry = baileysRequire.resolve('music-metadata');
const metadataVersion = packageVersion(metadataEntry, 'music-metadata');
const [major, minor] = metadataVersion.split('.').map(Number);
assert.ok(
  major > 11 || (major === 11 && minor >= 16),
  'music-metadata must include the 11.16 security fixes',
);
const { parseBuffer } = await import(pathToFileURL(metadataEntry).href);
const wav = Buffer.alloc(52);
wav.write('RIFF', 0);
wav.writeUInt32LE(44, 4);
wav.write('WAVEfmt ', 8);
wav.writeUInt32LE(16, 16);
wav.writeUInt16LE(1, 20); // PCM
wav.writeUInt16LE(1, 22); // mono
wav.writeUInt32LE(8000, 24);
wav.writeUInt32LE(16000, 28);
wav.writeUInt16LE(2, 32);
wav.writeUInt16LE(16, 34);
wav.write('data', 36);
wav.writeUInt32LE(8, 40); // four 16-bit samples
const metadata = await parseBuffer(wav, { mimeType: 'audio/wav', size: wav.length });
assert.equal(metadata.format.sampleRate, 8000);
assert.equal(metadata.format.numberOfChannels, 1);
assert.equal(metadata.format.numberOfSamples, 4);
assert.equal(metadata.format.duration, 4 / 8000);
console.log(`Baileys audio metadata ${metadataVersion}: passed`);
