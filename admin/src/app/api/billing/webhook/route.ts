import { NextRequest, NextResponse } from 'next/server';
import { logger } from '@/lib/shared/logger';
import { withErrorHandler } from '@/lib/shared/with-error-handler';

const STRIPE_WEBHOOK_SECRET = process.env.STRIPE_WEBHOOK_SECRET;

/**
 * POST /api/billing/webhook
 *
 * Stripe webhook endpoint for billing events.
 * Verifies webhook signature before processing events.
 */
export const POST = withErrorHandler(async (request: NextRequest) => {
  try {
    const body = await request.text();
    const signature = request.headers.get('stripe-signature');

    if (!signature) {
      logger.warn('Billing webhook: missing signature header');
      return NextResponse.json(
        { success: false, error: { message: 'Missing signature', code: 'WEBHOOK_INVALID' } },
        { status: 400 }
      );
    }

    if (!STRIPE_WEBHOOK_SECRET) {
      logger.error('Billing webhook: STRIPE_WEBHOOK_SECRET not configured');
      return NextResponse.json(
        { success: false, error: { message: 'Webhook not configured', code: 'WEBHOOK_CONFIG_ERROR' } },
        { status: 500 }
      );
    }

    // Verify signature using Stripe's algorithm (HMAC-SHA256)
    const encoder = new TextEncoder();
    const [, timestamp, sig] = parseSignatureHeader(signature);

    if (!timestamp || !sig) {
      logger.warn('Billing webhook: malformed signature header');
      return NextResponse.json(
        { success: false, error: { message: 'Malformed signature', code: 'WEBHOOK_INVALID' } },
        { status: 400 }
      );
    }

    // Check timestamp tolerance (5 minutes)
    const timestampAge = Math.abs(Date.now() / 1000 - Number(timestamp));
    if (timestampAge > 300) {
      logger.warn('Billing webhook: timestamp too old', { age: timestampAge });
      return NextResponse.json(
        { success: false, error: { message: 'Timestamp too old', code: 'WEBHOOK_EXPIRED' } },
        { status: 400 }
      );
    }

    const signedPayload = `${timestamp}.${body}`;
    const key = await crypto.subtle.importKey(
      'raw',
      encoder.encode(STRIPE_WEBHOOK_SECRET),
      { name: 'HMAC', hash: 'SHA-256' },
      false,
      ['sign']
    );
    const expectedSig = await crypto.subtle.sign('HMAC', key, encoder.encode(signedPayload));
    const expectedHex = Array.from(new Uint8Array(expectedSig))
      .map((b) => b.toString(16).padStart(2, '0'))
      .join('');

    if (expectedHex !== sig) {
      logger.warn('Billing webhook: signature mismatch');
      return NextResponse.json(
        { success: false, error: { message: 'Invalid signature', code: 'WEBHOOK_INVALID' } },
        { status: 400 }
      );
    }

    // Process the event
    const event = JSON.parse(body);
    logger.info('Billing webhook received', { type: event.type, id: event.id });

    switch (event.type) {
      case 'customer.subscription.created':
      case 'customer.subscription.updated':
      case 'customer.subscription.deleted':
        logger.info('Subscription event processed', { type: event.type });
        break;
      case 'invoice.paid':
      case 'invoice.payment_failed':
        logger.info('Invoice event processed', { type: event.type });
        break;
      default:
        logger.info('Unhandled webhook event', { type: event.type });
    }

    return NextResponse.json({ success: true, data: { received: true } });
  } catch (error) {
    logger.error('Billing webhook error', {
      error: error instanceof Error ? error.message : 'Unknown error',
    });
    return NextResponse.json(
      { success: false, error: { message: 'Webhook processing failed', code: 'WEBHOOK_ERROR' } },
      { status: 500 }
    );
  }
});

function parseSignatureHeader(header: string): [string, string | undefined, string | undefined] {
  const parts: Record<string, string> = {};
  header.split(',').forEach((item) => {
    const [key, value] = item.split('=');
    if (key && value) parts[key.trim()] = value.trim();
  });
  return ['stripe', parts['t'], parts['v1']];
}
