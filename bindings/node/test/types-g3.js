/**
 * TypeScript-surface tests for the finance / procurement / logistics /
 * operations / agentic modules (scripts/types/g3.d.ts).
 *
 * Part 1 reads the built `index.d.ts` and checks that the literal unions are
 * declared and referenced from the right fields, and that the formerly
 * unbounded `listClasses()` / `listRules()` / `listStations()` take a filter.
 * Part 2 exercises that pagination and a few enum wire forms at runtime.
 * Both need a rebuild (`npm run build`) after the Rust side changes.
 */

const { Commerce } = require('../index.js');
const assert = require('node:assert/strict');
const { test } = require('node:test');
const { readFileSync } = require('node:fs');
const path = require('node:path');

const dts = readFileSync(path.join(__dirname, '..', 'index.d.ts'), 'utf8');

/** The body of `export interface Name { ... }` or `export declare class Name { ... }`. */
function block(kind, name) {
  const re = new RegExp(`^export ${kind} ${name} \\{\\n([\\s\\S]*?)^\\}`, 'm');
  const m = re.exec(dts);
  assert.ok(m, `${kind} ${name} should be declared in index.d.ts`);
  return m[1];
}

function assertField(iface, field, tsType, { optional }) {
  const body = block('interface', iface);
  const re = new RegExp(`^  ${field}${optional ? '\\?' : ''}: ${tsType.replace(/[|]/g, '\\|')}$`, 'm');
  assert.match(body, re, `${iface}.${field} should be typed ${tsType}`);
}

/** Members of a `export type X = 'a' | 'b'` union, in declaration order. */
function unionMembers(name) {
  const re = new RegExp(`^export type ${name} =\\n?((?:\\s*\\|?\\s*'[^']*'\\s*\\n?)+)`, 'm');
  const m = re.exec(dts);
  assert.ok(m, `type ${name} should be declared in index.d.ts`);
  return [...m[1].matchAll(/'([^']*)'/g)].map((x) => x[1]);
}

const UNIONS = {
  FixedAssetCategory: [
    'land', 'building', 'machinery', 'equipment', 'vehicle', 'furniture_and_fixtures',
    'computer_hardware', 'software', 'leasehold_improvement', 'other',
  ],
  FixedAssetStatus: ['draft', 'in_service', 'fully_depreciated', 'disposed', 'written_off'],
  DepreciationMethod: ['straight_line', 'declining_balance', 'units_of_production'],
  DepreciationEntryStatus: ['scheduled', 'posted'],
  RecognitionMethod: ['point_in_time', 'ratable_over_time', 'milestone'],
  RevenueContractStatus: ['draft', 'active', 'completed', 'cancelled'],
  RevenueEntryStatus: ['deferred', 'recognized'],
  CycleCountStatus: ['draft', 'in_progress', 'completed', 'cancelled'],
  EdiDirection: ['inbound', 'outbound'],
  EdiStatus: ['pending', 'sent', 'acknowledged', 'processed', 'error'],
  PrepaymentStatus: ['open', 'applied', 'refunded', 'cancelled'],
  PrepaymentTargetType: ['bill', 'payment_obligation'],
  VendorCreditStatus: ['open', 'applied', 'cancelled'],
  VendorCreditTargetType: ['bill', 'payment_obligation'],
  PriceAdjustmentType: ['none', 'percentage_discount', 'percentage_markup'],
  TransferOrderStatus: ['draft', 'pending', 'in_transit', 'partially_received', 'received', 'cancelled'],
  ProductionBatchStatus: ['planned', 'in_progress', 'completed', 'cancelled'],
  InboundShipmentStatus: ['pending', 'in_transit', 'arrived', 'partially_received', 'received', 'cancelled'],
  ActorKind: ['user', 'system', 'integration', 'agent'],
  ChannelType: ['sales_channel', 'fulfillment_channel', 'end_to_end_channel'],
  ChannelStatus: ['active', 'paused', 'deleted'],
  CompanyStatus: ['active', 'inactive'],
  ConversionRuleType: ['SYSTEM', 'SKU'],
  ShippingMethodType: ['flat', 'weight_based', 'price_based', 'calculated', 'free'],
  PrintPayloadKind: ['zpl', 'pdf'],
  PrintJobStatus: ['queued', 'picked_up', 'printed', 'failed'],
  FieldTransform: ['none', 'uppercase', 'lowercase', 'trim'],
  PaymentObligationStatus: ['pending', 'scheduled', 'partially_paid', 'paid', 'cancelled'],
  HealthGrade: ['unknown', 'healthy', 'degraded', 'critical'],
  VendorReturnStatus: ['draft', 'pending', 'processed', 'cancelled'],
  VendorReturnReason: ['defective', 'overage', 'wrong_item', 'other'],
  FraudDecision: ['accept', 'review', 'reject'],
  FraudSignalType: [
    'velocity_spike', 'address_mismatch', 'high_value_first_order', 'geo_ip_anomaly',
    'bin_country_mismatch', 'device_fingerprint', 'proxy_vpn', 'disposable_email',
    'payment_retries', 'unusual_time',
  ],
  SearchTokenizer: ['standard', 'ngram', 'edge', 'keyword'],
  FacetType: ['value', 'range', 'hierarchical'],
  AgentWalletProofType: ['eip_712', 'erc_1271'],
  AgentWalletProofTypeInput: ['eip712', 'eip_712', 'erc1271', 'erc_1271'],
  ImportConflictPolicy: ['skip', 'fail'],
};

test('g3 d.ts: literal unions are declared with the wire value sets', () => {
  for (const [name, members] of Object.entries(UNIONS)) {
    assert.deepEqual(unionMembers(name), members, `type ${name}`);
  }
  assert.match(dts, /^export type DepreciationMethodOutput = DepreciationMethod \| 'unknown'$/m);
  assert.match(dts, /^export type RecognitionMethodOutput = RecognitionMethod \| 'unknown'$/m);
});

test('g3 d.ts: enum-shaped fields reference the unions', () => {
  const req = { optional: false };
  const opt = { optional: true };
  const cases = [
    ['CreateFixedAssetInput', 'category', 'FixedAssetCategory', req],
    ['CreateFixedAssetInput', 'depreciationMethod', 'DepreciationMethod', req],
    ['UpdateFixedAssetInput', 'category', 'FixedAssetCategory', opt],
    ['FixedAssetFilterInput', 'status', 'FixedAssetStatus', opt],
    ['FixedAssetOutput', 'status', 'FixedAssetStatus', req],
    ['FixedAssetOutput', 'depreciationMethod', 'DepreciationMethodOutput', req],
    ['DepreciationEntryOutput', 'status', 'DepreciationEntryStatus', req],
    ['DepreciationScheduleOutput', 'method', 'DepreciationMethodOutput', req],
    ['CreatePerformanceObligationInput', 'recognitionMethod', 'RecognitionMethod', req],
    ['PerformanceObligationOutput', 'recognitionMethod', 'RecognitionMethodOutput', req],
    ['RevenueContractOutput', 'status', 'RevenueContractStatus', req],
    ['UpdateRevenueContractInput', 'status', 'RevenueContractStatus', opt],
    ['RevenueScheduleEntryOutput', 'status', 'RevenueEntryStatus', req],
    ['CycleCountOutput', 'status', 'CycleCountStatus', req],
    ['EdiDocumentOutput', 'direction', 'EdiDirection', req],
    ['EdiDocumentOutput', 'status', 'EdiStatus', req],
    ['CreateEdiDocumentInput', 'direction', 'EdiDirection', opt],
    ['ApplyPrepaymentInput', 'targetType', 'PrepaymentTargetType', req],
    ['PrepaymentOutput', 'status', 'PrepaymentStatus', req],
    ['ApplyVendorCreditInput', 'targetType', 'VendorCreditTargetType', req],
    ['VendorCreditOutput', 'status', 'VendorCreditStatus', req],
    ['CreatePriceLevelInput', 'adjustmentType', 'PriceAdjustmentType', opt],
    ['PriceLevelOutput', 'adjustmentType', 'PriceAdjustmentType', req],
    ['TransferOrderOutput', 'status', 'TransferOrderStatus', req],
    ['UpdateProductionBatchInput', 'status', 'ProductionBatchStatus', opt],
    ['ProductionBatchOutput', 'status', 'ProductionBatchStatus', req],
    ['InboundShipmentOutput', 'status', 'InboundShipmentStatus', req],
    ['RecordActivityInput', 'actorKind', 'ActorKind', opt],
    ['ActivityLogEntryOutput', 'actorKind', 'ActorKind', req],
    ['CreateChannelInput', 'channelType', 'ChannelType', req],
    ['ChannelOutput', 'status', 'ChannelStatus', req],
    ['CompanyOutput', 'status', 'CompanyStatus', req],
    ['CreateUnitConversionRuleInput', 'ruleType', 'ConversionRuleType', req],
    ['UnitConversionRuleOutput', 'ruleType', 'ConversionRuleType', req],
    ['CreateZoneShippingMethodInput', 'methodType', 'ShippingMethodType', req],
    ['ZoneShippingMethodOutput', 'methodType', 'ShippingMethodType', req],
    ['EnqueuePrintJobInput', 'payloadKind', 'PrintPayloadKind', opt],
    ['PrintJobOutput', 'status', 'PrintJobStatus', req],
    ['CreateIntegrationFieldMappingInput', 'transform', 'FieldTransform', opt],
    ['IntegrationFieldMappingOutput', 'transform', 'FieldTransform', req],
    ['PaymentObligationOutput', 'status', 'PaymentObligationStatus', req],
    ['TopologySnapshotOutput', 'health', 'HealthGrade', req],
    ['CreateVendorReturnItemInput', 'reason', 'VendorReturnReason', opt],
    ['VendorReturnOutput', 'status', 'VendorReturnStatus', req],
    ['CreateFraudSignalInput', 'signalType', 'FraudSignalType', req],
    ['FraudAssessmentOutput', 'decision', 'FraudDecision', req],
    ['CreateFraudRuleInput', 'action', 'FraudDecision', req],
    ['FraudRuleOutput', 'signalType', 'FraudSignalType', req],
    ['SearchFieldInput', 'tokenizer', 'SearchTokenizer', opt],
    ['FacetConfigOutput', 'facetType', 'FacetType', req],
    ['CreateAgentIdentityInput', 'walletProofType', 'AgentWalletProofTypeInput', opt],
    ['AgentWalletProofInput', 'proofType', 'AgentWalletProofTypeInput', opt],
    ['AgentIdentityOutput', 'walletProofType', 'AgentWalletProofType', opt],
    ['ImportOptionsInput', 'onConflict', 'ImportConflictPolicy', opt],
  ];
  for (const [iface, field, tsType, o] of cases) assertField(iface, field, tsType, o);
});

test('g3 d.ts: enum-shaped method arguments reference the unions', () => {
  assert.match(block('declare class', 'EdiDocuments'), /^  setStatus\(id: string, status: EdiStatus, errorMessage\?: /m);
  assert.match(block('declare class', 'PaymentObligations'), /^  setStatus\(id: string, status: PaymentObligationStatus\)/m);
  assert.match(block('declare class', 'Fraud'), /^  reviewAssessment\(orderId: string, decision: FraudDecision, reviewer: string/m);
});

test('g3 d.ts: catalog lists take an optional filter', () => {
  const uom = block('declare class', 'UnitsOfMeasure');
  assert.match(uom, /^  listClasses\(filter\?: UnitClassFilterInput \| undefined \| null\): Promise<Array<UnitClassOutput>>$/m);
  assert.match(uom, /^  listRules\(filter\?: UnitConversionRuleFilterInput \| undefined \| null\): Promise<Array<UnitConversionRuleOutput>>$/m);
  assert.match(block('declare class', 'PrintStations'), /^  listStations\(filter\?: PrintStationFilterInput \| undefined \| null\): Promise<Array<PrintStationOutput>>$/m);
  for (const iface of ['UnitClassFilterInput', 'UnitConversionRuleFilterInput', 'PrintStationFilterInput']) {
    assertField(iface, 'limit', 'number', { optional: true });
    assertField(iface, 'offset', 'number', { optional: true });
  }
  assertField('UnitConversionRuleFilterInput', 'ruleType', 'ConversionRuleType', { optional: true });
  assertField('UnitConversionRuleFilterInput', 'productId', 'string', { optional: true });
  assertField('PrintStationFilterInput', 'revoked', 'boolean', { optional: true });
});

test('g3 runtime: unitsOfMeasure.listClasses pages, no-arg call still lists everything', async () => {
  const commerce = new Commerce(':memory:');
  const created = [];
  for (const name of ['Weight', 'Length', 'Volume']) {
    created.push(await commerce.unitsOfMeasure.createClass({ name }));
  }
  const all = await commerce.unitsOfMeasure.listClasses();
  assert.equal(all.length, 3);
  assert.deepEqual(new Set(all.map((c) => c.id)), new Set(created.map((c) => c.id)));

  const firstTwo = await commerce.unitsOfMeasure.listClasses({ limit: 2 });
  assert.deepEqual(firstTwo.map((c) => c.id), all.slice(0, 2).map((c) => c.id));

  const window = await commerce.unitsOfMeasure.listClasses({ offset: 1, limit: 1 });
  assert.deepEqual(window.map((c) => c.id), [all[1].id]);

  assert.deepEqual(await commerce.unitsOfMeasure.listClasses({ offset: 3 }), []);
  assert.deepEqual(await commerce.unitsOfMeasure.listClasses({ limit: 0 }), []);
  assert.equal((await commerce.unitsOfMeasure.listClasses({})).length, 3);
  await commerce.close();
});

test('g3 runtime: unitsOfMeasure.listRules filters by scope / product and pages', async () => {
  const commerce = new Commerce(':memory:');
  const weight = await commerce.unitsOfMeasure.createClass({ name: 'Weight' });
  const gram = await commerce.unitsOfMeasure.createUom({
    unitClassId: weight.id, name: 'Gram', abbreviation: 'g', factor: '1',
  });
  const kilo = await commerce.unitsOfMeasure.createUom({
    unitClassId: weight.id, name: 'Kilogram', abbreviation: 'kg', factor: '1000',
  });
  const product = await commerce.products.create({
    name: 'Flour',
    variants: [{ sku: 'FLOUR-1', name: 'Bag', price: 4.5 }],
  });
  const system = await commerce.unitsOfMeasure.createRule({
    ruleType: 'SYSTEM', fromUomId: kilo.id, toUomId: gram.id, factor: '1000',
  });
  const sku = await commerce.unitsOfMeasure.createRule({
    ruleType: 'sku', productId: product.id, fromUomId: gram.id, toUomId: kilo.id, factor: '0.001',
  });
  assert.equal(sku.ruleType, 'SKU', 'input is case-insensitive, output is the UPPERCASE wire form');

  const all = await commerce.unitsOfMeasure.listRules();
  assert.deepEqual(new Set(all.map((r) => r.id)), new Set([system.id, sku.id]));

  const systemOnly = await commerce.unitsOfMeasure.listRules({ ruleType: 'SYSTEM' });
  assert.deepEqual(systemOnly.map((r) => r.id), [system.id]);
  const byProduct = await commerce.unitsOfMeasure.listRules({ productId: product.id });
  assert.deepEqual(byProduct.map((r) => r.id), [sku.id]);
  assert.deepEqual(await commerce.unitsOfMeasure.listRules({ ruleType: 'SKU', limit: 1, offset: 1 }), []);
  assert.equal((await commerce.unitsOfMeasure.listRules({ limit: 1 })).length, 1);

  await assert.rejects(
    () => commerce.unitsOfMeasure.listRules({ ruleType: 'nope' }),
    (err) => err.code === 'VALIDATION' && /conversion rule type/i.test(err.message),
  );
  await assert.rejects(
    () => commerce.unitsOfMeasure.listRules({ productId: 'not-a-uuid' }),
    (err) => err.code === 'VALIDATION' && /product_id/i.test(err.message),
    'a malformed product id is refused, not silently dropped',
  );
  await commerce.close();
});

test('g3 runtime: printStations.listStations filters by revoked and pages', async () => {
  const commerce = new Commerce(':memory:');
  const stations = [];
  for (const name of ['Bench 1', 'Bench 2', 'Bench 3']) {
    stations.push((await commerce.printStations.pair({ name })).station);
  }
  await commerce.printStations.revokeStation(stations[1].id);

  const all = await commerce.printStations.listStations();
  assert.equal(all.length, 3);
  const live = await commerce.printStations.listStations({ revoked: false });
  assert.deepEqual(new Set(live.map((s) => s.id)), new Set([stations[0].id, stations[2].id]));
  const revoked = await commerce.printStations.listStations({ revoked: true });
  assert.deepEqual(revoked.map((s) => s.id), [stations[1].id]);

  const window = await commerce.printStations.listStations({ offset: 1, limit: 1 });
  assert.deepEqual(window.map((s) => s.id), [all[1].id]);
  assert.equal((await commerce.printStations.listStations({ limit: 2 })).length, 2);
  assert.deepEqual(await commerce.printStations.listStations({ offset: 10 }), []);
  await commerce.close();
});

test('g3 runtime: enum wire forms match the declared unions', async () => {
  const commerce = new Commerce(':memory:');

  const asset = await commerce.fixedAssets.create({
    name: 'Desk',
    category: 'FURNITURE_AND_FIXTURES',
    acquisitionDate: '2026-01-01',
    acquisitionCost: '500.00',
    salvageValue: '50.00',
    usefulLifeMonths: 60,
    depreciationMethod: 'declining_balance',
    decliningBalanceRate: '0.2',
  });
  assert.equal(asset.category, 'furniture_and_fixtures', 'category parses case-insensitively, renders snake_case');
  assert.equal(asset.depreciationMethod, 'declining_balance');
  assert.equal(asset.status, 'draft');
  const drafts = await commerce.fixedAssets.list({ status: 'DRAFT' });
  assert.deepEqual(drafts.map((a) => a.id), [asset.id]);
  await assert.rejects(
    () => commerce.fixedAssets.list({ status: 'parked' }),
    (err) => err.code === 'VALIDATION',
  );

  await commerce.erc8004.registerIdentity({
    agentRegistry: 'eip155:8453:0xregistry',
    agentId: '7',
    agentUri: 'https://agents.example/7.json',
    agentWallet: '0xwallet7',
    walletProofType: 'eip712',
    walletProof: '0xsig',
  });
  const identity = await commerce.erc8004.getIdentity('eip155:8453:0xregistry', '7');
  assert.equal(identity.walletProofType, 'eip_712', 'eip712 is accepted, records read back as eip_712');

  const doc = await commerce.ediDocuments.create({ documentType: '850', direction: 'OUTBOUND' });
  assert.equal(doc.direction, 'outbound');
  assert.equal(doc.status, 'pending');
  const sent = await commerce.ediDocuments.setStatus(doc.id, 'sent');
  assert.equal(sent.status, 'sent');
  await commerce.close();
});
