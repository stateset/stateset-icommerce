'use client';

import { useState, useEffect, useCallback } from 'react';
import {
  ClockIcon,
  ChatBubbleLeftIcon,
  CheckCircleIcon,
  ExclamationCircleIcon,
  ChevronRightIcon,
  PlusIcon,
  MagnifyingGlassIcon,
  ArrowPathIcon,
  PauseIcon,
  XCircleIcon,
} from '@heroicons/react/24/outline';
import { StatusPill, type StatusTone } from '@stateset/design';
import { cn } from '@/lib/utils';
import { formatRelativeTime, truncate } from '@/lib/utils';
import type { AgentSession, AgentSessionStatus, AgentSessionSummary } from '@/lib/types';

// Map domain session states onto the design system's operational status vocabulary
// (ok / run / warn / fail / review / idle) so sessions read the same as every
// other status surface in the product.
const STATUS_TONE: Record<AgentSessionStatus, StatusTone> = {
  pending: 'review',
  running: 'run',
  rotating: 'run',
  paused: 'warn',
  completed: 'ok',
  failed: 'fail',
  cancelled: 'idle',
};

function statusTone(status: AgentSessionStatus): StatusTone {
  return STATUS_TONE[status] ?? 'idle';
}

function getStatusIcon(status: AgentSessionStatus) {
  switch (status) {
    case 'running':
      return <div className="h-2 w-2 animate-ds-soft-pulse rounded-full bg-ds-status-run" />;
    case 'pending':
      return <ClockIcon className="h-3.5 w-3.5 text-ds-status-review" />;
    case 'rotating':
      return <ArrowPathIcon className="h-3.5 w-3.5 animate-spin text-ds-status-run" />;
    case 'paused':
      return <PauseIcon className="h-3.5 w-3.5 text-ds-status-warn" />;
    case 'completed':
      return <CheckCircleIcon className="h-3.5 w-3.5 text-ds-status-ok" />;
    case 'failed':
      return <ExclamationCircleIcon className="h-3.5 w-3.5 text-ds-status-fail" />;
    case 'cancelled':
      return <XCircleIcon className="h-3.5 w-3.5 text-ds-muted-foreground" />;
    default:
      return <div className="h-2 w-2 rounded-full bg-ds-status-idle" />;
  }
}

function formatDuration(seconds: number): string {
  if (seconds < 60) return `${seconds}s`;
  if (seconds < 3600) return `${Math.floor(seconds / 60)}m ${seconds % 60}s`;
  const hours = Math.floor(seconds / 3600);
  const mins = Math.floor((seconds % 3600) / 60);
  return `${hours}h ${mins}m`;
}

function formatCost(cents: number): string {
  return `$${(cents / 100).toFixed(2)}`;
}

interface SessionItemProps {
  session: AgentSession;
  isSelected: boolean;
  onClick: () => void;
}

function SessionItem({ session, isSelected, onClick }: SessionItemProps) {
  return (
    <button
      onClick={onClick}
      className={cn(
        'ds-focus-ring w-full rounded-lg border p-3 text-left transition-all',
        isSelected
          ? 'border-ds-brand-200 bg-ds-brand-50 dark:border-ds-brand-700 dark:bg-ds-brand-950/30'
          : 'border-transparent hover:bg-ds-muted',
      )}
    >
      <div className="flex items-start justify-between gap-2">
        <div className="flex min-w-0 items-center gap-2">
          {getStatusIcon(session.status)}
          <span className="truncate font-mono text-xs text-ds-muted-foreground">
            {truncate(session.id, 12)}
          </span>
        </div>
        <ChevronRightIcon className="h-3.5 w-3.5 flex-shrink-0 text-ds-muted-foreground" />
      </div>

      {/* Session name or org */}
      {(session.name || session.org_name) && (
        <div className="mt-1.5 truncate text-xs font-medium text-ds-foreground">
          {session.name || session.org_name}
        </div>
      )}

      <div className="mt-2 flex flex-wrap items-center gap-2">
        <StatusPill status={statusTone(session.status)}>{session.status}</StatusPill>
        <span className="text-[10px] text-ds-muted-foreground">{session.total_exec_count} ops</span>
        {session.budget_consumed.cost_cents > 0 && (
          <span className="text-[10px] text-ds-muted-foreground">
            {formatCost(session.budget_consumed.cost_cents)}
          </span>
        )}
      </div>

      {/* Duration or error */}
      {session.error_message ? (
        <div className="mt-1.5 truncate text-[10px] text-ds-destructive">
          {truncate(session.error_message, 40)}
        </div>
      ) : (
        session.budget_consumed.duration_seconds > 0 && (
          <div className="mt-1.5 text-[10px] text-ds-muted-foreground">
            Duration: {formatDuration(session.budget_consumed.duration_seconds)}
          </div>
        )
      )}

      <div className="mt-1.5 flex items-center gap-1 text-[10px] text-ds-muted-foreground">
        <ClockIcon className="h-3 w-3" />
        <span>{formatRelativeTime(session.last_activity_at)}</span>
      </div>
    </button>
  );
}

interface SessionsSidebarProps {
  className?: string;
}

interface ApiEnvelope<T> {
  success: boolean;
  data?: T;
  error?: {
    message?: string;
  };
}

export function SessionsSidebar({ className }: SessionsSidebarProps) {
  const [sessions, setSessions] = useState<AgentSession[]>([]);
  const [summary, setSummary] = useState<AgentSessionSummary | null>(null);
  const [selectedSession, setSelectedSession] = useState<string | null>(null);
  const [searchQuery, setSearchQuery] = useState('');
  const [statusFilter, setStatusFilter] = useState<AgentSessionStatus | ''>('');
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchSessions = useCallback(async () => {
    try {
      setError(null);
      const params = new URLSearchParams();
      params.set('limit', '50');
      if (searchQuery) params.set('search', searchQuery);
      if (statusFilter) params.set('status', statusFilter);

      const response = await fetch(`/api/sessions?${params.toString()}`);

      if (!response.ok) {
        const payload = await response
          .json()
          .catch(() => ({ error: { message: 'Unknown error' } }));
        throw new Error(payload.error?.message || `Failed to fetch sessions: ${response.status}`);
      }

      const payload = (await response.json()) as ApiEnvelope<AgentSession[]>;
      setSessions(Array.isArray(payload.data) ? payload.data : []);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load sessions');
      console.error('Error fetching sessions:', err);
    }
  }, [searchQuery, statusFilter]);

  const fetchSummary = useCallback(async () => {
    try {
      const response = await fetch('/api/sessions/summary');
      if (response.ok) {
        const payload = (await response.json()) as ApiEnvelope<AgentSessionSummary>;
        setSummary(payload.data ?? null);
      } else {
        const payload = await response
          .json()
          .catch(() => ({ error: { message: 'Unknown error' } }));
        console.error('Error fetching summary:', payload.error?.message || response.statusText);
      }
    } catch (err) {
      console.error('Error fetching summary:', err);
    }
  }, []);

  useEffect(() => {
    const loadData = async () => {
      setIsLoading(true);
      await Promise.all([fetchSessions(), fetchSummary()]);
      setIsLoading(false);
    };
    loadData();

    // Poll for updates every 30 seconds
    const interval = setInterval(() => {
      fetchSessions();
      fetchSummary();
    }, 30000);

    return () => clearInterval(interval);
  }, [fetchSessions, fetchSummary]);

  // Re-fetch when search or filter changes
  useEffect(() => {
    const debounce = setTimeout(() => {
      fetchSessions();
    }, 300);
    return () => clearTimeout(debounce);
  }, [searchQuery, statusFilter, fetchSessions]);

  const activeSessions = sessions.filter(
    (s) =>
      s.status === 'running' ||
      s.status === 'pending' ||
      s.status === 'rotating' ||
      s.status === 'paused',
  );
  const completedSessions = sessions.filter(
    (s) => s.status === 'completed' || s.status === 'failed' || s.status === 'cancelled',
  );

  return (
    <div
      className={cn(
        'flex w-64 flex-col border-r border-ds-enterprise-line bg-ds-enterprise-surface',
        className,
      )}
    >
      {/* Header */}
      <div className="border-b border-ds-enterprise-line p-4">
        <div className="mb-3 flex items-center justify-between">
          <div className="flex items-center gap-2">
            <ChatBubbleLeftIcon className="h-4 w-4 text-ds-muted-foreground" />
            <h2 className="text-sm font-semibold text-ds-foreground">Sessions</h2>
          </div>
          <div className="flex items-center gap-1">
            <button
              onClick={() => {
                fetchSessions();
                fetchSummary();
              }}
              className="ds-focus-ring rounded-md p-1.5 transition-colors hover:bg-ds-muted"
              title="Refresh"
            >
              <ArrowPathIcon
                className={cn('h-4 w-4 text-ds-muted-foreground', isLoading && 'animate-spin')}
              />
            </button>
            <button
              className="ds-focus-ring rounded-md p-1.5 transition-colors hover:bg-ds-muted"
              title="New Session"
            >
              <PlusIcon className="h-4 w-4 text-ds-muted-foreground" />
            </button>
          </div>
        </div>

        {/* Search */}
        <div className="relative mb-2">
          <MagnifyingGlassIcon className="absolute left-2.5 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-ds-muted-foreground" />
          <input
            type="text"
            placeholder="Search sessions..."
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className="ds-focus-ring w-full rounded-md border border-ds-input bg-ds-background py-1.5 pl-8 pr-3 text-xs text-ds-foreground placeholder:text-ds-muted-foreground"
          />
        </div>

        {/* Status Filter */}
        <select
          value={statusFilter}
          onChange={(e) => setStatusFilter(e.target.value as AgentSessionStatus | '')}
          className="ds-focus-ring w-full rounded-md border border-ds-input bg-ds-background px-2 py-1.5 text-xs text-ds-foreground"
        >
          <option value="">All statuses</option>
          <option value="running">Running</option>
          <option value="pending">Pending</option>
          <option value="paused">Paused</option>
          <option value="completed">Completed</option>
          <option value="failed">Failed</option>
          <option value="cancelled">Cancelled</option>
        </select>
      </div>

      {/* Sessions List */}
      <div className="flex-1 overflow-y-auto" aria-busy={isLoading} aria-live="polite">
        {isLoading && sessions.length === 0 ? (
          <div className="space-y-3 p-4">
            {[...Array(3)].map((_, i) => (
              <div key={i} className="animate-pulse">
                <div className="h-24 rounded-lg bg-ds-muted" />
              </div>
            ))}
          </div>
        ) : error ? (
          <div className="p-4 text-center">
            <ExclamationCircleIcon className="mx-auto h-8 w-8 text-ds-destructive/70" />
            <p className="mt-2 text-xs text-ds-destructive">{error}</p>
            <button
              onClick={() => {
                fetchSessions();
                fetchSummary();
              }}
              className="mt-2 text-xs font-medium text-ds-primary hover:underline"
            >
              Try again
            </button>
          </div>
        ) : (
          <div className="space-y-4 p-2">
            {/* Active Sessions */}
            {activeSessions.length > 0 && (
              <div>
                <div className="px-2 py-1.5 text-[10px] font-semibold uppercase tracking-ds-kicker text-ds-muted-foreground">
                  Active ({activeSessions.length})
                </div>
                <div className="space-y-1">
                  {activeSessions.map((session) => (
                    <SessionItem
                      key={session.id}
                      session={session}
                      isSelected={selectedSession === session.id}
                      onClick={() => setSelectedSession(session.id)}
                    />
                  ))}
                </div>
              </div>
            )}

            {/* Completed Sessions */}
            {completedSessions.length > 0 && (
              <div>
                <div className="px-2 py-1.5 text-[10px] font-semibold uppercase tracking-ds-kicker text-ds-muted-foreground">
                  Recent ({completedSessions.length})
                </div>
                <div className="space-y-1">
                  {completedSessions.map((session) => (
                    <SessionItem
                      key={session.id}
                      session={session}
                      isSelected={selectedSession === session.id}
                      onClick={() => setSelectedSession(session.id)}
                    />
                  ))}
                </div>
              </div>
            )}

            {/* Empty State */}
            {sessions.length === 0 && !isLoading && (
              <div className="px-4 py-8 text-center">
                <ChatBubbleLeftIcon className="mx-auto h-8 w-8 text-ds-muted-foreground/50" />
                <p className="mt-2 text-xs text-ds-muted-foreground">
                  {searchQuery || statusFilter
                    ? 'No sessions match your filters'
                    : 'No sessions yet'}
                </p>
              </div>
            )}
          </div>
        )}
      </div>

      {/* Footer Stats */}
      <div className="border-t border-ds-enterprise-line bg-ds-enterprise-raised p-3">
        <div className="flex items-center justify-between text-[10px] text-ds-muted-foreground">
          <span>{summary?.total || sessions.length} total</span>
          <span>{summary?.active_now || activeSessions.length} active</span>
        </div>
        {summary && summary.avg_duration_seconds > 0 && (
          <div className="mt-1 text-[10px] text-ds-muted-foreground">
            Avg duration: {formatDuration(Math.round(summary.avg_duration_seconds))}
          </div>
        )}
      </div>
    </div>
  );
}
