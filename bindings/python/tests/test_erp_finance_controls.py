import uuid
import pytest
from stateset_embedded import Commerce


def test_bill_maker_checker_uses_trusted_host_identity():
    commerce = Commerce(":memory:")
    bill = commerce.accounts_payable.create_bill(str(uuid.uuid4()), "2026-12-01T00:00:00Z", created_by="maker")
    assert bill.created_by == "maker"
    assert bill.currency == "USD"
    with pytest.raises(Exception, match="(?i)(creator|maker|same|actor|identity)"):
        commerce.accounts_payable.approve_bill(bill.id, actor="maker")
    with pytest.raises(Exception, match="(?i)(actor|identity)"):
        commerce.accounts_payable.approve_bill(bill.id)
    approved = commerce.accounts_payable.approve_bill(bill.id, actor="checker")
    assert approved.approved_by == "checker"
