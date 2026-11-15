// Guidance for status moves, from the engine's state machine catalog.
//
// The engine refuses a status move it does not allow with a sentence such as
// "return cannot transition from approved to received" -- true, but an agent
// still has to work out what it CAN do. This module reads such a refusal (or
// a preview of a move), finds the catalogued machine and the record's current
// state, and answers with:
//
//   allowedNext  the states the record can move to from where it is
//   hint         those moves with the tool that makes each, and the route to
//                the requested state when there is one
//   transition   { machine, from, to } as understood from the refusal
//
// It never decides anything: the engine already refused (or will judge) the
// move. Nothing here reads or writes the store -- a preview states only the
// catalog's precondition, because no transition tool loads its record before
// --apply.

import {
  acceptedFrom,
  canonicalState,
  describeMoves,
  explainStateMachine,
  findStateMachine,
  formatMoves,
  machineForTool,
  requestedMove,
} from '../state-machine-catalog.js';
import { TRANSITION_TOOLS } from '../state-machine-operations.js';

const WORD = "'?([a-z][a-z_]*)'?";

/**
 * Refusal sentences the engine and the kernel produce for status moves.
 * `machine`, `from`, `to` are capture-group indexes; a missing machine or
 * target is taken from the tool that was called.
 */
const REFUSALS = [
  // kernel plans, product/customer rules: "order cannot transition from pending to delivered",
  // "product status cannot transition from archived to active"
  {
    re: new RegExp(`\\b([a-z][a-z ]*?)(?: status)? cannot transition from ${WORD} to ${WORD}`, 'i'),
    machine: 1,
    from: 2,
    to: 3,
  },
  // shipments: "Invalid shipment status transition from pending to delivered"
  {
    re: new RegExp(`\\binvalid ([a-z_]+) status transition from ${WORD} to ${WORD}`, 'i'),
    machine: 1,
    from: 2,
    to: 3,
  },
  // the engine's StateTransitionError: "invalid state transition from X to Y"
  { re: new RegExp(`\\binvalid state transition from ${WORD} to ${WORD}`, 'i'), from: 1, to: 2 },
  // payments: "Payment is completed and cannot transition to cancelled"
  {
    re: new RegExp(`\\b([a-z_]+) is ${WORD} and cannot transition to ${WORD}`, 'i'),
    machine: 1,
    from: 2,
    to: 3,
  },
  // carts: "Cart cannot be cancelled in status: expired", "Cart cannot begin checkout in status: expired"
  {
    re: new RegExp(`\\b([a-z_]+) cannot [a-z ]+? in status:? ${WORD}`, 'i'),
    machine: 1,
    from: 2,
  },
  // refunds: "Cannot refund a payment in status 'pending'"
  {
    re: new RegExp(`\\bcannot [a-z]+ an? ([a-z_]+) in status:? ${WORD}`, 'i'),
    machine: 1,
    from: 2,
  },
  // "customer <id> is deleted and can no longer be updated"
  {
    re: new RegExp(`\\b([a-z_]+) \\S+ is ${WORD} and can no longer be\\b`, 'i'),
    machine: 1,
    from: 2,
  },
  // "Operation not permitted: payment is already completed"
  { re: new RegExp(`\\b([a-z_]+) is already ${WORD}`, 'i'), machine: 1, from: 2 },
];

/** "an order", "a return". */
/** "a", "a or b", "a, b or c". */
const listOr = (items) =>
  items.length <= 1 ? items.join('') : `${items.slice(0, -1).join(', ')} or ${items.at(-1)}`;
const capitalize = (text) => text.charAt(0).toUpperCase() + text.slice(1);
const withArticle = (noun) => `${/^[aeiou]/i.test(noun) ? 'an' : 'a'} ${noun}`;

/** "the return" -> "return"; "fixed asset" -> "fixed_asset"; unknown -> undefined. */
function machineFromWords(text) {
  const words = String(text).trim().toLowerCase().split(/\s+/);
  for (let start = 0; start < words.length; start += 1) {
    const machine = findStateMachine(words.slice(start).join('_'));
    if (machine) return machine.name;
  }
  return undefined;
}

/** The single state a call of `toolName` asks for, when it asks for exactly one. */
function requestedTarget(toolName, params, machine) {
  const move = requestedMove(toolName, params);
  return move && move.machine === machine && move.targets.length === 1
    ? move.targets[0]
    : undefined;
}

/** Kernel codes whose refusal is about which operation, not which state. */
const OPERATION_REFUSALS = {
  'commerce.shipment_command_required': {
    machine: 'order',
    hint:
      'Shipping is its own operation: call ship_order (pass `lines` to ship part of the order). ' +
      'update_order_status never sets shipped or partially_shipped.',
  },
};

/** Tools reaching `state` (from any state): "reached via add_return_tracking". */
function toolsInto(machine, state) {
  return [...new Set((TRANSITION_TOOLS[machine]?.[state] ?? []).map((move) => move.tool))];
}

/**
 * Read a refused status move. Returns `null` unless the refusal names a
 * catalogued machine's state.
 *
 * @param {{ toolName?: string, params?: object, message?: string, code?: string }} refusal
 * @returns {{ allowedNext?: string[], transition: { machine: string, from?: string, to?: string },
 *             hint: string } | null}
 */
export function guidanceForRefusal({ toolName, params, message, code } = {}) {
  const operation = code ? OPERATION_REFUSALS[code] : undefined;
  if (operation) {
    const to = requestedTarget(toolName, params, operation.machine);
    return {
      transition: { machine: operation.machine, ...(to ? { to } : {}) },
      hint: operation.hint,
    };
  }
  if (typeof message !== 'string' || message === '') return null;

  const toolMachine = toolName ? machineForTool(toolName) : undefined;
  for (const pattern of REFUSALS) {
    const match = pattern.re.exec(message);
    if (!match) continue;
    const named = pattern.machine ? machineFromWords(match[pattern.machine]) : undefined;
    const machine = named ?? toolMachine;
    if (!machine) continue;
    const from = canonicalState(machine, match[pattern.from]);
    if (!from) continue;
    const to =
      (pattern.to ? canonicalState(machine, match[pattern.to]) : undefined) ??
      (machine === toolMachine ? requestedTarget(toolName, params, machine) : undefined);
    return describeRefusal(machine, from, to);
  }
  return null;
}

function describeRefusal(machine, from, to) {
  const moves = describeMoves(machine, from);
  const allowedNext = moves.map((move) => move.status);
  const sentences = [];
  if (moves.length === 0) {
    sentences.push(
      `${capitalize(withArticle(machine))} in ${from} is final; nothing can follow it.`,
    );
  } else {
    sentences.push(
      `${capitalize(withArticle(machine))} in ${from} can move to: ${formatMoves(moves)}.`,
    );
  }
  if (to && to !== from && !allowedNext.includes(to)) {
    const route = explainStateMachine({ params: { machine, from, to } });
    if (Array.isArray(route.path)) sentences.push(`To reach ${to}: ${route.path.join(' -> ')}.`);
    else if (moves.length > 0) sentences.push(`${to} cannot be reached from ${from}.`);
  }
  return {
    allowedNext,
    transition: { machine, from, ...(to ? { to } : {}) },
    hint: sentences.join(' '),
  };
}

/**
 * What a preview of a catalogued move can say without reading the record:
 * the states the engine accepts the move from. `null` when the call is not a
 * catalogued move.
 *
 * @returns {{ transition: { machine: string, to: string | string[], acceptedFrom: string[] },
 *             sentence: string } | null}
 */
export function previewForMove(toolName, params) {
  const move = toolName ? requestedMove(toolName, params) : null;
  if (!move) return null;
  const sources = [
    ...new Set(move.targets.flatMap((target) => acceptedFrom(move.machine, target) ?? [])),
  ];
  const to = move.targets.length === 1 ? move.targets[0] : move.targets;
  let sentence;
  if (sources.length === 0) {
    sentence = `No ${move.machine} state moves into ${[].concat(to).join(' or ')}; the engine will refuse it.`;
  } else {
    const described =
      sources.length <= 3
        ? sources.map((state) => {
            const via = toolsInto(move.machine, state);
            return via.length > 0 ? `${state} (reached via ${via.join(' or ')})` : state;
          })
        : sources;
    sentence =
      `The engine accepts this move only for ${withArticle(move.machine)} in ${listOr(described)}; ` +
      'from any other status it is refused.';
  }
  return {
    transition: { machine: move.machine, to, acceptedFrom: sources },
    sentence,
  };
}
