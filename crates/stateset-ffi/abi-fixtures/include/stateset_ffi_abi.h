#ifndef STATESET_FFI_ABI_H
#define STATESET_FFI_ABI_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct FfiUuid {
    uint8_t bytes[16];
} FfiUuid;

typedef struct FfiMoney {
    int64_t amount_cents;
    uint8_t currency[3];
} FfiMoney;

typedef enum FfiOrderStatus {
    FfiOrderStatus_Pending = 0,
    FfiOrderStatus_Confirmed = 1,
    FfiOrderStatus_Processing = 2,
    FfiOrderStatus_Shipped = 3,
    FfiOrderStatus_Delivered = 4,
    FfiOrderStatus_Cancelled = 5,
    FfiOrderStatus_Refunded = 6,
    FfiOrderStatus_Unknown = 255
} FfiOrderStatus;

typedef struct FfiOrder {
    FfiUuid id;
    FfiUuid customer_id;
    FfiOrderStatus status;
    FfiMoney total;
    uint32_t item_count;
    int64_t created_at_epoch_ms;
} FfiOrder;

typedef struct FfiCustomer {
    FfiUuid id;
    char *name;
    char *email;
    int64_t created_at_epoch_ms;
} FfiCustomer;

typedef struct FfiProduct {
    FfiUuid id;
    char *name;
    uint8_t sku[64];
    FfiMoney price;
} FfiProduct;

typedef struct FfiInventoryLevel {
    uint8_t sku[64];
    int64_t quantity;
    int64_t reserved;
    int64_t available;
} FfiInventoryLevel;

typedef enum FfiErrorCode {
    FfiErrorCode_Ok = 0,
    FfiErrorCode_NotFound = 1,
    FfiErrorCode_InvalidArgument = 2,
    FfiErrorCode_InternalError = 3,
    FfiErrorCode_DatabaseError = 4,
    FfiErrorCode_SerializationError = 5,
    FfiErrorCode_NullPointer = 6,
    FfiErrorCode_Utf8Error = 7,
    FfiErrorCode_BufferTooSmall = 8
} FfiErrorCode;

typedef struct FfiResult_CommerceHandle {
    FfiErrorCode code;
    void *value;
} FfiResult_CommerceHandle;

typedef struct FfiResult_FfiOrder {
    FfiErrorCode code;
    FfiOrder value;
} FfiResult_FfiOrder;

typedef struct FfiResult_FfiCustomer {
    FfiErrorCode code;
    FfiCustomer value;
} FfiResult_FfiCustomer;

typedef struct FfiResult_FfiProduct {
    FfiErrorCode code;
    FfiProduct value;
} FfiResult_FfiProduct;

typedef struct FfiResult_FfiInventoryLevel {
    FfiErrorCode code;
    FfiInventoryLevel value;
} FfiResult_FfiInventoryLevel;

typedef struct FfiResult_FfiUuid {
    FfiErrorCode code;
    FfiUuid value;
} FfiResult_FfiUuid;

uint32_t stateset_abi_version(void);
const char *stateset_version(void);

FfiResult_CommerceHandle stateset_init(const char *db_path);
void stateset_destroy(void *engine);

FfiResult_FfiOrder stateset_order_get(void *engine, FfiUuid id);
FfiResult_FfiCustomer stateset_customer_create(
    void *engine,
    const char *email,
    const char *first_name,
    const char *last_name
);
FfiResult_FfiCustomer stateset_customer_get(void *engine, FfiUuid id);
void stateset_customer_free(FfiCustomer customer);

FfiResult_FfiProduct stateset_product_create(void *engine, const char *name);
FfiResult_FfiProduct stateset_product_get(void *engine, FfiUuid id);
void stateset_product_free(FfiProduct product);

FfiResult_FfiInventoryLevel stateset_inventory_get(void *engine, const char *sku);
FfiResult_FfiInventoryLevel stateset_inventory_adjust(void *engine, const char *sku, int64_t delta);

char *stateset_uuid_to_string(FfiUuid uuid);
FfiResult_FfiUuid stateset_uuid_from_string(const char *s);
FfiUuid stateset_uuid_generate(void);
FfiUuid stateset_uuid_nil(void);

char *stateset_money_format(FfiMoney money);
void stateset_string_free(char *ptr);

const char *stateset_last_error_message(void);
void stateset_clear_error(void);

#ifdef __cplusplus
}
#endif

#endif
