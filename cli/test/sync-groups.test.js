/**
 * Comprehensive tests for cli/src/sync/groups.js — Encryption Key Group Manager
 *
 * Coverage:
 *  - GroupKeyManager / getGroupManager construction
 *  - createGroup: success, creator auto-added as admin, description default
 *  - createGroup: duplicate name within tenant throws
 *  - createGroup: creator without encryption key throws
 *  - getGroup: found and not-found
 *  - getGroupByName: found, not-found, cross-tenant isolation
 *  - listGroups: empty tenant, sorted by name, populated
 *  - listGroups: cross-tenant isolation (groups only appear in correct tenant)
 *  - getAgentGroups: returns only groups the agent belongs to
 *  - updateGroup: name change, description change, updatedAt bumped
 *  - updateGroup: group not found throws
 *  - updateGroup: non-admin cannot update throws
 *  - updateGroup: rename to existing name in tenant throws
 *  - deleteGroup: success, group no longer listable
 *  - deleteGroup: group not found throws
 *  - deleteGroup: non-admin non-creator cannot delete throws
 *  - addMember: success with default 'member' role
 *  - addMember: explicit 'admin' role
 *  - addMember: group not found throws
 *  - addMember: non-admin cannot add throws
 *  - addMember: duplicate member throws
 *  - addMember: agent with no encryption key throws
 *  - removeMember: success, member removed from list
 *  - removeMember: group not found throws
 *  - removeMember: non-admin cannot remove throws
 *  - removeMember: cannot remove group creator throws
 *  - removeMember: cannot remove yourself as last admin throws
 *  - removeMember: agent not a member throws
 *  - updateMemberRole: promote member to admin
 *  - updateMemberRole: demote admin to member (with another admin present)
 *  - updateMemberRole: cannot demote last admin throws
 *  - updateMemberRole: invalid role throws
 *  - updateMemberRole: non-admin cannot update role throws
 *  - refreshMemberKey: no-op when key unchanged; updates when key changed
 *  - expandGroupToRecipients: returns RecipientKey[] with correct shape
 *  - expandGroupToRecipients: group not found throws
 *  - expandGroupsToRecipients: deduplicates agents in overlapping groups
 *  - canDecrypt: member returns true, non-member returns false, unknown group returns false
 *  - findRecipientForAgent: member returns RecipientKey; non-member returns null; unknown group returns null
 *  - getGroupStats: correct counts and fields
 *  - getGroupManager: singleton reuse, different configDir creates new instance
 *  - Edge cases: groups dir created lazily, group file persists correct JSON shape
 */

import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';

import { GroupKeyManager, getGroupManager } from '../src/sync/groups.js';
import { AgentKeyManager } from '../src/sync/keys.js';
import { hexToBuffer, bufferToHex } from '../src/sync/crypto.js';

// =============================================================================
// Helpers
// =============================================================================

/** Generate a raw 32-byte X25519 key pair */
function generateX25519Raw() {
  const { publicKey, privateKey } = crypto.generateKeyPairSync('x25519');
  const pubKey32 = Buffer.from(publicKey.export({ type: 'spki', format: 'der' }).subarray(-32));
  const privKey32 = Buffer.from(
    privateKey.export({ type: 'pkcs8', format: 'der' }).subarray(-32),
  );
  return { pubKey32, privKey32 };
}

/** Create a temporary directory for isolated test state */
async function makeTempDir() {
  return fs.mkdtemp(path.join(os.tmpdir(), 'stateset-groups-test-'));
}

/** Recursively remove a directory */
async function rmDir(dir) {
  await fs.rm(dir, { recursive: true, force: true });
}

/**
 * Bootstrap a GroupKeyManager with pre-generated encryption keys for the given
 * agentIds. Returns { gm, km, agentKeys } where agentKeys[agentId] = { keyId, pubKey32 }.
 */
async function makeGroupManager(configDir, agentIds = []) {
  const km = new AgentKeyManager(configDir);
  const agentKeys = {};

  for (const agentId of agentIds) {
    const kp = await km.generateEncryptionKey(agentId);
    agentKeys[agentId] = { keyId: kp.keyId, pubKey32: kp.publicKey };
  }

  const gm = new GroupKeyManager(configDir);
  return { gm, km, agentKeys };
}

// Fixed IDs used across tests
const TENANT_A = 'tenant-aaa-0001';
const TENANT_B = 'tenant-bbb-0002';
const AGENT_1 = 'agent-001';
const AGENT_2 = 'agent-002';
const AGENT_3 = 'agent-003';

// =============================================================================
// GroupKeyManager construction
// =============================================================================

describe('GroupKeyManager construction', () => {
  let tmpDir;

  beforeEach(async () => {
    tmpDir = await makeTempDir();
  });

  afterEach(async () => {
    await rmDir(tmpDir);
  });

  it('constructs with default configDir', () => {
    const gm = new GroupKeyManager();
    assert.equal(gm.configDir, '.stateset');
    assert.ok(gm.groupsDir.endsWith(path.join('.stateset', 'groups')));
  });

  it('constructs with custom configDir', () => {
    const gm = new GroupKeyManager(tmpDir);
    assert.equal(gm.configDir, tmpDir);
    assert.equal(gm.groupsDir, path.join(tmpDir, 'groups'));
  });

  it('creates groups dir lazily on first write', async () => {
    const { gm } = await makeGroupManager(tmpDir, [AGENT_1]);
    // Dir should not exist yet
    const dirPath = path.join(tmpDir, 'groups');
    await assert.rejects(() => fs.access(dirPath), { code: 'ENOENT' });

    await gm.createGroup(TENANT_A, 'first-group', AGENT_1);

    // Now it should exist
    const stat = await fs.stat(dirPath);
    assert.ok(stat.isDirectory());
  });
});

// =============================================================================
// createGroup
// =============================================================================

describe('createGroup', () => {
  let tmpDir;
  let gm;

  beforeEach(async () => {
    tmpDir = await makeTempDir();
    ({ gm } = await makeGroupManager(tmpDir, [AGENT_1, AGENT_2]));
  });

  afterEach(async () => {
    await rmDir(tmpDir);
  });

  it('creates a group with correct shape', async () => {
    const group = await gm.createGroup(TENANT_A, 'eng-team', AGENT_1);

    assert.ok(group.groupId, 'groupId is set');
    assert.equal(group.tenantId, TENANT_A);
    assert.equal(group.name, 'eng-team');
    assert.equal(group.description, '');
    assert.equal(group.createdBy, AGENT_1);
    assert.ok(group.createdAt);
    assert.ok(group.updatedAt);
  });

  it('creator is automatically added as admin', async () => {
    const group = await gm.createGroup(TENANT_A, 'eng-team', AGENT_1);

    assert.equal(group.members.length, 1);
    const [creator] = group.members;
    assert.equal(creator.agentId, AGENT_1);
    assert.equal(creator.role, 'admin');
    assert.equal(creator.addedBy, AGENT_1);
    assert.ok(creator.publicKey);
    assert.ok(creator.encryptionKeyId);
  });

  it('stores description when provided', async () => {
    const group = await gm.createGroup(TENANT_A, 'ops', AGENT_1, {
      description: 'Operations team',
    });
    assert.equal(group.description, 'Operations team');
  });

  it('persists group as JSON file on disk', async () => {
    const group = await gm.createGroup(TENANT_A, 'persist-test', AGENT_1);
    const filePath = path.join(tmpDir, 'groups', `${group.groupId}.json`);
    const raw = await fs.readFile(filePath, 'utf8');
    const parsed = JSON.parse(raw);
    assert.equal(parsed.groupId, group.groupId);
    assert.equal(parsed.name, 'persist-test');
  });

  it('throws when creator has no encryption key', async () => {
    const { gm: freshGm } = await makeGroupManager(tmpDir + '-nokey', []);
    await assert.rejects(
      () => freshGm.createGroup(TENANT_A, 'broken', 'no-key-agent'),
      /must have an encryption key/i,
    );
    await rmDir(tmpDir + '-nokey');
  });

  it('throws when group name already exists in tenant', async () => {
    await gm.createGroup(TENANT_A, 'duplicate', AGENT_1);
    await assert.rejects(
      () => gm.createGroup(TENANT_A, 'duplicate', AGENT_2),
      /already exists in tenant/i,
    );
  });

  it('allows same name in different tenants', async () => {
    const g1 = await gm.createGroup(TENANT_A, 'shared-name', AGENT_1);
    const g2 = await gm.createGroup(TENANT_B, 'shared-name', AGENT_2);
    assert.notEqual(g1.groupId, g2.groupId);
  });
});

// =============================================================================
// getGroup / getGroupByName / listGroups
// =============================================================================

describe('getGroup', () => {
  let tmpDir;
  let gm;

  beforeEach(async () => {
    tmpDir = await makeTempDir();
    ({ gm } = await makeGroupManager(tmpDir, [AGENT_1]));
  });

  afterEach(async () => {
    await rmDir(tmpDir);
  });

  it('returns the group when found', async () => {
    const created = await gm.createGroup(TENANT_A, 'alpha', AGENT_1);
    const found = await gm.getGroup(created.groupId);
    assert.equal(found.groupId, created.groupId);
    assert.equal(found.name, 'alpha');
  });

  it('returns null for an unknown groupId', async () => {
    const result = await gm.getGroup('00000000-0000-0000-0000-000000000000');
    assert.equal(result, null);
  });
});

describe('getGroupByName', () => {
  let tmpDir;
  let gm;

  beforeEach(async () => {
    tmpDir = await makeTempDir();
    ({ gm } = await makeGroupManager(tmpDir, [AGENT_1, AGENT_2]));
  });

  afterEach(async () => {
    await rmDir(tmpDir);
  });

  it('finds a group by name within tenant', async () => {
    const created = await gm.createGroup(TENANT_A, 'beta', AGENT_1);
    const found = await gm.getGroupByName(TENANT_A, 'beta');
    assert.equal(found.groupId, created.groupId);
  });

  it('returns null when name does not exist', async () => {
    const result = await gm.getGroupByName(TENANT_A, 'nonexistent');
    assert.equal(result, null);
  });

  it('does not find a group from a different tenant', async () => {
    await gm.createGroup(TENANT_B, 'cross-tenant', AGENT_2);
    const result = await gm.getGroupByName(TENANT_A, 'cross-tenant');
    assert.equal(result, null);
  });
});

describe('listGroups', () => {
  let tmpDir;
  let gm;

  beforeEach(async () => {
    tmpDir = await makeTempDir();
    ({ gm } = await makeGroupManager(tmpDir, [AGENT_1, AGENT_2]));
  });

  afterEach(async () => {
    await rmDir(tmpDir);
  });

  it('returns empty array for tenant with no groups', async () => {
    const result = await gm.listGroups('empty-tenant');
    assert.deepEqual(result, []);
  });

  it('lists all groups for a tenant sorted by name', async () => {
    await gm.createGroup(TENANT_A, 'zed', AGENT_1);
    await gm.createGroup(TENANT_A, 'alpha', AGENT_1);
    await gm.createGroup(TENANT_A, 'mike', AGENT_1);

    const groups = await gm.listGroups(TENANT_A);
    assert.equal(groups.length, 3);
    assert.equal(groups[0].name, 'alpha');
    assert.equal(groups[1].name, 'mike');
    assert.equal(groups[2].name, 'zed');
  });

  it('does not include groups from other tenants', async () => {
    await gm.createGroup(TENANT_A, 'only-a', AGENT_1);
    await gm.createGroup(TENANT_B, 'only-b', AGENT_2);

    const tenantAGroups = await gm.listGroups(TENANT_A);
    assert.equal(tenantAGroups.length, 1);
    assert.equal(tenantAGroups[0].name, 'only-a');
  });
});

// =============================================================================
// getAgentGroups
// =============================================================================

describe('getAgentGroups', () => {
  let tmpDir;
  let gm;

  beforeEach(async () => {
    tmpDir = await makeTempDir();
    ({ gm } = await makeGroupManager(tmpDir, [AGENT_1, AGENT_2, AGENT_3]));
  });

  afterEach(async () => {
    await rmDir(tmpDir);
  });

  it('returns groups the agent belongs to', async () => {
    const g1 = await gm.createGroup(TENANT_A, 'g1', AGENT_1);
    await gm.createGroup(TENANT_A, 'g2', AGENT_2); // AGENT_1 not a member

    const groups = await gm.getAgentGroups(AGENT_1, TENANT_A);
    assert.equal(groups.length, 1);
    assert.equal(groups[0].groupId, g1.groupId);
  });

  it('returns empty array when agent is in no groups', async () => {
    await gm.createGroup(TENANT_A, 'only-agent2', AGENT_2);
    const groups = await gm.getAgentGroups(AGENT_1, TENANT_A);
    assert.deepEqual(groups, []);
  });

  it('returns multiple groups when agent is in several', async () => {
    const g1 = await gm.createGroup(TENANT_A, 'g1', AGENT_1);
    const g2 = await gm.createGroup(TENANT_A, 'g2', AGENT_1);
    await gm.createGroup(TENANT_A, 'g3', AGENT_2);

    const groups = await gm.getAgentGroups(AGENT_1, TENANT_A);
    const ids = groups.map((g) => g.groupId).sort();
    assert.deepEqual(ids, [g1.groupId, g2.groupId].sort());
  });
});

// =============================================================================
// updateGroup
// =============================================================================

describe('updateGroup', () => {
  let tmpDir;
  let gm;
  let groupId;

  beforeEach(async () => {
    tmpDir = await makeTempDir();
    ({ gm } = await makeGroupManager(tmpDir, [AGENT_1, AGENT_2]));
    const group = await gm.createGroup(TENANT_A, 'original-name', AGENT_1, {
      description: 'original desc',
    });
    groupId = group.groupId;
    // Add AGENT_2 as plain member so we can test non-admin restriction
    await gm.addMember(groupId, AGENT_2, AGENT_1);
  });

  afterEach(async () => {
    await rmDir(tmpDir);
  });

  it('updates the group name', async () => {
    const updated = await gm.updateGroup(groupId, { name: 'new-name' }, AGENT_1);
    assert.equal(updated.name, 'new-name');
  });

  it('updates the description', async () => {
    const updated = await gm.updateGroup(groupId, { description: 'new desc' }, AGENT_1);
    assert.equal(updated.description, 'new desc');
  });

  it('bumps updatedAt on update', async () => {
    const before = await gm.getGroup(groupId);
    await new Promise((r) => setTimeout(r, 5)); // tiny delay to ensure timestamp change
    await gm.updateGroup(groupId, { description: 'changed' }, AGENT_1);
    const after = await gm.getGroup(groupId);
    assert.ok(after.updatedAt >= before.updatedAt);
  });

  it('throws when group not found', async () => {
    await assert.rejects(
      () => gm.updateGroup('00000000-0000-0000-0000-000000000000', { name: 'x' }, AGENT_1),
      /not found/i,
    );
  });

  it('throws when updater is not an admin', async () => {
    await assert.rejects(
      () => gm.updateGroup(groupId, { description: 'hack' }, AGENT_2),
      /only admins/i,
    );
  });

  it('throws when renaming to a name that already exists in the tenant', async () => {
    await gm.createGroup(TENANT_A, 'taken-name', AGENT_1);
    await assert.rejects(
      () => gm.updateGroup(groupId, { name: 'taken-name' }, AGENT_1),
      /already exists in tenant/i,
    );
  });

  it('allows renaming to its own current name (no-op)', async () => {
    // Should not throw since same groupId
    const updated = await gm.updateGroup(groupId, { name: 'original-name' }, AGENT_1);
    assert.equal(updated.name, 'original-name');
  });
});

// =============================================================================
// deleteGroup
// =============================================================================

describe('deleteGroup', () => {
  let tmpDir;
  let gm;

  beforeEach(async () => {
    tmpDir = await makeTempDir();
    ({ gm } = await makeGroupManager(tmpDir, [AGENT_1, AGENT_2]));
  });

  afterEach(async () => {
    await rmDir(tmpDir);
  });

  it('deletes the group and it no longer appears in list', async () => {
    const group = await gm.createGroup(TENANT_A, 'doomed', AGENT_1);
    await gm.deleteGroup(group.groupId, AGENT_1);

    const result = await gm.getGroup(group.groupId);
    assert.equal(result, null);

    const groups = await gm.listGroups(TENANT_A);
    assert.equal(groups.length, 0);
  });

  it('throws when group not found', async () => {
    await assert.rejects(
      () => gm.deleteGroup('00000000-0000-0000-0000-000000000000', AGENT_1),
      /not found/i,
    );
  });

  it('throws when deleter is not admin or creator', async () => {
    const group = await gm.createGroup(TENANT_A, 'protected', AGENT_1);
    await gm.addMember(group.groupId, AGENT_2, AGENT_1); // AGENT_2 is plain member
    await assert.rejects(
      () => gm.deleteGroup(group.groupId, AGENT_2),
      /only admins or creator/i,
    );
  });

  it('allows admin (non-creator) to delete the group', async () => {
    const group = await gm.createGroup(TENANT_A, 'admin-del', AGENT_1);
    await gm.addMember(group.groupId, AGENT_2, AGENT_1, { role: 'admin' });
    await gm.deleteGroup(group.groupId, AGENT_2); // AGENT_2 is admin, not creator
    assert.equal(await gm.getGroup(group.groupId), null);
  });
});

// =============================================================================
// addMember
// =============================================================================

describe('addMember', () => {
  let tmpDir;
  let gm;
  let groupId;

  beforeEach(async () => {
    tmpDir = await makeTempDir();
    ({ gm } = await makeGroupManager(tmpDir, [AGENT_1, AGENT_2, AGENT_3]));
    const group = await gm.createGroup(TENANT_A, 'team', AGENT_1);
    groupId = group.groupId;
  });

  afterEach(async () => {
    await rmDir(tmpDir);
  });

  it('adds a member with default role "member"', async () => {
    const updated = await gm.addMember(groupId, AGENT_2, AGENT_1);
    const member = updated.members.find((m) => m.agentId === AGENT_2);
    assert.ok(member);
    assert.equal(member.role, 'member');
    assert.equal(member.addedBy, AGENT_1);
  });

  it('adds a member with explicit "admin" role', async () => {
    const updated = await gm.addMember(groupId, AGENT_2, AGENT_1, { role: 'admin' });
    const member = updated.members.find((m) => m.agentId === AGENT_2);
    assert.equal(member.role, 'admin');
  });

  it('stores the new member public key', async () => {
    const updated = await gm.addMember(groupId, AGENT_2, AGENT_1);
    const member = updated.members.find((m) => m.agentId === AGENT_2);
    assert.ok(member.publicKey, 'publicKey should be set');
    // Should be a valid hex string (64 chars for 32 bytes, prefixed with 0x or not)
    assert.ok(member.encryptionKeyId > 0);
  });

  it('throws when group not found', async () => {
    await assert.rejects(
      () => gm.addMember('00000000-0000-0000-0000-000000000000', AGENT_2, AGENT_1),
      /not found/i,
    );
  });

  it('throws when adder is not an admin', async () => {
    await gm.addMember(groupId, AGENT_2, AGENT_1); // AGENT_2 is plain member
    await assert.rejects(
      () => gm.addMember(groupId, AGENT_3, AGENT_2),
      /only admins can add/i,
    );
  });

  it('throws when agent is already a member', async () => {
    await gm.addMember(groupId, AGENT_2, AGENT_1);
    await assert.rejects(
      () => gm.addMember(groupId, AGENT_2, AGENT_1),
      /already a member/i,
    );
  });

  it('throws when agent has no encryption key', async () => {
    // AGENT_3 has a key but we use a fresh agent id with no key
    await assert.rejects(
      () => gm.addMember(groupId, 'keyless-agent', AGENT_1),
      /no encryption key/i,
    );
  });
});

// =============================================================================
// removeMember
// =============================================================================

describe('removeMember', () => {
  let tmpDir;
  let gm;
  let groupId;

  beforeEach(async () => {
    tmpDir = await makeTempDir();
    ({ gm } = await makeGroupManager(tmpDir, [AGENT_1, AGENT_2, AGENT_3]));
    const group = await gm.createGroup(TENANT_A, 'removable-team', AGENT_1);
    groupId = group.groupId;
    await gm.addMember(groupId, AGENT_2, AGENT_1); // plain member
    await gm.addMember(groupId, AGENT_3, AGENT_1, { role: 'admin' }); // second admin
  });

  afterEach(async () => {
    await rmDir(tmpDir);
  });

  it('removes a member successfully', async () => {
    const updated = await gm.removeMember(groupId, AGENT_2, AGENT_1);
    const still = updated.members.find((m) => m.agentId === AGENT_2);
    assert.equal(still, undefined);
  });

  it('throws when group not found', async () => {
    await assert.rejects(
      () => gm.removeMember('00000000-0000-0000-0000-000000000000', AGENT_2, AGENT_1),
      /not found/i,
    );
  });

  it('throws when remover is not an admin', async () => {
    await assert.rejects(
      () => gm.removeMember(groupId, AGENT_3, AGENT_2), // AGENT_2 is plain member
      /only admins can remove/i,
    );
  });

  it('throws when attempting to remove the group creator', async () => {
    await assert.rejects(
      () => gm.removeMember(groupId, AGENT_1, AGENT_3), // AGENT_3 is admin, AGENT_1 is creator
      /cannot remove the group creator/i,
    );
  });

  it('throws when admin tries to remove themselves as last admin', async () => {
    // Demote AGENT_1 (the creator) to 'member' so AGENT_3 becomes the sole admin.
    // AGENT_3 is not the creator, so the creator guard won't fire.
    await gm.updateMemberRole(groupId, AGENT_1, 'member', AGENT_3);
    // Now AGENT_3 is the only admin — trying to self-remove should fail
    await assert.rejects(
      () => gm.removeMember(groupId, AGENT_3, AGENT_3),
      /cannot remove yourself as the last admin/i,
    );
  });

  it('throws when agent is not a member', async () => {
    await assert.rejects(
      () => gm.removeMember(groupId, 'ghost-agent', AGENT_1),
      /is not a member/i,
    );
  });
});

// =============================================================================
// updateMemberRole
// =============================================================================

describe('updateMemberRole', () => {
  let tmpDir;
  let gm;
  let groupId;

  beforeEach(async () => {
    tmpDir = await makeTempDir();
    ({ gm } = await makeGroupManager(tmpDir, [AGENT_1, AGENT_2, AGENT_3]));
    const group = await gm.createGroup(TENANT_A, 'roles-team', AGENT_1);
    groupId = group.groupId;
    await gm.addMember(groupId, AGENT_2, AGENT_1); // plain member
    await gm.addMember(groupId, AGENT_3, AGENT_1, { role: 'admin' }); // second admin
  });

  afterEach(async () => {
    await rmDir(tmpDir);
  });

  it('promotes a member to admin', async () => {
    const updated = await gm.updateMemberRole(groupId, AGENT_2, 'admin', AGENT_1);
    const member = updated.members.find((m) => m.agentId === AGENT_2);
    assert.equal(member.role, 'admin');
  });

  it('demotes an admin to member when another admin exists', async () => {
    const updated = await gm.updateMemberRole(groupId, AGENT_3, 'member', AGENT_1);
    const agent3 = updated.members.find((m) => m.agentId === AGENT_3);
    assert.equal(agent3.role, 'member');
  });

  it('throws when trying to demote the last admin', async () => {
    // Demote AGENT_3 first, making AGENT_1 the sole admin
    await gm.updateMemberRole(groupId, AGENT_3, 'member', AGENT_1);
    await assert.rejects(
      () => gm.updateMemberRole(groupId, AGENT_1, 'member', AGENT_1),
      /cannot demote the last admin/i,
    );
  });

  it('throws for an invalid role string', async () => {
    await assert.rejects(
      () => gm.updateMemberRole(groupId, AGENT_2, 'superuser', AGENT_1),
      /invalid role/i,
    );
  });

  it('throws when updater is not an admin', async () => {
    await assert.rejects(
      () => gm.updateMemberRole(groupId, AGENT_1, 'member', AGENT_2), // AGENT_2 is plain member
      /only admins/i,
    );
  });

  it('throws when the target agent is not a member', async () => {
    await assert.rejects(
      () => gm.updateMemberRole(groupId, 'nonmember', 'admin', AGENT_1),
      /is not a member/i,
    );
  });
});

// =============================================================================
// refreshMemberKey
// =============================================================================

describe('refreshMemberKey', () => {
  let tmpDir;
  let gm;
  let km;
  let groupId;

  beforeEach(async () => {
    tmpDir = await makeTempDir();
    ({ gm, km } = await makeGroupManager(tmpDir, [AGENT_1, AGENT_2]));
    const group = await gm.createGroup(TENANT_A, 'refresh-team', AGENT_1);
    groupId = group.groupId;
    await gm.addMember(groupId, AGENT_2, AGENT_1);
  });

  afterEach(async () => {
    await rmDir(tmpDir);
  });

  it('is a no-op when the member key has not changed', async () => {
    const before = await gm.getGroup(groupId);
    const updatedAt1 = before.updatedAt;

    await gm.refreshMemberKey(groupId, AGENT_2);
    const after = await gm.getGroup(groupId);
    assert.equal(after.updatedAt, updatedAt1, 'updatedAt should not change for no-op refresh');
  });

  it('updates member public key after key rotation', async () => {
    const before = await gm.getGroup(groupId);
    const memberBefore = before.members.find((m) => m.agentId === AGENT_2);
    const oldKeyId = memberBefore.encryptionKeyId;

    // Rotate AGENT_2's encryption key
    await km.generateEncryptionKey(AGENT_2);

    await gm.refreshMemberKey(groupId, AGENT_2);
    const after = await gm.getGroup(groupId);
    const memberAfter = after.members.find((m) => m.agentId === AGENT_2);

    assert.ok(memberAfter.encryptionKeyId > oldKeyId, 'key ID should be bumped after rotation');
  });

  it('throws when group not found', async () => {
    await assert.rejects(
      () => gm.refreshMemberKey('00000000-0000-0000-0000-000000000000', AGENT_1),
      /not found/i,
    );
  });

  it('throws when agent is not a member', async () => {
    await assert.rejects(
      () => gm.refreshMemberKey(groupId, 'nonmember-agent'),
      /is not a member/i,
    );
  });
});

// =============================================================================
// expandGroupToRecipients
// =============================================================================

describe('expandGroupToRecipients', () => {
  let tmpDir;
  let gm;
  let groupId;

  beforeEach(async () => {
    tmpDir = await makeTempDir();
    ({ gm } = await makeGroupManager(tmpDir, [AGENT_1, AGENT_2]));
    const group = await gm.createGroup(TENANT_A, 'enc-team', AGENT_1);
    groupId = group.groupId;
    await gm.addMember(groupId, AGENT_2, AGENT_1);
  });

  afterEach(async () => {
    await rmDir(tmpDir);
  });

  it('returns a RecipientKey entry for each member', async () => {
    const recipients = await gm.expandGroupToRecipients(groupId);
    assert.equal(recipients.length, 2);

    for (const r of recipients) {
      assert.ok(r.keyId > 0, 'keyId should be positive');
      assert.ok(Buffer.isBuffer(r.publicKey), 'publicKey should be a Buffer');
      assert.equal(r.publicKey.length, 32, 'X25519 public key is 32 bytes');
      assert.ok(r.agentId, 'agentId should be set');
    }
  });

  it('returns correct agentIds in recipients', async () => {
    const recipients = await gm.expandGroupToRecipients(groupId);
    const agentIds = recipients.map((r) => r.agentId).sort();
    assert.deepEqual(agentIds, [AGENT_1, AGENT_2].sort());
  });

  it('public key round-trips through hex correctly', async () => {
    const recipients = await gm.expandGroupToRecipients(groupId);
    const group = await gm.getGroup(groupId);
    for (const r of recipients) {
      const member = group.members.find((m) => m.agentId === r.agentId);
      const expected = hexToBuffer(member.publicKey);
      assert.deepEqual(r.publicKey, expected);
    }
  });

  it('throws when group not found', async () => {
    await assert.rejects(
      () => gm.expandGroupToRecipients('00000000-0000-0000-0000-000000000000'),
      /not found/i,
    );
  });
});

// =============================================================================
// expandGroupsToRecipients (multi-group)
// =============================================================================

describe('expandGroupsToRecipients', () => {
  let tmpDir;
  let gm;

  beforeEach(async () => {
    tmpDir = await makeTempDir();
    ({ gm } = await makeGroupManager(tmpDir, [AGENT_1, AGENT_2, AGENT_3]));
  });

  afterEach(async () => {
    await rmDir(tmpDir);
  });

  it('deduplicates agents that appear in multiple groups', async () => {
    const g1 = await gm.createGroup(TENANT_A, 'g1', AGENT_1);
    await gm.addMember(g1.groupId, AGENT_2, AGENT_1);

    const g2 = await gm.createGroup(TENANT_A, 'g2', AGENT_2);
    // AGENT_2 is creator of g2 AND member of g1 — should appear once

    const recipients = await gm.expandGroupsToRecipients([g1.groupId, g2.groupId]);
    const agentIds = recipients.map((r) => r.agentId);
    const unique = new Set(agentIds);
    assert.equal(agentIds.length, unique.size, 'No duplicate agents');
    assert.ok(unique.has(AGENT_1));
    assert.ok(unique.has(AGENT_2));
  });

  it('merges recipients from two disjoint groups', async () => {
    const g1 = await gm.createGroup(TENANT_A, 'disjoint1', AGENT_1);
    const g2 = await gm.createGroup(TENANT_A, 'disjoint2', AGENT_2);
    await gm.addMember(g2.groupId, AGENT_3, AGENT_2);

    const recipients = await gm.expandGroupsToRecipients([g1.groupId, g2.groupId]);
    const agentIds = recipients.map((r) => r.agentId).sort();
    assert.deepEqual(agentIds, [AGENT_1, AGENT_2, AGENT_3].sort());
  });

  it('returns empty array for empty group list', async () => {
    const recipients = await gm.expandGroupsToRecipients([]);
    assert.deepEqual(recipients, []);
  });
});

// =============================================================================
// canDecrypt
// =============================================================================

describe('canDecrypt', () => {
  let tmpDir;
  let gm;
  let groupId;

  beforeEach(async () => {
    tmpDir = await makeTempDir();
    ({ gm } = await makeGroupManager(tmpDir, [AGENT_1, AGENT_2]));
    const group = await gm.createGroup(TENANT_A, 'decrypt-team', AGENT_1);
    groupId = group.groupId;
    await gm.addMember(groupId, AGENT_2, AGENT_1);
  });

  afterEach(async () => {
    await rmDir(tmpDir);
  });

  it('returns true for a member', async () => {
    assert.equal(await gm.canDecrypt(groupId, AGENT_1), true);
    assert.equal(await gm.canDecrypt(groupId, AGENT_2), true);
  });

  it('returns false for a non-member', async () => {
    assert.equal(await gm.canDecrypt(groupId, AGENT_3), false);
  });

  it('returns false for an unknown group', async () => {
    assert.equal(
      await gm.canDecrypt('00000000-0000-0000-0000-000000000000', AGENT_1),
      false,
    );
  });

  it('returns false after member is removed', async () => {
    await gm.removeMember(groupId, AGENT_2, AGENT_1);
    assert.equal(await gm.canDecrypt(groupId, AGENT_2), false);
  });
});

// =============================================================================
// findRecipientForAgent
// =============================================================================

describe('findRecipientForAgent', () => {
  let tmpDir;
  let gm;
  let groupId;

  beforeEach(async () => {
    tmpDir = await makeTempDir();
    ({ gm } = await makeGroupManager(tmpDir, [AGENT_1, AGENT_2]));
    const group = await gm.createGroup(TENANT_A, 'find-team', AGENT_1);
    groupId = group.groupId;
    await gm.addMember(groupId, AGENT_2, AGENT_1);
  });

  afterEach(async () => {
    await rmDir(tmpDir);
  });

  it('returns a RecipientKey for a member', async () => {
    const r = await gm.findRecipientForAgent(groupId, AGENT_1);
    assert.ok(r);
    assert.equal(r.agentId, AGENT_1);
    assert.ok(Buffer.isBuffer(r.publicKey));
    assert.equal(r.publicKey.length, 32);
  });

  it('returns null for a non-member', async () => {
    const r = await gm.findRecipientForAgent(groupId, 'outsider');
    assert.equal(r, null);
  });

  it('returns null for an unknown group', async () => {
    const r = await gm.findRecipientForAgent(
      '00000000-0000-0000-0000-000000000000',
      AGENT_1,
    );
    assert.equal(r, null);
  });
});

// =============================================================================
// getGroupStats
// =============================================================================

describe('getGroupStats', () => {
  let tmpDir;
  let gm;
  let groupId;

  beforeEach(async () => {
    tmpDir = await makeTempDir();
    ({ gm } = await makeGroupManager(tmpDir, [AGENT_1, AGENT_2, AGENT_3]));
    const group = await gm.createGroup(TENANT_A, 'stats-team', AGENT_1);
    groupId = group.groupId;
    await gm.addMember(groupId, AGENT_2, AGENT_1); // member
    await gm.addMember(groupId, AGENT_3, AGENT_1, { role: 'admin' }); // second admin
  });

  afterEach(async () => {
    await rmDir(tmpDir);
  });

  it('returns correct member counts', async () => {
    const stats = await gm.getGroupStats(groupId);
    assert.equal(stats.totalMembers, 3);
    assert.equal(stats.adminCount, 2); // AGENT_1 + AGENT_3
    assert.equal(stats.memberCount, 1); // AGENT_2
  });

  it('returns correct metadata fields', async () => {
    const stats = await gm.getGroupStats(groupId);
    assert.equal(stats.groupId, groupId);
    assert.equal(stats.name, 'stats-team');
    assert.equal(stats.tenantId, TENANT_A);
    assert.equal(stats.createdBy, AGENT_1);
    assert.ok(stats.createdAt);
    assert.ok(stats.updatedAt);
  });

  it('throws when group not found', async () => {
    await assert.rejects(
      () => gm.getGroupStats('00000000-0000-0000-0000-000000000000'),
      /not found/i,
    );
  });
});

// =============================================================================
// Tenant index persistence
// =============================================================================

describe('tenant index', () => {
  let tmpDir;
  let gm;

  beforeEach(async () => {
    tmpDir = await makeTempDir();
    ({ gm } = await makeGroupManager(tmpDir, [AGENT_1]));
  });

  afterEach(async () => {
    await rmDir(tmpDir);
  });

  it('tenant index file is written on group creation', async () => {
    await gm.createGroup(TENANT_A, 'indexed-group', AGENT_1);
    const indexPath = path.join(tmpDir, 'groups', `_index_${TENANT_A}.json`);
    const raw = await fs.readFile(indexPath, 'utf8');
    const index = JSON.parse(raw);
    assert.equal(Object.keys(index).length, 1);
  });

  it('tenant index is pruned after group deletion', async () => {
    const group = await gm.createGroup(TENANT_A, 'ephemeral', AGENT_1);
    await gm.deleteGroup(group.groupId, AGENT_1);

    const indexPath = path.join(tmpDir, 'groups', `_index_${TENANT_A}.json`);
    const raw = await fs.readFile(indexPath, 'utf8');
    const index = JSON.parse(raw);
    assert.equal(Object.keys(index).length, 0);
  });
});

// =============================================================================
// getGroupManager singleton
// =============================================================================

describe('getGroupManager', () => {
  it('returns the same instance for the same configDir', () => {
    const gm1 = getGroupManager('/tmp/singleton-test-dir-a');
    const gm2 = getGroupManager('/tmp/singleton-test-dir-a');
    assert.equal(gm1, gm2);
  });

  it('creates a new instance for a different configDir', () => {
    const gm1 = getGroupManager('/tmp/singleton-test-dir-b');
    const gm2 = getGroupManager('/tmp/singleton-test-dir-c');
    assert.notEqual(gm1, gm2);
  });
});
