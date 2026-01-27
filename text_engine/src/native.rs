use crate::jvm::ENGINE;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;

#[unsafe(no_mangle)]
pub extern "C" fn text_engine_process(input: *const c_char) -> *mut c_char {
    let c_str = unsafe { CStr::from_ptr(input) };
    let input_str = c_str.to_str().unwrap_or("");

    let mut engine = ENGINE.lock().unwrap();
    // Use a fixed size for C-API demo or pass it in
    let result = engine.process_text(input_str, 24.0, 400.0);

    let output = format!("Processed: {} metrics", result.glyph_count);

    CString::new(output).unwrap().into_raw()
}

#[unsafe(no_mangle)]
pub extern "C" fn text_engine_free_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        unsafe {
            let _ = CString::from_raw(ptr);
        }
    }
}
