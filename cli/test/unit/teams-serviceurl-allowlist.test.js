/**
 * Tests for isAllowedBotServiceUrl — the Bot Framework serviceUrl allowlist
 * that guards against request-forgery / token-leak (CodeQL js/request-forgery).
 */

import { describe, it, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { isAllowedBotServiceUrl } from '../../src/teams/gateway.js';

describe('isAllowedBotServiceUrl', () => {
  afterEach(() => {
    delete process.env.STATESET_TEAMS_SERVICEURL_ALLOWLIST;
  });

  it('allows documented Microsoft Bot Framework / Teams hosts', () => {
    assert.equal(isAllowedBotServiceUrl('https://smba.trafficmanager.net/amer/'), true);
    assert.equal(isAllowedBotServiceUrl('https://example.botframework.com/'), true);
    assert.equal(isAllowedBotServiceUrl('https://gov.botframework.azure.us/'), true);
  });

  it('rejects non-HTTPS serviceUrls', () => {
    assert.equal(isAllowedBotServiceUrl('http://smba.trafficmanager.net/'), false);
  });

  it('rejects hosts outside the allowlist', () => {
    assert.equal(isAllowedBotServiceUrl('https://evil.example.com/'), false);
  });

  it('rejects suffix-spoofing hosts', () => {
    // Must not be fooled by an allowed suffix appearing mid-host.
    assert.equal(isAllowedBotServiceUrl('https://nottrafficmanager.net.evil.com/'), false);
    assert.equal(isAllowedBotServiceUrl('https://botframework.com.evil.com/'), false);
  });

  it('rejects internal / SSRF targets even on https', () => {
    assert.equal(isAllowedBotServiceUrl('https://localhost/'), false);
    assert.equal(isAllowedBotServiceUrl('https://169.254.169.254/'), false);
  });

  it('rejects empty, non-string, and malformed input', () => {
    assert.equal(isAllowedBotServiceUrl(''), false);
    assert.equal(isAllowedBotServiceUrl(null), false);
    assert.equal(isAllowedBotServiceUrl(42), false);
    assert.equal(isAllowedBotServiceUrl('not a url'), false);
  });

  it('honors STATESET_TEAMS_SERVICEURL_ALLOWLIST for custom deployments', () => {
    assert.equal(isAllowedBotServiceUrl('https://bot.contoso.example/'), false);
    process.env.STATESET_TEAMS_SERVICEURL_ALLOWLIST = 'contoso.example';
    assert.equal(isAllowedBotServiceUrl('https://bot.contoso.example/'), true);
    // Internal hosts are still rejected regardless of the env allowlist.
    process.env.STATESET_TEAMS_SERVICEURL_ALLOWLIST = 'localhost';
    assert.equal(isAllowedBotServiceUrl('https://localhost/'), false);
  });
});
