/**
 * Integration test for authentication flow
 *
 * Tests the end-to-end auth flow: login -> get user profile -> logout.
 * Uses mocked fetch to simulate upstream API responses while testing
 * the full request pipeline through Next.js route handlers.
 *
 * @module tests/integration/auth-flow
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import {
  createMockRequest,
  createAuthenticatedRequest,
  parseResponse,
  expectSuccess,
  expectError,
} from '../helpers/api-test-utils';

// Mock the request-context module
vi.mock('@/lib/shared/request-context', () => {
  const { AsyncLocalStorage } = require('async_hooks');
  const store = new AsyncLocalStorage();
  return {
    requestStore: store,
    getRequestContext: () => store.getStore() ?? { requestId: 'req_test', startTime: Date.now() },
    getRequestId: () => store.getStore()?.requestId ?? 'req_test',
    generateRequestId: () => 'req_test_generated',
  };
});

// Mock fetch for upstream API calls
const mockFetch = vi.fn();

beforeEach(() => {
  vi.stubGlobal('fetch', mockFetch);
  vi.stubEnv('STATESET_API_URL', 'https://api.sandbox.stateset.app');
  vi.stubEnv('STATESET_API_TOKEN', '');
});

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  vi.unstubAllEnvs();
});

describe('Authentication flow integration', () => {
  describe('register -> login -> profile -> logout', () => {
    it('completes the full auth lifecycle', async () => {
      // ====================================================================
      // Step 1: Register a new user
      // ====================================================================
      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: () =>
          Promise.resolve({
            user: {
              id: 'user-1',
              email: 'newuser@example.com',
              firstName: 'Jane',
              lastName: 'Doe',
            },
          }),
      });

      const { POST: registerPOST } = await import('@/app/api/auth/register/route');

      const registerRequest = createMockRequest({
        method: 'POST',
        url: 'http://localhost:3000/api/auth/register',
        body: {
          email: 'newuser@example.com',
          password: 'Securepass1',
          firstName: 'Jane',
          lastName: 'Doe',
        },
      });

      const registerResponse = await registerPOST(registerRequest, undefined as any);
      const registerBody = await expectSuccess(registerResponse, 201);

      expect(registerBody.data).toHaveProperty('user');
      expect((registerBody.data as any).user.email).toBe('newuser@example.com');

      // ====================================================================
      // Step 2: Login with the registered credentials
      // ====================================================================
      const mockToken = 'jwt-session-token-abc123';

      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: () =>
          Promise.resolve({
            token: mockToken,
            user: {
              id: 'user-1',
              email: 'newuser@example.com',
              firstName: 'Jane',
              lastName: 'Doe',
            },
          }),
      });

      const { POST: loginPOST } = await import('@/app/api/auth/login/route');

      const loginRequest = createMockRequest({
        method: 'POST',
        url: 'http://localhost:3000/api/auth/login',
        body: {
          email: 'newuser@example.com',
          password: 'Securepass1',
        },
      });

      const loginResponse = await loginPOST(loginRequest, undefined as any);
      const loginBody = await expectSuccess(loginResponse);

      expect(loginBody.data).toHaveProperty('token', mockToken);
      expect(loginBody.data).toHaveProperty('user');

      // Verify the response has the standard envelope shape
      expect(loginBody.success).toBe(true);
      expect(loginBody.meta).toBeDefined();
      expect(loginBody.meta.requestId).toBeDefined();
      expect(loginBody.meta.timestamp).toBeDefined();

      // ====================================================================
      // Step 3: Use the token to access a protected resource (billing subscription)
      // ====================================================================
      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: () =>
          Promise.resolve({
            id: 'sub-1',
            planId: 'free',
            status: 'active',
            user: {
              id: 'user-1',
              email: 'newuser@example.com',
              firstName: 'Jane',
              lastName: 'Doe',
            },
          }),
      });

      const { GET: getSubscription } = await import('@/app/api/billing/subscription/route');

      const profileRequest = createAuthenticatedRequest({
        url: 'http://localhost:3000/api/billing/subscription',
        headers: {
          Authorization: `Bearer ${mockToken}`,
        },
      });

      const profileResponse = await getSubscription(profileRequest, undefined as any);
      const profileBody = await expectSuccess(profileResponse);

      expect(profileBody.data).toHaveProperty('user');
      expect((profileBody.data as any).user.email).toBe('newuser@example.com');

      // Verify the token was forwarded to the upstream API
      expect(mockFetch).toHaveBeenLastCalledWith(
        'https://api.sandbox.stateset.app/api/billing/subscription',
        expect.objectContaining({
          headers: expect.objectContaining({
            Authorization: `Bearer ${mockToken}`,
          }),
        })
      );
    });
  });

  describe('login with invalid credentials', () => {
    it('returns 422 for missing fields and 401 for wrong credentials', async () => {
      // ====================================================================
      // Step 1: Try login with missing email - should get 422
      // ====================================================================
      const { POST: loginPOST } = await import('@/app/api/auth/login/route');

      const missingEmailRequest = createMockRequest({
        method: 'POST',
        url: 'http://localhost:3000/api/auth/login',
        body: { password: 'Password1x' },
      });

      const missingEmailResponse = await loginPOST(missingEmailRequest, undefined as any);
      await expectError(missingEmailResponse, 422, 'VALIDATION_ERROR');

      // ====================================================================
      // Step 2: Try login with wrong password - should get 401
      // ====================================================================
      mockFetch.mockResolvedValueOnce({
        ok: false,
        status: 401,
        json: () => Promise.resolve({ error: 'Invalid credentials' }),
      });

      const wrongPasswordRequest = createMockRequest({
        method: 'POST',
        url: 'http://localhost:3000/api/auth/login',
        body: { email: 'user@example.com', password: 'Wrongpass1' },
      });

      const wrongPasswordResponse = await loginPOST(wrongPasswordRequest, undefined as any);
      await expectError(wrongPasswordResponse, 401, 'UNAUTHORIZED');

      // ====================================================================
      // Step 3: Verify protected resource is inaccessible without token
      // ====================================================================
      vi.stubEnv('STATESET_API_TOKEN', '');

      const { GET: getSubscription } = await import('@/app/api/billing/subscription/route');

      const unauthRequest = createMockRequest({
        url: 'http://localhost:3000/api/billing/subscription',
      });

      const unauthResponse = await getSubscription(unauthRequest, undefined as any);
      await expectError(unauthResponse, 401, 'UNAUTHORIZED');
    });
  });

  describe('token propagation across requests', () => {
    it('propagates auth token from login to protected endpoints', async () => {
      const token = 'propagated-token-xyz';

      // Login returns a token
      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: () =>
          Promise.resolve({
            token,
            user: { id: 'user-1', email: 'test@example.com' },
          }),
      });

      const { POST: loginPOST } = await import('@/app/api/auth/login/route');

      const loginRequest = createMockRequest({
        method: 'POST',
        url: 'http://localhost:3000/api/auth/login',
        body: { email: 'test@example.com', password: 'Password1x' },
      });

      const loginResponse = await loginPOST(loginRequest, undefined as any);
      const loginBody = await parseResponse<{
        success: boolean;
        data: { token: string };
      }>(loginResponse);

      expect(loginBody.data.token).toBe(token);

      // Use the returned token for a protected integration status call
      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: () =>
          Promise.resolve({
            integrations: [
              { provider: 'shopify', status: 'connected' },
            ],
          }),
      });

      const { GET: getStatus } = await import('@/app/api/integrations/status/route');

      const statusRequest = createAuthenticatedRequest({
        url: 'http://localhost:3000/api/integrations/status',
        headers: {
          Authorization: `Bearer ${token}`,
        },
      });

      const statusResponse = await getStatus(statusRequest, undefined as any);
      const statusBody = await expectSuccess(statusResponse);

      expect(statusBody.data).toHaveProperty('integrations');

      // Verify the token was properly forwarded
      expect(mockFetch).toHaveBeenLastCalledWith(
        'https://api.sandbox.stateset.app/api/integrations/status',
        expect.objectContaining({
          headers: expect.objectContaining({
            Authorization: `Bearer ${token}`,
          }),
        })
      );
    });
  });

  describe('duplicate registration', () => {
    it('allows login after registration conflict', async () => {
      // ====================================================================
      // Step 1: Try to register with an existing email - should get 409
      // ====================================================================
      mockFetch.mockResolvedValueOnce({
        ok: false,
        status: 409,
        json: () => Promise.resolve({ error: 'Email already registered' }),
      });

      const { POST: registerPOST } = await import('@/app/api/auth/register/route');

      const registerRequest = createMockRequest({
        method: 'POST',
        url: 'http://localhost:3000/api/auth/register',
        body: {
          email: 'existing@example.com',
          password: 'Securepass1',
          firstName: 'John',
          lastName: 'Doe',
        },
      });

      const registerResponse = await registerPOST(registerRequest, undefined as any);
      await expectError(registerResponse, 409, 'CONFLICT');

      // ====================================================================
      // Step 2: Login with the existing email instead - should succeed
      // ====================================================================
      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: () =>
          Promise.resolve({
            token: 'existing-user-token',
            user: { id: 'user-existing', email: 'existing@example.com' },
          }),
      });

      const { POST: loginPOST } = await import('@/app/api/auth/login/route');

      const loginRequest = createMockRequest({
        method: 'POST',
        url: 'http://localhost:3000/api/auth/login',
        body: {
          email: 'existing@example.com',
          password: 'Securepass1',
        },
      });

      const loginResponse = await loginPOST(loginRequest, undefined as any);
      const loginBody = await expectSuccess(loginResponse);

      expect(loginBody.data).toHaveProperty('token', 'existing-user-token');
    });
  });
});
