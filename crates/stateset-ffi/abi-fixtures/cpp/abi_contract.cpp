#include <cassert>
#include <cstddef>
#include <cstdint>
#include <cstring>
#include <string>
#include <type_traits>

#include "../include/stateset_ffi_abi.h"

static bool uuid_is_nil(const FfiUuid &uuid) {
    for (std::uint8_t byte : uuid.bytes) {
        if (byte != 0) {
            return false;
        }
    }
    return true;
}

static_assert(std::is_standard_layout_v<FfiUuid>, "FfiUuid must be standard-layout");
static_assert(std::is_standard_layout_v<FfiOrder>, "FfiOrder must be standard-layout");
static_assert(sizeof(FfiUuid) == 16, "FfiUuid ABI size changed");
static_assert(sizeof(FfiMoney) == 16, "FfiMoney ABI size changed");
static_assert(sizeof(FfiOrder) == 72, "FfiOrder ABI size changed");
static_assert(alignof(FfiOrder) == 8, "FfiOrder ABI alignment changed");
static_assert(offsetof(FfiOrder, id) == 0, "FfiOrder.id offset changed");
static_assert(offsetof(FfiOrder, customer_id) == 16, "FfiOrder.customer_id offset changed");
static_assert(offsetof(FfiOrder, status) == 32, "FfiOrder.status offset changed");
static_assert(offsetof(FfiOrder, total) == 40, "FfiOrder.total offset changed");
static_assert(offsetof(FfiOrder, item_count) == 56, "FfiOrder.item_count offset changed");
static_assert(offsetof(FfiOrder, created_at_epoch_ms) == 64, "FfiOrder.created_at_epoch_ms offset changed");
static_assert(sizeof(FfiResult_FfiCustomer) == 48, "FfiResult_FfiCustomer ABI size changed");
static_assert(sizeof(FfiResult_FfiProduct) == 112, "FfiResult_FfiProduct ABI size changed");
static_assert(FfiErrorCode_NotFound == 1, "FfiErrorCode discriminant changed");

int main() {
    assert(stateset_abi_version() == 1);

    const char *version = stateset_version();
    assert(version != nullptr);
    assert(std::strlen(version) > 0);

    FfiUuid generated = stateset_uuid_generate();
    assert(!uuid_is_nil(generated));

    char *uuid_text = stateset_uuid_to_string(generated);
    assert(uuid_text != nullptr);
    FfiResult_FfiUuid parsed = stateset_uuid_from_string(uuid_text);
    assert(parsed.code == FfiErrorCode_Ok);
    assert(std::memcmp(parsed.value.bytes, generated.bytes, sizeof(generated.bytes)) == 0);
    stateset_string_free(uuid_text);

    FfiResult_CommerceHandle init = stateset_init(":memory:");
    assert(init.code == FfiErrorCode_Ok);
    assert(init.value != nullptr);

    FfiResult_FfiProduct created = stateset_product_create(init.value, "CPP Fixture Product");
    assert(created.code == FfiErrorCode_Ok);
    assert(created.value.name != nullptr);

    FfiResult_FfiProduct fetched = stateset_product_get(init.value, created.value.id);
    assert(fetched.code == FfiErrorCode_Ok);
    assert(std::memcmp(created.value.id.bytes, fetched.value.id.bytes, sizeof(created.value.id.bytes)) == 0);

    stateset_product_free(created.value);
    stateset_product_free(fetched.value);

    stateset_destroy(init.value);

    stateset_clear_error();
    FfiResult_FfiUuid bad = stateset_uuid_from_string("bad-value");
    assert(bad.code == FfiErrorCode_InvalidArgument);
    const char *last_error = stateset_last_error_message();
    assert(last_error != nullptr);
    assert(std::strlen(last_error) > 0);

    return 0;
}
