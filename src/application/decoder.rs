// ============================================================================
// APPLICATION LAYER — decoder.rs
//
// Use case: Decode a binary HRest TLV stream into a JSON value.
//
// Security guarantees (mandated by SCURITY.md):
//  [1] Every read is bounds-checked via ByteCursor::ensure_available
//  [2] Recursive nesting is capped at MAX_NESTING_DEPTH (no stack overflow)
//  [3] Unknown tokens immediately return HTTP-422-equivalent error
//  [4] NaN / Infinity floats are rejected (not representable in JSON)
//  [5] No unwrap() — all fallible ops use ? operator
//
// SOLID-S: Only responsible for decoding logic.
// SOLID-D: Depends on ContractProvider trait, not concrete loaders.
// ============================================================================

use serde_json::Value as Json;

use crate::application::ports::ContractProvider;
use crate::domain::contract::FieldMap;
use crate::domain::data_type::{DataType, COMPACT_SENTINEL, NESTED_KIND_ARRAY, NESTED_KIND_OBJECT};
use crate::domain::error::HrestError;
use crate::infrastructure::varint::decode_varint;

// ---------------------------------------------------------------------------
// Safety constants
// ---------------------------------------------------------------------------

/// Maximum allowed nesting depth for nested objects/arrays.
/// Prevents stack overflow via crafted deeply-nested binary payloads.
const MAX_NESTING_DEPTH: usize = 32;

/// Soft cap on pre-allocated capacity to prevent memory exhaustion.
/// Actual elements are still bounded by buffer size, but this prevents
/// large upfront allocations from a crafted count field.
const MAX_PREALLOC_CAPACITY: usize = 256;

// ---------------------------------------------------------------------------
// Public use-case entry point
// ---------------------------------------------------------------------------

/// Decode a binary HRest TLV stream into a JSON object.
///
/// # Errors
/// - `HrestError::UnknownRoute`    — route not in contract
/// - `HrestError::UnknownToken`    — token ID not whitelisted (→ HTTP 422)
/// - `HrestError::BufferOverflow`  — stream is malformed or truncated
/// - `HrestError::InvalidDataType` — unknown type byte encountered
/// - `HrestError::MalformedPayload`— nesting depth exceeded, NaN float, etc.
pub fn decode_payload(
    route: &str,
    bytes: &[u8],
    contract: &impl ContractProvider,
) -> Result<Json, HrestError> {
    let field_map = contract.contract().field_map(route)?;
    let mut cursor = ByteCursor::new(bytes);
    // Start at depth 0 — top-level packet is not counted as a nesting level
    read_top_level_fields(&mut cursor, field_map)
}

// ---------------------------------------------------------------------------
// ByteCursor — safe, bounds-checked byte stream reader
//
// SCURITY.md requirement: every byte read validates bounds before access.
// ---------------------------------------------------------------------------

struct ByteCursor<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> ByteCursor<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    /// Number of bytes remaining unread.
    fn remaining(&self) -> usize {
        self.data.len() - self.pos
    }

    /// Returns `true` when all bytes have been consumed.
    fn is_exhausted(&self) -> bool {
        self.pos >= self.data.len()
    }

    /// Read a single byte. Bounds-checked.
    fn read_u8(&mut self) -> Result<u8, HrestError> {
        self.ensure_available(1)?;
        let byte = self.data[self.pos];
        self.pos += 1;
        Ok(byte)
    }

    /// Read 2 bytes as a little-endian u16. Bounds-checked.
    fn read_u16_le(&mut self) -> Result<u16, HrestError> {
        self.ensure_available(2)?;
        let bytes: [u8; 2] = [self.data[self.pos], self.data[self.pos + 1]];
        self.pos += 2;
        Ok(u16::from_le_bytes(bytes))
    }

    /// Read exactly `len` bytes and return a slice. Bounds-checked.
    fn read_bytes(&mut self, len: usize) -> Result<&[u8], HrestError> {
        self.ensure_available(len)?;
        let slice = &self.data[self.pos..self.pos + len];
        self.pos += len;
        Ok(slice)
    }

    /// Read exactly 8 bytes as a fixed-size array. Bounds-checked. No unwrap.
    fn read_bytes_8(&mut self) -> Result<[u8; 8], HrestError> {
        self.ensure_available(8)?;
        let arr: [u8; 8] = self.data[self.pos..self.pos + 8]
            .try_into()
            .map_err(|_| HrestError::MalformedPayload(
                "Internal: failed to read 8-byte float slice".into()
            ))?;
        self.pos += 8;
        Ok(arr)
    }

    /// Read exactly 4 bytes as a fixed-size array. Bounds-checked. No unwrap.
    /// Used for f32 (TYPE_FLOAT32) in wire format v2.
    fn read_bytes_4(&mut self) -> Result<[u8; 4], HrestError> {
        self.ensure_available(4)?;
        let arr: [u8; 4] = self.data[self.pos..self.pos + 4]
            .try_into()
            .map_err(|_| HrestError::MalformedPayload(
                "Internal: failed to read 4-byte float slice".into()
            ))?;
        self.pos += 4;
        Ok(arr)
    }

    /// Return remaining bytes starting from current position (for varint).
    fn remaining_slice(&self) -> &[u8] {
        &self.data[self.pos..]
    }

    /// Advance cursor by `n` bytes (used after varint decode). Bounds-checked.
    fn advance(&mut self, n: usize) -> Result<(), HrestError> {
        self.ensure_available(n)?;
        self.pos += n;
        Ok(())
    }

    /// Bounds check — returns BufferOverflow if fewer than `needed` bytes remain.
    #[inline]
    fn ensure_available(&self, needed: usize) -> Result<(), HrestError> {
        let available = self.remaining();
        if needed > available {
            Err(HrestError::BufferOverflow { needed, available })
        } else {
            Ok(())
        }
    }
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

/// Read top-level `(field_id, value)` pairs until the cursor is exhausted.
/// Top-level is not counted as a nesting level (depth starts at 0).
fn read_top_level_fields(cursor: &mut ByteCursor, field_map: &FieldMap) -> Result<Json, HrestError> {
    let mut map = serde_json::Map::new();

    while !cursor.is_exhausted() {
        let field_id = cursor.read_u8()?;

        let field_name = field_map
            .field_name(field_id)
            .ok_or(HrestError::UnknownToken(field_id))?
            .to_string();

        let value = read_value(cursor, field_map, 0)?;
        map.insert(field_name, value);
    }

    Ok(Json::Object(map))
}

/// Read a typed value from the cursor.
///
/// `depth` tracks how many nested levels deep we are.
/// When a Nested type is encountered, depth is incremented before recursing.
fn read_value(cursor: &mut ByteCursor, field_map: &FieldMap, depth: usize) -> Result<Json, HrestError> {
    let type_byte = cursor.read_u8()?;
    let data_type = DataType::try_from(type_byte)?;

    match data_type {
        DataType::Null    => read_null(),
        DataType::Str     => read_string(cursor),
        DataType::Int     => read_int(cursor),
        DataType::Bool    => read_bool(cursor),
        DataType::Bytes   => read_bytes(cursor),
        DataType::Float   => read_float(cursor),
        DataType::Float32 => read_float32(cursor),
        DataType::Nested  => read_nested(cursor, field_map, depth + 1),
    }
}

/// `0x00` — returns `Json::Null`. No bytes consumed.
#[inline]
fn read_null() -> Result<Json, HrestError> {
    Ok(Json::Null)
}

/// `0x01 [u8 len | 0xFF u16-LE] [utf8 bytes]`  — wire format v2 compact length.
///
/// If the length byte is 0xFF (COMPACT_SENTINEL), the following 2 bytes
/// hold the actual u16-LE length. Otherwise the single byte is the length.
fn read_string(cursor: &mut ByteCursor) -> Result<Json, HrestError> {
    let len = read_compact_len(cursor)?;
    let bytes = cursor.read_bytes(len)?;

    let s = std::str::from_utf8(bytes).map_err(|e| {
        HrestError::MalformedPayload(format!("Invalid UTF-8 in string field: {}", e))
    })?;

    Ok(Json::String(s.to_string()))
}

/// `0x02 [zigzag varint]` → `Json::Number`
fn read_int(cursor: &mut ByteCursor) -> Result<Json, HrestError> {
    let (value, consumed) = decode_varint(cursor.remaining_slice())?;
    cursor.advance(consumed)?;
    Ok(Json::Number(serde_json::Number::from(value)))
}

/// `0x03 [0x00 | 0x01]` → `Json::Bool`
fn read_bool(cursor: &mut ByteCursor) -> Result<Json, HrestError> {
    let byte = cursor.read_u8()?;
    Ok(Json::Bool(byte != 0))
}

/// `0x04 [u8 len | 0xFF u16-LE] [raw bytes]`  — v2 compact length.
///
/// Raw bytes are hex-encoded in JSON output to safely represent all bit patterns.
fn read_bytes(cursor: &mut ByteCursor) -> Result<Json, HrestError> {
    let len = read_compact_len(cursor)?;
    let bytes = cursor.read_bytes(len)?;
    Ok(Json::String(hex::encode(bytes)))
}

/// `0x06 [f64 LE 8 bytes]` → `Json::Number`
///
/// Security: NaN and Infinity are rejected — not representable in JSON.
/// No unwrap() — uses bounds-checked read_bytes_8().
fn read_float(cursor: &mut ByteCursor) -> Result<Json, HrestError> {
    let arr = cursor.read_bytes_8()?;
    let f = f64::from_le_bytes(arr);

    serde_json::Number::from_f64(f)
        .map(Json::Number)
        .ok_or_else(|| {
            HrestError::MalformedPayload(
                "Float64 value is NaN or Infinity — not representable in JSON (security reject)"
                    .into(),
            )
        })
}

/// `0x07 [f32 LE 4 bytes]` → `Json::Number`  — wire format v2.
///
/// Decoded as f32, then widened to f64 for JSON (all f32 values are valid f64).
/// Security: NaN and Infinity are still rejected.
fn read_float32(cursor: &mut ByteCursor) -> Result<Json, HrestError> {
    let arr = cursor.read_bytes_4()?;
    let f = f32::from_le_bytes(arr) as f64; // widen to f64 for JSON compatibility

    serde_json::Number::from_f64(f)
        .map(Json::Number)
        .ok_or_else(|| {
            HrestError::MalformedPayload(
                "Float32 value is NaN or Infinity — not representable in JSON (security reject)"
                    .into(),
            )
        })
}

/// `0x05 [kind] ...` → object or array (recursive, depth-limited)
///
/// Security: checked against MAX_NESTING_DEPTH before any allocation.
fn read_nested(cursor: &mut ByteCursor, field_map: &FieldMap, depth: usize) -> Result<Json, HrestError> {
    // [1] Depth guard — prevents stack overflow via crafted deep binary
    if depth > MAX_NESTING_DEPTH {
        return Err(HrestError::MalformedPayload(format!(
            "Nesting depth {} exceeds maximum allowed ({}) — possible attack vector",
            depth, MAX_NESTING_DEPTH
        )));
    }

    let kind = cursor.read_u8()?;

    match kind {
        NESTED_KIND_OBJECT => read_nested_object(cursor, field_map, depth),
        NESTED_KIND_ARRAY  => read_nested_array(cursor, field_map, depth),
        other => Err(HrestError::MalformedPayload(format!(
            "Unknown nested kind 0x{:02X} (expected 0x00=object or 0x01=array)",
            other
        ))),
    }
}

/// `0x05 0x00 [u8 count | 0xFF u16-LE] (FieldID Value)*` → `Json::Object`
fn read_nested_object(cursor: &mut ByteCursor, field_map: &FieldMap, depth: usize) -> Result<Json, HrestError> {
    let field_count = read_compact_count(cursor)?;

    // [2] Soft-cap pre-allocation to prevent memory exhaustion
    let capacity = field_count.min(MAX_PREALLOC_CAPACITY);
    let mut map = serde_json::Map::with_capacity(capacity);

    for _ in 0..field_count {
        let field_id = cursor.read_u8()?;
        let field_name = field_map
            .field_name(field_id)
            .ok_or(HrestError::UnknownToken(field_id))?
            .to_string();

        let value = read_value(cursor, field_map, depth)?;
        map.insert(field_name, value);
    }

    Ok(Json::Object(map))
}

/// `0x05 0x01 [u8 count | 0xFF u16-LE] Value*` → `Json::Array`
fn read_nested_array(cursor: &mut ByteCursor, field_map: &FieldMap, depth: usize) -> Result<Json, HrestError> {
    let elem_count = read_compact_count(cursor)?;

    // [2] Soft-cap pre-allocation to prevent memory exhaustion
    let capacity = elem_count.min(MAX_PREALLOC_CAPACITY);
    let mut arr = Vec::with_capacity(capacity);

    for _ in 0..elem_count {
        arr.push(read_value(cursor, field_map, depth)?);
    }

    Ok(Json::Array(arr))
}

// ---------------------------------------------------------------------------
// Wire format v2 helpers — compact length/count decoding
// ---------------------------------------------------------------------------

/// Read a compact length (v2 format):
/// - If next byte < 0xFF: that byte IS the length (1 byte total)
/// - If next byte == 0xFF: read following 2 bytes as u16-LE length
#[inline]
fn read_compact_len(cursor: &mut ByteCursor) -> Result<usize, HrestError> {
    let byte = cursor.read_u8()?;
    if byte < COMPACT_SENTINEL {
        Ok(byte as usize)
    } else {
        Ok(cursor.read_u16_le()? as usize)
    }
}

/// Read a compact count (v2 format): same encoding as compact length.
#[inline]
fn read_compact_count(cursor: &mut ByteCursor) -> Result<usize, HrestError> {
    read_compact_len(cursor)
}
