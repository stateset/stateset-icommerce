import { readFileSync } from 'node:fs';

// Explicit engine-owned tables only. Neither user text nor sqlite_schema can
// add a table to this boundary. Optional tables remain visibly unavailable.
const GROUPS = {
  Commerce: {
    customers: 'customers customer_addresses',
    orders: 'orders order_items',
    products: 'products product_variants',
    inventory:
      'inventory_items inventory_balances inventory_reservations inventory_transactions inventory_movements inventory_locations',
    returns: 'returns return_items',
    carts: 'carts cart_items',
    payments: 'payments refunds payment_methods',
    shipments: 'shipments shipment_items shipment_events',
    tax: 'tax_jurisdictions tax_rates tax_exemptions tax_calculations tax_settings',
    currency:
      'exchange_rates exchange_rate_history product_currency_prices store_currency_settings',
    'shipping-zones': 'shipping_zones zone_shipping_methods',
    catalog: 'products product_variants',
    checkout: 'carts cart_items',
    'custom-objects': 'custom_object_types custom_object_records',
    analytics: 'orders order_items inventory_items',
    explain: 'orders payments inventory_reservations kernel_receipts',
  },
  Growth: {
    promotions: 'promotions coupon_codes promotion_conditions promotion_usage',
    subscriptions:
      'subscriptions subscription_items subscription_plans subscription_plan_items billing_cycles subscription_events',
    'gift-cards': 'gift_cards gift_card_transactions',
    'store-credits': 'store_credits store_credit_transactions',
    segments: 'segments segment_memberships',
    reviews: 'reviews',
    wishlists: 'wishlists wishlist_items',
    loyalty: 'loyalty_programs loyalty_accounts loyalty_transactions rewards',
    'search-config': 'search_configs',
    'price-schedules': 'price_schedules price_schedule_entries',
    'price-levels': 'price_levels price_level_entries',
    companies: 'companies contacts company_shipping_addresses company_price_overrides',
    channels: 'channels channel_product_mappings',
  },
  Operations: {
    suppliers: 'suppliers purchase_orders purchase_order_items',
    manufacturing:
      'manufacturing_boms manufacturing_bom_components manufacturing_work_orders manufacturing_work_order_materials manufacturing_work_order_tasks',
    quality: 'inspections inspection_items non_conformances quality_holds defect_codes',
    lots: 'lots lot_transactions lot_locations lot_reservations lot_genealogy lot_certificates',
    serials: 'serial_numbers serial_history serial_reservations',
    warehouse:
      'warehouses locations location_inventory warehouse_zones warehouse_bins inventory_bin_levels inventory_bin_movements',
    receiving: 'receipts receipt_items put_aways',
    fulfillment: 'waves wave_orders pick_tasks pack_tasks ship_tasks cartons carton_items',
    warranties: 'warranties warranty_claims',
    backorders: 'backorders backorder_allocations backorder_fulfillments',
    'cycle-counts': 'cycle_counts cycle_count_lines',
    'vendor-returns': 'vendor_returns vendor_return_items',
    'units-of-measure': 'unit_classes units_of_measure unit_conversion_rules',
    'stock-snapshots': 'stock_snapshots stock_snapshot_lines',
    'print-stations': 'print_stations print_jobs',
    'transfer-orders': 'transfer_orders transfer_order_items',
    'production-batches': 'production_batches',
    'supplier-skus': 'supplier_skus',
    'inbound-shipments': 'inbound_shipments inbound_shipment_items',
    maintenance: '',
    simulations: '',
  },
  Finance: {
    invoices: 'invoices invoice_items invoice_direct_payments',
    'accounts-payable':
      'ap_bills ap_bill_items ap_payments ap_payment_allocations ap_payment_runs ap_payment_run_bills',
    'accounts-receivable':
      'invoices ar_credit_memos ar_credit_memo_applications ar_payment_applications ar_write_offs ar_collection_activities',
    'cost-accounting':
      'item_costs cost_layers cost_transactions cost_adjustments cost_rollups cost_variances',
    credit:
      'credit_accounts credit_applications credit_holds credit_reservations credit_transactions',
    'general-ledger':
      'gl_accounts gl_journal_entries gl_journal_entry_lines gl_periods gl_account_balances gl_auto_posting_config',
    'fixed-assets': 'fixed_assets fixed_asset_depreciation_entries',
    'revenue-recognition': 'revenue_contracts performance_obligations revenue_schedule_entries',
    prepayments: 'prepayments prepayment_applications',
    'vendor-credits': 'vendor_credits vendor_credit_applications',
    'payment-obligations': 'payment_obligations',
    treasury: '',
    stablecoin: '',
  },
  'Agents & trust': {
    a2a: 'a2a_quotes a2a_purchases a2a_escrows a2a_disputes a2a_dispute_evidence a2a_agent_messages a2a_credit_terms a2a_credit_entries',
    'agent-cards': 'agent_cards',
    erc8004:
      'agent_identities agent_identity_metadata agent_feedback agent_feedback_responses agent_validation_requests agent_validation_responses',
    x402: 'x402_payment_intents x402_credit_accounts x402_credit_transactions',
    audit: 'audit_log kernel_receipts kernel_receipt_audit_log',
    proofs: 'kernel_receipts kernel_receipt_audit_log',
    policies: 'kernel_economic_budgets kernel_budget_commitments',
    fraud: 'fraud_rules fraud_assessments',
    compliance: 'customers orders audit_log',
    'agent-runtime': '',
    'agent-receipt': '',
    'a2a-automation': '',
    'a2a-observability': '',
    'a2a-platform': '',
    'a2a-intelligence': '',
    'circuit-breaker': '',
    'agentic-runtime': 'events kernel_outbox kernel_receipts',
  },
  Integrations: {
    sync: '',
    import: '',
    vector: '',
    connectors: 'integration_mappings integration_field_mappings',
    'integration-mappings': 'integration_mappings',
    'integration-field-mappings': 'integration_field_mappings',
    purgatory: 'purgatory_orders purgatory_line_items',
    'topology-snapshots': 'topology_snapshots',
    'edi-documents': 'edi_documents',
    'activity-logs': 'activity_logs',
  },
};

const catalog = JSON.parse(readFileSync(new URL('./catalog.json', import.meta.url), 'utf8'));
const groupOrder = Object.keys(GROUPS);
const placement = new Map(
  Object.entries(GROUPS).flatMap(([group, entries]) =>
    Object.entries(entries).map(([id, tables]) => [
      id,
      { group, tables: tables.split(' ').filter(Boolean) },
    ]),
  ),
);
if (catalog.length !== placement.size || catalog.some(({ id }) => !placement.has(id)))
  throw new Error('TUI domain coverage is stale; update domains.js and regenerate the catalog');

export const DOMAINS = Object.freeze(
  catalog
    .map((domain) => ({
      ...domain,
      ...placement.get(domain.id),
      name: domain.id.replace(/-/g, ' '),
    }))
    .sort(
      (a, b) =>
        groupOrder.indexOf(a.group) - groupOrder.indexOf(b.group) || a.name.localeCompare(b.name),
    ),
);
export const TABLES = Object.freeze([...new Set(DOMAINS.flatMap((d) => d.tables))]);
export function domainById(id) {
  return DOMAINS.find((d) => d.id === id);
}
export function tableLabel(table) {
  return table.replace(/_/g, ' ');
}
export function explorerRows(state) {
  const e = state.explorer;
  let rows;
  if (e.mode === 'domains') rows = DOMAINS;
  else if (e.mode === 'tools') rows = domainById(e.domain)?.tools || [];
  else if (e.mode === 'links') rows = e.links || [];
  else if (e.mode === 'resources')
    rows = (domainById(e.domain)?.tables || []).map((id) => ({ id, name: tableLabel(id) }));
  else
    return state.data?.explorer?.requestKey === explorerRequestKey(e)
      ? state.data.explorer.rows
      : [];
  const query = (state.input?.kind === 'search' ? state.input.text : e.query)
    .trim()
    .toLocaleLowerCase();
  return rows.filter((row) => {
    // Search meaningful labels, never serialized keys or every tool's hidden
    // description when the user is choosing a domain.
    const fields =
      e.mode === 'domains'
        ? [row.id, row.name, row.group, row.tier, row.tables.length ? 'records' : 'reference']
        : e.mode === 'tools'
          ? [row.name, row.description, row.permission, row.tier]
          : e.mode === 'links'
            ? [row.name, row.domain, row.table, row.column, row.value]
            : [row.id, row.name];
    return fields.some((value) =>
      String(value ?? '')
        .toLocaleLowerCase()
        .includes(query),
    );
  });
}
export function explorerRequest(e) {
  if (e.mode !== 'records') return null;
  return {
    domain: e.domain,
    table: e.table,
    query: e.query,
    page: e.page,
    sort: e.sort,
    relation: e.relation || null,
  };
}
export function explorerRequestKey(e) {
  return JSON.stringify(explorerRequest(e));
}

export function openExplorer(state, domain = null) {
  return {
    ...state,
    view: 6,
    focusRecord: null,
    focusTarget: null,
    relation: null,
    selected: 0,
    detail: false,
    detailOffset: 0,
    input: null,
    help: false,
    panel: null,
    query: '',
    explorer: {
      mode: domain ? 'resources' : 'domains',
      domain,
      table: null,
      query: '',
      page: 0,
      sort: 0,
      stack: [],
      relation: null,
    },
  };
}

function step(state, change, remember = true) {
  const e = state.explorer;
  const stack = remember
    ? [
        ...e.stack,
        { ...e, stack: undefined, selected: state.selected, detail: state.detail },
      ].slice(-20)
    : e.stack;
  return {
    ...state,
    selected: 0,
    detail: false,
    detailOffset: 0,
    notice: '',
    explorer: { ...e, query: '', page: 0, relation: null, ...change, stack },
  };
}

export function explorerKey(state, text, key) {
  const e = state.explorer,
    name = key.name;
  let next, effect;
  if (text === 'd') next = openExplorer(state);
  else if (['[', ']'].includes(text) && e.mode === 'domains' && !state.detail) {
    const rows = explorerRows(state);
    const groups = [...new Set(rows.map((row) => row.group))];
    if (groups.length < 2) return { state };
    const current = groups.indexOf(rows[state.selected]?.group);
    const group =
      groups[(Math.max(0, current) + (text === ']' ? 1 : -1) + groups.length) % groups.length];
    next = { ...state, selected: rows.findIndex((row) => row.group === group) };
  } else if (text === 'T' && e.domain) next = step(state, { mode: 'tools' });
  else if (text === 'L' && !state.detail && e.domain === 'simulations')
    return { state, effect: 'open-lab' };
  else if (text === 'b' && state.detail) next = { ...state, detail: false, detailOffset: 0 };
  else if ((name === 'escape' || text === 'b') && !state.detail) {
    if (e.query && name === 'escape')
      next = { ...state, selected: 0, explorer: { ...e, query: '', page: 0 } };
    else if (e.stack.length) {
      const previous = e.stack[e.stack.length - 1];
      next = {
        ...state,
        detailOffset: 0,
        selected: previous.selected,
        detail: previous.detail,
        explorer: { ...previous, stack: e.stack.slice(0, -1) },
      };
    } else next = openExplorer(state);
    if (next.explorer.mode === 'records') effect = 'refresh';
  } else if (name === 'return' && !state.detail) {
    const row = explorerRows(state)[state.selected];
    if (!row) return { state };
    if (e.mode === 'domains') next = step(state, { mode: 'resources', domain: row.id });
    else if (e.mode === 'resources') {
      next = step(state, { mode: 'records', table: row.id, sort: 0 });
      effect = 'refresh';
    } else if (e.mode === 'links') {
      next = step(state, {
        mode: 'records',
        domain: row.domain,
        table: row.table,
        relation: { column: row.column, value: row.value },
        sort: 0,
      });
      effect = 'refresh';
    } else next = { ...state, detail: true, detailOffset: 0 };
  } else if (text === 't' && e.mode === 'records' && !state.loading) {
    const row = explorerRows(state)[state.selected];
    const links = row ? state.data.explorer.links?.[state.selected] || [] : [];
    next = links.length
      ? step(state, { mode: 'links', links })
      : { ...state, notice: 'No persisted record links found' };
  } else if (text === 'o' && e.mode === 'records' && !state.detail && !state.loading) {
    next = { ...state, selected: 0, explorer: { ...e, sort: e.sort === 1 ? 0 : 1, page: 0 } };
    effect = 'refresh';
  } else if (['n', 'p'].includes(text) && e.mode === 'records' && !state.detail && !state.loading) {
    const total = state.data?.explorer?.matched;
    const page = Math.max(0, e.page + (text === 'n' ? 1 : -1));
    if (typeof total !== 'number' || page === e.page || page * 50 >= total) return { state };
    next = { ...state, selected: 0, explorer: { ...e, page } };
    effect = 'refresh';
  } else if (text === 'u' || text === 'b') return { state };
  else return null;
  return { state: next, effect };
}
