#ifndef STATESET_H
#define STATESET_H

#include <stdbool.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef void *StateSetHandle;

StateSetHandle stateset_commerce_new(const char *db_path);
void stateset_commerce_free(StateSetHandle handle);
void stateset_string_free(char *s);
char *stateset_get_last_error(void);

#ifdef __cplusplus
}
#endif

#endif
