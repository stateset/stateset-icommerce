import { issuePortal, revokePortal } from '../lib/merchant-portal.js';
import { linkMembership } from '../lib/digital-access.js';
import { parseArgs } from 'node:util';
import {
  onboardSeller,
  onboardAffiliate,
  settleSellers,
  resolveRenewalSeller,
  sellerBalances,
  sellerBankPayouts,
  affiliateBankPayouts,
} from '../lib/seller-payouts.js';
import {
  affiliateExport,
  campaignAccounting,
  registerAffiliate,
  affiliateBalances,
  payoutExport,
  confirmPayoutExport,
} from '../lib/campaigns.js';
import { settleAffiliateTransfer } from '../lib/affiliate-payouts.js';
import { Commerce } from '@stateset/embedded';
import Stripe from 'stripe';
import { getStripeConfiguration, getStripePlatformConfiguration } from '../lib/stripe-config.js';
import { loadStoreEnvironment } from '../lib/store-environment.js';
import { inspectStoreDatabase } from '../lib/store-database-check.js';
import {
  installIntegration,
  processIntegrations,
  sendKlaviyoEvent,
} from '../lib/merchant-integrations.js';
import {
  RenewalRecoveryError,
  linkBillingSubscription,
  reconcileRenewals,
  renewalStatement,
  recoveryReport,
  runRenewalRecovery,
  sendRecoveryEmail,
} from '../lib/billing-recovery.js';

const { positionals, values } = parseArgs({
  allowPositionals: true,
  options: {
    order: { type: 'string' },
    subscription: { type: 'string' },
    'provider-subscription': { type: 'string' },
    role: { type: 'string' },
    days: { type: 'string', default: '7' },
    help: { type: 'boolean' },
    apply: { type: 'boolean' },
    batch: { type: 'string' },
    'by-source': { type: 'boolean' },
    reference: { type: 'string' },
    affiliate: { type: 'string' },
    seller: { type: 'string' },
    'platform-owned': { type: 'boolean' },
    rail: { type: 'string', default: 'external' },
    'paid-at': { type: 'string' },
    'retry-missing-attempt': { type: 'string' },
    country: { type: 'string' },
    email: { type: 'string' },
    bps: { type: 'string' },
    campaign: { type: 'string' },
    delivery: { type: 'string' },
    check: { type: 'boolean' },
    'retry-dispatch': { type: 'string' },
  },
});
const usage = [
  'npm run merchant -- COMMAND [--apply]',
  'link-membership RESOURCE --order ORDER_ID --subscription SUBSCRIPTION_ID',
  'link-billing SUBSCRIPTION_ID --provider-subscription STRIPE_SUBSCRIPTION_ID',
  'renewal-statement | reconcile-renewals SUBSCRIPTION_ID [--apply]',
  'resolve-renewal-seller SUBSCRIPTION_ID (--seller CODE --bps FEE | --platform-owned) [--apply]',
  'portal ACCOUNT --role merchant|affiliate|seller [--days N] | revoke-portal LINK_ID',
  'install PROVIDER | deliveries [--delivery KEY]',
  'deliveries --delivery KEY --check --apply | deliveries --delivery KEY --retry-dispatch CLAIM_ID --apply',
  'campaign-report [--by-source] | recovery-report | recover-renewals',
  'affiliate CODE --bps RATE [--campaign CAMPAIGN] | commissions | affiliate-balances',
  'export-commissions | export-payouts --batch BATCH [--rail external|stripe] [--affiliate CODE]',
  'confirm-payouts --batch BATCH [--affiliate CODE] --reference REFERENCE [--paid-at ISO_TIMESTAMP]',
  'onboard-affiliate CODE --country CODE --email EMAIL | settle-affiliate CODE --batch BATCH | affiliate-payouts [CODE] | affiliate-bank-payouts CODE',
  'onboard-seller SELLER --country CODE --email EMAIL | seller-statement [SELLER] | settle-sellers | bank-payouts SELLER',
].join('\n');
if (values.help || !positionals.length) {
  console.log(usage);
  process.exit(0);
}
let commerce;
try {
  await loadStoreEnvironment();
  if (inspectStoreDatabase(process.env, { launch: true }).status !== 'pass')
    throw new Error('An existing store database is required');
  commerce = new Commerce(process.env.STATESET_DB_PATH || './store.db');
  const [command, name] = positionals;
  let result;
  if (command === 'portal')
    result = await issuePortal(
      commerce,
      process.env.NEXT_PUBLIC_BASE_URL,
      values.role,
      name,
      Number(values.days),
      values.apply,
    );
  else if (command === 'revoke-portal') result = await revokePortal(commerce, name, values.apply);
  else if (command === 'link-membership')
    result = await linkMembership(commerce, values.order, name, values.subscription, values.apply);
  else if (command === 'install')
    result = await installIntegration(commerce, process.env, name, values.apply);
  else if (command === 'deliveries') {
    result = await processIntegrations(commerce, process.env, {
      apply: values.apply,
      onlyKey: values.delivery,
      checkOnly: values.check,
      retryClaim: values['retry-dispatch'],
    });
    if (
      result.some(
        (row) =>
          row.status === 'failed' ||
          row.status === 'review' ||
          (values.check && values.apply && values.delivery && row.status !== 'delivered'),
      )
    )
      process.exitCode = 1;
  } else if (command === 'campaign-report')
    result = (await campaignAccounting(commerce, { bySource: values['by-source'] })).variants;
  else if (command === 'affiliate') {
    result = await registerAffiliate(commerce, name, Number(values.bps), values.apply);
    if (values.campaign) {
      const link = new URL(
        '/c/' + encodeURIComponent(values.campaign),
        process.env.NEXT_PUBLIC_BASE_URL,
      );
      link.searchParams.set('ref', name);
      result.referralUrl = link.href;
    }
  } else if (command === 'commissions') result = (await campaignAccounting(commerce)).commissions;
  else if (command === 'affiliate-balances') result = (await affiliateBalances(commerce)).balances;
  else if (command === 'affiliate-payouts')
    result = (await affiliateBalances(commerce)).payouts.filter(
      (row) => !name || row.affiliate === name,
    );
  else if (command === 'export-payouts')
    result = await payoutExport(
      commerce,
      values.batch,
      values.apply,
      values.rail,
      values.affiliate,
    );
  else if (command === 'confirm-payouts')
    result = await confirmPayoutExport(commerce, values.batch, values.reference, values.apply, {
      affiliate: values.affiliate,
      paidAt: values['paid-at'],
    });
  else if (command === 'export-commissions') result = await affiliateExport(commerce);
  else if (command === 'renewal-statement') result = await renewalStatement(commerce);
  else if (command === 'resolve-renewal-seller')
    result = await resolveRenewalSeller(commerce, name, {
      seller: values.seller,
      bps: values.bps,
      platformOwned: values['platform-owned'],
      apply: values.apply,
    });
  else if (command === 'reconcile-renewals') {
    const stripe = new Stripe(getStripeConfiguration(process.env).key, {
      maxNetworkRetries: 2,
      timeout: 12000,
    });
    result = await reconcileRenewals(commerce, stripe, name, values.apply);
  } else if (['recovery-report', 'recover-renewals', 'link-billing'].includes(command)) {
    if (process.env.STATESET_RENEWAL_RECOVERY !== 'true')
      throw new Error('Enable renewal recovery for this Stripe account');
    const stripe = new Stripe(getStripeConfiguration(process.env).key, {
      maxNetworkRetries: 2,
      timeout: 12000,
    });
    result =
      command === 'link-billing'
        ? await linkBillingSubscription(
            commerce,
            stripe,
            name,
            values['provider-subscription'],
            values.apply,
          )
        : command === 'recovery-report'
          ? await recoveryReport(commerce, stripe)
          : await runRenewalRecovery({
              commerce,
              stripe,
              env: process.env,
              apply: values.apply,
              notify: (notice) =>
                process.env.RESEND_API_KEY
                  ? sendRecoveryEmail(process.env, notice)
                  : sendKlaviyoEvent(process.env, {
                      key: notice.key,
                      email: notice.email,
                      metric: 'StateSet Renewal Recovery',
                      properties: {
                        invoice_id: notice.invoiceId,
                        update_url: notice.updateUrl,
                        payment_url: notice.paymentUrl,
                      },
                    }),
            });
  } else if (command === 'seller-statement') result = await sellerBalances(commerce, name);
  else if (
    [
      'onboard-seller',
      'settle-sellers',
      'bank-payouts',
      'affiliate-bank-payouts',
      'onboard-affiliate',
      'settle-affiliate',
    ].includes(command)
  ) {
    const readsProvider = command === 'bank-payouts' || command === 'affiliate-bank-payouts';
    const stripe =
      values.apply || readsProvider
        ? new Stripe(getStripePlatformConfiguration(process.env).key, {
            maxNetworkRetries: 2,
            timeout: 12000,
          })
        : null;
    if (command === 'settle-affiliate')
      result = await settleAffiliateTransfer(
        commerce,
        stripe,
        values.batch,
        name,
        values.apply,
        values['retry-missing-attempt'] === undefined
          ? undefined
          : Number(values['retry-missing-attempt']),
      );
    else if (command === 'onboard-affiliate')
      result = await onboardAffiliate(commerce, stripe, process.env, {
        affiliate: name,
        country: values.country,
        email: values.email,
        apply: values.apply,
      });
    else if (command === 'onboard-seller')
      result = await onboardSeller(commerce, stripe, process.env, {
        seller: name,
        country: values.country,
        email: values.email,
        apply: values.apply,
      });
    else if (command === 'affiliate-bank-payouts')
      result = await affiliateBankPayouts(commerce, stripe, name, values.apply);
    else if (command === 'bank-payouts')
      result = await sellerBankPayouts(commerce, stripe, name, values.apply);
    else {
      result = await settleSellers(commerce, stripe, process.env, values.apply);
      if (result.some((row) => row.state === 'failed')) process.exitCode = 1;
    }
  } else throw new Error(usage);
  console.log(typeof result === 'string' ? result : JSON.stringify(result, null, 2));
} catch (error) {
  if (error instanceof RenewalRecoveryError) console.log(JSON.stringify(error.report, null, 2));
  console.error(error instanceof Error ? error.message : 'Merchant operation failed');
  process.exitCode = 1;
} finally {
  if (commerce) await commerce.close();
}
