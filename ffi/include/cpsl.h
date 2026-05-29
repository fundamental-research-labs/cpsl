#ifndef CPSL_H
#define CPSL_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define CPSL_ABI_VERSION 1

typedef struct cpsl_session cpsl_session_t;

uint32_t cpsl_abi_version(void);

/* Returns caller-owned UTF-8 JSON. Free with cpsl_string_free. */
char *cpsl_backend_metadata_json(void);

/*
 * config_json is borrowed for the duration of the call.
 * Returns NULL on FFI/contract failure and sets cpsl_last_error.
 */
cpsl_session_t *cpsl_session_new(const char *config_json);

/* NULL is a no-op. Passing any other non-CPSL pointer is undefined behavior. */
void cpsl_session_free(cpsl_session_t *session);

/*
 * request_json is borrowed for the duration of the call.
 * Returns caller-owned UTF-8 JSON on successful evaluation handling.
 * Returns NULL on FFI/contract failure and sets cpsl_last_error.
 */
char *cpsl_eval(cpsl_session_t *session, const char *request_json);

/* NULL is a no-op. Passing any other non-CPSL pointer is undefined behavior. */
void cpsl_string_free(char *value);

/*
 * Returns a borrowed UTF-8 string. An empty string means no current error.
 * The pointer remains valid until the next non-cpsl_last_error FFI call.
 */
const char *cpsl_last_error(void);

#ifdef __cplusplus
}
#endif

#endif /* CPSL_H */
