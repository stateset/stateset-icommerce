#!/usr/bin/env node
import assert from 'node:assert/strict';
import { mkdirSync, mkdtempSync, rmSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import postcss from 'postcss';
import tailwindcss from 'tailwindcss';
import loadConfig from 'tailwindcss/loadConfig.js';

const config = loadConfig(fileURLToPath(new URL('../tailwind.config.ts', import.meta.url)));
const originalDirectory = process.cwd();
const fixture = mkdtempSync(path.join(os.tmpdir(), 'stateset-design-css-'));
const adminDirectory = path.join(fixture, 'repo', 'admin');
mkdirSync(adminDirectory, { recursive: true });

try {
  // No sibling design repository or application source can supply these
  // utilities. They must come from the installed design package itself.
  process.chdir(adminDirectory);
  const result = await postcss([tailwindcss(config)]).process('@tailwind utilities;', {
    from: undefined,
  });
  const required = new Map([
    ['.left-\\[12px\\]', ['left', '12px']],
    ['.top-\\[28px\\]', ['top', '28px']],
    ['.inset-x-0', ['left', '0px']],
    ['.-mx-1', ['margin-left', '-0.25rem']],
  ]);
  for (const [selector, [property, value]] of required) {
    let found = false;
    result.root.walkRules(selector, (rule) => {
      rule.walkDecls(property, (declaration) => {
        if (declaration.value === value) found = true;
      });
    });
    assert.ok(found, `Installed design CSS must include ${selector} { ${property}: ${value} }`);
  }
  console.log('Installed design package generates component geometry without a sibling repository');
} finally {
  process.chdir(originalDirectory);
  rmSync(fixture, { recursive: true, force: true });
}
