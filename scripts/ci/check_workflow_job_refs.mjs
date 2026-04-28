#!/usr/bin/env node

import { readdirSync, readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const scriptDir = dirname(fileURLToPath(import.meta.url));
const repoRoot = join(scriptDir, '..', '..');
const workflowsDir = join(repoRoot, '.github', 'workflows');
const expectedReleaseWorkflowTags = new Map([
  ['publish-cli.yml', ['cli-v*']],
  ['publish-python.yml', ['py-v*']],
  ['publish-rust-crates.yml', ['v*']],
  ['java-release.yml', ['java-v*']],
  ['ruby-release.yml', ['ruby-v*']],
  ['php-release.yml', ['php-v*']],
]);

function parseInlineNeeds(raw) {
  return raw
    .split(',')
    .map((item) => item.trim().replace(/^['"]|['"]$/g, ''))
    .filter(Boolean);
}

function parseWorkflow(filePath) {
  const lines = readFileSync(filePath, 'utf8').split(/\r?\n/);
  const jobIds = new Map();
  const refs = [];
  const errors = [];

  let inJobs = false;
  let currentJob = null;
  let collectingNeeds = false;
  let collectingJobIf = false;

  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index];
    const lineNumber = index + 1;

    if (!inJobs) {
      if (line === 'jobs:') {
        inJobs = true;
      }
      continue;
    }

    if (/^\S/.test(line)) {
      inJobs = false;
      currentJob = null;
      collectingNeeds = false;
      collectingJobIf = false;
      continue;
    }

    const jobMatch = line.match(/^  ([A-Za-z0-9_-]+):\s*(?:#.*)?$/);
    if (jobMatch) {
      currentJob = jobMatch[1];
      collectingNeeds = false;
      collectingJobIf = false;
      if (jobIds.has(currentJob)) {
        errors.push(
          `${filePath}:${lineNumber} duplicates job id "${currentJob}" (first declared on line ${jobIds.get(currentJob)})`,
        );
      } else {
        jobIds.set(currentJob, lineNumber);
      }
      continue;
    }

    if (!currentJob) {
      continue;
    }

    if (collectingJobIf) {
      if (/^    [A-Za-z0-9_-]+:\s*/.test(line) || /^  [A-Za-z0-9_-]+:\s*/.test(line)) {
        collectingJobIf = false;
      } else if (/\bhashFiles\s*\(/.test(line)) {
        errors.push(
          `${filePath}:${lineNumber} job "${currentJob}" cannot use hashFiles() in a job-level if expression`,
        );
      }
    }

    const jobIfMatch = line.match(/^    if:\s*(.*)$/);
    if (jobIfMatch) {
      collectingJobIf = /^[>|]/.test(jobIfMatch[1].trim());
      if (/\bhashFiles\s*\(/.test(jobIfMatch[1])) {
        errors.push(
          `${filePath}:${lineNumber} job "${currentJob}" cannot use hashFiles() in a job-level if expression`,
        );
      }
      continue;
    }

    if (collectingNeeds) {
      if (/^\s*$/.test(line) || /^      #/.test(line)) {
        continue;
      }

      const listItemMatch = line.match(/^      -\s*([A-Za-z0-9_-]+)\s*(?:#.*)?$/);
      if (listItemMatch) {
        refs.push({
          jobId: currentJob,
          needs: listItemMatch[1],
          lineNumber,
        });
        continue;
      }

      collectingNeeds = false;
    }

    const inlineNeedsMatch = line.match(/^    needs:\s*\[(.*)\]\s*(?:#.*)?$/);
    if (inlineNeedsMatch) {
      for (const need of parseInlineNeeds(inlineNeedsMatch[1])) {
        refs.push({
          jobId: currentJob,
          needs: need,
          lineNumber,
        });
      }
      continue;
    }

    const scalarNeedsMatch = line.match(/^    needs:\s*([A-Za-z0-9_-]+)\s*(?:#.*)?$/);
    if (scalarNeedsMatch) {
      refs.push({
        jobId: currentJob,
        needs: scalarNeedsMatch[1],
        lineNumber,
      });
      continue;
    }

    if (/^    needs:\s*(?:#.*)?$/.test(line)) {
      collectingNeeds = true;
    }
  }

  return { jobIds, refs, errors };
}

function validateCiSuccessCoverage(filePath, jobIds, refs) {
  const errors = [];

  if (!filePath.endsWith('/ci.yml') || !jobIds.has('ci-success')) {
    return errors;
  }

  const ciSuccessNeeds = new Set(
    refs.filter((ref) => ref.jobId === 'ci-success').map((ref) => ref.needs),
  );
  const expectedNeeds = [...jobIds.keys()].filter((jobId) => jobId !== 'ci-success');

  for (const jobId of expectedNeeds) {
    if (!ciSuccessNeeds.has(jobId)) {
      errors.push(
        `${filePath}:${jobIds.get('ci-success')} aggregate job "ci-success" must need "${jobId}"`,
      );
    }
  }

  return errors;
}

function readPushTagTriggers(filePath) {
  const lines = readFileSync(filePath, 'utf8').split(/\r?\n/);
  const tags = [];
  let inPush = false;
  let inTags = false;

  for (const line of lines) {
    if (!inPush) {
      if (line === '  push:') {
        inPush = true;
      }
      continue;
    }

    if (!inTags) {
      if (/^    tags:\s*$/.test(line)) {
        inTags = true;
        continue;
      }

      if (/^  [A-Za-z0-9_-]+:\s*$/.test(line)) {
        break;
      }

      continue;
    }

    const tagMatch = line.match(/^\s{6}-\s*['"]([^'"]+)['"]\s*(?:#.*)?$/);
    if (tagMatch) {
      tags.push(tagMatch[1]);
      continue;
    }

    if (/^\s*$/.test(line) || /^\s{6}#/.test(line)) {
      continue;
    }

    break;
  }

  return tags;
}

function validateReleaseWorkflowTags(fileName, filePath) {
  const expectedTags = expectedReleaseWorkflowTags.get(fileName);
  if (!expectedTags) {
    return [];
  }

  const actualTags = readPushTagTriggers(filePath);
  const expectedKey = expectedTags.join('\u0000');
  const actualKey = actualTags.join('\u0000');

  if (expectedKey === actualKey) {
    return [];
  }

  return [
    `${filePath} push tag triggers must be [${expectedTags.join(', ')}], found [${actualTags.join(', ')}]`,
  ];
}

const workflowFiles = readdirSync(workflowsDir)
  .filter((fileName) => /\.ya?ml$/i.test(fileName))
  .sort();

const errors = [];
let totalJobs = 0;
let totalRefs = 0;

for (const fileName of workflowFiles) {
  const filePath = join(workflowsDir, fileName);
  const { jobIds, refs, errors: parseErrors } = parseWorkflow(filePath);
  totalJobs += jobIds.size;
  totalRefs += refs.length;
  errors.push(...parseErrors);
  errors.push(...validateCiSuccessCoverage(filePath, jobIds, refs));
  errors.push(...validateReleaseWorkflowTags(fileName, filePath));

  for (const ref of refs) {
    if (ref.jobId === ref.needs) {
      errors.push(`${filePath}:${ref.lineNumber} job "${ref.jobId}" cannot need itself`);
      continue;
    }

    if (!jobIds.has(ref.needs)) {
      errors.push(
        `${filePath}:${ref.lineNumber} job "${ref.jobId}" needs unknown job "${ref.needs}"`,
      );
    }
  }
}

if (errors.length > 0) {
  console.error('Workflow job reference check failed:\n');
  for (const error of errors) {
    console.error(`- ${error}`);
  }
  process.exit(1);
}

console.log(
  `Workflow job references are valid across ${workflowFiles.length} workflow files, ${totalJobs} jobs, and ${totalRefs} needs edges.`,
);
