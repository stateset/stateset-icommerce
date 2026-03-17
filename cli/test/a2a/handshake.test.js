/**
 * Tests for cli/src/a2a/handshake.js
 *
 * Covers: createHandshakeService — initiateHandshake, respondToHandshake,
 * checkCompatibility, getMyCapabilities, overlapping/non-overlapping
 * networks & assets, feature warnings, protocol version mismatch,
 * bestNetwork/bestAsset selection.
 */

import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';

import { createHandshakeService } from '../../src/a2a/handshake.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Full-featured agent config for the "seller" side. */
function createSellerConfig(overrides = {}) {
  return {
    agentId: 'agent-seller-01',
    protocolVersion: '1.0',
    supportedNetworks: ['set_chain', 'base', 'ethereum'],
    supportedAssets: ['USDC', 'USDT'],
    features: {
      escrow: true,
      subscriptions: true,
      splits: true,
      sagas: false,
      sse: true,
    },
    maxTransactionAmount: 50000,
    preferredFinality: 'final',
    webhookEndpoint: 'https://seller.example/hooks',
    publicKey: '0xSELLER_KEY',
    ...overrides,
  };
}

/** Full-featured agent config for the "buyer" side. */
function createBuyerConfig(overrides = {}) {
  return {
    agentId: 'agent-buyer-01',
    protocolVersion: '1.0',
    supportedNetworks: ['set_chain', 'base', 'arbitrum'],
    supportedAssets: ['USDC', 'DAI'],
    features: {
      escrow: true,
      subscriptions: false,
      splits: false,
      sagas: false,
      sse: true,
    },
    maxTransactionAmount: 10000,
    preferredFinality: 'confirmed',
    webhookEndpoint: 'https://buyer.example/hooks',
    publicKey: '0xBUYER_KEY',
    ...overrides,
  };
}

// ===========================================================================
// Tests
// ===========================================================================

describe('createHandshakeService', () => {
  // -----------------------------------------------------------------------
  // getMyCapabilities
  // -----------------------------------------------------------------------

  describe('getMyCapabilities', () => {
    it('returns correct manifest based on config', () => {
      const hs = createHandshakeService(createSellerConfig());
      const caps = hs.getMyCapabilities();

      assert.equal(caps.protocolVersion, '1.0');
      assert.equal(caps.agentId, 'agent-seller-01');
      assert.deepEqual(caps.supportedNetworks, ['set_chain', 'base', 'ethereum']);
      assert.deepEqual(caps.supportedAssets, ['USDC', 'USDT']);
      assert.equal(caps.features.escrow, true);
      assert.equal(caps.features.subscriptions, true);
      assert.equal(caps.features.splits, true);
      assert.equal(caps.features.sagas, false);
      assert.equal(caps.features.sse, true);
      assert.equal(caps.maxTransactionAmount, 50000);
      assert.equal(caps.preferredFinality, 'final');
      assert.equal(caps.webhookEndpoint, 'https://seller.example/hooks');
      assert.equal(caps.publicKey, '0xSELLER_KEY');
    });

    it('applies defaults for missing fields', () => {
      const hs = createHandshakeService({});
      const caps = hs.getMyCapabilities();

      assert.equal(caps.protocolVersion, '1.0');
      assert.equal(caps.agentId, null);
      assert.deepEqual(caps.supportedNetworks, ['set_chain']);
      assert.deepEqual(caps.supportedAssets, ['USDC']);
      assert.equal(caps.features.escrow, false);
      assert.equal(caps.features.subscriptions, false);
      assert.equal(caps.features.splits, false);
      assert.equal(caps.features.sagas, false);
      assert.equal(caps.features.sse, false);
      assert.equal(caps.maxTransactionAmount, 10000);
      assert.equal(caps.preferredFinality, 'confirmed');
      assert.equal(caps.webhookEndpoint, null);
      assert.equal(caps.publicKey, null);
    });

    it('returns a copy (immutable)', () => {
      const hs = createHandshakeService(createSellerConfig());
      const caps1 = hs.getMyCapabilities();
      const caps2 = hs.getMyCapabilities();
      assert.notEqual(caps1, caps2);
      assert.deepEqual(caps1, caps2);
    });
  });

  // -----------------------------------------------------------------------
  // Compatible agents (overlapping networks + assets)
  // -----------------------------------------------------------------------

  describe('compatible agents', () => {
    it('reports compatible when networks and assets overlap', () => {
      const hs = createHandshakeService(createSellerConfig());
      const result = hs.initiateHandshake(createBuyerConfig());

      assert.equal(result.compatible, true);
      assert.equal(result.mismatches.length, 0);
    });

    it('returns shared networks', () => {
      const hs = createHandshakeService(createSellerConfig());
      const result = hs.initiateHandshake(createBuyerConfig());

      assert.deepEqual(result.sharedNetworks, ['set_chain', 'base']);
    });

    it('returns shared assets', () => {
      const hs = createHandshakeService(createSellerConfig());
      const result = hs.initiateHandshake(createBuyerConfig());

      assert.deepEqual(result.sharedAssets, ['USDC']);
    });
  });

  // -----------------------------------------------------------------------
  // Incompatible agents (no overlap)
  // -----------------------------------------------------------------------

  describe('incompatible agents', () => {
    it('reports incompatible when no network overlap', () => {
      const hs = createHandshakeService(
        createSellerConfig({ supportedNetworks: ['ethereum'] }),
      );
      const result = hs.initiateHandshake(
        createBuyerConfig({ supportedNetworks: ['solana'] }),
      );

      assert.equal(result.compatible, false);
      assert.ok(result.mismatches.some((m) => m.includes('No overlapping networks')));
    });

    it('reports incompatible when no asset overlap', () => {
      const hs = createHandshakeService(
        createSellerConfig({ supportedAssets: ['USDT'] }),
      );
      const result = hs.initiateHandshake(
        createBuyerConfig({ supportedAssets: ['DAI'] }),
      );

      assert.equal(result.compatible, false);
      assert.ok(result.mismatches.some((m) => m.includes('No overlapping assets')));
    });

    it('reports both network and asset mismatches', () => {
      const hs = createHandshakeService(
        createSellerConfig({
          supportedNetworks: ['ethereum'],
          supportedAssets: ['USDT'],
        }),
      );
      const result = hs.initiateHandshake(
        createBuyerConfig({
          supportedNetworks: ['solana'],
          supportedAssets: ['DAI'],
        }),
      );

      assert.equal(result.compatible, false);
      assert.equal(result.mismatches.length, 2);
    });

    it('includes human-readable mismatch descriptions', () => {
      const hs = createHandshakeService(
        createSellerConfig({ supportedNetworks: ['ethereum'] }),
      );
      const result = hs.initiateHandshake(
        createBuyerConfig({ supportedNetworks: ['solana'] }),
      );

      assert.ok(result.mismatches[0].includes('ethereum'));
      assert.ok(result.mismatches[0].includes('solana'));
    });
  });

  // -----------------------------------------------------------------------
  // Partial overlap
  // -----------------------------------------------------------------------

  describe('partial overlap', () => {
    it('returns shared networks from partial overlap', () => {
      const hs = createHandshakeService(
        createSellerConfig({ supportedNetworks: ['set_chain', 'base', 'ethereum'] }),
      );
      const result = hs.initiateHandshake(
        createBuyerConfig({ supportedNetworks: ['base', 'arbitrum', 'solana'] }),
      );

      assert.equal(result.compatible, true);
      assert.deepEqual(result.sharedNetworks, ['base']);
    });

    it('returns shared assets from partial overlap', () => {
      const hs = createHandshakeService(
        createSellerConfig({ supportedAssets: ['USDC', 'USDT'] }),
      );
      const result = hs.initiateHandshake(
        createBuyerConfig({ supportedAssets: ['USDT', 'DAI'] }),
      );

      assert.equal(result.compatible, true);
      assert.deepEqual(result.sharedAssets, ['USDT']);
    });
  });

  // -----------------------------------------------------------------------
  // bestNetwork / bestAsset
  // -----------------------------------------------------------------------

  describe('bestNetwork and bestAsset selection', () => {
    it('picks set_chain as best network when available', () => {
      const hs = createHandshakeService(createSellerConfig());
      const result = hs.initiateHandshake(createBuyerConfig());

      assert.equal(result.bestNetwork, 'set_chain');
    });

    it('picks base when set_chain is not shared', () => {
      const hs = createHandshakeService(
        createSellerConfig({ supportedNetworks: ['base', 'ethereum'] }),
      );
      const result = hs.initiateHandshake(
        createBuyerConfig({ supportedNetworks: ['base', 'arbitrum'] }),
      );

      assert.equal(result.bestNetwork, 'base');
    });

    it('picks USDC as best asset when available', () => {
      const hs = createHandshakeService(createSellerConfig());
      const result = hs.initiateHandshake(createBuyerConfig());

      assert.equal(result.bestAsset, 'USDC');
    });

    it('picks USDT when USDC is not shared', () => {
      const hs = createHandshakeService(
        createSellerConfig({ supportedAssets: ['USDT', 'ssUSD'] }),
      );
      const result = hs.initiateHandshake(
        createBuyerConfig({ supportedAssets: ['USDT', 'DAI'] }),
      );

      assert.equal(result.bestAsset, 'USDT');
    });

    it('picks first shared item when none are in priority list', () => {
      const hs = createHandshakeService(
        createSellerConfig({ supportedNetworks: ['polygon', 'optimism'] }),
      );
      const result = hs.initiateHandshake(
        createBuyerConfig({ supportedNetworks: ['optimism', 'polygon'] }),
      );

      // polygon is first in seller's list and shared
      assert.equal(result.bestNetwork, 'polygon');
    });

    it('returns null for bestNetwork when no overlap', () => {
      const hs = createHandshakeService(
        createSellerConfig({ supportedNetworks: ['ethereum'] }),
      );
      const result = hs.initiateHandshake(
        createBuyerConfig({ supportedNetworks: ['solana'] }),
      );

      assert.equal(result.bestNetwork, null);
    });

    it('returns null for bestAsset when no overlap', () => {
      const hs = createHandshakeService(
        createSellerConfig({ supportedAssets: ['USDT'] }),
      );
      const result = hs.initiateHandshake(
        createBuyerConfig({ supportedAssets: ['DAI'] }),
      );

      assert.equal(result.bestAsset, null);
    });
  });

  // -----------------------------------------------------------------------
  // Feature warnings
  // -----------------------------------------------------------------------

  describe('feature warnings', () => {
    it('warns when counterparty does not support escrow', () => {
      const hs = createHandshakeService(
        createSellerConfig({ features: { escrow: true } }),
      );
      const result = hs.initiateHandshake(
        createBuyerConfig({ features: { escrow: false } }),
      );

      assert.ok(result.warnings.some((w) => w.includes('escrow')));
    });

    it('warns when counterparty does not support subscriptions', () => {
      const hs = createHandshakeService(
        createSellerConfig({ features: { subscriptions: true } }),
      );
      const result = hs.initiateHandshake(
        createBuyerConfig({ features: { subscriptions: false } }),
      );

      assert.ok(result.warnings.some((w) => w.includes('subscriptions')));
    });

    it('warns when counterparty does not support splits', () => {
      const hs = createHandshakeService(
        createSellerConfig({ features: { splits: true } }),
      );
      const result = hs.initiateHandshake(
        createBuyerConfig({ features: { splits: false } }),
      );

      assert.ok(result.warnings.some((w) => w.includes('splits')));
    });

    it('does not warn for features we ourselves do not support', () => {
      const hs = createHandshakeService(
        createSellerConfig({ features: { sagas: false } }),
      );
      const result = hs.initiateHandshake(
        createBuyerConfig({ features: { sagas: true } }),
      );

      assert.ok(!result.warnings.some((w) => w.includes('sagas')));
    });

    it('warns when counterparty has no webhook endpoint', () => {
      const hs = createHandshakeService(createSellerConfig());
      const result = hs.initiateHandshake(
        createBuyerConfig({ webhookEndpoint: null }),
      );

      assert.ok(result.warnings.some((w) => w.includes('webhook')));
    });

    it('no webhook warning when both have endpoints', () => {
      const hs = createHandshakeService(createSellerConfig());
      const result = hs.initiateHandshake(createBuyerConfig());

      assert.ok(!result.warnings.some((w) => w.includes('webhook')));
    });
  });

  // -----------------------------------------------------------------------
  // Protocol version mismatch
  // -----------------------------------------------------------------------

  describe('protocol version mismatch', () => {
    it('warns on different protocol versions', () => {
      const hs = createHandshakeService(
        createSellerConfig({ protocolVersion: '1.0' }),
      );
      const result = hs.initiateHandshake(
        createBuyerConfig({ protocolVersion: '2.0' }),
      );

      assert.ok(result.warnings.some((w) => w.includes('Protocol version mismatch')));
      assert.ok(result.warnings.some((w) => w.includes('1.0') && w.includes('2.0')));
    });

    it('does not warn when protocol versions match', () => {
      const hs = createHandshakeService(
        createSellerConfig({ protocolVersion: '1.0' }),
      );
      const result = hs.initiateHandshake(
        createBuyerConfig({ protocolVersion: '1.0' }),
      );

      assert.ok(!result.warnings.some((w) => w.includes('Protocol version')));
    });

    it('protocol version mismatch does not make agents incompatible', () => {
      const hs = createHandshakeService(
        createSellerConfig({ protocolVersion: '1.0' }),
      );
      const result = hs.initiateHandshake(
        createBuyerConfig({ protocolVersion: '2.0' }),
      );

      // Still compatible if networks + assets overlap
      assert.equal(result.compatible, true);
    });
  });

  // -----------------------------------------------------------------------
  // initiateHandshake
  // -----------------------------------------------------------------------

  describe('initiateHandshake', () => {
    it('returns compatibility result with both capabilities', () => {
      const hs = createHandshakeService(createSellerConfig());
      const result = hs.initiateHandshake(createBuyerConfig());

      assert.equal(typeof result.compatible, 'boolean');
      assert.ok(Array.isArray(result.sharedNetworks));
      assert.ok(Array.isArray(result.sharedAssets));
      assert.ok(Array.isArray(result.mismatches));
      assert.ok(Array.isArray(result.warnings));
      assert.ok(result.ourCapabilities);
      assert.ok(result.theirCapabilities);
    });

    it('includes our capabilities in the result', () => {
      const sellerConfig = createSellerConfig();
      const hs = createHandshakeService(sellerConfig);
      const result = hs.initiateHandshake(createBuyerConfig());

      assert.equal(result.ourCapabilities.agentId, 'agent-seller-01');
    });

    it('includes their normalised capabilities in the result', () => {
      const hs = createHandshakeService(createSellerConfig());
      const result = hs.initiateHandshake(createBuyerConfig());

      assert.equal(result.theirCapabilities.agentId, 'agent-buyer-01');
    });

    it('computes effectiveMaxAmount as min of both', () => {
      const hs = createHandshakeService(
        createSellerConfig({ maxTransactionAmount: 50000 }),
      );
      const result = hs.initiateHandshake(
        createBuyerConfig({ maxTransactionAmount: 10000 }),
      );

      assert.equal(result.effectiveMaxAmount, 10000);
    });
  });

  // -----------------------------------------------------------------------
  // respondToHandshake
  // -----------------------------------------------------------------------

  describe('respondToHandshake', () => {
    it('evaluates incoming capabilities correctly', () => {
      const hs = createHandshakeService(createSellerConfig());
      const result = hs.respondToHandshake(createBuyerConfig());

      assert.equal(result.compatible, true);
      assert.deepEqual(result.sharedNetworks, ['set_chain', 'base']);
      assert.deepEqual(result.sharedAssets, ['USDC']);
    });

    it('includes our capabilities', () => {
      const hs = createHandshakeService(createSellerConfig());
      const result = hs.respondToHandshake(createBuyerConfig());

      assert.equal(result.ourCapabilities.agentId, 'agent-seller-01');
    });

    it('returns same result as initiateHandshake', () => {
      const hs = createHandshakeService(createSellerConfig());
      const initResult = hs.initiateHandshake(createBuyerConfig());
      const respondResult = hs.respondToHandshake(createBuyerConfig());

      assert.equal(initResult.compatible, respondResult.compatible);
      assert.deepEqual(initResult.sharedNetworks, respondResult.sharedNetworks);
      assert.deepEqual(initResult.sharedAssets, respondResult.sharedAssets);
      assert.equal(initResult.bestNetwork, respondResult.bestNetwork);
      assert.equal(initResult.bestAsset, respondResult.bestAsset);
    });

    it('handles incompatible incoming agent', () => {
      const hs = createHandshakeService(
        createSellerConfig({ supportedNetworks: ['ethereum'] }),
      );
      const result = hs.respondToHandshake(
        createBuyerConfig({ supportedNetworks: ['solana'] }),
      );

      assert.equal(result.compatible, false);
    });
  });

  // -----------------------------------------------------------------------
  // checkCompatibility (direct access)
  // -----------------------------------------------------------------------

  describe('checkCompatibility', () => {
    it('works with raw capability objects', () => {
      const hs = createHandshakeService(createSellerConfig());
      const mine = hs.getMyCapabilities();
      const theirs = {
        protocolVersion: '1.0',
        supportedNetworks: ['set_chain'],
        supportedAssets: ['USDC'],
        features: {},
        maxTransactionAmount: 5000,
        webhookEndpoint: 'https://other.example/hooks',
      };

      const result = hs.checkCompatibility(mine, theirs);
      assert.equal(result.compatible, true);
      assert.deepEqual(result.sharedNetworks, ['set_chain']);
      assert.deepEqual(result.sharedAssets, ['USDC']);
    });
  });

  // -----------------------------------------------------------------------
  // Edge cases
  // -----------------------------------------------------------------------

  describe('edge cases', () => {
    it('handles empty config gracefully', () => {
      const hs = createHandshakeService({});
      const caps = hs.getMyCapabilities();
      assert.deepEqual(caps.supportedNetworks, ['set_chain']);
      assert.deepEqual(caps.supportedAssets, ['USDC']);
    });

    it('handles counterparty with no features object', () => {
      const hs = createHandshakeService(createSellerConfig());
      const result = hs.initiateHandshake({
        supportedNetworks: ['set_chain'],
        supportedAssets: ['USDC'],
      });

      assert.equal(result.compatible, true);
      // Features should default to false, so we get warnings for our supported features
      assert.ok(result.warnings.some((w) => w.includes('escrow')));
    });

    it('handles counterparty with undefined networks', () => {
      const hs = createHandshakeService(createSellerConfig());
      const result = hs.initiateHandshake({
        supportedAssets: ['USDC'],
        // supportedNetworks intentionally omitted
      });

      // defaults to ['set_chain'], which overlaps with seller
      assert.equal(result.compatible, true);
      assert.deepEqual(result.sharedNetworks, ['set_chain']);
    });
  });
});
