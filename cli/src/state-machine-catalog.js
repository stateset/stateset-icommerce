/**
 * State machine catalog -- "where can this record go next?"
 *
 * The catalogued status lifecycles the engine enforces (order, payment,
 * return, shipment, cart, subscription, ...) are published as data in
 * `./generated/state-machines.json`. That file is generated from the Rust
 * transition rules themselves (`stateset_core::state_machine::catalog`) and a
 * Rust test fails when it is stale, so what this module reports is what the
 * engine will accept -- it is not a second, hand-maintained copy.
 *
 * Not every lifecycle is catalogued yet (gift cards, invoices, x402 intents,
 * an order's payment and fulfilment status, ...). Those are named in
 * `./state-machine-operations.js` so they are reported as "known, not yet
 * catalogued" rather than "unknown"; their moves are never guessed. That file
 * also says which tool performs each catalogued move, and lists the one move
 * the persistence layer accepts that the generator omits (payment
 * pending -> completed), which is merged in here with its Rust source cited.
 *
 * Regenerate after changing a transition rule:
 *   UPDATE_STATE_MACHINES=1 cargo test -p stateset-core --test state_machine_catalog
 *
 * Nothing here reads or writes the store.
 */

import { readFileSync } from 'node:fs';

import { z } from 'zod';

import {
  PERSISTENCE_MOVES,
  STATUS_GATED_TOOLS,
  STATUS_PARAMETER_TOOLS,
  TRANSITION_TOOLS,
  UNCATALOGUED_LIFECYCLES,
} from './state-machine-operations.js';

const CATALOG = JSON.parse(
  readFileSync(new URL('./generated/state-machines.json', import.meta.url), 'utf8'),
);

/** @typedef {{ name: string, next: string[], terminal: boolean, reentrant: boolean }} State */
/** @typedef {{ name: string, domain: string, rust_type: string, initial: string, states: State[] }} Machine */

/**
 * The generated machines, plus the few persistence-layer moves the generator
 * does not emit (PERSISTENCE_MOVES, each citing its Rust rule).
 * @type {readonly Machine[]}
 */
export const STATE_MACHINES = Object.freeze(
  CATALOG.machines.map((machine) => {
    const extra = PERSISTENCE_MOVES[machine.name];
    if (!extra) return machine;
    return {
      ...machine,
      states: machine.states.map((state) => {
        const added = extra
          .filter((move) => move.from === state.name && !state.next.includes(move.to))
          .map((move) => move.to);
        return added.length > 0 ? { ...state, next: [...state.next, ...added] } : state;
      }),
    };
  }),
);

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
 * `{ allowedNext }` for a read tool's result: the states a record in `status`
 * can move to next. `{}` when the status is not a state of the machine, so a
 * read never claims a terminal state it does not know.
 * @returns {{ allowedNext?: string[] }}
 */
export function allowedNextField(machineName, status) {
  const next =
    status === undefined || status === null ? undefined : nextStates(machineName, status);
  return next ? { allowedNext: next } : {};
}

/**
 * The catalog's spelling of a state (`Canceled` -> `cancelled`), or
 * `undefined` when the machine or the state is unknown.
 * @returns {string | undefined}
 */
export function canonicalState(machineName, value) {
  if (value === undefined || value === null || value === '') return undefined;
  const machine = findStateMachine(machineName);
  return machine ? findState(machine, value)?.name : undefined;
}

/** States with a move into `to`: where a record must be for that move. */
export function acceptedFrom(machineName, to) {
  const machine = findStateMachine(machineName);
  const target = machine && findState(machine, to);
  if (!target) return undefined;
  return machine.states.filter((s) => s.next.includes(target.name)).map((s) => s.name);
}

/**
 * The tools that move a `machine` record from `from` into `to` (see
 * TRANSITION_TOOLS). Empty when no tool makes that move.
 * @returns {import('./state-machine-operations.js').MoveTool[]}
 */
export function toolsForMove(machineName, from, to) {
  const machine = findStateMachine(machineName);
  const source = machine && findState(machine, from);
  const target = machine && findState(machine, to);
  if (!source || !target) return [];
  return (TRANSITION_TOOLS[machine.name]?.[target.name] ?? []).filter(
    (move) => !move.from || move.from.includes(source.name),
  );
}

/**
 * Every move out of `from`, each with the tools that make it, or `undefined`
 * when the machine or the state is unknown.
 * @returns {{ status: string, tools: string[] }[] | undefined}
 */
export function describeMoves(machineName, from) {
  const next = nextStates(machineName, from);
  if (!next) return undefined;
  return next.map((status) => ({
    status,
    tools: toolsForMove(machineName, from, status).map((move) => move.tool),
  }));
}

/** `in_transit (add_return_tracking), cancelled (cancel_return)` */
export function formatMoves(moves) {
  return moves
    .map(({ status, tools }) => (tools.length > 0 ? `${status} (${tools.join(' or ')})` : status))
    .join(', ');
}

/** tool -> { machine, targets } for every tool named in TRANSITION_TOOLS. */
const TOOL_MOVES = (() => {
  const index = new Map();
  for (const [machine, targets] of Object.entries(TRANSITION_TOOLS)) {
    for (const [target, moves] of Object.entries(targets)) {
      for (const { tool } of moves) {
        const entry = index.get(tool) ?? { machine, targets: [] };
        entry.targets.push(target);
        index.set(tool, entry);
      }
    }
  }
  return index;
})();

/**
 * The catalog machine whose status `toolName` moves -- or, for a tool such as
 * `create_refund`, whose status gates it.
 * @returns {string | undefined}
 */
export function machineForTool(toolName) {
  return TOOL_MOVES.get(toolName)?.machine ?? STATUS_GATED_TOOLS[toolName];
}

/**
 * The move a call of `toolName` with `params` asks for: the machine and the
 * status(es) it would move the record into. `null` when the call is not a
 * catalogued move (e.g. `update_customer` without a `status`).
 * @returns {{ machine: string, targets: string[] } | null}
 */
export function requestedMove(toolName, params = {}) {
  const entry = TOOL_MOVES.get(toolName);
  if (!entry) return null;
  const statusParam = STATUS_PARAMETER_TOOLS[toolName];
  if (statusParam) {
    const target = canonicalState(entry.machine, params?.[statusParam]);
    return target ? { machine: entry.machine, targets: [target] } : null;
  }
  return { machine: entry.machine, targets: [...entry.targets] };
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

/** target -> tool names, for the targets some tool reaches. */
function toolsByTarget(machine) {
  return Object.fromEntries(
    Object.entries(TRANSITION_TOOLS[machine.name] ?? {}).map(([target, moves]) => [
      target,
      moves.map((move) => move.tool),
    ]),
  );
}

/**
 * The catalogued moves only one operation can make: a plain status update is
 * refused for them (e.g. processing -> shipped needs ship_order).
 */
function onlyViaMoves(machine, from = undefined) {
  const moves = [];
  for (const state of machine.states) {
    if (from !== undefined && state.name !== from) continue;
    for (const next of state.next) {
      for (const move of toolsForMove(machine.name, state.name, next)) {
        if (move.only) {
          moves.push({
            from: state.name,
            to: next,
            tool: move.tool,
            ...(move.note ? { note: move.note } : {}),
          });
        }
      }
    }
  }
  return moves;
}

const sameDomainLifecycles = (domain, except) => [
  ...STATE_MACHINES.filter((m) => m.domain === domain && m.name !== except).map((m) => m.name),
  ...UNCATALOGUED_LIFECYCLES.filter((l) => l.domain === domain && l.name !== except).map(
    (l) => l.name,
  ),
];

/**
 * Resolve what an agent calls a lifecycle: a catalogued machine (by name, Rust
 * type, or a domain holding exactly one), a known lifecycle the catalog does
 * not publish yet, a domain holding several (ambiguous), or nothing known.
 */
export function resolveLifecycle(identifier) {
  const key = normalize(identifier);
  const keys = key.endsWith('_status') ? [key, key.slice(0, -'_status'.length)] : [key];
  for (const candidate of keys) {
    const machine =
      STATE_MACHINES.find((m) => m.name === candidate) ??
      STATE_MACHINES.find((m) => normalize(m.rust_type) === candidate);
    if (machine) return { kind: 'catalogued', machine };
    const lifecycle = UNCATALOGUED_LIFECYCLES.find(
      (l) =>
        l.name === candidate ||
        l.aliases?.includes(candidate) ||
        normalize(l.tsType) === candidate ||
        (l.rustType !== undefined && normalize(l.rustType) === candidate),
    );
    if (lifecycle) return { kind: 'uncatalogued', lifecycle };
  }
  const catalogued = STATE_MACHINES.filter((m) => m.domain === key);
  const uncatalogued = UNCATALOGUED_LIFECYCLES.filter((l) => l.domain === key);
  if (catalogued.length === 1) return { kind: 'catalogued', machine: catalogued[0] };
  if (catalogued.length === 0 && uncatalogued.length === 1) {
    return { kind: 'uncatalogued', lifecycle: uncatalogued[0] };
  }
  if (catalogued.length + uncatalogued.length > 1) {
    return {
      kind: 'ambiguous',
      domain: key,
      candidates: [
        ...catalogued.map((m) => ({ ...summarize(m), catalogued: true })),
        ...uncatalogued.map((l) => ({
          machine: l.name,
          domain: l.domain,
          states: [...l.states],
          catalogued: false,
        })),
      ],
    };
  }
  return { kind: 'unknown' };
}

/** A known lifecycle whose moves the catalog does not publish: say so, never guess. */
function explainUncatalogued(lifecycle, from, to) {
  const base = {
    success: true,
    catalogued: false,
    machine: lifecycle.name,
    domain: lifecycle.domain,
    states: [...lifecycle.states],
    reason:
      `${lifecycle.name} is a known lifecycle that is not catalogued yet: the engine has it, ` +
      'but its transition rules are not in the generated catalog, so this tool cannot say ' +
      'which moves it accepts. Run the operation as a preview and read its result.',
    ...(lifecycle.note ? { note: lifecycle.note } : {}),
  };
  if (from === undefined) {
    if (to !== undefined) {
      return { success: false, error: 'Pass `from` (the current status) together with `to`.' };
    }
    return base;
  }
  const known = (value) =>
    lifecycle.states.find((s) => s === (STATE_ALIASES[normalize(value)] ?? normalize(value)));
  const current = known(from);
  if (!current) {
    return {
      success: false,
      error: `"${from}" is not a ${lifecycle.name} state (from).`,
      machine: lifecycle.name,
      states: [...lifecycle.states],
    };
  }
  if (to === undefined) return { ...base, from: current, next: null };
  const target = known(to);
  if (!target) {
    return {
      success: false,
      error: `"${to}" is not a ${lifecycle.name} state (to).`,
      machine: lifecycle.name,
      states: [...lifecycle.states],
    };
  }
  return { ...base, from: current, to: target, allowed: null, next: null };
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
  const uncataloguedNames = UNCATALOGUED_LIFECYCLES.map((l) => l.name);

  if (machineId === undefined) {
    if (from !== undefined || to !== undefined) {
      return {
        success: false,
        error: 'Pass `machine` together with `from`/`to`.',
        machines: STATE_MACHINES.map((m) => m.name),
      };
    }
    return {
      success: true,
      count: STATE_MACHINES.length,
      machines: STATE_MACHINES.map(summarize),
      uncatalogued: UNCATALOGUED_LIFECYCLES.map((l) => ({ machine: l.name, domain: l.domain })),
    };
  }

  const resolved = resolveLifecycle(machineId);
  if (resolved.kind === 'uncatalogued') return explainUncatalogued(resolved.lifecycle, from, to);
  if (resolved.kind === 'ambiguous') {
    const names = resolved.candidates.map((c) => c.machine);
    return {
      success: false,
      error: `"${machineId}" names several lifecycles: ${names.join(', ')}.`,
      hint: `Pass one of them as \`machine\` (e.g. "${names[0]}").`,
      candidates: resolved.candidates,
    };
  }
  if (resolved.kind === 'unknown') {
    return {
      success: false,
      error: `Unknown state machine "${machineId}".`,
      machines: STATE_MACHINES.map((m) => m.name),
      uncatalogued: uncataloguedNames,
    };
  }
  const { machine } = resolved;

  if (from === undefined) {
    if (to !== undefined) {
      return { success: false, error: 'Pass `from` (the current status) together with `to`.' };
    }
    const related = sameDomainLifecycles(machine.domain, machine.name);
    return {
      success: true,
      ...summarize(machine),
      rustType: machine.rust_type,
      entryStates: entryStates(machine),
      transitions: Object.fromEntries(machine.states.map((s) => [s.name, s.next])),
      tools: toolsByTarget(machine),
      onlyVia: onlyViaMoves(machine),
      ...(PERSISTENCE_MOVES[machine.name]
        ? { persistenceMoves: PERSISTENCE_MOVES[machine.name] }
        : {}),
      ...(related.length > 0 ? { related } : {}),
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
      moves: describeMoves(machine.name, current.name),
      onlyVia: onlyViaMoves(machine, current.name),
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
    const moves = toolsForMove(machine.name, current.name, target.name);
    const only = moves.find((move) => move.only);
    return {
      ...base,
      allowed: true,
      next: current.next,
      tools: moves.map((move) => move.tool),
      ...(only
        ? {
            onlyVia: only.tool,
            reason:
              `Allowed, but only through ${only.tool}; a plain status update is refused` +
              `${only.note ? ` (${only.note})` : ''}.`,
          }
        : {}),
    };
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
  return {
    ...base,
    allowed: false,
    reason,
    next: current.next,
    path,
    moves: describeMoves(machine.name, current.name),
  };
}

export const stateMachineTool = {
  name: 'explain_state_machine',
  description:
    'Ask what a record can do next BEFORE trying to change its status. With no arguments, lists ' +
    'the catalogued status lifecycles (order, payment, refund, return, shipment, cart, customer, ' +
    'product, subscription, purchase_order, ...) and the known lifecycles not catalogued yet. ' +
    'With `machine`, returns its full transition graph and the tool that makes each move. With ' +
    '`machine` + `from`, the statuses reachable next and the tool for each. With `machine` + ' +
    '`from` + `to`, whether that move is allowed and, if not, the shortest route. Generated from ' +
    'the engine rules; read-only and touches no data. Some moves only one operation can make ' +
    '(`onlyVia`, e.g. processing -> shipped via ship_order); a plain status update is refused ' +
    'for them. For a known lifecycle that is not catalogued yet (order_payment, ' +
    'order_fulfillment, gift_card, coupon, invoice, x402_intent, gl_period, ...) it says so and ' +
    'lists its states without guessing its moves.',
  inputSchema: {
    machine: z
      .string()
      .min(1)
      .max(64)
      .optional()
      .describe(
        'Machine name, e.g. "order", "payment", "return", "shipment" (a Rust type such as ' +
          '"OrderStatus" or a single-lifecycle domain also works). Omit to list all',
      ),
    from: z.string().min(1).max(64).optional().describe('Current status, e.g. "pending"'),
    to: z.string().min(1).max(64).optional().describe('Status you want to reach, e.g. "delivered"'),
  },
  permission: 'read',
  handler: explainStateMachine,
};
