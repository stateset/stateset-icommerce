/**
 * Unit tests for skills/parser.js
 */

import { describe, it } from 'node:test';
import assert from 'node:assert';
import { extractFrontmatter, parseSkillContent, parseSkillMd } from '../../src/skills/parser.js';

// ===========================================================================
// extractFrontmatter
// ===========================================================================

describe('extractFrontmatter', () => {
  it('extracts valid YAML frontmatter and body', () => {
    const content = `---
name: test-skill
description: A test skill
---
# My Skill

Some body text.`;

    const { frontmatter, body, error } = extractFrontmatter(content);
    assert.ok(frontmatter);
    assert.strictEqual(frontmatter.name, 'test-skill');
    assert.strictEqual(frontmatter.description, 'A test skill');
    assert.ok(body.includes('# My Skill'));
    assert.ok(body.includes('Some body text.'));
    assert.strictEqual(error, undefined);
  });

  it('returns error for empty content', () => {
    const { frontmatter, error } = extractFrontmatter('');
    assert.strictEqual(frontmatter, null);
    assert.ok(error.includes('Empty'));
  });

  it('returns error for null', () => {
    const { frontmatter, error } = extractFrontmatter(null);
    assert.strictEqual(frontmatter, null);
    assert.ok(error);
  });

  it('returns error for non-string', () => {
    const { frontmatter, error } = extractFrontmatter(42);
    assert.strictEqual(frontmatter, null);
    assert.ok(error);
  });

  it('returns body and error when no frontmatter', () => {
    const content = '# Just a heading\n\nSome text.';
    const { frontmatter, body, error } = extractFrontmatter(content);
    assert.strictEqual(frontmatter, null);
    assert.ok(body.includes('Just a heading'));
    assert.ok(error.includes('No YAML'));
  });

  it('handles malformed YAML gracefully', () => {
    const content = `---
invalid: [unclosed
---
Body here.`;

    const { frontmatter, body, error } = extractFrontmatter(content);
    assert.strictEqual(frontmatter, null);
    assert.ok(body.includes('Body'));
    assert.ok(error.includes('YAML parse error'));
  });

  it('handles frontmatter that is not an object', () => {
    const content = `---
just a string
---
Body.`;

    const { frontmatter, error } = extractFrontmatter(content);
    assert.strictEqual(frontmatter, null);
    assert.ok(error.includes('not an object'));
  });
});

// ===========================================================================
// parseSkillContent
// ===========================================================================

describe('parseSkillContent', () => {
  it('parses a complete skill', () => {
    const content = `---
name: commerce-orders
description: Order lifecycle management skill
---
# Order Management

## Overview
Manage orders through their lifecycle.

## Fulfillment
Ship orders with tracking.

Use \`list_orders\` to view all orders.
Use \`create_order\` to create a new order.
Use \`stateset-orders\` for the orders agent.`;

    const result = parseSkillContent(content);
    assert.ok(result);
    assert.strictEqual(result.name, 'commerce-orders');
    assert.strictEqual(result.description, 'Order lifecycle management skill');
    assert.strictEqual(result.title, 'Order Management');
    assert.ok(result.sections.includes('Overview'));
    assert.ok(result.sections.includes('Fulfillment'));
    assert.ok(result.mcpTools.includes('list_orders'));
    assert.ok(result.mcpTools.includes('create_order'));
    assert.ok(result.cliCommands.includes('stateset-orders'));
    assert.ok(result.raw);
  });

  it('returns null when name is missing', () => {
    const content = `---
description: No name
---
Body.`;

    const result = parseSkillContent(content);
    assert.strictEqual(result, null);
  });

  it('returns null when description is missing', () => {
    const content = `---
name: test
---
Body.`;

    const result = parseSkillContent(content);
    assert.strictEqual(result, null);
  });

  it('returns null when frontmatter is missing', () => {
    const content = '# Just a heading\nNo frontmatter here.';
    const result = parseSkillContent(content);
    assert.strictEqual(result, null);
  });

  it('extracts multiple MCP tools', () => {
    const content = `---
name: multi
description: Multi-tool skill
---
# Tools

Use \`list_customers\`, \`get_customer\`, \`create_customer\`, and \`update_order_status\`.`;

    const result = parseSkillContent(content);
    assert.ok(result);
    assert.ok(result.mcpTools.includes('list_customers'));
    assert.ok(result.mcpTools.includes('get_customer'));
    assert.ok(result.mcpTools.includes('create_customer'));
    assert.ok(result.mcpTools.includes('update_order_status'));
  });

  it('extracts CLI commands', () => {
    const content = `---
name: cli-skill
description: CLI commands skill
---
# CLI

Use \`stateset\` or \`stateset-checkout\` or \`stateset-inventory\`.`;

    const result = parseSkillContent(content);
    assert.ok(result);
    assert.ok(result.cliCommands.includes('stateset'));
    assert.ok(result.cliCommands.includes('stateset-checkout'));
    assert.ok(result.cliCommands.includes('stateset-inventory'));
  });

  it('deduplicates MCP tools', () => {
    const content = `---
name: dedup
description: Dedup test
---
# Test

\`list_orders\` and then \`list_orders\` again.`;

    const result = parseSkillContent(content);
    assert.ok(result);
    const count = result.mcpTools.filter((t) => t === 'list_orders').length;
    assert.strictEqual(count, 1);
  });

  it('returns sorted tools and commands', () => {
    const content = `---
name: sorted
description: Sort test
---
# Test

\`create_order\` then \`approve_return\` then \`list_customers\`.
\`stateset-returns\` and \`stateset-checkout\`.`;

    const result = parseSkillContent(content);
    assert.ok(result);
    assert.deepStrictEqual(result.mcpTools, [...result.mcpTools].sort());
    assert.deepStrictEqual(result.cliCommands, [...result.cliCommands].sort());
  });

  it('handles body with no sections', () => {
    const content = `---
name: minimal
description: Minimal skill
---
Just plain text, no headings.`;

    const result = parseSkillContent(content);
    assert.ok(result);
    assert.strictEqual(result.title, '');
    assert.deepStrictEqual(result.sections, []);
  });

  it('trims name and description', () => {
    const content = `---
name: "  spaced  "
description: "  also spaced  "
---
# Body`;

    const result = parseSkillContent(content);
    assert.ok(result);
    assert.strictEqual(result.name, 'spaced');
    assert.strictEqual(result.description, 'also spaced');
  });
});

// ===========================================================================
// parseSkillMd
// ===========================================================================

describe('parseSkillMd', () => {
  it('returns null for nonexistent file', () => {
    const result = parseSkillMd('/tmp/nonexistent-skill-file.md');
    assert.strictEqual(result, null);
  });
});
