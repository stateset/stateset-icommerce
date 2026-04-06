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
import { cn } from '@/lib/utils';
import { formatRelativeTime, truncate } from '@/lib/utils';
import type { AgentSession, AgentSessionStatus, AgentSessionSummary } from '@/lib/types';

function getStatusIcon(status: AgentSessionStatus) {
  switch (status) {
    case 'running':
      return <div className="w-2 h-2 bg-emerald-500 rounded-full animate-pulse" />;
    case 'pending':
      return <ClockIcon className="w-3.5 h-3.5 text-amber-500" />;
    case 'rotating':
      return <ArrowPathIcon className="w-3.5 h-3.5 text-blue-500 animate-spin" />;
    case 'paused':
      return <PauseIcon className="w-3.5 h-3.5 text-amber-500" />;
    case 'completed':
      return <CheckCircleIcon className="w-3.5 h-3.5 text-gray-400" />;
    case 'failed':
      return <ExclamationCircleIcon className="w-3.5 h-3.5 text-red-500" />;
    case 'cancelled':
      return <XCircleIcon className="w-3.5 h-3.5 text-gray-400" />;
    default:
      return <div className="w-2 h-2 bg-gray-400 rounded-full" />;
  }
}

function getStatusColor(status: AgentSessionStatus): string {
  const colors: Record<AgentSessionStatus, string> = {
    pending: 'bg-amber-100 text-amber-700 dark:bg-amber-900/30 dark:text-amber-400',
    running: 'bg-emerald-100 text-emerald-700 dark:bg-emerald-900/30 dark:text-emerald-400',
    rotating: 'bg-blue-100 text-blue-700 dark:bg-blue-900/30 dark:text-blue-400',
    paused: 'bg-amber-100 text-amber-700 dark:bg-amber-900/30 dark:text-amber-400',
    completed: 'bg-gray-100 text-gray-700 dark:bg-gray-800 dark:text-gray-400',
    failed: 'bg-red-100 text-red-700 dark:bg-red-900/30 dark:text-red-400',
    cancelled: 'bg-gray-100 text-gray-700 dark:bg-gray-800 dark:text-gray-400',
  };
  return colors[status] || 'bg-gray-100 text-gray-700 dark:bg-gray-800 dark:text-gray-400';
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
        'w-full text-left p-3 rounded-lg transition-all',
        isSelected
          ? 'bg-indigo-50 dark:bg-indigo-900/20 border border-indigo-200 dark:border-indigo-800'
          : 'hover:bg-gray-50 dark:hover:bg-gray-800/50 border border-transparent'
      )}
    >
      <div className="flex items-start justify-between gap-2">
        <div className="flex items-center gap-2 min-w-0">
          {getStatusIcon(session.status)}
          <span className="text-xs font-mono text-gray-600 dark:text-gray-400 truncate">
            {truncate(session.id, 12)}
          </span>
        </div>
        <ChevronRightIcon className="w-3.5 h-3.5 text-gray-400 flex-shrink-0" />
      </div>

      {/* Session name or org */}
      {(session.name || session.org_name) && (
        <div className="mt-1.5 text-xs font-medium text-gray-900 dark:text-white truncate">
          {session.name || session.org_name}
        </div>
      )}

      <div className="mt-2 flex items-center gap-2 flex-wrap">
        <span className={cn(
          'text-[10px] font-medium px-1.5 py-0.5 rounded',
          getStatusColor(session.status)
        )}>
          {session.status}
        </span>
        <span className="text-[10px] text-gray-500 dark:text-gray-500">
          {session.total_exec_count} ops
        </span>
        {session.budget_consumed.cost_cents > 0 && (
          <span className="text-[10px] text-gray-500 dark:text-gray-500">
            {formatCost(session.budget_consumed.cost_cents)}
          </span>
        )}
      </div>

      {/* Duration or error */}
      {session.error_message ? (
        <div className="mt-1.5 text-[10px] text-red-500 truncate">
          {truncate(session.error_message, 40)}
        </div>
      ) : session.budget_consumed.duration_seconds > 0 && (
        <div className="mt-1.5 text-[10px] text-gray-500 dark:text-gray-500">
          Duration: {formatDuration(session.budget_consumed.duration_seconds)}
        </div>
      )}

      <div className="mt-1.5 flex items-center gap-1 text-[10px] text-gray-400 dark:text-gray-600">
        <ClockIcon className="w-3 h-3" />
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
        const payload = await response.json().catch(() => ({ error: { message: 'Unknown error' } }));
        throw new Error(payload.error?.message || `Failed to fetch sessions: ${response.status}`);
      }

      const payload = await response.json() as ApiEnvelope<AgentSession[]>;
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
        const payload = await response.json() as ApiEnvelope<AgentSessionSummary>;
        setSummary(payload.data ?? null);
      } else {
        const payload = await response.json().catch(() => ({ error: { message: 'Unknown error' } }));
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

  const activeSessions = sessions.filter(s =>
    s.status === 'running' || s.status === 'pending' || s.status === 'rotating' || s.status === 'paused'
  );
  const completedSessions = sessions.filter(s =>
    s.status === 'completed' || s.status === 'failed' || s.status === 'cancelled'
  );

  return (
    <div className={cn(
      'flex flex-col w-64 border-r border-gray-200 dark:border-gray-800 bg-gray-50 dark:bg-gray-900/50',
      className
    )}>
      {/* Header */}
      <div className="p-4 border-b border-gray-200 dark:border-gray-800">
        <div className="flex items-center justify-between mb-3">
          <div className="flex items-center gap-2">
            <ChatBubbleLeftIcon className="w-4 h-4 text-gray-500" />
            <h2 className="text-sm font-semibold text-gray-900 dark:text-white">Sessions</h2>
          </div>
          <div className="flex items-center gap-1">
            <button
              onClick={() => { fetchSessions(); fetchSummary(); }}
              className="p-1.5 rounded-md hover:bg-gray-200 dark:hover:bg-gray-700 transition-colors"
              title="Refresh"
            >
              <ArrowPathIcon className={cn("w-4 h-4 text-gray-500", isLoading && "animate-spin")} />
            </button>
            <button
              className="p-1.5 rounded-md hover:bg-gray-200 dark:hover:bg-gray-700 transition-colors"
              title="New Session"
            >
              <PlusIcon className="w-4 h-4 text-gray-500" />
            </button>
          </div>
        </div>

        {/* Search */}
        <div className="relative mb-2">
          <MagnifyingGlassIcon className="absolute left-2.5 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-gray-400" />
          <input
            type="text"
            placeholder="Search sessions..."
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className="w-full pl-8 pr-3 py-1.5 text-xs rounded-md border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 text-gray-900 dark:text-white placeholder-gray-400 focus:outline-none focus:ring-1 focus:ring-indigo-500 focus:border-indigo-500"
          />
        </div>

        {/* Status Filter */}
        <select
          value={statusFilter}
          onChange={(e) => setStatusFilter(e.target.value as AgentSessionStatus | '')}
          className="w-full px-2 py-1.5 text-xs rounded-md border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 text-gray-900 dark:text-white focus:outline-none focus:ring-1 focus:ring-indigo-500 focus:border-indigo-500"
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
          <div className="p-4 space-y-3">
            {[...Array(3)].map((_, i) => (
              <div key={i} className="animate-pulse">
                <div className="h-24 bg-gray-200 dark:bg-gray-800 rounded-lg" />
              </div>
            ))}
          </div>
        ) : error ? (
          <div className="p-4 text-center">
            <ExclamationCircleIcon className="w-8 h-8 mx-auto text-red-400" />
            <p className="mt-2 text-xs text-red-500">{error}</p>
            <button
              onClick={() => { fetchSessions(); fetchSummary(); }}
              className="mt-2 text-xs text-indigo-500 hover:text-indigo-600"
            >
              Try again
            </button>
          </div>
        ) : (
          <div className="p-2 space-y-4">
            {/* Active Sessions */}
            {activeSessions.length > 0 && (
              <div>
                <div className="px-2 py-1.5 text-[10px] font-semibold uppercase tracking-wider text-gray-500 dark:text-gray-500">
                  Active ({activeSessions.length})
                </div>
                <div className="space-y-1">
                  {activeSessions.map(session => (
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
                <div className="px-2 py-1.5 text-[10px] font-semibold uppercase tracking-wider text-gray-500 dark:text-gray-500">
                  Recent ({completedSessions.length})
                </div>
                <div className="space-y-1">
                  {completedSessions.map(session => (
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
              <div className="text-center py-8 px-4">
                <ChatBubbleLeftIcon className="w-8 h-8 mx-auto text-gray-300 dark:text-gray-600" />
                <p className="mt-2 text-xs text-gray-500 dark:text-gray-500">
                  {searchQuery || statusFilter ? 'No sessions match your filters' : 'No sessions yet'}
                </p>
              </div>
            )}
          </div>
        )}
      </div>

      {/* Footer Stats */}
      <div className="p-3 border-t border-gray-200 dark:border-gray-800 bg-white dark:bg-gray-900">
        <div className="flex items-center justify-between text-[10px] text-gray-500 dark:text-gray-500">
          <span>{summary?.total || sessions.length} total</span>
          <span>{summary?.active_now || activeSessions.length} active</span>
        </div>
        {summary && summary.avg_duration_seconds > 0 && (
          <div className="mt-1 text-[10px] text-gray-400 dark:text-gray-600">
            Avg duration: {formatDuration(Math.round(summary.avg_duration_seconds))}
          </div>
        )}
      </div>
    </div>
  );
}
