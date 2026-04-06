/**
 * API Route Error Handler Wrapper
 *
 * Wraps route handlers with consistent error handling, request context,
 * and structured logging. Supports SSE error mode for streaming endpoints.
 */

import { NextRequest, NextResponse } from 'next/server';
import { requestStore, generateRequestId, type RequestContext } from './request-context';
import { AppError, ValidationError } from './errors';
import { logger } from './logger';
import { sendError } from './response';
import { requireCsrf } from './csrf';

/** Default max request body size: 1MB */
const DEFAULT_MAX_BODY_SIZE = 1_048_576;

interface ErrorHandlerOptions {
  /** When true, sends errors as SSE events instead of JSON (for streaming endpoints) */
  sse?: boolean;
  /** Maximum allowed request body size in bytes (default: 1MB) */
  maxBodySize?: number;
  /** When true, validate CSRF token for state-changing requests */
  requireCsrf?: boolean;
}

type RouteHandler = (
  request: NextRequest,
  context?: { params: Promise<Record<string, string>> }
) => Promise<NextResponse | Response>;

/**
 * Wrap an API route handler with error handling, request context, and logging.
 */
export function withErrorHandler(
  handler: RouteHandler,
  options: ErrorHandlerOptions = {}
): RouteHandler {
  return async (request: NextRequest, routeContext?) => {
    const requestId = generateRequestId();
    const startTime = Date.now();

    const reqContext: RequestContext = {
      requestId,
      startTime,
      path: request.nextUrl.pathname,
      method: request.method,
    };

    // Extract orgId from auth header if present
    const authHeader = request.headers.get('Authorization');
    if (authHeader) {
      // orgId extraction could be done from JWT claims in production
      reqContext.orgId = request.headers.get('x-org-id') ?? undefined;
    }

    return requestStore.run(reqContext, async () => {
      try {
        if (options.requireCsrf) {
          const csrfResponse = await requireCsrf(request);
          if (csrfResponse) {
            csrfResponse.headers.set('X-Request-Id', requestId);
            return csrfResponse;
          }
        }

        // Enforce request body size limit
        const contentLength = request.headers.get('content-length');
        const maxSize = options.maxBodySize ?? DEFAULT_MAX_BODY_SIZE;
        if (contentLength && parseInt(contentLength, 10) > maxSize) {
          return sendError(413, `Request body exceeds maximum size of ${maxSize} bytes`, 'PAYLOAD_TOO_LARGE');
        }

        logger.info('Request started', {
          path: reqContext.path,
          method: reqContext.method,
        });

        const response = await handler(request, routeContext);

        const durationMs = Date.now() - startTime;
        logger.info('Request completed', {
          durationMs,
          status: response instanceof NextResponse ? response.status : 200,
        });

        // Add request ID to response headers
        if (response instanceof NextResponse) {
          response.headers.set('X-Request-Id', requestId);
        }

        return response;
      } catch (error) {
        const durationMs = Date.now() - startTime;

        if (error instanceof ValidationError) {
          logger.warn('Validation error', {
            durationMs,
            details: error.details,
          });

          if (options.sse) {
            return sseError(error.message, error.code, error.statusCode);
          }

          return sendError(error.statusCode, error.message, error.code);
        }

        if (error instanceof AppError) {
          if (error.isOperational) {
            logger.warn('Operational error', {
              durationMs,
              code: error.code,
              statusCode: error.statusCode,
            });
          } else {
            logger.error('Non-operational error', {
              durationMs,
              code: error.code,
              statusCode: error.statusCode,
              stack: error.stack,
            });
          }

          if (options.sse) {
            return sseError(error.message, error.code, error.statusCode);
          }

          return sendError(error.statusCode, error.message, error.code);
        }

        // Unknown errors
        const message = error instanceof Error ? error.message : 'Internal server error';
        const stack = error instanceof Error ? error.stack : undefined;

        logger.error('Unhandled error', {
          durationMs,
          error: message,
          stack,
        });

        if (options.sse) {
          return sseError('Internal server error', 'INTERNAL_ERROR', 500);
        }

        return sendError(500, 'Internal server error', 'INTERNAL_ERROR');
      }
    });
  };
}

/**
 * Send an error as an SSE event (for streaming endpoints where headers may already be sent).
 */
function sseError(message: string, code: string, _status: number): Response {
  const errorEvent = `event: error\ndata: ${JSON.stringify({ message, code })}\n\n`;
  return new Response(errorEvent, {
    headers: {
      'Content-Type': 'text/event-stream',
      'Cache-Control': 'no-cache',
      Connection: 'keep-alive',
    },
  });
}
