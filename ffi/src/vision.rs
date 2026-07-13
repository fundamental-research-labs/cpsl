use cpsl_core::{VisionCallback, VisionInput};
use std::ffi::{c_char, c_void, CString};
use std::sync::{Arc, Mutex};

type VisionHandleFn = unsafe extern "C" fn(
    user_data: *mut c_void,
    inputs: *const cpsl_vision_input_t,
    input_count: usize,
    query: *const c_char,
    response_context: *mut c_void,
);
type VisionUserDataFreeFn = unsafe extern "C" fn(user_data: *mut c_void);

#[allow(non_camel_case_types)]
#[repr(C)]
pub struct cpsl_vision_input_t {
    data: *const u8,
    data_len: usize,
    filename: *const c_char,
    media_type: *const c_char,
}

#[allow(non_camel_case_types)]
#[repr(C)]
pub struct cpsl_vision_callbacks_t {
    user_data: *mut c_void,
    handle: Option<VisionHandleFn>,
    user_data_free: Option<VisionUserDataFreeFn>,
}

struct FfiVisionBridge {
    user_data: *mut c_void,
    handle: VisionHandleFn,
    user_data_free: Option<VisionUserDataFreeFn>,
    callback_lock: Mutex<()>,
}

unsafe impl Send for FfiVisionBridge {}
unsafe impl Sync for FfiVisionBridge {}

impl Drop for FfiVisionBridge {
    fn drop(&mut self) {
        if !self.user_data.is_null() {
            if let Some(user_data_free) = self.user_data_free {
                unsafe { user_data_free(self.user_data) };
            }
        }
    }
}

struct VisionResponseSlot {
    result: Option<Result<String, String>>,
}

impl FfiVisionBridge {
    fn invoke(&self, inputs: &[VisionInput], query: &str) -> Result<String, String> {
        let query = CString::new(query)
            .map_err(|_| "vision query contained an embedded NUL byte".to_string())?;
        let filenames = inputs
            .iter()
            .map(|input| {
                CString::new(input.filename.as_str())
                    .map_err(|_| "vision filename contained an embedded NUL byte".to_string())
            })
            .collect::<Result<Vec<_>, _>>()?;
        let media_types = inputs
            .iter()
            .map(|input| {
                CString::new(input.media_type.as_str())
                    .map_err(|_| "vision media type contained an embedded NUL byte".to_string())
            })
            .collect::<Result<Vec<_>, _>>()?;
        let ffi_inputs = inputs
            .iter()
            .enumerate()
            .map(|(index, input)| cpsl_vision_input_t {
                data: input.data.as_ptr(),
                data_len: input.data.len(),
                filename: filenames[index].as_ptr(),
                media_type: media_types[index].as_ptr(),
            })
            .collect::<Vec<_>>();
        let mut response = VisionResponseSlot { result: None };
        let _guard = self
            .callback_lock
            .lock()
            .map_err(|_| "vision callback lock was poisoned".to_string())?;

        unsafe {
            (self.handle)(
                self.user_data,
                ffi_inputs.as_ptr(),
                ffi_inputs.len(),
                query.as_ptr(),
                &mut response as *mut VisionResponseSlot as *mut c_void,
            );
        }

        response
            .result
            .unwrap_or_else(|| Err("vision callback returned without a response".to_string()))
    }
}

pub fn validate_vision_callbacks(
    callbacks: *const cpsl_vision_callbacks_t,
) -> Result<Option<VisionCallback>, String> {
    if callbacks.is_null() {
        return Ok(None);
    }
    let callbacks = unsafe { &*callbacks };
    let handle = callbacks
        .handle
        .ok_or_else(|| "vision callbacks.handle must not be NULL".to_string())?;
    let bridge = Arc::new(FfiVisionBridge {
        user_data: callbacks.user_data,
        handle,
        user_data_free: callbacks.user_data_free,
        callback_lock: Mutex::new(()),
    });
    Ok(Some(Arc::new(move |inputs, query| {
        bridge.invoke(inputs, query)
    })))
}

pub fn free_vision_callback_context(callbacks: *const cpsl_vision_callbacks_t) {
    if callbacks.is_null() {
        return;
    }
    let callbacks = unsafe { &*callbacks };
    if !callbacks.user_data.is_null() {
        if let Some(user_data_free) = callbacks.user_data_free {
            unsafe { user_data_free(callbacks.user_data) };
        }
    }
}

/// Completes the in-flight synchronous vision callback. `data` is borrowed and
/// copied before this function returns. `is_error` selects Ok versus Err.
#[no_mangle]
pub extern "C" fn cpsl_vision_respond(
    response_context: *mut c_void,
    data: *const u8,
    data_len: usize,
    is_error: u8,
) {
    if response_context.is_null() || (data.is_null() && data_len != 0) {
        return;
    }
    let response = unsafe { &mut *(response_context as *mut VisionResponseSlot) };
    let bytes = if data_len == 0 {
        &[][..]
    } else {
        unsafe { std::slice::from_raw_parts(data, data_len) }
    };
    let value = match std::str::from_utf8(bytes) {
        Ok(value) => value.to_string(),
        Err(_) => {
            response.result = Some(Err("vision callback returned non-UTF-8 text".to_string()));
            return;
        }
    };
    response.result = Some(if is_error != 0 { Err(value) } else { Ok(value) });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CStr;
    use std::ptr;

    unsafe extern "C" fn echo_callback(
        _user_data: *mut c_void,
        inputs: *const cpsl_vision_input_t,
        input_count: usize,
        _query: *const c_char,
        response_context: *mut c_void,
    ) {
        assert_eq!(input_count, 1);
        let input = unsafe { &*inputs };
        assert_eq!(
            unsafe { CStr::from_ptr(input.filename) }.to_str().unwrap(),
            "page.png"
        );
        let value = b"vision result";
        cpsl_vision_respond(response_context, value.as_ptr(), value.len(), 0);
    }

    #[test]
    fn callback_copies_inputs_and_response() {
        let callbacks = cpsl_vision_callbacks_t {
            user_data: ptr::null_mut(),
            handle: Some(echo_callback),
            user_data_free: None,
        };
        let callback = validate_vision_callbacks(&callbacks).unwrap().unwrap();
        let result = callback(
            &[VisionInput {
                data: vec![1, 2, 3],
                filename: "page.png".to_string(),
                media_type: "image/png".to_string(),
            }],
            "read it",
        )
        .unwrap();
        assert_eq!(result, "vision result");
    }
}
