// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.24;

import "forge-std/Test.sol";
import {ICPEscrow} from "../src/ICPEscrow.sol";
import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import {ERC20} from "@openzeppelin/contracts/token/ERC20/ERC20.sol";

/// @notice Mock USDC for unit tests. Real USDC has 6 decimals and a more complex implementation;
///         the lifecycle tests only need transfer / approve / balance semantics.
contract MockUSDC is ERC20 {
    constructor() ERC20("Mock USD Coin", "USDC") {}
    function decimals() public pure override returns (uint8) { return 6; }
    function mint(address to, uint256 amount) external { _mint(to, amount); }
}

contract ICPEscrowTest is Test {
    MockUSDC usdc;
    ICPEscrow escrow;

    address admin = address(0xA001);
    address arbiter = address(0xA002);
    address pauser = address(0xA003);

    address buyer = address(0xB001);
    address merchant = address(0xB002);
    address relayer = address(0xB003);
    address attacker = address(0xB004);

    bytes32 constant ESCROW_ID = bytes32(uint256(0xE5C807));
    bytes32 constant QUOTE_HASH = bytes32(uint256(0xCAFE));
    bytes32 constant FULFILLMENT_HASH = bytes32(uint256(0xFFFF));
    bytes32 constant ARBITER_DECISION = bytes32(uint256(0xDEC1));

    uint128 constant AMOUNT = 100_000_000; // 100 USDC (6 decimals)

    function setUp() public {
        usdc = new MockUSDC();
        escrow = new ICPEscrow(IERC20(address(usdc)), admin, arbiter, pauser);

        usdc.mint(buyer, 1_000_000_000); // 1000 USDC
        vm.prank(buyer);
        usdc.approve(address(escrow), type(uint256).max);
    }

    // ---------------------------------------------------------------------
    // Happy path: fund → wait → release
    // ---------------------------------------------------------------------

    function test_fundAndRelease() public {
        uint64 deadline = uint64(block.timestamp + 1 days);
        uint64 disputeWindow = 7 days;

        vm.prank(buyer);
        escrow.fund(ESCROW_ID, buyer, merchant, AMOUNT, deadline, disputeWindow, QUOTE_HASH);

        assertEq(uint8(escrow.escrowState(ESCROW_ID)), uint8(ICPEscrow.State.Funded));
        assertEq(usdc.balanceOf(address(escrow)), AMOUNT);
        assertEq(usdc.balanceOf(merchant), 0);

        // Time travel past fulfillment deadline + dispute window
        vm.warp(deadline + disputeWindow + 1);

        // Anyone can call release — relayer gas sponsorship pattern
        vm.prank(relayer);
        escrow.release(ESCROW_ID, FULFILLMENT_HASH);

        assertEq(uint8(escrow.escrowState(ESCROW_ID)), uint8(ICPEscrow.State.Released));
        assertEq(usdc.balanceOf(merchant), AMOUNT);
        assertEq(usdc.balanceOf(address(escrow)), 0);
    }

    // ---------------------------------------------------------------------
    // Release before time-lock MUST revert
    // ---------------------------------------------------------------------

    function test_releaseBeforeTimelockReverts() public {
        uint64 deadline = uint64(block.timestamp + 1 days);
        uint64 disputeWindow = 7 days;

        vm.prank(buyer);
        escrow.fund(ESCROW_ID, buyer, merchant, AMOUNT, deadline, disputeWindow, QUOTE_HASH);

        vm.warp(deadline + disputeWindow - 1); // 1 second too early
        vm.expectRevert();
        vm.prank(relayer);
        escrow.release(ESCROW_ID, FULFILLMENT_HASH);
    }

    // ---------------------------------------------------------------------
    // Dispute path
    // ---------------------------------------------------------------------

    function test_buyerCanDispute() public {
        _fundDefault();
        vm.prank(buyer);
        escrow.dispute(ESCROW_ID, "wrong item shipped");
        assertEq(uint8(escrow.escrowState(ESCROW_ID)), uint8(ICPEscrow.State.Disputed));
    }

    function test_merchantCanDispute() public {
        _fundDefault();
        vm.prank(merchant);
        escrow.dispute(ESCROW_ID, "buyer chargeback fraud");
        assertEq(uint8(escrow.escrowState(ESCROW_ID)), uint8(ICPEscrow.State.Disputed));
    }

    function test_attackerCannotDispute() public {
        _fundDefault();
        vm.expectRevert();
        vm.prank(attacker);
        escrow.dispute(ESCROW_ID, "lol");
    }

    function test_disputeBlocksRelease() public {
        _fundDefault();
        vm.prank(buyer);
        escrow.dispute(ESCROW_ID, "x");

        vm.warp(block.timestamp + 30 days);
        vm.expectRevert();
        escrow.release(ESCROW_ID, FULFILLMENT_HASH);
    }

    // ---------------------------------------------------------------------
    // Arbiter resolution
    // ---------------------------------------------------------------------

    function test_arbiterResolvesToMerchant() public {
        _fundDefault();
        vm.prank(buyer);
        escrow.dispute(ESCROW_ID, "x");

        vm.prank(arbiter);
        escrow.arbiterResolve(ESCROW_ID, merchant, ARBITER_DECISION);
        assertEq(uint8(escrow.escrowState(ESCROW_ID)), uint8(ICPEscrow.State.Released));
        assertEq(usdc.balanceOf(merchant), AMOUNT);
    }

    function test_arbiterResolvesToBuyer() public {
        _fundDefault();
        vm.prank(merchant);
        escrow.dispute(ESCROW_ID, "x");

        vm.prank(arbiter);
        escrow.arbiterResolve(ESCROW_ID, buyer, ARBITER_DECISION);
        assertEq(uint8(escrow.escrowState(ESCROW_ID)), uint8(ICPEscrow.State.Refunded));
        assertEq(usdc.balanceOf(buyer), 1_000_000_000); // back to original
    }

    function test_arbiterCannotRedirectToThirdParty() public {
        _fundDefault();
        vm.prank(buyer);
        escrow.dispute(ESCROW_ID, "x");

        vm.expectRevert();
        vm.prank(arbiter);
        escrow.arbiterResolve(ESCROW_ID, attacker, ARBITER_DECISION);
    }

    function test_nonArbiterCannotResolve() public {
        _fundDefault();
        vm.prank(buyer);
        escrow.dispute(ESCROW_ID, "x");

        vm.expectRevert();
        vm.prank(attacker);
        escrow.arbiterResolve(ESCROW_ID, merchant, ARBITER_DECISION);
    }

    // ---------------------------------------------------------------------
    // Refund path
    // ---------------------------------------------------------------------

    function test_merchantCanRefund() public {
        _fundDefault();
        vm.prank(merchant);
        escrow.refund(ESCROW_ID, "out of stock");
        assertEq(uint8(escrow.escrowState(ESCROW_ID)), uint8(ICPEscrow.State.Refunded));
        assertEq(usdc.balanceOf(buyer), 1_000_000_000);
    }

    function test_buyerCannotRefund() public {
        _fundDefault();
        vm.expectRevert();
        vm.prank(buyer);
        escrow.refund(ESCROW_ID, "i changed my mind");
    }

    // ---------------------------------------------------------------------
    // Reuse / collision
    // ---------------------------------------------------------------------

    function test_cannotReuseEscrowId() public {
        _fundDefault();
        vm.expectRevert();
        vm.prank(buyer);
        escrow.fund(
            ESCROW_ID, buyer, merchant, AMOUNT,
            uint64(block.timestamp + 1 days), 1 days, QUOTE_HASH
        );
    }

    // ---------------------------------------------------------------------
    // Pause
    // ---------------------------------------------------------------------

    function test_pauseBlocksFund() public {
        vm.prank(pauser);
        escrow.pause();

        vm.expectRevert();
        vm.prank(buyer);
        escrow.fund(
            ESCROW_ID, buyer, merchant, AMOUNT,
            uint64(block.timestamp + 1 days), 1 days, QUOTE_HASH
        );
    }

    function test_unauthorizedPauseReverts() public {
        vm.expectRevert();
        vm.prank(attacker);
        escrow.pause();
    }

    // ---------------------------------------------------------------------
    // Helpers
    // ---------------------------------------------------------------------

    function _fundDefault() internal {
        uint64 deadline = uint64(block.timestamp + 1 days);
        uint64 disputeWindow = 7 days;
        vm.prank(buyer);
        escrow.fund(ESCROW_ID, buyer, merchant, AMOUNT, deadline, disputeWindow, QUOTE_HASH);
    }
}
