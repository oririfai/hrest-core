// ============================================================================
// DOMAIN LAYER — contract.rs
//
// Pure domain structs for HRest contract data.
// No serde, no I/O, no external dependencies.
// Loading/parsing lives in infrastructure::contract_loader.
//
// SOLID-S: Only responsible for in-memory contract representation.
// SOLID-D: Application layer depends on these structs, not on loaders.
// ============================================================================

use std::collections::HashMap;
use crate::domain::error::HrestError;

// ---------------------------------------------------------------------------
// FieldMap — bidirectional name ↔ token ID lookup for one route
// ---------------------------------------------------------------------------

/// Bidirectional mapping between field names and their binary token IDs,
/// scoped to a single route.
///
/// Constructed from the `fields` object in `hrest-contract.json`.
#[derive(Debug, Clone)]
pub struct FieldMap {
    /// name → token ID (used during encoding)
    name_to_id: HashMap<String, u8>,
    /// token ID → name (used during decoding)
    id_to_name: HashMap<u8, String>,
}

impl FieldMap {
    /// Build a `FieldMap` from a name→id mapping.
    /// The inverse (id→name) is computed automatically.
    pub fn new(name_to_id: HashMap<String, u8>) -> Self {
        let id_to_name = name_to_id
            .iter()
            .map(|(name, &id)| (id, name.clone()))
            .collect();

        Self { name_to_id, id_to_name }
    }

    /// Look up the token ID for a given field name.
    ///
    /// Returns `None` if the field is not in the contract whitelist.
    pub fn field_id(&self, name: &str) -> Option<u8> {
        self.name_to_id.get(name).copied()
    }

    /// Look up the field name for a given token ID.
    ///
    /// Returns `None` if the token ID is not registered.
    pub fn field_name(&self, id: u8) -> Option<&str> {
        self.id_to_name.get(&id).map(String::as_str)
    }

    /// Returns the number of fields in this route's contract.
    pub fn len(&self) -> usize {
        self.name_to_id.len()
    }

    /// Returns `true` if this route has no fields defined.
    pub fn is_empty(&self) -> bool {
        self.name_to_id.is_empty()
    }
}

// ---------------------------------------------------------------------------
// ContractData — full in-memory representation of hrest-contract.json
// ---------------------------------------------------------------------------

/// The complete HRest contract — the Single Source of Truth for all
/// field token mappings across all API routes.
///
/// Loaded from `hrest-contract.json` by `JsonContractLoader` (infrastructure layer).
#[derive(Debug, Clone)]
pub struct ContractData {
    /// Protocol version string (e.g. `"1.0.0"`).
    pub version: String,
    /// SHA-256 hash of the canonical route structure.
    /// Used for `X-Hrest-Hash` header validation.
    pub contract_hash: String,
    /// Route → FieldMap lookup table.
    routes: HashMap<String, FieldMap>,
}

impl ContractData {
    /// Create a `ContractData` instance.
    pub fn new(version: String, contract_hash: String, routes: HashMap<String, FieldMap>) -> Self {
        Self { version, contract_hash, routes }
    }

    /// Get the `FieldMap` for a given route key.
    ///
    /// # Errors
    /// Returns `HrestError::UnknownRoute` if the route is not in the contract.
    pub fn field_map(&self, route: &str) -> Result<&FieldMap, HrestError> {
        self.routes
            .get(route)
            .ok_or_else(|| HrestError::UnknownRoute(route.to_string()))
    }

    /// Returns `true` if the given hash matches the stored contract hash.
    /// Used to validate the `X-Hrest-Hash` request header.
    pub fn verify_hash(&self, client_hash: &str) -> bool {
        self.contract_hash == client_hash
    }

    /// Returns an iterator over all registered route keys.
    pub fn routes(&self) -> impl Iterator<Item = &str> {
        self.routes.keys().map(String::as_str)
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    fn make_field_map() -> FieldMap {
        let mut m = HashMap::new();
        m.insert("event".into(), 1u8);
        m.insert("task_id".into(), 2u8);
        m.insert("headless".into(), 5u8);
        FieldMap::new(m)
    }

    #[test]
    fn field_id_lookup() {
        let map = make_field_map();
        assert_eq!(map.field_id("event"), Some(1));
        assert_eq!(map.field_id("task_id"), Some(2));
        assert_eq!(map.field_id("nonexistent"), None);
    }

    #[test]
    fn field_name_lookup() {
        let map = make_field_map();
        assert_eq!(map.field_name(1), Some("event"));
        assert_eq!(map.field_name(2), Some("task_id"));
        assert_eq!(map.field_name(99), None);
    }

    #[test]
    fn contract_data_verify_hash() {
        let contract = ContractData::new(
            "1.0.0".into(),
            "abc123".into(),
            HashMap::new(),
        );
        assert!(contract.verify_hash("abc123"));
        assert!(!contract.verify_hash("wrong"));
    }

    #[test]
    fn contract_data_unknown_route() {
        let contract = ContractData::new("1.0.0".into(), "hash".into(), HashMap::new());
        let result = contract.field_map("POST /unknown");
        assert!(matches!(result, Err(HrestError::UnknownRoute(_))));
    }
}
