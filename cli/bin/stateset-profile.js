#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { parseArgs } from 'node:util';
import YAML from 'yaml';
import {
  businessProfileDoctor,
  businessProfileContext,
  compileBusinessProfileKernelPolicy,
  createBusinessPack,
  diffBusinessProfiles,
  initBusinessProfile,
  installBusinessPack,
  listBusinessPacks,
  loadBusinessPack,
  loadBusinessProfile,
  validateBusinessProfile,
  writeBusinessProfile,
} from '../src/business-profile.js';
import { runMain } from '../src/graceful-shutdown.js';

const HELP = `
StateSet business profiles

USAGE:
  stateset-profile <init|show|doctor|context|diff|export|apply|pack|kernel-policy> [options]

COMMANDS:
  init                 Create .stateset/business.yaml
  show                 Print the current profile
  doctor               Validate the profile and its declarations
  context              Print the compact operating brief for agents
  diff --against FILE  Compare the current profile with another profile
  export --output FILE Export the current profile to a portable file
  apply --file FILE    Validate and install a profile (preview by default)
  pack list             List local starter packs
  pack inspect --file   Validate and inspect a pack
  pack install --file   Preview or install a local pack
  pack create --output  Fork the current profile into a pack
  kernel-policy         Narrow a trusted kernel policy with profile restrictions

OPTIONS:
  --root DIR           Project directory (default: current directory)
  --file FILE          Profile input for apply
  --against FILE       Profile to compare for diff
  --output FILE        Output file for export
  --force              Replace an existing profile
  --json               Emit machine-readable output
  --apply              Install a profile or pack instead of previewing it
  --replace             Replace the current profile instead of merging a pack
  --name NAME           Pack name for pack create
  --description TEXT    Pack description for pack create
  --base FILE           Operator-owned kernel policy for kernel-policy
  --version VERSION     New kernel policy version for kernel-policy
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
  const { values, positionals } = parseArgs({
    args: args.slice(1),
    options: {
      root: { type: 'string', default: process.cwd() },
      file: { type: 'string' },
      against: { type: 'string' },
      output: { type: 'string' },
      force: { type: 'boolean', default: false },
      json: { type: 'boolean', default: false },
      apply: { type: 'boolean', default: false },
      replace: { type: 'boolean', default: false },
      name: { type: 'string' },
      description: { type: 'string', default: '' },
      base: { type: 'string' },
      version: { type: 'string' },
    },
    allowPositionals: true,
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
  if (command === 'context') {
    const context = businessProfileContext(root);
    output(values.json ? context : context.text, values.json);
    if (!context.ready) process.exitCode = 1;
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
    fs.writeFileSync(path.resolve(values.output), JSON.stringify(current.profile, null, 2) + '\n', {
      mode: 0o600,
    });
    return output({ ok: true, file: path.resolve(values.output) }, values.json);
  }
  if (command === 'kernel-policy') {
    if (!values.base || !values.version)
      throw new Error('kernel-policy requires --base FILE and --version VERSION');
    const current = loadBusinessProfile(root);
    if (!current.exists) throw new Error('kernel-policy requires an installed business profile');
    if (current.errors?.length) throw new Error(current.errors.join('; '));
    let basePolicy;
    try {
      basePolicy = JSON.parse(fs.readFileSync(path.resolve(values.base), 'utf8'));
    } catch (error) {
      throw new Error(`Unable to load base kernel policy: ${error.message}`);
    }
    const compiled = compileBusinessProfileKernelPolicy(
      current.profile,
      basePolicy,
      values.version,
    );
    const destination = values.output ? path.resolve(values.output) : null;
    if (!values.apply) {
      return output(
        {
          ok: true,
          preview: true,
          destination,
          restrictions: compiled.restrictions,
          policy: compiled.policy,
          message: 'Preview only. Review the policy, then re-run with --apply --output FILE.',
        },
        values.json,
      );
    }
    if (!destination) throw new Error('kernel-policy --apply requires --output FILE');
    if (
      destination === path.resolve(values.base) ||
      (fs.existsSync(destination) &&
        fs.realpathSync(destination) === fs.realpathSync(path.resolve(values.base)))
    )
      throw new Error('kernel-policy output must differ from the operator-owned base policy');
    fs.mkdirSync(path.dirname(destination), { recursive: true });
    fs.writeFileSync(destination, `${JSON.stringify(compiled.policy, null, 2)}\n`, {
      mode: 0o600,
      flag: values.force ? 'w' : 'wx',
    });
    fs.chmodSync(destination, 0o600);
    return output(
      { ok: true, preview: false, file: destination, restrictions: compiled.restrictions },
      values.json,
    );
  }
  if (command === 'apply') {
    if (!values.file) throw new Error('apply requires --file FILE');
    const profile = readProfile(values.file);
    const current = loadBusinessProfile(root);
    if (current.errors?.length) throw new Error(current.errors.join('; '));
    const destination = path.resolve(root, '.stateset', 'business.yaml');
    const changes = diffBusinessProfiles(current.profile, profile);
    if (!values.apply) {
      return output(
        {
          ok: true,
          preview: true,
          destination,
          changes,
          message: 'Preview only. Re-run with --apply to install the profile.',
        },
        values.json,
      );
    }
    const file = writeBusinessProfile(profile, root, { force: values.force });
    return output(
      {
        ok: true,
        preview: false,
        file,
        changes,
        message: 'Profile installed; database mutations still require governed writes.',
      },
      values.json,
    );
  }
  if (command === 'pack') {
    const action = positionals[0] || 'list';
    if (action === 'list') {
      return output(listBusinessPacks(path.resolve(root, 'profiles')), values.json);
    }
    if (action === 'create') {
      if (!values.output) throw new Error('pack create requires --output DIR');
      if (!values.name) throw new Error('pack create requires --name NAME');
      return output(
        createBusinessPack(values.output, root, {
          name: values.name,
          description: values.description,
        }),
        values.json,
      );
    }
    if (!values.file) throw new Error(`pack ${action} requires --file PATH`);
    if (action === 'inspect') {
      const pack = loadBusinessPack(values.file);
      return output(
        {
          name: pack.name,
          version: pack.version,
          description: pack.description,
          source: pack.directory,
          valid: pack.errors.length === 0,
          errors: pack.errors,
        },
        values.json,
      );
    }
    if (action === 'install') {
      const result = installBusinessPack(values.file, root, {
        force: values.force,
        preview: !values.apply,
        replace: values.replace,
      });
      return output(
        {
          ok: true,
          preview: result.preview,
          name: result.name,
          version: result.version,
          destination: result.destination,
          lockFile: result.lockFile,
          changes: result.changes,
          message: result.preview
            ? 'Preview only. Re-run with --apply to install the pack.'
            : 'Pack installed; database mutations still require governed writes.',
        },
        values.json,
      );
    }
    throw new Error(`Unknown pack action: ${action}`);
  }
  throw new Error(`Unknown command: ${command}`);
}

runMain('stateset-profile', main);
