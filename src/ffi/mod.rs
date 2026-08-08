// ============================================================================
// FFI EXPORTS — mod.rs
// Feature: "ffi"
//
// C-compatible API for integrating hrest-core into any language
// via native shared libraries (.so / .dll / .dylib).
//
// Memory contract:
//   - hrest_encode  → caller must free returned buffer with hrest_free_bytes
//   - hrest_decode  → caller must free returned string with hrest_free_str
//   - hrest_error_str → caller must free returned string with hrest_free_str
//
// All functions are null-safe and never panic.
// Errors are signaled by returning null pointers.
// ============================================================================

use std::ffi::{CStr, CString};
use std::os::raw::c_char;

use crate::infrastructure::contract_loader::JsonContractLoader;
use crate::{decode, encode};

// ---------------------------------------------------------------------------
// Encode
// ---------------------------------------------------------------------------

/// Encode a JSON payload to a binary HRest TLV buffer.
///
/// # Parameters
/// - `route`         — null-terminated C string (e.g. `"POST /api/v1/test/run"`)
/// - `json`          — null-terminated C string containing a JSON object
/// - `contract_json` — null-terminated C string containing `hrest-contract.json`
/// - `out_len`       — pointer to a `usize` that will receive the buffer length
///
/// # Returns
/// Pointer to a heap-allocated byte buffer on success, or `NULL` on error.
/// Must be freed with [`hrest_free_bytes`].
///
/// # Safety
/// All pointer arguments must be valid, non-null, null-terminated C strings.
/// `out_len` must be a valid writable pointer.
#[no_mangle]
pub unsafe extern "C" fn hrest_encode(
    route: *const c_char,
    json: *const c_char,
    contract_json: *const c_char,
    out_len: *mut usize,
) -> *mut u8 {
    // Initialize output length to 0 in case we return early
    if !out_len.is_null() {
        *out_len = 0;
    }

    let result = ffi_encode(route, json, contract_json);

    match result {
        Ok(bytes) => {
            let len = bytes.len();
            let mut boxed = bytes.into_boxed_slice();
            let ptr = boxed.as_mut_ptr();
            std::mem::forget(boxed);

            if !out_len.is_null() {
                *out_len = len;
            }
            ptr
        }
        Err(_) => std::ptr::null_mut(),
    }
}

/// Decode a binary HRest TLV buffer to a JSON string.
///
/// # Parameters
/// - `route`         — null-terminated C string
/// - `bytes`         — pointer to the binary buffer
/// - `bytes_len`     — number of bytes in the buffer
/// - `contract_json` — null-terminated C string containing `hrest-contract.json`
///
/// # Returns
/// Pointer to a null-terminated, heap-allocated UTF-8 string on success,
/// or `NULL` on error. Must be freed with [`hrest_free_str`].
///
/// # Safety
/// - `route` and `contract_json` must be valid null-terminated C strings.
/// - `bytes` must point to a valid buffer of at least `bytes_len` bytes.
#[no_mangle]
pub unsafe extern "C" fn hrest_decode(
    route: *const c_char,
    bytes: *const u8,
    bytes_len: usize,
    contract_json: *const c_char,
) -> *mut c_char {
    let result = ffi_decode(route, bytes, bytes_len, contract_json);

    match result {
        Ok(json) => CString::new(json)
            .map(CString::into_raw)
            .unwrap_or(std::ptr::null_mut()),
        Err(_) => std::ptr::null_mut(),
    }
}

// ---------------------------------------------------------------------------
// Contract utilities
// ---------------------------------------------------------------------------

/// Verify that a client-provided hash matches the contract's stored hash.
///
/// # Returns
/// `1` if hashes match, `0` otherwise.
///
/// # Safety
/// Both pointer arguments must be valid, null-terminated C strings.
#[no_mangle]
pub unsafe extern "C" fn hrest_verify_hash(
    contract_json: *const c_char,
    client_hash: *const c_char,
) -> i32 {
    let result = (|| -> Result<bool, Box<dyn std::error::Error>> {
        let contract_str = CStr::from_ptr(contract_json).to_str()?;
        let hash_str = CStr::from_ptr(client_hash).to_str()?;
        let loader = JsonContractLoader::from_str(contract_str)?;
        Ok(loader.verify_hash(hash_str))
    })();

    match result {
        Ok(true) => 1,
        _ => 0,
    }
}

/// Compute the SHA-256 hash of a contract JSON string.
///
/// # Returns
/// Null-terminated hex string on success, or `NULL` on error.
/// Must be freed with [`hrest_free_str`].
///
/// # Safety
/// `contract_json` must be a valid, null-terminated C string.
#[no_mangle]
pub unsafe extern "C" fn hrest_compute_hash(contract_json: *const c_char) -> *mut c_char {
    let result = (|| -> Result<String, Box<dyn std::error::Error>> {
        let json = CStr::from_ptr(contract_json).to_str()?;
        Ok(JsonContractLoader::compute_hash(json)?)
    })();

    match result {
        Ok(hash) => CString::new(hash)
            .map(CString::into_raw)
            .unwrap_or(std::ptr::null_mut()),
        Err(_) => std::ptr::null_mut(),
    }
}

// ---------------------------------------------------------------------------
// Memory management
// ---------------------------------------------------------------------------

/// Free a byte buffer allocated by [`hrest_encode`].
///
/// # Safety
/// `ptr` must have been returned by `hrest_encode` with the exact `len`
/// that was written to `out_len`.
#[no_mangle]
pub unsafe extern "C" fn hrest_free_bytes(ptr: *mut u8, len: usize) {
    if !ptr.is_null() && len > 0 {
        drop(Box::from_raw(std::slice::from_raw_parts_mut(ptr, len)));
    }
}

/// Free a C string allocated by [`hrest_decode`] or [`hrest_compute_hash`].
///
/// # Safety
/// `ptr` must have been returned by one of the above functions.
#[no_mangle]
pub unsafe extern "C" fn hrest_free_str(ptr: *mut c_char) {
    if !ptr.is_null() {
        drop(CString::from_raw(ptr));
    }
}

// ---------------------------------------------------------------------------
// Safe inner functions — unwrap all C pointer handling here
// ---------------------------------------------------------------------------

unsafe fn ffi_encode(
    route: *const c_char,
    json: *const c_char,
    contract_json: *const c_char,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let route_str = CStr::from_ptr(route).to_str()?;
    let json_str = CStr::from_ptr(json).to_str()?;
    let contract_str = CStr::from_ptr(contract_json).to_str()?;

    let loader = JsonContractLoader::from_str(contract_str)?;
    let bytes = encode(route_str, json_str, &loader)?;
    Ok(bytes)
}

unsafe fn ffi_decode(
    route: *const c_char,
    bytes: *const u8,
    bytes_len: usize,
    contract_json: *const c_char,
) -> Result<String, Box<dyn std::error::Error>> {
    let route_str = CStr::from_ptr(route).to_str()?;
    let contract_str = CStr::from_ptr(contract_json).to_str()?;
    let byte_slice = std::slice::from_raw_parts(bytes, bytes_len);

    let loader = JsonContractLoader::from_str(contract_str)?;
    let json = decode(route_str, byte_slice, &loader)?;
    Ok(json)
}
