/**
 * Application Metrics
 *
 * In-memory counters for Prometheus-compatible metrics.
 * These are reset on deploy, which is acceptable for Prometheus scraping.
 */

import { APP_VERSION } from '../version';

// In-memory counters
const counters = {
  httpRequestsTotal: new Map<string, number>(),
  httpRequestDurationSum: new Map<string, number>(),
  httpRequestDurationCount: new Map<string, number>(),
  activeLoops: 0,
};

/**
 * Record an HTTP request metric.
 */
export function recordRequest(
  method: string,
  path: string,
  status: number,
  durationMs: number,
): void {
  const key = `method="${method}",path="${path}",status="${status}"`;
  counters.httpRequestsTotal.set(key, (counters.httpRequestsTotal.get(key) || 0) + 1);
  counters.httpRequestDurationSum.set(
    key,
    (counters.httpRequestDurationSum.get(key) || 0) + durationMs / 1000,
  );
  counters.httpRequestDurationCount.set(key, (counters.httpRequestDurationCount.get(key) || 0) + 1);
}

/**
 * Update active loops gauge.
 */
export function setActiveLoops(count: number): void {
  counters.activeLoops = count;
}

/**
 * Format all metrics as Prometheus text.
 */
export function formatPrometheus(): string {
  const lines: string[] = [];

  // App info
  lines.push('# HELP app_info Application metadata');
  lines.push('# TYPE app_info gauge');
  lines.push(`app_info{version="${APP_VERSION}",env="${process.env.NODE_ENV || 'development'}"} 1`);
  lines.push('');

  // HTTP requests total
  lines.push('# HELP http_requests_total Total number of HTTP requests');
  lines.push('# TYPE http_requests_total counter');
  for (const [labels, count] of counters.httpRequestsTotal) {
    lines.push(`http_requests_total{${labels}} ${count}`);
  }
  lines.push('');

  // HTTP request duration
  lines.push('# HELP http_request_duration_seconds HTTP request duration in seconds');
  lines.push('# TYPE http_request_duration_seconds summary');
  for (const [labels, sum] of counters.httpRequestDurationSum) {
    const count = counters.httpRequestDurationCount.get(labels) || 0;
    lines.push(`http_request_duration_seconds_sum{${labels}} ${sum.toFixed(6)}`);
    lines.push(`http_request_duration_seconds_count{${labels}} ${count}`);
  }
  lines.push('');

  // Active loops
  lines.push('# HELP active_loops_total Number of currently active agent loops');
  lines.push('# TYPE active_loops_total gauge');
  lines.push(`active_loops_total ${counters.activeLoops}`);
  lines.push('');

  // Process uptime
  lines.push('# HELP process_uptime_seconds Process uptime in seconds');
  lines.push('# TYPE process_uptime_seconds gauge');
  lines.push(`process_uptime_seconds ${Math.floor(process.uptime())}`);
  lines.push('');

  return lines.join('\n');
}
