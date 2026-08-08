// ============================================================================
// hrest-core — lib.rs
// Public API surface (KISS: keep it minimal and obvious)
//
// Clean Architecture entry point:
//   - Declares internal layer modules
//   - Re-exports only what consumers need
//   - Two top-level functions: encode() and decode()
//   - Feature-gates for FFI and WASM
// ============================================================================

// ---------------------------------------------------------------------------
// Internal modules (Clean Architecture layers)
// ---------------------------------------------------------------------------

mod domain;
mod application;
mod infrastructure;

// ---------------------------------------------------------------------------
// Feature-gated export modules
// ---------------------------------------------------------------------------

/// C-compatible FFI exports. Enabled with `--features ffi`.
#[cfg(feature = "ffi")]
pub mod ffi;

/// WebAssembly exports via wasm-bindgen. Enabled with `--features wasm`.
#[cfg(feature = "wasm")]
pub mod wasm;

// ---------------------------------------------------------------------------
// Public re-exports — only what SDK consumers need
// ---------------------------------------------------------------------------

/// The error type for all HRest operations.
pub use domain::error::HrestError;

/// Port trait — implement this to provide a custom contract source.
pub use application::ports::ContractProvider;

/// Primary contract loader — loads from JSON string or file.
pub use infrastructure::contract_loader::JsonContractLoader;

/// Domain primitives for advanced users and SDK implementors.
pub mod types {
    pub use crate::domain::contract::{ContractData, FieldMap};
    pub use crate::domain::data_type::{
        DataType,
        NESTED_KIND_ARRAY, NESTED_KIND_OBJECT,
        TYPE_BOOL, TYPE_BYTES, TYPE_FLOAT, TYPE_INT,
        TYPE_NESTED, TYPE_NULL, TYPE_STRING,
    };
}

// ---------------------------------------------------------------------------
// Public API — two functions, that's it (KISS)
// ---------------------------------------------------------------------------

/// Encode a JSON string payload into a binary HRest TLV stream.
///
/// # Arguments
/// - `route`    — Route key from the contract (e.g. `"POST /api/v1/test/run"`)
/// - `json`     — JSON object payload as a `&str`
/// - `contract` — Any [`ContractProvider`] implementation
///
/// # Returns
/// `Ok(Vec<u8>)` containing the binary TLV packet on success.
///
/// # Errors
/// | Error                  | HTTP status | Cause                            |
/// |------------------------|-------------|----------------------------------|
/// | `UnknownRoute`         | 400         | Route not in contract            |
/// | `UnknownField`         | 422         | Field not in whitelist           |
/// | `MalformedPayload`     | 400         | Payload is not a JSON object     |
/// | `Json`                 | 400         | Invalid JSON input               |
///
/// # Example
/// ```rust
/// use hrest_core::{encode, JsonContractLoader};
///
/// let contract_json = r#"{
///   "version": "1.0.0",
///   "contract_hash": "abc123",
///   "routes": {
///     "POST /api/v1/test/run": {
///       "fields": { "event": 1, "task_id": 2, "headless": 5 }
///     }
///   }
/// }"#;
///
/// let loader = JsonContractLoader::from_str(contract_json).unwrap();
/// let payload = r#"{"event": "start", "task_id": 42, "headless": true}"#;
/// let binary  = encode("POST /api/v1/test/run", payload, &loader).unwrap();
///
/// assert!(!binary.is_empty());
/// ```
pub fn encode(
    route: &str,
    json: &str,
    contract: &impl ContractProvider,
) -> Result<Vec<u8>, HrestError> {
    let payload: serde_json::Value = serde_json::from_str(json)?;
    application::encoder::encode_payload(route, &payload, contract)
}

/// Decode a binary HRest TLV stream back into a JSON string.
///
/// # Arguments
/// - `route`    — Route key matching the one used during encoding
/// - `bytes`    — The binary TLV packet to decode
/// - `contract` — Any [`ContractProvider`] implementation
///
/// # Returns
/// `Ok(String)` containing the reconstructed JSON on success.
///
/// # Errors
/// | Error                  | HTTP status | Cause                             |
/// |------------------------|-------------|-----------------------------------|
/// | `UnknownRoute`         | 400         | Route not in contract             |
/// | `UnknownToken`         | 422         | Token ID not in contract          |
/// | `BufferOverflow`       | 400         | Malformed or truncated bytes      |
/// | `InvalidDataType`      | 400         | Unknown type byte                 |
///
/// # Example
/// ```rust
/// use hrest_core::{encode, decode, JsonContractLoader};
///
/// let contract_json = r#"{
///   "version": "1.0.0",
///   "contract_hash": "abc123",
///   "routes": {
///     "POST /api/v1/test/run": {
///       "fields": { "event": 1, "task_id": 2, "headless": 5 }
///     }
///   }
/// }"#;
///
/// let loader  = JsonContractLoader::from_str(contract_json).unwrap();
/// let payload = r#"{"event": "start", "task_id": 42, "headless": true}"#;
/// let binary  = encode("POST /api/v1/test/run", payload, &loader).unwrap();
/// let restored = decode("POST /api/v1/test/run", &binary, &loader).unwrap();
///
/// let original: serde_json::Value = serde_json::from_str(payload).unwrap();
/// let roundtrip: serde_json::Value = serde_json::from_str(&restored).unwrap();
/// assert_eq!(original, roundtrip);
/// ```
pub fn decode(
    route: &str,
    bytes: &[u8],
    contract: &impl ContractProvider,
) -> Result<String, HrestError> {
    let value = application::decoder::decode_payload(route, bytes, contract)?;
    Ok(serde_json::to_string(&value)?)
}
