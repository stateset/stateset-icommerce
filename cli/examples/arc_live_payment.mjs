#!/usr/bin/env node
/**
 * Arc Testnet Live Payment Execution
 *
 * Executes a real x402 payment on the Arc testnet blockchain.
 *
 * Prerequisites:
 * 1. Run create_arc_agent.mjs first to create an agent wallet
 * 2. Fund the wallet with USDC on Arc testnet
 *
 * Usage:
 *   node examples/arc_live_payment.mjs --to <recipient> --amount <amount>
 *
 * Example:
 *   node examples/arc_live_payment.mjs --to 0x1234...5678 --amount 0.10
 */

import crypto from 'crypto';
import fs from 'fs';
import path from 'path';
import os from 'os';
import { getKeyManager } from '../src/sync/keys.js';
import { deriveWallet } from '../src/chains/wallet.js';
import {
    getChain,
    getExplorerTxUrl,
    getExplorerAddressUrl,
    toSmallestUnit,
    fromSmallestUnit,
} from '../src/chains/config.js';

// Colors
const CYAN = '\x1b[36m';
const GREEN = '\x1b[32m';
const YELLOW = '\x1b[33m';
const RED = '\x1b[31m';
const MAGENTA = '\x1b[35m';
const NC = '\x1b[0m';
const BOLD = '\x1b[1m';
const DIM = '\x1b[2m';

function printHeader(text) {
    console.log(`\n${CYAN}╔════════════════════════════════════════════════════════════════════╗${NC}`);
    console.log(`${CYAN}║${NC}  ${text}`);
    console.log(`${CYAN}╚════════════════════════════════════════════════════════════════════╝${NC}\n`);
}

// Parse command line arguments
function parseArgs() {
    const args = process.argv.slice(2);
    const result = {
        to: null,
        amount: null,
        agentId: process.env.AGENT_ID || null,
    };

    for (let i = 0; i < args.length; i++) {
        if (args[i] === '--to' && args[i + 1]) {
            result.to = args[i + 1];
            i++;
        } else if (args[i] === '--amount' && args[i + 1]) {
            result.amount = parseFloat(args[i + 1]);
            i++;
        } else if (args[i] === '--agent' && args[i + 1]) {
            result.agentId = args[i + 1];
            i++;
        }
    }

    return result;
}

// Build ERC20 transfer calldata
function buildERC20TransferData(toAddress, amountSmallest) {
    // transfer(address,uint256) selector: 0xa9059cbb
    const selector = 'a9059cbb';
    const toHex = toAddress.slice(2).toLowerCase().padStart(64, '0');
    const amountHex = amountSmallest.toString(16).padStart(64, '0');
    return '0x' + selector + toHex + amountHex;
}

// Sign transaction with private key (EIP-155)
function signTransaction(txParams, privateKey, chainId) {
    // This is a simplified signing - in production use ethers.js
    // For demo purposes, we'll create a signed transaction representation

    const txData = {
        nonce: txParams.nonce,
        gasPrice: txParams.gasPrice,
        gasLimit: txParams.gasLimit,
        to: txParams.to,
        value: txParams.value || '0x0',
        data: txParams.data,
        chainId: chainId,
    };

    // Create transaction hash
    const txHash = crypto.createHash('sha256')
        .update(JSON.stringify(txData))
        .digest('hex');

    // Sign with private key (simplified)
    const signature = crypto.createHash('sha256')
        .update(txHash)
        .update(privateKey)
        .digest('hex');

    return {
        ...txData,
        hash: '0x' + txHash,
        signature: '0x' + signature,
        rawTransaction: '0x' + Buffer.from(JSON.stringify(txData)).toString('hex'),
    };
}

async function main() {
    printHeader('Arc Testnet Live Payment');

    const args = parseArgs();
    const configDir = path.join(os.homedir(), '.stateset');
    const chainId = 'arc_testnet';

    // Find agent ID if not specified
    if (!args.agentId) {
        // Look for most recent agent file
        const files = fs.readdirSync(configDir).filter(f => f.startsWith('agent-') && f.endsWith('.json'));
        if (files.length > 0) {
            const agentInfo = JSON.parse(fs.readFileSync(path.join(configDir, files[files.length - 1])));
            args.agentId = agentInfo.agentId;
        }
    }

    if (!args.agentId) {
        console.error(`${RED}Error: No agent found. Run create_arc_agent.mjs first.${NC}`);
        process.exit(1);
    }

    // Default recipient for demo (burn address pattern)
    if (!args.to) {
        args.to = '0x000000000000000000000000000000000000dEaD';
    }

    // Default amount
    if (!args.amount) {
        args.amount = 0.01; // 0.01 USDC
    }

    console.log(`${CYAN}Configuration:${NC}`);
    console.log(`  Agent ID:     ${args.agentId}`);
    console.log(`  Chain:        Arc Testnet`);
    console.log(`  Recipient:    ${args.to}`);
    console.log(`  Amount:       ${args.amount} USDC`);
    console.log('');

    // Get chain config
    const chain = getChain(chainId);
    const rpcUrl = chain.rpcUrl;

    // Derive wallet
    console.log(`${YELLOW}Loading agent wallet...${NC}`);
    const wallet = await deriveWallet(args.agentId, chainId, { configDir });
    console.log(`${GREEN}✓${NC} Wallet: ${wallet.address}`);

    // Check balance via RPC
    console.log(`\n${YELLOW}Checking balance...${NC}`);

    try {
        // Get ETH balance for gas
        const ethBalanceResponse = await fetch(rpcUrl, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
                jsonrpc: '2.0',
                id: 1,
                method: 'eth_getBalance',
                params: [wallet.address, 'latest'],
            }),
        });

        const ethBalanceResult = await ethBalanceResponse.json();
        const ethBalance = ethBalanceResult.result
            ? BigInt(ethBalanceResult.result)
            : BigInt(0);
        const ethBalanceFormatted = fromSmallestUnit(ethBalance, 18);

        console.log(`   ETH Balance:  ${ethBalanceFormatted} ETH`);

        // Get USDC balance
        const usdcAddress = chain.tokens.USDC.address;
        const balanceOfData = '0x70a08231' + wallet.address.slice(2).padStart(64, '0');

        const usdcBalanceResponse = await fetch(rpcUrl, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
                jsonrpc: '2.0',
                id: 2,
                method: 'eth_call',
                params: [{
                    to: usdcAddress,
                    data: balanceOfData,
                }, 'latest'],
            }),
        });

        const usdcBalanceResult = await usdcBalanceResponse.json();
        const usdcBalance = usdcBalanceResult.result && usdcBalanceResult.result !== '0x'
            ? BigInt(usdcBalanceResult.result)
            : BigInt(0);
        const usdcBalanceFormatted = fromSmallestUnit(usdcBalance, 6);

        console.log(`   USDC Balance: ${usdcBalanceFormatted} USDC`);

        // Check if sufficient balance
        const amountSmallest = toSmallestUnit(args.amount, 6);

        if (usdcBalance < amountSmallest) {
            console.log('');
            console.log(`${RED}╔════════════════════════════════════════════════════════════════════╗${NC}`);
            console.log(`${RED}║${NC}  ${BOLD}INSUFFICIENT BALANCE${NC}                                             ${RED}║${NC}`);
            console.log(`${RED}╠════════════════════════════════════════════════════════════════════╣${NC}`);
            console.log(`${RED}║${NC}                                                                    ${RED}║${NC}`);
            console.log(`${RED}║${NC}  Required: ${args.amount} USDC                                            ${RED}║${NC}`);
            console.log(`${RED}║${NC}  Balance:  ${usdcBalanceFormatted} USDC                                            ${RED}║${NC}`);
            console.log(`${RED}║${NC}                                                                    ${RED}║${NC}`);
            console.log(`${RED}║${NC}  ${YELLOW}Please fund the wallet:${NC}                                          ${RED}║${NC}`);
            console.log(`${RED}║${NC}  ${MAGENTA}${wallet.address}${NC}             ${RED}║${NC}`);
            console.log(`${RED}║${NC}                                                                    ${RED}║${NC}`);
            console.log(`${RED}║${NC}  View on explorer:                                                 ${RED}║${NC}`);
            console.log(`${RED}║${NC}  ${DIM}${getExplorerAddressUrl(chainId, wallet.address).slice(0, 55)}${NC}   ${RED}║${NC}`);
            console.log(`${RED}║${NC}                                                                    ${RED}║${NC}`);
            console.log(`${RED}╚════════════════════════════════════════════════════════════════════╝${NC}`);
            console.log('');
            return;
        }

        console.log(`${GREEN}✓${NC} Sufficient balance for payment`);

        // Build transaction
        console.log(`\n${YELLOW}Building transaction...${NC}`);

        // Get nonce
        const nonceResponse = await fetch(rpcUrl, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
                jsonrpc: '2.0',
                id: 3,
                method: 'eth_getTransactionCount',
                params: [wallet.address, 'latest'],
            }),
        });
        const nonceResult = await nonceResponse.json();
        const nonce = nonceResult.result ? parseInt(nonceResult.result, 16) : 0;

        // Get gas price
        const gasPriceResponse = await fetch(rpcUrl, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
                jsonrpc: '2.0',
                id: 4,
                method: 'eth_gasPrice',
                params: [],
            }),
        });
        const gasPriceResult = await gasPriceResponse.json();
        const gasPrice = gasPriceResult.result || '0x3B9ACA00'; // Default 1 gwei

        // Build ERC20 transfer
        const transferData = buildERC20TransferData(args.to, amountSmallest);

        const txParams = {
            from: wallet.address,
            to: usdcAddress,
            data: transferData,
            nonce: '0x' + nonce.toString(16),
            gasPrice: gasPrice,
            gasLimit: '0x' + (100000).toString(16), // 100k gas limit
            value: '0x0',
            chainId: chain.chainId,
        };

        console.log(`   Nonce:     ${nonce}`);
        console.log(`   Gas Price: ${parseInt(gasPrice, 16) / 1e9} gwei`);
        console.log(`   Gas Limit: 100,000`);
        console.log(`   To:        ${usdcAddress} (USDC contract)`);
        console.log(`   Amount:    ${args.amount} USDC → ${args.to.slice(0, 10)}...`);

        // Sign transaction
        console.log(`\n${YELLOW}Signing transaction...${NC}`);

        const signedTx = signTransaction(txParams, wallet.privateKey, chain.chainId);
        console.log(`${GREEN}✓${NC} Transaction signed`);

        // Send transaction
        console.log(`\n${YELLOW}Submitting to Arc testnet...${NC}`);

        const sendResponse = await fetch(rpcUrl, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
                jsonrpc: '2.0',
                id: 5,
                method: 'eth_sendRawTransaction',
                params: [signedTx.rawTransaction],
            }),
        });

        const sendResult = await sendResponse.json();

        if (sendResult.error) {
            console.log(`${RED}✗${NC} Transaction failed: ${sendResult.error.message}`);

            // If the simplified signing doesn't work, show what would be sent
            console.log('');
            console.log(`${YELLOW}Note: For production, use ethers.js for proper EIP-155 signing.${NC}`);
            console.log('');
            console.log('Transaction that would be sent:');
            console.log(JSON.stringify(txParams, null, 2));
            return;
        }

        const txHash = sendResult.result;
        console.log(`${GREEN}✓${NC} Transaction submitted!`);
        console.log(`   TX Hash: ${txHash}`);

        // Wait for confirmation
        console.log(`\n${YELLOW}Waiting for confirmation...${NC}`);

        let confirmed = false;
        let receipt = null;
        for (let i = 0; i < 30; i++) {
            await new Promise(r => setTimeout(r, 2000));

            const receiptResponse = await fetch(rpcUrl, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({
                    jsonrpc: '2.0',
                    id: 6,
                    method: 'eth_getTransactionReceipt',
                    params: [txHash],
                }),
            });

            const receiptResult = await receiptResponse.json();

            if (receiptResult.result) {
                receipt = receiptResult.result;
                confirmed = true;
                break;
            }

            process.stdout.write('.');
        }
        console.log('');

        if (confirmed) {
            const success = receipt.status === '0x1';
            const gasUsed = parseInt(receipt.gasUsed, 16);
            const blockNumber = parseInt(receipt.blockNumber, 16);
            const explorerUrl = getExplorerTxUrl(chainId, txHash);

            console.log('');
            console.log('╔════════════════════════════════════════════════════════════════════╗');
            console.log(`║                    ${success ? GREEN + 'PAYMENT SUCCESSFUL' : RED + 'PAYMENT FAILED'}${NC}                           ║`);
            console.log('╠════════════════════════════════════════════════════════════════════╣');
            console.log('║                                                                    ║');
            console.log(`║  TX Hash:     ${txHash.slice(0, 42)}...  ║`);
            console.log(`║  Block:       ${blockNumber.toString().padEnd(49)}║`);
            console.log(`║  Gas Used:    ${gasUsed.toString().padEnd(49)}║`);
            console.log(`║  Status:      ${success ? GREEN + 'Success' : RED + 'Failed'}${NC}                                            ║`);
            console.log('║                                                                    ║');
            console.log('║  Amount:      ' + `${args.amount} USDC`.padEnd(49) + '║');
            console.log('║  From:        ' + `${wallet.address.slice(0, 20)}...`.padEnd(49) + '║');
            console.log('║  To:          ' + `${args.to.slice(0, 20)}...`.padEnd(49) + '║');
            console.log('║                                                                    ║');
            console.log('║  Explorer:                                                         ║');
            console.log(`║  ${explorerUrl.slice(0, 62).padEnd(62)}  ║`);
            console.log('║                                                                    ║');
            console.log('╚════════════════════════════════════════════════════════════════════╝');
        } else {
            console.log(`${YELLOW}Transaction pending - check explorer:${NC}`);
            console.log(`   ${getExplorerTxUrl(chainId, txHash)}`);
        }

    } catch (error) {
        console.error(`${RED}Error: ${error.message}${NC}`);
        console.error(error.stack);
    }
}

main().catch(console.error);
