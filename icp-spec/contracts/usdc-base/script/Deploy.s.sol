// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.24;

import "forge-std/Script.sol";
import {ICPEscrow} from "../src/ICPEscrow.sol";
import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";

/// @notice Deployment script for the ICPEscrow contract.
///         Two profiles: `mainnet` (Base) and `testnet` (Base Sepolia, used by the StateSet
///         bootstrap Settler).
///
/// Usage:
///   forge script script/Deploy.s.sol \
///     --rpc-url $BASE_RPC \
///     --broadcast \
///     --verify \
///     --etherscan-api-key $BASESCAN_KEY \
///     --sig "deployTestnet()"
contract Deploy is Script {
    // Base L2 mainnet
    address constant USDC_BASE_MAINNET = 0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913;

    // Base Sepolia testnet (Circle test USDC)
    address constant USDC_BASE_SEPOLIA = 0x036CbD53842c5426634e7929541eC2318f3dCF7e;

    /// @notice Mainnet deployment. Admin / arbiter / pauser MUST be Safes (not EOAs).
    function deployMainnet() external {
        address admin = vm.envAddress("ADMIN_SAFE");
        address arbiter = vm.envAddress("ARBITER_SAFE");
        address pauser = vm.envAddress("PAUSER_SAFE");

        require(admin != address(0) && arbiter != address(0) && pauser != address(0),
            "set ADMIN_SAFE / ARBITER_SAFE / PAUSER_SAFE env vars");

        vm.startBroadcast();
        ICPEscrow icp = new ICPEscrow(IERC20(USDC_BASE_MAINNET), admin, arbiter, pauser);
        vm.stopBroadcast();

        console.log("ICPEscrow deployed (mainnet):", address(icp));
        console.log("USDC:", USDC_BASE_MAINNET);
        console.log("Admin Safe:", admin);
        console.log("Arbiter Safe:", arbiter);
        console.log("Pauser Safe:", pauser);
    }

    /// @notice Testnet deployment. Single-EOA admin/arbiter/pauser is acceptable.
    function deployTestnet() external {
        address deployer = vm.envAddress("DEPLOYER");
        require(deployer != address(0), "set DEPLOYER env var");

        vm.startBroadcast();
        ICPEscrow icp = new ICPEscrow(
            IERC20(USDC_BASE_SEPOLIA),
            deployer,
            deployer,
            deployer
        );
        vm.stopBroadcast();

        console.log("ICPEscrow deployed (testnet):", address(icp));
        console.log("USDC (testnet):", USDC_BASE_SEPOLIA);
        console.log("Admin / arbiter / pauser (single EOA):", deployer);
    }
}
