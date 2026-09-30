"""Closing a non-conformance report requires a disposition.

A closed NCR is the quality record of what was done with the material, so the
engine refuses to close one without a disposition. ``update_ncr`` is how the
binding records one; before it existed the binding could not close an NCR.
"""

import pytest

from stateset_embedded import Commerce


@pytest.fixture
def commerce():
    return Commerce(":memory:")


def open_ncr(commerce, sku):
    return commerce.quality.create_ncr(
        sku=sku,
        description="scratched units",
        quantity_affected=5.0,
        source="internal_audit",
        severity="major",
    )


def test_close_without_a_disposition_is_refused(commerce):
    ncr = open_ncr(commerce, "NCR-NO-DISP")
    with pytest.raises(ValueError, match="disposition"):
        commerce.quality.close_ncr(ncr.id)
    with pytest.raises(ValueError, match="disposition"):
        commerce.quality.update_ncr(ncr.id, status="closed")
    # Still open: listing it by status finds it.
    open_ids = [n.id for n in commerce.quality.list_ncrs(status="open")]
    assert ncr.id in open_ids


def test_update_with_a_disposition_then_close(commerce):
    ncr = open_ncr(commerce, "NCR-DISP")
    updated = commerce.quality.update_ncr(
        ncr.id,
        disposition="return_to_vendor",
        disposition_quantity_exact="4.5",
        root_cause="supplier tooling",
    )
    assert updated.status == "Open"
    assert updated.disposition == "ReturnToVendor"
    assert updated.disposition_quantity_exact == "4.5"
    assert updated.root_cause == "supplier tooling"

    closed = commerce.quality.close_ncr(ncr.id)
    assert closed.status == "Closed"
    assert closed.closed_at is not None
    assert commerce.quality.close_ncr(ncr.id).status == "Closed"


def test_one_update_may_set_the_disposition_and_close(commerce):
    ncr = open_ncr(commerce, "NCR-ONE-CALL")
    closed = commerce.quality.update_ncr(ncr.id, disposition="Scrap", status="closed")
    assert closed.status == "Closed"
    assert closed.disposition == "Scrap"


def test_unknown_enum_spellings_are_refused(commerce):
    ncr = open_ncr(commerce, "NCR-BAD-ENUM")
    with pytest.raises(ValueError, match="Invalid NCR disposition 'burn'"):
        commerce.quality.update_ncr(ncr.id, disposition="burn")
    with pytest.raises(ValueError, match="Invalid NCR status 'done'"):
        commerce.quality.update_ncr(ncr.id, status="done")
