"""Finance hardening tests: accounts_payable.three_way_match and
general_ledger.revalue for the stateset_embedded Python bindings.

Money crosses as exact decimal strings; dates as ISO strings; enums as
snake_case strings.
"""

import pytest
from stateset_embedded import Commerce


@pytest.fixture
def commerce():
    return Commerce(":memory:")


class TestThreeWayMatch:
    def test_bill_without_purchase_order_is_not_required(self, commerce):
        bill = commerce.accounts_payable.create_bill(
            "3f2504e0-4f89-41d3-9a0c-0305e82c3301", "2026-08-01T00:00:00Z"
        )
        result = commerce.accounts_payable.three_way_match(bill.id, "5")
        assert result.match_status == "not_required"
        assert result.lines == []

    def test_rejects_invalid_bill_uuid(self, commerce):
        with pytest.raises(ValueError, match="Invalid UUID"):
            commerce.accounts_payable.three_way_match("not-a-uuid")

    def test_rejects_invalid_tolerance_decimal(self, commerce):
        with pytest.raises(ValueError, match="Invalid tolerance_percent decimal"):
            commerce.accounts_payable.three_way_match(
                "3f2504e0-4f89-41d3-9a0c-0305e82c3301", "abc"
            )


class TestRevalue:
    def test_revalue_with_no_foreign_accounts_is_noop(self, commerce):
        commerce.general_ledger.initialize_chart_of_accounts()
        result = commerce.general_ledger.revalue("2026-07-01", "USD")
        assert result.as_of_date == "2026-07-01"
        assert result.base_currency == "USD"
        assert result.total_unrealized_gain_loss == "0"
        assert result.lines == []
        assert result.journal_entry is None

    def test_rejects_invalid_date(self, commerce):
        with pytest.raises(ValueError, match="Invalid date format"):
            commerce.general_ledger.revalue("July 1")

    def test_rejects_invalid_base_currency(self, commerce):
        with pytest.raises(ValueError, match="Invalid base currency code"):
            commerce.general_ledger.revalue("2026-07-01", "NOPE")
