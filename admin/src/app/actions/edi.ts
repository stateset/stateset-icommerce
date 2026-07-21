'use server';

/**
 * EDI operations server actions (trading-partner document tracking).
 *
 * Every exported action is gated by `requireAdminSession()` — server actions
 * bypass the API middleware, so each one must enforce the admin session
 * itself (skipped in the auth-disabled dev mode, like middleware).
 *
 * Read-only slice: listing, detail, and aggregate summary.
 */

import {
  ediDocumentsApi,
  type EdiDocument,
  type EdiDocumentFilter,
  type EdiSummary,
} from '@/lib/embedded';
import { requireAdminSession } from '@/lib/shared/auth-session';

export async function getEdiDocuments(filter?: EdiDocumentFilter): Promise<EdiDocument[]> {
  await requireAdminSession();
  return ediDocumentsApi.list(filter);
}

export async function getEdiDocument(id: string): Promise<EdiDocument | null> {
  await requireAdminSession();
  if (typeof id !== 'string' || id.trim().length === 0) {
    throw new Error('id is required');
  }
  return ediDocumentsApi.get(id);
}

export async function getEdiSummary(): Promise<EdiSummary> {
  await requireAdminSession();
  return ediDocumentsApi.summary();
}

/** Documents + aggregate summary in one round trip for the operations page. */
export async function getEdiPageData(): Promise<{
  documents: EdiDocument[];
  summary: EdiSummary;
}> {
  await requireAdminSession();
  const [documents, summary] = await Promise.all([
    ediDocumentsApi.list(),
    ediDocumentsApi.summary(),
  ]);
  return { documents, summary };
}
