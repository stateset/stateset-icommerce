// Drift tests for the admin's engine lifecycle vocabulary.
//
// `src/lib/engine-lifecycles.ts` copies two things from outside the Next.js
// project: the engine's StateMachine catalog (cli/src/generated/
// state-machines.json) and the status unions the native binding declares
// (bindings/node/index.d.ts). These tests read both files from the repo and
// fail when the admin's copy disagrees with either, so a renamed or added
// engine status cannot leave the admin rendering an invented vocabulary.

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { describe, expect, it } from 'vitest';

import {
  BILL_STATUSES,
  CREDIT_MEMO_STATUSES,
  ENGINE_LIFECYCLES,
  GL_ACCOUNT_STATUSES,
  INSPECTION_STATUSES,
  INVOICE_STATUSES,
  JOURNAL_ENTRY_STATUSES,
  LOT_STATUSES,
  NCR_SEVERITIES,
  NCR_STATUSES,
  ORDER_STATUSES,
  ORDER_STATUS_UPDATES,
  PICK_TASK_STATUSES,
  PURCHASE_ORDER_STATUSES,
  RECEIPT_STATUSES,
  RETURN_LIFECYCLE,
  RETURN_REASONS,
  RETURN_STATUSES,
  SERIAL_STATUSES,
  SUBSCRIPTION_STATUSES,
  WAVE_STATUSES,
  WORK_ORDER_STATUSES,
  canApplyReturnAction,
  canTransition,
  isTerminal,
  nextStates,
  normalizeEngineStatus,
  openStates,
  ORDER_LIFECYCLE,
  type Lifecycle,
  type ReturnAction,
} from '@/lib/engine-lifecycles';

const REPO_ROOT = path.resolve(__dirname, '..', '..', '..', '..');

interface CatalogState {
  name: string;
  next: string[];
  terminal: boolean;
}
interface CatalogMachine {
  name: string;
  initial: string;
  states: CatalogState[];
}

const catalog = JSON.parse(
  readFileSync(path.join(REPO_ROOT, 'cli/src/generated/state-machines.json'), 'utf8'),
) as { machines: CatalogMachine[] };

const bindingTypes = readFileSync(path.join(REPO_ROOT, 'bindings/node/index.d.ts'), 'utf8');

/** The string literals of `export type <name> = ...` in the binding's d.ts. */
function bindingUnion(name: string): string[] {
  const start = bindingTypes.indexOf(`export type ${name} =`);
  if (start === -1) throw new Error(`bindings/node/index.d.ts declares no type ${name}`);
  const rest = bindingTypes.slice(start + `export type ${name} =`.length);
  // The union ends at the first line that is not a continuation (`| ...`).
  const lines = rest.split('\n');
  const body = [lines[0]];
  for (const line of lines.slice(1)) {
    if (!/^\s*\|/.test(line)) break;
    body.push(line);
  }
  return Array.from(body.join('\n').matchAll(/'([^']+)'/g), (match) => match[1]);
}

const sorted = (values: readonly string[]) => [...values].sort();

describe('normalizeEngineStatus', () => {
  it.each([
    ['PartiallyPaid', 'partially_paid'],
    ['FullyApplied', 'fully_applied'],
    ['InProgress', 'in_progress'],
    ['OnHold', 'on_hold'],
    ['Quarantine', 'quarantine'],
    ['PendingDisposition', 'pending_disposition'],
    ['Posted', 'posted'],
    ['partially_paid', 'partially_paid'],
    ['in_transit', 'in_transit'],
    ['open', 'open'],
  ])('%s -> %s', (input, expected) => {
    expect(normalizeEngineStatus(input)).toBe(expected);
  });

  it('is idempotent and leaves non-strings alone', () => {
    expect(normalizeEngineStatus(normalizeEngineStatus('PuttingAway'))).toBe('putting_away');
    expect(normalizeEngineStatus(undefined)).toBeUndefined();
    expect(normalizeEngineStatus(null)).toBeNull();
    expect(normalizeEngineStatus(3)).toBe(3);
  });
});

describe('lifecycles match the engine StateMachine catalog', () => {
  it.each(
    ENGINE_LIFECYCLES.map((lifecycle) => [lifecycle.name, lifecycle as Lifecycle<string>] as const),
  )('%s', (name, lifecycle) => {
    const machine = catalog.machines.find((candidate) => candidate.name === name);
    expect(machine, `catalog has no machine named ${name}`).toBeDefined();
    if (!machine) return;

    expect(lifecycle.initial).toBe(machine.initial);
    expect(sorted(Object.keys(lifecycle.next))).toEqual(
      sorted(machine.states.map((state) => state.name)),
    );
    for (const state of machine.states) {
      const copied = (lifecycle.next as Record<string, readonly string[]>)[state.name];
      expect(sorted(copied), `${name}.${state.name} next states`).toEqual(sorted(state.next));
      expect(isTerminal(lifecycle, state.name), `${name}.${state.name} terminal`).toBe(
        state.terminal,
      );
    }
  });
});

describe('vocabularies match the binding status unions', () => {
  it.each([
    ['OrderStatus', ORDER_STATUSES],
    ['OrderStatusUpdate', ORDER_STATUS_UPDATES],
    ['ReturnStatus', RETURN_STATUSES],
    ['ReturnReason', RETURN_REASONS],
    ['SubscriptionStatus', SUBSCRIPTION_STATUSES],
    ['PurchaseOrderStatus', PURCHASE_ORDER_STATUSES],
    ['WorkOrderStatus', WORK_ORDER_STATUSES],
    ['InvoiceStatus', INVOICE_STATUSES],
  ] as const)('%s (rendered snake_case)', (typeName, admin) => {
    expect(sorted(admin)).toEqual(sorted(bindingUnion(typeName)));
  });

  it.each([
    ['BillStatus', BILL_STATUSES, 'BillStatusInput'],
    ['CreditMemoStatus', CREDIT_MEMO_STATUSES, 'CreditMemoStatusInput'],
    ['LotStatus', LOT_STATUSES, 'LotStatusInput'],
    ['SerialStatus', SERIAL_STATUSES, 'SerialStatusInput'],
    ['InspectionStatus', INSPECTION_STATUSES, null],
    ['NcrStatus', NCR_STATUSES, 'NcrStatusInput'],
    ['NcrSeverity', NCR_SEVERITIES, 'NcrSeverityInput'],
    ['WaveStatus', WAVE_STATUSES, 'WaveStatusInput'],
    ['PickTaskStatus', PICK_TASK_STATUSES, 'PickTaskStatusInput'],
    ['ReceiptStatus', RECEIPT_STATUSES, 'ReceiptStatusInput'],
    ['GlAccountStatus', GL_ACCOUNT_STATUSES, 'GlAccountStatusInput'],
    ['GlJournalEntryStatus', JOURNAL_ENTRY_STATUSES, 'GlJournalEntryStatusInput'],
  ] as const)('%s (rendered Debug, normalised at the boundary)', (typeName, admin, inputName) => {
    const rendered = bindingUnion(typeName);
    // The binding really does render these PascalCase — that is why the
    // admin normalises them.
    expect(rendered.every((value) => /^[A-Z]/.test(value))).toBe(true);
    const normalised = rendered.map((value) => normalizeEngineStatus(value));
    expect(sorted(admin)).toEqual(sorted(normalised));
    if (inputName) {
      // ...and the normalised spelling is the engine's own snake_case: the
      // filter input accepts every one of them.
      const accepted = bindingUnion(inputName);
      for (const value of normalised) expect(accepted).toContain(value);
    }
  });
});

describe('lifecycle helpers', () => {
  it('reports next states, terminal states and the open queue', () => {
    expect(nextStates(ORDER_LIFECYCLE, 'processing')).toEqual([
      'partially_shipped',
      'shipped',
      'cancelled',
    ]);
    expect(nextStates(ORDER_LIFECYCLE, 'not-a-status')).toEqual([]);
    expect(canTransition(ORDER_LIFECYCLE, 'delivered', 'refunded')).toBe(true);
    expect(canTransition(ORDER_LIFECYCLE, 'shipped', 'cancelled')).toBe(false);
    expect(isTerminal(ORDER_LIFECYCLE, 'refunded')).toBe(true);
    expect(isTerminal(ORDER_LIFECYCLE, 'not-a-status')).toBe(false);
    expect(sorted(openStates(RETURN_LIFECYCLE))).toEqual(
      sorted(['requested', 'approved', 'in_transit', 'received', 'inspecting']),
    );
  });

  // Which return actions the admin may offer in each state, straight from
  // the catalog: this is the table the RMA inbox renders buttons from.
  const RETURN_ACTIONS_BY_STATUS: Record<string, ReturnAction[]> = {
    requested: ['approve', 'reject', 'cancel'],
    approved: ['cancel', 'addTracking'],
    in_transit: ['markReceived'],
    received: ['complete'],
    inspecting: ['reject', 'complete'],
    rejected: [],
    completed: [],
    cancelled: [],
  };

  it.each(Object.entries(RETURN_ACTIONS_BY_STATUS))(
    'return actions allowed from %s',
    (status, allowed) => {
      const actions: ReturnAction[] = [
        'approve',
        'reject',
        'cancel',
        'addTracking',
        'markReceived',
        'complete',
      ];
      expect(actions.filter((action) => canApplyReturnAction(status, action))).toEqual(allowed);
    },
  );
});
