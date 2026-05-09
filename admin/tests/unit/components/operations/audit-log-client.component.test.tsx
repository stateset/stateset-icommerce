/**
 * Component tests for `<AuditLogClient />`.
 *
 * The pure helpers (`eventMatchesFilter`, `bufferToCsv`) are covered by
 * `audit-log-client.test.ts`. This file covers the rendered UI:
 *   - connection badge transitions (connecting → open → error)
 *   - SSE message ingestion + rolling buffer
 *   - pause/resume button (paused state suppresses ingestion)
 *   - clear button (empties the buffer)
 *   - filter input (narrows the rendered list)
 *   - export button (disabled when empty, triggers download when filled)
 *
 * jsdom doesn't ship `EventSource`, so we install a manual stub that
 * captures the latest `onopen`/`onmessage`/`onerror` handlers so the
 * test can fire them synchronously.
 */

import React from 'react';
import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  within,
} from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { AuditLogClient } from '@/components/operations/audit-log-client';

// ---------------------------------------------------------------------------
// EventSource stub
// ---------------------------------------------------------------------------

interface FakeEventSource {
  onopen: ((this: EventSource, ev: Event) => unknown) | null;
  onmessage: ((this: EventSource, ev: MessageEvent) => unknown) | null;
  onerror: ((this: EventSource, ev: Event) => unknown) | null;
  close: () => void;
  url: string;
}

let lastEventSource: FakeEventSource | null = null;

class EventSourceStub implements FakeEventSource {
  onopen: FakeEventSource['onopen'] = null;
  onmessage: FakeEventSource['onmessage'] = null;
  onerror: FakeEventSource['onerror'] = null;
  url: string;
  close = vi.fn();

  constructor(url: string, _init?: EventSourceInit) {
    this.url = url;
    lastEventSource = this;
  }
}

beforeEach(() => {
  lastEventSource = null;
  vi.stubGlobal('EventSource', EventSourceStub);
  // Prevent jsdom's anchor-click navigation and Blob URL allocation from
  // exploding under the export-CSV path.
  vi.stubGlobal('URL', { ...URL, createObjectURL: vi.fn(() => 'blob:fake'), revokeObjectURL: vi.fn() });
});

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

function fireMessage(payload: Record<string, unknown>, lastEventId = '') {
  expect(lastEventSource).not.toBeNull();
  act(() => {
    lastEventSource!.onmessage?.call(lastEventSource as unknown as EventSource, {
      data: JSON.stringify(payload),
      lastEventId,
    } as MessageEvent);
  });
}

function fireOpen() {
  act(() => {
    lastEventSource!.onopen?.call(lastEventSource as unknown as EventSource, {} as Event);
  });
}

function fireError() {
  act(() => {
    lastEventSource!.onerror?.call(lastEventSource as unknown as EventSource, {} as Event);
  });
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('<AuditLogClient />', () => {
  it('renders the connecting state on initial mount', () => {
    render(<AuditLogClient />);

    expect(screen.getByText('Connecting…')).toBeInTheDocument();
    expect(screen.getByText(/0 of 0 events shown/)).toBeInTheDocument();
    // Empty-state copy reflects the connecting status.
    expect(screen.getByText(/Connecting to the event stream/)).toBeInTheDocument();
  });

  it('flips to "Connected" when the SSE stream opens', () => {
    render(<AuditLogClient />);

    fireOpen();

    expect(screen.getByText('Connected')).toBeInTheDocument();
    // The empty-state message switches to "Waiting for events…".
    expect(screen.getByText('Waiting for events…')).toBeInTheDocument();
  });

  it('shows the error state when EventSource fires onerror', () => {
    render(<AuditLogClient />);

    fireError();

    expect(screen.getByText('Connection error')).toBeInTheDocument();
    expect(screen.getByText(/Cannot connect to the event stream/)).toBeInTheDocument();
  });

  it('appends incoming events and updates the count', () => {
    render(<AuditLogClient />);
    fireOpen();

    fireMessage({ type: 'order_created', orderId: 'ord_1' }, 'evt-1');
    fireMessage({ type: 'inventory_adjusted', sku: 'SKU-X' }, 'evt-2');

    expect(screen.getByText(/2 of 2 events shown/)).toBeInTheDocument();
    // Newest first — most recent badge appears before the older one.
    const list = screen.getByRole('list');
    const items = within(list).getAllByRole('listitem');
    expect(items[0].textContent).toMatch(/inventory_adjusted/);
    expect(items[1].textContent).toMatch(/order_created/);
  });

  it('toggles the pause button label and suppresses new events while paused', () => {
    render(<AuditLogClient />);
    fireOpen();
    fireMessage({ type: 'order_created' }, 'a');
    expect(screen.getByText(/1 of 1 events shown/)).toBeInTheDocument();

    // Click pause → button now says "Resume", new events are dropped.
    fireEvent.click(screen.getByLabelText('Pause stream'));
    expect(screen.getByText('Resume')).toBeInTheDocument();
    fireMessage({ type: 'inventory_adjusted' }, 'b');
    expect(screen.getByText(/1 of 1 events shown/)).toBeInTheDocument();
    // Empty-state copy isn't shown — there are events; instead the paused
    // banner appears at the bottom-empty area only when the list is empty.
    // So just confirm we still only see the original event.
    const list = screen.getByRole('list');
    expect(within(list).getAllByRole('listitem').length).toBe(1);

    // Resume → new events are accepted again.
    fireEvent.click(screen.getByLabelText('Resume stream'));
    expect(screen.getByText('Pause')).toBeInTheDocument();
    fireMessage({ type: 'cart.updated' }, 'c');
    expect(screen.getByText(/2 of 2 events shown/)).toBeInTheDocument();
  });

  it('shows the paused empty-state banner when paused with no events', () => {
    render(<AuditLogClient />);
    fireOpen();

    fireEvent.click(screen.getByLabelText('Pause stream'));

    expect(
      screen.getByText('Stream paused. Resume to receive new events.'),
    ).toBeInTheDocument();
  });

  it('clears all events when Clear is clicked', () => {
    render(<AuditLogClient />);
    fireOpen();
    fireMessage({ type: 'order_created' }, '1');
    fireMessage({ type: 'order_shipped' }, '2');
    expect(screen.getByText(/2 of 2 events shown/)).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Clear' }));

    expect(screen.getByText(/0 of 0 events shown/)).toBeInTheDocument();
  });

  it('filters the rendered list by typed event type pattern', () => {
    render(<AuditLogClient />);
    fireOpen();
    fireMessage({ type: 'order_created' }, '1');
    fireMessage({ type: 'inventory_adjusted' }, '2');
    fireMessage({ type: 'order_shipped' }, '3');
    expect(screen.getByText(/3 of 3 events shown/)).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText('Event type filter'), {
      target: { value: 'order.*' },
    });

    expect(screen.getByText(/2 of 3 events shown/)).toBeInTheDocument();
  });

  it('disables Export CSV when the buffer is empty and enables it once events arrive', () => {
    render(<AuditLogClient />);
    fireOpen();

    const exportBtn = screen.getByRole('button', { name: 'Export CSV' });
    expect(exportBtn).toBeDisabled();

    fireMessage({ type: 'order_created' }, '1');

    expect(screen.getByRole('button', { name: 'Export CSV' })).not.toBeDisabled();
  });

  it('closes the EventSource on unmount', () => {
    const { unmount } = render(<AuditLogClient />);
    expect(lastEventSource).not.toBeNull();
    const closeFn = lastEventSource!.close;

    unmount();

    expect(closeFn).toHaveBeenCalled();
  });
});
