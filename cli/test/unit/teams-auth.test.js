import { before, test } from 'node:test';
import assert from 'node:assert/strict';
import { exportJWK, generateKeyPair, SignJWT } from 'jose';
import { createTeamsActivityVerifier } from '../../src/teams/auth.js';
import { startTeamsGateway } from '../../src/teams/gateway.js';

const APP_ID = 'test-teams-app';
const SERVICE_URL = 'https://smba.trafficmanager.net/amer/';
const METADATA_URL = 'https://login.botframework.com/v1/.well-known/openidconfiguration';
const KEYS_URL = 'https://login.botframework.com/v1/.well-known/keys';
const activity = { type: 'typing', channelId: 'msteams', serviceUrl: SERVICE_URL };

let privateKey;
let publicJwk;

before(async () => {
  const pair = await generateKeyPair('RS256', { extractable: true });
  privateKey = pair.privateKey;
  publicJwk = { ...(await exportJWK(pair.publicKey)), kid: 'test-key', alg: 'RS256', use: 'sig', endorsements: ['msteams'] };
});

function metadataFetch({ issuer = 'https://api.botframework.com', keys = [publicJwk], jwksUri = KEYS_URL } = {}) {
  const calls = [];
  const fetchImpl = async (url) => {
    calls.push(String(url));
    if (String(url) === METADATA_URL) {
      return Response.json({ issuer, jwks_uri: jwksUri, id_token_signing_alg_values_supported: ['RS256'] });
    }
    if (String(url) === KEYS_URL) return Response.json({ keys });
    throw new Error(`unexpected URL: ${url}`);
  };
  return { fetchImpl, calls };
}

async function signedToken({ audience = APP_ID, issuer = 'https://api.botframework.com', serviceUrl = SERVICE_URL, kid = 'test-key', expires = 3600, includeNbf = true, includeExp = true, key = privateKey } = {}) {
  const now = Math.floor(Date.now() / 1000);
  let jwt = new SignJWT({ serviceUrl })
    .setProtectedHeader({ alg: 'RS256', kid })
    .setIssuer(issuer)
    .setAudience(audience);
  if (includeNbf) jwt = jwt.setNotBefore(now - 60);
  if (includeExp) jwt = jwt.setExpirationTime(now + expires);
  return jwt.sign(key);
}

test('accepts only a signed, Teams-endorsed activity bound to the app and service URL', async () => {
  const { fetchImpl, calls } = metadataFetch();
  const verify = createTeamsActivityVerifier({ fetchImpl });
  const token = await signedToken();
  assert.equal(await verify(`Bearer ${token}`, activity, APP_ID), true);
  assert.deepEqual(calls, [METADATA_URL, KEYS_URL]);
  assert.equal(await verify(`Bearer ${token}`, activity, APP_ID), true);
  assert.deepEqual(calls, [METADATA_URL, KEYS_URL], 'signing keys are cached');

  assert.equal(await verify('', activity, APP_ID), false);
  assert.equal(await verify(`Basic ${token}`, activity, APP_ID), false);
  assert.equal(await verify(`Bearer ${token}`, { ...activity, channelId: 'webchat' }, APP_ID), false);
  assert.equal(await verify(`Bearer ${token}`, { ...activity, serviceUrl: 'https://attacker.example/' }, APP_ID), false);
  assert.equal(await verify(`Bearer ${await signedToken({ audience: 'other-bot' })}`, activity, APP_ID), false);
  assert.equal(await verify(`Bearer ${await signedToken({ issuer: 'https://attacker.example/' })}`, activity, APP_ID), false);
  assert.equal(await verify(`Bearer ${await signedToken({ expires: -3600 })}`, activity, APP_ID), false);
  assert.equal(await verify(`Bearer ${await signedToken({ includeNbf: false })}`, activity, APP_ID), false);
  assert.equal(await verify(`Bearer ${await signedToken({ includeExp: false })}`, activity, APP_ID), false);
  assert.equal(await verify(`Bearer ${await signedToken({ kid: 'unknown' })}`, activity, APP_ID), false);
  const otherKey = await generateKeyPair('RS256');
  assert.equal(await verify(`Bearer ${await signedToken({ key: otherKey.privateKey })}`, activity, APP_ID), false);
});

test('rejects unendorsed keys and unexpected metadata', async () => {
  const token = await signedToken();
  const unendorsed = metadataFetch({ keys: [{ ...publicJwk, endorsements: [] }] });
  await assert.rejects(
    createTeamsActivityVerifier({ fetchImpl: unendorsed.fetchImpl })(`Bearer ${token}`, activity, APP_ID),
    /no Teams-endorsed signing keys/,
  );
  const wrongIssuer = metadataFetch({ issuer: 'https://attacker.example/' });
  await assert.rejects(
    createTeamsActivityVerifier({ fetchImpl: wrongIssuer.fetchImpl })(`Bearer ${token}`, activity, APP_ID),
    /unexpected issuer/,
  );
  const untrustedKeys = metadataFetch({ jwksUri: 'https://attacker.example/keys' });
  await assert.rejects(
    createTeamsActivityVerifier({ fetchImpl: untrustedKeys.fetchImpl })(`Bearer ${token}`, activity, APP_ID),
    /untrusted signing-key URL/,
  );
});

test('webhook rejects forged activities before dispatch and accepts a verified activity', async () => {
  const originalFetch = globalThis.fetch;
  const originalAppId = process.env.TEAMS_APP_ID;
  const originalPassword = process.env.TEAMS_APP_PASSWORD;
  const { fetchImpl, calls } = metadataFetch();
  let gateway;
  try {
    globalThis.fetch = fetchImpl;
    process.env.TEAMS_APP_ID = APP_ID;
    process.env.TEAMS_APP_PASSWORD = 'test-secret';
    gateway = await startTeamsGateway({ webhookPort: 0 });
    const endpoint = `http://127.0.0.1:${gateway.port}/api/messages`;
    const post = (body, authorization) => originalFetch(endpoint, {
      method: 'POST',
      headers: { 'content-type': 'application/json', ...(authorization ? { authorization } : {}) },
      body: JSON.stringify(body),
    });

    assert.equal((await post(activity)).status, 403);
    assert.deepEqual(calls, [], 'anonymous requests never fetch signing keys or reach dispatch');
    assert.equal((await post(activity, 'Bearer forged')).status, 403);
    const token = await signedToken();
    assert.equal((await post({ ...activity, serviceUrl: 'https://attacker.example/' }, `Bearer ${token}`)).status, 403);
    assert.equal((await post(activity, `Bearer ${token}`)).status, 200);
    assert.equal((await post({ ...activity, padding: 'x'.repeat(1024 * 1024) }, `Bearer ${token}`)).status, 413);
  } finally {
    if (gateway) await gateway.shutdown();
    globalThis.fetch = originalFetch;
    if (originalAppId === undefined) delete process.env.TEAMS_APP_ID;
    else process.env.TEAMS_APP_ID = originalAppId;
    if (originalPassword === undefined) delete process.env.TEAMS_APP_PASSWORD;
    else process.env.TEAMS_APP_PASSWORD = originalPassword;
  }
});
