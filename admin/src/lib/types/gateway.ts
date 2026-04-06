/**
 * TypeScript types for CLI HTTP Gateway data.
 *
 * These mirror the response shapes from the gateway endpoints
 * defined in cli/src/channels/http-gateway.js and cli/src/channels/metrics.js.
 */

// -- GET /health
export interface GatewayHealth {
  status: 'ok';
  uptime: number;
  timestamp: string;
  version: string;
  subsystems: {
    voice: 'enabled' | 'disabled';
    browser: 'enabled' | 'disabled';
    memory: 'enabled' | 'disabled';
    heartbeat: 'enabled' | 'disabled';
  };
  memory?: Record<string, unknown>;
  channels?: Record<string, unknown>;
}

// -- GET /ready
export interface GatewayReadiness {
  status: 'ready' | 'not_ready';
  timestamp: string;
  checks: {
    database: 'ok' | 'unavailable';
    memory?: 'ok' | 'error';
    embeddingService: 'configured' | 'not_configured';
  };
}

// -- Per-channel stats from ChannelMetrics.getSummary()
export interface ChannelStats {
  messagesReceived: number;
  responsesSent: number;
  errors: number;
  blocked: number;
  avgResponseMs: number;
  lastMessageAt: string | null;
}

// -- GET /metrics
export interface GatewayMetrics {
  uptime: string;
  uptimeMs: number;
  totals: {
    messagesReceived: number;
    responsesSent: number;
    errors: number;
    blocked: number;
    avgResponseMs: number;
  };
  channels: Record<string, ChannelStats>;
  commandUsage: Record<string, number>;
}

// -- GET /plugins
export interface GatewayPlugin {
  id: string;
  name: string;
  version?: string;
  enabled?: boolean;
}

// -- GET /commands
export interface GatewayCommand {
  name: string;
  description: string;
  aliases: string[];
  source: string;
  category: string;
  acceptsArgs: boolean;
}

// -- GET /daemon
export interface GatewayDaemon {
  service: string;
  active: string;
  enabled: string;
  pid?: number;
  processUptime?: string;
  memoryBytes?: number;
  memoryMB?: number;
  tailscale?: {
    connected: boolean;
    hostname?: string;
    tailnet?: string;
    ips?: string[];
    url?: string | null;
  };
  sshTunnels?: { activeProcesses: number };
}

// -- Channel name constants
export const CHANNEL_NAMES = [
  'telegram',
  'discord',
  'slack',
  'whatsapp',
  'signal',
  'google-chat',
  'imessage',
  'teams',
  'matrix',
  'webchat',
  'http',
] as const;

export type ChannelName = (typeof CHANNEL_NAMES)[number];

// -- Time-series snapshot for chart accumulation
export interface MetricsSnapshot {
  timestamp: string;
  messagesReceived: number;
  responsesSent: number;
  errors: number;
  avgResponseMs: number;
}

// -- Composite type for overview dashboard
export interface GatewayOverview {
  health: GatewayHealth;
  metrics: GatewayMetrics;
  readiness: GatewayReadiness;
}
