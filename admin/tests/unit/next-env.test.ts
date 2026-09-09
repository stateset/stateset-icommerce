/**
 * `next-env.d.ts` drift guard.
 *
 * Next regenerates `next-env.d.ts` on almost every command (`next lint`,
 * `next build`, `next dev`, `next typegen`). The file says "should not be
 * edited", so nobody looks at it — which is exactly how a developer running
 * a *different* Next than the lockfile pins ends up committing that other
 * version's output, and the checkout shows a permanently-modified file that
 * every subsequent branch re-modifies.
 *
 * That is what happened here: the committed file is Next 15.5.24's output,
 * and an environment whose `admin/node_modules` still held Next 14.2.35 kept
 * rewriting it to the 14 format (no `routes.d.ts` reference, old docs URL).
 * The fix is `npm ci` in `admin/`, not editing the file.
 *
 * This test derives the expected content from the *lockfile* rather than from
 * whatever happens to be installed, so it is deterministic. If it fails:
 *
 *  - and your `admin/node_modules` is not the lockfile's Next, run `npm ci`
 *    in `admin/` and `git checkout -- next-env.d.ts`;
 *  - and the lockfile has moved to a new Next major, regenerate the file with
 *    that Next and update the expectation below.
 *
 * The Next 15 form comes from `next/dist/lib/typescript/writeAppTypeDeclarations`:
 * `next` types, image types (image imports are on by default), the routes
 * reference (emitted unconditionally in 15.x, `typedRoutes` or not), then the
 * notice — `app` in the docs URL because this project is App Router only, with
 * no `pages/` directory.
 *
 * @module tests/unit/next-env
 */

import { readFileSync } from 'node:fs';
import path from 'node:path';

import { describe, expect, it } from 'vitest';

const ADMIN_ROOT = path.resolve(__dirname, '..', '..');

function read(relative: string): string {
  return readFileSync(path.join(ADMIN_ROOT, relative), 'utf8');
}

/** The Next major the lockfile actually installs. */
function lockedNextMajor(): number {
  const lock = JSON.parse(read('package-lock.json')) as {
    packages?: Record<string, { version?: string }>;
  };
  const version = lock.packages?.['node_modules/next']?.version;
  expect(version, 'admin/package-lock.json must pin a next version').toBeTruthy();
  return Number.parseInt(String(version).split('.')[0], 10);
}

const NEXT_15_APP_ROUTER = [
  '/// <reference types="next" />',
  '/// <reference types="next/image-types/global" />',
  '/// <reference path="./.next/types/routes.d.ts" />',
  '',
  '// NOTE: This file should not be edited',
  '// see https://nextjs.org/docs/app/api-reference/config/typescript for more information.',
  '',
].join('\n');

describe('admin/next-env.d.ts', () => {
  it('is the output of the Next version the lockfile pins', () => {
    expect(lockedNextMajor()).toBe(15);
    expect(read('next-env.d.ts')).toBe(NEXT_15_APP_ROUTER);
  });

  it('carries no pages-router typings (this app is App Router only)', () => {
    // Next only emits the navigation-compat directive when BOTH app/ and
    // pages/ exist. Its presence would mean a stray pages directory.
    expect(read('next-env.d.ts')).not.toContain('navigation-types/compat');
  });

  it('is not tracked as editable — the notice must survive regeneration', () => {
    expect(read('next-env.d.ts')).toContain('This file should not be edited');
  });
});
