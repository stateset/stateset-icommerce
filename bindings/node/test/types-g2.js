/**
 * Group-2 TypeScript surface: literal unions and paginated `list()` methods.
 *
 * The declaration half reads `index.d.ts` (after a build + postbuild) and
 * asserts the hand-written unions from `scripts/types/g2.d.ts` are present
 * and referenced from the generated interfaces. The behavioural half runs
 * the paginated lists against a fresh in-memory store.
 */

const { Commerce } = require('../index.js');
const assert = require('node:assert/strict');
const { test } = require('node:test');
const fs = require('node:fs');
const path = require('node:path');

const dts = fs.readFileSync(path.join(__dirname, '..', 'index.d.ts'), 'utf8');

/** Every union the g2 fragment declares. */
const UNIONS = [
  'LotStatus', 'LotStatusInput', 'SerialStatus', 'SerialStatusInput',
  'InspectionType', 'InspectionTypeInput', 'InspectionStatus', 'NcrSource', 'NcrSourceInput',
  'NcrSeverity', 'NcrSeverityInput', 'NcrStatus', 'QualityHoldType', 'QualityHoldTypeInput',
  'QualityHoldTypeFilter', 'QualityHoldStatus',
  'WarehouseType', 'WarehouseTypeInput', 'WarehouseTypeFilter', 'WarehouseLocationType',
  'WarehouseLocationTypeInput', 'ReceiptType', 'ReceiptTypeInput', 'ReceiptTypeFilter',
  'ReceiptStatus', 'ReceiptStatusInput', 'WaveStatus', 'WaveStatusInput', 'PickTaskStatus',
  'PickTaskStatusInput',
  'BillStatus', 'BillStatusInput', 'ThreeWayMatchStatus', 'CreditMemoStatus', 'CreditMemoStatusInput',
  'CreditMemoReason', 'CreditMemoReasonInput', 'CreditMemoReasonFilter', 'CostMethod', 'CostMethodInput',
  'CostMethodFilter', 'CreditAccountStatus', 'CreditAccountStatusInput', 'BackorderStatus',
  'BackorderStatusInput', 'BackorderPriority', 'BackorderPriorityInput', 'BackorderPriorityFilter',
  'GlAccountType', 'GlAccountTypeInput', 'GlAccountTypeFilter', 'GlAccountStatus', 'GlAccountStatusInput',
  'GlJournalEntryStatus', 'GlJournalEntryStatusInput', 'GlPeriodStatus', 'CloseMonthStepStatus',
  'GlBalanceSide',
  'SubscriptionPlanStatus', 'SubscriptionBillingInterval', 'SubscriptionBillingIntervalInput',
  'SubscriptionStatus', 'BillingCycleStatus', 'SubscriptionEventType',
  'PromotionType', 'PromotionTypeInput', 'PromotionTrigger', 'PromotionTriggerInput', 'PromotionTarget',
  'PromotionTargetInput', 'PromotionStacking', 'PromotionStackingInput', 'PromotionStatus', 'CouponStatus',
  'TaxType', 'ProductTaxCategory', 'TaxJurisdictionLevel', 'TaxExemptionType', 'TaxExemptionTypeInput',
  'TaxCalculationMethod', 'TaxCompoundMethod',
  'X402Network', 'X402NetworkInput', 'X402Asset', 'X402AssetInput', 'X402IntentStatus',
  'X402IntentStatusInput', 'X402SignatureScheme', 'X402TrustLevel', 'X402TrustLevelInput', 'X402A2ASkill',
  'X402A2ASkillInput', 'X402CreditDirection', 'X402CreditDirectionInput',
  'VectorEntityType', 'VesHashDomain',
  'GiftCardStatus', 'GiftCardTransactionType', 'StoreCreditStatus', 'StoreCreditReason',
  'StoreCreditTransactionType', 'ReviewStatus', 'SegmentType', 'SegmentOperator', 'LoyaltyProgramStatus',
  'LoyaltyTransactionType', 'LoyaltyRewardType',
];

/** `[interface, field, type]` triples that must render with the union. */
const FIELDS = [
  ['LotOutput', 'status', 'LotStatus'],
  ['LotFilterInput', 'status?', 'LotStatusInput'],
  ['SerialOutput', 'status', 'SerialStatus'],
  ['SerialFilterInput', 'status?', 'SerialStatusInput'],
  ['InspectionOutput', 'inspectionType', 'InspectionType'],
  ['NcrOutput', 'severity', 'NcrSeverity'],
  ['QualityHoldOutput', 'status', 'QualityHoldStatus'],
  ['WarehouseOutput', 'warehouseType', 'WarehouseType'],
  ['LocationOutput', 'locationType', 'WarehouseLocationType'],
  ['ReceiptOutput', 'status', 'ReceiptStatus'],
  ['WaveOutput', 'status', 'WaveStatus'],
  ['PickTaskOutput', 'status', 'PickTaskStatus'],
  ['BillOutput', 'status', 'BillStatus'],
  ['ThreeWayMatchOutput', 'matchStatus', 'ThreeWayMatchStatus'],
  ['CreditMemoOutput', 'reason', 'CreditMemoReason'],
  ['ItemCostOutput', 'costMethod', 'CostMethod'],
  ['CreditAccountOutput', 'status', 'CreditAccountStatus'],
  ['BackorderOutput', 'priority', 'BackorderPriority'],
  ['GlAccountOutput', 'accountType', 'GlAccountType'],
  ['JournalEntryOutput', 'status', 'GlJournalEntryStatus'],
  ['GlPeriodOutput', 'status', 'GlPeriodStatus'],
  ['CloseMonthStepOutput', 'status', 'CloseMonthStepStatus'],
  ['SubscriptionPlanOutput', 'billingInterval', 'SubscriptionBillingInterval'],
  ['CreateSubscriptionPlanInput', 'billingInterval', 'SubscriptionBillingIntervalInput'],
  ['SubscriptionOutput', 'status', 'SubscriptionStatus'],
  ['SubscriptionEventOutput', 'eventType', 'SubscriptionEventType'],
  ['PromotionOutput', 'promotionType', 'PromotionType'],
  ['CreatePromotionInput', 'promotionType?', 'PromotionTypeInput'],
  ['AppliedPromotionOutput', 'discountType', 'PromotionType'],
  ['CouponOutput', 'status', 'CouponStatus'],
  ['TaxRateOutput', 'taxType', 'TaxType'],
  ['TaxExemptionOutput', 'exemptCategories', 'ProductTaxCategory[]'],
  ['TaxSettingsOutput', 'compoundMethod', 'TaxCompoundMethod'],
  ['X402IntentOutput', 'asset', 'X402Asset'],
  ['X402IntentOutput', 'network', 'X402Network'],
  ['X402CreateIntentInput', 'network?', 'X402NetworkInput'],
  ['X402AgentCardOutput', 'a2ASkills', 'X402A2ASkill[]'],
  ['X402AgentCardOutput', 'trustLevel', 'X402TrustLevel'],
  ['X402CreditTransactionOutput', 'direction', 'X402CreditDirection'],
  ['GiftCardOutput', 'status', 'GiftCardStatus'],
  ['StoreCreditOutput', 'reason', 'StoreCreditReason'],
  ['ReviewOutput', 'status', 'ReviewStatus'],
  ['SegmentRuleOutput', 'operator', 'SegmentOperator'],
  ['SegmentOutput', 'segmentType', 'SegmentType'],
  ['LoyaltyTransactionOutput', 'transactionType', 'LoyaltyTransactionType'],
  ['RewardOutput', 'rewardType', 'LoyaltyRewardType'],
];

/** `[class, method, filterInterface, outputInterface]` for every paginated list. */
const LISTS = [
  ['Lots', 'list', 'LotFilterInput', 'LotOutput'],
  ['Serials', 'list', 'SerialFilterInput', 'SerialOutput'],
  ['Quality', 'listHolds', 'QualityHoldFilterInput', 'QualityHoldOutput'],
  ['Warehouse', 'listWarehouses', 'WarehouseFilterInput', 'WarehouseOutput'],
  ['Receiving', 'listReceipts', 'ReceiptFilterInput', 'ReceiptOutput'],
  ['Fulfillment', 'listWaves', 'WaveFilterInput', 'WaveOutput'],
  ['Fulfillment', 'listPicks', 'PickTaskFilterInput', 'PickTaskOutput'],
  ['AccountsPayable', 'listBills', 'BillFilterInput', 'BillOutput'],
  ['AccountsReceivable', 'listCreditMemos', 'CreditMemoFilterInput', 'CreditMemoOutput'],
  ['CostAccounting', 'listItemCosts', 'ItemCostFilterInput', 'ItemCostOutput'],
  ['Credit', 'listCreditAccounts', 'CreditAccountFilterInput', 'CreditAccountOutput'],
  ['Backorders', 'listBackorders', 'BackorderFilterInput', 'BackorderOutput'],
  ['GeneralLedger', 'listAccounts', 'GlAccountFilterInput', 'GlAccountOutput'],
  ['GeneralLedger', 'listJournalEntries', 'JournalEntryFilterInput', 'JournalEntryOutput'],
];

function block(kind, name) {
  // napi emits `export interface X {` and `export declare class X {`.
  const prefix = kind === 'class' ? 'export declare class' : `export ${kind}`;
  const re = new RegExp(`^${prefix} ${name} \\{[\\s\\S]*?^\\}`, 'm');
  const m = dts.match(re);
  assert.ok(m, `${kind} ${name} is declared in index.d.ts`);
  return m[0];
}

test('g2 declarations: every union is declared exactly once with a doc comment', () => {
  for (const name of UNIONS) {
    const decls = dts.match(new RegExp(`^export type ${name} = `, 'gm')) || [];
    assert.equal(decls.length, 1, `export type ${name} declared once (found ${decls.length})`);
    const idx = dts.indexOf(`export type ${name} = `);
    const before = dts.slice(0, idx).trimEnd();
    assert.ok(before.endsWith('*/'), `${name} carries a doc comment`);
  }
});

test('g2 declarations: unions spell the wire strings the engine renders', () => {
  const union = (name) => dts.match(new RegExp(`^export type ${name} = (.*)$`, 'm'))[1];
  // Debug-rendered outputs are PascalCase; strum-rendered ones are snake_case.
  assert.match(union('LotStatus'), /'OnHold'/);
  assert.match(union('SerialStatus'), /'InProduction'/);
  assert.match(union('BackorderStatus'), /'ReadyToShip'/);
  // strum Display picks the longest `serialize` alias; the binding does not paper over it.
  assert.match(union('SubscriptionBillingInterval'), /'yearly'/);
  assert.match(union('SubscriptionBillingInterval'), /'bi-weekly'/);
  assert.match(union('X402Asset'), /'tether'/);
  assert.match(union('X402Network'), /'arc-testnet'/);
  // Promotions lower-case the Debug form, so there is no underscore.
  assert.match(union('PromotionType'), /'percentageoff'/);
  assert.match(union('PromotionTypeInput'), /'percentage_off'/);
});

test('g2 declarations: generated interfaces reference the unions', () => {
  for (const [iface, field, type] of FIELDS) {
    const body = block('interface', iface);
    const escaped = type.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
    assert.match(
      body,
      new RegExp(`^  ${field.replace('?', '\\?')}: ${escaped}$`, 'm'),
      `${iface}.${field} is typed ${type}`,
    );
  }
});

test('g2 declarations: any-typed arguments are narrowed', () => {
  assert.match(block('class', 'VectorSearch'), /clear\(entityType: VectorEntityType\): Promise<number>/);
  assert.match(dts, /^export declare function domainHash\(domain: VesHashDomain, data: Buffer\): Buffer$/m);
  assert.match(
    block('class', 'X402'),
    /discoverAgents\(network\?: X402NetworkInput, asset\?: X402AssetInput, skill\?: X402A2ASkillInput, trustLevel\?: X402TrustLevelInput\)/,
  );
});

test('g2 declarations: every paginated list takes an optional filter object', () => {
  for (const [cls, method, filter, output] of LISTS) {
    const body = block('class', cls);
    assert.match(
      body,
      new RegExp(`^  ${method}\\(filter\\?: ${filter} \\| undefined \\| null\\): Promise<Array<${output}>>$`, 'm'),
      `${cls}.${method} accepts ${filter}`,
    );
    const filterBody = block('interface', filter);
    assert.match(filterBody, /^  limit\?: number$/m, `${filter}.limit`);
    assert.match(filterBody, /^  offset\?: number$/m, `${filter}.offset`);
  }
});

async function rejectsWith(code, promise) {
  await assert.rejects(promise, (err) => {
    assert.equal(err.code, code, `expected ${code}, got ${err.code}: ${err.message}`);
    return true;
  });
}

test('g2 pagination: lots', async (t) => {
  const commerce = new Commerce(':memory:');
  for (const sku of ['G2-LOT-A', 'G2-LOT-B', 'G2-LOT-C']) {
    await commerce.lots.create({ sku, quantityProduced: 10 });
  }

  await t.test('no argument keeps listing everything', async () => {
    assert.equal((await commerce.lots.list()).length, 3);
    assert.equal((await commerce.lots.list({})).length, 3);
  });

  await t.test('limit and offset page the result', async () => {
    assert.equal((await commerce.lots.list({ limit: 2 })).length, 2);
    assert.equal((await commerce.lots.list({ limit: 2, offset: 2 })).length, 1);
    assert.equal((await commerce.lots.list({ offset: 3 })).length, 0);
  });

  await t.test('engine filters are wired', async () => {
    const bySku = await commerce.lots.list({ sku: 'G2-LOT-B' });
    assert.equal(bySku.length, 1);
    assert.equal(bySku[0].sku, 'G2-LOT-B');
    assert.equal(bySku[0].status, 'Active');
  });

  await t.test('status accepts the rendered form and the engine form', async () => {
    assert.equal((await commerce.lots.list({ status: 'Active' })).length, 3);
    assert.equal((await commerce.lots.list({ status: 'active' })).length, 3);
    assert.equal((await commerce.lots.list({ status: 'OnHold' })).length, 0);
    assert.equal((await commerce.lots.list({ status: 'on_hold' })).length, 0);
  });

  await t.test('malformed filter values are refused, never dropped', async () => {
    await rejectsWith('VALIDATION', commerce.lots.list({ status: 'bogus' }));
    await rejectsWith('VALIDATION', commerce.lots.list({ supplierId: 'not-a-uuid' }));
    await rejectsWith('VALIDATION', commerce.lots.list({ expiringBefore: 'yesterday' }));
  });
});

test('g2 pagination: serials', async (t) => {
  const commerce = new Commerce(':memory:');
  for (const serial of ['G2-SN-1', 'G2-SN-2', 'G2-SN-3']) {
    await commerce.serials.create({ serial, sku: 'G2-SER' });
  }
  await commerce.serials.create({ serial: 'G2-OTHER', sku: 'G2-SER-OTHER' });

  await t.test('no argument keeps listing everything', async () => {
    assert.equal((await commerce.serials.list()).length, 4);
  });

  await t.test('limit, offset and filters', async () => {
    assert.equal((await commerce.serials.list({ limit: 2 })).length, 2);
    assert.equal((await commerce.serials.list({ limit: 10, offset: 3 })).length, 1);
    assert.equal((await commerce.serials.list({ sku: 'G2-SER' })).length, 3);
    assert.equal((await commerce.serials.list({ serialPrefix: 'G2-SN' })).length, 3);
    assert.equal((await commerce.serials.list({ status: 'Available' })).length, 4);
    assert.equal((await commerce.serials.list({ status: 'available' })).length, 4);
    assert.equal((await commerce.serials.list({ status: 'InService' })).length, 0);
  });

  await t.test('malformed filter values are refused', async () => {
    await rejectsWith('VALIDATION', commerce.serials.list({ status: 'nope' }));
    await rejectsWith('VALIDATION', commerce.serials.list({ lotId: 'not-a-uuid' }));
    await rejectsWith('VALIDATION', commerce.serials.list({ soldAfter: 'later' }));
  });
});

test('g2 pagination: warehouse, quality holds, item costs, credit accounts', async (t) => {
  const commerce = new Commerce(':memory:');

  await t.test('warehouses', async () => {
    await commerce.warehouse.createWarehouse({ code: 'G2-W1', name: 'One', warehouseType: 'retail' });
    await commerce.warehouse.createWarehouse({ code: 'G2-W2', name: 'Two' });
    assert.equal((await commerce.warehouse.listWarehouses()).length, 2);
    assert.equal((await commerce.warehouse.listWarehouses({ limit: 1 })).length, 1);
    const retail = await commerce.warehouse.listWarehouses({ warehouseType: 'Retail' });
    assert.equal(retail.length, 1);
    assert.equal(retail[0].warehouseType, 'Retail');
    assert.equal((await commerce.warehouse.listWarehouses({ warehouseType: 'retail' })).length, 1);
    await rejectsWith('VALIDATION', commerce.warehouse.listWarehouses({ warehouseType: 'igloo' }));
  });

  await t.test('quality holds', async () => {
    await commerce.quality.createHold({ sku: 'G2-H', quantityHeld: 1, reason: 'a', holdType: 'damaged' });
    await commerce.quality.createHold({ sku: 'G2-H', quantityHeld: 1, reason: 'b', holdType: 'recall' });
    assert.equal((await commerce.quality.listHolds()).length, 2);
    assert.equal((await commerce.quality.listHolds({ limit: 1 })).length, 1);
    const damaged = await commerce.quality.listHolds({ holdType: 'Damaged' });
    assert.equal(damaged.length, 1);
    assert.equal(damaged[0].holdType, 'Damaged');
    assert.equal(damaged[0].status, 'held');
    assert.equal((await commerce.quality.listHolds({ holdType: 'damaged' })).length, 1);
    await rejectsWith('VALIDATION', commerce.quality.listHolds({ holdType: 'sticky' }));
  });

  await t.test('item costs', async () => {
    await commerce.costAccounting.setItemCost({ sku: 'G2-C1', costMethod: 'fifo', standardCost: 1 });
    await commerce.costAccounting.setItemCost({ sku: 'G2-C2', costMethod: 'average', standardCost: 1 });
    assert.equal((await commerce.costAccounting.listItemCosts()).length, 2);
    assert.equal((await commerce.costAccounting.listItemCosts({ limit: 1 })).length, 1);
    const fifo = await commerce.costAccounting.listItemCosts({ costMethod: 'Fifo' });
    assert.equal(fifo.length, 1);
    assert.equal(fifo[0].costMethod, 'Fifo');
    assert.equal((await commerce.costAccounting.listItemCosts({ costMethod: 'fifo' })).length, 1);
  });

  await t.test('credit accounts', async () => {
    const ids = [];
    for (const email of ['g2a@example.com', 'g2b@example.com']) {
      const c = await commerce.customers.create({ email, firstName: 'G', lastName: 'Two' });
      ids.push(c.id);
      await commerce.credit.createCreditAccount({ customerId: c.id, creditLimit: 100 });
    }
    assert.equal((await commerce.credit.listCreditAccounts()).length, 2);
    assert.equal((await commerce.credit.listCreditAccounts({ limit: 1 })).length, 1);
    assert.equal((await commerce.credit.listCreditAccounts({ customerId: ids[0] })).length, 1);
    assert.equal((await commerce.credit.listCreditAccounts({ status: 'Active' })).length, 2);
    assert.equal((await commerce.credit.listCreditAccounts({ status: 'active' })).length, 2);
    await rejectsWith('VALIDATION', commerce.credit.listCreditAccounts({ customerId: 'nope' }));
  });
});

test('g2 pagination: general ledger', async (t) => {
  const commerce = new Commerce(':memory:');
  const seeded = await commerce.generalLedger.initializeChartOfAccounts();
  assert.ok(seeded.length > 3, 'the default chart has more than three accounts');

  await t.test('accounts', async () => {
    assert.equal((await commerce.generalLedger.listAccounts()).length, seeded.length);
    assert.equal((await commerce.generalLedger.listAccounts({ limit: 3 })).length, 3);
    const assets = await commerce.generalLedger.listAccounts({ accountType: 'Asset' });
    assert.ok(assets.length > 0);
    assert.ok(assets.every((a) => a.accountType === 'Asset'));
    assert.equal((await commerce.generalLedger.listAccounts({ accountType: 'asset' })).length, assets.length);
    await rejectsWith('VALIDATION', commerce.generalLedger.listAccounts({ accountType: 'treasure' }));
    await rejectsWith('VALIDATION', commerce.generalLedger.listAccounts({ status: 'sleepy' }));
  });

  await t.test('journal entries', async () => {
    assert.deepEqual(await commerce.generalLedger.listJournalEntries(), []);
    assert.deepEqual(await commerce.generalLedger.listJournalEntries({ status: 'Posted', limit: 5 }), []);
    await rejectsWith('VALIDATION', commerce.generalLedger.listJournalEntries({ fromDate: '2026/01/01' }));
    await rejectsWith('VALIDATION', commerce.generalLedger.listJournalEntries({ status: 'lost' }));
  });
});

test('g2 pagination: empty stores still answer filtered lists', async () => {
  const commerce = new Commerce(':memory:');
  assert.deepEqual(await commerce.receiving.listReceipts({ status: 'Expected', limit: 1 }), []);
  assert.deepEqual(await commerce.receiving.listReceipts({ receiptType: 'PurchaseOrder' }), []);
  assert.deepEqual(await commerce.fulfillment.listWaves({ status: 'in_progress' }), []);
  assert.deepEqual(await commerce.fulfillment.listPicks({ status: 'InProgress', offset: 0 }), []);
  assert.deepEqual(await commerce.accountsPayable.listBills({ status: 'PartiallyPaid' }), []);
  assert.deepEqual(await commerce.accountsReceivable.listCreditMemos({ reason: 'ReturnedGoods' }), []);
  assert.deepEqual(await commerce.backorder.listBackorders({ status: 'ready_to_ship', priority: 'High' }), []);
  await rejectsWith('VALIDATION', commerce.receiving.listReceipts({ status: 'teleported' }));
  await rejectsWith('VALIDATION', commerce.fulfillment.listPicks({ waveId: 'not-a-uuid' }));
  await rejectsWith('VALIDATION', commerce.accountsPayable.listBills({ fromDate: 'soon' }));
  await rejectsWith('VALIDATION', commerce.backorder.listBackorders({ priority: 'urgent-ish' }));
});
