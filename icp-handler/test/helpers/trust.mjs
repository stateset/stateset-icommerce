// Real principal bindings for the handler's own test suite.
//
// Most of these tests used to post
//
//     signature: { alg: 'ed25519', kid: 'self', sig: 'deadbeef' }
//
// at a handler running permissive, where a binding naming an unregistered
// principal is waved through unverified. So the suite asserted that the
// handler accepts a delegation it never checked — it would have gone on
// passing if `checkDelegation` had been deleted outright, and every example a
// reader copied out of it was a forgery.
//
// This helper puts the handler in ICP_TRUST_MODE=enforce with an operator-
// registered principal key and an operator-admitted set of agent AIDs, and
// signs bindings with the SDK's own `signPrincipalBinding` — the function
// partners call. A binding that does not verify now fails the test.
//
// Usage. The handler reads its trust configuration once, at module
// evaluation, so `enforceTrust` MUST run before the server is imported —
// which means importing the server dynamically:
//
//     const trust = enforceTrust({ agents: [{ aid: agentAid, edHex }] });
//     const { server } = await import('../src/server.mjs');
//     ...
//     principal_binding: trust.binding({ agent: agentAid, verbs: ['purchase.create'] })

import {
  canonicalJson,
  generatePrincipalIdentity,
  signEd25519,
  signPrincipalBinding,
} from '../../../packages/icp-client/src/index.mjs';

/** The principal these tests delegate from unless they say otherwise. */
export const DEFAULT_PRINCIPAL = 'did:web:test.example';

/**
 * Configure the handler for enforced trust and return a binding signer.
 *
 * @param {{ agents?: { aid: string, edHex: string }[], principals?: string[] }} [opts]
 *   `agents` are admitted as signers (ICP_AGENT_KEYS_JSON); each entry in
 *   `principals` gets a freshly generated key registered under
 *   ICP_PRINCIPAL_KEYS_JSON.
 * @returns {{ binding: (params: object) => object, principals: Map<string, object> }}
 */
export function enforceTrust({ agents = [], principals = [DEFAULT_PRINCIPAL] } = {}) {
  const identities = new Map(principals.map((p) => [p, generatePrincipalIdentity()]));

  process.env.PORT ??= '0';
  process.env.ICP_TRUST_MODE = 'enforce';
  process.env.ICP_PRINCIPAL_KEYS_JSON = JSON.stringify(
    Object.fromEntries([...identities].map(([p, id]) => [p, id.ed25519_pubkey.toString('hex')])),
  );
  process.env.ICP_AGENT_KEYS_JSON = JSON.stringify(
    Object.fromEntries(agents.map((a) => [a.aid, a.edHex])),
  );

  /**
   * Sign a PrincipalBinding with the registered principal's key.
   *
   * @param {{ agent: string, verbs: string[], principal?: string,
   *   maxPerIntent?: { amount: string, currency: string },
   *   expiresAt?: Date|number|string, revocation?: string,
   *   authority?: object }} params
   *   `authority` carries fields the SDK helper does not model (today only
   *   `max_per_payout`); they are merged in and the binding is re-signed over
   *   the same canonical bytes the handler checks, so they are covered by the
   *   signature exactly like the rest of it.
   */
  function binding({ principal = DEFAULT_PRINCIPAL, authority, ...params }) {
    const identity = identities.get(principal);
    if (!identity) throw new Error(`principal ${principal} was not registered by enforceTrust()`);
    const signed = signPrincipalBinding(
      { principal, revocation: 'https://test.example/revoke', ...params },
      identity,
    );
    if (!authority) return signed;
    const { signature: _replaced, ...body } = signed;
    const extended = { ...body, authority: { ...body.authority, ...authority } };
    return {
      ...extended,
      signature: {
        alg: 'ed25519',
        kid: principal,
        sig: signEd25519(canonicalJson(extended), identity),
      },
    };
  }

  return { binding, principals: identities };
}
