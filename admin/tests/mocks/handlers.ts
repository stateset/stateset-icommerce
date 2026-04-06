/**
 * MSW Request Handlers
 *
 * Mock handlers for external services used in tests.
 */

import { http, HttpResponse } from 'msw';

const API_URL = 'https://api.sandbox.stateset.app';

// ============================================================================
// Sandbox API Handlers
// ============================================================================

export const sandboxHandlers = [
  // List sessions
  http.get(`${API_URL}/api/admin/agent-sessions`, ({ request }) => {
    const url = new URL(request.url);
    const limit = Number(url.searchParams.get('limit') || '20');
    const offset = Number(url.searchParams.get('offset') || '0');

    return HttpResponse.json({
      total: 2,
      sessions: [
        {
          id: 'session-1',
          organization_id: 'org-1',
          org_name: 'Test Org',
          org_slug: 'test-org',
          status: 'running',
          current_sandbox_id: 'sbx-1',
          name: 'Test Session',
          description: 'A test session',
          budget_config: { cost_cap_cents: 1000, iteration_limit: 50 },
          budget_consumed: { cost_cents: 100, iterations: 5, duration_seconds: 120 },
          rotation_count: 0,
          total_exec_count: 5,
          created_at: '2026-01-28T00:00:00Z',
          started_at: '2026-01-28T00:01:00Z',
          completed_at: null,
          last_activity_at: '2026-01-28T00:02:00Z',
          error_message: null,
          error_code: null,
        },
        {
          id: 'session-2',
          organization_id: 'org-1',
          org_name: 'Test Org',
          org_slug: 'test-org',
          status: 'completed',
          current_sandbox_id: null,
          name: 'Completed Session',
          description: null,
          budget_config: {},
          budget_consumed: { cost_cents: 50, iterations: 3, duration_seconds: 60 },
          rotation_count: 1,
          total_exec_count: 3,
          created_at: '2026-01-27T00:00:00Z',
          started_at: '2026-01-27T00:01:00Z',
          completed_at: '2026-01-27T00:02:00Z',
          last_activity_at: '2026-01-27T00:02:00Z',
          error_message: null,
          error_code: null,
        },
      ].slice(offset, offset + limit),
    });
  }),

  // Get session detail
  http.get(`${API_URL}/api/admin/agent-sessions/:id`, ({ params }) => {
    const { id } = params;
    return HttpResponse.json({
      session: {
        id,
        organization_id: 'org-1',
        org_name: 'Test Org',
        org_slug: 'test-org',
        status: 'running',
        current_sandbox_id: 'sbx-1',
        name: 'Test Session',
        description: 'A test session',
        budget_config: { cost_cap_cents: 1000 },
        budget_consumed: { cost_cents: 100, iterations: 5, duration_seconds: 120 },
        rotation_count: 0,
        total_exec_count: 5,
        created_at: '2026-01-28T00:00:00Z',
        started_at: '2026-01-28T00:01:00Z',
        completed_at: null,
        last_activity_at: '2026-01-28T00:02:00Z',
        error_message: null,
        error_code: null,
      },
      events: [
        {
          id: 'evt-1',
          session_id: id,
          sandbox_id: 'sbx-1',
          event_type: 'execution',
          event_subtype: 'tool_call',
          payload: { tool: 'search' },
          sequence_number: 1,
          duration_ms: 150,
          success: true,
          error_message: null,
          created_at: '2026-01-28T00:01:30Z',
        },
      ],
    });
  }),

  // Get session summary
  http.get(`${API_URL}/api/admin/agent-sessions/summary`, () => {
    return HttpResponse.json({
      total: 10,
      by_status: {
        pending: 1,
        running: 2,
        rotating: 0,
        paused: 1,
        completed: 5,
        failed: 1,
        cancelled: 0,
      },
      active_now: 3,
      rotations_last_hour: 2,
      avg_duration_seconds: 180,
    });
  }),

  // Cancel session
  http.post(`${API_URL}/api/admin/agent-sessions/:id/cancel`, ({ params }) => {
    const { id } = params;
    return HttpResponse.json({
      session_id: id,
      status: 'cancelled',
    });
  }),

  // Health check
  http.get(`${API_URL}/health`, () => {
    return HttpResponse.json({ status: 'healthy' });
  }),
];

// ============================================================================
// Redis (Upstash) Handlers
// ============================================================================

export const redisHandlers = [
  http.post('https://redis.upstash.io', () => {
    return HttpResponse.json({ result: 'PONG' });
  }),
];

// ============================================================================
// All handlers combined
// ============================================================================

export const handlers = [...sandboxHandlers, ...redisHandlers];
