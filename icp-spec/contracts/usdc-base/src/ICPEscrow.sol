// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.24;

import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import {SafeERC20} from "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";
import {AccessControl} from "@openzeppelin/contracts/access/AccessControl.sol";
import {Pausable} from "@openzeppelin/contracts/utils/Pausable.sol";
import {ReentrancyGuard} from "@openzeppelin/contracts/utils/ReentrancyGuard.sol";

/// @title ICPEscrow — on-chain custody for the `settler:circle.usdc.base` ICP Settler binding
/// @notice Implements the on-chain side of the ICP-1.0 escrow state machine over USDC on Base L2.
/// @dev Off-chain attestations (fulfillment evidence, EscrowEvent emission) are the Settler operator's
///      responsibility. This contract is intentionally minimal: it custodies USDC, enforces the
///      lifecycle invariants, and emits events that an off-chain Settler subscribes to in order to
///      produce signed ICP EscrowEvents and SettlementReceipts.
///
///      State machine (matches ICP-1.0 §8 sans the off-chain `fulfilled` state):
///
///        None ──fund()──▶ Funded ──release()──▶ Released
///                            │
///                            ├──dispute()──▶ Disputed ──arbiterResolve()──▶ Released | Refunded
///                            │
///                            └──refund()──▶ Refunded
///
///      `fulfilled` is an off-chain Settler attestation; on-chain it is a precondition gate to
///      release(), encoded by the `fulfillmentDeadline + disputeWindow` time-lock.
contract ICPEscrow is AccessControl, Pausable, ReentrancyGuard {
    using SafeERC20 for IERC20;

    // -------------------------------------------------------------------
    // Roles
    // -------------------------------------------------------------------

    /// @notice Role allowed to resolve disputes. Held by the Foundation-published arbiter set.
    bytes32 public constant ARBITER_ROLE = keccak256("ICP_ARBITER_ROLE");

    /// @notice Role allowed to pause the contract for compliance reasons (Circle's prudential stop).
    bytes32 public constant PAUSER_ROLE = keccak256("ICP_PAUSER_ROLE");

    // -------------------------------------------------------------------
    // Storage
    // -------------------------------------------------------------------

    enum State {
        None,
        Funded,
        Disputed,
        Released,
        Refunded
    }

    struct Escrow {
        State state;
        address buyer;
        address merchant;
        uint128 amount;              // USDC has 6 decimals; uint128 is a comfortable headroom
        uint64 fundedAt;             // unix seconds
        uint64 fulfillmentDeadline;  // unix seconds; merchant SHOULD fulfill by this time
        uint64 disputeWindow;        // seconds after fulfillmentDeadline before release is permitted
        bytes32 quoteHash;           // keccak256(JCS(Quote)) — RFC 8785 canonical JSON per ICP-1.0 §5.1; off-chain proof anchor
    }

    /// @notice The USDC token contract.
    IERC20 public immutable USDC;

    /// @notice escrowId → Escrow. escrowId is computed off-chain (typically keccak256(intent_id || quote_id)).
    mapping(bytes32 => Escrow) public escrows;

    // -------------------------------------------------------------------
    // Events (one per ICP-1.0 escrow state transition)
    //
    // These are the on-chain anchors for the off-chain Settler's signed EscrowEvent.
    // The Settler subscribes via web3 RPC, builds an EscrowEvent referencing
    // (block_number, tx_hash), signs it with the Settler key, and broadcasts to ICP
    // counterparties. See SETTLERS.md §S.2.
    // -------------------------------------------------------------------

    event EscrowFunded(
        bytes32 indexed escrowId,
        address indexed buyer,
        address indexed merchant,
        uint128 amount,
        uint64 fulfillmentDeadline,
        uint64 disputeWindow,
        bytes32 quoteHash
    );

    event EscrowDisputed(bytes32 indexed escrowId, address indexed by, string reason);

    event EscrowReleased(
        bytes32 indexed escrowId,
        address indexed merchant,
        uint128 amount,
        bytes32 fulfillmentReceiptHash
    );

    event EscrowRefunded(
        bytes32 indexed escrowId,
        address indexed buyer,
        uint128 amount,
        string reason
    );

    event EscrowResolved(
        bytes32 indexed escrowId,
        address indexed beneficiary,
        uint128 amount,
        bytes32 arbitrationDecisionHash
    );

    // -------------------------------------------------------------------
    // Errors
    // -------------------------------------------------------------------

    error EscrowAlreadyExists(bytes32 escrowId);
    error EscrowNotFound(bytes32 escrowId);
    error EscrowWrongState(bytes32 escrowId, State expected, State actual);
    error AmountZero();
    error MerchantZero();
    error BuyerZero();
    error DeadlineInPast(uint64 deadline, uint64 nowUnix);
    error DisputeWindowZero();
    error ReleaseTooEarly(uint64 earliestRelease, uint64 nowUnix);
    error UnauthorizedCaller(address caller, address required);
    error ArbiterDecisionInvalid(address beneficiary);

    // -------------------------------------------------------------------
    // Construction
    // -------------------------------------------------------------------

    /// @param usdc          USDC token address. Base mainnet: 0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913
    /// @param admin         Initial DEFAULT_ADMIN_ROLE holder (typically the 5-of-9 Safe).
    /// @param arbiterSet    Initial ARBITER_ROLE holder (typically the Foundation arbiter contract).
    /// @param pauser        Initial PAUSER_ROLE holder (typically the Settler operator's compliance Safe).
    constructor(
        IERC20 usdc,
        address admin,
        address arbiterSet,
        address pauser
    ) {
        USDC = usdc;
        _grantRole(DEFAULT_ADMIN_ROLE, admin);
        _grantRole(ARBITER_ROLE, arbiterSet);
        _grantRole(PAUSER_ROLE, pauser);
    }

    // -------------------------------------------------------------------
    // Lifecycle
    // -------------------------------------------------------------------

    /// @notice Fund a new escrow. Caller is the funder (typically the buyer's wallet) and
    ///         must have approved this contract to spend `amount` USDC.
    /// @dev    Computes `escrowId` deterministically off-chain so that ICP Intent + Quote
    ///         can reference it before any on-chain action. The contract MUST reject reuse.
    function fund(
        bytes32 escrowId,
        address buyer,
        address merchant,
        uint128 amount,
        uint64 fulfillmentDeadline,
        uint64 disputeWindow,
        bytes32 quoteHash
    ) external whenNotPaused nonReentrant {
        if (amount == 0) revert AmountZero();
        if (merchant == address(0)) revert MerchantZero();
        if (buyer == address(0)) revert BuyerZero();
        if (fulfillmentDeadline <= block.timestamp) {
            revert DeadlineInPast(fulfillmentDeadline, uint64(block.timestamp));
        }
        if (disputeWindow == 0) revert DisputeWindowZero();
        if (escrows[escrowId].state != State.None) revert EscrowAlreadyExists(escrowId);

        escrows[escrowId] = Escrow({
            state: State.Funded,
            buyer: buyer,
            merchant: merchant,
            amount: amount,
            fundedAt: uint64(block.timestamp),
            fulfillmentDeadline: fulfillmentDeadline,
            disputeWindow: disputeWindow,
            quoteHash: quoteHash
        });

        USDC.safeTransferFrom(msg.sender, address(this), amount);

        emit EscrowFunded(escrowId, buyer, merchant, amount, fulfillmentDeadline, disputeWindow, quoteHash);
    }

    /// @notice Release escrow to the merchant. Permitted only after
    ///         `fulfillmentDeadline + disputeWindow` has elapsed and no dispute is open.
    /// @dev    Anyone can call this — releases are deterministic given chain state, so a
    ///         relayer can poke the contract on the merchant's behalf. The merchant doesn't
    ///         need to be online at the moment of release.
    function release(bytes32 escrowId, bytes32 fulfillmentReceiptHash)
        external
        whenNotPaused
        nonReentrant
    {
        Escrow storage e = _mustExist(escrowId);
        if (e.state != State.Funded) {
            revert EscrowWrongState(escrowId, State.Funded, e.state);
        }
        uint64 earliest = e.fulfillmentDeadline + e.disputeWindow;
        if (block.timestamp < earliest) {
            revert ReleaseTooEarly(earliest, uint64(block.timestamp));
        }

        uint128 amount = e.amount;
        address merchant = e.merchant;
        e.state = State.Released;
        e.amount = 0; // zero before transfer for additional safety

        USDC.safeTransfer(merchant, amount);

        emit EscrowReleased(escrowId, merchant, amount, fulfillmentReceiptHash);
    }

    /// @notice Open a dispute. Either buyer or merchant may call. Locks the escrow until
    ///         arbiter resolution.
    /// @param  reason free-form short string. Off-chain DisputeIntent is the canonical record;
    ///         this is a human-readable hint surfaced in block explorers.
    function dispute(bytes32 escrowId, string calldata reason) external whenNotPaused {
        Escrow storage e = _mustExist(escrowId);
        if (e.state != State.Funded) {
            revert EscrowWrongState(escrowId, State.Funded, e.state);
        }
        if (msg.sender != e.buyer && msg.sender != e.merchant) {
            revert UnauthorizedCaller(msg.sender, e.buyer);
        }

        e.state = State.Disputed;
        emit EscrowDisputed(escrowId, msg.sender, reason);
    }

    /// @notice Merchant-initiated refund (cancellation). Only the merchant can call this.
    ///         Cannot be called once disputed.
    function refund(bytes32 escrowId, string calldata reason)
        external
        whenNotPaused
        nonReentrant
    {
        Escrow storage e = _mustExist(escrowId);
        if (e.state != State.Funded) {
            revert EscrowWrongState(escrowId, State.Funded, e.state);
        }
        if (msg.sender != e.merchant) {
            revert UnauthorizedCaller(msg.sender, e.merchant);
        }

        uint128 amount = e.amount;
        address buyer = e.buyer;
        e.state = State.Refunded;
        e.amount = 0;

        USDC.safeTransfer(buyer, amount);
        emit EscrowRefunded(escrowId, buyer, amount, reason);
    }

    /// @notice Arbiter resolves a dispute by directing funds to either the buyer or merchant.
    /// @param  beneficiary MUST be either the recorded buyer or merchant; arbiter cannot
    ///         redirect funds to a third party.
    function arbiterResolve(
        bytes32 escrowId,
        address beneficiary,
        bytes32 arbitrationDecisionHash
    ) external onlyRole(ARBITER_ROLE) whenNotPaused nonReentrant {
        Escrow storage e = _mustExist(escrowId);
        if (e.state != State.Disputed) {
            revert EscrowWrongState(escrowId, State.Disputed, e.state);
        }
        if (beneficiary != e.buyer && beneficiary != e.merchant) {
            revert ArbiterDecisionInvalid(beneficiary);
        }

        uint128 amount = e.amount;
        e.state = (beneficiary == e.merchant) ? State.Released : State.Refunded;
        e.amount = 0;

        USDC.safeTransfer(beneficiary, amount);
        emit EscrowResolved(escrowId, beneficiary, amount, arbitrationDecisionHash);
    }

    // -------------------------------------------------------------------
    // Pause / admin
    // -------------------------------------------------------------------

    function pause() external onlyRole(PAUSER_ROLE) { _pause(); }
    function unpause() external onlyRole(PAUSER_ROLE) { _unpause(); }

    // -------------------------------------------------------------------
    // View
    // -------------------------------------------------------------------

    function getEscrow(bytes32 escrowId) external view returns (Escrow memory) {
        return escrows[escrowId];
    }

    function escrowState(bytes32 escrowId) external view returns (State) {
        return escrows[escrowId].state;
    }

    /// @notice Earliest unix timestamp at which `release` becomes callable for this escrow,
    ///         assuming no dispute is opened. Returns 0 if escrow is not in Funded state.
    function earliestRelease(bytes32 escrowId) external view returns (uint64) {
        Escrow memory e = escrows[escrowId];
        if (e.state != State.Funded) return 0;
        return e.fulfillmentDeadline + e.disputeWindow;
    }

    // -------------------------------------------------------------------
    // Internal
    // -------------------------------------------------------------------

    function _mustExist(bytes32 escrowId) internal view returns (Escrow storage) {
        Escrow storage e = escrows[escrowId];
        if (e.state == State.None) revert EscrowNotFound(escrowId);
        return e;
    }
}
