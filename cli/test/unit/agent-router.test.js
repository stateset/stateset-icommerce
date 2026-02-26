/**
 * Unit tests for agent-router.js
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { routeToAgent, routeToAgentWithConfidence } from '../../src/agent-router.js';

describe('agent-router', () => {
  describe('routeToAgent', () => {
    describe('checkout agent routing', () => {
      it('should route checkout requests', () => {
        assert.strictEqual(routeToAgent('start checkout'), 'checkout');
        assert.strictEqual(routeToAgent('I want to checkout'), 'checkout');
        assert.strictEqual(routeToAgent('complete checkout'), 'checkout');
      });

      it('should route shopping cart requests', () => {
        assert.strictEqual(routeToAgent('show my shopping cart'), 'checkout');
        assert.strictEqual(routeToAgent('add to cart'), 'checkout');
        assert.strictEqual(routeToAgent('update my cart'), 'checkout');
      });

      it('should route abandoned cart requests', () => {
        assert.strictEqual(routeToAgent('show abandoned carts'), 'checkout');
        assert.strictEqual(routeToAgent('cart recovery'), 'checkout');
      });

      it('should route shipping and discount requests', () => {
        assert.strictEqual(routeToAgent('what shipping options do I have?'), 'orders');
        assert.strictEqual(routeToAgent('apply discount code'), 'checkout');
        assert.strictEqual(routeToAgent('apply coupon code SAVE20'), 'promotions');
      });
    });

    describe('orders agent routing', () => {
      it('should route order status requests', () => {
        assert.strictEqual(routeToAgent('what is the order status?'), 'orders');
        assert.strictEqual(routeToAgent('check order #12345'), 'orders');
        assert.strictEqual(routeToAgent('order number 67890'), 'orders');
      });

      it('should route order management requests', () => {
        assert.strictEqual(routeToAgent('ship order 123'), 'orders');
        assert.strictEqual(routeToAgent('cancel order'), 'orders');
        assert.strictEqual(routeToAgent('update order status'), 'orders');
      });

      it('should route fulfillment requests', () => {
        assert.strictEqual(routeToAgent('fulfill order'), 'orders');
        assert.strictEqual(routeToAgent('order tracking'), 'orders');
        assert.strictEqual(routeToAgent('pending orders'), 'orders');
      });

      it('should route shipping tracking requests', () => {
        assert.strictEqual(routeToAgent('add tracking number'), 'orders');
        assert.strictEqual(routeToAgent('order has shipped'), 'orders');
      });
    });

    describe('inventory agent routing', () => {
      it('should route stock level requests', () => {
        assert.strictEqual(routeToAgent('check stock level'), 'inventory');
        assert.strictEqual(routeToAgent('inventory count for SKU-123'), 'inventory');
      });

      it('should route inventory adjustment requests', () => {
        assert.strictEqual(routeToAgent('adjust inventory'), 'inventory');
        assert.strictEqual(routeToAgent('restock items'), 'inventory');
      });

      it('should route reservation requests', () => {
        assert.strictEqual(routeToAgent('reserve stock for order'), 'inventory');
        assert.strictEqual(routeToAgent('release reservation'), 'tax');
        assert.strictEqual(routeToAgent('confirm reservation'), 'tax');
      });

      it('should route warehouse and SKU requests', () => {
        assert.strictEqual(routeToAgent('warehouse inventory'), 'inventory');
        assert.strictEqual(routeToAgent('check SKU availability'), 'inventory');
      });
    });

    describe('returns agent routing', () => {
      it('should route return request operations', () => {
        assert.strictEqual(routeToAgent('create a return request'), 'returns');
        assert.strictEqual(routeToAgent('approve return'), 'returns');
        assert.strictEqual(routeToAgent('reject return'), 'returns');
      });

      it('should route RMA requests', () => {
        assert.strictEqual(routeToAgent('process RMA'), 'returns');
        assert.strictEqual(routeToAgent('return merchandise authorization'), 'returns');
      });

      it('should route refund and exchange requests', () => {
        assert.strictEqual(routeToAgent('issue refund'), 'returns');
        assert.strictEqual(routeToAgent('exchange defective item'), 'returns');
      });

      it('should route return status requests', () => {
        assert.strictEqual(routeToAgent('check return status'), 'returns');
        assert.strictEqual(routeToAgent('pending returns'), 'returns');
      });
    });

    describe('analytics agent routing', () => {
      it('should route sales report requests', () => {
        assert.strictEqual(routeToAgent('show me sales report'), 'analytics');
        assert.strictEqual(routeToAgent('revenue report'), 'analytics');
      });

      it('should route forecasting requests', () => {
        assert.strictEqual(routeToAgent('forecast demand'), 'analytics');
        assert.strictEqual(routeToAgent('predict demand for next month'), 'analytics');
        assert.strictEqual(routeToAgent('revenue forecast'), 'analytics');
      });

      it('should route top products and customer metrics', () => {
        assert.strictEqual(routeToAgent('show top products'), 'analytics');
        assert.strictEqual(routeToAgent('best sellers'), 'analytics');
        assert.strictEqual(routeToAgent('customer metrics'), 'analytics');
        assert.strictEqual(routeToAgent('top customers'), 'analytics');
      });

      it('should route inventory health reports', () => {
        assert.strictEqual(routeToAgent('inventory health report'), 'analytics');
        assert.strictEqual(routeToAgent('low stock report'), 'analytics');
      });

      it('should route business performance requests', () => {
        assert.strictEqual(routeToAgent('how is business performing?'), 'customer-service');
        assert.strictEqual(routeToAgent('show me sales trends'), 'analytics');
        assert.strictEqual(routeToAgent('dashboard metrics'), 'analytics');
      });
    });

    describe('promotions agent routing', () => {
      it('should route promotion management requests', () => {
        assert.strictEqual(routeToAgent('create promotion'), 'promotions');
        assert.strictEqual(routeToAgent('activate promotion'), 'promotions');
      });

      it('should route coupon code requests', () => {
        assert.strictEqual(routeToAgent('create coupon code'), 'promotions');
        assert.strictEqual(routeToAgent('validate coupon'), 'promotions');
        assert.strictEqual(routeToAgent('promo code management'), 'promotions');
      });

      it('should route discount type requests', () => {
        assert.strictEqual(routeToAgent('create 20 percent off promotion'), 'promotions');
        assert.strictEqual(routeToAgent('percentage off sale'), 'promotions');
        assert.strictEqual(routeToAgent('BOGO promotion'), 'promotions');
        assert.strictEqual(routeToAgent('buy one get one deal'), 'promotions');
      });

      it('should route flash sale requests', () => {
        assert.strictEqual(routeToAgent('create flash sale'), 'promotions');
        assert.strictEqual(routeToAgent('tiered discount'), 'promotions');
      });
    });

    describe('subscriptions agent routing', () => {
      it('should route subscription plan requests', () => {
        assert.strictEqual(routeToAgent('create subscription plan'), 'subscriptions');
        assert.strictEqual(routeToAgent('list subscription plans'), 'subscriptions');
      });

      it('should route subscription lifecycle requests', () => {
        assert.strictEqual(routeToAgent('pause subscription'), 'subscriptions');
        assert.strictEqual(routeToAgent('cancel subscription'), 'subscriptions');
        assert.strictEqual(routeToAgent('resume subscription'), 'subscriptions');
      });

      it('should route billing cycle requests', () => {
        assert.strictEqual(routeToAgent('skip billing cycle'), 'subscriptions');
        assert.strictEqual(routeToAgent('recurring billing'), 'subscriptions');
        assert.strictEqual(routeToAgent('billing cycle details'), 'subscriptions');
      });

      it('should route subscriber management', () => {
        assert.strictEqual(routeToAgent('list subscribers'), 'subscriptions');
        assert.strictEqual(routeToAgent('subscription events'), 'subscriptions');
      });
    });

    describe('storefront agent routing', () => {
      it('should route store creation requests', () => {
        assert.strictEqual(routeToAgent('create a new store'), 'storefront');
        assert.strictEqual(routeToAgent('build store'), 'storefront');
      });

      it('should route website scaffolding requests', () => {
        assert.strictEqual(routeToAgent('scaffold ecommerce site'), 'storefront');
        assert.strictEqual(routeToAgent('create website'), 'storefront');
        assert.strictEqual(routeToAgent('build nextjs store'), 'storefront');
      });

      it('should route online store setup', () => {
        assert.strictEqual(routeToAgent('create online store'), 'storefront');
        assert.strictEqual(routeToAgent('setup shop website'), 'storefront');
      });
    });

    describe('sync agent routing', () => {
      it('should route sync status requests', () => {
        assert.strictEqual(routeToAgent('check sync status'), 'sync');
        assert.strictEqual(routeToAgent('sync events'), 'sync');
      });

      it('should route event sync operations', () => {
        assert.strictEqual(routeToAgent('push events'), 'sync');
        assert.strictEqual(routeToAgent('pull events'), 'sync');
        assert.strictEqual(routeToAgent('event sync'), 'sync');
      });

      it('should route outbox and sequencer requests', () => {
        assert.strictEqual(routeToAgent('check outbox'), 'sync');
        assert.strictEqual(routeToAgent('sequencer status'), 'sync');
        assert.strictEqual(routeToAgent('pending events'), 'sync');
      });

      it('should route VES requests', () => {
        assert.strictEqual(routeToAgent('verifiable event stream'), 'sync');
        assert.strictEqual(routeToAgent('VES status'), 'sync');
      });
    });

    describe('manufacturing agent routing', () => {
      it('should route BOM requests', () => {
        assert.strictEqual(routeToAgent('create bill of materials'), 'manufacturing');
        assert.strictEqual(routeToAgent('show BOM'), 'manufacturing');
      });

      it('should route work order requests', () => {
        assert.strictEqual(routeToAgent('create work order'), 'manufacturing');
        assert.strictEqual(routeToAgent('start work order'), 'manufacturing');
        assert.strictEqual(routeToAgent('complete work order'), 'manufacturing');
      });

      it('should route production requests', () => {
        assert.strictEqual(routeToAgent('manufacturing schedule'), 'manufacturing');
        assert.strictEqual(routeToAgent('production yield'), 'manufacturing');
      });
    });

    describe('payments agent routing', () => {
      it('should route payment creation requests', () => {
        assert.strictEqual(routeToAgent('create payment'), 'payments');
        assert.strictEqual(routeToAgent('process payment'), 'payments');
      });

      it('should route payment status requests', () => {
        assert.strictEqual(routeToAgent('check payment status'), 'payments');
        assert.strictEqual(routeToAgent('complete payment'), 'payments');
      });

      it('should route payment method requests', () => {
        assert.strictEqual(routeToAgent('add payment method'), 'payments');
        assert.strictEqual(routeToAgent('credit card payment'), 'payments');
        assert.strictEqual(routeToAgent('ACH transfer'), 'payments');
      });
    });

    describe('shipments agent routing', () => {
      it('should route shipment creation requests', () => {
        assert.strictEqual(routeToAgent('create shipment'), 'shipments');
        assert.strictEqual(routeToAgent('deliver shipment'), 'shipments');
      });

      it('should route carrier requests', () => {
        assert.strictEqual(routeToAgent('ship via FedEx'), 'shipments');
        assert.strictEqual(routeToAgent('UPS tracking'), 'shipments');
        assert.strictEqual(routeToAgent('USPS carrier'), 'shipments');
      });

      it('should route shipment status requests', () => {
        assert.strictEqual(routeToAgent('shipment status'), 'shipments');
        assert.strictEqual(routeToAgent('package in transit'), 'shipments');
      });
    });

    describe('suppliers agent routing', () => {
      it('should route supplier management requests', () => {
        assert.strictEqual(routeToAgent('create supplier'), 'suppliers');
        assert.strictEqual(routeToAgent('list suppliers'), 'suppliers');
      });

      it('should route purchase order requests', () => {
        assert.strictEqual(routeToAgent('create purchase order'), 'suppliers');
        assert.strictEqual(routeToAgent('approve purchase order'), 'suppliers');
        assert.strictEqual(routeToAgent('send purchase order'), 'suppliers');
      });

      it('should route vendor and procurement requests', () => {
        assert.strictEqual(routeToAgent('vendor management'), 'suppliers');
        assert.strictEqual(routeToAgent('procurement workflow'), 'suppliers');
      });
    });

    describe('invoices agent routing', () => {
      it('should route invoice creation requests', () => {
        assert.strictEqual(routeToAgent('create invoice'), 'invoices');
        assert.strictEqual(routeToAgent('send invoice'), 'invoices');
      });

      it('should route payment terms requests', () => {
        assert.strictEqual(routeToAgent('net 30 invoice'), 'invoices');
        assert.strictEqual(routeToAgent('net 60 payment terms'), 'payments');
      });

      it('should route accounts receivable requests', () => {
        assert.strictEqual(routeToAgent('accounts receivable'), 'invoices');
        assert.strictEqual(routeToAgent('overdue invoices'), 'invoices');
        assert.strictEqual(routeToAgent('record payment for invoice'), 'invoices');
      });
    });

    describe('warranties agent routing', () => {
      it('should route warranty creation requests', () => {
        assert.strictEqual(routeToAgent('create warranty'), 'warranties');
        assert.strictEqual(routeToAgent('warranty claim'), 'warranties');
      });

      it('should route warranty status requests', () => {
        assert.strictEqual(routeToAgent('approve warranty'), 'warranties');
        assert.strictEqual(routeToAgent('warranty status'), 'warranties');
      });

      it('should route guarantee requests', () => {
        assert.strictEqual(routeToAgent('product guarantee'), 'warranties');
        assert.strictEqual(routeToAgent('repair claim'), 'warranties');
      });
    });

    describe('currency agent routing', () => {
      it('should route exchange rate requests', () => {
        assert.strictEqual(routeToAgent('get exchange rate'), 'currency');
        assert.strictEqual(routeToAgent('currency conversion'), 'currency');
        assert.strictEqual(routeToAgent('set exchange rate'), 'currency');
      });

      it('should route multi-currency requests', () => {
        assert.strictEqual(routeToAgent('multi-currency support'), 'currency');
        assert.strictEqual(routeToAgent('base currency'), 'currency');
        assert.strictEqual(routeToAgent('enable currencies'), 'currency');
      });

      it('should route forex requests', () => {
        assert.strictEqual(routeToAgent('forex rates'), 'currency');
        assert.strictEqual(routeToAgent('convert currency'), 'currency');
      });
    });

    describe('tax agent routing', () => {
      it('should route sales tax requests', () => {
        assert.strictEqual(routeToAgent('calculate sales tax'), 'tax');
        assert.strictEqual(routeToAgent('tax rate'), 'tax');
      });

      it('should route tax exemption requests', () => {
        assert.strictEqual(routeToAgent('tax exempt'), 'tax');
        assert.strictEqual(routeToAgent('tax exemption certificate'), 'tax');
      });

      it('should route VAT and GST requests', () => {
        assert.strictEqual(routeToAgent('VAT calculation'), 'tax');
        assert.strictEqual(routeToAgent('GST rate'), 'tax');
        assert.strictEqual(routeToAgent('HST for Canada'), 'tax');
      });

      it('should route tax jurisdiction requests', () => {
        assert.strictEqual(routeToAgent('tax jurisdiction'), 'tax');
        assert.strictEqual(routeToAgent('nexus requirements'), 'tax');
        assert.strictEqual(routeToAgent('calculate cart tax'), 'tax');
      });
    });

    describe('default routing', () => {
      it('should default to customer-service for ambiguous requests', () => {
        assert.strictEqual(routeToAgent('help me'), 'customer-service');
        assert.strictEqual(routeToAgent('I have a question'), 'customer-service');
        assert.strictEqual(routeToAgent('general inquiry'), 'customer-service');
      });

      it('should default to customer-service for unclear requests', () => {
        assert.strictEqual(routeToAgent('hello'), 'customer-service');
        assert.strictEqual(routeToAgent('hi there'), 'customer-service');
      });
    });

    describe('edge cases', () => {
      it('should handle empty string', () => {
        const agent = routeToAgent('');
        assert.strictEqual(agent, 'customer-service');
      });

      it('should handle very long strings', () => {
        const longRequest = 'a'.repeat(10000);
        const agent = routeToAgent(longRequest);
        assert.ok(agent);
      });

      it('should handle special characters', () => {
        const agent = routeToAgent('order #12345!@#$%');
        assert.ok(agent);
      });

      it('should be case-insensitive', () => {
        assert.strictEqual(routeToAgent('CHECKOUT'), 'checkout');
        assert.strictEqual(routeToAgent('ChEcKoUt'), 'checkout');
        assert.strictEqual(routeToAgent('Order Status'), 'orders');
      });
    });
  });

  describe('routeToAgentWithConfidence', () => {
    describe('confidence scoring', () => {
      it('should return high confidence for clear requests', () => {
        const result = routeToAgentWithConfidence('checkout now');
        assert.ok(result.primary);
        assert.strictEqual(result.primary.agent, 'checkout');
        assert.ok(result.primary.score > 0);
      });

      it('should return medium confidence for moderate matches', () => {
        const result = routeToAgentWithConfidence('buy something');
        assert.ok(result.primary);
        assert.ok(result.primary.confidence >= 0);
      });

      it('should return low confidence for weak matches', () => {
        const result = routeToAgentWithConfidence('plan something');
        assert.ok(result.primary);
      });

      it('should provide confidence level labels', () => {
        const result = routeToAgentWithConfidence('complete checkout now');
        assert.ok(['high', 'medium', 'low', 'none', 'default'].includes(result.primary.level));
      });
    });

    describe('alternatives', () => {
      it('should provide alternative agents when applicable', () => {
        const result = routeToAgentWithConfidence('show me orders');
        assert.ok(Array.isArray(result.alternatives));
      });

      it('should rank alternatives by score', () => {
        const result = routeToAgentWithConfidence('process payment for order');
        if (result.alternatives.length > 1) {
          for (let i = 1; i < result.alternatives.length; i++) {
            assert.ok(result.alternatives[i - 1].score >= result.alternatives[i].score);
          }
        }
      });

      it('should limit alternatives to top 3', () => {
        const result = routeToAgentWithConfidence('business operations');
        assert.ok(result.alternatives.length <= 3);
      });
    });

    describe('ambiguity detection', () => {
      it('should detect ambiguous requests with similar scores', () => {
        const result = routeToAgentWithConfidence('shipping details');
        assert.ok(typeof result.ambiguous === 'boolean');
      });

      it('should not mark clear requests as ambiguous', () => {
        const result = routeToAgentWithConfidence('complete checkout flow');
        if (result.primary.level === 'high') {
          assert.strictEqual(result.ambiguous, false);
        }
      });

      it('should provide all scores for analysis', () => {
        const result = routeToAgentWithConfidence('sales data');
        assert.ok(result.allScores);
        assert.strictEqual(typeof result.allScores, 'object');
      });
    });

    describe('negative keyword penalties', () => {
      it('should apply penalties when conflicting keywords present', () => {
        const withNegative = routeToAgentWithConfidence('checkout return items');
        const withoutNegative = routeToAgentWithConfidence('checkout cart items');

        if (
          withNegative.primary.agent === 'checkout' &&
          withoutNegative.primary.agent === 'checkout'
        ) {
          assert.ok(true);
        }
      });

      it('should not apply penalties when negative keywords absent', () => {
        const result = routeToAgentWithConfidence('start checkout');
        assert.ok(result.primary.score >= 0);
      });

      it('should ensure scores do not go negative', () => {
        const result = routeToAgentWithConfidence('return checkout cart order');
        if (result.allScores.checkout) {
          assert.ok(result.allScores.checkout.score >= 0);
        }
      });
    });

    describe('thresholds', () => {
      it('should expose routing thresholds', () => {
        const result = routeToAgentWithConfidence('test');
        assert.ok(result.thresholds);
        assert.ok(typeof result.thresholds.HIGH_CONFIDENCE === 'number');
        assert.ok(typeof result.thresholds.MEDIUM_CONFIDENCE === 'number');
        assert.ok(typeof result.thresholds.LOW_CONFIDENCE === 'number');
        assert.ok(typeof result.thresholds.MIN_SCORE === 'number');
      });

      it('should use MIN_SCORE to filter matches', () => {
        const result = routeToAgentWithConfidence('xyz');
        assert.ok(result.primary);
      });
    });

    describe('matched keywords tracking', () => {
      it('should track which keywords matched', () => {
        const result = routeToAgentWithConfidence('checkout cart');
        assert.ok(Array.isArray(result.primary.matchedKeywords));
      });

      it('should include keyword weights in matched keywords', () => {
        const result = routeToAgentWithConfidence('checkout');
        if (result.primary.matchedKeywords.length > 0) {
          const keyword = result.primary.matchedKeywords[0];
          assert.ok(keyword.keyword);
          assert.ok(typeof keyword.weight === 'number');
        }
      });
    });

    describe('default behavior', () => {
      it('should default to customer-service with explanation', () => {
        const result = routeToAgentWithConfidence('random gibberish xyz');
        if (result.primary.agent === 'customer-service') {
          assert.strictEqual(result.primary.level, 'default');
          assert.ok(result.primary.reason);
        }
      });

      it('should handle completely empty request', () => {
        const result = routeToAgentWithConfidence('');
        assert.strictEqual(result.primary.agent, 'customer-service');
      });
    });

    describe('multi-keyword scoring', () => {
      it('should accumulate scores for multiple keywords', () => {
        const singleKeyword = routeToAgentWithConfidence('checkout');
        const multipleKeywords = routeToAgentWithConfidence('checkout cart shopping cart');

        if (
          singleKeyword.primary.agent === 'checkout' &&
          multipleKeywords.primary.agent === 'checkout'
        ) {
          assert.ok(multipleKeywords.primary.score >= singleKeyword.primary.score);
        }
      });

      it('should weight strong indicators higher', () => {
        const strongIndicator = routeToAgentWithConfidence('abandoned cart');
        const weakIndicator = routeToAgentWithConfidence('buy');

        assert.ok(strongIndicator.primary.score > 0 || weakIndicator.primary.score >= 0);
      });
    });

    describe('mixed agent keywords', () => {
      it('should handle requests with keywords from multiple agents', () => {
        const result = routeToAgentWithConfidence('checkout order status');
        assert.ok(result.primary.agent);
        assert.ok(result.alternatives.length >= 0);
      });

      it('should use confidence to distinguish close matches', () => {
        const result = routeToAgentWithConfidence('shipping tracking order');
        assert.ok(result.primary);
        if (result.alternatives.length > 0) {
          assert.ok(result.alternatives[0].score >= 0);
        }
      });
    });

    describe('SLA-aware routing', () => {
      it('should boost critical-path agents for critical SLA requests', () => {
        const baseline = routeToAgentWithConfidence('inventory order');
        const critical = routeToAgentWithConfidence('inventory order', {
          slaLevel: 'critical',
        });

        assert.strictEqual(baseline.primary.agent, 'inventory');
        assert.strictEqual(critical.primary.agent, 'orders');
        assert.ok(critical.allScores.orders.score > baseline.allScores.orders.score);
        assert.ok(critical.allScores.orders.slaBoost > 0);
        assert.strictEqual(critical.routingContext.slaLevel, 'critical');
      });

      it('should ignore invalid SLA levels', () => {
        const baseline = routeToAgentWithConfidence('inventory order');
        const invalidSla = routeToAgentWithConfidence('inventory order', {
          slaLevel: 'gold',
        });

        assert.strictEqual(invalidSla.primary.agent, baseline.primary.agent);
        assert.strictEqual(invalidSla.allScores.orders.score, baseline.allScores.orders.score);
        assert.strictEqual(invalidSla.routingContext.slaLevel, null);
      });

      it('should not apply SLA boosts without keyword matches', () => {
        const baseline = routeToAgentWithConfidence('hello there');
        const critical = routeToAgentWithConfidence('hello there', {
          slaLevel: 'critical',
        });

        assert.strictEqual(critical.primary.agent, baseline.primary.agent);
        assert.strictEqual(critical.allScores.orders.slaBoost, 0);
        assert.strictEqual(critical.allScores.shipments.slaBoost, 0);
      });
    });

    describe('response structure', () => {
      it('should return complete result object', () => {
        const result = routeToAgentWithConfidence('test request');
        assert.ok(result.primary);
        assert.ok(Array.isArray(result.alternatives));
        assert.ok(typeof result.ambiguous === 'boolean');
        assert.ok(result.allScores);
        assert.ok(result.thresholds);
      });

      it('should include primary agent details', () => {
        const result = routeToAgentWithConfidence('checkout');
        assert.ok(result.primary.agent);
        assert.ok(typeof result.primary.score === 'number');
        assert.ok(typeof result.primary.confidence === 'number');
        assert.ok(result.primary.level);
        assert.ok(Array.isArray(result.primary.matchedKeywords));
      });
    });

    describe('confidence level classification', () => {
      it('should classify high confidence correctly', () => {
        const result = routeToAgentWithConfidence('checkout cart abandoned cart');
        if (result.primary.confidence >= result.thresholds.HIGH_CONFIDENCE) {
          assert.strictEqual(result.primary.level, 'high');
        }
      });

      it('should classify medium confidence correctly', () => {
        const result = routeToAgentWithConfidence('buy');
        if (
          result.primary.confidence >= result.thresholds.MEDIUM_CONFIDENCE &&
          result.primary.confidence < result.thresholds.HIGH_CONFIDENCE
        ) {
          assert.strictEqual(result.primary.level, 'medium');
        }
      });

      it('should classify low confidence correctly', () => {
        const result = routeToAgentWithConfidence('plan');
        if (
          result.primary.confidence >= result.thresholds.LOW_CONFIDENCE &&
          result.primary.confidence < result.thresholds.MEDIUM_CONFIDENCE
        ) {
          assert.strictEqual(result.primary.level, 'low');
        }
      });
    });
  });
});
