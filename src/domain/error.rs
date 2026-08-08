// ============================================================================
// DOMAIN LAYER — error.rs
//
// All possible errors in the HRest protocol.
// No external deps beyond std + thiserror.
//
// SOLID-S: Single Responsibility — this file owns ONLY error definitions.
// ============================================================================

use thiserror::Error;

/// All possible errors that can occur during HRest encoding/decoding.
///
/// HTTP status semantics (for middleware implementations):
/// - `UnknownField` / `UnknownToken` → **HTTP 422 Unprocessable Entity**
/// - `InvalidContractHash`           → **HTTP 400 Bad Request**
/// - `BufferOverflow` / `MalformedPayload` → **HTTP 400 Bad Request**
#[derive(Debug, Error)]
pub enum HrestError {
    /// The requested route is not defined in the contract.
    #[error("Unknown route '{0}' — not registered in contract")]
    UnknownRoute(String),

    /// A field name is not in the contract whitelist for this route.
    /// Middleware must respond with HTTP 422.
    #[error("Unknown field '{0}' — not in contract whitelist (HTTP 422)")]
    UnknownField(String),

    /// A token ID in the binary stream is not registered in the contract.
    /// Middleware must respond with HTTP 422.
    #[error("Unknown token ID 0x{0:02X} — not registered in contract (HTTP 422)")]
    UnknownToken(u8),

    /// The client's contract hash does not match the server's active contract.
    /// Middleware must respond with HTTP 400.
    #[error("Contract hash mismatch: expected '{expected}', got '{got}' (HTTP 400)")]
    InvalidContractHash { expected: String, got: String },

    /// A data type byte is not recognized.
    #[error("Invalid data type byte 0x{0:02X} — unknown type")]
    InvalidDataType(u8),

    /// Reading would exceed the buffer boundary. Memory safety guard.
    #[error("Buffer overflow: needed {needed} bytes but only {available} available")]
    BufferOverflow { needed: usize, available: usize },

    /// Varint encoding/decoding failure.
    #[error("Varint error: {0}")]
    VarintError(String),

    /// Generic malformed payload.
    #[error("Malformed payload: {0}")]
    MalformedPayload(String),

    /// I/O error (file reading, etc.).
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON parsing error (contract loading, etc.).
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}
