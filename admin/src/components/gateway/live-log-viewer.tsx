'use client';

import { useState, useEffect, useRef, useCallback } from 'react';
import { Card, Title, Text, Badge, TextInput, Select, SelectItem } from '@tremor/react';
import { MagnifyingGlassIcon, PauseIcon, PlayIcon } from '@heroicons/react/24/outline';
import { useEmbeddedData } from '@/hooks/use-embedded-data';
import { getGatewayMetrics } from '@/lib/gateway-client';
import type { GatewayMetrics, ChannelStats } from '@/lib/types/gateway';

interface LogEntry {
  id: string;
  timestamp: string;
  channel: string;
  level: 'info' | 'warn' | 'error';
  message: string;
}

const LEVEL_COLORS: Record<string, 'emerald' | 'amber' | 'red'> = {
  info: 'emerald',
  warn: 'amber',
  error: 'red',
};

let entryId = 0;

export function LiveLogViewer() {
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const [search, setSearch] = useState('');
  const [channelFilter, setChannelFilter] = useState('all');
  const [paused, setPaused] = useState(false);
  const prevMetricsRef = useRef<Record<string, ChannelStats> | null>(null);
  const scrollRef = useRef<HTMLDivElement>(null);

  const generateLogs = useCallback((metrics: GatewayMetrics) => {
    if (paused) return;
    const prev = prevMetricsRef.current;
    prevMetricsRef.current = { ...metrics.channels };

    if (!prev) return;

    const newEntries: LogEntry[] = [];
    const now = new Date().toISOString();

    for (const [channel, stats] of Object.entries(metrics.channels)) {
      const prevStats = prev[channel];
      if (!prevStats) {
        newEntries.push({
          id: String(++entryId),
          timestamp: now,
          channel,
          level: 'info',
          message: `Channel appeared: ${stats.messagesReceived} messages`,
        });
        continue;
      }

      const newMsgs = stats.messagesReceived - prevStats.messagesReceived;
      const newErrors = stats.errors - prevStats.errors;
      const newBlocked = stats.blocked - prevStats.blocked;

      if (newMsgs > 0) {
        newEntries.push({
          id: String(++entryId),
          timestamp: now,
          channel,
          level: 'info',
          message: `${newMsgs} new message${newMsgs > 1 ? 's' : ''} received (avg ${Math.round(stats.avgResponseMs)}ms)`,
        });
      }

      if (newErrors > 0) {
        newEntries.push({
          id: String(++entryId),
          timestamp: now,
          channel,
          level: 'error',
          message: `${newErrors} new error${newErrors > 1 ? 's' : ''} (total: ${stats.errors})`,
        });
      }

      if (newBlocked > 0) {
        newEntries.push({
          id: String(++entryId),
          timestamp: now,
          channel,
          level: 'warn',
          message: `${newBlocked} message${newBlocked > 1 ? 's' : ''} blocked`,
        });
      }
    }

    if (newEntries.length > 0) {
      setLogs((prev) => [...prev, ...newEntries].slice(-500));
    }
  }, [paused]);

  const { data: metrics } = useEmbeddedData<GatewayMetrics>(getGatewayMetrics, {
    refreshInterval: 5_000,
    enabled: !paused,
  });

  useEffect(() => {
    if (metrics) generateLogs(metrics);
  }, [metrics, generateLogs]);

  // Auto-scroll
  useEffect(() => {
    if (scrollRef.current && !paused) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [logs, paused]);

  const channels = Array.from(new Set(logs.map((l) => l.channel)));

  const filtered = logs.filter((log) => {
    if (channelFilter !== 'all' && log.channel !== channelFilter) return false;
    if (search && !log.message.toLowerCase().includes(search.toLowerCase())) return false;
    return true;
  });

  return (
    <Card>
      <div className="flex items-center justify-between mb-4">
        <Title>Live Activity Log</Title>
        <button
          onClick={() => setPaused((p) => !p)}
          className="flex items-center space-x-1 px-3 py-1.5 text-sm rounded-md bg-gray-100 dark:bg-gray-800 hover:bg-gray-200 dark:hover:bg-gray-700 transition-colors"
        >
          {paused ? (
            <>
              <PlayIcon className="w-4 h-4" />
              <span>Resume</span>
            </>
          ) : (
            <>
              <PauseIcon className="w-4 h-4" />
              <span>Pause</span>
            </>
          )}
        </button>
      </div>

      <div className="flex items-center space-x-3 mb-4">
        <TextInput
          icon={MagnifyingGlassIcon}
          placeholder="Search logs..."
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          className="flex-1"
        />
        <Select value={channelFilter} onValueChange={setChannelFilter} className="w-40">
          <SelectItem value="all">All Channels</SelectItem>
          {channels.map((ch) => (
            <SelectItem key={ch} value={ch}>
              {ch}
            </SelectItem>
          ))}
        </Select>
      </div>

      <div
        ref={scrollRef}
        className="h-96 overflow-y-auto bg-gray-950 rounded-lg p-4 font-mono text-xs space-y-1"
      >
        {filtered.length === 0 ? (
          <div className="flex items-center justify-center h-full">
            <Text className="text-gray-500">
              {paused ? 'Paused — click Resume to continue' : 'Waiting for activity...'}
            </Text>
          </div>
        ) : (
          filtered.map((log) => (
            <div key={log.id} className="flex items-start space-x-2 text-gray-300">
              <span className="text-gray-600 shrink-0">
                {new Date(log.timestamp).toLocaleTimeString()}
              </span>
              <Badge color={LEVEL_COLORS[log.level]} size="xs" className="shrink-0">
                {log.level.toUpperCase()}
              </Badge>
              <Badge color="blue" size="xs" className="shrink-0">
                {log.channel}
              </Badge>
              <span>{log.message}</span>
            </div>
          ))
        )}
      </div>

      <div className="flex items-center justify-between mt-2">
        <Text className="text-xs text-gray-500">{filtered.length} entries</Text>
        {logs.length > 0 && (
          <button
            onClick={() => setLogs([])}
            className="text-xs text-gray-500 hover:text-gray-700 dark:hover:text-gray-300 transition-colors"
          >
            Clear
          </button>
        )}
      </div>
    </Card>
  );
}
