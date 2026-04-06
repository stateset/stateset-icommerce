import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  DEFAULT_STATESET_API_URL,
  getPublicStateSetApiUrl,
  getServerStateSetApiUrl,
  getStateSetApiConnectSources,
} from '@/lib/stateset-api-url';

afterEach(() => {
  vi.unstubAllEnvs();
});

describe('stateset-api-url', () => {
  it('defaults the public URL to the sandbox API', () => {
    expect(getPublicStateSetApiUrl()).toBe(DEFAULT_STATESET_API_URL);
  });

  it('prefers STATESET_API_URL on the server', () => {
    vi.stubEnv('STATESET_API_URL', 'https://api.internal.stateset.app/');
    vi.stubEnv('NEXT_PUBLIC_STATESET_API_URL', 'https://api.public.stateset.app');

    expect(getServerStateSetApiUrl()).toBe('https://api.internal.stateset.app');
  });

  it('falls back to NEXT_PUBLIC_STATESET_API_URL when STATESET_API_URL is unset', () => {
    vi.stubEnv('NEXT_PUBLIC_STATESET_API_URL', 'https://api.public.stateset.app/');

    expect(getServerStateSetApiUrl()).toBe('https://api.public.stateset.app');
  });

  it('deduplicates connect sources by origin', () => {
    vi.stubEnv('STATESET_API_URL', 'https://api.internal.stateset.app/v1');
    vi.stubEnv('NEXT_PUBLIC_STATESET_API_URL', 'https://api.internal.stateset.app');

    expect(getStateSetApiConnectSources()).toEqual([
      'https://api.sandbox.stateset.app',
      'https://api.internal.stateset.app',
    ]);
  });
});
