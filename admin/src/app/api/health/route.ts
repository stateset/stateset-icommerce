/**
 * GET /api/health
 *
 * Enhanced health endpoint with dependency checks.
 * Returns overall health status, uptime, and dependency connectivity.
 */

import { NextResponse } from 'next/server';
import { getServerStateSetApiUrl } from '@/lib/stateset-api-url';
import { APP_VERSION } from '@/lib/version';

const startTime = Date.now();

interface DependencyStatus {
  name: string;
  status: 'healthy' | 'unhealthy' | 'degraded';
  latencyMs?: number;
  error?: string;
}

async function checkRedis(): Promise<DependencyStatus> {
  const url = process.env.UPSTASH_REDIS_REST_URL;
  const token = process.env.UPSTASH_REDIS_REST_TOKEN;

  if (!url || !token) {
    return { name: 'redis', status: 'degraded', error: 'Not configured' };
  }

  const start = Date.now();
  try {
    const response = await fetch(url, {
      method: 'POST',
      headers: {
        Authorization: `Bearer ${token}`,
        'Content-Type': 'application/json',
      },
      body: JSON.stringify(['PING']),
      signal: AbortSignal.timeout(3000),
    });

    const latencyMs = Date.now() - start;

    if (response.ok) {
      return { name: 'redis', status: 'healthy', latencyMs };
    }
    return { name: 'redis', status: 'unhealthy', latencyMs, error: `HTTP ${response.status}` };
  } catch (error) {
    return {
      name: 'redis',
      status: 'unhealthy',
      latencyMs: Date.now() - start,
      error: error instanceof Error ? error.message : 'Unknown error',
    };
  }
}

async function checkStateSetAPI(): Promise<DependencyStatus> {
  const apiUrl = getServerStateSetApiUrl();
  const start = Date.now();

  try {
    const response = await fetch(`${apiUrl}/health`, {
      method: 'GET',
      signal: AbortSignal.timeout(5000),
    });

    const latencyMs = Date.now() - start;

    if (response.ok) {
      return { name: 'stateset-api', status: 'healthy', latencyMs };
    }
    return { name: 'stateset-api', status: 'degraded', latencyMs, error: `HTTP ${response.status}` };
  } catch (error) {
    return {
      name: 'stateset-api',
      status: 'unhealthy',
      latencyMs: Date.now() - start,
      error: error instanceof Error ? error.message : 'Unknown error',
    };
  }
}

export async function GET() {
  const [redis, api] = await Promise.all([checkRedis(), checkStateSetAPI()]);

  const dependencies: DependencyStatus[] = [redis, api];
  const allHealthy = dependencies.every((d) => d.status === 'healthy');
  const anyUnhealthy = dependencies.some((d) => d.status === 'unhealthy');

  const overallStatus = anyUnhealthy ? 'unhealthy' : allHealthy ? 'healthy' : 'degraded';
  const uptimeSeconds = Math.floor((Date.now() - startTime) / 1000);

  const body = {
    status: overallStatus,
    version: APP_VERSION,
    uptime: uptimeSeconds,
    timestamp: new Date().toISOString(),
    dependencies,
  };

  const httpStatus = overallStatus === 'unhealthy' ? 503 : 200;

  return NextResponse.json(body, { status: httpStatus });
}
