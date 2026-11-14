/**
 * Interior-node forgery against the raw-leaf proof tools and commands.
 *
 * `verify_inclusion_proof` and `verify_chain_anchor` (tools/proofs.js), and
 * `proofs verify-proof` / `proofs verify-anchor` (commands/proofs.js), take a
 * raw leaf hash on trust. Without a tree size the caller knows independently
 * of the proof, nothing ties the path to a leaf: an interior node's hash plus
 * the rest of the path above it walks to the root just as a leaf does. Before
 * this suite, `verify_chain_anchor` answered that with `anchored: true` and
 * "Event proof verified against on-chain root in tx ...".
 *
 * `valid` and `anchored` keep their meaning (the hash chain reaches the
 * root). A caller-supplied `leafCount` binds the path (`pathBound`), and the
 * prose only says an event is anchored when it is bound.
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import * as cryptoMod from '../../src/sync/crypto.js';
import { createProofGenerator } from '../../src/sync/proof-generator.js';
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

describe('the forgery is real', () => {
  it('the generator built a 4-leaf tree whose interior node walks to its root', () => {
    assert.equal(REAL.leafCount, 4);
    assert.equal(REAL.proof.length, 2);
    const reached = computeNodeHash(N01, Buffer.from(REAL.proof[1].hash, 'hex'));
    assert.equal(hex(reached), REAL.root);
  });
});

describe('verify_inclusion_proof: interior-node forgery', () => {
  for (const [label, forged] of [
    ['an interior node one level up', FORGED_INTERIOR],
    ['the root itself', FORGED_ROOT],
  ]) {
    it(`${label}: without leafCount the chain reaches the root, but nothing is bound`, async () => {
      const verdict = await callTool('verify_inclusion_proof', forged);
      assert.equal(verdict.success, true);
      // `valid` keeps its meaning: the hash chain reaches the root.
      assert.equal(verdict.valid, true);
      assert.equal(verdict.pathBound, false);
      assert.equal(verdict.leafCountSource, null);
      assert.match(verdict.detail, /not bound to a tree size/);
      assert.match(verdict.detail, /leafCount/);
    });

    it(`${label}: the caller's leafCount refuses it`, async () => {
      for (const leafCount of [4, '4']) {
        const verdict = await callTool('verify_inclusion_proof', { ...forged, leafCount });
        assert.equal(verdict.valid, false, `leafCount ${JSON.stringify(leafCount)}`);
        assert.equal(verdict.pathBound, true);
        assert.equal(verdict.leafCountSource, 'caller');
        assert.doesNotMatch(verdict.detail, /^Leaf/);
      }
    });
  }

  it('a genuine leaf with the true leafCount (number or canonical string) is bound', async () => {
    for (const leafCount of [4, '4']) {
      const verdict = await callTool('verify_inclusion_proof', { ...GENUINE, leafCount });
      assert.equal(verdict.valid, true);
      assert.equal(verdict.pathBound, true);
      assert.equal(verdict.leafCountSource, 'caller');
      assert.match(verdict.detail, /4-leaf tree/);
    }
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
    }
  });

  it('a wrong or unusable caller leafCount is invalid, not trusted', async () => {
    for (const leafCount of [2, 8, 0, '04', 2 ** 32, '4294967296']) {
      const verdict = await callTool('verify_inclusion_proof', { ...GENUINE, leafCount });
      assert.equal(verdict.valid, false, `leafCount ${JSON.stringify(leafCount)}`);
      assert.equal(verdict.leafCountSource, 'caller');
    }
  });

  it('without leafCount a genuine proof still verifies as before', async () => {
    const verdict = await callTool('verify_inclusion_proof', GENUINE);
    assert.equal(verdict.valid, true);
    assert.equal(verdict.pathBound, false);
  });

  it('a failed proof is still reported as invalid', async () => {
    const verdict = await callTool('verify_inclusion_proof', {
      ...GENUINE,
      expectedRoot: hex(Buffer.alloc(32, 0xee)),
    });
    assert.equal(verdict.valid, false);
    assert.match(verdict.detail, /does not/);
  });
});

describe('verify_chain_anchor: interior-node forgery', () => {
  for (const [label, forged] of [
    ['an interior node one level up', FORGED_INTERIOR],
    ['the root itself', FORGED_ROOT],
  ]) {
    it(`${label}: without leafCount it does not claim the event was anchored`, async () => {
      const verdict = await callTool('verify_chain_anchor', { ...forged, anchorTxHash: TX });
      assert.equal(verdict.success, true);
      // `anchored` keeps its meaning: the hash chain reaches the on-chain root.
      assert.equal(verdict.anchored, true);
      assert.equal(verdict.valid, true);
      assert.equal(verdict.anchorTxHash, TX);
      assert.equal(verdict.pathBound, false);
      assert.equal(verdict.leafCountSource, null);
      // ...but the prose no longer says an event was committed.
      assert.doesNotMatch(verdict.detail, /Event proof verified/);
      assert.match(verdict.detail, /not bound to a tree size/);
      assert.match(verdict.detail, /leafCount/);
    });

    it(`${label}: the caller's leafCount refuses it`, async () => {
      const verdict = await callTool('verify_chain_anchor', {
        ...forged,
        anchorTxHash: TX,
        leafCount: 4,
      });
      assert.equal(verdict.anchored, false);
      assert.equal(verdict.valid, false);
      assert.equal(verdict.pathBound, true);
      assert.equal(verdict.leafCountSource, 'caller');
      assert.doesNotMatch(verdict.detail, /Event proof verified/);
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
    assert.equal(verdict.pathBound, true);
    assert.equal(verdict.leafCountSource, 'caller');
    assert.match(
      verdict.detail,
      new RegExp(`Event proof verified against on-chain root in tx ${TX}`),
    );
  });

  it('a proof that does not reach the root is not anchored', async () => {
    const verdict = await callTool('verify_chain_anchor', {
      ...GENUINE,
      expectedRoot: hex(Buffer.alloc(32, 0xee)),
      anchorTxHash: TX,
      leafCount: 4,
    });
    assert.equal(verdict.anchored, false);
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
    });
  }
});

describe('proofs verify-proof / verify-anchor commands: --leaf-count', () => {
  const forgedArgs = [FORGED_INTERIOR.leafHash, FORGED_INTERIOR.proof, REAL.root];
  const genuineArgs = [GENUINE.leafHash, GENUINE.proof, REAL.root];

  it('verify-proof: the forgery reaches the root unbound, and --leaf-count refuses it', async () => {
    const unbound = await runCommand('verify-proof', forgedArgs);
    assert.equal(unbound.valid, true);
    assert.equal(unbound.pathBound, false);
    assert.match(unbound.detail, /not bound to a tree size/);

    for (const flag of [['--leaf-count', '4'], ['--leaf-count=4']]) {
      const bound = await runCommand('verify-proof', [...forgedArgs, ...flag]);
      assert.equal(bound.valid, false, flag.join(' '));
      assert.equal(bound.pathBound, true);
      assert.equal(bound.leafCountSource, 'caller');
    }
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

  it('verify-anchor: the forgery is not called anchored-and-verified without --leaf-count', async () => {
    const unbound = await runCommand('verify-anchor', [...forgedArgs, TX]);
    assert.equal(unbound.anchored, true);
    assert.equal(unbound.pathBound, false);
    assert.doesNotMatch(unbound.detail, /Event proof verified/);

    const bound = await runCommand('verify-anchor', [...forgedArgs, TX, '--leaf-count', '4']);
    assert.equal(bound.anchored, false);
    assert.equal(bound.pathBound, true);

    const genuine = await runCommand('verify-anchor', [
      ...genuineArgs,
      TX,
      '--leaf-count',
      '4',
      '--leaf-index',
      '0',
    ]);
    assert.equal(genuine.anchored, true);
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

  it('the text line only says "verified" for a bound path', async () => {
    const unbound = await runCommand('verify-anchor', [...forgedArgs, TX], false);
    assert.doesNotMatch(unbound.formatted, /verified/);
    assert.match(unbound.formatted, /--leaf-count/);
    const bound = await runCommand(
      'verify-anchor',
      [...genuineArgs, TX, '--leaf-count', '4'],
      false,
    );
    assert.match(bound.formatted, /verified/);

    const proofUnbound = await runCommand('verify-proof', forgedArgs, false);
    assert.match(proofUnbound.formatted, /^Inclusion proof: valid/);
    assert.match(proofUnbound.formatted, /--leaf-count/);
    const proofBound = await runCommand(
      'verify-proof',
      [...genuineArgs, '--leaf-count', '4'],
      false,
    );
    assert.doesNotMatch(proofBound.formatted, /--leaf-count/);
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
