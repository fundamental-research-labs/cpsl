#ifndef CPSL_H
#define CPSL_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define CPSL_ABI_VERSION 2

typedef struct cpsl_session cpsl_session_t;

typedef char *(*cpsl_webbrowser_handle_json_fn)(void *user_data, const char *request_json);
typedef void (*cpsl_webbrowser_string_free_fn)(char *value);
typedef void (*cpsl_webbrowser_user_data_free_fn)(void *user_data);

typedef struct cpsl_webbrowser_callbacks {
    void *user_data;
    cpsl_webbrowser_handle_json_fn handle_json;
    cpsl_webbrowser_string_free_fn string_free;
    cpsl_webbrowser_user_data_free_fn user_data_free;
} cpsl_webbrowser_callbacks_t;

uint32_t cpsl_abi_version(void);

/* Returns caller-owned UTF-8 JSON. Free with cpsl_string_free. */
char *cpsl_backend_metadata_json(void);

/*
 * config_json is borrowed for the duration of the call.
 * Returns NULL on FFI/contract failure and sets cpsl_last_error.
 */
cpsl_session_t *cpsl_session_new(const char *config_json);

/*
 * Creates a session with the webbrowser module enabled when callbacks is not NULL.
 * The browser callback receives a borrowed UTF-8 JSON request and must return
 * caller-owned UTF-8 JSON. CPSL releases that response with callbacks->string_free.
 * Callback invocations are synchronous, serialized per session, and may happen
 * on a non-main CPSL evaluation thread. Host callbacks that need UI work should
 * marshal to the UI thread before returning. callbacks->user_data_free, when
 * non-NULL, is called when the CPSL session releases the callback context.
 *
 * Response JSON may be either raw wb-style JSON, for example:
 *   {"ok":true,"browser":"...","page":{...}}
 * or an envelope:
 *   {"ok":true,"result":{...}}
 * Returning {"ok":false,"error":"message"} raises a Luau error.
 */
cpsl_session_t *cpsl_session_new_with_webbrowser_callbacks(
    const char *config_json,
    const cpsl_webbrowser_callbacks_t *callbacks
);

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
