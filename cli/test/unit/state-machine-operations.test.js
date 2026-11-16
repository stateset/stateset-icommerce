/**
 * The hand-maintained half of the lifecycle catalog
 * (src/state-machine-operations.js) against the generated half and the
 * registry: every tool it names exists, every move it names is a catalogued
 * move, and every known-but-uncatalogued lifecycle's states are exactly the
 * binding's generated status type. If the engine or the registry changes,
 * this fails instead of the guidance quietly going stale.
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

import {
  STATE_MACHINES,
  findStateMachine,
  nextStates,
  requestedMove,
  resolveLifecycle,
} from '../../src/state-machine-catalog.js';
import {
  PERSISTENCE_MOVES,
  STATUS_GATED_TOOLS,
  STATUS_PARAMETER_TOOLS,
  TRANSITION_TOOLS,
  UNCATALOGUED_LIFECYCLES,
} from '../../src/state-machine-operations.js';
import { ALL_DOMAIN_TOOLS } from '../../src/tools/domain-registry.js';

const TOOL_NAMES = new Set(ALL_DOMAIN_TOOLS.map((tool) => tool.name));
const GENERATED = JSON.parse(
  readFileSync(new URL('../../src/generated/state-machines.json', import.meta.url), 'utf8'),
);
const BINDING_TYPES = readFileSync(
  new URL('../../../bindings/node/index.d.ts', import.meta.url),
  'utf8',
);

/** Members of `export type Name = 'a' | 'b' ...` in the generated binding types. */
function bindingUnion(name) {
  const match = new RegExp(`export type ${name} =([^\\n]*(?:\\n\\s*\\|[^\\n]*)*)`).exec(
    BINDING_TYPES,
  );
  assert.ok(match, `bindings/node/index.d.ts declares ${name}`);
  return [...match[1].matchAll(/'([^']+)'/g)].map(([, member]) =>
    member.replace(/([a-z0-9])([A-Z])/g, '$1_$2').toLowerCase(),
  );
}

describe('TRANSITION_TOOLS', () => {
  it('names only existing tools', () => {
    const named = [
      ...Object.values(TRANSITION_TOOLS).flatMap((targets) =>
        Object.values(targets).flatMap((moves) => moves.map((move) => move.tool)),
      ),
      ...Object.keys(STATUS_PARAMETER_TOOLS),
      ...Object.keys(STATUS_GATED_TOOLS),
    ];
    assert.deepEqual(
      named.filter((tool) => !TOOL_NAMES.has(tool)),
      [],
    );
  });

  it('names only catalogued machines, states and moves', () => {
    for (const [machineName, targets] of Object.entries(TRANSITION_TOOLS)) {
      const machine = findStateMachine(machineName);
      assert.ok(machine && machine.name === machineName, `${machineName} is a catalog machine`);
      const states = new Map(machine.states.map((s) => [s.name, s]));
      for (const [target, moves] of Object.entries(targets)) {
        assert.ok(states.has(target), `${machineName}.${target} is a state`);
        const into = machine.states.filter((s) => s.next.includes(target)).map((s) => s.name);
        assert.ok(into.length > 0, `some ${machineName} state moves into ${target}`);
        for (const move of moves) {
          for (const from of move.from ?? []) {
            assert.ok(
              into.includes(from),
              `${machineName}: ${from} -> ${target} (${move.tool}) is a catalogued move`,
            );
          }
        }
      }
    }
    for (const machine of Object.values(STATUS_GATED_TOOLS)) {
      assert.ok(findStateMachine(machine), machine);
    }
  });

  it('puts each tool under one machine, and status-parameter tools take a status', () => {
    const machineOf = new Map();
    for (const [machine, targets] of Object.entries(TRANSITION_TOOLS)) {
      for (const moves of Object.values(targets)) {
        for (const { tool } of moves) {
          assert.ok(
            !machineOf.has(tool) || machineOf.get(tool) === machine,
            `${tool} moves one machine`,
          );
          machineOf.set(tool, machine);
        }
      }
    }
    const byName = new Map(ALL_DOMAIN_TOOLS.map((tool) => [tool.name, tool]));
    for (const [tool, param] of Object.entries(STATUS_PARAMETER_TOOLS)) {
      assert.ok(machineOf.has(tool), `${tool} is in TRANSITION_TOOLS`);
      assert.ok(byName.get(tool).inputSchema[param], `${tool} has a ${param} parameter`);
    }
  });

  it('reads the move a call asks for', () => {
    assert.deepEqual(requestedMove('mark_return_received'), {
      machine: 'return',
      targets: ['received'],
    });
    assert.deepEqual(requestedMove('update_order_status', { status: 'Canceled' }), {
      machine: 'order',
      targets: ['cancelled'],
    });
    assert.equal(requestedMove('update_customer', { email: 'a@example.com' }), null);
    assert.equal(requestedMove('list_orders', {}), null);
  });

  it('reads a target that depends on a parameter (cancel_subscription immediate)', () => {
    // The engine stores `expired` for an immediate cancellation and
    // `cancelled` for the default end-of-period one
    // (crates/stateset-db/src/{sqlite,postgres}/subscriptions.rs).
    assert.deepEqual(requestedMove('cancel_subscription', { immediate: true }), {
      machine: 'subscription',
      targets: ['expired'],
    });
    for (const params of [{ immediate: false }, {}, undefined]) {
      assert.deepEqual(requestedMove('cancel_subscription', params), {
        machine: 'subscription',
        targets: ['cancelled'],
      });
    }
  });

  it('names the parameter a parameter-dependent move needs', () => {
    for (const [machine, targets] of Object.entries(TRANSITION_TOOLS)) {
      for (const [target, moves] of Object.entries(targets)) {
        for (const move of moves.filter((m) => m.when)) {
          const byName = ALL_DOMAIN_TOOLS.find((tool) => tool.name === move.tool);
          for (const param of Object.keys(move.when)) {
            assert.ok(
              byName.inputSchema[param],
              `${machine}.${target}: ${move.tool} has a ${param} parameter`,
            );
          }
        }
      }
    }
  });
});

describe('PERSISTENCE_MOVES', () => {
  it('mirrors a rule that still exists in the Rust source, and is merged into the machine', () => {
    for (const [machineName, moves] of Object.entries(PERSISTENCE_MOVES)) {
      const generated = GENERATED.machines.find((m) => m.name === machineName);
      for (const move of moves) {
        const [file, fn] = move.source.split(' ');
        const source = readFileSync(new URL(`../../../${file}`, import.meta.url), 'utf8');
        assert.match(source, new RegExp(`fn ${fn}\\b`), `${file} still defines ${fn}`);
        for (const evidence of move.evidence) {
          assert.match(
            source,
            new RegExp(evidence),
            `${fn} still accepts ${move.from} -> ${move.to}`,
          );
        }
        // Not in the generated catalog (else this entry is redundant) ...
        const before = generated.states.find((s) => s.name === move.from);
        assert.ok(!before.next.includes(move.to), `${machineName} catalog omits the move`);
        // ... but merged into what the tools report.
        assert.ok(nextStates(machineName, move.from).includes(move.to));
      }
    }
  });
});

describe('UNCATALOGUED_LIFECYCLES', () => {
  it("lists each lifecycle's states exactly as the binding's generated status type", () => {
    for (const lifecycle of UNCATALOGUED_LIFECYCLES) {
      assert.deepEqual(lifecycle.states, bindingUnion(lifecycle.tsType), lifecycle.name);
    }
  });

  it('never shadows or duplicates a catalogued machine', () => {
    const catalogued = new Set(STATE_MACHINES.map((m) => m.name));
    const names = UNCATALOGUED_LIFECYCLES.flatMap((l) => [l.name, ...(l.aliases ?? [])]);
    assert.equal(new Set(names).size, names.length, 'names and aliases are unique');
    assert.deepEqual(
      names.filter((name) => catalogued.has(name)),
      [],
    );
    for (const lifecycle of UNCATALOGUED_LIFECYCLES) {
      assert.equal(resolveLifecycle(lifecycle.name).kind, 'uncatalogued', lifecycle.name);
      assert.equal(resolveLifecycle(lifecycle.tsType).kind, 'uncatalogued', lifecycle.tsType);
    }
  });

  it('covers the lifecycles agents ask about that the catalog does not publish', () => {
    for (const name of [
      'order_payment',
      'order_fulfillment',
      'gift_card',
      'store_credit',
      'promotion',
      'coupon',
      'inventory_reservation',
      'invoice',
      'backorder',
      'x402_intent',
      'payment_obligation',
      'gl_period',
    ]) {
      assert.equal(resolveLifecycle(name).kind, 'uncatalogued', name);
    }
  });
});
