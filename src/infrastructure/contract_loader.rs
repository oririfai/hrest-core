// ============================================================================
// INFRASTRUCTURE LAYER — contract_loader.rs
//
// Loads and parses hrest-contract.json into domain structs.
// This is the ONLY file in the crate that touches serde_json for contract
// parsing and sha2 for hash computation.
//
// SOLID-D: Implements ContractProvider so application layer stays decoupled.
// SOLID-L: Any ContractProvider implementor is substitutable here.
// Clean Architecture: All I/O and external library use lives in this layer.
// ============================================================================

use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::application::ports::ContractProvider;
use crate::domain::contract::{ContractData, FieldMap};
use crate::domain::error::HrestError;

// ---------------------------------------------------------------------------
// Raw deserialization structs — match hrest-contract.json exactly
// ---------------------------------------------------------------------------

/// Raw form of `hrest-contract.json`, used only for deserialization.
#[derive(Debug, Deserialize, serde::Serialize)]
struct RawContract {
    version: String,
    contract_hash: String,
    routes: HashMap<String, RawRoute>,
}

/// Raw route entry with its field→token mapping.
#[derive(Debug, Deserialize, serde::Serialize)]
struct RawRoute {
    /// Maps field name (string) → token ID (u8).
    fields: HashMap<String, u8>,
}

// ---------------------------------------------------------------------------
// JsonContractLoader — primary infrastructure adapter
// ---------------------------------------------------------------------------

/// Loads an HRest contract from a JSON source and provides it to the
/// application layer via the [`ContractProvider`] port.
///
/// # Usage
/// ```rust,no_run
/// use hrest_core::JsonContractLoader;
///
/// let json = std::fs::read_to_string("hrest-contract.json").unwrap();
/// let loader = JsonContractLoader::from_str(&json).unwrap();
/// ```
pub struct JsonContractLoader {
    data: ContractData,
}

impl JsonContractLoader {
    /// Load a contract from a JSON string.
    ///
    /// # Errors
    /// Returns `HrestError::Json` if the JSON is malformed or missing fields.
    pub fn from_str(json: &str) -> Result<Self, HrestError> {
        let raw: RawContract = serde_json::from_str(json)?;
        Ok(Self {
            data: build_contract_data(raw)?,
        })
    }

    /// Load a contract from a JSON file at the given path.
    ///
    /// # Errors
    /// Returns `HrestError::Io` on file read failure, or `HrestError::Json`
    /// if the file contents are not valid contract JSON.
    pub fn from_file(path: &Path) -> Result<Self, HrestError> {
        let content = std::fs::read_to_string(path)?;
        Self::from_str(&content)
    }

    /// Verify that a client-provided hash matches this contract's hash.
    ///
    /// Used to validate the `X-Hrest-Hash` HTTP header.
    /// Returns `true` only when hashes match exactly.
    pub fn verify_hash(&self, client_hash: &str) -> bool {
        self.data.verify_hash(client_hash)
    }

    /// Compute the SHA-256 hash of the contract's route structure.
    ///
    /// Uses **sorted (BTreeMap) serialization** to guarantee deterministic output
    /// regardless of HashMap iteration order (which is randomized in Rust).
    /// This matches the `contract_hash` field that the CLI tool should emit.
    ///
    /// # Errors
    /// Returns `HrestError::Json` if the input is not valid contract JSON.
    pub fn compute_hash(json: &str) -> Result<String, HrestError> {
        let raw: RawContract = serde_json::from_str(json)?;

        // Build deterministic BTreeMap<route, BTreeMap<field, id>>
        // BTreeMap guarantees sorted-key iteration, producing a stable canonical form.
        let canonical_routes: std::collections::BTreeMap<&str, std::collections::BTreeMap<&str, u8>> =
            raw.routes
                .iter()
                .map(|(route, route_data)| {
                    let sorted_fields: std::collections::BTreeMap<&str, u8> = route_data
                        .fields
                        .iter()
                        .map(|(k, &v)| (k.as_str(), v))
                        .collect();
                    (route.as_str(), sorted_fields)
                })
                .collect();

        let canonical = serde_json::to_string(&canonical_routes)?;
        let digest = Sha256::digest(canonical.as_bytes());
        Ok(hex::encode(digest))
    }

    /// Returns the protocol version string from the loaded contract.
    pub fn version(&self) -> &str {
        &self.data.version
    }

    /// Returns the stored contract hash string.
    pub fn contract_hash(&self) -> &str {
        &self.data.contract_hash
    }
}

impl ContractProvider for JsonContractLoader {
    fn contract(&self) -> &ContractData {
        &self.data
    }
}

// ---------------------------------------------------------------------------
// Private builder — converts raw deserialized form → domain structs
// ---------------------------------------------------------------------------

/// Validate that no two field names map to the same token ID.
/// Duplicate IDs would corrupt bidirectional lookup in FieldMap.
fn validate_no_duplicate_ids(route: &str, fields: &std::collections::HashMap<String, u8>) -> Result<(), HrestError> {
    let mut seen_ids = std::collections::HashSet::new();
    for (&id, _name) in fields.iter().map(|(k, v)| (v, k)) {
        if !seen_ids.insert(id) {
            return Err(HrestError::MalformedPayload(format!(
                "Contract error in route '{}': duplicate field ID 0x{:02X} detected — \
                 two fields cannot share the same token ID",
                route, id
            )));
        }
    }
    Ok(())
}

fn build_contract_data(raw: RawContract) -> Result<ContractData, HrestError> {
    let mut routes = HashMap::new();

    for (route_key, raw_route) in raw.routes {
        // Validate uniqueness before constructing FieldMap
        validate_no_duplicate_ids(&route_key, &raw_route.fields)?;
        let field_map = FieldMap::new(raw_route.fields);
        routes.insert(route_key, field_map);
    }

    Ok(ContractData::new(raw.version, raw.contract_hash, routes))
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_CONTRACT: &str = r#"
    {
        "version": "1.0.0",
        "contract_hash": "abc123",
        "routes": {
            "POST /api/v1/test/run": {
                "fields": {
                    "event":      1,
                    "task_id":    2,
                    "config":     3,
                    "target_url": 4,
                    "headless":   5,
                    "timeout":    6,
                    "flow":       7,
                    "step":       8,
                    "type":       9,
                    "selector":  10,
                    "value":     11
                }
            }
        }
    }"#;

    #[test]
    fn load_from_str_succeeds() {
        let loader = JsonContractLoader::from_str(SAMPLE_CONTRACT)
            .expect("Should load valid contract");
        assert_eq!(loader.version(), "1.0.0");
        assert_eq!(loader.contract_hash(), "abc123");
    }

    #[test]
    fn field_map_lookup_works() {
        let loader = JsonContractLoader::from_str(SAMPLE_CONTRACT).unwrap();
        let map = loader.contract()
            .field_map("POST /api/v1/test/run")
            .expect("Route should exist");

        assert_eq!(map.field_id("event"),   Some(1));
        assert_eq!(map.field_id("task_id"), Some(2));
        assert_eq!(map.field_name(1),  Some("event"));
        assert_eq!(map.field_name(99), None);
    }

    #[test]
    fn verify_hash_matches() {
        let loader = JsonContractLoader::from_str(SAMPLE_CONTRACT).unwrap();
        assert!(loader.verify_hash("abc123"));
        assert!(!loader.verify_hash("wrong_hash"));
    }

    #[test]
    fn invalid_json_returns_error() {
        let result = JsonContractLoader::from_str("{ not valid json }");
        assert!(matches!(result, Err(HrestError::Json(_))));
    }

    #[test]
    fn compute_hash_returns_hex_string() {
        let hash = JsonContractLoader::compute_hash(SAMPLE_CONTRACT)
            .expect("Should compute hash");
        // SHA-256 hex is always 64 chars
        assert_eq!(hash.len(), 64);
        // Should be lowercase hex
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
