use std::fs;

fn main() {
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
    let root: Result<serde_json::Value, _> = serde_json::from_str(contract);
    println!("{:?}", root);
    
    // Oh wait! HrestError comes from parsing RawContract!
    // Ah, wait! `JsonContractLoader::from_str` doesn't just parse into `Value`, it does:
    // let root: Value = serde_json::from_str(json)?;
    // Wait, let's see what `from_str` actually does.
}
