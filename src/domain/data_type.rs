// ============================================================================
// DOMAIN LAYER — data_type.rs
//
// Wire format type constants and the DataType enum.
// These constants define the HRest Binary Protocol v1 wire format.
// DO NOT change byte values without updating the protocol version.
//
// SOLID-O: Open for extension (add new variants), closed for modification.
// ============================================================================

use crate::domain::error::HrestError;

// ---------------------------------------------------------------------------
// Wire format type byte constants — HRest Protocol v1
// ---------------------------------------------------------------------------

/// Null value. Field is present but carries no data.
pub const TYPE_NULL: u8 = 0x00;

/// UTF-8 string. Encoded as `[u16 LE length][utf8 bytes]`.
pub const TYPE_STRING: u8 = 0x01;

/// 64-bit signed integer. Encoded as zigzag varint.
pub const TYPE_INT: u8 = 0x02;

/// Boolean. Encoded as `0x00` (false) or `0x01` (true).
pub const TYPE_BOOL: u8 = 0x03;

/// Raw bytes or UUID. Encoded as `[u16 LE length][raw bytes]`.
pub const TYPE_BYTES: u8 = 0x04;

/// Nested object or array.
/// Encoded as `[kind: 0x00=object | 0x01=array][u16 LE count][...]`.
pub const TYPE_NESTED: u8 = 0x05;

/// 64-bit IEEE 754 float. Encoded as 8 bytes little-endian.
pub const TYPE_FLOAT: u8 = 0x06;

// Nested kind discriminants
pub const NESTED_KIND_OBJECT: u8 = 0x00;
pub const NESTED_KIND_ARRAY: u8 = 0x01;

// ---------------------------------------------------------------------------
// DataType enum
// ---------------------------------------------------------------------------

/// Canonical representation of all supported wire-format data types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DataType {
    /// `0x00` — Null: field present, no value bytes.
    Null,
    /// `0x01` — UTF-8 string.
    Str,
    /// `0x02` — 64-bit signed integer (zigzag varint).
    Int,
    /// `0x03` — Boolean.
    Bool,
    /// `0x04` — Raw bytes / UUID.
    Bytes,
    /// `0x05` — Nested object or array.
    Nested,
    /// `0x06` — 64-bit IEEE 754 float.
    Float,
}

impl TryFrom<u8> for DataType {
    type Error = HrestError;

    /// Parse a wire-format type byte into a `DataType`.
    ///
    /// # Errors
    /// Returns `HrestError::InvalidDataType` for unknown byte values.
    fn try_from(byte: u8) -> Result<Self, Self::Error> {
        match byte {
            TYPE_NULL    => Ok(DataType::Null),
            TYPE_STRING  => Ok(DataType::Str),
            TYPE_INT     => Ok(DataType::Int),
            TYPE_BOOL    => Ok(DataType::Bool),
            TYPE_BYTES   => Ok(DataType::Bytes),
            TYPE_NESTED  => Ok(DataType::Nested),
            TYPE_FLOAT   => Ok(DataType::Float),
            other        => Err(HrestError::InvalidDataType(other)),
        }
    }
}

impl From<DataType> for u8 {
    fn from(dt: DataType) -> u8 {
        match dt {
            DataType::Null   => TYPE_NULL,
            DataType::Str    => TYPE_STRING,
            DataType::Int    => TYPE_INT,
            DataType::Bool   => TYPE_BOOL,
            DataType::Bytes  => TYPE_BYTES,
            DataType::Nested => TYPE_NESTED,
            DataType::Float  => TYPE_FLOAT,
        }
    }
}

impl std::fmt::Display for DataType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            DataType::Null   => "Null",
            DataType::Str    => "String",
            DataType::Int    => "Int",
            DataType::Bool   => "Bool",
            DataType::Bytes  => "Bytes",
            DataType::Nested => "Nested",
            DataType::Float  => "Float",
        };
        write!(f, "{}", name)
    }
}

// ---------------------------------------------------------------------------
// Unit tests (domain logic only — no I/O)
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_all_types() {
        let types = [
            (TYPE_NULL,   DataType::Null),
            (TYPE_STRING, DataType::Str),
            (TYPE_INT,    DataType::Int),
            (TYPE_BOOL,   DataType::Bool),
            (TYPE_BYTES,  DataType::Bytes),
            (TYPE_NESTED, DataType::Nested),
            (TYPE_FLOAT,  DataType::Float),
        ];

        for (byte, expected) in types {
            let parsed = DataType::try_from(byte).expect("Should parse known byte");
            assert_eq!(parsed, expected);
            assert_eq!(u8::from(parsed), byte);
        }
    }

    #[test]
    fn unknown_type_byte_returns_error() {
        let result = DataType::try_from(0xFF);
        assert!(matches!(result, Err(HrestError::InvalidDataType(0xFF))));
    }
}
