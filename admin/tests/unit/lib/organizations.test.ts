// Unit tests for the org-list server action. Covers the env-var
// override path used by `<TopBar />` to populate the multi-org switcher
// during local development before the backend `/api/v1/organizations`
// endpoint exists.

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { listOrganizations } from '@/app/actions/organizations';

describe('listOrganizations', () => {
  let originalEnv: string | undefined;

  beforeEach(() => {
    originalEnv = process.env.NEXT_PUBLIC_ADMIN_DEV_ORGS;
  });

  afterEach(() => {
    if (originalEnv === undefined) {
      delete process.env.NEXT_PUBLIC_ADMIN_DEV_ORGS;
    } else {
      process.env.NEXT_PUBLIC_ADMIN_DEV_ORGS = originalEnv;
    }
    vi.restoreAllMocks();
  });

  it('returns empty when the dev env var is unset (production default)', async () => {
    delete process.env.NEXT_PUBLIC_ADMIN_DEV_ORGS;
    expect(await listOrganizations()).toEqual([]);
  });

  it('returns empty when the dev env var is whitespace only', async () => {
    process.env.NEXT_PUBLIC_ADMIN_DEV_ORGS = '   ';
    expect(await listOrganizations()).toEqual([]);
  });

  it('parses a comma-separated id list, falling back to id-as-name', async () => {
    process.env.NEXT_PUBLIC_ADMIN_DEV_ORGS = 'acme,globex,umbrella';
    expect(await listOrganizations()).toEqual([
      { id: 'acme', name: 'acme' },
      { id: 'globex', name: 'globex' },
      { id: 'umbrella', name: 'umbrella' },
    ]);
  });

  it('parses id:Name pairs', async () => {
    process.env.NEXT_PUBLIC_ADMIN_DEV_ORGS = 'acme:Acme Corp,globex:Globex';
    expect(await listOrganizations()).toEqual([
      { id: 'acme', name: 'Acme Corp' },
      { id: 'globex', name: 'Globex' },
    ]);
  });

  it('mixes id-only and id:Name entries', async () => {
    process.env.NEXT_PUBLIC_ADMIN_DEV_ORGS = 'acme,globex:Globex Industries';
    expect(await listOrganizations()).toEqual([
      { id: 'acme', name: 'acme' },
      { id: 'globex', name: 'Globex Industries' },
    ]);
  });

  it('drops empty entries from the comma list', async () => {
    process.env.NEXT_PUBLIC_ADMIN_DEV_ORGS = 'acme,,globex,';
    expect(await listOrganizations()).toEqual([
      { id: 'acme', name: 'acme' },
      { id: 'globex', name: 'globex' },
    ]);
  });

  it('preserves a colon-bearing display name verbatim', async () => {
    // Display names with colons (e.g. "Acme: West Region") should round-trip.
    process.env.NEXT_PUBLIC_ADMIN_DEV_ORGS = 'acme:Acme: West Region';
    const result = await listOrganizations();
    expect(result).toEqual([{ id: 'acme', name: 'Acme: West Region' }]);
  });

  it('trims whitespace around id and name', async () => {
    process.env.NEXT_PUBLIC_ADMIN_DEV_ORGS = '  acme  :  Acme  ,  globex  ';
    expect(await listOrganizations()).toEqual([
      { id: 'acme', name: 'Acme' },
      { id: 'globex', name: 'globex' },
    ]);
  });
});
