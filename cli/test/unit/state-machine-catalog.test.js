/**
 * explain_state_machine -- the catalog generated from the engine's transition
 * rules, and the four question shapes the tool answers.
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { z } from 'zod';

import {
  STATE_MACHINES,
  explainStateMachine,
  findStateMachine,
  nextStates,
  stateMachineTool,
} from '../../src/state-machine-catalog.js';

const ask = (params) => explainStateMachine({ params });

describe('state machine catalog -- generated data', () => {
  it('is internally consistent', () => {
    assert.ok(STATE_MACHINES.length >= 22);
    const names = STATE_MACHINES.map((m) => m.name);
    assert.equal(new Set(names).size, names.length);
    for (const machine of STATE_MACHINES) {
      const states = new Set(machine.states.map((s) => s.name));
      assert.equal(states.size, machine.states.length, machine.name);
      assert.ok(states.has(machine.initial), `${machine.name} initial`);
      for (const state of machine.states) {
        assert.equal(state.terminal, state.next.length === 0, `${machine.name}.${state.name}`);
        for (const next of state.next) {
          assert.ok(states.has(next), `${machine.name}: ${state.name} -> ${next}`);
          assert.notEqual(next, state.name);
        }
      }
    }
  });

  it('covers the everyday commerce loop', () => {
    for (const name of ['order', 'cart', 'payment', 'refund', 'return', 'shipment']) {
      assert.ok(findStateMachine(name), name);
    }
  });

  it('finds a machine by name, Rust type, or a single-machine domain', () => {
    assert.equal(findStateMachine('order').name, 'order');
    assert.equal(findStateMachine('OrderStatus').name, 'order');
    assert.equal(findStateMachine('orders').name, 'order');
    // `payments` holds two machines (payment, refund): ambiguous, so no match.
    assert.equal(findStateMachine('payments'), undefined);
    assert.equal(findStateMachine('nope'), undefined);
  });

  it('nextStates returns a copy, or undefined for unknown input', () => {
    assert.deepEqual(nextStates('order', 'pending'), ['confirmed', 'cancelled']);
    nextStates('order', 'pending').push('x');
    assert.deepEqual(nextStates('order', 'pending'), ['confirmed', 'cancelled']);
    assert.equal(nextStates('order', 'bogus'), undefined);
    assert.equal(nextStates('bogus', 'pending'), undefined);
  });
});

describe('explain_state_machine', () => {
  it('is a read tool whose schema accepts every question shape', () => {
    assert.equal(stateMachineTool.name, 'explain_state_machine');
    assert.equal(stateMachineTool.permission, 'read');
    const schema = z.object(stateMachineTool.inputSchema);
    assert.ok(schema.safeParse({}).success);
    assert.ok(schema.safeParse({ machine: 'order', from: 'pending', to: 'shipped' }).success);
    assert.equal(schema.safeParse({ machine: '' }).success, false);
  });

  it('lists every machine when asked nothing', () => {
    const result = ask({});
    assert.equal(result.success, true);
    assert.equal(result.count, STATE_MACHINES.length);
    const order = result.machines.find((m) => m.machine === 'order');
    assert.equal(order.initial, 'pending');
    assert.deepEqual(order.terminal, ['cancelled', 'refunded']);
  });

  it('returns the full graph for one machine', () => {
    const result = ask({ machine: 'order' });
    assert.equal(result.rustType, 'OrderStatus');
    assert.deepEqual(result.transitions.shipped, ['delivered']);
    assert.deepEqual(result.entryStates, ['pending']);
    assert.match(result.mermaid, /^stateDiagram-v2\n {4}\[\*\] --> pending\n/);
    assert.match(result.mermaid, /refunded --> \[\*\]/);
  });

  it('reports entry states that the default initial state cannot reach', () => {
    assert.deepEqual(ask({ machine: 'serial' }).entryStates, ['available', 'in_production']);
  });

  it('says what can happen next, tolerating spelling variants', () => {
    for (const from of ['in_transit', 'In Transit', 'in-transit', 'InTransit']) {
      const result = ask({ machine: 'shipment', from });
      assert.equal(result.from, 'in_transit');
      assert.deepEqual(result.next, ['out_for_delivery', 'failed']);
      assert.equal(result.terminal, false);
    }
  });

  it('accepts an allowed move, including the engine alias `canceled`', () => {
    const result = ask({ machine: 'order', from: 'pending', to: 'canceled' });
    assert.equal(result.allowed, true);
    assert.equal(result.to, 'cancelled');
  });

  it('refuses a skipped step and gives the shortest route', () => {
    const result = ask({ machine: 'order', from: 'pending', to: 'delivered' });
    assert.equal(result.success, true);
    assert.equal(result.allowed, false);
    assert.deepEqual(result.path, ['pending', 'confirmed', 'processing', 'shipped', 'delivered']);
    assert.deepEqual(result.next, ['confirmed', 'cancelled']);
    assert.match(result.reason, /Shortest route: pending -> confirmed/);
  });

  it('explains terminal and unreachable targets', () => {
    const terminal = ask({ machine: 'order', from: 'refunded', to: 'pending' });
    assert.equal(terminal.allowed, false);
    assert.equal(terminal.path, null);
    assert.match(terminal.reason, /terminal/);

    const unreachable = ask({ machine: 'subscription', from: 'active', to: 'pending' });
    assert.equal(unreachable.allowed, false);
    assert.equal(unreachable.path, null);
    assert.match(unreachable.reason, /cannot be reached/);
  });

  it('reports whether re-setting the current status is a no-op or refused', () => {
    const noop = ask({ machine: 'order', from: 'pending', to: 'pending' });
    assert.equal(noop.allowed, true);
    assert.match(noop.reason, /no-op/);
  });

  it('names the tool for each move, and the moves only one operation can make', () => {
    const graph = ask({ machine: 'order' });
    assert.deepEqual(graph.tools.shipped, ['ship_order']);
    assert.deepEqual(graph.tools.cancelled, ['cancel_order', 'update_order_status']);
    assert.ok(
      graph.onlyVia.some(
        (m) => m.from === 'processing' && m.to === 'shipped' && m.tool === 'ship_order',
      ),
    );
    const customer = ask({ machine: 'customer' });
    assert.ok(
      customer.onlyVia.some(
        (m) => m.from === 'active' && m.to === 'deleted' && m.tool === 'delete_customer',
      ),
    );

    const next = ask({ machine: 'return', from: 'approved' });
    assert.deepEqual(next.moves, [
      { status: 'in_transit', tools: ['add_return_tracking'] },
      { status: 'cancelled', tools: ['cancel_return'] },
    ]);

    const ship = ask({ machine: 'order', from: 'processing', to: 'shipped' });
    assert.equal(ship.allowed, true);
    assert.equal(ship.onlyVia, 'ship_order');
    assert.match(ship.reason, /only through ship_order/);

    const refused = ask({ machine: 'return', from: 'approved', to: 'received' });
    assert.equal(refused.allowed, false);
    assert.deepEqual(refused.path, ['approved', 'in_transit', 'received']);
    assert.equal(refused.moves[0].tools[0], 'add_return_tracking');
  });

  it('reports the persistence-layer move the generator omits, with its source', () => {
    assert.deepEqual(nextStates('payment', 'pending'), [
      'processing',
      'failed',
      'cancelled',
      'completed',
    ]);
    const graph = ask({ machine: 'payment' });
    assert.equal(graph.persistenceMoves[0].from, 'pending');
    assert.match(graph.persistenceMoves[0].source, /rules\/payments\.rs/);
    assert.equal(ask({ machine: 'payment', from: 'pending', to: 'completed' }).allowed, true);
  });

  it('answers a known lifecycle that is not catalogued without guessing its moves', () => {
    for (const machine of ['gift_card', 'gift-cards', 'GiftCardStatus']) {
      const result = ask({ machine });
      assert.equal(result.success, true, machine);
      assert.equal(result.catalogued, false);
      assert.equal(result.machine, 'gift_card');
      assert.deepEqual(result.states, ['active', 'depleted', 'expired', 'disabled']);
      assert.match(result.reason, /not catalogued/);
    }
    const move = ask({ machine: 'x402_intent', from: 'signed', to: 'settled' });
    assert.equal(move.success, true);
    assert.equal(move.allowed, null);
    assert.equal(move.next, null);
    assert.equal(ask({ machine: 'coupon', from: 'depleted' }).success, false);

    // An order's own payment status is `PaymentStatus` in the engine.
    const orderPayment = ask({ machine: 'payment_status' });
    assert.equal(orderPayment.machine, 'order_payment');
    assert.match(orderPayment.note, /never set directly/);

    const all = ask({});
    assert.ok(all.uncatalogued.some((l) => l.machine === 'invoice'));
    assert.equal(all.count, STATE_MACHINES.length);
    assert.deepEqual(ask({ machine: 'order' }).related, ['order_payment', 'order_fulfillment']);
  });

  it('reports the lifecycles whose rule the repositories enforce', () => {
    const all = ask({});
    for (const name of [
      'transfer_order',
      'vendor_return',
      'edi_document',
      'receipt',
      'put_away',
      'wave',
      'pick_task',
      'bom',
    ]) {
      assert.ok(
        all.machines.some((m) => m.machine === name),
        `${name} is listed`,
      );
      assert.ok(!all.uncatalogued.some((l) => l.machine === name), `${name} is not uncatalogued`);
    }
    assert.equal(
      ask({ machine: 'vendor_return', from: 'cancelled', to: 'cancelled' }).allowed,
      true,
    );
    assert.equal(ask({ machine: 'wave', from: 'completed' }).terminal, true);
    assert.equal(ask({ machine: 'fulfillment_wave' }).machine, 'wave');
  });

  it('lists the candidates when a domain holds several lifecycles', () => {
    const payments = ask({ machine: 'payments' });
    assert.equal(payments.success, false);
    assert.deepEqual(
      payments.candidates.map((c) => [c.machine, c.catalogued]),
      [
        ['payment', true],
        ['refund', true],
      ],
    );
    assert.match(payments.hint, /payment/);
    const promotions = ask({ machine: 'promotions', from: 'active' });
    assert.deepEqual(
      promotions.candidates.map((c) => c.machine),
      ['promotion', 'coupon'],
    );
  });

  it('describes itself as the catalogued lifecycles, not every one', () => {
    assert.doesNotMatch(stateMachineTool.description, /every status lifecycle the engine enforces/);
    assert.match(stateMachineTool.description, /not catalogued yet/);
  });

  it('fails cleanly, naming the valid choices', () => {
    const unknownMachine = ask({ machine: 'nope' });
    assert.equal(unknownMachine.success, false);
    assert.ok(unknownMachine.machines.includes('order'));
    assert.ok(unknownMachine.uncatalogued.includes('gift_card'));

    const unknownState = ask({ machine: 'payment', from: 'bogus' });
    assert.equal(unknownState.success, false);
    assert.ok(unknownState.states.includes('completed'));

    assert.equal(ask({ machine: 'payment', from: 'pending', to: 'bogus' }).success, false);
    assert.equal(ask({ from: 'pending' }).success, false);
    assert.equal(ask({ machine: 'order', to: 'shipped' }).success, false);
  });
});
