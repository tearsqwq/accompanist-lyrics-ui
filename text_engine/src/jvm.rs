use jni::objects::{JByteBuffer, JObject, JString};
use jni::sys::{jboolean, jbyteArray, jfloat, jint};
use jni::JNIEnv;
use once_cell::sync::Lazy;
use std::sync::Mutex;

use crate::core::TextEngine;

// Global singleton for now, or use a handle map for multiple instances.
// For simplicity in this demo, a global instance protected by a Mutex.
pub static ENGINE: Lazy<Mutex<TextEngine>> = Lazy::new(|| Mutex::new(TextEngine::new(2048, 2048)));

#[no_mangle]
pub unsafe extern "C" fn Java_com_mocharealm_accompanist_lyrics_text_NativeTextEngine_init(
    _env: JNIEnv,
    _this: JObject,
    atlas_width: jint,
    atlas_height: jint,
) {
    // Initialize Android logger on first init
    crate::init_logging();

    let mut engine = ENGINE.lock().unwrap();
    *engine = TextEngine::new(atlas_width as u32, atlas_height as u32);
}

#[no_mangle]
pub unsafe extern "C" fn Java_com_mocharealm_accompanist_lyrics_text_NativeTextEngine_loadFont(
    env: JNIEnv,
    _this: JObject,
    bytes: jbyteArray,
) {
    let byte_vec = env.convert_byte_array(bytes).unwrap_or_default();
    let mut engine = ENGINE.lock().unwrap();
    engine.load_font(byte_vec);
}

#[no_mangle]
pub unsafe extern "C" fn Java_com_mocharealm_accompanist_lyrics_text_NativeTextEngine_loadFallbackFont(
    env: JNIEnv,
    _this: JObject,
    bytes: jbyteArray,
) {
    let byte_vec = env.convert_byte_array(bytes).unwrap_or_default();
    let mut engine = ENGINE.lock().unwrap();
    engine.load_fallback_font(byte_vec);
}

/// Load a fallback font from a file descriptor (Android only).
/// This is more memory-efficient than passing the entire font as bytes.
#[no_mangle]
pub unsafe extern "C" fn Java_com_mocharealm_accompanist_lyrics_text_NativeTextEngine_loadFallbackFontFd(
    _env: JNIEnv,
    _this: JObject,
    fd: jint,
) -> jboolean {
    let mut engine = ENGINE.lock().unwrap();
    #[cfg(unix)]
    {
        if engine.load_fallback_font_from_fd(fd) {
            1
        } else {
            0
        }
    }
    #[cfg(not(unix))]
    {
        let _ = fd;
        0 // Not supported on non-Unix platforms
    }
}

#[no_mangle]
pub unsafe extern "C" fn Java_com_mocharealm_accompanist_lyrics_text_NativeTextEngine_clearFallbackFonts(
    _env: JNIEnv,
    _this: JObject,
) {
    let mut engine = ENGINE.lock().unwrap();
    engine.clear_fallback_fonts();
}

#[no_mangle]
pub unsafe extern "C" fn Java_com_mocharealm_accompanist_lyrics_text_NativeTextEngine_processText<
    'local,
>(
    env: JNIEnv<'local>,
    _this: JObject<'local>,
    text: JString<'local>,
    size_fn: jfloat,
    weight: jfloat,
) -> JString<'local> {
    let text_str: String = env.get_string(text).map(|s| s.into()).unwrap_or_default();

    let mut engine = ENGINE.lock().unwrap();
    let result = engine.process_text(&text_str, size_fn, weight);

    let json = serde_json::to_string(&result).unwrap_or_else(|_| "{}".to_string());

    env.new_string(&json)
        .unwrap_or_else(|_| env.new_string("{}").unwrap())
}

#[no_mangle]
pub unsafe extern "C" fn Java_com_mocharealm_accompanist_lyrics_text_NativeTextEngine_hasPendingUploads(
    _env: JNIEnv,
    _this: JObject,
) -> jboolean {
    let engine = ENGINE.lock().unwrap();
    if engine.has_pending_uploads() {
        1
    } else {
        0
    }
}

#[no_mangle]
pub unsafe extern "C" fn Java_com_mocharealm_accompanist_lyrics_text_NativeTextEngine_getPendingUploads<
    'local,
>(
    env: JNIEnv<'local>,
    _this: JObject<'local>,
) -> JString<'local> {
    let mut engine = ENGINE.lock().unwrap();
    let uploads = engine.get_pending_uploads();

    // Serialize as JSON array: [{x, y, width, height, data_base64}, ...]
    let json_uploads: Vec<serde_json::Value> = uploads
        .iter()
        .map(|u| {
            serde_json::json!({
                "x": u.x,
                "y": u.y,
                "width": u.width,
                "height": u.height,
                "data": base64_encode(&u.data)
            })
        })
        .collect();

    let json = serde_json::to_string(&json_uploads).unwrap_or_else(|_| "[]".to_string());
    env.new_string(&json)
        .unwrap_or_else(|_| env.new_string("[]").unwrap())
}

#[no_mangle]
pub unsafe extern "C" fn Java_com_mocharealm_accompanist_lyrics_text_NativeTextEngine_getAtlasSize<
    'local,
>(
    env: JNIEnv<'local>,
    _this: JObject<'local>,
) -> JString<'local> {
    let engine = ENGINE.lock().unwrap();
    let (width, height) = engine.get_atlas_size();
    let json = format!(r#"{{"width":{},"height":{}}}"#, width, height);
    env.new_string(&json)
        .unwrap_or_else(|_| env.new_string("{}").unwrap())
}

/// Clear all cached data and reset the engine.
/// Call this when done with text rendering to free memory.
#[no_mangle]
pub unsafe extern "C" fn Java_com_mocharealm_accompanist_lyrics_text_NativeTextEngine_destroy(
    _env: JNIEnv,
    _this: JObject,
) {
    let mut engine = ENGINE.lock().unwrap();
    engine.clear();
}

// Simple base64 encoder (no padding for simplicity)
fn base64_encode(data: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::with_capacity((data.len() + 2) / 3 * 4);

    for chunk in data.chunks(3) {
        let b0 = chunk[0] as usize;
        let b1 = chunk.get(1).copied().unwrap_or(0) as usize;
        let b2 = chunk.get(2).copied().unwrap_or(0) as usize;

        result.push(CHARS[b0 >> 2] as char);
        result.push(CHARS[((b0 & 0x03) << 4) | (b1 >> 4)] as char);

        if chunk.len() > 1 {
            result.push(CHARS[((b1 & 0x0F) << 2) | (b2 >> 6)] as char);
        } else {
            result.push('=');
        }

        if chunk.len() > 2 {
            result.push(CHARS[b2 & 0x3F] as char);
        } else {
            result.push('=');
        }
    }

    result
}

// ============================================================================
// DirectByteBuffer Zero-Copy API
// ============================================================================

/// Helper to write i32 to buffer at offset
fn write_i32(buf: &mut [u8], offset: usize, val: i32) {
    let bytes = val.to_ne_bytes();
    buf[offset..offset + 4].copy_from_slice(&bytes);
}

/// Helper to write f32 to buffer at offset
fn write_f32(buf: &mut [u8], offset: usize, val: f32) {
    let bytes = val.to_ne_bytes();
    buf[offset..offset + 4].copy_from_slice(&bytes);
}

/// Helper to write u16 to buffer at offset
fn write_u16(buf: &mut [u8], offset: usize, val: u16) {
    let bytes = val.to_ne_bytes();
    buf[offset..offset + 2].copy_from_slice(&bytes);
}

/// Process text and write layout results directly into a DirectByteBuffer.
/// Returns the number of glyphs written, or -1 on error.
///
/// Buffer layout (per glyph, 28 bytes each):
/// - offset 0:  u16  glyph_id
/// - offset 2:  u16  reserved (padding)
/// - offset 4:  f32  x_position
/// - offset 8:  f32  y_position
/// - offset 12: f32  atlas_x (u in atlas, normalized 0-1)
/// - offset 16: f32  atlas_y (v in atlas, normalized 0-1)
/// - offset 20: f32  atlas_w (width in atlas, normalized 0-1)
/// - offset 24: f32  atlas_h (height in atlas, normalized 0-1)
///
/// Header (16 bytes):
/// - offset 0:  i32  glyph_count
/// - offset 4:  f32  total_width
/// - offset 8:  f32  ascent
/// - offset 12: f32  descent
#[no_mangle]
pub unsafe extern "C" fn Java_com_mocharealm_accompanist_lyrics_text_NativeTextEngine_processTextDirect(
    env: JNIEnv,
    _this: JObject,
    text: JString,
    size_px: jfloat,
    weight: jfloat,
    buffer: JByteBuffer,
) -> jint {
    // Get text string
    let text_str: String = match env.get_string(text) {
        Ok(s) => s.into(),
        Err(_) => return -1,
    };

    // Get direct buffer as mutable slice
    let buf: &mut [u8] = match env.get_direct_buffer_address(buffer) {
        Ok(slice) => slice,
        Err(_) => return -1,
    };

    let buffer_capacity = buf.len();

    // Process text
    let mut engine = ENGINE.lock().unwrap();
    let result = engine.process_text(&text_str, size_px, weight);

    // Calculate required size
    let header_size = 16; // 4 i32/f32 values
    let glyph_size = 28; // per glyph data
    let required_size = header_size + result.glyph_count * glyph_size;

    if buffer_capacity < required_size {
        return -2; // Buffer too small
    }

    // Get atlas size for normalization
    let (atlas_width, atlas_height) = engine.get_atlas_size();
    let atlas_w_f = atlas_width as f32;
    let atlas_h_f = atlas_height as f32;

    // Write header
    write_i32(buf, 0, result.glyph_count as i32);
    write_f32(buf, 4, result.total_width);
    write_f32(buf, 8, result.ascent);
    write_f32(buf, 12, result.descent);

    // Write glyph data
    for i in 0..result.glyph_count {
        let offset = header_size + i * glyph_size;

        // Glyph ID (u16)
        write_u16(buf, offset, result.glyph_ids[i] as u16);

        // Reserved padding (u16)
        write_u16(buf, offset + 2, 0);

        // Position (f32 x, f32 y)
        let pos_idx = i * 2;
        write_f32(buf, offset + 4, result.positions[pos_idx]);
        write_f32(buf, offset + 8, result.positions[pos_idx + 1]);

        // Atlas rect (normalized f32 x, y, w, h)
        let rect_idx = i * 4;
        write_f32(buf, offset + 12, result.atlas_rects[rect_idx] / atlas_w_f);
        write_f32(
            buf,
            offset + 16,
            result.atlas_rects[rect_idx + 1] / atlas_h_f,
        );
        write_f32(
            buf,
            offset + 20,
            result.atlas_rects[rect_idx + 2] / atlas_w_f,
        );
        write_f32(
            buf,
            offset + 24,
            result.atlas_rects[rect_idx + 3] / atlas_h_f,
        );
    }

    result.glyph_count as jint
}

/// Write pending atlas uploads directly into a DirectByteBuffer.
/// Returns the number of upload regions written, or -1 on error.
///
/// Buffer layout:
/// - offset 0: i32 upload_count
/// - For each upload:
///   - i32 x, i32 y, i32 width, i32 height (16 bytes)
///   - [u8; width * height * 4] RGBA data
#[no_mangle]
pub unsafe extern "C" fn Java_com_mocharealm_accompanist_lyrics_text_NativeTextEngine_getPendingUploadsDirect(
    env: JNIEnv,
    _this: JObject,
    buffer: JByteBuffer,
) -> jint {
    // Get direct buffer as mutable slice
    let buf: &mut [u8] = match env.get_direct_buffer_address(buffer) {
        Ok(slice) => slice,
        Err(_) => return -1,
    };

    let buffer_capacity = buf.len();

    let mut engine = ENGINE.lock().unwrap();
    let uploads = engine.get_pending_uploads();

    if uploads.is_empty() {
        // Write zero count
        write_i32(buf, 0, 0);
        return 0;
    }

    // Calculate required size
    let mut required_size = 4; // upload_count
    for upload in &uploads {
        required_size += 16; // x, y, width, height
        required_size += (upload.width * upload.height * 4) as usize; // RGBA data
    }

    if buffer_capacity < required_size {
        return -2; // Buffer too small
    }

    let mut offset = 0;

    // Write upload count
    write_i32(buf, offset, uploads.len() as i32);
    offset += 4;

    // Write each upload
    for upload in &uploads {
        // Header: x, y, width, height
        write_i32(buf, offset, upload.x as i32);
        offset += 4;
        write_i32(buf, offset, upload.y as i32);
        offset += 4;
        write_i32(buf, offset, upload.width as i32);
        offset += 4;
        write_i32(buf, offset, upload.height as i32);
        offset += 4;

        // RGBA data
        let data_size = upload.data.len();
        buf[offset..offset + data_size].copy_from_slice(&upload.data);
        offset += data_size;
    }

    uploads.len() as jint
}
