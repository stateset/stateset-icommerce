#!/usr/bin/env node
// Tests for scripts/ci/generate_binding_parity.mjs and its source scanner.
//
// The parity gate is only as good as the call tracing underneath it, so most
// of these pin the tracer on small fixtures: what counts as an engine call,
// what does not (doc-comment examples, cross-domain lookups), and how the
// gate classifies a disagreement with the baseline.

import test from 'node:test';
import assert from 'node:assert/strict';

import {
  analyseGoBinding,
  analyseRustClassBinding,
  buildBindingRecords,
  buildEngineModel,
  evaluateGate,
  extractEngineCalls,
  findHollowSignals,
  lookupAlias,
  main,
  matchSurfaceByName,
  validateBaselineConfig,
} from './generate_binding_parity.mjs';
import { maskRust, normalizeName, snakeToCamel } from './lib/rust_source.mjs';

const ENGINE_ACCESSORS = `
impl Commerce {
    pub fn orders(&self) -> Orders {
        Orders::new()
    }

    pub fn promotions(&self) -> Promotions {
        Promotions::new()
    }

    /// Alias for promotions.
    pub fn deals(&self) -> Promotions {
        self.promotions()
    }

    #[cfg(feature = "vector")]
    pub fn vector(&self, api_key: String) -> Result<Vector, CommerceError> {
        todo!()
    }

    pub fn calculate_cart_tax(
        &self,
        cart_id: uuid::Uuid,
    ) -> Result<TaxResult> {
        todo!()
    }

    pub fn new(path: &str) -> Result<Self, CommerceError> {
        todo!()
    }
}
`;

const ENGINE_DOMAINS = `
impl Orders {
    pub(crate) fn new() -> Self {
        Self
    }

    pub fn create(&self, input: CreateOrder) -> Result<Order> {
        todo!()
    }

    pub fn get(&self, id: OrderId) -> Result<Option<Order>> {
        todo!()
    }

    pub fn cancel(&self, id: OrderId) -> Result<Order> {
        todo!()
    }

    fn private_helper(&self) {}
}

impl Promotions {
    pub fn get_by_code(&self, code: &str) -> Result<Option<Promotion>> {
        todo!()
    }

    pub fn apply(&self, request: ApplyPromotionsRequest) -> Result<Applied> {
        todo!()
    }
}

impl Vector {
    pub fn search_products(&self, query: &str, limit: usize) -> Result<Vec<Hit>> {
        todo!()
    }
}

impl std::fmt::Debug for Orders {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}
`;

function engine() {
  return buildEngineModel([
    { path: 'engine/commerce/accessors.rs', text: ENGINE_ACCESSORS },
    { path: 'engine/domains.rs', text: ENGINE_DOMAINS },
  ]);
}

test('maskRust blanks comments and literals but keeps offsets and lifetimes', () => {
  const source = [
    '/// commerce.orders().create(x)',
    'let s = "commerce.orders().get(y)"; // trailing',
    "fn f<'a>(x: &'a str) -> char { 'x' }",
    'let r = r#"orders().cancel("#;',
  ].join('\n');
  const masked = maskRust(source);
  assert.equal(masked.length, source.length);
  assert.equal(masked.split('\n').length, 4);
  assert.doesNotMatch(masked, /orders\(\)/);
  assert.match(masked, /fn f<'a>\(x: &'a str\)/);
});

test('naming helpers map across conventions', () => {
  assert.equal(snakeToCamel('get_by_code'), 'getByCode');
  assert.equal(normalizeName('get_by_code'), normalizeName('GetByCode'));
  assert.equal(normalizeName('getByCode'), 'getbycode');
});

test('engine model: accessors become domains, the rest is the root domain', () => {
  const model = engine();
  assert.deepEqual([...model.domains.keys()], ['commerce', 'orders', 'promotions', 'vector']);
  assert.deepEqual(
    model.domains.get('orders').methods.map((m) => m.name),
    ['cancel', 'create', 'get'],
    'pub(crate) new, private helpers and trait impls are not engine API',
  );
  assert.deepEqual(model.accessorAliases, [{ accessor: 'deals', aliasOf: 'promotions' }]);
  assert.deepEqual(
    model.domains.get('commerce').methods.map((m) => m.name),
    ['calculate_cart_tax'],
    'constructors without a self receiver are not root methods',
  );
  assert.deepEqual(model.domains.get('vector').features, ['vector']);
});

test('extractEngineCalls: chained, bound, aliased and root calls; comments ignored', () => {
  const body = maskRust(`{
    // commerce.orders().cancel(id) in a comment does not count
    let order = commerce.orders().create(input).map_err(wrap)?;
    let promo = commerce
        .deals()
        .get_by_code(&code)?;
    let vector = {
        let commerce = self.commerce.get()?;
        commerce.vector(self.key.clone()).map_err(|e| wrap(e))?
    };
    let hits = vector.search_products(&q, 10)?;
    let tax = commerce.calculate_cart_tax(cart_id)?;
    let unrelated = map.get(&key);
    commerce.orders().not_an_engine_method();
  }`);
  const calls = extractEngineCalls(body, engine())
    .map((call) => `${call.domain}.${call.method}`)
    .sort();
  assert.deepEqual(calls, [
    'commerce.calculate_cart_tax',
    'orders.create',
    'promotions.get_by_code',
    'vector.search_products',
  ]);
});

const NODE_FIXTURE = `
#[napi]
impl Orders {
    #[napi]
    pub async fn create(&self, input: CreateOrderInput) -> Result<OrderOutput> {
        let commerce = self.commerce.get()?;
        commerce.orders().create(input.into()).map_err(wrap)?;
        self.load(commerce)
    }

    #[napi]
    pub async fn cancel_order(&self, id: String) -> Result<OrderOutput> {
        cancel_helper(&self.commerce, id)
    }

    #[napi]
    pub async fn with_promotion(&self, code: String) -> Result<()> {
        let commerce = self.commerce.get()?;
        commerce.promotions().get_by_code(&code)?;
        commerce.orders().get(id)?;
        Ok(())
    }

    fn load(&self, commerce: &Commerce) -> Result<OrderOutput> {
        commerce.orders().get(id).map(Into::into)
    }

    pub fn not_exported(&self) {
        commerce.orders().cancel(id);
    }
}

fn cancel_helper(handle: &Handle, id: String) -> Result<OrderOutput> {
    handle.get()?.orders().cancel(id.parse()?).map(Into::into)
}
`;

test('Rust class bindings: helpers followed, cross-domain calls not credited', () => {
  const text = NODE_FIXTURE;
  const result = analyseRustClassBinding(
    [{ path: 'node.rs', text, masked: maskRust(text) }],
    engine(),
    {
      implAttribute: '#[napi',
      isExported: (m) => m.visibility === 'pub' && m.attributes.some((a) => a.startsWith('#[napi')),
      hostName: (m) => snakeToCamel(m.name),
    },
  );
  assert.deepEqual(result.classes, [{ className: 'Orders', domain: 'orders', methodCount: 3 }]);
  assert.deepEqual([...result.exposed.keys()].sort(), [
    'orders.cancel',
    'orders.create',
    'orders.get',
  ]);
  assert.deepEqual(result.exposed.get('orders.cancel'), ['Orders.cancelOrder']);
  assert.deepEqual([...result.crossDomain.keys()], ['promotions.get_by_code']);
});

test('Go: host method -> C export -> Rust FFI -> engine call', () => {
  const ffi = `
#[unsafe(no_mangle)]
pub extern "C" fn stateset_order_cancel(handle: *mut Handle, id: *const c_char) -> *mut c_char {
    use_handle(handle, |commerce| commerce.orders().cancel(id.into()))
}

#[unsafe(no_mangle)]
pub extern "C" fn stateset_order_unused(handle: *mut Handle) -> *mut c_char {
    use_handle(handle, |commerce| commerce.orders().get(id))
}
`;
  const go = `package stateset

// Cancel cancels an order (C.stateset_order_unused in a comment is ignored)
func (api *OrdersAPI) Cancel(id string) (*Order, error) {
\tresult := C.stateset_order_cancel(api.commerce.handle, cID)
\treturn parseJSON[Order](result)
}
`;
  const result = analyseGoBinding(
    [{ path: 'apis.go', text: go }],
    [{ path: 'lib.rs', text: ffi, masked: maskRust(ffi) }],
    engine(),
  );
  assert.deepEqual([...result.exposed.keys()], ['orders.cancel']);
  assert.deepEqual(result.exposed.get('orders.cancel'), ['OrdersAPI.Cancel']);
});

test('name-matched surfaces use conventions plus the alias map', () => {
  const aliases = { 'orders.void_order': 'cancel' };
  assert.equal(lookupAlias(aliases, 'orders', 'VoidOrder'), 'cancel');
  assert.equal(lookupAlias(aliases, 'orders', 'voidOrder'), 'cancel');
  const result = matchSurfaceByName(
    [
      { type: 'OrdersApi', method: 'Create' },
      { type: 'OrdersApi', method: 'VoidOrder' },
      { type: 'OrdersApi', method: 'Teleport' },
      { type: 'PromotionsApi', method: 'GetByCode' },
      { type: 'Unmapped', method: 'Create' },
    ],
    new Map([
      ['OrdersApi', 'orders'],
      ['PromotionsApi', 'promotions'],
    ]),
    engine(),
    aliases,
  );
  assert.deepEqual([...result.exposed.keys()].sort(), [
    'orders.cancel',
    'orders.create',
    'promotions.get_by_code',
  ]);
  assert.deepEqual(result.unmatched, ['OrdersApi.Teleport']);
});

function records(exposedById, baseline) {
  const model = engine();
  return buildBindingRecords(
    model,
    Object.entries(exposedById).map(([id, keys]) => ({
      id,
      language: id,
      evidence: 'traced',
      engineBacked: true,
      exposed: new Map(keys.map((key) => [key, ['x']])),
    })),
    baseline,
  );
}

test('gate: clean when the baseline matches, including exemptions', () => {
  const baseline = {
    reference: 'node',
    gated: ['node', 'python'],
    exempt: { all: { 'orders.cancel': 'not portable' } },
    exposed: {
      node: { orders: ['create', 'get'], promotions: ['apply'] },
      python: { orders: ['create'] },
    },
    gaps: { python: { orders: ['get'], promotions: ['apply'] } },
  };
  const current = records(
    {
      node: ['orders.create', 'orders.get', 'promotions.apply', 'orders.cancel'],
      python: ['orders.create'],
    },
    baseline,
  );
  assert.deepEqual(evaluateGate(current, baseline).errors, []);
});

test('gate: loss, unlisted new gap, stale gap and unrecorded gain all fail', () => {
  const baseline = {
    reference: 'node',
    gated: ['node', 'python'],
    exempt: { all: {} },
    exposed: {
      node: { orders: ['create', 'get'] },
      python: { orders: ['create', 'cancel'] },
    },
    gaps: { python: { orders: ['get'] } },
  };
  // Python lost `cancel`, gained `get` (closing the listed gap); Node gained
  // `promotions.apply`, which Python lacks and the baseline does not list.
  const current = records(
    {
      node: ['orders.create', 'orders.get', 'promotions.apply'],
      python: ['orders.create', 'orders.get'],
    },
    baseline,
  );
  const { errors, current: state } = evaluateGate(current, baseline);
  const has = (pattern) => errors.some((error) => pattern.test(error));
  assert.ok(has(/^python lost engine method orders\.cancel/), errors.join('\n'));
  assert.ok(has(/^node exposes promotions\.apply but python does not/), errors.join('\n'));
  assert.ok(has(/^Stale baseline gap: python orders\.get/), errors.join('\n'));
  assert.ok(has(/^node now exposes promotions\.apply/), errors.join('\n'));
  assert.ok(has(/^python now exposes orders\.get/), errors.join('\n'));
  assert.equal(errors.length, 5, errors.join('\n'));
  assert.deepEqual(state.gaps.python, { promotions: ['apply'] });
});

test('baseline exemptions and aliases must name real engine methods', () => {
  const errors = validateBaselineConfig(
    {
      exempt: { all: { 'orders.cancel': 'ok', 'orders.not_a_method': 'no such method' } },
      aliases: { 'orders.void_order': 'cancel', 'promotions.redeem': 'redeem' },
    },
    engine(),
  );
  assert.deepEqual(errors, [
    'Exemption all:orders.not_a_method does not name an engine method; fix or remove it.',
    'Alias promotions.redeem -> redeem does not name an engine method; fix or remove it.',
  ]);
});

test('hollow signals flag empty collections handed to engine inputs only', () => {
  const text = `
fn f() {
    let promo = commerce.promotions().apply(stateset_core::ApplyPromotionsRequest {
        coupon_codes: codes,
        line_items: vec![],
    });
    let bill = commerce.accounts_payable().create_bill(CreateBill {
        items: Vec::new(),
    });
    let out = PromotionOutput {
        tags: vec![],
    };
    Self {
        variants: Vec::new(),
    }
}
`;
  const signals = findHollowSignals([{ path: 'b.rs', text, masked: maskRust(text) }]);
  assert.deepEqual(
    signals.map((s) => `${s.line}:${s.structLiteral}.${s.field}`),
    ['5:stateset_core::ApplyPromotionsRequest.line_items', '8:CreateBill.items'],
  );
});

test('the checked-in report is fresh and the repository is within its baseline', async () => {
  const errors = [];
  const originalError = console.error;
  const originalLog = console.log;
  console.error = (message) => errors.push(message);
  console.log = () => {};
  try {
    const code = await main(['--check']);
    assert.equal(code, 0, errors.join('\n'));
  } finally {
    console.error = originalError;
    console.log = originalLog;
  }
});
