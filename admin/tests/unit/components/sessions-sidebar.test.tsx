import React from 'react';
import { render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { SessionsSidebar } from '@/components/sessions-sidebar';

const mockFetch = vi.fn();

beforeEach(() => {
  vi.stubGlobal('fetch', mockFetch);
});

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

describe('SessionsSidebar', () => {
  it('reads sessions and summary from the standard API envelope', async () => {
    mockFetch.mockImplementation(async (input: string) => {
      if (input.startsWith('/api/sessions?')) {
        return {
          ok: true,
          json: async () => ({
            success: true,
            data: [
              {
                id: 'sess_123456789',
                organization_id: 'org_1',
                org_name: 'StateSet',
                org_slug: 'stateset',
                status: 'running',
                current_sandbox_id: 'sandbox_1',
                name: 'Checkout Recovery',
                description: null,
                budget_config: {},
                budget_consumed: {
                  cost_cents: 250,
                  iterations: 4,
                  duration_seconds: 120,
                },
                rotation_count: 0,
                total_exec_count: 4,
                created_at: '2026-03-05T00:00:00.000Z',
                started_at: '2026-03-05T00:01:00.000Z',
                completed_at: null,
                last_activity_at: new Date().toISOString(),
                error_message: null,
                error_code: null,
              },
            ],
            meta: {
              requestId: 'req_test',
              timestamp: '2026-03-05T00:00:00.000Z',
              pagination: {
                total: 1,
                limit: 50,
                offset: 0,
                hasMore: false,
              },
            },
          }),
        };
      }

      if (input === '/api/sessions/summary') {
        return {
          ok: true,
          json: async () => ({
            success: true,
            data: {
              total: 1,
              by_status: {
                pending: 0,
                running: 1,
                rotating: 0,
                paused: 0,
                completed: 0,
                failed: 0,
                cancelled: 0,
              },
              active_now: 1,
              rotations_last_hour: 0,
              avg_duration_seconds: 120,
            },
            meta: {
              requestId: 'req_test',
              timestamp: '2026-03-05T00:00:00.000Z',
            },
          }),
        };
      }

      throw new Error(`Unexpected fetch: ${input}`);
    });

    render(React.createElement(SessionsSidebar));

    await waitFor(() => {
      expect(screen.getByText('Checkout Recovery')).toBeInTheDocument();
    });

    expect(screen.getByText('1 total')).toBeInTheDocument();
    expect(screen.getByText('1 active')).toBeInTheDocument();
    expect(screen.getByText('Avg duration: 2m 0s')).toBeInTheDocument();
  });
});
