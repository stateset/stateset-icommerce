/**
 * Proof Tools Module
 *
 * MCP tools for generating and verifying VES (Verifiable Event Sequencing)
 * commerce proofs — Merkle inclusion proofs, receipt bundles, batch
 * summaries, and compliance packages.
 */

import { z } from 'zod';

// A raw leaf hash says nothing about where in the tree it sits, so only a
// tree size the caller knows independently of the proof binds the path, and
// the raw-leaf tools report valid/anchored true only then; see
// sync/raw-leaf-inclusion.js. Both inputs take an integer or its canonical
// decimal spelling, as parseTreeInteger in sync/merkle-shape.js reads them
// (gRPC carries uint64 values as strings).
const leafCountInput = z
  .union([
    z.number().int().min(1).max(4294967295),
    z.string().regex(/^[1-9][0-9]*$/, 'must be a canonical decimal integer of at least 1'),
  ])
  .optional()
  .describe(
    'Number of events in the batch, as you know it independently of the proof (e.g. from the ' +
      'batch commitment or anchor record). Required for a true result: only this binds the ' +
      'path to the tree (pathBound), and the path must then be exactly as long as a tree of ' +
      'that size makes it and end at a leaf inside the tree.',
  );

const leafIndexInput = z
  .union([
    z.number().int().min(0),
    z.string().regex(/^(0|[1-9][0-9]*)$/, 'must be a canonical decimal integer'),
  ])
  .optional()
  .describe(
    "The leaf's position in the batch (0-based), if you know it; the proof's left/right " +
      'positions must spell it. It does not bind the path on its own: pass leafCount for that.',
  );

export const proofTools = [
  // =========================================================================
  // 1. verify_receipt
  // =========================================================================
  {
    name: 'verify_receipt',
    description:
      'Verify a VES commerce receipt bundle: its leaf, its Merkle inclusion proof against the ' +
      "bundle's own merkleRoot (compare that with the anchored root yourself), its payload hash, " +
      'and its signature when publicKeyHex is given. valid is true only when every check passes. ' +
      'A ves-v1 leaf is recomputed from the event in its own hash domain (leafBound: true), so ' +
      'it cannot be an interior node; leafCount is optional there and also holds the path to ' +
      'the batch size (pathBound: true). A legacy-v0 leaf hash is taken as given (leafBound: ' +
      'false), so a legacy-v0 bundle is valid only with leafCount (the batch event count, known ' +
      'independently of the bundle): without it a path_bound check fails, because the inclusion ' +
      'proof then only shows that some node of the tree hashes to the root. rootReached reports ' +
      "the plain hash chain; the bundle's own leafCount is only a sanity bound.",
    inputSchema: {
      receiptBundle: z.string().min(1).describe('JSON-encoded receipt bundle'),
      publicKeyHex: z
        .string()
        .optional()
        .describe('Optional hex-encoded Ed25519 public key for signature verification'),
      leafCount: leafCountInput.describe(
        'Number of events in the batch, as you know it independently of the bundle (e.g. from ' +
          'the batch commitment or anchor record). Required for a legacy-v0 bundle to be valid; ' +
          'optional for ves-v1. It binds the inclusion path (pathBound: true), and a bundle ' +
          'stating a different leafCount fails.',
      ),
    },
    permission: 'read',
    handler: async ({ params }) => {
      try {
        const cryptoMod = await import('../sync/crypto.js');
        const { createProofGenerator } = await import('../sync/proof-generator.js');
        const { verifyReceipt } = await import('../sync/raw-leaf-inclusion.js');
        const pg = createProofGenerator(cryptoMod);
        const bundle = JSON.parse(params.receiptBundle);
        const publicKey = params.publicKeyHex || undefined;
        const result = verifyReceipt(pg, bundle, publicKey, { leafCount: params.leafCount });
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
    description:
      'Generate a Merkle inclusion proof for an event within a batch. Events carrying ' +
      'tenantId, storeId, sequenceNumber, eventSigningHash and agentSignature get a ves-v1 ' +
      'proof that the SDK spec verifier accepts; events with only id and eventSigningHash ' +
      'get a legacy-v0 proof and a warning saying which fields are missing.',
    inputSchema: {
      eventId: z.string().min(1).describe('ID of the event to prove'),
      events: z
        .string()
        .min(1)
        .describe(
          'JSON-encoded array of events in the batch. For a ves-v1 proof each needs id, ' +
            'tenantId, storeId, sequenceNumber, eventSigningHash and agentSignature.',
        ),
      batchId: z.string().optional().describe('Optional batch identifier'),
      anchorTxHash: z.string().optional().describe('Optional on-chain anchor transaction hash'),
      format: z
        .enum(['ves-v1', 'legacy-v0'])
        .optional()
        .describe(
          'Proof format. Omit to choose by the fields supplied. legacy-v0 exists only so ' +
            'roots already anchored on-chain stay verifiable.',
        ),
    },
    permission: 'read',
    handler: async ({ params }) => {
      try {
        const cryptoMod = await import('../sync/crypto.js');
        const { createProofGenerator } = await import('../sync/proof-generator.js');
        const pg = createProofGenerator(cryptoMod);
        const events = JSON.parse(params.events);
        const proof = pg.generateInclusionProof(
          params.eventId,
          events,
          { batchId: params.batchId, anchorTxHash: params.anchorTxHash },
          { format: params.format },
        );
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
      'Verify a Merkle inclusion proof for a raw leaf hash, which is taken on trust. valid is ' +
      'true only when leafCount (the batch event count, known independently of the proof) is ' +
      'given and the path leads from a leaf position of a tree of that size to expectedRoot ' +
      '(pathBound: true); it says nothing about which event the hash belongs to. Without ' +
      'leafCount valid is false: rootReached then reports whether the hash chain reaches the ' +
      'root, which only proves that some node of the tree, possibly an interior node rather ' +
      'than a leaf, hashes to it.',
    inputSchema: {
      leafHash: z.string().min(1).describe('Hex-encoded leaf hash'),
      proof: z.string().min(1).describe('JSON-encoded proof array (each step: { position, hash })'),
      expectedRoot: z.string().min(1).describe('Hex-encoded expected Merkle root'),
      eventId: z.string().optional().describe('Optional event ID for context'),
      leafCount: leafCountInput,
      leafIndex: leafIndexInput,
    },
    permission: 'read',
    handler: async ({ params }) => {
      try {
        const cryptoMod = await import('../sync/crypto.js');
        const { createProofGenerator } = await import('../sync/proof-generator.js');
        const { verifyRawLeafInclusion } = await import('../sync/raw-leaf-inclusion.js');
        const pg = createProofGenerator(cryptoMod);
        const proof = JSON.parse(params.proof);
        const result = verifyRawLeafInclusion(pg, {
          leafHash: params.leafHash,
          proof,
          expectedRoot: params.expectedRoot,
          eventId: params.eventId,
          leafIndex: params.leafIndex,
          leafCount: params.leafCount,
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
      'Verify that a raw leaf hash, taken on trust, is committed under the Merkle root of an ' +
      'on-chain anchor transaction. anchored (and valid) are true only when leafCount (the ' +
      'anchored batch event count, known independently of the proof) is given and the path ' +
      'leads from a leaf position of a tree of that size to the root (pathBound: true). Without ' +
      'leafCount anchored is false: rootReached then reports whether the hash chain reaches the ' +
      'root, which only proves that some node of the anchored tree hashes to it, not that an ' +
      'event was committed.',
    inputSchema: {
      leafHash: z.string().min(1).describe('Hex-encoded leaf hash of the event'),
      proof: z.string().min(1).describe('JSON-encoded Merkle proof array'),
      expectedRoot: z.string().min(1).describe('Hex-encoded Merkle root from the on-chain anchor'),
      anchorTxHash: z.string().min(1).describe('On-chain anchor transaction hash'),
      leafCount: leafCountInput,
      leafIndex: leafIndexInput,
    },
    permission: 'read',
    handler: async ({ params }) => {
      try {
        const cryptoMod = await import('../sync/crypto.js');
        const { createProofGenerator } = await import('../sync/proof-generator.js');
        const { verifyRawLeafAnchor } = await import('../sync/raw-leaf-inclusion.js');
        const pg = createProofGenerator(cryptoMod);
        const proof = JSON.parse(params.proof);
        const result = verifyRawLeafAnchor(pg, {
          leafHash: params.leafHash,
          proof,
          expectedRoot: params.expectedRoot,
          anchorTxHash: params.anchorTxHash,
          leafIndex: params.leafIndex,
          leafCount: params.leafCount,
        });
        return { success: true, ...result };
      } catch (error) {
        return { success: false, error: error.message };
      }
    },
  },
];

export default proofTools;
