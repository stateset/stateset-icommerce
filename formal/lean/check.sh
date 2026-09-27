#!/usr/bin/env bash
# Check the Lean proofs, and hold them to the code.
#
#   1. Every proof file must compile, and its public theorems must rest on
#      Lean's standard axioms only -- a `sorry` would show up as sorryAx;
#   2. the golden vectors the proved model computes must equal the committed
#      allocate_rounded.golden.json, which the Rust test
#      `allocate_rounded_matches_the_lean_model` holds the engine to.
#
# Usage: formal/lean/check.sh   (needs the `lean` named in lean-toolchain on
#                                PATH; with elan that is automatic)
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
cd "$HERE"
want="$(sed 's/.*:v//' lean-toolchain)"
have="$(lean --version | sed -E 's/^Lean \(version ([0-9.]+).*/\1/')"
if [[ "$have" != "$want" ]]; then
  echo "FAIL: lean-toolchain pins $want but lean on PATH is $have" >&2
  exit 1
fi

build="$(mktemp -d)"
trap 'rm -rf "$build"' EXIT
LEAN_PATH="$build:$(lean --print-prefix)/lib/lean"
export LEAN_PATH
fail=0

echo "== Allocation.lean: the proofs must check"
lean -o "$build/Allocation.olean" Allocation.lean
echo "== MerklePath.lean: the proofs must check"
lean -o "$build/MerklePath.olean" MerklePath.lean
echo "== RevenueSchedule.lean: the proofs must check"
lean -o "$build/RevenueSchedule.olean" RevenueSchedule.lean
echo "== LedgerRevaluation.lean: the proofs must check"
lean -o "$build/LedgerRevaluation.olean" LedgerRevaluation.lean
echo "== PromotionCaps.lean: the proofs must check"
lean -o "$build/PromotionCaps.olean" PromotionCaps.lean
echo "== LedgerPosting.lean: the proofs must check"
lean -o "$build/LedgerPosting.olean" LedgerPosting.lean
echo "== Depreciation.lean: the proofs must check"
lean -o "$build/Depreciation.olean" Depreciation.lean
echo "== CreditAllocation.lean: the proofs must check"
lean -o "$build/CreditAllocation.olean" CreditAllocation.lean
echo "== PayableAllocation.lean: the proofs must check"
lean -o "$build/PayableAllocation.olean" PayableAllocation.lean
echo "== PickQuantity.lean: the proofs must check"
lean -o "$build/PickQuantity.olean" PickQuantity.lean
echo "== SubscriptionBilling.lean: the proofs must check"
lean -o "$build/SubscriptionBilling.olean" SubscriptionBilling.lean
echo "== CashReconciliation.lean: the proofs must check"
lean -o "$build/CashReconciliation.olean" CashReconciliation.lean
echo "== CommerceQuantities.lean: the proofs must check"
lean -o "$build/CommerceQuantities.olean" CommerceQuantities.lean
echo "== OperationalReconciliation.lean: the proofs must check"
lean -o "$build/OperationalReconciliation.olean" OperationalReconciliation.lean
echo "== OperationalClaims.lean: the proofs must check"
lean -o "$build/OperationalClaims.olean" OperationalClaims.lean
echo "== OperationalIntegrity.lean: the proofs must check"
lean -o "$build/OperationalIntegrity.olean" OperationalIntegrity.lean
echo "== LifecycleAccounting.lean: the proofs must check"
lean -o "$build/LifecycleAccounting.olean" LifecycleAccounting.lean
echo "== CommerceSettlement.lean: the proofs must check"
lean -o "$build/CommerceSettlement.olean" CommerceSettlement.lean

echo "== the theorems must use no axiom beyond Lean's standard three"
theorems=(round_within residue_le_length select_marks nudge_sum allocate_sum allocate_near)
axioms="$(printf 'import Allocation\nimport MerklePath\nimport RevenueSchedule\nimport LedgerRevaluation\nimport PromotionCaps\nimport LedgerPosting\nimport Depreciation\nimport CreditAllocation\nimport PayableAllocation\nimport PickQuantity\nimport SubscriptionBilling\nimport CashReconciliation\n' ; for t in "${theorems[@]}"; do printf '#print axioms Allocation.%s\n' "$t"; done; printf '#print axioms MerklePath.leaf_binding\n#print axioms MerklePath.index_exhausted\n#print axioms RevenueSchedule.ratable_sum\n#print axioms RevenueSchedule.recognized_add_deferred\n#print axioms LedgerRevaluation.journal_balanced\n#print axioms LedgerRevaluation.journal_lines_valid\n#print axioms LedgerRevaluation.reversal_balanced\n#print axioms PromotionCaps.step_bounded\n#print axioms PromotionCaps.stack_bounded\n#print axioms PromotionCaps.step_never_reverses\n#print axioms PromotionCaps.step_request_bound\n#print axioms LedgerPosting.balanced_net_zero\n#print axioms LedgerPosting.posting_preserves_trial_balance\n#print axioms LedgerPosting.append_balanced\n#print axioms LedgerPosting.reversal_balanced\n#print axioms Depreciation.accumulated_eq_base\n#print axioms Depreciation.final_book_is_salvage\n#print axioms CreditAllocation.accepted_matches_cap\n#print axioms CreditAllocation.accepted_credit_conserved\n#print axioms CreditAllocation.credit_conserved\n#print axioms CreditAllocation.invoice_never_overpaid\n#print axioms CreditAllocation.applications_conserve_credit\n#print axioms PayableAllocation.bill_conserved\n#print axioms PayableAllocation.allocation_never_overpays\n#print axioms PayableAllocation.allocations_conserve_bill\n#print axioms PickQuantity.valid_iff_total_bounded\n#print axioms PickQuantity.remaining_conserved\n#print axioms SubscriptionBilling.discount_bounded\n#print axioms SubscriptionBilling.cycle_conserved\n#print axioms SubscriptionBilling.excessive_discount_zeroes_total\n#print axioms CashReconciliation.refund_conserves_capture\n#print axioms CashReconciliation.refunds_conserve_capture\n')"
report="$(lean --stdin <<< "$axioms")"
commerce_axioms="$(printf 'import CommerceQuantities\n#print axioms MaterialConsumption.accepted_within_reservation\n#print axioms StoredValue.charge_conserved\n#print axioms StoredValue.refund_conserved\n#print axioms ReceivingPutAway.completion_within_received\n#print axioms ReceivingPutAway.stock_and_movement_advance\n#print axioms Backorder.allocate_within_remaining\n#print axioms Backorder.fulfill_conserved\n#print axioms Receivable.payment_conserved\n#print axioms Receivable.credit_conserved\n#print axioms Receivable.writeoff_conserved\n')"
report="$report
$(lean --stdin <<< "$commerce_axioms")"
operational_axioms="$(printf 'import OperationalReconciliation\n#print axioms CreditExposure.reserve_preserves_available\n#print axioms CreditExposure.charge_converts_own_hold\n#print axioms SerialQuarantine.quarantine_moves_only_sellable\n#print axioms SerialQuarantine.block_all_sellable\n#print axioms ManufacturingYield.report_good_and_scrap\n#print axioms ManufacturingYield.completed_within_plan\n#print axioms PaymentLedger.payment_running_balance\n#print axioms PaymentLedger.overpayment_clamps_balance\n#print axioms ReturnDisposition.restock_increases_sellable\n#print axioms ReturnDisposition.quarantine_preserves_sellable\n')"
report="$report
$(lean --stdin <<< "$operational_axioms")"
claims_axioms="$(printf 'import OperationalClaims\n#print axioms CycleCount.variance_reconciles\n#print axioms CycleCount.zero_variance_leaves_stock\n#print axioms TransferReceipt.accepted_within_shipped\n#print axioms TransferReceipt.line_totals\n#print axioms WarrantyClaims.claim_consumes_one_slot\n#print axioms VendorCredit.apply_conserves\n#print axioms VendorCredit.reverse_conserves\n#print axioms X402Credit.debit_conserves\n#print axioms X402Credit.credit_conserves\n')"
report="$report
$(lean --stdin <<< "$claims_axioms")"
integrity_axioms="$(printf 'import OperationalIntegrity\n#print axioms HttpIdempotency.first_write_wins\n#print axioms HttpIdempotency.expired_generation_restarts\n#print axioms LoyaltyPoints.redeem_conserves\n#print axioms LoyaltyPoints.earn_conserves\n#print axioms PrepaymentSettlement.application_conserves\n#print axioms PrepaymentSettlement.refund_conserves\n#print axioms QualityHoldRelease.release_once\n#print axioms EdiTerminalStatus.terminal_write_once\n')"
report="$report
$(lean --stdin <<< "$integrity_axioms")"
lifecycle_axioms="$(printf 'import LifecycleAccounting\n#print axioms SubscriptionCancel.paid_cycle_cannot_reschedule_cancelled\n#print axioms PurchaseOrderReceipt.receipt_within_order\n#print axioms PurchaseOrderReceipt.remaining_conserved\n#print axioms AssetDisposal.book_value_at_disposal\n#print axioms AssetDisposal.proceeds_split\n#print axioms RevenuePosting.recognize_conserves\n#print axioms RevenuePosting.journal_balanced\n#print axioms GiftCardExpiry.refund_conserves\n#print axioms GiftCardExpiry.expired_refund_not_spendable\n')"
report="$report
$(lean --stdin <<< "$lifecycle_axioms")"
settlement_axioms="$(printf 'import CommerceSettlement\n#print axioms RatePublication.current_is_recorded\n#print axioms RatePublication.fixed_point_remainder\n#print axioms ObligationSettlement.payment_conserves\n#print axioms ObligationSettlement.cancelled_amount_frozen\n#print axioms CostLayerIssue.one_layer_conserves\n#print axioms CostLayerIssue.two_layers_conserve\n#print axioms InboundCancelReceipt.receipt_conserves\n#print axioms VendorReturnDecision.credit_of_appended_lines\n')"
report="$report
$(lean --stdin <<< "$settlement_axioms")"
echo "$report"
if grep -Eo '\b[A-Za-z.]+\b' <<< "$(grep -o '\[.*\]' <<< "$report")" \
    | grep -vxE 'propext|Classical\.choice|Quot\.sound' | grep -q .; then
  echo "FAIL: a theorem depends on a non-standard axiom (sorryAx means an unfinished proof)" >&2
  fail=1
fi

echo "== the model's golden vectors must equal allocate_rounded.golden.json"
lean --run Golden.lean > "$build/golden.json"
if diff -u allocate_rounded.golden.json "$build/golden.json" > "$build/golden.diff"; then
  echo "ok: $(grep -c '"strategy"' allocate_rounded.golden.json) vectors, identical"
else
  head -40 "$build/golden.diff" >&2
  echo "FAIL: regenerate with 'lean --run Golden.lean > allocate_rounded.golden.json'" >&2
  echo "      (LEAN_PATH as in this script) and rerun the Rust test." >&2
  fail=1
fi

exit $fail
