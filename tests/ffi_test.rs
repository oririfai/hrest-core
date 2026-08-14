use std::ffi::CString;
use std::fs;

#[test]
fn test_ffi_decode() {
    // Load contract
    let contract_str = fs::read_to_string("../hrest-benchmark/contracts/req-contract.json").unwrap();
    let c_contract = CString::new(contract_str).unwrap();
    
    // Load binary payload
    let bytes = fs::read("../hrest-benchmark/temp_payload.hrest").unwrap();
    
    // Route
    let c_route = CString::new("POST /api/hrest").unwrap();
    
    unsafe {
        let result = hrest_core::ffi::hrest_decode(
            c_route.as_ptr(),
            bytes.as_ptr(),
            bytes.len(),
            c_contract.as_ptr()
        );
        assert!(!result.is_null(), "hrest_decode returned null");
    }
}
