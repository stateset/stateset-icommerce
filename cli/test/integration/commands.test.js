/**
 * Integration tests for command modules
 */

import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  commands,
  expandResource,
  expandAction,
  getCommand,
  generateHelp,
  RESOURCE_ALIASES,
  ACTION_ALIASES
} from '../../src/commands/index.js';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const commandsDir = path.resolve(__dirname, '../../src/commands');

describe('commands integration', () => {
  describe('command registry', () => {
    it('should have all registered resource modules', () => {
      assert.ok(commands.customers, 'customers module should exist');
      assert.ok(commands.orders, 'orders module should exist');
      assert.ok(commands.products, 'products module should exist');
      assert.ok(commands.inventory, 'inventory module should exist');
      assert.ok(commands.returns, 'returns module should exist');
      assert.ok(commands.a2a, 'a2a module should exist');
      assert.ok(commands['agent-cards'], 'agent-cards module should exist');
      assert.ok(commands['agent-runtime'], 'agent-runtime module should exist');
      assert.ok(commands['a2a-automation'], 'a2a-automation module should exist');
      assert.ok(commands['a2a-intelligence'], 'a2a-intelligence module should exist');
      assert.ok(commands['a2a-observability'], 'a2a-observability module should exist');
      assert.ok(commands['a2a-platform'], 'a2a-platform module should exist');
      assert.ok(commands.carts, 'carts module should exist');
      assert.ok(commands.checkout, 'checkout module should exist');
      assert.ok(commands.analytics, 'analytics module should exist');
      assert.ok(commands.loyalty, 'loyalty module should exist');
      assert.ok(commands['gift-cards'], 'gift-cards module should exist');
      assert.ok(commands['store-credits'], 'store-credits module should exist');
      assert.ok(commands.warehouse, 'warehouse module should exist');
      assert.ok(commands.receiving, 'receiving module should exist');
      assert.ok(commands.fulfillment, 'fulfillment module should exist');
      assert.ok(commands['accounts-payable'], 'accounts-payable module should exist');
      assert.ok(commands['accounts-receivable'], 'accounts-receivable module should exist');
      assert.ok(commands['general-ledger'], 'general-ledger module should exist');
      assert.ok(commands['cost-accounting'], 'cost-accounting module should exist');
      assert.ok(commands.credit, 'credit module should exist');
      assert.ok(commands.backorders, 'backorders module should exist');
      assert.ok(commands.lots, 'lots module should exist');
      assert.ok(commands.serials, 'serials module should exist');
      assert.ok(commands.quality, 'quality module should exist');
      assert.ok(commands.reviews, 'reviews module should exist');
      assert.ok(commands.wishlists, 'wishlists module should exist');
      assert.ok(commands.segments, 'segments module should exist');
      assert.ok(commands.catalog, 'catalog module should exist');
      assert.ok(commands.fraud, 'fraud module should exist');
      assert.ok(commands.audit, 'audit module should exist');
      assert.ok(commands.manufacturing, 'manufacturing module should exist');
      assert.ok(commands['custom-objects'], 'custom-objects module should exist');
      assert.ok(commands.connectors, 'connectors module should exist');
      assert.ok(commands['shipping-zones'], 'shipping-zones module should exist');
      assert.ok(commands.compliance, 'compliance module should exist');
      assert.ok(commands.policies, 'policies module should exist');
      assert.ok(commands.sync, 'sync module should exist');
      assert.ok(commands['circuit-breaker'], 'circuit-breaker module should exist');
      assert.ok(commands.erc8004, 'erc8004 module should exist');
      assert.ok(commands.import, 'import module should exist');
      assert.ok(commands.proofs, 'proofs module should exist');
      assert.ok(commands.promotions, 'promotions module should exist');
      assert.ok(commands.stablecoin, 'stablecoin module should exist');
      assert.ok(commands.subscriptions, 'subscriptions module should exist');
      assert.ok(commands.currency, 'currency module should exist');
      assert.ok(commands.tax, 'tax module should exist');
      assert.ok(commands.treasury, 'treasury module should exist');
      assert.ok(commands.vector, 'vector module should exist');
      assert.ok(commands.payments, 'payments module should exist');
      assert.ok(commands.shipments, 'shipments module should exist');
      assert.ok(commands.suppliers, 'suppliers module should exist');
      assert.ok(commands.invoices, 'invoices module should exist');
      assert.ok(commands.warranties, 'warranties module should exist');
      assert.ok(commands.x402, 'x402 module should exist');
    });

    it('should register every command module file on disk except index.js', () => {
      const moduleFiles = fs
        .readdirSync(commandsDir)
        .filter((entry) => entry.endsWith('.js') && entry !== 'index.js')
        .map((entry) => entry.replace(/\.js$/, ''))
        .sort();

      assert.deepStrictEqual(Object.keys(commands).sort(), moduleFiles);
    });

    it('should have execute function for each command', () => {
      for (const [name, cmd] of Object.entries(commands)) {
        assert.strictEqual(typeof cmd.execute, 'function', `${name} should have execute function`);
      }
    });

    it('should have metadata for each command', () => {
      for (const [name, cmd] of Object.entries(commands)) {
        assert.ok(cmd.metadata, `${name} should have metadata`);
        assert.ok(cmd.metadata.name, `${name} should have metadata.name`);
        assert.ok(cmd.metadata.aliases, `${name} should have metadata.aliases`);
        assert.ok(cmd.metadata.actions, `${name} should have metadata.actions`);
      }
    });
  });

  describe('resource aliases', () => {
    it('should have single letter aliases', () => {
      assert.strictEqual(RESOURCE_ALIASES['c'], 'customers');
      assert.strictEqual(RESOURCE_ALIASES['o'], 'orders');
      assert.strictEqual(RESOURCE_ALIASES['p'], 'products');
      assert.strictEqual(RESOURCE_ALIASES['i'], 'inventory');
      assert.strictEqual(RESOURCE_ALIASES['r'], 'returns');
      assert.strictEqual(RESOURCE_ALIASES['a'], 'analytics');
      assert.strictEqual(RESOURCE_ALIASES['t'], 'tax');
      assert.strictEqual(RESOURCE_ALIASES['cart'], 'carts');
    });

    it('should have abbreviated aliases', () => {
      assert.strictEqual(RESOURCE_ALIASES['cust'], 'customers');
      assert.strictEqual(RESOURCE_ALIASES['ord'], 'orders');
      assert.strictEqual(RESOURCE_ALIASES['prod'], 'products');
      assert.strictEqual(RESOURCE_ALIASES['inv'], 'inventory');
      assert.strictEqual(RESOURCE_ALIASES['ret'], 'returns');
      assert.strictEqual(RESOURCE_ALIASES['p2p'], 'a2a');
      assert.strictEqual(RESOURCE_ALIASES['cards'], 'agent-cards');
      assert.strictEqual(RESOURCE_ALIASES['agent-card'], 'agent-cards');
      assert.strictEqual(RESOURCE_ALIASES['rt'], 'agent-runtime');
      assert.strictEqual(RESOURCE_ALIASES['runtime'], 'agent-runtime');
      assert.strictEqual(RESOURCE_ALIASES['a2aa'], 'a2a-automation');
      assert.strictEqual(RESOURCE_ALIASES['ops'], 'a2a-automation');
      assert.strictEqual(RESOURCE_ALIASES['a2ai'], 'a2a-intelligence');
      assert.strictEqual(RESOURCE_ALIASES['intel'], 'a2a-intelligence');
      assert.strictEqual(RESOURCE_ALIASES['a2ao'], 'a2a-observability');
      assert.strictEqual(RESOURCE_ALIASES['obs'], 'a2a-observability');
      assert.strictEqual(RESOURCE_ALIASES['a2ap'], 'a2a-platform');
      assert.strictEqual(RESOURCE_ALIASES['messaging'], 'a2a-platform');
      assert.strictEqual(RESOURCE_ALIASES['xpay'], 'x402');
      assert.strictEqual(RESOURCE_ALIASES['basket'], 'carts');
      assert.strictEqual(RESOURCE_ALIASES['cko'], 'checkout');
      assert.strictEqual(RESOURCE_ALIASES['paylink'], 'checkout');
      assert.strictEqual(RESOURCE_ALIASES['rewards'], 'loyalty');
      assert.strictEqual(RESOURCE_ALIASES['points'], 'loyalty');
      assert.strictEqual(RESOURCE_ALIASES['giftcard'], 'gift-cards');
      assert.strictEqual(RESOURCE_ALIASES['gc'], 'gift-cards');
      assert.strictEqual(RESOURCE_ALIASES['credits'], 'store-credits');
      assert.strictEqual(RESOURCE_ALIASES['credit'], 'store-credits');
      assert.strictEqual(RESOURCE_ALIASES['wh'], 'warehouse');
      assert.strictEqual(RESOURCE_ALIASES['warehouses'], 'warehouse');
      assert.strictEqual(RESOURCE_ALIASES['receipts'], 'receiving');
      assert.strictEqual(RESOURCE_ALIASES['recv'], 'receiving');
      assert.strictEqual(RESOURCE_ALIASES['fulfill'], 'fulfillment');
      assert.strictEqual(RESOURCE_ALIASES['pick'], 'fulfillment');
      assert.strictEqual(RESOURCE_ALIASES['ap'], 'accounts-payable');
      assert.strictEqual(RESOURCE_ALIASES['bills'], 'accounts-payable');
      assert.strictEqual(RESOURCE_ALIASES['ar'], 'accounts-receivable');
      assert.strictEqual(RESOURCE_ALIASES['credit-memos'], 'accounts-receivable');
      assert.strictEqual(RESOURCE_ALIASES['gl'], 'general-ledger');
      assert.strictEqual(RESOURCE_ALIASES['ledger'], 'general-ledger');
      assert.strictEqual(RESOURCE_ALIASES['costs'], 'cost-accounting');
      assert.strictEqual(RESOURCE_ALIASES['costing'], 'cost-accounting');
      assert.strictEqual(RESOURCE_ALIASES['credit-accounts'], 'credit');
      assert.strictEqual(RESOURCE_ALIASES['lending'], 'credit');
      assert.strictEqual(RESOURCE_ALIASES['bo'], 'backorders');
      assert.strictEqual(RESOURCE_ALIASES['backorder'], 'backorders');
      assert.strictEqual(RESOURCE_ALIASES['lot'], 'lots');
      assert.strictEqual(RESOURCE_ALIASES['batches'], 'lots');
      assert.strictEqual(RESOURCE_ALIASES['serial'], 'serials');
      assert.strictEqual(RESOURCE_ALIASES['sn'], 'serials');
      assert.strictEqual(RESOURCE_ALIASES['qa'], 'quality');
      assert.strictEqual(RESOURCE_ALIASES['ncr'], 'quality');
      assert.strictEqual(RESOURCE_ALIASES['rev'], 'reviews');
      assert.strictEqual(RESOURCE_ALIASES['review'], 'reviews');
      assert.strictEqual(RESOURCE_ALIASES['wl'], 'wishlists');
      assert.strictEqual(RESOURCE_ALIASES['wishlist'], 'wishlists');
      assert.strictEqual(RESOURCE_ALIASES['seg'], 'segments');
      assert.strictEqual(RESOURCE_ALIASES['segment'], 'segments');
      assert.strictEqual(RESOURCE_ALIASES['cat'], 'catalog');
      assert.strictEqual(RESOURCE_ALIASES['catalogue'], 'catalog');
      assert.strictEqual(RESOURCE_ALIASES['risk'], 'fraud');
      assert.strictEqual(RESOURCE_ALIASES['fraud-review'], 'fraud');
      assert.strictEqual(RESOURCE_ALIASES['logs'], 'audit');
      assert.strictEqual(RESOURCE_ALIASES['auditlog'], 'audit');
      assert.strictEqual(RESOURCE_ALIASES['mfg'], 'manufacturing');
      assert.strictEqual(RESOURCE_ALIASES['bom'], 'manufacturing');
      assert.strictEqual(RESOURCE_ALIASES['co'], 'custom-objects');
      assert.strictEqual(RESOURCE_ALIASES['metaobjects'], 'custom-objects');
      assert.strictEqual(RESOURCE_ALIASES['conn'], 'connectors');
      assert.strictEqual(RESOURCE_ALIASES['wasm'], 'connectors');
      assert.strictEqual(RESOURCE_ALIASES['zones'], 'shipping-zones');
      assert.strictEqual(RESOURCE_ALIASES['shipzones'], 'shipping-zones');
      assert.strictEqual(RESOURCE_ALIASES['cmp'], 'compliance');
      assert.strictEqual(RESOURCE_ALIASES['regulatory'], 'compliance');
      assert.strictEqual(RESOURCE_ALIASES['policy'], 'policies');
      assert.strictEqual(RESOURCE_ALIASES['rules'], 'policies');
      assert.strictEqual(RESOURCE_ALIASES['ves'], 'sync');
      assert.strictEqual(RESOURCE_ALIASES['sequencer'], 'sync');
      assert.strictEqual(RESOURCE_ALIASES['cb'], 'circuit-breaker');
      assert.strictEqual(RESOURCE_ALIASES['breaker'], 'circuit-breaker');
      assert.strictEqual(RESOURCE_ALIASES['identity'], 'erc8004');
      assert.strictEqual(RESOURCE_ALIASES['registry'], 'erc8004');
      assert.strictEqual(RESOURCE_ALIASES['ingest'], 'import');
      assert.strictEqual(RESOURCE_ALIASES['etl'], 'import');
      assert.strictEqual(RESOURCE_ALIASES['proof'], 'proofs');
      assert.strictEqual(RESOURCE_ALIASES['an'], 'analytics');
      assert.strictEqual(RESOURCE_ALIASES['promo'], 'promotions');
      assert.strictEqual(RESOURCE_ALIASES['sc'], 'stablecoin');
      assert.strictEqual(RESOURCE_ALIASES['stable'], 'stablecoin');
      assert.strictEqual(RESOURCE_ALIASES['subs'], 'subscriptions');
      assert.strictEqual(RESOURCE_ALIASES['curr'], 'currency');
      assert.strictEqual(RESOURCE_ALIASES['treas'], 'treasury');
      assert.strictEqual(RESOURCE_ALIASES['cash'], 'treasury');
      assert.strictEqual(RESOURCE_ALIASES['vec'], 'vector');
      assert.strictEqual(RESOURCE_ALIASES['semantic'], 'vector');
      assert.strictEqual(RESOURCE_ALIASES['pay'], 'payments');
      assert.strictEqual(RESOURCE_ALIASES['pmt'], 'payments');
      assert.strictEqual(RESOURCE_ALIASES['ship'], 'shipments');
      assert.strictEqual(RESOURCE_ALIASES['ships'], 'shipments');
      assert.strictEqual(RESOURCE_ALIASES['shp'], 'shipments');
      assert.strictEqual(RESOURCE_ALIASES['supp'], 'suppliers');
      assert.strictEqual(RESOURCE_ALIASES['po'], 'suppliers');
      assert.strictEqual(RESOURCE_ALIASES['invc'], 'invoices');
      assert.strictEqual(RESOURCE_ALIASES['bill'], 'invoices');
      assert.strictEqual(RESOURCE_ALIASES['warranty'], 'warranties');
      assert.strictEqual(RESOURCE_ALIASES['claims'], 'warranties');
    });

    it('should have special aliases', () => {
      assert.strictEqual(RESOURCE_ALIASES['stock'], 'inventory');
      assert.strictEqual(RESOURCE_ALIASES['fx'], 'currency');
      assert.strictEqual(RESOURCE_ALIASES['vat'], 'tax');
    });
  });

  describe('action aliases', () => {
    it('should have list aliases', () => {
      assert.strictEqual(ACTION_ALIASES['l'], 'list');
      assert.strictEqual(ACTION_ALIASES['ls'], 'list');
    });

    it('should have action shortcuts', () => {
      assert.strictEqual(ACTION_ALIASES['g'], 'get');
      assert.strictEqual(ACTION_ALIASES['s'], 'ship');
      assert.strictEqual(ACTION_ALIASES['x'], 'cancel');
      assert.strictEqual(ACTION_ALIASES['a'], 'adjust');
    });

    it('should have count shortcuts', () => {
      assert.strictEqual(ACTION_ALIASES['n'], 'count');
      assert.strictEqual(ACTION_ALIASES['#'], 'count');
    });
  });

  describe('expandResource', () => {
    it('should expand single letter aliases', () => {
      assert.strictEqual(expandResource('c'), 'customers');
      assert.strictEqual(expandResource('o'), 'orders');
    });

    it('should expand abbreviated aliases', () => {
      assert.strictEqual(expandResource('cust'), 'customers');
      assert.strictEqual(expandResource('inv'), 'inventory');
      assert.strictEqual(expandResource('p2p'), 'a2a');
      assert.strictEqual(expandResource('cards'), 'agent-cards');
      assert.strictEqual(expandResource('rt'), 'agent-runtime');
      assert.strictEqual(expandResource('a2aa'), 'a2a-automation');
      assert.strictEqual(expandResource('a2ai'), 'a2a-intelligence');
      assert.strictEqual(expandResource('a2ao'), 'a2a-observability');
      assert.strictEqual(expandResource('a2ap'), 'a2a-platform');
      assert.strictEqual(expandResource('xpay'), 'x402');
      assert.strictEqual(expandResource('basket'), 'carts');
      assert.strictEqual(expandResource('cko'), 'checkout');
      assert.strictEqual(expandResource('rewards'), 'loyalty');
      assert.strictEqual(expandResource('giftcard'), 'gift-cards');
      assert.strictEqual(expandResource('credits'), 'store-credits');
      assert.strictEqual(expandResource('wh'), 'warehouse');
      assert.strictEqual(expandResource('recv'), 'receiving');
      assert.strictEqual(expandResource('fulfill'), 'fulfillment');
      assert.strictEqual(expandResource('ap'), 'accounts-payable');
      assert.strictEqual(expandResource('ar'), 'accounts-receivable');
      assert.strictEqual(expandResource('gl'), 'general-ledger');
      assert.strictEqual(expandResource('costs'), 'cost-accounting');
      assert.strictEqual(expandResource('lending'), 'credit');
      assert.strictEqual(expandResource('bo'), 'backorders');
      assert.strictEqual(expandResource('lot'), 'lots');
      assert.strictEqual(expandResource('sn'), 'serials');
      assert.strictEqual(expandResource('qa'), 'quality');
      assert.strictEqual(expandResource('rev'), 'reviews');
      assert.strictEqual(expandResource('wl'), 'wishlists');
      assert.strictEqual(expandResource('seg'), 'segments');
      assert.strictEqual(expandResource('cat'), 'catalog');
      assert.strictEqual(expandResource('risk'), 'fraud');
      assert.strictEqual(expandResource('logs'), 'audit');
      assert.strictEqual(expandResource('mfg'), 'manufacturing');
      assert.strictEqual(expandResource('co'), 'custom-objects');
      assert.strictEqual(expandResource('conn'), 'connectors');
      assert.strictEqual(expandResource('zones'), 'shipping-zones');
      assert.strictEqual(expandResource('cmp'), 'compliance');
      assert.strictEqual(expandResource('policy'), 'policies');
      assert.strictEqual(expandResource('ves'), 'sync');
      assert.strictEqual(expandResource('cb'), 'circuit-breaker');
      assert.strictEqual(expandResource('identity'), 'erc8004');
      assert.strictEqual(expandResource('ingest'), 'import');
      assert.strictEqual(expandResource('proof'), 'proofs');
      assert.strictEqual(expandResource('promo'), 'promotions');
      assert.strictEqual(expandResource('sc'), 'stablecoin');
      assert.strictEqual(expandResource('subs'), 'subscriptions');
      assert.strictEqual(expandResource('treas'), 'treasury');
      assert.strictEqual(expandResource('vec'), 'vector');
      assert.strictEqual(expandResource('pay'), 'payments');
      assert.strictEqual(expandResource('ship'), 'shipments');
      assert.strictEqual(expandResource('supp'), 'suppliers');
      assert.strictEqual(expandResource('invc'), 'invoices');
      assert.strictEqual(expandResource('claims'), 'warranties');
    });

    it('should be case insensitive', () => {
      assert.strictEqual(expandResource('C'), 'customers');
      assert.strictEqual(expandResource('ORDERS'), 'orders');
    });

    it('should pass through unknown resources', () => {
      assert.strictEqual(expandResource('unknown'), 'unknown');
    });

    it('should handle null/undefined', () => {
      assert.strictEqual(expandResource(null), null);
      assert.strictEqual(expandResource(undefined), undefined);
    });
  });

  describe('expandAction', () => {
    it('should expand action aliases', () => {
      assert.strictEqual(expandAction('l'), 'list');
      assert.strictEqual(expandAction('g'), 'get');
      assert.strictEqual(expandAction('s'), 'ship');
    });

    it('should be case insensitive', () => {
      assert.strictEqual(expandAction('L'), 'list');
      assert.strictEqual(expandAction('LS'), 'list');
    });

    it('should pass through unknown actions', () => {
      assert.strictEqual(expandAction('create'), 'create');
    });
  });

  describe('getCommand', () => {
    it('should get command by full name', () => {
      const cmd = getCommand('customers');
      assert.ok(cmd);
      assert.strictEqual(cmd.metadata.name, 'customers');
    });

    it('should get command by alias', () => {
      const cmd = getCommand('c');
      assert.ok(cmd);
      assert.strictEqual(cmd.metadata.name, 'customers');
    });

    it('should return undefined for unknown command', () => {
      const cmd = getCommand('unknown');
      assert.strictEqual(cmd, undefined);
    });
  });

  describe('command metadata', () => {
    describe('customers', () => {
      const meta = commands.customers.metadata;

      it('should have correct name', () => {
        assert.strictEqual(meta.name, 'customers');
      });

      it('should have aliases', () => {
        assert.ok(meta.aliases.includes('c'));
        assert.ok(meta.aliases.includes('cust'));
      });

      it('should have all actions', () => {
        assert.ok(meta.actions.list);
        assert.ok(meta.actions.get);
        assert.ok(meta.actions.create);
        assert.ok(meta.actions.count);
      });
    });

    describe('orders', () => {
      const meta = commands.orders.metadata;

      it('should have order-specific actions', () => {
        assert.ok(meta.actions.ship);
        assert.ok(meta.actions.cancel);
        assert.ok(meta.actions.status);
        assert.ok(meta.actions.pending);
        assert.ok(meta.actions.recent);
      });
    });

    describe('inventory', () => {
      const meta = commands.inventory.metadata;

      it('should have inventory-specific actions', () => {
        assert.ok(meta.actions.stock);
        assert.ok(meta.actions.adjust);
        assert.ok(meta.actions.low);
        assert.ok(meta.actions.reserve);
        assert.ok(meta.actions.release);
      });
    });

    describe('returns', () => {
      const meta = commands.returns.metadata;

      it('should have return-specific actions', () => {
        assert.ok(meta.actions.approve);
        assert.ok(meta.actions.reject);
        assert.ok(meta.actions.pending);
        assert.ok(meta.actions.stats);
      });
    });

    describe('carts', () => {
      const meta = commands.carts.metadata;

      it('should have cart-specific actions', () => {
        assert.ok(meta.actions.add);
        assert.ok(meta.actions.payment);
        assert.ok(meta.actions.complete);
        assert.ok(meta.actions.abandoned);
      });
    });

    describe('checkout', () => {
      const meta = commands.checkout.metadata;

      it('should have checkout-specific actions', () => {
        assert.ok(meta.actions['create-link']);
        assert.ok(meta.actions.resolve);
        assert.ok(meta.actions.express);
        assert.ok(meta.actions.crypto);
      });
    });

    describe('analytics', () => {
      const meta = commands.analytics.metadata;

      it('should have analytics-specific actions', () => {
        assert.ok(meta.actions.sales);
        assert.ok(meta.actions.revenue);
        assert.ok(meta.actions.forecast);
        assert.ok(meta.actions.fulfillment);
      });
    });

    describe('loyalty', () => {
      const meta = commands.loyalty.metadata;

      it('should have loyalty-specific actions', () => {
        assert.ok(meta.actions.program);
        assert.ok(meta.actions.enroll);
        assert.ok(meta.actions.earn);
        assert.ok(meta.actions['create-reward']);
      });
    });

    describe('gift-cards', () => {
      const meta = commands['gift-cards'].metadata;

      it('should have gift-card-specific actions', () => {
        assert.ok(meta.actions.create);
        assert.ok(meta.actions.charge);
        assert.ok(meta.actions.refund);
        assert.ok(meta.actions.balance);
      });
    });

    describe('store-credits', () => {
      const meta = commands['store-credits'].metadata;

      it('should have store-credit-specific actions', () => {
        assert.ok(meta.actions.create);
        assert.ok(meta.actions.adjust);
        assert.ok(meta.actions.apply);
      });
    });

    describe('warehouse', () => {
      const meta = commands.warehouse.metadata;

      it('should have warehouse-specific actions', () => {
        assert.ok(meta.actions.locations);
        assert.ok(meta.actions['create-location']);
        assert.ok(meta.actions.pickable);
        assert.ok(meta.actions.available);
      });
    });

    describe('receiving', () => {
      const meta = commands.receiving.metadata;

      it('should have receiving-specific actions', () => {
        assert.ok(meta.actions.create);
        assert.ok(meta.actions['from-po']);
        assert.ok(meta.actions.start);
        assert.ok(meta.actions.complete);
      });
    });

    describe('fulfillment', () => {
      const meta = commands.fulfillment.metadata;

      it('should have fulfillment-specific actions', () => {
        assert.ok(meta.actions.waves);
        assert.ok(meta.actions['create-wave']);
        assert.ok(meta.actions.picks);
        assert.ok(meta.actions['assign-pick']);
      });
    });

    describe('accounts-payable', () => {
      const meta = commands['accounts-payable'].metadata;

      it('should have accounts-payable-specific actions', () => {
        assert.ok(meta.actions.bills);
        assert.ok(meta.actions['create-bill']);
        assert.ok(meta.actions.aging);
        assert.ok(meta.actions.outstanding);
      });
    });

    describe('accounts-receivable', () => {
      const meta = commands['accounts-receivable'].metadata;

      it('should have accounts-receivable-specific actions', () => {
        assert.ok(meta.actions.aging);
        assert.ok(meta.actions.dso);
        assert.ok(meta.actions['credit-memos']);
        assert.ok(meta.actions['create-credit-memo']);
      });
    });

    describe('general-ledger', () => {
      const meta = commands['general-ledger'].metadata;

      it('should have general-ledger-specific actions', () => {
        assert.ok(meta.actions.accounts);
        assert.ok(meta.actions['create-account']);
        assert.ok(meta.actions['trial-balance']);
        assert.ok(meta.actions['income-statement']);
      });
    });

    describe('cost-accounting', () => {
      const meta = commands['cost-accounting'].metadata;

      it('should have cost-accounting-specific actions', () => {
        assert.ok(meta.actions.list);
        assert.ok(meta.actions.set);
        assert.ok(meta.actions.average);
        assert.ok(meta.actions['inventory-value']);
      });
    });

    describe('credit', () => {
      const meta = commands.credit.metadata;

      it('should have credit-specific actions', () => {
        assert.ok(meta.actions.accounts);
        assert.ok(meta.actions.check);
        assert.ok(meta.actions['adjust-limit']);
        assert.ok(meta.actions['over-limit']);
      });
    });

    describe('backorders', () => {
      const meta = commands.backorders.metadata;

      it('should have backorder-specific actions', () => {
        assert.ok(meta.actions.create);
        assert.ok(meta.actions.order);
        assert.ok(meta.actions.summary);
        assert.ok(meta.actions.overdue);
      });
    });

    describe('lots', () => {
      const meta = commands.lots.metadata;

      it('should have lot-specific actions', () => {
        assert.ok(meta.actions.create);
        assert.ok(meta.actions.active);
        assert.ok(meta.actions.quarantine);
        assert.ok(meta.actions.expiring);
      });
    });

    describe('serials', () => {
      const meta = commands.serials.metadata;

      it('should have serial-specific actions', () => {
        assert.ok(meta.actions.create);
        assert.ok(meta.actions.available);
        assert.ok(meta.actions.sold);
        assert.ok(meta.actions.check);
      });
    });

    describe('quality', () => {
      const meta = commands.quality.metadata;

      it('should have quality-specific actions', () => {
        assert.ok(meta.actions.inspections);
        assert.ok(meta.actions['create-inspection']);
        assert.ok(meta.actions.ncrs);
        assert.ok(meta.actions['create-hold']);
      });
    });

    describe('reviews', () => {
      const meta = commands.reviews.metadata;

      it('should have review-specific actions', () => {
        assert.ok(meta.actions.create);
        assert.ok(meta.actions.approve);
        assert.ok(meta.actions.summary);
        assert.ok(meta.actions.flag);
      });
    });

    describe('wishlists', () => {
      const meta = commands.wishlists.metadata;

      it('should have wishlist-specific actions', () => {
        assert.ok(meta.actions.create);
        assert.ok(meta.actions['add-item']);
        assert.ok(meta.actions['remove-item']);
        assert.ok(meta.actions.convert);
      });
    });

    describe('segments', () => {
      const meta = commands.segments.metadata;

      it('should have segment-specific actions', () => {
        assert.ok(meta.actions.create);
        assert.ok(meta.actions.update);
        assert.ok(meta.actions.evaluate);
        assert.ok(meta.actions.rebuild);
      });
    });

    describe('catalog', () => {
      const meta = commands.catalog.metadata;

      it('should have catalog-specific actions', () => {
        assert.ok(meta.actions.publish);
        assert.ok(meta.actions.query);
        assert.ok(meta.actions['match-agent']);
        assert.ok(meta.actions.export);
      });
    });

    describe('fraud', () => {
      const meta = commands.fraud.metadata;

      it('should have fraud-specific actions', () => {
        assert.ok(meta.actions.assess);
        assert.ok(meta.actions.assessment);
        assert.ok(meta.actions['create-rule']);
        assert.ok(meta.actions.review);
      });
    });

    describe('audit', () => {
      const meta = commands.audit.metadata;

      it('should have audit-specific actions', () => {
        assert.ok(meta.actions.query);
        assert.ok(meta.actions.summary);
        assert.ok(meta.actions.export);
        assert.ok(meta.actions.retention);
      });
    });

    describe('manufacturing', () => {
      const meta = commands.manufacturing.metadata;

      it('should have manufacturing-specific actions', () => {
        assert.ok(meta.actions.boms);
        assert.ok(meta.actions['create-bom']);
        assert.ok(meta.actions['work-orders']);
        assert.ok(meta.actions['complete-work-order']);
      });
    });

    describe('custom-objects', () => {
      const meta = commands['custom-objects'].metadata;

      it('should have custom-object-specific actions', () => {
        assert.ok(meta.actions.types);
        assert.ok(meta.actions['create-type']);
        assert.ok(meta.actions.create);
        assert.ok(meta.actions.handle);
      });
    });

    describe('connectors', () => {
      const meta = commands.connectors.metadata;

      it('should have connector-specific actions', () => {
        assert.ok(meta.actions.marketplace);
        assert.ok(meta.actions.install);
        assert.ok(meta.actions.verify);
        assert.ok(meta.actions.execute);
      });
    });

    describe('shipping-zones', () => {
      const meta = commands['shipping-zones'].metadata;

      it('should have shipping-zone-specific actions', () => {
        assert.ok(meta.actions.zones);
        assert.ok(meta.actions['create-zone']);
        assert.ok(meta.actions['create-method']);
        assert.ok(meta.actions.rates);
      });
    });

    describe('compliance', () => {
      const meta = commands.compliance.metadata;

      it('should have compliance-specific actions', () => {
        assert.ok(meta.actions['audit-trail']);
        assert.ok(meta.actions['1099k']);
        assert.ok(meta.actions['export-gdpr']);
        assert.ok(meta.actions.soc2);
      });
    });

    describe('policies', () => {
      const meta = commands.policies.metadata;

      it('should have policy-specific actions', () => {
        assert.ok(meta.actions.evaluate);
        assert.ok(meta.actions.list);
        assert.ok(meta.actions.template);
        assert.ok(meta.actions.explain);
      });
    });

    describe('sync', () => {
      const meta = commands.sync.metadata;

      it('should have sync-specific actions', () => {
        assert.ok(meta.actions.status);
        assert.ok(meta.actions.pull);
        assert.ok(meta.actions.conflicts);
        assert.ok(meta.actions['key-export']);
      });
    });

    describe('circuit-breaker', () => {
      const meta = commands['circuit-breaker'].metadata;

      it('should have circuit-breaker-specific actions', () => {
        assert.ok(meta.actions.state);
        assert.ok(meta.actions.spending);
        assert.ok(meta.actions.trip);
        assert.ok(meta.actions.limits);
      });
    });

    describe('erc8004', () => {
      const meta = commands.erc8004.metadata;

      it('should have erc8004-specific actions', () => {
        assert.ok(meta.actions.register);
        assert.ok(meta.actions['link-wallet']);
        assert.ok(meta.actions.get);
        assert.ok(meta.actions.list);
      });
    });

    describe('import', () => {
      const meta = commands.import.metadata;

      it('should have import-specific actions', () => {
        assert.ok(meta.actions.shopify);
        assert.ok(meta.actions.status);
        assert.ok(meta.actions.export);
        assert.ok(meta.actions.woocommerce);
      });
    });

    describe('proofs', () => {
      const meta = commands.proofs.metadata;

      it('should have proofs-specific actions', () => {
        assert.ok(meta.actions['verify-receipt']);
        assert.ok(meta.actions['generate-proof']);
        assert.ok(meta.actions.bundle);
        assert.ok(meta.actions['verify-anchor']);
      });
    });

    describe('promotions', () => {
      const meta = commands.promotions.metadata;

      it('should have promotion-specific actions', () => {
        assert.ok(meta.actions.active);
        assert.ok(meta.actions.coupon);
        assert.ok(meta.actions.validate);
        assert.ok(meta.actions.apply);
      });
    });

    describe('subscriptions', () => {
      const meta = commands.subscriptions.metadata;

      it('should have subscription-specific actions', () => {
        assert.ok(meta.actions.plans);
        assert.ok(meta.actions.plan);
        assert.ok(meta.actions.cycles);
        assert.ok(meta.actions.events);
      });
    });

    describe('stablecoin', () => {
      const meta = commands.stablecoin.metadata;

      it('should have stablecoin-specific actions', () => {
        assert.ok(meta.actions.wallet);
        assert.ok(meta.actions.balance);
        assert.ok(meta.actions.pay);
        assert.ok(meta.actions.chains);
      });
    });

    describe('currency', () => {
      const meta = commands.currency.metadata;

      it('should have currency-specific actions', () => {
        assert.ok(meta.actions.rate);
        assert.ok(meta.actions.rates);
        assert.ok(meta.actions.convert);
        assert.ok(meta.actions.settings);
      });
    });

    describe('tax', () => {
      const meta = commands.tax.metadata;

      it('should have tax-specific actions', () => {
        assert.ok(meta.actions.rate);
        assert.ok(meta.actions.item);
        assert.ok(meta.actions.providers);
        assert.ok(meta.actions.exemptions);
      });
    });

    describe('treasury', () => {
      const meta = commands.treasury.metadata;

      it('should have treasury-specific actions', () => {
        assert.ok(meta.actions.balance);
        assert.ok(meta.actions.ledger);
        assert.ok(meta.actions.deposit);
        assert.ok(meta.actions['register-token']);
      });
    });

    describe('vector', () => {
      const meta = commands.vector.metadata;

      it('should have vector-specific actions', () => {
        assert.ok(meta.actions['search-products']);
        assert.ok(meta.actions['search-customers']);
        assert.ok(meta.actions.stats);
        assert.ok(meta.actions['reindex-all']);
      });
    });

    describe('payments', () => {
      const meta = commands.payments.metadata;

      it('should have payment-specific actions', () => {
        assert.ok(meta.actions.complete);
        assert.ok(meta.actions.providers);
        assert.ok(meta.actions.intents);
        assert.ok(meta.actions.reconcile);
      });
    });

    describe('shipments', () => {
      const meta = commands.shipments.metadata;

      it('should have shipment-specific actions', () => {
        assert.ok(meta.actions.providers);
        assert.ok(meta.actions.rates);
        assert.ok(meta.actions.labels);
        assert.ok(meta.actions.track);
      });
    });

    describe('suppliers', () => {
      const meta = commands.suppliers.metadata;

      it('should have supplier-specific actions', () => {
        assert.ok(meta.actions.orders);
        assert.ok(meta.actions['create-order']);
        assert.ok(meta.actions.approve);
        assert.ok(meta.actions.send);
      });
    });

    describe('invoices', () => {
      const meta = commands.invoices.metadata;

      it('should have invoice-specific actions', () => {
        assert.ok(meta.actions.create);
        assert.ok(meta.actions.send);
        assert.ok(meta.actions.pay);
        assert.ok(meta.actions.overdue);
      });
    });

    describe('warranties', () => {
      const meta = commands.warranties.metadata;

      it('should have warranty-specific actions', () => {
        assert.ok(meta.actions.create);
        assert.ok(meta.actions.claim);
        assert.ok(meta.actions.approve);
        assert.ok(meta.actions.complete);
      });
    });
  });

  describe('help output', () => {
    it('should include the expanded command resources', () => {
      const help = generateHelp();
      for (const resource of ['carts', 'checkout', 'analytics', 'loyalty', 'gift-cards', 'store-credits', 'warehouse', 'receiving', 'fulfillment', 'accounts-payable', 'accounts-receivable', 'general-ledger', 'cost-accounting', 'credit', 'backorders', 'lots', 'serials', 'quality', 'reviews', 'wishlists', 'segments', 'catalog', 'fraud', 'audit', 'manufacturing', 'custom-objects', 'connectors', 'shipping-zones', 'compliance', 'policies', 'sync', 'circuit-breaker', 'erc8004', 'import', 'proofs', 'promotions', 'stablecoin', 'subscriptions', 'currency', 'tax', 'treasury', 'vector', 'payments', 'shipments', 'suppliers', 'invoices', 'warranties']) {
        assert.ok(help.includes(resource), `help should include ${resource}`);
      }
    });
  });

  describe('error handling', () => {
    it('should throw descriptive errors for unknown actions', async () => {
      const mockContext = {
        commerce: {},
        output: { table: () => '' },
        jsonOutput: false,
        resolveId: async (id) => id
      };

      try {
        await commands.customers.execute('unknown_action', [], mockContext);
        assert.fail('Should have thrown');
      } catch (error) {
        assert.ok(error.message.includes('Unknown action'));
        assert.ok(error.message.includes('Available actions'));
      }
    });

    it('should throw descriptive errors for missing arguments', async () => {
      const mockContext = {
        commerce: {
          customers: {
            get: async () => null,
            getByEmail: async () => null
          }
        },
        output: { table: () => '' },
        jsonOutput: false,
        resolveId: async (id) => id
      };

      try {
        await commands.customers.execute('get', [], mockContext);
        assert.fail('Should have thrown');
      } catch (error) {
        assert.ok(error.message.includes('Usage'));
      }
    });
  });
});
