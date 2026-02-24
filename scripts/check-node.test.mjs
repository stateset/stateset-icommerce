import assert from "node:assert/strict";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const scriptPath = path.join(__dirname, "check-node.mjs");

function runCheckNode(args = [], env = {}) {
  return spawnSync(process.execPath, [scriptPath, ...args], {
    cwd: path.join(__dirname, ".."),
    env: { ...process.env, ...env },
    encoding: "utf8",
  });
}

function parseVersion(input) {
  const [major = "0", minor = "0", patch = "0"] = String(input).split(".");
  return {
    major: Number.parseInt(major, 10),
    minor: Number.parseInt(minor, 10),
    patch: Number.parseInt(patch, 10),
  };
}

function toVersionString(version) {
  return `${version.major}.${version.minor}.${version.patch}`;
}

function assertStatus(result, expectedStatus) {
  assert.equal(result.status, expectedStatus);

  // In restricted sandboxes, child stderr/stdout can be suppressed and an EPERM
  // error can be attached even when exit status is available.
  if (result.stderr === "" && result.stdout === "" && result.error) {
    assert.equal(result.error?.code, "EPERM");
  }
}

test("exits with usage when required node version is missing", () => {
  const result = runCheckNode();
  assertStatus(result, 2);
  if (result.stderr) {
    assert.match(result.stderr, /Usage: node scripts\/check-node\.mjs/);
  }
});

test("fails when required node version is greater than current", () => {
  const currentNode = parseVersion(process.versions.node);
  const impossibleRequirement = toVersionString({
    major: currentNode.major + 1,
    minor: 0,
    patch: 0,
  });

  const result = runCheckNode([impossibleRequirement]);
  assertStatus(result, 1);
  if (result.stderr) {
    assert.match(result.stderr, /Node .* is required/);
  }
});

test("fails when npm requirement is provided but npm user agent is unavailable", () => {
  const result = runCheckNode([process.versions.node, "10.0.0"], {
    npm_config_user_agent: "",
  });

  assertStatus(result, 1);
  if (result.stderr) {
    assert.match(result.stderr, /Unable to determine npm version/);
  }
});

test("fails when npm version from user agent is lower than required", () => {
  const result = runCheckNode([process.versions.node, "10.0.0"], {
    npm_config_user_agent: "npm/9.9.9 node/v20.20.0 linux x64",
  });

  assertStatus(result, 1);
  if (result.stderr) {
    assert.match(result.stderr, /npm 10\.0\.0\+ is required/);
  }
});

test("passes when both node and npm satisfy requirements", () => {
  const result = runCheckNode([process.versions.node, "10.0.0"], {
    npm_config_user_agent: "npm/10.8.2 node/v20.20.0 linux x64",
  });

  assertStatus(result, 0);
});
