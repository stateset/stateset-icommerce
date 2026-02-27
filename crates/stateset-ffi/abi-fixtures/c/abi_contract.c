#include <assert.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

#include "../include/stateset_ffi_abi.h"

static int uuid_is_nil(FfiUuid uuid) {
    for (size_t i = 0; i < sizeof(uuid.bytes); i++) {
        if (uuid.bytes[i] != 0) {
            return 0;
        }
    }
    return 1;
}

_Static_assert(sizeof(FfiUuid) == 16, "FfiUuid ABI size changed");
_Static_assert(sizeof(FfiMoney) == 16, "FfiMoney ABI size changed");
_Static_assert(offsetof(FfiMoney, amount_cents) == 0, "FfiMoney.amount_cents offset changed");
_Static_assert(offsetof(FfiMoney, currency) == 8, "FfiMoney.currency offset changed");
_Static_assert(sizeof(FfiCustomer) == 40, "FfiCustomer ABI size changed");
_Static_assert(offsetof(FfiCustomer, name) == 16, "FfiCustomer.name offset changed");
_Static_assert(offsetof(FfiCustomer, email) == 24, "FfiCustomer.email offset changed");
_Static_assert(sizeof(FfiProduct) == 104, "FfiProduct ABI size changed");
_Static_assert(offsetof(FfiProduct, sku) == 24, "FfiProduct.sku offset changed");
_Static_assert(offsetof(FfiProduct, price) == 88, "FfiProduct.price offset changed");
_Static_assert(sizeof(FfiInventoryLevel) == 88, "FfiInventoryLevel ABI size changed");
_Static_assert(offsetof(FfiInventoryLevel, quantity) == 64, "FfiInventoryLevel.quantity offset changed");
_Static_assert(sizeof(FfiOrder) == 72, "FfiOrder ABI size changed");
_Static_assert(offsetof(FfiOrder, status) == 32, "FfiOrder.status offset changed");
_Static_assert(offsetof(FfiOrder, total) == 40, "FfiOrder.total offset changed");
_Static_assert(offsetof(FfiOrder, created_at_epoch_ms) == 64, "FfiOrder.created_at_epoch_ms offset changed");
_Static_assert(sizeof(FfiErrorCode) == 4, "FfiErrorCode ABI size changed");
_Static_assert(FfiOrderStatus_Unknown == 255, "FfiOrderStatus discriminant changed");
_Static_assert(FfiErrorCode_BufferTooSmall == 8, "FfiErrorCode discriminant changed");

int main(void) {
    const uint32_t abi = stateset_abi_version();
    assert(abi == 1);

    const char *version = stateset_version();
    assert(version != NULL);
    assert(strlen(version) > 0);

    const FfiUuid nil = stateset_uuid_nil();
    assert(uuid_is_nil(nil));

    const FfiUuid generated = stateset_uuid_generate();
    assert(!uuid_is_nil(generated));

    char *uuid_text = stateset_uuid_to_string(generated);
    assert(uuid_text != NULL);

    const FfiResult_FfiUuid parsed = stateset_uuid_from_string(uuid_text);
    assert(parsed.code == FfiErrorCode_Ok);
    assert(memcmp(parsed.value.bytes, generated.bytes, sizeof(generated.bytes)) == 0);
    stateset_string_free(uuid_text);

    FfiMoney usd = {0};
    usd.amount_cents = 1234;
    usd.currency[0] = 'U';
    usd.currency[1] = 'S';
    usd.currency[2] = 'D';
    char *money = stateset_money_format(usd);
    assert(money != NULL);
    assert(strcmp(money, "$12.34") == 0);
    stateset_string_free(money);

    const FfiResult_CommerceHandle init = stateset_init(":memory:");
    assert(init.code == FfiErrorCode_Ok);
    assert(init.value != NULL);

    const FfiResult_FfiCustomer created =
        stateset_customer_create(init.value, "c-fixture@example.com", "C", "Fixture");
    assert(created.code == FfiErrorCode_Ok);
    assert(created.value.name != NULL);
    assert(created.value.email != NULL);

    const FfiResult_FfiCustomer fetched = stateset_customer_get(init.value, created.value.id);
    assert(fetched.code == FfiErrorCode_Ok);
    assert(memcmp(created.value.id.bytes, fetched.value.id.bytes, sizeof(created.value.id.bytes)) == 0);

    stateset_customer_free(created.value);
    stateset_customer_free(fetched.value);
    stateset_destroy(init.value);

    stateset_clear_error();
    const FfiResult_FfiUuid bad_uuid = stateset_uuid_from_string("not-a-uuid");
    assert(bad_uuid.code == FfiErrorCode_InvalidArgument);
    const char *err = stateset_last_error_message();
    assert(err != NULL);
    assert(strlen(err) > 0);

    return 0;
}
