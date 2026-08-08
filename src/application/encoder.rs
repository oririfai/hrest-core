// ============================================================================
// APPLICATION LAYER — encoder.rs
//
// Use case: Encode a JSON payload into a binary HRest TLV stream.
//
// Wire format produced:
//   Packet = (FieldID:u8  Value)*
//   Value  =
//     0x00 → Null                            (no value bytes)
//     0x01 → [u16-LE len][utf8 bytes]
//     0x02 → [zigzag varint bytes]
//     0x03 → [0x00 | 0x01]
//     0x04 → [u16-LE len][raw bytes]
//     0x05 → [0x00=object | 0x01=array]
//              object: [u16-LE count] (FieldID Value)*
//              array:  [u16-LE count] Value*
//     0x06 → [f64 LE 8 bytes]
//
// SOLID-S: Only responsible for encoding logic.
// SOLID-D: Depends on ContractProvider trait, not concrete loader.
// KISS: Two entry points — encode_payload (advanced) used by lib.rs (simple).
// ============================================================================

use serde_json::Value as Json;

use crate::application::ports::ContractProvider;
use crate::domain::contract::FieldMap;
use crate::domain::data_type::{
    NESTED_KIND_ARRAY, NESTED_KIND_OBJECT, TYPE_BOOL, TYPE_BYTES, TYPE_FLOAT, TYPE_INT,
    TYPE_NESTED, TYPE_NULL, TYPE_STRING,
};
use crate::domain::error::HrestError;
use crate::infrastructure::varint::encode_varint;

// ---------------------------------------------------------------------------
// Public use-case entry point
// ---------------------------------------------------------------------------

/// Encode a JSON object payload into a binary HRest TLV stream.
///
/// The top-level `payload` **must** be a JSON object (`{}`).
/// Nested arrays and objects are supported recursively.
///
/// # Errors
/// - `HrestError::UnknownRoute`  — route not in contract
/// - `HrestError::UnknownField`  — field not whitelisted (→ HTTP 422)
/// - `HrestError::MalformedPayload` — payload is not a JSON object, or
///   a string/array exceeds 65 535 elements/bytes
pub fn encode_payload(
    route: &str,
    payload: &Json,
    contract: &impl ContractProvider,
) -> Result<Vec<u8>, HrestError> {
    let field_map = contract.contract().field_map(route)?;

    let obj = payload.as_object().ok_or_else(|| {
        HrestError::MalformedPayload(
            "Top-level payload must be a JSON object `{}`".into(),
        )
    })?;

    let mut out = Vec::new();
    encode_object_fields(&mut out, obj, field_map)?;
    Ok(out)
}

// ---------------------------------------------------------------------------
// Private helpers — each function has a single responsibility (SOLID-S)
// ---------------------------------------------------------------------------

/// Write all fields of a JSON object as `[field_id][Value]` pairs.
fn encode_object_fields(
    out: &mut Vec<u8>,
    obj: &serde_json::Map<String, Json>,
    field_map: &FieldMap,
) -> Result<(), HrestError> {
    for (key, value) in obj {
        let field_id = field_map
            .field_id(key)
            .ok_or_else(|| HrestError::UnknownField(key.clone()))?;

        out.push(field_id);
        encode_value(out, value, field_map)?;
    }
    Ok(())
}

/// Write `[type_byte][value_bytes]` for any JSON value (recursive).
fn encode_value(out: &mut Vec<u8>, value: &Json, field_map: &FieldMap) -> Result<(), HrestError> {
    match value {
        Json::Null => encode_null(out),

        Json::String(s) => encode_string(out, s)?,

        Json::Number(n) => encode_number(out, n)?,

        Json::Bool(b) => encode_bool(out, *b),

        Json::Array(arr) => encode_array(out, arr, field_map)?,

        Json::Object(obj) => encode_nested_object(out, obj, field_map)?,
    }
    Ok(())
}

/// `0x00` — Null: type byte only, no value bytes.
#[inline]
fn encode_null(out: &mut Vec<u8>) {
    out.push(TYPE_NULL);
}

/// `0x01 [u16-LE len] [utf8 bytes]`
fn encode_string(out: &mut Vec<u8>, s: &str) -> Result<(), HrestError> {
    let bytes = s.as_bytes();
    let len = u16::try_from(bytes.len()).map_err(|_| {
        HrestError::MalformedPayload(format!(
            "String too long: {} bytes (max 65535)",
            bytes.len()
        ))
    })?;
    out.push(TYPE_STRING);
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(bytes);
    Ok(())
}

/// `0x02 [zigzag varint]` for integers, or `0x06 [f64 LE]` for floats.
fn encode_number(out: &mut Vec<u8>, n: &serde_json::Number) -> Result<(), HrestError> {
    if n.is_i64() {
        let i = n.as_i64().unwrap(); // safe: is_i64() confirmed
        out.push(TYPE_INT);
        out.extend_from_slice(&encode_varint(i));
    } else if n.is_f64() {
        let f = n.as_f64().unwrap(); // safe: is_f64() confirmed
        out.push(TYPE_FLOAT);
        out.extend_from_slice(&f.to_le_bytes());
    } else {
        // u64 > i64::MAX — cannot encode in current protocol
        return Err(HrestError::MalformedPayload(format!(
            "Integer {} exceeds i64::MAX; use a float for large unsigned values",
            n
        )));
    }
    Ok(())
}

/// `0x03 [0x00 | 0x01]`
#[inline]
fn encode_bool(out: &mut Vec<u8>, b: bool) {
    out.push(TYPE_BOOL);
    out.push(b as u8);
}

/// `0x04 [u16-LE len] [raw bytes]`
/// Reserved for programmatic use — e.g. encoding pre-computed UUID bytes.
#[allow(dead_code)]
fn encode_bytes(out: &mut Vec<u8>, bytes: &[u8]) -> Result<(), HrestError> {
    let len = u16::try_from(bytes.len()).map_err(|_| {
        HrestError::MalformedPayload(format!(
            "Bytes value too long: {} bytes (max 65535)",
            bytes.len()
        ))
    })?;
    out.push(TYPE_BYTES);
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(bytes);
    Ok(())
}

/// `0x05 0x01 [u16-LE count] Value*`
fn encode_array(
    out: &mut Vec<u8>,
    arr: &[Json],
    field_map: &FieldMap,
) -> Result<(), HrestError> {
    let count = u16::try_from(arr.len()).map_err(|_| {
        HrestError::MalformedPayload(format!(
            "Array too long: {} elements (max 65535)",
            arr.len()
        ))
    })?;
    out.push(TYPE_NESTED);
    out.push(NESTED_KIND_ARRAY);
    out.extend_from_slice(&count.to_le_bytes());

    for elem in arr {
        encode_value(out, elem, field_map)?;
    }
    Ok(())
}

/// `0x05 0x00 [u16-LE field_count] (FieldID Value)*`
fn encode_nested_object(
    out: &mut Vec<u8>,
    obj: &serde_json::Map<String, Json>,
    field_map: &FieldMap,
) -> Result<(), HrestError> {
    let count = u16::try_from(obj.len()).map_err(|_| {
        HrestError::MalformedPayload(format!(
            "Nested object has too many fields: {} (max 65535)",
            obj.len()
        ))
    })?;
    out.push(TYPE_NESTED);
    out.push(NESTED_KIND_OBJECT);
    out.extend_from_slice(&count.to_le_bytes());

    encode_object_fields(out, obj, field_map)
}
