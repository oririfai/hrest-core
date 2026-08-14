use hrest_core::infrastructure::contract_loader::JsonContractLoader;

#[test]
fn test_hrest_loader_new() {
    let contract = r#"{
  "version": "1.0.0",
  "hash": "test1234",
  "routes": {
    "POST /api/v1/test": {
      "request": {
        "id": "u32",
        "name": "string",
        "isActive": "boolean"
      }
    }
  }
}"#;
    match JsonContractLoader::from_str(contract) {
        Ok(_) => println!("OK"),
        Err(e) => panic!("Error: {:?}", e),
    }
}
