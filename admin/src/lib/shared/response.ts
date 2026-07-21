/**
 * Standard Response Envelope
 *
 * Consistent API response shape for all endpoints.
 * Shape: { success: boolean, data?, error?: { message, code }, meta?: { requestId, timestamp, pagination? } }
 */

import { NextResponse } from 'next/server';
import { getRequestId } from './request-context';

interface ResponseMeta {
  requestId: string;
  timestamp: string;
  pagination?: PaginationMeta;
}

interface PaginationMeta {
  total: number;
  limit: number;
  offset: number;
  hasMore: boolean;
}

interface SuccessResponse<T> {
  success: true;
  data: T;
  meta: ResponseMeta;
}

interface ErrorResponse {
  success: false;
  error: {
    message: string;
    code: string;
  };
  meta: ResponseMeta;
}

interface PaginatedResponse<T> {
  success: true;
  data: T[];
  meta: ResponseMeta & { pagination: PaginationMeta };
}

function buildMeta(extra?: Partial<ResponseMeta>): ResponseMeta {
  return {
    requestId: getRequestId(),
    timestamp: new Date().toISOString(),
    ...extra,
  };
}

/**
 * Send a success response with standard envelope.
 */
export function sendSuccess<T>(data: T, status: number = 200): NextResponse<SuccessResponse<T>> {
  return NextResponse.json(
    {
      success: true as const,
      data,
      meta: buildMeta(),
    },
    { status },
  );
}

/**
 * Send an error response with standard envelope.
 */
export function sendError(
  status: number,
  message: string,
  code: string = 'ERROR',
): NextResponse<ErrorResponse> {
  return NextResponse.json(
    {
      success: false as const,
      error: { message, code },
      meta: buildMeta(),
    },
    { status },
  );
}

/**
 * Send a paginated response with standard envelope.
 */
export function sendPaginated<T>(
  data: T[],
  pagination: { total: number; limit: number; offset: number },
): NextResponse<PaginatedResponse<T>> {
  return NextResponse.json({
    success: true as const,
    data,
    meta: {
      ...buildMeta(),
      pagination: {
        total: pagination.total,
        limit: pagination.limit,
        offset: pagination.offset,
        hasMore: pagination.offset + pagination.limit < pagination.total,
      },
    },
  });
}
