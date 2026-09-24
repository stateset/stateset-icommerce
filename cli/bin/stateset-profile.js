#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { parseArgs } from 'node:util';
import YAML from 'yaml';
import {
  businessProfileDoctor,
  diffBusinessProfiles,
  initBusinessProfile,
  loadBusinessProfile,
  validateBusinessProfile,
  writeBusinessProfile,
} from '../src/business-profile.js';
import { runMain } from '../src/graceful-shutdown.js';

const HELP = `
StateSet business profiles

USAGE:
  stateset-profile <init|show|doctor|diff|export|apply> [options]

COMMANDS:
  init                 Create .stateset/business.yaml
  show                 Print the current profile
  doctor               Validate the profile and its declarations
  diff --against FILE  Compare the current profile with another profile
  export --output FILE Export the current profile to a portable file
  apply --file FILE    Validate and install a profile (preview by default)

OPTIONS:
  --root DIR           Project directory (default: current directory)
  --file FILE          Profile input for apply
  --against FILE       Profile to compare for diff
  --output FILE        Output file for export
  --force              Replace an existing profile
  --json               Emit machine-readable output
`;

function readProfile(file) {
  const resolved = path.resolve(file);
  let profile;
  try {
    profile = YAML.parse(fs.readFileSync(resolved, 'utf8'));
  } catch (error) {
    throw new Error(`Unable to load profile ${file}: ${error.message}`);
  }
  const errors = validateBusinessProfile(profile);
  if (errors.length) throw new Error(errors.join('; '));
  return profile;
}

function output(value, json) {
  console.log(json ? JSON.stringify(value, null, 2) : value);
}

async function main() {
  const args = process.argv.slice(2);
  if (args.includes('--help') || args.includes('-h')) return console.log(HELP.trim());
  const command = args[0] || 'show';
  const { values } = parseArgs({
    args: args.slice(1),
    options: {
      root: { type: 'string', default: process.cwd() },
      file: { type: 'string' },
      against: { type: 'string' },
      output: { type: 'string' },
      force: { type: 'boolean', default: false },
      json: { type: 'boolean', default: false },
    },
    allowPositionals: false,
  });
  const root = path.resolve(values.root);
  if (command === 'init') {
    const file = initBusinessProfile(root, { force: values.force });
    return output({ ok: true, file }, values.json);
  }
  if (command === 'doctor') {
    const report = businessProfileDoctor(root);
    output(report, values.json);
    if (!report.ready) process.exitCode = 1;
    return;
  }
  if (command === 'show') {
    const loaded = loadBusinessProfile(root);
    if (loaded.errors?.length) throw new Error(loaded.errors.join('; '));
    return output(loaded.profile, values.json);
  }
  if (command === 'diff') {
    if (!values.against) throw new Error('diff requires --against FILE');
    const current = loadBusinessProfile(root);
    if (current.errors?.length) throw new Error(current.errors.join('; '));
    return output(diffBusinessProfiles(current.profile, readProfile(values.against)), values.json);
  }
  if (command === 'export') {
    if (!values.output) throw new Error('export requires --output FILE');
    const current = loadBusinessProfile(root);
    if (current.errors?.length) throw new Error(current.errors.join('; '));
    fs.mkdirSync(path.dirname(path.resolve(values.output)), { recursive: true });
    fs.writeFileSync(path.resolve(values.output), JSON.stringify(current.profile, null, 2) + '\n', { mode: 0o600 });
    return output({ ok: true, file: path.resolve(values.output) }, values.json);
  }
  if (command === 'apply') {
    if (!values.file) throw new Error('apply requires --file FILE');
    const profile = readProfile(values.file);
    const file = writeBusinessProfile(profile, root, { force: values.force });
    return output({ ok: true, preview: true, file, message: 'Profile installed; database mutations require an explicit governed apply command.' }, values.json);
  }
  throw new Error(`Unknown command: ${command}`);
}

runMain('stateset-profile', main);
