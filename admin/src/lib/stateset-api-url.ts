export const DEFAULT_STATESET_API_URL = 'https://api.sandbox.stateset.app';

function normalizeApiUrl(value: string | undefined | null): string | null {
  if (typeof value !== 'string') return null;
  const trimmed = value.trim();
  if (!trimmed) return null;
  return trimmed.replace(/\/+$/, '');
}

function toOrigin(value: string): string {
  try {
    return new URL(value).origin;
  } catch {
    return value;
  }
}

export function getPublicStateSetApiUrl(): string {
  return normalizeApiUrl(process.env.NEXT_PUBLIC_STATESET_API_URL) ?? DEFAULT_STATESET_API_URL;
}

export function getServerStateSetApiUrl(): string {
  return (
    normalizeApiUrl(process.env.STATESET_API_URL) ??
    normalizeApiUrl(process.env.NEXT_PUBLIC_STATESET_API_URL) ??
    DEFAULT_STATESET_API_URL
  );
}

export function getStateSetApiConnectSources(): string[] {
  return Array.from(
    new Set(
      [DEFAULT_STATESET_API_URL, getPublicStateSetApiUrl(), getServerStateSetApiUrl()].map(
        toOrigin,
      ),
    ),
  );
}
