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

  it('fails cleanly, naming the valid choices', () => {
    const unknownMachine = ask({ machine: 'nope' });
    assert.equal(unknownMachine.success, false);
    assert.ok(unknownMachine.machines.includes('order'));

    const unknownState = ask({ machine: 'payment', from: 'bogus' });
    assert.equal(unknownState.success, false);
    assert.ok(unknownState.states.includes('completed'));

    assert.equal(ask({ machine: 'payment', from: 'pending', to: 'bogus' }).success, false);
    assert.equal(ask({ from: 'pending' }).success, false);
    assert.equal(ask({ machine: 'order', to: 'shipped' }).success, false);
  });
});
