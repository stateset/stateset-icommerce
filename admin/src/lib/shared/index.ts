/**
 * Shared Library Barrel Export
 *
 * Provides convenient access to all shared infrastructure.
 */

export { AppError, ValidationError } from './errors';
export { logger } from './logger';
export {
  CacheMap,
  pendingConfirmations,
  sandboxCache,
  healthCheckCache,
  successCache,
  activeLoops,
  loopMetadata,
  guardrailCache,
  sandboxApiKeyCache,
  rateLimitStore,
} from './redis';
export {
  requestStore,
  getRequestContext,
  getRequestId,
  generateRequestId,
  type RequestContext,
} from './request-context';
export { sendSuccess, sendError, sendPaginated } from './response';
export { withErrorHandler } from './with-error-handler';
export {
  paginationQuerySchema,
  loginSchema,
  registerSchema,
  forgotPasswordSchema,
  resetPasswordSchema,
  verifyEmailSchema,
  listSessionsSchema,
  cancelSessionSchema,
  agentChatMessageSchema,
  confirmActionSchema,
  createSubscriptionSchema,
  updateSubscriptionSchema,
  integrationCredentialSchema,
  createAutonomousSessionSchema,
  sessionActionSchema,
  validateBody,
  validateQuery,
  type PaginationQuery,
  type ValidationResult,
} from './schemas';
export { getOrCreateCsrfToken, validateCsrfToken, requireCsrf } from './csrf';
export {
  ADMIN_AUTH_DISABLE_FLAG,
  getBypassAdminUser,
  isAdminAuthDisabled,
} from './admin-auth-config';
export {
  ADMIN_SESSION_COOKIE,
  extractBearerToken,
  getRequestSessionToken,
  getServiceSessionToken,
  requireRequestSessionToken,
  getServerSessionToken,
  requireServerSessionToken,
  requireAdminSession,
  setSessionCookie,
  clearSessionCookie,
  isAuthenticatedRequest,
  validateSessionToken,
} from './auth-session';
export { AdminLoginGate } from './admin-login-gate';
export { RateLimiter, apiRateLimiter, authRateLimiter } from './rate-limit';
