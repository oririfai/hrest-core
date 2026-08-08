// ============================================================================
// INFRASTRUCTURE LAYER — varint.rs
//
// Zigzag varint encoding/decoding for i64 integers.
//
// Zigzag maps signed integers to unsigned:
//   0 → 0,  -1 → 1,  1 → 2,  -2 → 3,  2 → 4, ...
// This ensures small negative numbers encode efficiently.
//
// SOLID-S: Only responsible for varint byte encoding math.
// ============================================================================

use crate::domain::error::HrestError;

// Maximum bytes a 64-bit varint can occupy (ceil(64/7) = 10)
const MAX_VARINT_BYTES: usize = 10;

/// Encode a signed 64-bit integer using zigzag varint encoding.
///
/// Output is 1–10 bytes. Small absolute values produce fewer bytes.
pub(crate) fn encode_varint(n: i64) -> Vec<u8> {
    // Zigzag: map signed → unsigned
    let mut zigzag = ((n << 1) ^ (n >> 63)) as u64;

    let mut buf = Vec::with_capacity(MAX_VARINT_BYTES);

    loop {
        // Take 7 low bits
        let byte = (zigzag & 0x7F) as u8;
        zigzag >>= 7;

        if zigzag == 0 {
            // Last byte: MSB = 0 (continuation bit not set)
            buf.push(byte);
            break;
        } else {
            // More bytes follow: MSB = 1
            buf.push(byte | 0x80);
        }
    }

    buf
}

/// Decode a zigzag varint from the start of `bytes`.
///
/// Returns `(value, bytes_consumed)`.
///
/// # Errors
/// - `HrestError::VarintError` — varint is truncated or overflows 64 bits.
pub(crate) fn decode_varint(bytes: &[u8]) -> Result<(i64, usize), HrestError> {
    let mut result: u64 = 0;
    let mut shift: u32 = 0;

    for (i, &byte) in bytes.iter().enumerate() {
        if shift >= 64 {
            return Err(HrestError::VarintError(
                "Varint overflow: exceeds 64-bit capacity".into(),
            ));
        }

        // Accumulate 7 data bits
        result |= ((byte & 0x7F) as u64) << shift;
        shift += 7;

        if byte & 0x80 == 0 {
            // Continuation bit not set → last byte
            // Zigzag decode: unsigned → signed
            let decoded = ((result >> 1) as i64) ^ -((result & 1) as i64);
            return Ok((decoded, i + 1));
        }
    }

    Err(HrestError::VarintError(
        "Varint truncated: stream ended before terminating byte".into(),
    ))
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(n: i64) -> i64 {
        let encoded = encode_varint(n);
        let (decoded, consumed) = decode_varint(&encoded).expect("Should decode");
        assert_eq!(consumed, encoded.len(), "Should consume all bytes for {}", n);
        decoded
    }

    #[test]
    fn round_trip_zero() {
        assert_eq!(round_trip(0), 0);
    }

    #[test]
    fn round_trip_positive() {
        assert_eq!(round_trip(1), 1);
        assert_eq!(round_trip(127), 127);
        assert_eq!(round_trip(128), 128);
        assert_eq!(round_trip(i32::MAX as i64), i32::MAX as i64);
    }

    #[test]
    fn round_trip_negative() {
        assert_eq!(round_trip(-1), -1);
        assert_eq!(round_trip(-127), -127);
        assert_eq!(round_trip(-128), -128);
        assert_eq!(round_trip(i32::MIN as i64), i32::MIN as i64);
    }

    #[test]
    fn round_trip_i64_extremes() {
        assert_eq!(round_trip(i64::MAX), i64::MAX);
        assert_eq!(round_trip(i64::MIN), i64::MIN);
    }

    #[test]
    fn small_values_encode_compactly() {
        // 0 should encode to 1 byte
        assert_eq!(encode_varint(0).len(), 1);
        // 1 and -1 should also be 1 byte
        assert_eq!(encode_varint(1).len(), 1);
        assert_eq!(encode_varint(-1).len(), 1);
        // 63 and -64 should be 1 byte (max for 7-bit zigzag)
        assert_eq!(encode_varint(63).len(), 1);
        assert_eq!(encode_varint(-64).len(), 1);
    }

    #[test]
    fn truncated_varint_returns_error() {
        // A byte with continuation bit set but no following bytes
        let truncated = &[0x80u8];
        let result = decode_varint(truncated);
        assert!(matches!(result, Err(HrestError::VarintError(_))));
    }

    #[test]
    fn decode_with_trailing_bytes() {
        // Encode 42, then append extra bytes — should only consume varint bytes
        let mut buf = encode_varint(42);
        let varint_len = buf.len();
        buf.extend_from_slice(&[0xFF, 0xFF]);
        let (val, consumed) = decode_varint(&buf).expect("Should decode");
        assert_eq!(val, 42);
        assert_eq!(consumed, varint_len);
    }
}
