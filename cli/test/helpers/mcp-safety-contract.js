/** The same safety assertions run on both transports and both protocol eras. */
import assert from 'node:assert/strict';
import { validateToolResultContract } from '../../src/mcp/tool-result-contract.js';

/** Every advertised required-input tool must refuse empty arguments uniformly. */
export async function assertRequiredArgumentsContract(tools, call) {
  const required = tools.filter((tool) => tool.inputSchema?.required?.length > 0);
  assert.ok(required.length > 0, 'catalog must exercise required arguments');
  for (const tool of required) {
    const response = await call(tool.name, {});
    assert.ok(response?.structuredContent, `${tool.name}: missing structured refusal`);
    assert.deepEqual(validateToolResultContract(response.structuredContent), [], tool.name);
    assert.equal(response.isError, true, `${tool.name}: empty required arguments accepted`);
    assert.equal(response.structuredContent.ok, false, tool.name);
    assert.equal(response.structuredContent.preview, false, tool.name);
    assert.equal(response.structuredContent.error.code, 'INVALID_INPUT', tool.name);
    assert.equal(response.structuredContent.error.retryable, false, tool.name);
  }
  return required.length;
}

export async function assertPreviewSafetyContract(call, tools = []) {
  const invoke = async (name, args = {}) => {
    const response = await call(name, args);
    assert.ok(response?.structuredContent, `${name} must carry a structured result`);
    assert.deepEqual(validateToolResultContract(response.structuredContent), [], name);
    return response;
  };
  const before = JSON.parse((await invoke('list_customers')).content[0].text);
  for (const [name, args, code] of [
    ['create_payment', { orderId: 'unused', amount: '0.00' }, 'INVALID_INPUT'],
    ['create_payment', { orderId: 'unused', amount: '-0.01' }, 'INVALID_INPUT'],
    ['create_payment', { orderId: 'unused', amount: 'NaN' }, 'INVALID_INPUT'],
    ['get_payment', {}, 'INVALID_INPUT'],
    ['get_payment', { paymentId: 'not-a-uuid' }, 'VALIDATION'],
  ]) {
    const response = await invoke(name, args);
    assert.equal(response.isError, true, `${name}: ${JSON.stringify(args)}`);
    assert.equal(response.structuredContent.ok, false);
    assert.equal(response.structuredContent.preview, false);
    assert.equal(response.structuredContent.error.code, code);
    assert.equal(response.structuredContent.error.retryable, false);
  }
  const preview = await invoke('create_customer', {
    email: 'transport-preview-contract@example.test',
    firstName: 'Preview',
    lastName: 'Only',
  });
  assert.notEqual(preview.isError, true);
  assert.equal(preview.structuredContent.ok, true);
  assert.equal(preview.structuredContent.preview, true);
  assert.equal(preview.structuredContent.notice.code, 'APPLY_REQUIRED');
  const checked = tools.length ? await assertRequiredArgumentsContract(tools, call) : 0;
  const after = JSON.parse((await invoke('list_customers')).content[0].text);
  assert.deepEqual(after, before, 'preview and rejected calls must leave the store unchanged');
  return checked;
}
