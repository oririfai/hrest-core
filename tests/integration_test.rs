// ============================================================================
// Integration Tests — end-to-end round-trip for hrest-core
//
// Covers all 7 type scenarios, security guards, and contract validation.
// ============================================================================

use hrest_core::{decode, encode, JsonContractLoader};

// ---------------------------------------------------------------------------
// Shared test fixtures
// ---------------------------------------------------------------------------

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
                "value":     11,
                "score":     12,
                "data":      13,
                "active":    14,
                "count":     15
            }
        }
    }
}
"#;

const ROUTE: &str = "POST /api/v1/test/run";

fn make_loader() -> JsonContractLoader {
    JsonContractLoader::from_str(SAMPLE_CONTRACT).expect("Contract should load")
}

fn round_trip(json: &str) -> serde_json::Value {
    let loader = make_loader();
    let binary = encode(ROUTE, json, &loader).expect("encode should succeed");
    let restored = decode(ROUTE, &binary, &loader).expect("decode should succeed");
    serde_json::from_str(&restored).expect("restored should be valid JSON")
}

// ---------------------------------------------------------------------------
// Type: String (0x01)
// ---------------------------------------------------------------------------

#[test]
fn round_trip_string_field() {
    let original: serde_json::Value = serde_json::from_str(
        r#"{"event": "start", "target_url": "https://example.com"}"#
    ).unwrap();

    let result = round_trip(&original.to_string());
    assert_eq!(result["event"], "start");
    assert_eq!(result["target_url"], "https://example.com");
}

#[test]
fn round_trip_empty_string() {
    let result = round_trip(r#"{"event": ""}"#);
    assert_eq!(result["event"], "");
}

#[test]
fn round_trip_unicode_string() {
    let result = round_trip(r#"{"event": "こんにちは 🦀"}"#);
    assert_eq!(result["event"], "こんにちは 🦀");
}

// ---------------------------------------------------------------------------
// Type: Integer (0x02)
// ---------------------------------------------------------------------------

#[test]
fn round_trip_positive_integer() {
    let result = round_trip(r#"{"timeout": 30000}"#);
    assert_eq!(result["timeout"], 30000);
}

#[test]
fn round_trip_negative_integer() {
    let result = round_trip(r#"{"timeout": -1}"#);
    assert_eq!(result["timeout"], -1);
}

#[test]
fn round_trip_zero_integer() {
    let result = round_trip(r#"{"timeout": 0}"#);
    assert_eq!(result["timeout"], 0);
}

#[test]
fn round_trip_large_integer() {
    let result = round_trip(r#"{"timeout": 9223372036854775807}"#); // i64::MAX
    assert_eq!(result["timeout"], 9223372036854775807i64);
}

// ---------------------------------------------------------------------------
// Type: Boolean (0x03)
// ---------------------------------------------------------------------------

#[test]
fn round_trip_bool_true() {
    let result = round_trip(r#"{"headless": true}"#);
    assert_eq!(result["headless"], true);
}

#[test]
fn round_trip_bool_false() {
    let result = round_trip(r#"{"headless": false}"#);
    assert_eq!(result["headless"], false);
}

// ---------------------------------------------------------------------------
// Type: Float (0x06)
// ---------------------------------------------------------------------------

#[test]
fn round_trip_float() {
    // Wire format v2 uses f32 encoding (4 bytes). f32 has ~7 significant digits.
    // 98.6_f64 as f32 = 98.5999984741211, so tolerance must be >= ~2e-7.
    let result = round_trip(r#"{"score": 98.6}"#);
    let f = result["score"].as_f64().expect("Should be a float");
    let expected = 98.6_f64 as f32 as f64; // what f32 encoding actually produces
    assert!((f - expected).abs() < 1e-9, "Float value should round-trip through f32: {}", f);
}

#[test]
fn round_trip_float_negative() {
    // Wire format v2 uses f32; compare against f32 round-trip value.
    let result = round_trip(r#"{"score": -273.15}"#);
    let f = result["score"].as_f64().unwrap();
    let expected = (-273.15_f64) as f32 as f64;
    assert!((f - expected).abs() < 1e-9);
}

#[test]
fn round_trip_float_zero() {
    let result = round_trip(r#"{"score": 0.0}"#);
    assert_eq!(result["score"].as_f64().unwrap(), 0.0);
}

// ---------------------------------------------------------------------------
// Type: Null (0x00) — must be written EXPLICITLY to stream
// ---------------------------------------------------------------------------

#[test]
fn round_trip_null_field() {
    let result = round_trip(r#"{"event": null}"#);
    assert!(result["event"].is_null(), "null should round-trip explicitly");
}

#[test]
fn null_and_non_null_together() {
    let result = round_trip(r#"{"event": null, "timeout": 5000}"#);
    assert!(result["event"].is_null());
    assert_eq!(result["timeout"], 5000);
}

// ---------------------------------------------------------------------------
// Type: Nested Object (0x05, kind=0x00)
// ---------------------------------------------------------------------------

#[test]
fn round_trip_nested_object() {
    let json = r#"{"config": {"step": 1, "type": "click"}}"#;
    let result = round_trip(json);
    assert_eq!(result["config"]["step"], 1);
    assert_eq!(result["config"]["type"], "click");
}

// ---------------------------------------------------------------------------
// Type: Nested Array (0x05, kind=0x01)
// ---------------------------------------------------------------------------

#[test]
fn round_trip_array_of_primitives() {
    let result = round_trip(r#"{"flow": [1, 2, 3]}"#);
    let arr = result["flow"].as_array().expect("Should be array");
    assert_eq!(arr.len(), 3);
    assert_eq!(arr[0], 1);
    assert_eq!(arr[1], 2);
    assert_eq!(arr[2], 3);
}

#[test]
fn round_trip_array_of_objects() {
    let json = r##"{
        "flow": [
            {"step": 1, "type": "click",  "selector": "#btn",   "value": ""},
            {"step": 2, "type": "input",  "selector": "#input", "value": "hello"}
        ]
    }"##;
    let result = round_trip(json);
    let arr = result["flow"].as_array().unwrap();
    assert_eq!(arr.len(), 2);
    assert_eq!(arr[0]["step"], 1);
    assert_eq!(arr[0]["type"], "click");
    assert_eq!(arr[1]["value"], "hello");
}

#[test]
fn round_trip_empty_array() {
    let result = round_trip(r#"{"flow": []}"#);
    let arr = result["flow"].as_array().unwrap();
    assert!(arr.is_empty());
}

// ---------------------------------------------------------------------------
// Full payload round-trip (all types combined)
// ---------------------------------------------------------------------------

#[test]
fn round_trip_full_payload() {
    let json = r##"{
        "event":      "test_run",
        "task_id":    42,
        "headless":   true,
        "timeout":    30000,
        "score":      99.5,
        "target_url": "https://example.com",
        "config":     null,
        "flow": [
            {"step": 1, "type": "click", "selector": "#go", "value": ""},
            {"step": 2, "type": "wait",  "selector": "",    "value": "1000"}
        ]
    }"##;

    let loader = make_loader();
    let binary = encode(ROUTE, json, &loader).unwrap();
    let restored_str = decode(ROUTE, &binary, &loader).unwrap();
    let restored: serde_json::Value = serde_json::from_str(&restored_str).unwrap();

    assert_eq!(restored["event"], "test_run");
    assert_eq!(restored["task_id"], 42);
    assert_eq!(restored["headless"], true);
    assert_eq!(restored["timeout"], 30000);
    assert!((restored["score"].as_f64().unwrap() - 99.5).abs() < 1e-9);
    assert_eq!(restored["target_url"], "https://example.com");
    assert!(restored["config"].is_null());
    assert_eq!(restored["flow"].as_array().unwrap().len(), 2);
}

// ---------------------------------------------------------------------------
// Compression: binary payload should be smaller than JSON
// ---------------------------------------------------------------------------

#[test]
fn binary_is_smaller_than_json() {
    let json = r#"{
        "event": "test_run",
        "task_id": 12345,
        "target_url": "https://example.com/api/test",
        "headless": true,
        "timeout": 30000
    }"#;

    let loader = make_loader();
    let binary = encode(ROUTE, json, &loader).unwrap();
    let json_bytes = json.trim().as_bytes().len();

    println!("JSON size:   {} bytes", json_bytes);
    println!("Binary size: {} bytes", binary.len());
    println!("Compression: {:.1}%", (1.0 - binary.len() as f64 / json_bytes as f64) * 100.0);

    assert!(
        binary.len() < json_bytes,
        "Binary ({} bytes) should be smaller than JSON ({} bytes)",
        binary.len(), json_bytes
    );
}

// ---------------------------------------------------------------------------
// Security: Unknown field rejection (→ HTTP 422)
// ---------------------------------------------------------------------------

#[test]
fn unknown_field_is_rejected() {
    let loader = make_loader();
    let json = r#"{"injection_field": "evil_payload"}"#;
    let result = encode(ROUTE, json, &loader);

    assert!(
        matches!(result, Err(hrest_core::HrestError::UnknownField(_))),
        "Unknown field must be rejected with UnknownField error (HTTP 422)"
    );
}

// ---------------------------------------------------------------------------
// Security: Unknown token ID rejection (→ HTTP 422)
// ---------------------------------------------------------------------------

#[test]
fn unknown_token_in_binary_is_rejected() {
    let loader = make_loader();
    // Craft a malicious binary: field_id=0xFF (not in contract), type=string
    let malicious = &[0xFF_u8, 0x01, 0x03, 0x00, b'a', b'b', b'c'];
    let result = decode(ROUTE, malicious, &loader);

    assert!(
        matches!(result, Err(hrest_core::HrestError::UnknownToken(0xFF))),
        "Unknown token 0xFF must be rejected (HTTP 422)"
    );
}

// ---------------------------------------------------------------------------
// Security: Buffer overflow protection
// ---------------------------------------------------------------------------

#[test]
fn truncated_buffer_returns_overflow_error() {
    let loader = make_loader();
    // Valid field_id=1 (event), type=string, then claims 100 bytes but provides none
    let truncated = &[0x01_u8, 0x01, 0x64, 0x00]; // len=100 declared but 0 bytes follow
    let result = decode(ROUTE, truncated, &loader);

    assert!(
        matches!(result, Err(hrest_core::HrestError::BufferOverflow { .. })),
        "Truncated buffer must produce BufferOverflow error"
    );
}

#[test]
fn completely_empty_buffer_decodes_to_empty_object() {
    let loader = make_loader();
    let result = decode(ROUTE, &[], &loader).unwrap();
    let obj: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert!(obj.as_object().unwrap().is_empty());
}

// ---------------------------------------------------------------------------
// Contract: Unknown route rejection
// ---------------------------------------------------------------------------

#[test]
fn unknown_route_is_rejected_on_encode() {
    let loader = make_loader();
    let result = encode("GET /does/not/exist", r#"{"event": "x"}"#, &loader);
    assert!(matches!(result, Err(hrest_core::HrestError::UnknownRoute(_))));
}

#[test]
fn unknown_route_is_rejected_on_decode() {
    let loader = make_loader();
    let result = decode("GET /does/not/exist", &[0x01, 0x01, 0x00, 0x00], &loader);
    assert!(matches!(result, Err(hrest_core::HrestError::UnknownRoute(_))));
}

// ---------------------------------------------------------------------------
// Contract: Hash verification
// ---------------------------------------------------------------------------

#[test]
fn contract_hash_verification() {
    let loader = make_loader();
    assert!(loader.verify_hash("abc123"), "Correct hash should verify");
    assert!(!loader.verify_hash("wrong"), "Wrong hash should fail");
    assert!(!loader.verify_hash(""), "Empty hash should fail");
}

#[test]
fn compute_hash_produces_valid_hex() {
    let hash = JsonContractLoader::compute_hash(SAMPLE_CONTRACT).unwrap();
    assert_eq!(hash.len(), 64, "SHA-256 hex should be 64 chars");
    assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
}

// ---------------------------------------------------------------------------
// Encoder: invalid payload type
// ---------------------------------------------------------------------------

#[test]
fn non_object_payload_is_rejected() {
    let loader = make_loader();
    // Array at top level is not allowed
    let result = encode(ROUTE, r#"[1, 2, 3]"#, &loader);
    assert!(matches!(result, Err(hrest_core::HrestError::MalformedPayload(_))));
}

// ===========================================================================
// SECURITY TESTS — attack surface coverage
// ===========================================================================

// ---------------------------------------------------------------------------
// [SEC-1] Stack overflow prevention — maximum nesting depth enforcement
// ---------------------------------------------------------------------------

#[test]
fn deeply_nested_binary_exceeding_limit_is_rejected() {
    // Craft a binary with 34 levels of nesting (MAX = 32).
    // Wire format v2: [field_id, TYPE_NESTED=0x05, OBJECT=0x00, count:u8]
    let loader = make_loader();
    let mut binary: Vec<u8> = Vec::new();

    // 34 nested object wrappers using field "config" (id=3)
    for _ in 0..34 {
        binary.push(3);    // field_id = config
        binary.push(0x05); // TYPE_NESTED
        binary.push(0x00); // NESTED_KIND_OBJECT
        binary.push(1);    // compact count = 1 (v2: single u8)
    }
    // Innermost leaf: config = null
    binary.push(3);        // field_id = config
    binary.push(0x00);     // TYPE_NULL

    let result = decode(ROUTE, &binary, &loader);
    assert!(
        matches!(result, Err(hrest_core::HrestError::MalformedPayload(_))),
        "Binary exceeding max nesting depth (34 > 32) must be rejected. Got: {:?}", result
    );
}

#[test]
fn nesting_at_exact_max_depth_is_accepted() {
    // 32 levels of nesting should succeed (MAX = 32).
    // Wire format v2: compact count uses 1 byte for count < 255.
    let loader = make_loader();
    let mut binary: Vec<u8> = Vec::new();

    for _ in 0..32 {
        binary.push(3);    // config
        binary.push(0x05); // TYPE_NESTED
        binary.push(0x00); // OBJECT
        binary.push(1);    // compact count = 1 (v2: single u8)
    }
    // Leaf: config = null
    binary.push(3);
    binary.push(0x00); // TYPE_NULL

    let result = decode(ROUTE, &binary, &loader);
    assert!(
        result.is_ok(),
        "Binary at exactly max depth (32) must be accepted. Got: {:?}", result
    );
}

// ---------------------------------------------------------------------------
// [SEC-2] NaN and Infinity float rejection
// ---------------------------------------------------------------------------

#[test]
fn nan_float_in_binary_stream_is_rejected() {
    let loader = make_loader();
    // field_id=12 (score), TYPE_FLOAT=0x06, then NaN bytes
    let mut binary = vec![12u8, 0x06];
    binary.extend_from_slice(&f64::NAN.to_le_bytes());

    let result = decode(ROUTE, &binary, &loader);
    assert!(
        matches!(result, Err(hrest_core::HrestError::MalformedPayload(_))),
        "NaN float must be rejected — not representable in JSON. Got: {:?}", result
    );
}

#[test]
fn positive_infinity_in_binary_stream_is_rejected() {
    let loader = make_loader();
    let mut binary = vec![12u8, 0x06];
    binary.extend_from_slice(&f64::INFINITY.to_le_bytes());

    let result = decode(ROUTE, &binary, &loader);
    assert!(
        matches!(result, Err(hrest_core::HrestError::MalformedPayload(_))),
        "Infinity float must be rejected. Got: {:?}", result
    );
}

#[test]
fn negative_infinity_in_binary_stream_is_rejected() {
    let loader = make_loader();
    let mut binary = vec![12u8, 0x06];
    binary.extend_from_slice(&f64::NEG_INFINITY.to_le_bytes());

    let result = decode(ROUTE, &binary, &loader);
    assert!(
        matches!(result, Err(hrest_core::HrestError::MalformedPayload(_))),
        "Negative infinity must be rejected. Got: {:?}", result
    );
}

// ---------------------------------------------------------------------------
// [SEC-3] Memory exhaustion prevention — crafted large count fields
// ---------------------------------------------------------------------------

#[test]
fn large_array_count_with_empty_body_causes_overflow_not_panic() {
    let loader = make_loader();
    // field_id=7 (flow), TYPE_NESTED=0x05, ARRAY=0x01, count=65535 (max u16)
    // but no actual elements follow → BufferOverflow, not panic or OOM
    let binary = &[7u8, 0x05, 0x01, 0xFF, 0xFF];
    let result = decode(ROUTE, binary, &loader);
    assert!(
        matches!(result, Err(hrest_core::HrestError::BufferOverflow { .. })),
        "Claiming 65535 elements with no data must BufferOverflow. Got: {:?}", result
    );
}

#[test]
fn large_object_count_with_empty_body_causes_overflow_not_panic() {
    let loader = make_loader();
    // field_id=3 (config), TYPE_NESTED=0x05, OBJECT=0x00, count=65535
    let binary = &[3u8, 0x05, 0x00, 0xFF, 0xFF];
    let result = decode(ROUTE, binary, &loader);
    assert!(
        matches!(result, Err(hrest_core::HrestError::BufferOverflow { .. })),
        "Claiming 65535 fields with no data must BufferOverflow. Got: {:?}", result
    );
}

// ---------------------------------------------------------------------------
// [SEC-4] Multi-byte injection attempts
// ---------------------------------------------------------------------------

#[test]
fn completely_random_bytes_do_not_panic() {
    let loader = make_loader();
    // Fuzz-like: random-looking bytes should error gracefully, never panic
    let garbage: &[u8] = &[0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0xFF, 0x42, 0x13, 0x37];
    let result = decode(ROUTE, garbage, &loader);
    // We don't care about the specific error — just that it does NOT panic
    assert!(result.is_err(), "Garbage bytes must produce an error, not succeed");
}

#[test]
fn unknown_token_mid_stream_is_rejected() {
    let loader = make_loader();
    // Valid first field: event (id=1) = string "ok"
    // Wire format v2: string length is compact u8 (single byte for len < 255)
    // Then illegal field_id 0xFE mid-stream
    let binary = &[
        0x01u8,             // field_id = event
        0x01,               // TYPE_STRING
        0x02,               // compact len = 2 (v2: single u8, no high byte)
        b'o', b'k',         // "ok"
        0xFE,               // ILLEGAL field_id (not in contract)
        0x01,               // TYPE_STRING (doesn't matter)
    ];
    let result = decode(ROUTE, binary, &loader);
    assert!(
        matches!(result, Err(hrest_core::HrestError::UnknownToken(0xFE))),
        "Unknown token mid-stream must return UnknownToken(0xFE). Got: {:?}", result
    );
}

// ===========================================================================
// RELIABILITY TESTS — edge cases and boundary conditions
// ===========================================================================

// ---------------------------------------------------------------------------
// [REL-1] String edge cases
// ---------------------------------------------------------------------------

#[test]
fn string_with_embedded_null_bytes_round_trips() {
    // Rust strings can contain null bytes; this tests UTF-8 correctness
    let payload = r#"{"event": "hello\u0000world"}"#;
    let loader = make_loader();
    let binary = encode(ROUTE, payload, &loader).expect("encode should succeed");
    let restored = decode(ROUTE, &binary, &loader).expect("decode should succeed");
    let json: serde_json::Value = serde_json::from_str(&restored).unwrap();
    assert_eq!(json["event"].as_str().unwrap(), "hello\0world");
}

#[test]
fn string_with_special_chars_round_trips() {
    let payload = r#"{"event": "tab:\there\nnewline\"quote\\back"}"#;
    let loader = make_loader();
    let binary = encode(ROUTE, payload, &loader).unwrap();
    let restored_str = decode(ROUTE, &binary, &loader).unwrap();
    let original: serde_json::Value = serde_json::from_str(payload).unwrap();
    let restored: serde_json::Value = serde_json::from_str(&restored_str).unwrap();
    assert_eq!(original["event"], restored["event"]);
}

#[test]
fn multibyte_unicode_round_trips() {
    let payload = r#"{"event": "日本語テスト 🦀🔥💡"}"#;
    let result = round_trip(payload);
    assert_eq!(result["event"], "日本語テスト 🦀🔥💡");
}

// ---------------------------------------------------------------------------
// [REL-2] Integer boundary conditions
// ---------------------------------------------------------------------------

#[test]
fn i64_min_round_trips() {
    let loader = make_loader();
    let json = format!(r#"{{"timeout": {}}}"#, i64::MIN);
    let binary = encode(ROUTE, &json, &loader).unwrap();
    let restored: serde_json::Value = serde_json::from_str(&decode(ROUTE, &binary, &loader).unwrap()).unwrap();
    assert_eq!(restored["timeout"], i64::MIN);
}

#[test]
fn i64_max_round_trips() {
    let loader = make_loader();
    let json = format!(r#"{{"timeout": {}}}"#, i64::MAX);
    let binary = encode(ROUTE, &json, &loader).unwrap();
    let restored: serde_json::Value = serde_json::from_str(&decode(ROUTE, &binary, &loader).unwrap()).unwrap();
    assert_eq!(restored["timeout"], i64::MAX);
}

// ---------------------------------------------------------------------------
// [REL-3] Float precision
// ---------------------------------------------------------------------------

#[test]
fn float_precision_is_preserved() {
    // Wire format v2 uses f32 encoding (~7 significant digits).
    // PI as f32 = 3.1415927410125732 (vs f64 PI = 3.141592653589793).
    // The round-trip value must match the f32 representation exactly.
    let result = round_trip(r#"{"score": 3.141592653589793}"#);
    let f = result["score"].as_f64().unwrap();
    let expected = std::f64::consts::PI as f32 as f64; // exact f32 representation
    assert!((f - expected).abs() < 1e-9,
        "Float must round-trip through f32 precision: got {}, expected {}", f, expected);
}

#[test]
fn float_subnormal_round_trips() {
    // Wire format v2 uses f32 encoding.
    // Test that f32 subnormal values round-trip with exact bit equality.
    // Note: f64 subnormals (e.g. 5e-324) underflow to 0.0 in f32; that is expected.
    let loader = make_loader();
    let subnormal_f32 = 1.4e-45_f32; // smallest positive f32 subnormal
    let subnormal_f64 = subnormal_f32 as f64;
    let payload = format!(r#"{{"score": {}}}"#, subnormal_f64);
    let binary = encode(ROUTE, &payload, &loader).unwrap();
    let restored_str = decode(ROUTE, &binary, &loader).unwrap();
    let restored: serde_json::Value = serde_json::from_str(&restored_str).unwrap();
    let f = restored["score"].as_f64().unwrap();
    // Round-trip through f32: decoded value must match f32 encoding
    let expected = (subnormal_f64 as f32) as f64;
    assert!((f - expected).abs() < 1e-50,
        "f32 subnormal must round-trip with f32 precision: got {}, expected {}", f, expected);
}

// ---------------------------------------------------------------------------
// [REL-4] Multiple fields, ordering independence
// ---------------------------------------------------------------------------

#[test]
fn all_field_types_in_one_payload() {
    let json = r#"{
        "event":      "check",
        "task_id":    -99,
        "headless":   false,
        "timeout":    0,
        "score":      -1.5,
        "target_url": "",
        "config":     null,
        "count":      255
    }"#;
    let result = round_trip(json);
    assert_eq!(result["event"],       "check");
    assert_eq!(result["task_id"],     -99);
    assert_eq!(result["headless"],    false);
    assert_eq!(result["timeout"],     0);
    assert_eq!(result["score"].as_f64().unwrap(), -1.5);
    assert_eq!(result["target_url"],  "");
    assert!(result["config"].is_null());
    assert_eq!(result["count"],       255);
}

// ===========================================================================
// DURABILITY TESTS — stability and contract integrity
// ===========================================================================

// ---------------------------------------------------------------------------
// [DUR-1] Hash determinism — same input always produces same hash
// ---------------------------------------------------------------------------

#[test]
fn compute_hash_is_deterministic_across_calls() {
    // Run 10 times — hash must never change between runs
    let mut hashes = Vec::new();
    for _ in 0..10 {
        let h = JsonContractLoader::compute_hash(SAMPLE_CONTRACT).unwrap();
        hashes.push(h);
    }
    let first = &hashes[0];
    for (i, h) in hashes.iter().enumerate() {
        assert_eq!(first, h,
            "Hash at iteration {} differs from first: {} vs {}", i, first, h);
    }
}

#[test]
fn compute_hash_is_content_dependent() {
    // Contracts with different fields must produce different hashes
    let contract_a = r#"{
        "version": "1.0.0", "contract_hash": "x",
        "routes": { "POST /a": { "fields": { "x": 1 } } }
    }"#;
    let contract_b = r#"{
        "version": "1.0.0", "contract_hash": "x",
        "routes": { "POST /a": { "fields": { "y": 1 } } }
    }"#;
    let hash_a = JsonContractLoader::compute_hash(contract_a).unwrap();
    let hash_b = JsonContractLoader::compute_hash(contract_b).unwrap();
    assert_ne!(hash_a, hash_b,
        "Different contracts must produce different hashes");
}

// ---------------------------------------------------------------------------
// [DUR-2] Duplicate field ID detection
// ---------------------------------------------------------------------------

#[test]
fn contract_with_duplicate_field_ids_is_rejected() {
    // field_a and field_b both map to token ID 1 — invalid
    let bad_contract = r#"{
        "version": "1.0.0",
        "contract_hash": "test",
        "routes": {
            "POST /api/test": {
                "fields": {
                    "field_a": 1,
                    "field_b": 1
                }
            }
        }
    }"#;
    let result = JsonContractLoader::from_str(bad_contract);
    assert!(
        matches!(result, Err(hrest_core::HrestError::MalformedPayload(_))),
        "Contract with duplicate field IDs (field_a and field_b both = 1) must be rejected"
    );
}

#[test]
fn contract_with_unique_field_ids_is_accepted() {
    let good_contract = r#"{
        "version": "1.0.0",
        "contract_hash": "test",
        "routes": {
            "POST /api/test": {
                "fields": {
                    "field_a": 1,
                    "field_b": 2,
                    "field_c": 3
                }
            }
        }
    }"#;
    let result = JsonContractLoader::from_str(good_contract);
    assert!(result.is_ok(), "Contract with unique IDs must load successfully");
}

// ---------------------------------------------------------------------------
// [DUR-3] Wire format stability — binary output is reproducible
// ---------------------------------------------------------------------------

#[test]
fn encoding_same_payload_produces_identical_binary() {
    let loader = make_loader();
    let payload = r#"{"event": "start", "task_id": 42, "headless": true}"#;

    let binary1 = encode(ROUTE, payload, &loader).unwrap();
    let binary2 = encode(ROUTE, payload, &loader).unwrap();

    assert_eq!(binary1, binary2,
        "Same payload must always encode to identical binary");
}

#[test]
fn different_payloads_produce_different_binaries() {
    let loader = make_loader();
    let a = r#"{"event": "start"}"#;
    let b = r#"{"event": "stop"}"#;

    let bin_a = encode(ROUTE, a, &loader).unwrap();
    let bin_b = encode(ROUTE, b, &loader).unwrap();

    assert_ne!(bin_a, bin_b, "Different payloads must produce different binaries");
}

// ---------------------------------------------------------------------------
// [DUR-4] Contract version and hash fields preserved
// ---------------------------------------------------------------------------

#[test]
fn contract_version_is_accessible() {
    let loader = make_loader();
    assert_eq!(loader.version(), "1.0.0");
}

#[test]
fn contract_hash_field_is_accessible() {
    let loader = make_loader();
    assert_eq!(loader.contract_hash(), "abc123");
}

// ---------------------------------------------------------------------------
// [DUR-5] Empty / minimal payloads
// ---------------------------------------------------------------------------

#[test]
fn empty_json_object_encodes_to_zero_bytes() {
    let loader = make_loader();
    let binary = encode(ROUTE, r#"{}"#, &loader).unwrap();
    assert!(binary.is_empty(), "Empty JSON object must encode to 0 bytes");
}

#[test]
fn zero_bytes_decode_to_empty_json_object() {
    let loader = make_loader();
    let restored = decode(ROUTE, &[], &loader).unwrap();
    let json: serde_json::Value = serde_json::from_str(&restored).unwrap();
    assert!(json.as_object().unwrap().is_empty());
}

#[test]
fn single_null_field_is_two_bytes() {
    // field_id (1 byte) + TYPE_NULL (1 byte) = 2 bytes
    let loader = make_loader();
    let binary = encode(ROUTE, r#"{"event": null}"#, &loader).unwrap();
    assert_eq!(binary.len(), 2,
        "Single null field must encode to exactly 2 bytes, got {}", binary.len());
}

#[test]
fn boolean_fields_are_three_bytes() {
    // field_id (1B) + TYPE_BOOL (1B) + value (1B) = 3 bytes each
    let loader = make_loader();
    let binary = encode(ROUTE, r#"{"headless": true}"#, &loader).unwrap();
    assert_eq!(binary.len(), 3,
        "Single boolean field must encode to exactly 3 bytes, got {}", binary.len());
}

