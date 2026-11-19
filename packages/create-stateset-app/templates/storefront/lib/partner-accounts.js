import { randomUUID } from 'node:crypto';
import { firstMerchantRecord, readMerchantRecord, saveMerchantRecord } from './merchant-records.js';
import { issuePortal } from './merchant-portal.js';
import { normalizeCheckoutEmail } from './checkout-service.js';
import { onboardAffiliate, onboardSeller } from './seller-payouts.js';
import { partnerMessage } from './partner-signature.js';

export function partnerPrograms(env) {
  const rate = env.STATESET_AFFILIATE_DEFAULT_BPS;
  const bps = /^\d{1,5}$/.test(rate || '') && Number(rate) <= 10000 ? Number(rate) : null;
  const countries = (value) =>
    (value || '')
      .split(',')
      .map((s) => s.trim().toUpperCase())
      .filter((s) => /^[A-Z]{2}$/.test(s));
  return {
    bps,
    countries: countries(env.STATESET_SELLER_SIGNUP_COUNTRIES),
    affiliateCountries: countries(env.STATESET_AFFILIATE_PAYOUT_COUNTRIES),
  };
}

export async function openPartnerAccount(
  commerce,
  env,
  input,
  verify,
  stripeClient,
  now = Date.now(),
) {
  const { wallet, role, action, email = '', country = '', at, nonce, signature } = input;
  if (
    typeof wallet !== 'string' ||
    !/^0x[0-9a-f]{40}$/i.test(wallet) ||
    !['affiliate', 'seller'].includes(role) ||
    !['portal', 'onboard'].includes(action) ||
    !Number.isSafeInteger(at) ||
    Math.abs(at - now) > 300000 ||
    typeof nonce !== 'string' ||
    !/^[a-f\d-]{36}$/i.test(nonce) ||
    typeof signature !== 'string' ||
    !/^0x[0-9a-fA-F]{130}$/.test(signature) ||
    typeof email !== 'string' ||
    email.length > 254 ||
    /[\r\n]/.test(email) ||
    typeof country !== 'string' ||
    (country && !/^[A-Z]{2}$/.test(country))
  )
    throw new Error('Invalid or expired partner request');
  const origin = new URL(env.NEXT_PUBLIC_BASE_URL).origin;
  if (!(await verify({ address: wallet, message: partnerMessage(origin, input), signature })))
    throw new Error('Signature does not match the partner request');
  const owner = wallet.toLowerCase();
  const code = 'wallet-' + owner.slice(2);
  const existing = await readMerchantRecord(commerce, role + '-' + code);
  if (existing && existing.ownerWallet !== owner)
    throw new Error('This account is operator-managed');
  const { bps, countries, affiliateCountries } = partnerPrograms(env);
  if (!existing && role === 'affiliate' && (bps === null || bps !== input.bps))
    throw new Error('Affiliate enrollment terms changed or enrollment is closed');
  if (action === 'onboard' || (role === 'seller' && !existing)) {
    const bound =
      role === 'seller'
        ? existing
        : await readMerchantRecord(commerce, 'affiliate-connect-' + code);
    if (
      action !== 'onboard' ||
      (!bound && !(role === 'seller' ? countries : affiliateCountries).includes(country))
    )
      throw new Error('Complete payout verification for a supported country');
    if (normalizeCheckoutEmail(email) !== email) throw new Error('Use a normalized email address');
  }
  const claim = randomUUID();
  const proof = await firstMerchantRecord(
    commerce,
    'partner-signature-' + owner.slice(2) + '-' + nonce,
    { claim },
  );
  if (proof.claim !== claim) throw new Error('Sign a fresh partner request');
  if (role === 'affiliate' && !existing) {
    await saveMerchantRecord(
      commerce,
      'affiliate-' + code,
      { code, commissionBps: bps, ownerWallet: owner },
      'affiliates',
    );
  }
  if (action === 'onboard') {
    const onboard = role === 'seller' ? onboardSeller : onboardAffiliate;
    const result = await onboard(commerce, stripeClient(), env, {
      [role]: code,
      email,
      country,
      ownerWallet: owner,
      apply: true,
    });
    return { url: result.onboardingUrl };
  }
  const portal = await issuePortal(commerce, origin, role, code, 7, true, now);
  return { url: portal.url };
}
