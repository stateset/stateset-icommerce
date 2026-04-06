/**
 * GET /api/metrics
 *
 * Prometheus-compatible metrics endpoint.
 * Exposes: http_requests_total, http_request_duration_seconds, active_loops_total, app_info
 */

import { NextResponse } from 'next/server';
import { formatPrometheus } from '@/lib/shared/metrics';

export async function GET() {
  const body = formatPrometheus();

  return new NextResponse(body, {
    status: 200,
    headers: {
      'Content-Type': 'text/plain; version=0.0.4; charset=utf-8',
      'Cache-Control': 'no-cache, no-store, must-revalidate',
    },
  });
}
