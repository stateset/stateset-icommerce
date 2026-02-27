#!/usr/bin/env python3
import ctypes
import sys


class FfiUuid(ctypes.Structure):
    _fields_ = [("bytes", ctypes.c_uint8 * 16)]


class FfiMoney(ctypes.Structure):
    _fields_ = [("amount_cents", ctypes.c_int64), ("currency", ctypes.c_uint8 * 3)]


class FfiCustomer(ctypes.Structure):
    _fields_ = [
        ("id", FfiUuid),
        ("name", ctypes.c_void_p),
        ("email", ctypes.c_void_p),
        ("created_at_epoch_ms", ctypes.c_int64),
    ]


class FfiResultCommerceHandle(ctypes.Structure):
    _fields_ = [("code", ctypes.c_int32), ("value", ctypes.c_void_p)]


class FfiResultFfiUuid(ctypes.Structure):
    _fields_ = [("code", ctypes.c_int32), ("value", FfiUuid)]


class FfiResultFfiCustomer(ctypes.Structure):
    _fields_ = [("code", ctypes.c_int32), ("value", FfiCustomer)]


def as_bytes(ptr):
    return ctypes.cast(ptr, ctypes.c_char_p).value


def ensure(condition, message):
    if not condition:
        raise AssertionError(message)


def main() -> int:
    if len(sys.argv) != 2:
        raise RuntimeError("usage: ctypes_smoke.py /path/to/libstateset_ffi.so")

    lib = ctypes.CDLL(sys.argv[1])

    lib.stateset_abi_version.argtypes = []
    lib.stateset_abi_version.restype = ctypes.c_uint32

    lib.stateset_version.argtypes = []
    lib.stateset_version.restype = ctypes.c_char_p

    lib.stateset_uuid_generate.argtypes = []
    lib.stateset_uuid_generate.restype = FfiUuid

    lib.stateset_uuid_to_string.argtypes = [FfiUuid]
    lib.stateset_uuid_to_string.restype = ctypes.c_void_p

    lib.stateset_uuid_from_string.argtypes = [ctypes.c_char_p]
    lib.stateset_uuid_from_string.restype = FfiResultFfiUuid

    lib.stateset_money_format.argtypes = [FfiMoney]
    lib.stateset_money_format.restype = ctypes.c_void_p

    lib.stateset_string_free.argtypes = [ctypes.c_void_p]
    lib.stateset_string_free.restype = None

    lib.stateset_init.argtypes = [ctypes.c_char_p]
    lib.stateset_init.restype = FfiResultCommerceHandle

    lib.stateset_destroy.argtypes = [ctypes.c_void_p]
    lib.stateset_destroy.restype = None

    lib.stateset_customer_create.argtypes = [
        ctypes.c_void_p,
        ctypes.c_char_p,
        ctypes.c_char_p,
        ctypes.c_char_p,
    ]
    lib.stateset_customer_create.restype = FfiResultFfiCustomer

    lib.stateset_customer_get.argtypes = [ctypes.c_void_p, FfiUuid]
    lib.stateset_customer_get.restype = FfiResultFfiCustomer

    lib.stateset_customer_free.argtypes = [FfiCustomer]
    lib.stateset_customer_free.restype = None

    lib.stateset_clear_error.argtypes = []
    lib.stateset_clear_error.restype = None

    lib.stateset_last_error_message.argtypes = []
    lib.stateset_last_error_message.restype = ctypes.c_char_p

    ensure(lib.stateset_abi_version() == 1, "unexpected ABI version")
    version = lib.stateset_version()
    ensure(version is not None and len(version) > 0, "version string missing")

    uuid = lib.stateset_uuid_generate()
    uuid_ptr = lib.stateset_uuid_to_string(uuid)
    ensure(uuid_ptr is not None and uuid_ptr != 0, "uuid string pointer is null")
    uuid_text = as_bytes(uuid_ptr)
    ensure(uuid_text is not None and len(uuid_text) > 0, "uuid string empty")
    parsed = lib.stateset_uuid_from_string(uuid_text)
    ensure(parsed.code == 0, "uuid parse should succeed")
    ensure(bytes(parsed.value.bytes) == bytes(uuid.bytes), "uuid roundtrip mismatch")
    lib.stateset_string_free(uuid_ptr)

    usd = (ctypes.c_uint8 * 3)(*b"USD")
    money = FfiMoney(amount_cents=1234, currency=usd)
    money_ptr = lib.stateset_money_format(money)
    ensure(money_ptr is not None and money_ptr != 0, "money format pointer is null")
    money_text = as_bytes(money_ptr)
    ensure(money_text == b"$12.34", "money format output mismatch")
    lib.stateset_string_free(money_ptr)
    lib.stateset_string_free(None)

    init = lib.stateset_init(b":memory:")
    ensure(init.code == 0 and init.value not in (None, 0), "stateset_init failed")

    created = lib.stateset_customer_create(
        init.value, b"py-fixture@example.com", b"Py", b"Fixture"
    )
    ensure(created.code == 0, "stateset_customer_create failed")
    ensure(created.value.name not in (None, 0), "customer name pointer is null")
    ensure(created.value.email not in (None, 0), "customer email pointer is null")

    fetched = lib.stateset_customer_get(init.value, created.value.id)
    ensure(fetched.code == 0, "stateset_customer_get failed")

    lib.stateset_customer_free(created.value)
    lib.stateset_customer_free(fetched.value)
    lib.stateset_destroy(init.value)

    lib.stateset_clear_error()
    bad_uuid = lib.stateset_uuid_from_string(b"not-a-uuid")
    ensure(bad_uuid.code == 2, "invalid uuid should return InvalidArgument")
    err = lib.stateset_last_error_message()
    ensure(err is not None and len(err) > 0, "last error message missing")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
