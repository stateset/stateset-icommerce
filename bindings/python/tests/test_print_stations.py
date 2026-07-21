"""Print station API tests for the stateset_embedded Python bindings.

IDs cross as UUID strings, timestamps as RFC3339 strings, enums as
snake_case strings.
"""

import pytest
from stateset_embedded import Commerce


@pytest.fixture
def commerce():
    return Commerce(":memory:")


def test_print_stations_api_exists(commerce):
    assert commerce.print_stations is not None


def test_print_station_full_lifecycle(commerce):
    ps = commerce.print_stations
    if not ps.is_supported():
        pytest.skip("print stations are not supported on this engine build")

    paired = ps.pair(name="Packing Bench 1", printers=["zebra-1", "zebra-2"])
    assert paired.token
    station = paired.station
    assert station.id
    assert station.name == "Packing Bench 1"
    assert station.printers == ["zebra-1", "zebra-2"]
    assert station.revoked is False
    assert station.created_at

    found = ps.get_station(station.id)
    assert found is not None
    assert found.id == station.id
    assert any(s.id == station.id for s in ps.list_stations())

    job = ps.enqueue_job(station.id, "^XA^FO50,50^A0N,50^FDhello^FS^XZ", printer_name="zebra-1")
    assert job.id
    assert job.station_id == station.id
    assert job.status == "queued"
    assert job.payload_kind == "zpl"
    assert job.printer_name == "zebra-1"
    assert job.picked_up_at is None

    queued = ps.list_jobs(station.id, status="queued")
    assert any(j.id == job.id for j in queued)

    picked = ps.next_job(station.id)
    assert picked is not None
    assert picked.id == job.id
    assert picked.status == "picked_up"
    assert picked.picked_up_at is not None

    done = ps.complete_job(job.id, True)
    assert done.status == "printed"
    assert not ps.list_jobs(station.id, status="queued")

    pdf_job = ps.enqueue_job(station.id, "JVBERi0=", payload_kind="pdf")
    assert pdf_job.payload_kind == "pdf"
    failed = ps.complete_job(ps.next_job(station.id).id, False)
    assert failed.status == "failed"

    revoked = ps.revoke_station(station.id)
    assert revoked.revoked is True


def test_invalid_inputs_raise(commerce):
    ps = commerce.print_stations
    if not ps.is_supported():
        pytest.skip("print stations are not supported on this engine build")

    with pytest.raises(ValueError):
        ps.get_station("not-a-uuid")
    with pytest.raises(ValueError):
        ps.complete_job("not-a-uuid", True)

    station = ps.pair(name="Bench 2").station
    assert station.printers == []
    with pytest.raises(ValueError):
        ps.enqueue_job(station.id, "^XA^XZ", payload_kind="not_a_kind")
    with pytest.raises(ValueError):
        ps.list_jobs(station.id, status="not_a_status")
