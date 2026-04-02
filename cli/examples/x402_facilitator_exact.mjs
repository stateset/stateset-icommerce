#!/usr/bin/env node

import http from 'http';
import { createFacilitatorHttpHandler } from '../src/x402/facilitator.js';

const PORT = Number(process.env.X402_FACILITATOR_PORT || 8403);
const FACILITATOR_PRIVATE_KEY = process.env.X402_FACILITATOR_PRIVATE_KEY;
const VERIFY_ONCHAIN = process.env.X402_VERIFY_ONCHAIN !== 'false';

if (!FACILITATOR_PRIVATE_KEY) {
  console.error('X402_FACILITATOR_PRIVATE_KEY is required');
  process.exit(1);
}

const handler = createFacilitatorHttpHandler({
  facilitatorPrivateKey: FACILITATOR_PRIVATE_KEY,
  defaultCheckOnchain: VERIFY_ONCHAIN,
});

http.createServer(handler).listen(PORT, () => {
  console.log(`x402 facilitator listening on http://localhost:${PORT}`);
  console.log('Endpoints: GET /supported, POST /verify, POST /settle');
  console.log(`Onchain verification: ${VERIFY_ONCHAIN ? 'enabled' : 'disabled'}`);
});
