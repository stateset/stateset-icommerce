export const DEFAULT_ASSET = 'USDC';
export const DEFAULT_NETWORK = 'set_chain';
const DEFAULT_DECIMALS = 6;

const NETWORK_DEFAULT_ASSET = {
  bitcoin: 'BTC',
  bitcoin_testnet: 'BTC',
  zcash: 'ZEC',
  zcash_testnet: 'ZEC',
};

export function getDefaultAssetForNetwork(network) {
  return NETWORK_DEFAULT_ASSET[network] || DEFAULT_ASSET;
}

export function getAssetDecimals(asset) {
  const upper = String(asset).toUpperCase();
  switch (upper) {
    case 'USDC':
    case 'USDT':
    case 'SSUSD':
    case 'WSSUSD':
      return 6;
    case 'BTC':
    case 'ZEC':
      return 8;
    case 'DAI':
    case 'ETH':
      return 18;
    default:
      return DEFAULT_DECIMALS;
  }
}

export function toSmallestUnit(amount, decimals = DEFAULT_DECIMALS) {
  const numeric = typeof amount === 'string' ? parseFloat(amount) : amount;
  return Math.round(numeric * Math.pow(10, decimals));
}

export function fromSmallestUnit(amount, decimals = DEFAULT_DECIMALS) {
  return amount / Math.pow(10, decimals);
}
