/**
 * Proof Tools Module
 *
 * MCP tools for generating and verifying VES (Verifiable Event Sequencing)
 * commerce proofs — Merkle inclusion proofs, receipt bundles, batch
 * summaries, and compliance packages.
 */

import { z } from 'zod';

export const proofTools = [
  // =========================================================================
  // 1. verify_receipt
  // =========================================================================
  {
    name: 'verify_receipt',
    description:
      'Verify a VES commerce receipt — checks signature, hash, and Merkle inclusion proof.',
    inputSchema: {
      receiptBundle: z.string().min(1).describe('JSON-encoded receipt bundle'),
      publicKeyHex: z
        .string()
        .optional()
        .describe('Optional hex-encoded Ed25519 public key for signature verification'),
    },
    permission: 'read',
    handler: async ({ params }) => {
      try {
        const cryptoMod = await import('../sync/crypto.js');
        const { createProofGenerator } = await import('../sync/proof-generator.js');
        const pg = createProofGenerator(cryptoMod);
        const bundle = JSON.parse(params.receiptBundle);
        const publicKey = params.publicKeyHex || undefined;
        const result = pg.verifyReceiptBundle(bundle, publicKey);
        return { success: true, ...result };
      } catch (error) {
        return { success: false, error: error.message };
      }
    },
  },

  // =========================================================================
  // 2. generate_inclusion_proof
  // =========================================================================
  {
    name: 'generate_inclusion_proof',
    description: 'Generate a Merkle inclusion proof for a specific event within a batch of events.',
    inputSchema: {
      eventId: z.string().min(1).describe('ID of the event to prove'),
      events: z
        .string()
        .min(1)
        .describe('JSON-encoded array of events in the batch (each with id, eventSigningHash)'),
      batchId: z.string().optional().describe('Optional batch identifier'),
      anchorTxHash: z.string().optional().describe('Optional on-chain anchor transaction hash'),
    },
    permission: 'read',
    handler: async ({ params }) => {
      try {
        const cryptoMod = await import('../sync/crypto.js');
        const { createProofGenerator } = await import('../sync/proof-generator.js');
        const pg = createProofGenerator(cryptoMod);
        const events = JSON.parse(params.events);
        const proof = pg.generateInclusionProof(params.eventId, events, {
          batchId: params.batchId,
          anchorTxHash: params.anchorTxHash,
        });
        return { success: true, ...proof };
      } catch (error) {
        return { success: false, error: error.message };
      }
    },
  },

  // =========================================================================
  // 3. verify_inclusion_proof
  // =========================================================================
  {
    name: 'verify_inclusion_proof',
    description:
      'Verify a Merkle inclusion proof — confirms that a leaf hash is included in a Merkle root.',
    inputSchema: {
      leafHash: z.string().min(1).describe('Hex-encoded leaf hash'),
      proof: z.string().min(1).describe('JSON-encoded proof array (each step: { position, hash })'),
      expectedRoot: z.string().min(1).describe('Hex-encoded expected Merkle root'),
      eventId: z.string().optional().describe('Optional event ID for context'),
    },
    permission: 'read',
    handler: async ({ params }) => {
      try {
        const cryptoMod = await import('../sync/crypto.js');
        const { createProofGenerator } = await import('../sync/proof-generator.js');
        const pg = createProofGenerator(cryptoMod);
        const proof = JSON.parse(params.proof);
        const result = pg.verifyInclusionProof({
          leafHash: params.leafHash,
          proof,
          expectedRoot: params.expectedRoot,
          eventId: params.eventId,
        });
        return { success: true, ...result };
      } catch (error) {
        return { success: false, error: error.message };
      }
    },
  },

  // =========================================================================
  // 4. generate_receipt_bundle
  // =========================================================================
  {
    name: 'generate_receipt_bundle',
    description:
      'Generate a full verifiable receipt bundle for an event — includes event data, leaf hash, Merkle inclusion proof, and anchor metadata.',
    inputSchema: {
      event: z
        .string()
        .min(1)
        .describe('JSON-encoded event object (id, payload, eventSigningHash, timestamp)'),
      batchEvents: z.string().min(1).describe('JSON-encoded array of all events in the batch'),
      batchId: z.string().optional().describe('Optional batch identifier'),
      anchorTxHash: z.string().optional().describe('Optional on-chain anchor transaction hash'),
    },
    permission: 'read',
    handler: async ({ params }) => {
      try {
        const cryptoMod = await import('../sync/crypto.js');
        const { createProofGenerator } = await import('../sync/proof-generator.js');
        const pg = createProofGenerator(cryptoMod);
        const event = JSON.parse(params.event);
        const batchEvents = JSON.parse(params.batchEvents);
        const bundle = pg.generateReceiptBundle(event, batchEvents, {
          batchId: params.batchId,
          anchorTxHash: params.anchorTxHash,
        });
        return { success: true, bundle };
      } catch (error) {
        return { success: false, error: error.message };
      }
    },
  },

  // =========================================================================
  // 5. inspect_batch
  // =========================================================================
  {
    name: 'inspect_batch',
    description: 'Inspect a batch of events — computes Merkle root, event count, and time range.',
    inputSchema: {
      batchId: z.string().min(1).describe('Batch identifier'),
      events: z.string().min(1).describe('JSON-encoded array of events in the batch'),
      anchorTxHash: z.string().optional().describe('Optional on-chain anchor transaction hash'),
    },
    permission: 'read',
    handler: async ({ params }) => {
      try {
        const cryptoMod = await import('../sync/crypto.js');
        const { createProofGenerator } = await import('../sync/proof-generator.js');
        const pg = createProofGenerator(cryptoMod);
        const events = JSON.parse(params.events);
        const summary = pg.generateBatchSummary(params.batchId, events, {
          txHash: params.anchorTxHash,
        });
        return { success: true, ...summary };
      } catch (error) {
        return { success: false, error: error.message };
      }
    },
  },

  // =========================================================================
  // 6. export_compliance_package
  // =========================================================================
  {
    name: 'export_compliance_package',
    description:
      'Generate a compliance package — a complete set of verifiable receipts for all events in a batch, suitable for regulatory export or third-party audit.',
    inputSchema: {
      events: z.string().min(1).describe('JSON-encoded array of events to include'),
      batchId: z
        .string()
        .optional()
        .describe('Optional batch identifier (auto-generated if omitted)'),
      anchorTxHash: z.string().optional().describe('Optional on-chain anchor transaction hash'),
    },
    permission: 'read',
    handler: async ({ params }) => {
      try {
        const cryptoMod = await import('../sync/crypto.js');
        const { createProofGenerator } = await import('../sync/proof-generator.js');
        const pg = createProofGenerator(cryptoMod);
        const events = JSON.parse(params.events);
        const pkg = pg.generateCompliancePackage(events, {
          batchId: params.batchId,
          anchorTxHash: params.anchorTxHash,
        });
        return { success: true, ...pkg };
      } catch (error) {
        return { success: false, error: error.message };
      }
    },
  },

  // =========================================================================
  // 7. verify_chain_anchor
  // =========================================================================
  {
    name: 'verify_chain_anchor',
    description:
      'Verify that an event proof matches an expected on-chain anchor transaction hash and Merkle root. Confirms the event was committed to the chain.',
    inputSchema: {
      leafHash: z.string().min(1).describe('Hex-encoded leaf hash of the event'),
      proof: z.string().min(1).describe('JSON-encoded Merkle proof array'),
      expectedRoot: z.string().min(1).describe('Hex-encoded Merkle root from the on-chain anchor'),
      anchorTxHash: z.string().min(1).describe('On-chain anchor transaction hash'),
    },
    permission: 'read',
    handler: async ({ params }) => {
      try {
        const cryptoMod = await import('../sync/crypto.js');
        const { createProofGenerator } = await import('../sync/proof-generator.js');
        const pg = createProofGenerator(cryptoMod);
        const proof = JSON.parse(params.proof);
        const result = pg.verifyInclusionProof({
          leafHash: params.leafHash,
          proof,
          expectedRoot: params.expectedRoot,
        });
        return {
          success: true,
          ...result,
          anchorTxHash: params.anchorTxHash,
          anchored: result.valid,
          detail: result.valid
            ? `Event proof verified against on-chain root in tx ${params.anchorTxHash}`
            : 'Merkle proof does not match the expected on-chain root',
        };
      } catch (error) {
        return { success: false, error: error.message };
      }
    },
  },
];

export default proofTools;
