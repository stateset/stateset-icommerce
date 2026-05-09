// Unit tests for the active-org validator. Cookie + server-action paths
// are integration-tested via the `<OrgSwitcher />` flow in browser tests;
// this file just locks down the validation rules used by both the cookie
// reader and the server action.

import { describe, expect, it } from 'vitest';

import {
  ACTIVE_ORG_COOKIE,
  ACTIVE_ORG_COOKIE_OPTIONS,
  isValidOrgId,
} from '@/lib/shared/active-org';

describe('active-org · isValidOrgId', () => {
  it('accepts URL-safe ids', () => {
    expect(isValidOrgId('acme')).toBe(true);
    expect(isValidOrgId('ACME-001')).toBe(true);
    expect(isValidOrgId('org_42.test')).toBe(true);
    expect(isValidOrgId('a')).toBe(true);
  });

  it('rejects empty / whitespace / overlong values', () => {
    expect(isValidOrgId('')).toBe(false);
    expect(isValidOrgId('a'.repeat(129))).toBe(false);
  });

  it('rejects non-strings', () => {
    expect(isValidOrgId(null)).toBe(false);
    expect(isValidOrgId(undefined)).toBe(false);
    expect(isValidOrgId(42)).toBe(false);
    expect(isValidOrgId({})).toBe(false);
  });

  it('rejects characters outside [A-Za-z0-9_.-]', () => {
    expect(isValidOrgId('has space')).toBe(false);
    expect(isValidOrgId('has/slash')).toBe(false);
    expect(isValidOrgId('has;semi')).toBe(false);
    expect(isValidOrgId('has<>tag')).toBe(false);
    expect(isValidOrgId('has=equals')).toBe(false);
    expect(isValidOrgId('has\nnewline')).toBe(false);
    // Cookie injection-style payloads must be rejected outright.
    expect(isValidOrgId('a"; evil; b="c')).toBe(false);
  });

  it('accepts the boundary length (exactly 128 chars)', () => {
    expect(isValidOrgId('a'.repeat(128))).toBe(true);
  });
});

describe('active-org · constants', () => {
  it('exposes a stable cookie name', () => {
    expect(ACTIVE_ORG_COOKIE).toBe('stateset_active_org');
  });

  it('cookie options keep it operator-readable but server-coordinated', () => {
    // Not HttpOnly: client component reads it for the dropdown.
    expect(ACTIVE_ORG_COOKIE_OPTIONS.httpOnly).toBe(false);
    // SameSite=lax stops cross-site form leakage but keeps top-level navs working.
    expect(ACTIVE_ORG_COOKIE_OPTIONS.sameSite).toBe('lax');
    // 30-day max age — operator preference, not a security boundary.
    expect(ACTIVE_ORG_COOKIE_OPTIONS.maxAge).toBe(60 * 60 * 24 * 30);
    expect(ACTIVE_ORG_COOKIE_OPTIONS.path).toBe('/');
    expect(ACTIVE_ORG_COOKIE_OPTIONS.name).toBe(ACTIVE_ORG_COOKIE);
  });
});
