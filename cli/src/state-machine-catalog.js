/**
 * State machine catalog -- "where can this record go next?"
 *
 * Every status lifecycle the engine enforces (order, payment, return,
 * shipment, cart, subscription, ...) is published as data in
 * `./generated/state-machines.json`. That file is generated from the Rust
 * transition rules themselves (`stateset_core::state_machine::catalog`) and a
 * Rust test fails when it is stale, so what this module reports is what the
 * engine will accept -- it is not a second, hand-maintained copy.
 *
 * Regenerate after changing a transition rule:
 *   UPDATE_STATE_MACHINES=1 cargo test -p stateset-core --test state_machine_catalog
 *
 * Nothing here reads or writes the store.
 */

import { readFileSync } from 'node:fs';

import { z } from 'zod';

const CATALOG = JSON.parse(
  readFileSync(new URL('./generated/state-machines.json', import.meta.url), 'utf8'),
);

/** @typedef {{ name: string, next: string[], terminal: boolean, reentrant: boolean }} State */
/** @typedef {{ name: string, domain: string, rust_type: string, initial: string, states: State[] }} Machine */

/** @type {readonly Machine[]} */
export const STATE_MACHINES = Object.freeze(CATALOG.machines);

/** `In Transit`, `in-transit`, `InTransit` and `in_transit` all name one state. */
function normalize(value) {
  return String(value)
    .trim()
    .replace(/([a-z0-9])([A-Z])/g, '$1_$2')
    .replace(/[\s-]+/g, '_')
    .toLowerCase();
}

/** Spellings the engine itself accepts for the same state. */
const STATE_ALIASES = { canceled: 'cancelled' };

/**
 * Find a machine by name (`order`), Rust type (`OrderStatus`) or, when the
 * domain has exactly one machine, by domain (`orders`).
 * @returns {Machine | undefined}
 */
export function findStateMachine(identifier) {
  const key = normalize(identifier);
  const byName = STATE_MACHINES.find((m) => m.name === key);
  if (byName) return byName;
  const byType = STATE_MACHINES.find((m) => normalize(m.rust_type) === key);
  if (byType) return byType;
  const inDomain = STATE_MACHINES.filter((m) => m.domain === key);
  return inDomain.length === 1 ? inDomain[0] : undefined;
}

/** @returns {State | undefined} */
function findState(machine, value) {
  const key = normalize(value);
  const name = STATE_ALIASES[key] ?? key;
  return machine.states.find((s) => s.name === name);
}

/**
 * States reachable from `from` in one step, or `undefined` when the machine
 * or the state is unknown.
 * @returns {string[] | undefined}
 */
export function nextStates(machineName, from) {
  const machine = findStateMachine(machineName);
  const state = machine && findState(machine, from);
  return state ? [...state.next] : undefined;
}

/**
 * Shortest sequence of states from `from` to `to` (both inclusive), or `null`
 * when `to` cannot be reached.
 */
function shortestPath(machine, from, to) {
  const previous = new Map([[from, null]]);
  const queue = [from];
  while (queue.length > 0) {
    const current = queue.shift();
    if (current === to) {
      const path = [];
      for (let at = to; at !== null; at = previous.get(at)) path.unshift(at);
      return path;
    }
    for (const next of machine.states.find((s) => s.name === current).next) {
      if (!previous.has(next)) {
        previous.set(next, current);
        queue.push(next);
      }
    }
  }
  return null;
}

/** States nothing transitions into: where a record can start. */
function entryStates(machine) {
  const targets = new Set(machine.states.flatMap((s) => s.next));
  const entries = machine.states.map((s) => s.name).filter((name) => !targets.has(name));
  return entries.includes(machine.initial) ? entries : [machine.initial, ...entries];
}

function summarize(machine) {
  return {
    machine: machine.name,
    domain: machine.domain,
    initial: machine.initial,
    states: machine.states.map((s) => s.name),
    terminal: machine.states.filter((s) => s.terminal).map((s) => s.name),
  };
}

function toMermaid(machine) {
  const lines = ['stateDiagram-v2', `    [*] --> ${machine.initial}`];
  for (const state of machine.states) {
    for (const next of state.next) lines.push(`    ${state.name} --> ${next}`);
    if (state.terminal) lines.push(`    ${state.name} --> [*]`);
  }
  return lines.join('\n');
}

function unknownState(machine, role, value) {
  return {
    success: false,
    error: `"${value}" is not a ${machine.name} state (${role}).`,
    machine: machine.name,
    states: machine.states.map((s) => s.name),
  };
}

/**
 * `explain_state_machine` handler. Four shapes, by which params are given:
 *   (none)            every machine, summarised
 *   machine           the full graph
 *   machine+from      what can happen next
 *   machine+from+to   is this move allowed, and if not, the route there
 */
export function explainStateMachine({ params = {} } = {}) {
  const { machine: machineId, from, to } = params;

  if (machineId === undefined) {
    if (from !== undefined || to !== undefined) {
      return {
        success: false,
        error: 'Pass `machine` together with `from`/`to`.',
        machines: STATE_MACHINES.map((m) => m.name),
      };
    }
    return { success: true, count: STATE_MACHINES.length, machines: STATE_MACHINES.map(summarize) };
  }

  const machine = findStateMachine(machineId);
  if (!machine) {
    return {
      success: false,
      error: `Unknown state machine "${machineId}".`,
      machines: STATE_MACHINES.map((m) => m.name),
    };
  }

  if (from === undefined) {
    if (to !== undefined) {
      return { success: false, error: 'Pass `from` (the current status) together with `to`.' };
    }
    return {
      success: true,
      ...summarize(machine),
      rustType: machine.rust_type,
      entryStates: entryStates(machine),
      transitions: Object.fromEntries(machine.states.map((s) => [s.name, s.next])),
      mermaid: toMermaid(machine),
    };
  }

  const current = findState(machine, from);
  if (!current) return unknownState(machine, 'from', from);

  if (to === undefined) {
    return {
      success: true,
      machine: machine.name,
      from: current.name,
      next: current.next,
      terminal: current.terminal,
    };
  }

  const target = findState(machine, to);
  if (!target) return unknownState(machine, 'to', to);

  const base = { success: true, machine: machine.name, from: current.name, to: target.name };

  if (current.name === target.name) {
    return {
      ...base,
      allowed: current.reentrant,
      reason: current.reentrant
        ? `Already ${current.name}; setting it again is accepted as a no-op.`
        : `Already ${current.name}; setting it again is refused.`,
      next: current.next,
    };
  }

  if (current.next.includes(target.name)) {
    return { ...base, allowed: true, next: current.next };
  }

  const path = shortestPath(machine, current.name, target.name);
  let reason;
  if (current.terminal) {
    reason = `${current.name} is a terminal ${machine.name} state; nothing can follow it.`;
  } else if (path) {
    reason =
      `${machine.name} cannot move from ${current.name} straight to ${target.name}. ` +
      `Shortest route: ${path.join(' -> ')}.`;
  } else {
    reason =
      `${target.name} cannot be reached from ${current.name}. ` +
      `Allowed next: ${current.next.join(', ')}.`;
  }
  return { ...base, allowed: false, reason, next: current.next, path };
}

export const stateMachineTool = {
  name: 'explain_state_machine',
  description:
    'Ask what a record can do next BEFORE trying to change its status. With no arguments, lists ' +
    'every status lifecycle the engine enforces (order, payment, refund, return, shipment, cart, ' +
    'subscription, purchase_order, ...). With `machine`, returns its full transition graph. With ' +
    '`machine` + `from`, the statuses reachable next. With `machine` + `from` + `to`, whether ' +
    'that move is allowed and, if not, the shortest route. Generated from the engine rules; ' +
    'read-only and touches no data. A status reached only by an operation (e.g. shipping an ' +
    'order) still needs that operation, not a plain status update.',
  inputSchema: {
    machine: z
      .string()
      .min(1)
      .max(64)
      .optional()
      .describe('Machine name, e.g. "order", "payment", "return", "shipment". Omit to list all'),
    from: z.string().min(1).max(64).optional().describe('Current status, e.g. "pending"'),
    to: z.string().min(1).max(64).optional().describe('Status you want to reach, e.g. "delivered"'),
  },
  permission: 'read',
  handler: explainStateMachine,
};
