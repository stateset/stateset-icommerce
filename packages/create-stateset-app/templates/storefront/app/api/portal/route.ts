import { NextRequest, NextResponse } from 'next/server';
import { getCommerce } from '@/lib/commerce';
import { portalPrincipal, portalReport } from '@/lib/merchant-portal.js';
import { installIntegration, processIntegrations } from '@/lib/merchant-integrations.js';
import { cardAccounts, verifyCardAccount } from '@/lib/card-accounts.js';
import { affiliateBankPayouts } from '@/lib/seller-payouts.js';
import { publishCampaign } from '@/lib/campaigns.js';
import {
  confirmPayoutExport,
  reserveAffiliateBatch,
  settleAffiliateTransfer,
} from '@/lib/affiliate-payouts.js';
import { recoveryReport } from '@/lib/billing-recovery.js';
import { storefrontStripe, payoutStripe } from '@/lib/stripe-client';
import { publishPricing } from '@/lib/store-pricing.js';
import {
  importCatalog,
  runLaunchDrill,
  validateCatalog,
  verifyStripeConnection,
} from '@/lib/merchant-onboarding.js';
import { readMerchantRecord } from '@/lib/merchant-records.js';

export const runtime = 'nodejs';
const cookie = 'stateset_merchant_portal';
const headers = { 'Cache-Control': 'private, no-store', 'Referrer-Policy': 'no-referrer' };
export async function GET(request: NextRequest) {
  const commerce = getCommerce();
  const principal = await portalPrincipal(commerce, request.cookies.get(cookie)?.value);
  if (!principal)
    return NextResponse.json(
      { error: 'Open a current portal link from your merchant.' },
      { status: 401, headers },
    );
  try {
    if (request.nextUrl.searchParams.get('view') === 'verification') {
      if (!['seller', 'affiliate'].includes(principal.role))
        return new NextResponse(null, { status: 403, headers });
      const prefix = principal.role === 'seller' ? 'seller-' : 'affiliate-connect-';
      const binding = await readMerchantRecord(commerce, prefix + principal.subject);
      if (!binding)
        return NextResponse.json(
          { error: 'Set up a payout account first.' },
          { status: 404, headers },
        );
      const account = await payoutStripe().accounts.retrieve(binding.account);
      return NextResponse.json(
        {
          enabled: account.payouts_enabled,
          required: account.requirements?.currently_due || [],
          pastDue: account.requirements?.past_due || [],
          reason: account.requirements?.disabled_reason,
        },
        { headers },
      );
    }
    if (request.nextUrl.searchParams.get('view') === 'renewals') {
      if (principal.role !== 'merchant' || process.env.STATESET_RENEWAL_RECOVERY !== 'true')
        return new NextResponse(null, { status: 403, headers });
      return NextResponse.json(await recoveryReport(commerce, storefrontStripe().stripe), {
        headers,
      });
    }
    const params = request.nextUrl.searchParams;
    if (params.get('view') === 'delivery') {
      if (principal.role !== 'merchant') return new NextResponse(null, { status: 403, headers });
      const key = params.get('key') || '';
      if (!/^delivery-[a-f0-9]{64}$/.test(key))
        return new NextResponse(null, { status: 400, headers });
      return NextResponse.json(
        (await processIntegrations(commerce, process.env, { onlyKey: key }))[0],
        { headers },
      );
    }
    return NextResponse.json(
      await portalReport(commerce, process.env, principal, {
        from: params.get('from') || '',
        to: params.get('to') || '',
        control: params.get('control') || '',
      }),
      { headers },
    );
  } catch {
    return NextResponse.json(
      { error: 'Unable to load the report. Check the dates and store configuration.' },
      { status: 422, headers },
    );
  }
}
export async function POST(request: NextRequest) {
  if (
    request.headers.get('origin') !==
    new URL(process.env.NEXT_PUBLIC_BASE_URL || request.url).origin
  )
    return new NextResponse(null, { status: 403, headers });
  const body = await request.json().catch(() => null);
  if (!body || typeof body !== 'object' || Array.isArray(body))
    return NextResponse.json(
      { error: 'Supply a portal action as a JSON object' },
      { status: 400, headers },
    );
  const commerce = getCommerce();
  if (body.token) {
    const grant = await portalPrincipal(commerce, body.token);
    const response = grant
      ? NextResponse.json({ role: grant.role }, { headers })
      : new NextResponse(null, { status: 403, headers });
    response.cookies.set(cookie, grant ? body.token : '', {
      httpOnly: true,
      secure: new URL(process.env.NEXT_PUBLIC_BASE_URL || request.url).protocol === 'https:',
      sameSite: 'strict',
      path: '/api/portal',
      expires: new Date(grant?.expires ?? 0),
    });
    return response;
  }
  const principal = await portalPrincipal(commerce, request.cookies.get(cookie)?.value);
  if (principal?.role !== 'merchant') return new NextResponse(null, { status: 403, headers });
  try {
    if (body.action === 'reserve-commissions') {
      if (body.apply === true && !/^[a-f0-9]{64}$/.test(body.fingerprint || ''))
        throw new Error('Preview the commission batch first.');
      return NextResponse.json(
        await reserveAffiliateBatch(commerce, body.batch, body.apply === true, body.rail, {
          expected: body.fingerprint,
          affiliate: body.affiliate,
        }),
        { headers },
      );
    }
    if (body.action === 'sync-affiliate-bank')
      return NextResponse.json(
        await affiliateBankPayouts(commerce, payoutStripe(), body.affiliate, body.apply === true),
        { headers },
      );
    if (body.action === 'settle-commission') {
      if (typeof body.affiliate !== 'string' || !body.affiliate)
        throw new Error('Choose an affiliate.');
      const result =
        body.rail === 'stripe'
          ? await settleAffiliateTransfer(
              commerce,
              body.apply === true ? payoutStripe() : null,
              body.batch,
              body.affiliate,
              body.apply === true,
            )
          : await confirmPayoutExport(commerce, body.batch, body.reference, body.apply === true, {
              affiliate: body.affiliate,
              paidAt: body.paidAt,
            });
      return NextResponse.json(result, { headers });
    }
    if (body.pricing)
      return NextResponse.json(
        await publishPricing(commerce, process.env, body.pricing, body.apply === true),
        { headers },
      );
    if (body.catalog) {
      const rows = validateCatalog(body.catalog);
      return NextResponse.json(
        body.apply === true
          ? await importCatalog(commerce, rows)
          : { rows: rows.length, mode: 'preview' },
        { headers },
      );
    }
    if (body.action === 'verify-routes') {
      const accounts = await Promise.all(
        cardAccounts(process.env).map(async (account) => {
          try {
            return await verifyCardAccount(account);
          } catch {
            return {
              account: account.account,
              provider: account.provider,
              error: 'Account lookup failed. Check the account credentials and location.',
            };
          }
        }),
      );
      return NextResponse.json({ accounts }, { headers });
    }
    if (body.action === 'verify-processor') {
      const { stripe, origin } = storefrontStripe();
      return NextResponse.json(
        await verifyStripeConnection(
          stripe,
          origin,
          process.env.STATESET_RENEWAL_RECOVERY === 'true',
        ),
        { headers },
      );
    }
    if (body.drill)
      return NextResponse.json(
        await runLaunchDrill(commerce, storefrontStripe().stripe, process.env, {
          ...body.drill,
          apply: body.apply === true,
        }),
        { headers },
      );
    if (body.campaign)
      return NextResponse.json(
        await publishCampaign(commerce, process.env, body.campaign, body.apply === true),
        { headers },
      );
    if (typeof body.integration === 'string')
      return NextResponse.json(
        await installIntegration(commerce, process.env, body.integration, body.apply === true),
        { headers },
      );
    if (typeof body.delivery === 'string' && /^delivery-[a-f0-9]{64}$/.test(body.delivery)) {
      return NextResponse.json(
        await processIntegrations(commerce, process.env, {
          apply: true,
          onlyKey: body.delivery,
          checkOnly: body.check === true,
          retryClaim: typeof body.retryClaim === 'string' ? body.retryClaim : '',
        }),
        { headers },
      );
    }
    return new NextResponse(null, { status: 400, headers });
  } catch (error) {
    return NextResponse.json(
      { error: error instanceof Error ? error.message : 'Merchant operation failed' },
      { status: 422, headers },
    );
  }
}
