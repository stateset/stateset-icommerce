/**
 * Tests for Application Metrics
 *
 * @module tests/unit/lib/metrics
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { recordRequest, setActiveLoops, formatPrometheus } from '@/lib/shared/metrics';

describe('recordRequest', () => {
  it('records a request without throwing', () => {
    expect(() => recordRequest('GET', '/api/health', 200, 42)).not.toThrow();
  });

  it('recorded request appears in Prometheus output', () => {
    recordRequest('POST', '/api/orders', 201, 150);
    const output = formatPrometheus();
    expect(output).toContain('http_requests_total');
    expect(output).toContain('method="POST"');
    expect(output).toContain('path="/api/orders"');
    expect(output).toContain('status="201"');
  });

  it('increments counter for repeated requests', () => {
    recordRequest('GET', '/api/test-inc', 200, 10);
    recordRequest('GET', '/api/test-inc', 200, 20);
    const output = formatPrometheus();
    const lines = output.split('\n');
    const counterLine = lines.find(
      (l) => l.includes('http_requests_total') && l.includes('/api/test-inc')
    );
    expect(counterLine).toBeDefined();
    // The counter should be at least 2
    const count = parseInt(counterLine!.split(' ').pop()!, 10);
    expect(count).toBeGreaterThanOrEqual(2);
  });

  it('records duration in seconds', () => {
    recordRequest('GET', '/api/dur-test', 200, 2500);
    const output = formatPrometheus();
    expect(output).toContain('http_request_duration_seconds_sum');
    expect(output).toContain('/api/dur-test');
  });
});

describe('setActiveLoops', () => {
  it('updates the active loops gauge', () => {
    setActiveLoops(5);
    const output = formatPrometheus();
    expect(output).toContain('active_loops_total 5');
  });

  it('can set to zero', () => {
    setActiveLoops(0);
    const output = formatPrometheus();
    expect(output).toContain('active_loops_total 0');
  });
});

describe('formatPrometheus', () => {
  it('returns a string', () => {
    const output = formatPrometheus();
    expect(typeof output).toBe('string');
  });

  it('includes app_info metric', () => {
    const output = formatPrometheus();
    expect(output).toContain('# HELP app_info');
    expect(output).toContain('# TYPE app_info gauge');
    expect(output).toContain('app_info{');
  });

  it('includes process_uptime_seconds metric', () => {
    const output = formatPrometheus();
    expect(output).toContain('# HELP process_uptime_seconds');
    expect(output).toContain('process_uptime_seconds');
  });

  it('includes TYPE declarations for all metric families', () => {
    const output = formatPrometheus();
    expect(output).toContain('# TYPE http_requests_total counter');
    expect(output).toContain('# TYPE http_request_duration_seconds summary');
    expect(output).toContain('# TYPE active_loops_total gauge');
    expect(output).toContain('# TYPE process_uptime_seconds gauge');
    expect(output).toContain('# TYPE app_info gauge');
  });

  it('includes HELP declarations for all metric families', () => {
    const output = formatPrometheus();
    expect(output).toContain('# HELP http_requests_total');
    expect(output).toContain('# HELP http_request_duration_seconds');
    expect(output).toContain('# HELP active_loops_total');
    expect(output).toContain('# HELP process_uptime_seconds');
    expect(output).toContain('# HELP app_info');
  });

  it('includes environment label in app_info', () => {
    const output = formatPrometheus();
    expect(output).toMatch(/app_info\{.*env="/);
  });

  it('includes version label in app_info', () => {
    const output = formatPrometheus();
    expect(output).toMatch(/app_info\{.*version="/);
  });
});
