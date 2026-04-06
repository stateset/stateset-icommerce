/**
 * StateSet Sessions API Client
 *
 * Connects to the StateSet Sandbox API to fetch agent session data.
 * API endpoint: https://api.sandbox.stateset.app
 */

import { getPublicStateSetApiUrl } from '@/lib/stateset-api-url';

const API_URL = getPublicStateSetApiUrl();
const TOKEN_KEY = 'stateset_admin_token';

// ============================================
// Types matching the Sandbox API
// ============================================

export type AgentSessionStatus =
  | 'pending'
  | 'running'
  | 'rotating'
  | 'paused'
  | 'completed'
  | 'failed'
  | 'cancelled';

export interface AgentSessionRow {
  id: string;
  organization_id: string;
  org_name: string | null;
  org_slug: string | null;
  status: AgentSessionStatus;
  current_sandbox_id: string | null;
  name: string | null;
  description: string | null;
  budget_config: {
    cost_cap_cents?: number;
    iteration_limit?: number;
    duration_limit_seconds?: number;
  };
  budget_consumed: {
    cost_cents: number;
    iterations: number;
    duration_seconds: number;
  };
  rotation_count: number;
  total_exec_count: number;
  created_at: string;
  started_at: string | null;
  completed_at: string | null;
  last_activity_at: string;
  error_message: string | null;
  error_code: string | null;
}

export interface AgentSessionsResponse {
  total: number;
  sessions: AgentSessionRow[];
}

export interface AgentEventRow {
  id: string;
  session_id: string;
  sandbox_id: string | null;
  event_type: string;
  event_subtype: string | null;
  payload: Record<string, unknown>;
  sequence_number: number;
  duration_ms: number | null;
  success: boolean | null;
  error_message: string | null;
  created_at: string;
}

export interface AgentSessionDetailResponse {
  session: AgentSessionRow;
  events: AgentEventRow[];
}

export interface AgentSessionSummary {
  total: number;
  by_status: Record<AgentSessionStatus, number>;
  active_now: number;
  rotations_last_hour: number;
  avg_duration_seconds: number;
}

export interface ListSessionsParams {
  limit?: number;
  offset?: number;
  status?: AgentSessionStatus;
  org_id?: string;
  search?: string;
}

// ============================================
// API Client
// ============================================

class SessionsApi {
  private token: string | null = null;

  setToken(token: string) {
    this.token = token.trim();
    if (typeof window !== 'undefined') {
      localStorage.setItem(TOKEN_KEY, this.token);
    }
  }

  getToken(): string | null {
    if (this.token) return this.token;
    if (typeof window !== 'undefined') {
      return localStorage.getItem(TOKEN_KEY);
    }
    return process.env.STATESET_API_TOKEN || null;
  }

  clearToken() {
    this.token = null;
    if (typeof window !== 'undefined') {
      localStorage.removeItem(TOKEN_KEY);
    }
  }

  private buildQuery(params: Record<string, string | number | undefined>): string {
    const searchParams = new URLSearchParams();
    Object.entries(params).forEach(([key, value]) => {
      if (value === undefined || value === '') return;
      searchParams.set(key, String(value));
    });
    const query = searchParams.toString();
    return query ? `?${query}` : '';
  }

  private async fetch<T>(endpoint: string, options: RequestInit = {}): Promise<T> {
    const token = this.getToken();
    if (!token) {
      throw new Error('Not authenticated. Please set an API token.');
    }

    const headers: Record<string, string> = {
      'Content-Type': 'application/json',
      ...(options.headers as Record<string, string>),
      Authorization: `Bearer ${token}`,
    };

    const response = await fetch(`${API_URL}${endpoint}`, {
      ...options,
      headers,
    });

    if (!response.ok) {
      if (response.status === 401 || response.status === 403) {
        this.clearToken();
        throw new Error('Session expired. Please log in again.');
      }

      const error = await response.json().catch(() => ({ error: 'Unknown error' }));
      throw new Error(error.error?.message || error.error || `Request failed: ${response.status}`);
    }

    return response.json();
  }

  // ============================================
  // Session Methods
  // ============================================

  async listSessions(params: ListSessionsParams = {}): Promise<AgentSessionsResponse> {
    const query = this.buildQuery({
      limit: params.limit,
      offset: params.offset,
      status: params.status,
      org_id: params.org_id,
      search: params.search,
    });
    return this.fetch(`/api/admin/agent-sessions${query}`);
  }

  async getSession(id: string): Promise<AgentSessionDetailResponse> {
    return this.fetch(`/api/admin/agent-sessions/${id}`);
  }

  async getSessionSummary(): Promise<AgentSessionSummary> {
    return this.fetch('/api/admin/agent-sessions/summary');
  }

  async cancelSession(id: string): Promise<{ session_id: string; status: string }> {
    return this.fetch(`/api/admin/agent-sessions/${id}/cancel`, { method: 'POST' });
  }

  async getSessionEvents(
    sessionId: string,
    params?: { limit?: number; offset?: number; after_sequence?: number }
  ): Promise<{ events: AgentEventRow[]; total: number }> {
    const query = this.buildQuery({
      limit: params?.limit,
      offset: params?.offset,
      after_sequence: params?.after_sequence,
    });
    return this.fetch(`/api/admin/agent-sessions/${sessionId}/events${query}`);
  }

  // ============================================
  // Fetch all sessions with pagination
  // ============================================

  async fetchAllSessions(params: Omit<ListSessionsParams, 'limit' | 'offset'> = {}): Promise<AgentSessionsResponse> {
    const PAGE_SIZE = 100;
    let allSessions: AgentSessionRow[] = [];
    let offset = 0;
    let total = 0;

    do {
      const result = await this.listSessions({
        limit: PAGE_SIZE,
        offset,
        ...params,
      });
      allSessions = allSessions.concat(result.sessions);
      total = result.total;
      offset += PAGE_SIZE;
    } while (offset < total);

    return { sessions: allSessions, total };
  }
}

export const sessionsApi = new SessionsApi();
