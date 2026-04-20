/**
 * Proofs Commands Module
 */

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
      const [leafHash, proofJson, expectedRoot, eventId] = args;
      if (!leafHash || !proofJson || !expectedRoot) {
        throw new Error(
          'Usage: proofs verify-proof <leafHash> <proofJson> <expectedRoot> [eventId]',
        );
      }
      const result = pg.verifyInclusionProof({
        leafHash,
        proof: parseJsonArg(proofJson, 'proof'),
        expectedRoot,
        eventId: eventId || undefined,
      });
      return jsonOutput
        ? result
        : { result, formatted: `Inclusion proof: ${result.valid ? 'valid' : 'invalid'}` };
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
      const [leafHash, proofJson, expectedRoot, anchorTxHash] = args;
      if (!leafHash || !proofJson || !expectedRoot || !anchorTxHash) {
        throw new Error(
          'Usage: proofs verify-anchor <leafHash> <proofJson> <expectedRoot> <anchorTxHash>',
        );
      }
      const result = pg.verifyInclusionProof({
        leafHash,
        proof: parseJsonArg(proofJson, 'proof'),
        expectedRoot,
      });
      const output = {
        ...result,
        anchorTxHash,
        anchored: result.valid,
      };
      return jsonOutput
        ? output
        : { output, formatted: `Chain anchor: ${output.anchored ? 'verified' : 'invalid'}` };
    }

    default:
      throw new Error(
        `Unknown action: proofs ${action}\n\n` +
          'Available actions:\n' +
          '  verify-receipt <receiptBundleJson> [publicKeyHex]     Verify receipt bundle\n' +
          '  generate-proof <eventId> <eventsJson> [batchId] [anchorTxHash]  Generate inclusion proof\n' +
          '  verify-proof <leafHash> <proofJson> <expectedRoot> [eventId]    Verify inclusion proof\n' +
          '  bundle <eventJson> <batchEventsJson> [batchId] [anchorTxHash]   Generate receipt bundle\n' +
          '  inspect-batch <batchId> <eventsJson> [anchorTxHash]             Inspect batch\n' +
          '  compliance-package <eventsJson> [batchId] [anchorTxHash]        Export compliance package\n' +
          '  verify-anchor <leafHash> <proofJson> <expectedRoot> <anchorTxHash>  Verify chain anchor',
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
      args: ['<leafHash>', '<proofJson>', '<expectedRoot>', '[eventId]'],
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
      args: ['<leafHash>', '<proofJson>', '<expectedRoot>', '<anchorTxHash>'],
    },
  },
};

export default { execute, metadata };
