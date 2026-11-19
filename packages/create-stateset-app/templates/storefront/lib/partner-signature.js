export function partnerMessage(origin, request) {
  return [
    'StateSet partner account',
    origin,
    'Wallet: ' + request.wallet.toLowerCase(),
    'Role: ' + request.role,
    'Action: ' + request.action,
    'Email: ' + (request.email || ''),
    'Country: ' + (request.country || ''),
    'New affiliate commission (basis points): ' + (request.bps ?? ''),
    'Time: ' + request.at,
    'Request: ' + request.nonce,
  ].join('\n');
}
