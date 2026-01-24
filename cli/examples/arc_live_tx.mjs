#!/usr/bin/env node
/**
 * Arc Testnet Live Transaction with ethers.js
 *
 * Executes a real USDC transfer on Arc testnet.
 */

import { ethers } from 'ethers';
import fs from 'fs';
import path from 'path';
import os from 'os';
import { deriveWallet } from '../src/chains/wallet.js';
import { getChain } from '../src/chains/config.js';

const CYAN = '\x1b[36m';
const GREEN = '\x1b[32m';
const YELLOW = '\x1b[33m';
const MAGENTA = '\x1b[35m';
const NC = '\x1b[0m';

async function main() {
    console.log(`\n${CYAN}╔════════════════════════════════════════════════════════════════════╗${NC}`);
    console.log(`${CYAN}║${NC}  Arc Testnet Live Transaction`);
    console.log(`${CYAN}╚════════════════════════════════════════════════════════════════════╝${NC}\n`);

    const configDir = path.join(os.homedir(), '.stateset');
    const chainId = 'arc_testnet';

    // Find agent
    const files = fs.readdirSync(configDir).filter(f => f.startsWith('agent-') && f.endsWith('.json'));
    if (files.length === 0) {
        console.error('No agent found. Run create_arc_agent.mjs first.');
        process.exit(1);
    }

    const agentInfo = JSON.parse(fs.readFileSync(path.join(configDir, files[files.length - 1])));
    const agentId = agentInfo.agentId;

    // Get chain config
    const chain = getChain(chainId);

    // Derive wallet
    console.log(`${YELLOW}Loading agent wallet...${NC}`);
    const walletData = await deriveWallet(agentId, chainId, { configDir });

    // Create ethers provider and wallet
    const provider = new ethers.JsonRpcProvider(chain.rpcUrl, {
        chainId: chain.chainId,
        name: 'arc-testnet'
    });

    // Convert Buffer to hex string for ethers.js
    const privateKeyHex = '0x' + walletData.privateKey.toString('hex');
    const wallet = new ethers.Wallet(privateKeyHex, provider);

    console.log(`${GREEN}✓${NC} Wallet: ${wallet.address}`);

    // Check balances
    console.log(`\n${YELLOW}Checking balances...${NC}`);

    const ethBalance = await provider.getBalance(wallet.address);
    console.log(`   ETH Balance:  ${ethers.formatEther(ethBalance)} ETH`);

    // USDC contract
    const usdcAddress = chain.tokens.USDC.address;
    const usdcAbi = [
        'function balanceOf(address) view returns (uint256)',
        'function transfer(address to, uint256 amount) returns (bool)',
        'function decimals() view returns (uint8)'
    ];

    const usdc = new ethers.Contract(usdcAddress, usdcAbi, wallet);
    const usdcBalance = await usdc.balanceOf(wallet.address);
    const decimals = await usdc.decimals();

    console.log(`   USDC Balance: ${ethers.formatUnits(usdcBalance, decimals)} USDC`);
    console.log(`   USDC Contract: ${usdcAddress}`);

    // Payment parameters
    const recipient = '0x000000000000000000000000000000000000dEaD';
    const amount = ethers.parseUnits('1.0', decimals); // 1 USDC

    console.log(`\n${YELLOW}Preparing transaction...${NC}`);
    console.log(`   To:     ${recipient}`);
    console.log(`   Amount: 1.0 USDC`);

    // Check sufficient balance
    if (usdcBalance < amount) {
        console.error(`\n${YELLOW}Insufficient USDC balance${NC}`);
        return;
    }

    // Execute transfer
    console.log(`\n${YELLOW}Sending transaction...${NC}`);

    try {
        const tx = await usdc.transfer(recipient, amount);
        console.log(`${GREEN}✓${NC} Transaction submitted!`);
        console.log(`   TX Hash: ${tx.hash}`);
        console.log(`   Explorer: ${chain.explorerUrl}/tx/${tx.hash}`);

        console.log(`\n${YELLOW}Waiting for confirmation...${NC}`);
        const receipt = await tx.wait();

        console.log(`\n${GREEN}╔════════════════════════════════════════════════════════════════════╗${NC}`);
        console.log(`${GREEN}║${NC}  ${GREEN}TRANSACTION CONFIRMED!${NC}                                          ${GREEN}║${NC}`);
        console.log(`${GREEN}╠════════════════════════════════════════════════════════════════════╣${NC}`);
        console.log(`${GREEN}║${NC}                                                                    ${GREEN}║${NC}`);
        console.log(`${GREEN}║${NC}  TX Hash:   ${tx.hash.slice(0,42)}...  ${GREEN}║${NC}`);
        console.log(`${GREEN}║${NC}  Block:     ${receipt.blockNumber.toString().padEnd(50)}${GREEN}║${NC}`);
        console.log(`${GREEN}║${NC}  Gas Used:  ${receipt.gasUsed.toString().padEnd(50)}${GREEN}║${NC}`);
        console.log(`${GREEN}║${NC}  Status:    ${receipt.status === 1 ? 'Success' : 'Failed'}                                            ${GREEN}║${NC}`);
        console.log(`${GREEN}║${NC}                                                                    ${GREEN}║${NC}`);
        console.log(`${GREEN}║${NC}  Amount:    1.0 USDC                                               ${GREEN}║${NC}`);
        console.log(`${GREEN}║${NC}  From:      ${wallet.address.slice(0,20)}...                       ${GREEN}║${NC}`);
        console.log(`${GREEN}║${NC}  To:        ${recipient.slice(0,20)}...                       ${GREEN}║${NC}`);
        console.log(`${GREEN}║${NC}                                                                    ${GREEN}║${NC}`);
        console.log(`${GREEN}║${NC}  Explorer:                                                         ${GREEN}║${NC}`);
        console.log(`${GREEN}║${NC}  ${chain.explorerUrl}/tx/${tx.hash.slice(0,30)}...  ${GREEN}║${NC}`);
        console.log(`${GREEN}║${NC}                                                                    ${GREEN}║${NC}`);
        console.log(`${GREEN}╚════════════════════════════════════════════════════════════════════╝${NC}`);

    } catch (error) {
        console.error(`\nTransaction failed: ${error.message}`);
        if (error.data) {
            console.error(`Error data: ${error.data}`);
        }
    }
}

main().catch(console.error);
