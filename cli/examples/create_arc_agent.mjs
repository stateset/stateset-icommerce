#!/usr/bin/env node
/**
 * Create AI Agent Wallet for Arc Testnet
 *
 * This script:
 * 1. Creates an AI agent with a unique ID
 * 2. Generates VES Ed25519 signing keys
 * 3. Derives an EVM wallet for Arc testnet
 * 4. Displays the address for funding
 *
 * Usage:
 *   node examples/create_arc_agent.mjs
 *
 * After funding, run:
 *   node examples/arc_live_payment.mjs
 */

import crypto from 'crypto';
import fs from 'fs';
import path from 'path';
import os from 'os';
import { getKeyManager } from '../src/sync/keys.js';
import { deriveWallet, getOrCreateWallet } from '../src/chains/wallet.js';
import { getChain, getExplorerAddressUrl } from '../src/chains/config.js';

// Colors
const CYAN = '\x1b[36m';
const GREEN = '\x1b[32m';
const YELLOW = '\x1b[33m';
const MAGENTA = '\x1b[35m';
const NC = '\x1b[0m';
const BOLD = '\x1b[1m';

function printHeader(text) {
    console.log(`\n${CYAN}╔════════════════════════════════════════════════════════════════════╗${NC}`);
    console.log(`${CYAN}║${NC}  ${text}`);
    console.log(`${CYAN}╚════════════════════════════════════════════════════════════════════╝${NC}\n`);
}

async function main() {
    printHeader('Create AI Agent Wallet for Arc Testnet');

    // Configuration
    const configDir = path.join(os.homedir(), '.stateset');
    const agentId = process.env.AGENT_ID || `agent-${crypto.randomBytes(4).toString('hex')}`;
    const chainId = 'arc_testnet';

    console.log(`${CYAN}Configuration:${NC}`);
    console.log(`  Config Dir:   ${configDir}`);
    console.log(`  Agent ID:     ${agentId}`);
    console.log(`  Chain:        Arc Testnet (${chainId})`);
    console.log('');

    // Ensure config directory exists
    if (!fs.existsSync(configDir)) {
        fs.mkdirSync(configDir, { recursive: true });
        console.log(`${GREEN}✓${NC} Created config directory: ${configDir}`);
    }

    // Get or create key manager
    const keyManager = getKeyManager(configDir);

    // Check if agent already has keys
    let existingKey = null;
    try {
        existingKey = await keyManager.getCurrentSigningKey(agentId);
    } catch (e) {
        // No keys yet
    }

    if (existingKey) {
        console.log(`${YELLOW}ℹ${NC} Agent ${agentId} already has keys`);
        console.log(`   Key ID: ${existingKey.keyId}`);
    } else {
        // Generate new keys
        console.log(`\n${YELLOW}Generating VES keys...${NC}`);
        await keyManager.ensureKeys(agentId);
        const newKey = await keyManager.getCurrentSigningKey(agentId);
        console.log(`${GREEN}✓${NC} Generated Ed25519 signing key`);
        console.log(`   Key ID: ${newKey.keyId}`);
    }

    // Derive wallet for Arc testnet
    console.log(`\n${YELLOW}Deriving Arc testnet wallet...${NC}`);

    const wallet = await deriveWallet(agentId, chainId, { configDir });

    const chain = getChain(chainId);
    const explorerUrl = getExplorerAddressUrl(chainId, wallet.address);

    console.log(`${GREEN}✓${NC} Wallet derived successfully`);
    console.log('');

    // Display wallet info
    console.log('╔════════════════════════════════════════════════════════════════════╗');
    console.log('║                    AI AGENT WALLET CREATED                         ║');
    console.log('╠════════════════════════════════════════════════════════════════════╣');
    console.log('║                                                                    ║');
    console.log(`║  ${BOLD}Agent ID:${NC}     ${agentId.padEnd(43)}║`);
    console.log('║                                                                    ║');
    console.log(`║  ${BOLD}Network:${NC}      Arc Testnet (Chain ID: ${chain.chainId})                  ║`);
    console.log(`║  ${BOLD}RPC URL:${NC}      ${chain.rpcUrl.slice(0, 40).padEnd(40)}   ║`);
    console.log('║                                                                    ║');
    console.log('║  ─────────────────────────────────────────────────────────────     ║');
    console.log('║                                                                    ║');
    console.log(`║  ${BOLD}${GREEN}WALLET ADDRESS:${NC}                                                   ║`);
    console.log(`║  ${MAGENTA}${wallet.address}${NC}             ║`);
    console.log('║                                                                    ║');
    console.log('║  ─────────────────────────────────────────────────────────────     ║');
    console.log('║                                                                    ║');
    console.log(`║  ${BOLD}Explorer:${NC}                                                        ║`);
    console.log(`║  ${explorerUrl.slice(0, 60).padEnd(60)}   ║`);
    console.log('║                                                                    ║');
    console.log('╚════════════════════════════════════════════════════════════════════╝');
    console.log('');

    // Instructions for funding
    console.log(`${YELLOW}To fund this wallet:${NC}`);
    console.log('');
    console.log('  1. Go to the Arc testnet faucet or transfer USDC to the address above');
    console.log('  2. Arc testnet uses USDC for gas fees');
    console.log('  3. You can view the wallet at:');
    console.log(`     ${CYAN}${explorerUrl}${NC}`);
    console.log('');

    // Save agent info to file for later use
    const agentInfoPath = path.join(configDir, `agent-${agentId}.json`);
    const agentInfo = {
        agentId,
        chainId,
        address: wallet.address,
        derivationPath: wallet.derivationPath,
        createdAt: new Date().toISOString(),
        explorerUrl,
    };

    fs.writeFileSync(agentInfoPath, JSON.stringify(agentInfo, null, 2));
    console.log(`${GREEN}✓${NC} Agent info saved to: ${agentInfoPath}`);
    console.log('');

    // Export for use in other scripts
    console.log(`${YELLOW}To use this agent in other scripts:${NC}`);
    console.log(`  export AGENT_ID="${agentId}"`);
    console.log('');

    return {
        agentId,
        address: wallet.address,
        chainId,
        explorerUrl,
    };
}

main()
    .then(result => {
        console.log(`${GREEN}✅ Agent wallet ready!${NC}`);
        console.log(`   Address: ${result.address}`);
    })
    .catch(err => {
        console.error(`Error: ${err.message}`);
        process.exit(1);
    });
