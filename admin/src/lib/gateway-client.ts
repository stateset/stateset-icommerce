/**
 * Client-side fetch wrappers for the gateway proxy API routes.
 *
 * All requests go through /api/gateway/* which proxies to the CLI HTTP gateway.
 */

import type {
  GatewayHealth,
  GatewayReadiness,
  GatewayMetrics,
  GatewayPlugin,
  GatewayCommand,
  GatewayDaemon,
} from '@/lib/types/gateway';

const BASE = '/api/gateway';

async function gatewayFetch<T>(path: string): Promise<T> {
  const res = await fetch(`${BASE}${path}`);
  if (!res.ok) {
    const body = await res.json().catch(() => ({ error: res.statusText }));
    throw new Error(
      body?.data?.error || body?.error?.message || `Gateway request failed: ${res.status}`
    );
  }
  const json = await res.json();
  return (json.data ?? json) as T;
}

export async function getGatewayHealth(): Promise<GatewayHealth> {
  return gatewayFetch<GatewayHealth>('/health');
}

export async function getGatewayReadiness(): Promise<GatewayReadiness> {
  return gatewayFetch<GatewayReadiness>('/ready');
}

export async function getGatewayMetrics(): Promise<GatewayMetrics> {
  return gatewayFetch<GatewayMetrics>('/metrics');
}

export async function getGatewayPlugins(): Promise<GatewayPlugin[]> {
  const data = await gatewayFetch<{ plugins: GatewayPlugin[] }>('/plugins');
  return data.plugins;
}

export async function getGatewayCommands(): Promise<GatewayCommand[]> {
  const data = await gatewayFetch<{ commands: GatewayCommand[] }>('/commands');
  return data.commands;
}

export async function getGatewayDaemon(): Promise<GatewayDaemon> {
  return gatewayFetch<GatewayDaemon>('/daemon');
}
