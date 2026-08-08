// ============================================================================
// WASM EXPORTS — mod.rs
// Feature: "wasm"
//
// WebAssembly bindings for hrest-core, enabling use in browsers and Node.js
// via the generated JavaScript/TypeScript wrapper from wasm-bindgen.
//
// Build with:
//   wasm-pack build --features wasm --target web
//   wasm-pack build --features wasm --target nodejs
// ============================================================================

use wasm_bindgen::prelude::*;

use crate::infrastructure::contract_loader::JsonContractLoader;
use crate::{decode, encode};

// ---------------------------------------------------------------------------
// Core encode / decode
// ---------------------------------------------------------------------------

/// Encode a JSON payload string into a binary HRest TLV `Uint8Array`.
///
/// @param {string} route         - Route key (e.g. `"POST /api/v1/test/run"`)
/// @param {string} json          - JSON object payload as a string
/// @param {string} contractJson  - Contents of `hrest-contract.json`
/// @returns {Uint8Array} Encoded binary payload
/// @throws {Error} On unknown field, unknown route, or malformed payload
#[wasm_bindgen(js_name = encode)]
pub fn wasm_encode(route: &str, json: &str, contract_json: &str) -> Result<Vec<u8>, JsValue> {
    let loader = JsonContractLoader::from_str(contract_json)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;

    encode(route, json, &loader).map_err(|e| JsValue::from_str(&e.to_string()))
}

/// Decode a binary HRest TLV `Uint8Array` back to a JSON string.
///
/// @param {string}     route        - Route key matching the one used during encoding
/// @param {Uint8Array} bytes        - The binary payload to decode
/// @param {string}     contractJson - Contents of `hrest-contract.json`
/// @returns {string} Decoded JSON string
/// @throws {Error} On unknown token, buffer overflow, or malformed bytes
#[wasm_bindgen(js_name = decode)]
pub fn wasm_decode(route: &str, bytes: &[u8], contract_json: &str) -> Result<String, JsValue> {
    let loader = JsonContractLoader::from_str(contract_json)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;

    decode(route, bytes, &loader).map_err(|e| JsValue::from_str(&e.to_string()))
}

// ---------------------------------------------------------------------------
// Contract utilities
// ---------------------------------------------------------------------------

/// Verify that a client-provided hash matches the loaded contract's hash.
///
/// @param {string} contractJson - Contents of `hrest-contract.json`
/// @param {string} clientHash   - The hash from the `X-Hrest-Hash` header
/// @returns {boolean}
#[wasm_bindgen(js_name = verifyHash)]
pub fn wasm_verify_hash(contract_json: &str, client_hash: &str) -> bool {
    JsonContractLoader::from_str(contract_json)
        .map(|loader| loader.verify_hash(client_hash))
        .unwrap_or(false)
}

/// Compute the SHA-256 hash of the contract's route structure.
/// Use this to generate the value for the `contract_hash` field.
///
/// @param {string} contractJson - Contents of `hrest-contract.json`
/// @returns {string} SHA-256 hex string
/// @throws {Error} If the contract JSON is malformed
#[wasm_bindgen(js_name = computeHash)]
pub fn wasm_compute_hash(contract_json: &str) -> Result<String, JsValue> {
    JsonContractLoader::compute_hash(contract_json)
        .map_err(|e| JsValue::from_str(&e.to_string()))
}

// ---------------------------------------------------------------------------
// Metadata
// ---------------------------------------------------------------------------

/// Returns the HRest protocol version this build implements.
#[wasm_bindgen(js_name = protocolVersion)]
pub fn wasm_protocol_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}
