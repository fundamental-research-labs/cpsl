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
typedef char *(*cpsl_location_handle_json_fn)(void *user_data, const char *request_json);
typedef void (*cpsl_location_string_free_fn)(char *value);
typedef void (*cpsl_location_user_data_free_fn)(void *user_data);
typedef void (*cpsl_file_activity_handle_fn)(
    void *user_data,
    const char *path,
    const char *operation
);
typedef void (*cpsl_file_activity_user_data_free_fn)(void *user_data);
typedef void (*cpsl_calendar_activity_handle_fn)(void *user_data, const char *operation);
typedef void (*cpsl_calendar_activity_user_data_free_fn)(void *user_data);
typedef struct cpsl_vision_input cpsl_vision_input_t;
typedef void (*cpsl_vision_handle_fn)(
    void *user_data,
    const cpsl_vision_input_t *inputs,
    uintptr_t input_count,
    const char *query,
    void *response_context
);
typedef void (*cpsl_vision_user_data_free_fn)(void *user_data);

typedef struct cpsl_webbrowser_callbacks {
    void *user_data;
    cpsl_webbrowser_handle_json_fn handle_json;
    cpsl_webbrowser_string_free_fn string_free;
    cpsl_webbrowser_user_data_free_fn user_data_free;
} cpsl_webbrowser_callbacks_t;

typedef struct cpsl_file_activity_callbacks {
    void *user_data;
    cpsl_file_activity_handle_fn handle_activity;
    cpsl_file_activity_user_data_free_fn user_data_free;
} cpsl_file_activity_callbacks_t;

typedef struct cpsl_calendar_activity_callbacks {
    void *user_data;
    cpsl_calendar_activity_handle_fn handle_activity;
    cpsl_calendar_activity_user_data_free_fn user_data_free;
} cpsl_calendar_activity_callbacks_t;

typedef struct cpsl_location_callbacks {
    void *user_data;
    cpsl_location_handle_json_fn handle_json;
    cpsl_location_string_free_fn string_free;
    cpsl_location_user_data_free_fn user_data_free;
} cpsl_location_callbacks_t;

struct cpsl_vision_input {
    const uint8_t *data;
    uintptr_t data_len;
    const char *filename;
    const char *media_type;
};

typedef struct cpsl_vision_callbacks {
    void *user_data;
    cpsl_vision_handle_fn handle;
    cpsl_vision_user_data_free_fn user_data_free;
} cpsl_vision_callbacks_t;

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

/*
 * Creates a session with optional callback families. Pass NULL for callback
 * groups the host does not need. File activity callbacks receive borrowed
 * virtual paths and operation names such as "read" or "write".
 */
cpsl_session_t *cpsl_session_new_with_callbacks(
    const char *config_json,
    const cpsl_webbrowser_callbacks_t *webbrowser_callbacks,
    const cpsl_file_activity_callbacks_t *file_activity_callbacks
);

/*
 * Creates a session with all supported host callback families. Location
 * callbacks receive borrowed UTF-8 JSON requests and must return caller-owned
 * UTF-8 JSON that CPSL releases with location_callbacks->string_free.
 */
cpsl_session_t *cpsl_session_new_with_host_callbacks(
    const char *config_json,
    const cpsl_webbrowser_callbacks_t *webbrowser_callbacks,
    const cpsl_file_activity_callbacks_t *file_activity_callbacks,
    const cpsl_location_callbacks_t *location_callbacks
);

/*
 * Creates a session with all supported host callback families. Calendar
 * activity callbacks receive operation names such as "status" or "events".
 */
cpsl_session_t *cpsl_session_new_with_host_callbacks_v2(
    const char *config_json,
    const cpsl_webbrowser_callbacks_t *webbrowser_callbacks,
    const cpsl_file_activity_callbacks_t *file_activity_callbacks,
    const cpsl_calendar_activity_callbacks_t *calendar_activity_callbacks,
    const cpsl_location_callbacks_t *location_callbacks
);

/*
 * Adds document-vision callbacks. The callback receives one or more borrowed
 * inputs and must synchronously call cpsl_vision_respond exactly once before
 * returning. PDF inputs are rendered to page images when PDFium is available.
 */
cpsl_session_t *cpsl_session_new_with_host_callbacks_v3(
    const char *config_json,
    const cpsl_webbrowser_callbacks_t *webbrowser_callbacks,
    const cpsl_file_activity_callbacks_t *file_activity_callbacks,
    const cpsl_calendar_activity_callbacks_t *calendar_activity_callbacks,
    const cpsl_location_callbacks_t *location_callbacks,
    const cpsl_vision_callbacks_t *vision_callbacks
);

/* data is borrowed and copied before this function returns. */
void cpsl_vision_respond(
    void *response_context,
    const uint8_t *data,
    uintptr_t data_len,
    uint8_t is_error
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
