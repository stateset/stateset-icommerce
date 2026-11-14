/**
 * Proofs Commands Module
 */

import { takeOption } from '../utils/cli-options.js';

const LEAF_COUNT_HINT = 'pass --leaf-count <n> (the batch event count) to bind it';

function parseJsonArg(value, label) {
  try {
    return JSON.parse(value);
  } catch (error) {
    throw new Error(`Invalid ${label} JSON: ${error.message}`);
  }
}

async function getProofGenerator() {
  const cryptoMod = await import('../sync/crypto.js');
  const { createProofGenerator } = await import('../sync/proof-generator.js');
  return createProofGenerator(cryptoMod);
}

/**
 * Take `--leaf-count <n>` and `--leaf-index <i>` out of the arguments.
 *
 * --leaf-count is the batch size as the caller knows it, independently of the
 * proof; only it binds the path (pathBound). --leaf-index is the position the
 * caller expects; the proof's positions must spell it.
 *
 * @param {string[]} args
 * @param {string} usage
 * @returns {Promise<{ leafCount: number|undefined, leafIndex: number|undefined, rest: string[] }>}
 */
async function takeTreeOptions(args, usage) {
  const { merklePathDepth, parseTreeInteger } = await import('../sync/merkle-shape.js');
  const count = takeOption(args, '--leaf-count', usage);
  const index = takeOption(count.rest, '--leaf-index', usage);
  if (count.value !== undefined && merklePathDepth(count.value) === null) {
    throw new Error(`${usage}\n--leaf-count must be an integer from 1 to 4294967295`);
  }
  if (index.value !== undefined && parseTreeInteger(index.value) === null) {
    throw new Error(`${usage}\n--leaf-index must be a nonnegative integer`);
  }
  return {
    leafCount: count.value === undefined ? undefined : parseTreeInteger(count.value),
    leafIndex: index.value === undefined ? undefined : parseTreeInteger(index.value),
    rest: index.rest,
  };
}

export async function execute(action, args, { jsonOutput }) {
  const pg = await getProofGenerator();

  switch (action) {
    case 'verify-receipt': {
      const [receiptBundleJson, publicKeyHex] = args;
      if (!receiptBundleJson)
        throw new Error('Usage: proofs verify-receipt <receiptBundleJson> [publicKeyHex]');
      const result = pg.verifyReceiptBundle(
        parseJsonArg(receiptBundleJson, 'receiptBundle'),
        publicKeyHex || undefined,
      );
      return jsonOutput
        ? result
        : { result, formatted: `Receipt verification: ${result.valid ? 'valid' : 'invalid'}` };
    }

    case 'generate-proof': {
      const [eventId, eventsJson, batchId, anchorTxHash] = args;
      if (!eventId || !eventsJson)
        throw new Error(
          'Usage: proofs generate-proof <eventId> <eventsJson> [batchId] [anchorTxHash]',
        );
      const result = pg.generateInclusionProof(eventId, parseJsonArg(eventsJson, 'events'), {
        batchId: batchId || undefined,
        anchorTxHash: anchorTxHash || undefined,
      });
      return jsonOutput
        ? result
        : { result, formatted: `Generated inclusion proof for ${eventId}` };
    }

    case 'verify-proof': {
      const usage =
        'Usage: proofs verify-proof <leafHash> <proofJson> <expectedRoot> [eventId] ' +
        '[--leaf-count <n>] [--leaf-index <i>]';
      const { leafCount, leafIndex, rest } = await takeTreeOptions(args, usage);
      const [leafHash, proofJson, expectedRoot, eventId] = rest;
      if (!leafHash || !proofJson || !expectedRoot) {
        throw new Error(usage);
      }
      const { verifyRawLeafInclusion } = await import('../sync/raw-leaf-inclusion.js');
      const result = verifyRawLeafInclusion(
        pg,
        {
          leafHash,
          proof: parseJsonArg(proofJson, 'proof'),
          expectedRoot,
          eventId: eventId || undefined,
          leafIndex,
          leafCount,
        },
        { leafCountHint: LEAF_COUNT_HINT },
      );
      const unbound = result.valid && !result.pathBound;
      return jsonOutput
        ? result
        : {
            result,
            formatted:
              `Inclusion proof: ${result.valid ? 'valid' : 'invalid'}` +
              (unbound ? ' (path not bound to a tree size; pass --leaf-count <n> to bind it)' : ''),
          };
    }

    case 'bundle': {
      const [eventJson, batchEventsJson, batchId, anchorTxHash] = args;
      if (!eventJson || !batchEventsJson) {
        throw new Error(
          'Usage: proofs bundle <eventJson> <batchEventsJson> [batchId] [anchorTxHash]',
        );
      }
      const result = pg.generateReceiptBundle(
        parseJsonArg(eventJson, 'event'),
        parseJsonArg(batchEventsJson, 'batchEvents'),
        {
          batchId: batchId || undefined,
          anchorTxHash: anchorTxHash || undefined,
        },
      );
      return jsonOutput ? result : { result, formatted: 'Generated receipt bundle' };
    }

    case 'inspect-batch': {
      const [batchId, eventsJson, anchorTxHash] = args;
      if (!batchId || !eventsJson)
        throw new Error('Usage: proofs inspect-batch <batchId> <eventsJson> [anchorTxHash]');
      const result = pg.generateBatchSummary(batchId, parseJsonArg(eventsJson, 'events'), {
        txHash: anchorTxHash || undefined,
      });
      return jsonOutput
        ? result
        : { result, formatted: `Batch ${batchId}: ${result.eventCount} events` };
    }

    case 'compliance-package': {
      const [eventsJson, batchId, anchorTxHash] = args;
      if (!eventsJson)
        throw new Error('Usage: proofs compliance-package <eventsJson> [batchId] [anchorTxHash]');
      const result = pg.generateCompliancePackage(parseJsonArg(eventsJson, 'events'), {
        batchId: batchId || undefined,
        anchorTxHash: anchorTxHash || undefined,
      });
      return jsonOutput ? result : { result, formatted: 'Generated compliance package' };
    }

    case 'verify-anchor': {
      const usage =
        'Usage: proofs verify-anchor <leafHash> <proofJson> <expectedRoot> <anchorTxHash> ' +
        '[--leaf-count <n>] [--leaf-index <i>]';
      const { leafCount, leafIndex, rest } = await takeTreeOptions(args, usage);
      const [leafHash, proofJson, expectedRoot, anchorTxHash] = rest;
      if (!leafHash || !proofJson || !expectedRoot || !anchorTxHash) {
        throw new Error(usage);
      }
      const { verifyRawLeafAnchor } = await import('../sync/raw-leaf-inclusion.js');
      const output = verifyRawLeafAnchor(
        pg,
        {
          leafHash,
          proof: parseJsonArg(proofJson, 'proof'),
          expectedRoot,
          anchorTxHash,
          leafIndex,
          leafCount,
        },
        { leafCountHint: LEAF_COUNT_HINT },
      );
      let formatted = 'Chain anchor: invalid';
      if (output.anchored && output.pathBound) {
        formatted = 'Chain anchor: verified';
      } else if (output.anchored) {
        formatted =
          'Chain anchor: unbound (the proof reaches the on-chain root, but no --leaf-count ' +
          'bound the path to a tree size, so it does not show the event was committed)';
      }
      return jsonOutput ? output : { output, formatted };
    }

    default:
      throw new Error(
        `Unknown action: proofs ${action}\n\n` +
          'Available actions:\n' +
          '  verify-receipt <receiptBundleJson> [publicKeyHex]     Verify receipt bundle\n' +
          '  generate-proof <eventId> <eventsJson> [batchId] [anchorTxHash]  Generate inclusion proof\n' +
          '  verify-proof <leafHash> <proofJson> <expectedRoot> [eventId] [--leaf-count <n>] [--leaf-index <i>]  Verify inclusion proof\n' +
          '  bundle <eventJson> <batchEventsJson> [batchId] [anchorTxHash]   Generate receipt bundle\n' +
          '  inspect-batch <batchId> <eventsJson> [anchorTxHash]             Inspect batch\n' +
          '  compliance-package <eventsJson> [batchId] [anchorTxHash]        Export compliance package\n' +
          '  verify-anchor <leafHash> <proofJson> <expectedRoot> <anchorTxHash> [--leaf-count <n>] [--leaf-index <i>]  Verify chain anchor',
      );
  }
}

export const metadata = {
  name: 'proofs',
  aliases: ['proof', 'receipts'],
  description: 'VES proof generation and verification commands',
  actions: {
    'verify-receipt': {
      description: 'Verify receipt bundle',
      args: ['<receiptBundleJson>', '[publicKeyHex]'],
    },
    'generate-proof': {
      description: 'Generate inclusion proof',
      args: ['<eventId>', '<eventsJson>', '[batchId]', '[anchorTxHash]'],
    },
    'verify-proof': {
      description: 'Verify inclusion proof',
      args: [
        '<leafHash>',
        '<proofJson>',
        '<expectedRoot>',
        '[eventId]',
        '[--leaf-count <n>]',
        '[--leaf-index <i>]',
      ],
    },
    bundle: {
      description: 'Generate receipt bundle',
      args: ['<eventJson>', '<batchEventsJson>', '[batchId]', '[anchorTxHash]'],
    },
    'inspect-batch': {
      description: 'Inspect batch',
      args: ['<batchId>', '<eventsJson>', '[anchorTxHash]'],
    },
    'compliance-package': {
      description: 'Export compliance package',
      args: ['<eventsJson>', '[batchId]', '[anchorTxHash]'],
    },
    'verify-anchor': {
      description: 'Verify chain anchor',
      args: [
        '<leafHash>',
        '<proofJson>',
        '<expectedRoot>',
        '<anchorTxHash>',
        '[--leaf-count <n>]',
        '[--leaf-index <i>]',
      ],
    },
  },
};

export default { execute, metadata };
