// ============================================================================
// APPLICATION LAYER — ports.rs
//
// Trait abstractions (Ports) that decouple the application use-cases
// from concrete infrastructure implementations.
//
// SOLID-D: Application depends on these abstractions, not on concrete types.
// SOLID-I: Separate Encode and Decode traits; implementors choose what to support.
// ============================================================================

use crate::domain::{contract::ContractData, error::HrestError};

// ---------------------------------------------------------------------------
// ContractProvider — dependency-inverted access to contract data
// ---------------------------------------------------------------------------

/// A source of HRest contract data.
///
/// Implement this trait to provide contract data from any source
/// (JSON file, in-memory, database, etc.) without changing the
/// encoding/decoding logic.
///
/// # Example
/// ```rust,no_run
/// use hrest_core::ContractProvider;
/// use hrest_core::types::ContractData;
///
/// struct HardcodedContract(ContractData);
///
/// impl ContractProvider for HardcodedContract {
///     fn contract(&self) -> &ContractData { &self.0 }
/// }
/// ```
pub trait ContractProvider {
    /// Returns a reference to the underlying contract data.
    fn contract(&self) -> &ContractData;
}

// ---------------------------------------------------------------------------
// Encode / Decode ports — Interface Segregation (SOLID-I)
// ---------------------------------------------------------------------------

/// Encoding capability: converts a JSON payload to a binary TLV stream.
#[allow(dead_code)]
pub trait Encode {
    /// Encode the given JSON value for the specified route.
    ///
    /// # Errors
    /// - `HrestError::UnknownRoute` — route not in contract
    /// - `HrestError::UnknownField` — field not in whitelist (→ HTTP 422)
    fn encode(&self, route: &str, payload: &serde_json::Value) -> Result<Vec<u8>, HrestError>;
}

/// Decoding capability: converts a binary TLV stream to a JSON value.
#[allow(dead_code)]
pub trait Decode {
    /// Decode the given binary stream for the specified route.
    ///
    /// # Errors
    /// - `HrestError::UnknownRoute`  — route not in contract
    /// - `HrestError::UnknownToken`  — token ID not in contract (→ HTTP 422)
    /// - `HrestError::BufferOverflow` — malformed/truncated bytes
    fn decode(&self, route: &str, bytes: &[u8]) -> Result<serde_json::Value, HrestError>;
}
