#!/usr/bin/env node

function parseVersion(input) {
  const parts = String(input).trim().split(".");
  return {
    major: Number.parseInt(parts[0] || "0", 10),
    minor: Number.parseInt(parts[1] || "0", 10),
    patch: Number.parseInt(parts[2] || "0", 10),
  };
}

function compareVersions(a, b) {
  if (a.major !== b.major) return a.major - b.major;
  if (a.minor !== b.minor) return a.minor - b.minor;
  return a.patch - b.patch;
}

const requiredNodeRaw = process.argv[2];
const requiredNpmRaw = process.argv[3];

if (!requiredNodeRaw) {
  console.error("Usage: node scripts/check-node.mjs <required-node-version> [required-npm-version]");
  process.exit(2);
}

const requiredNode = parseVersion(requiredNodeRaw);
const currentNode = parseVersion(process.versions.node);

if (compareVersions(currentNode, requiredNode) < 0) {
  console.error(
    `Node ${requiredNodeRaw}+ is required for this command. Current: ${process.versions.node}. Run: nvm use`,
  );
  process.exit(1);
}

if (requiredNpmRaw) {
  const userAgent = process.env.npm_config_user_agent || "";
  const npmMatch = userAgent.match(/npm\/([0-9]+(?:\.[0-9]+){0,2})/i);
  const currentNpmRaw = npmMatch?.[1];
  if (!currentNpmRaw) {
    console.error("Unable to determine npm version from npm_config_user_agent.");
    process.exit(1);
  }
  const requiredNpm = parseVersion(requiredNpmRaw);
  const currentNpm = parseVersion(currentNpmRaw);

  if (compareVersions(currentNpm, requiredNpm) < 0) {
    console.error(
      `npm ${requiredNpmRaw}+ is required for this command. Current: ${currentNpmRaw}. Run: nvm use`,
    );
    process.exit(1);
  }
}
