/**
 * Interior-node forgery against the raw-leaf proof tools and commands.
 *
 * `verify_inclusion_proof`, `verify_chain_anchor` and `verify_receipt`
 * (tools/proofs.js), and `proofs verify-proof` / `verify-anchor` /
 * `verify-receipt` (commands/proofs.js), take a leaf hash on trust (for a
 * receipt: a legacy-v0 bundle's). Without a tree size the caller knows
 * independently of the proof, nothing ties the path to a leaf: an interior
 * node's hash plus the rest of the path above it walks to the root just as a
 * leaf does.
 *
 * So `valid` and `anchored` are true only for a caller-supplied `leafCount`
 * (`pathBound`) and a path from a leaf position of a tree of that size.
 * `rootReached` keeps the plain hash-chain result. A ves-v1 receipt bundle is
 * the exception: its leaf is recomputed from the event in the VES_LEAF_V1
 * domain, which no interior node (VES_NODE_V1) can match, so `leafCount` is
 * optional there.
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import * as cryptoMod from '../../src/sync/crypto.js';
import { createProofGenerator } from '../../src/sync/proof-generator.js';
import { UNBOUND_REASON } from '../../src/sync/raw-leaf-inclusion.js';
import { proofTools } from '../../src/tools/proofs.js';
import { execute as executeProofsCommand } from '../../src/commands/proofs.js';

const { computeNodeHash } = cryptoMod;
const pg = createProofGenerator(cryptoMod);

const hex = (buf) => buf.toString('hex');
const toolNamed = (name) => proofTools.find((t) => t.name === name);
const callTool = (name, params) => toolNamed(name).handler({ params });
const runCommand = (action, args, jsonOutput = true) =>
  executeProofsCommand(action, args, { jsonOutput, output: () => {} });

const TX = '0xabc123';

// A real 4-leaf batch, proved with the module's own generator.
const EVENTS = [0, 1, 2, 3].map((i) => ({
  id: `evt-${i}`,
  eventSigningHash: hex(Buffer.alloc(32, i + 1)),
}));
const REAL = pg.generateInclusionProof('evt-0', EVENTS);
// evt-0's path is [L1 (right), n23 (right)], so n01 = H(L0, L1) is an
// interior node, and [n23] is the path above it.
const N01 = computeNodeHash(Buffer.from(REAL.leaf, 'hex'), Buffer.from(REAL.proof[0].hash, 'hex'));
const FORGED_INTERIOR = {
  leafHash: hex(N01),
  proof: JSON.stringify(REAL.proof.slice(1)),
  expectedRoot: REAL.root,
};
// The root itself, with an empty path.
const FORGED_ROOT = { leafHash: REAL.root, proof: '[]', expectedRoot: REAL.root };
const GENUINE = { leafHash: REAL.leaf, proof: JSON.stringify(REAL.proof), expectedRoot: REAL.root };

const FORGERIES = [
  ['an interior node one level up', FORGED_INTERIOR],
  ['the root itself', FORGED_ROOT],
];

describe('the forgery is real', () => {
  it('the generator built a 4-leaf tree whose interior node walks to its root', () => {
    assert.equal(REAL.leafCount, 4);
    assert.equal(REAL.proof.length, 2);
    const reached = computeNodeHash(N01, Buffer.from(REAL.proof[1].hash, 'hex'));
    assert.equal(hex(reached), REAL.root);
  });
});

describe('verify_inclusion_proof: valid only for a caller-bound path', () => {
  for (const [label, forged] of FORGERIES) {
    it(`${label}, no leafCount: the chain reaches the root, but it is not valid`, async () => {
      const verdict = await callTool('verify_inclusion_proof', forged);
      assert.equal(verdict.success, true);
      assert.equal(verdict.valid, false);
      assert.equal(verdict.rootReached, true);
      assert.equal(verdict.pathBound, false);
      assert.equal(verdict.leafCountSource, null);
      assert.equal(verdict.reason, UNBOUND_REASON);
      assert.match(verdict.detail, /^Not valid/);
      assert.match(verdict.detail, /not bound to a tree size/);
      assert.match(verdict.detail, /some node/);
      assert.match(verdict.detail, /leafCount/);
    });

    it(`${label}, with leafCount: refused, though the chain reaches the root`, async () => {
      for (const leafCount of [4, '4']) {
        const verdict = await callTool('verify_inclusion_proof', { ...forged, leafCount });
        assert.equal(verdict.valid, false, `leafCount ${JSON.stringify(leafCount)}`);
        assert.equal(verdict.rootReached, true);
        assert.equal(verdict.pathBound, true);
        assert.equal(verdict.leafCountSource, 'caller');
        assert.match(verdict.detail, /not from a leaf position of a 4-leaf tree/);
        assert.doesNotMatch(verdict.detail, /^Valid/);
      }
    });
  }

  it('a genuine leaf with the true leafCount (number or canonical string) is valid', async () => {
    for (const leafCount of [4, '4']) {
      const verdict = await callTool('verify_inclusion_proof', { ...GENUINE, leafCount });
      assert.equal(verdict.valid, true);
      assert.equal(verdict.rootReached, true);
      assert.equal(verdict.pathBound, true);
      assert.equal(verdict.leafCountSource, 'caller');
      assert.equal(verdict.reason, undefined);
      assert.match(verdict.detail, /^Valid/);
      assert.match(verdict.detail, /4-leaf tree/);
    }
  });

  it('a genuine leaf without leafCount reaches the root but is not valid', async () => {
    // The verifier cannot tell it from the forgery: nothing places the hash.
    const verdict = await callTool('verify_inclusion_proof', GENUINE);
    assert.equal(verdict.valid, false);
    assert.equal(verdict.rootReached, true);
    assert.equal(verdict.pathBound, false);
    assert.equal(verdict.reason, UNBOUND_REASON);
  });

  it('leafIndex must be the position the path spells', async () => {
    for (const leafIndex of [0, '0']) {
      const verdict = await callTool('verify_inclusion_proof', {
        ...GENUINE,
        leafCount: 4,
        leafIndex,
      });
      assert.equal(verdict.valid, true, `leafIndex ${JSON.stringify(leafIndex)}`);
    }
    for (const leafIndex of [1, '1', 3, '00', -1]) {
      const verdict = await callTool('verify_inclusion_proof', {
        ...GENUINE,
        leafCount: 4,
        leafIndex,
      });
      assert.equal(verdict.valid, false, `leafIndex ${JSON.stringify(leafIndex)}`);
      assert.equal(verdict.rootReached, true);
    }
    // Without leafCount a wrong leafIndex is named as the reason.
    const wrongIndex = await callTool('verify_inclusion_proof', { ...GENUINE, leafIndex: 2 });
    assert.equal(wrongIndex.valid, false);
    assert.equal(wrongIndex.reason, undefined);
    assert.match(wrongIndex.detail, /leafIndex/);
  });

  it('a wrong or unusable caller leafCount is invalid, not trusted', async () => {
    for (const leafCount of [2, 8, 0, '04', 2 ** 32, '4294967296']) {
      const verdict = await callTool('verify_inclusion_proof', { ...GENUINE, leafCount });
      assert.equal(verdict.valid, false, `leafCount ${JSON.stringify(leafCount)}`);
      assert.equal(verdict.leafCountSource, 'caller');
    }
  });

  it('a proof that does not reach the root is invalid and says so', async () => {
    for (const leafCount of [undefined, 4]) {
      const verdict = await callTool('verify_inclusion_proof', {
        ...GENUINE,
        expectedRoot: hex(Buffer.alloc(32, 0xee)),
        leafCount,
      });
      assert.equal(verdict.valid, false);
      assert.equal(verdict.rootReached, false);
      assert.match(verdict.detail, /does not reach the expected root/);
    }
  });
});

describe('verify_chain_anchor: anchored only for a caller-bound path', () => {
  for (const [label, forged] of FORGERIES) {
    it(`${label}, no leafCount: not anchored, though the chain reaches the root`, async () => {
      const verdict = await callTool('verify_chain_anchor', { ...forged, anchorTxHash: TX });
      assert.equal(verdict.success, true);
      assert.equal(verdict.anchored, false);
      assert.equal(verdict.valid, false);
      assert.equal(verdict.rootReached, true);
      assert.equal(verdict.anchorTxHash, TX);
      assert.equal(verdict.pathBound, false);
      assert.equal(verdict.leafCountSource, null);
      assert.equal(verdict.reason, UNBOUND_REASON);
      assert.doesNotMatch(verdict.detail, /Event proof verified/);
      assert.match(verdict.detail, /^Not anchored/);
      assert.match(verdict.detail, /not bound to a tree size/);
      assert.match(verdict.detail, /leafCount/);
    });

    it(`${label}, with leafCount: refused`, async () => {
      const verdict = await callTool('verify_chain_anchor', {
        ...forged,
        anchorTxHash: TX,
        leafCount: 4,
      });
      assert.equal(verdict.anchored, false);
      assert.equal(verdict.valid, false);
      assert.equal(verdict.rootReached, true);
      assert.equal(verdict.pathBound, true);
      assert.equal(verdict.leafCountSource, 'caller');
      assert.doesNotMatch(verdict.detail, /Event proof verified/);
      assert.match(verdict.detail, /not from a leaf position of a 4-leaf tree/);
    });
  }

  it('a genuine leaf with the true leafCount is anchored and says so', async () => {
    const verdict = await callTool('verify_chain_anchor', {
      ...GENUINE,
      anchorTxHash: TX,
      leafCount: '4',
      leafIndex: 0,
    });
    assert.equal(verdict.anchored, true);
    assert.equal(verdict.valid, true);
    assert.equal(verdict.rootReached, true);
    assert.equal(verdict.pathBound, true);
    assert.equal(verdict.leafCountSource, 'caller');
    assert.match(
      verdict.detail,
      new RegExp(`Event proof verified against on-chain root in tx ${TX}`),
    );
  });

  it('a genuine leaf without leafCount is not anchored', async () => {
    const verdict = await callTool('verify_chain_anchor', { ...GENUINE, anchorTxHash: TX });
    assert.equal(verdict.anchored, false);
    assert.equal(verdict.valid, false);
    assert.equal(verdict.rootReached, true);
    assert.equal(verdict.pathBound, false);
    assert.doesNotMatch(verdict.detail, /Event proof verified/);
  });

  it('a proof that does not reach the root is not anchored', async () => {
    const verdict = await callTool('verify_chain_anchor', {
      ...GENUINE,
      expectedRoot: hex(Buffer.alloc(32, 0xee)),
      anchorTxHash: TX,
      leafCount: 4,
    });
    assert.equal(verdict.anchored, false);
    assert.equal(verdict.rootReached, false);
    assert.match(verdict.detail, /does not match the expected on-chain root/);
  });
});

describe('proof tools: inputs and descriptions', () => {
  for (const name of ['verify_inclusion_proof', 'verify_chain_anchor']) {
    it(`${name} declares optional leafCount and leafIndex inputs`, () => {
      const { inputSchema, description } = toolNamed(name);
      const { leafCount, leafIndex } = inputSchema;
      assert.ok(leafCount, 'leafCount input');
      assert.ok(leafIndex, 'leafIndex input');
      for (const ok of [undefined, 1, 4, '4', 4294967295]) {
        assert.equal(leafCount.safeParse(ok).success, true, `leafCount ${JSON.stringify(ok)}`);
      }
      for (const bad of [0, -1, 1.5, '0', '04', ' 4', '4.0', 'abc', 4294967296]) {
        assert.equal(leafCount.safeParse(bad).success, false, `leafCount ${JSON.stringify(bad)}`);
      }
      for (const ok of [undefined, 0, 3, '0', '3']) {
        assert.equal(leafIndex.safeParse(ok).success, true, `leafIndex ${JSON.stringify(ok)}`);
      }
      for (const bad of [-1, 1.5, '01', '-1', 'x']) {
        assert.equal(leafIndex.safeParse(bad).success, false, `leafIndex ${JSON.stringify(bad)}`);
      }
      assert.match(description, /leafCount/);
      assert.match(description, /some node/);
      assert.match(description, /rootReached/);
    });
  }

  it('the descriptions say the success boolean needs leafCount', () => {
    assert.match(
      toolNamed('verify_inclusion_proof').description,
      /valid is true only when leafCount/,
    );
    assert.match(
      toolNamed('verify_chain_anchor').description,
      /anchored \(and valid\) are true only when leafCount/,
    );
    const receipt = toolNamed('verify_receipt').description;
    assert.match(receipt, /legacy-v0 bundle is valid only with leafCount/);
    assert.match(receipt, /ves-v1/);
    assert.match(receipt, /rootReached/);
  });
});

describe('proofs verify-proof / verify-anchor commands: --leaf-count', () => {
  const forgedArgs = [FORGED_INTERIOR.leafHash, FORGED_INTERIOR.proof, REAL.root];
  const genuineArgs = [GENUINE.leafHash, GENUINE.proof, REAL.root];

  it('verify-proof: valid only with --leaf-count, and never for the forgery', async () => {
    const unbound = await runCommand('verify-proof', forgedArgs);
    assert.equal(unbound.valid, false);
    assert.equal(unbound.rootReached, true);
    assert.equal(unbound.pathBound, false);
    assert.match(unbound.detail, /not bound to a tree size/);
    assert.match(unbound.detail, /--leaf-count/);

    for (const flag of [['--leaf-count', '4'], ['--leaf-count=4']]) {
      const bound = await runCommand('verify-proof', [...forgedArgs, ...flag]);
      assert.equal(bound.valid, false, flag.join(' '));
      assert.equal(bound.pathBound, true);
      assert.equal(bound.leafCountSource, 'caller');
    }
    const genuineUnbound = await runCommand('verify-proof', [...genuineArgs, 'evt-0']);
    assert.equal(genuineUnbound.valid, false);
    assert.equal(genuineUnbound.rootReached, true);
    const genuine = await runCommand('verify-proof', [
      '--leaf-count',
      '4',
      ...genuineArgs,
      'evt-0',
    ]);
    assert.equal(genuine.valid, true);
    assert.equal(genuine.pathBound, true);
    assert.equal(genuine.eventId, 'evt-0');
  });

  it('verify-anchor: anchored only with --leaf-count, and never for the forgery', async () => {
    const unbound = await runCommand('verify-anchor', [...forgedArgs, TX]);
    assert.equal(unbound.anchored, false);
    assert.equal(unbound.valid, false);
    assert.equal(unbound.rootReached, true);
    assert.equal(unbound.pathBound, false);
    assert.doesNotMatch(unbound.detail, /Event proof verified/);

    const bound = await runCommand('verify-anchor', [...forgedArgs, TX, '--leaf-count', '4']);
    assert.equal(bound.anchored, false);
    assert.equal(bound.pathBound, true);

    const genuineUnbound = await runCommand('verify-anchor', [...genuineArgs, TX]);
    assert.equal(genuineUnbound.anchored, false);
    assert.equal(genuineUnbound.rootReached, true);

    const genuine = await runCommand('verify-anchor', [
      ...genuineArgs,
      TX,
      '--leaf-count',
      '4',
      '--leaf-index',
      '0',
    ]);
    assert.equal(genuine.anchored, true);
    assert.equal(genuine.valid, true);
    assert.equal(genuine.pathBound, true);
    assert.match(genuine.detail, /Event proof verified/);

    const wrongIndex = await runCommand('verify-anchor', [
      ...genuineArgs,
      TX,
      '--leaf-count=4',
      '--leaf-index=2',
    ]);
    assert.equal(wrongIndex.anchored, false);
  });

  it('the text line says "verified" / "valid" only for a bound path', async () => {
    for (const args of [
      [...forgedArgs, TX],
      [...genuineArgs, TX],
    ]) {
      const unbound = await runCommand('verify-anchor', args, false);
      assert.match(unbound.formatted, /^Chain anchor: unbound/);
      assert.doesNotMatch(unbound.formatted, /verified/);
      assert.match(unbound.formatted, /--leaf-count/);
    }
    const bound = await runCommand(
      'verify-anchor',
      [...genuineArgs, TX, '--leaf-count', '4'],
      false,
    );
    assert.equal(bound.formatted, 'Chain anchor: verified');
    const forgedBound = await runCommand(
      'verify-anchor',
      [...forgedArgs, TX, '--leaf-count', '4'],
      false,
    );
    assert.equal(forgedBound.formatted, 'Chain anchor: invalid');

    for (const args of [forgedArgs, genuineArgs]) {
      const proofUnbound = await runCommand('verify-proof', args, false);
      assert.match(proofUnbound.formatted, /^Inclusion proof: unbound/);
      assert.doesNotMatch(proofUnbound.formatted, /: valid/);
      assert.match(proofUnbound.formatted, /--leaf-count/);
    }
    const proofBound = await runCommand(
      'verify-proof',
      [...genuineArgs, '--leaf-count', '4'],
      false,
    );
    assert.equal(proofBound.formatted, 'Inclusion proof: valid');
    const proofForgedBound = await runCommand(
      'verify-proof',
      [...forgedArgs, '--leaf-count', '4'],
      false,
    );
    assert.equal(proofForgedBound.formatted, 'Inclusion proof: invalid');
    const missed = await runCommand(
      'verify-proof',
      [GENUINE.leafHash, GENUINE.proof, hex(Buffer.alloc(32, 0xee))],
      false,
    );
    assert.equal(missed.formatted, 'Inclusion proof: invalid');
  });

  it('refuses a missing, repeated or unusable --leaf-count / --leaf-index with a usage error', async () => {
    for (const action of ['verify-proof', 'verify-anchor']) {
      const base = action === 'verify-anchor' ? [...genuineArgs, TX] : genuineArgs;
      await assert.rejects(runCommand(action, [...base, '--leaf-count']), /Usage/);
      await assert.rejects(
        runCommand(action, [...base, '--leaf-count', '4', '--leaf-count=4']),
        /Usage/,
      );
      for (const value of ['0', '-1', '1.5', '04', 'abc', '', '4294967296']) {
        await assert.rejects(
          runCommand(action, [...base, `--leaf-count=${value}`]),
          /--leaf-count must be/,
          `${action} --leaf-count=${value}`,
        );
      }
      for (const value of ['-1', '1.5', '01', 'abc', '']) {
        await assert.rejects(
          runCommand(action, [...base, '--leaf-count=4', `--leaf-index=${value}`]),
          /--leaf-index must be/,
          `${action} --leaf-index=${value}`,
        );
      }
    }
  });
});

// ---------------------------------------------------------------------------
// verify_receipt / proofs verify-receipt
//
// A legacy-v0 bundle's leaf binds a batch position the bundle does not carry,
// so it is not recomputed (leafBound: false) and the bundle's leafHash is
// taken on trust, exactly like the raw leaf above: such a bundle is valid only
// with a caller leafCount. The bundle's own leafCount is only a sanity bound,
// and whoever wrote the bundle can leave it out.
//
// A ves-v1 bundle's leaf is recomputed from its event in the VES_LEAF_V1
// domain, and every interior node is hashed in VES_NODE_V1, so an interior
// node can never pass the leaf check: leafCount is optional there.
// ---------------------------------------------------------------------------

const checkNamed = (verdict, name) => verdict.checks.find((c) => c.check === name);

/** A bundle whose leafHash is n01 = H(L0, L1), with the path above it. */
function forgeInterior(genuine) {
  const { leafCount: _dropped, ...unsized } = genuine;
  const n01 = computeNodeHash(
    Buffer.from(genuine.leafHash, 'hex'),
    Buffer.from(genuine.inclusionProof[0].hash, 'hex'),
  );
  return { ...unsized, leafHash: hex(n01), inclusionProof: genuine.inclusionProof.slice(1) };
}

describe('verify_receipt / proofs verify-receipt: legacy-v0 needs a caller leafCount', () => {
  const genuine = pg.generateReceiptBundle(EVENTS[0], EVENTS);
  // The bundle's own leafCount is dropped: the forger writes the bundle.
  const forged = forgeInterior(genuine);

  it('the bundles are what they claim', () => {
    assert.equal(genuine.format, 'legacy-v0');
    assert.equal(genuine.leafCount, 4);
    assert.equal(forged.leafHash, hex(N01));
    assert.equal(pg.verifyReceiptBundle(forged).leafBound, false);
  });

  it('forged interior node, no leafCount: not valid, though the chain reaches the root', async () => {
    const verdict = await callTool('verify_receipt', { receiptBundle: JSON.stringify(forged) });
    assert.equal(verdict.success, true);
    assert.equal(verdict.valid, false);
    assert.equal(verdict.rootReached, true);
    assert.equal(verdict.pathBound, false);
    assert.equal(verdict.leafBound, false);
    assert.equal(verdict.leafCountSource, null);
    assert.equal(checkNamed(verdict, 'inclusion_proof').passed, true);
    assert.equal(checkNamed(verdict, 'path_bound').passed, false);
    assert.match(verdict.detail, /^Not valid/);
    assert.match(verdict.detail, /some node/);
    assert.match(verdict.detail, /leafCount/);
  });

  it('genuine bundle, no leafCount: not valid either (nothing places the leaf)', async () => {
    const verdict = await callTool('verify_receipt', { receiptBundle: JSON.stringify(genuine) });
    assert.equal(verdict.valid, false);
    assert.equal(verdict.rootReached, true);
    assert.equal(verdict.pathBound, false);
    // The bundle's own leafCount was applied, but only as a sanity bound.
    assert.equal(verdict.leafCountSource, 'bundle');
    assert.equal(checkNamed(verdict, 'path_bound').passed, false);
  });

  it("the caller's leafCount refuses the forgery and makes the genuine bundle valid", async () => {
    for (const leafCount of [4, '4']) {
      const refused = await callTool('verify_receipt', {
        receiptBundle: JSON.stringify(forged),
        leafCount,
      });
      assert.equal(refused.valid, false, `leafCount ${JSON.stringify(leafCount)}`);
      assert.equal(refused.rootReached, true);
      assert.equal(refused.pathBound, true);
      assert.equal(refused.leafCountSource, 'caller');
      assert.equal(checkNamed(refused, 'inclusion_proof').passed, false);

      const bound = await callTool('verify_receipt', {
        receiptBundle: JSON.stringify(genuine),
        leafCount,
      });
      assert.equal(bound.valid, true);
      assert.equal(bound.rootReached, true);
      assert.equal(bound.pathBound, true);
      assert.equal(bound.leafCountSource, 'caller');
      assert.equal(checkNamed(bound, 'path_bound').passed, true);
      assert.match(bound.detail, /^Valid/);
      assert.match(bound.detail, /taken as given/);
    }
    // A bundle stating a different size than the caller's is refused.
    const disagreeing = await callTool('verify_receipt', {
      receiptBundle: JSON.stringify(genuine),
      leafCount: 8,
    });
    assert.equal(disagreeing.valid, false);
  });

  it('a bundle with no format is read as legacy-v0 and needs leafCount too', async () => {
    const { format: _format, ...unformatted } = genuine;
    const unbound = await callTool('verify_receipt', {
      receiptBundle: JSON.stringify(unformatted),
    });
    assert.equal(unbound.format, 'legacy-v0');
    assert.equal(unbound.valid, false);
    const bound = await callTool('verify_receipt', {
      receiptBundle: JSON.stringify(unformatted),
      leafCount: 4,
    });
    assert.equal(bound.valid, true);
  });

  it('verify_receipt declares an optional leafCount input and explains it', () => {
    const { inputSchema, description } = toolNamed('verify_receipt');
    assert.ok(inputSchema.leafCount);
    assert.equal(inputSchema.leafCount.safeParse(undefined).success, true);
    assert.equal(inputSchema.leafCount.safeParse('4').success, true);
    assert.equal(inputSchema.leafCount.safeParse(0).success, false);
    assert.match(description, /leafCount/);
  });

  it('proofs verify-receipt: valid only with --leaf-count, and never for the forgery', async () => {
    for (const bundle of [forged, genuine]) {
      const unbound = await runCommand('verify-receipt', [JSON.stringify(bundle)]);
      assert.equal(unbound.valid, false);
      assert.equal(unbound.rootReached, true);
      assert.equal(unbound.pathBound, false);
      assert.match(unbound.detail, /--leaf-count/);
    }

    for (const args of [
      [JSON.stringify(forged), '--leaf-count', '4'],
      ['--leaf-count=4', JSON.stringify(forged)],
    ]) {
      const refused = await runCommand('verify-receipt', args);
      assert.equal(refused.valid, false);
      assert.equal(refused.pathBound, true);
    }
    const bound = await runCommand('verify-receipt', [
      JSON.stringify(genuine),
      '--leaf-count',
      '4',
    ]);
    assert.equal(bound.valid, true);
    assert.equal(bound.pathBound, true);
    assert.equal(bound.leafCountSource, 'caller');

    await assert.rejects(
      runCommand('verify-receipt', [JSON.stringify(genuine), '--leaf-count']),
      /Usage/,
    );
    await assert.rejects(
      runCommand('verify-receipt', [JSON.stringify(genuine), '--leaf-count=04']),
      /--leaf-count must be/,
    );
  });

  it('proofs verify-receipt: the text line says "valid" only for a bound path', async () => {
    for (const bundle of [forged, genuine]) {
      const unbound = await runCommand('verify-receipt', [JSON.stringify(bundle)], false);
      assert.match(unbound.formatted, /^Receipt verification: unbound/);
      assert.doesNotMatch(unbound.formatted, /: valid/);
      assert.match(unbound.formatted, /--leaf-count/);
    }
    const bound = await runCommand(
      'verify-receipt',
      [JSON.stringify(genuine), '--leaf-count', '4'],
      false,
    );
    assert.equal(bound.formatted, 'Receipt verification: valid');
    const forgedBound = await runCommand(
      'verify-receipt',
      [JSON.stringify(forged), '--leaf-count', '4'],
      false,
    );
    assert.equal(forgedBound.formatted, 'Receipt verification: invalid');
  });
});

describe('verify_receipt / proofs verify-receipt: ves-v1 binds the leaf without leafCount', () => {
  const TENANT = '550e8400-e29b-41d4-a716-446655440002';
  const STORE = '550e8400-e29b-41d4-a716-446655440003';
  const SPEC_EVENTS = [0, 1, 2, 3].map((i) => ({
    id: `spec-${i}`,
    tenantId: TENANT,
    storeId: STORE,
    sequenceNumber: i + 1,
    eventSigningHash: hex(Buffer.alloc(32, 0x40 + i)),
    agentSignature: hex(Buffer.alloc(64, 0x80 + i)),
  }));
  const genuine = pg.generateReceiptBundle(SPEC_EVENTS[0], SPEC_EVENTS);
  const forged = forgeInterior(genuine);

  it('the bundles are what they claim', () => {
    assert.equal(genuine.format, 'ves-v1');
    assert.equal(genuine.leafCount, 4);
    assert.equal(genuine.inclusionProof.length, 2);
  });

  it('genuine bundle, no leafCount: valid, leaf-bound, path not caller-bound', async () => {
    const { leafCount: _dropped, ...unsized } = genuine;
    for (const [bundle, source] of [
      [genuine, 'bundle'],
      [unsized, null],
    ]) {
      const verdict = await callTool('verify_receipt', { receiptBundle: JSON.stringify(bundle) });
      assert.equal(verdict.valid, true, `leafCountSource ${source}`);
      assert.equal(verdict.leafBound, true);
      assert.equal(verdict.rootReached, true);
      assert.equal(verdict.pathBound, false);
      assert.equal(verdict.leafCountSource, source);
      // No path_bound check: the leaf_hash check already rules out a node.
      assert.equal(checkNamed(verdict, 'path_bound'), undefined);
      assert.equal(checkNamed(verdict, 'leaf_hash').passed, true);
      assert.match(verdict.detail, /^Valid/);
      assert.match(verdict.detail, /cannot be an interior node/);
    }
  });

  it('genuine bundle with the true leafCount: valid and path-bound', async () => {
    const verdict = await callTool('verify_receipt', {
      receiptBundle: JSON.stringify(genuine),
      leafCount: 4,
    });
    assert.equal(verdict.valid, true);
    assert.equal(verdict.leafBound, true);
    assert.equal(verdict.pathBound, true);
    assert.match(verdict.detail, /4-leaf tree/);
    const wrong = await callTool('verify_receipt', {
      receiptBundle: JSON.stringify(genuine),
      leafCount: 8,
    });
    assert.equal(wrong.valid, false);
  });

  it('forged interior node: refused by the leaf check, with or without leafCount', async () => {
    for (const leafCount of [undefined, 4]) {
      const verdict = await callTool('verify_receipt', {
        receiptBundle: JSON.stringify(forged),
        leafCount,
      });
      assert.equal(verdict.valid, false, `leafCount ${leafCount}`);
      assert.equal(verdict.leafBound, false);
      // The hash chain does reach the root: only the leaf domain stops it.
      assert.equal(verdict.rootReached, true);
      assert.equal(checkNamed(verdict, 'leaf_hash').passed, false);
    }
  });

  it('proofs verify-receipt: valid without --leaf-count, invalid for the forgery', async () => {
    const unbound = await runCommand('verify-receipt', [JSON.stringify(genuine)], false);
    assert.equal(unbound.formatted, 'Receipt verification: valid');
    assert.equal(unbound.result.pathBound, false);
    const bound = await runCommand('verify-receipt', [JSON.stringify(genuine), '--leaf-count=4']);
    assert.equal(bound.valid, true);
    assert.equal(bound.pathBound, true);
    const forgedText = await runCommand('verify-receipt', [JSON.stringify(forged)], false);
    assert.equal(forgedText.formatted, 'Receipt verification: invalid');
  });
});
