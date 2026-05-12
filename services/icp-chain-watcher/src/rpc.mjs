// Minimal JSON-RPC 2.0 client over fetch. Zero dependencies.
//
// Implements only the methods this watcher needs:
//   - eth_blockNumber
//   - eth_getLogs
//   - eth_getBlockByNumber (for finality lag tracking)

export class RpcClient {
  constructor(url) {
    this.url = url;
    this._id = 1;
  }

  async call(method, params = []) {
    const id = this._id++;
    const r = await fetch(this.url, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ jsonrpc: '2.0', id, method, params }),
    });
    if (!r.ok) throw new Error(`rpc HTTP ${r.status}: ${method}`);
    const j = await r.json();
    if (j.error) throw new Error(`rpc error ${j.error.code}: ${j.error.message}`);
    return j.result;
  }

  /** @returns {Promise<number>} current head block number */
  async blockNumber() {
    const hex = await this.call('eth_blockNumber');
    return parseInt(hex, 16);
  }

  /**
   * @param {object} filter eth_getLogs filter
   *   filter.fromBlock — hex string or "earliest"/"latest"
   *   filter.toBlock   — hex string or "latest"
   *   filter.address   — contract address (hex string)
   *   filter.topics    — array of topic filters
   */
  async getLogs(filter) {
    const f = {
      ...filter,
      fromBlock: typeof filter.fromBlock === 'number' ? `0x${filter.fromBlock.toString(16)}` : filter.fromBlock,
      toBlock: typeof filter.toBlock === 'number' ? `0x${filter.toBlock.toString(16)}` : filter.toBlock,
    };
    return this.call('eth_getLogs', [f]);
  }
}
