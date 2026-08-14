# HRest — Hyper-REST Binary Protocol

> **High-Performance, REST-Compatible Binary Protocol — Up to 53% Smaller Payload, 30% Less Bandwidth**

[![Crates.io](https://img.shields.io/crates/v/hrest-core)](https://crates.io/crates/hrest-core)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

HRest is a binary communication protocol that combines the developer-friendliness of REST/JSON with the transmission efficiency of binary protocols like gRPC — without requiring `.proto` file generation or code generation pipelines.

Payloads are transmitted as compact binary (TLV) over standard HTTP. The middleware intercepts transparently: route handlers write and read normal JSON, while clients and servers communicate over the wire in binary. **Bandwidth consumption drops by 28–31% compared to standard REST**, with payload size reduced by 53%.

---

## Why HRest?

|                   | REST (JSON)               | gRPC (Protobuf)                     | HRest                                     |
|-------------------|---------------------------|--------------------------------------|-------------------------------------------|
| Wire format       | JSON text                 | Binary protobuf                      | Binary TLV                                |
| Dev experience    | Easy                      | Requires .proto + codegen            | Uses your existing schemas                |
| Payload size      | Baseline                  | Small                                | **53% smaller than JSON** (measured)      |
| Bandwidth usage   | Baseline                  | Lower                                | **28–31% lower than JSON** (measured)     |
| HTTP version      | HTTP/1.1                  | HTTP/2 (required)                    | HTTP/1.1, HTTP/2, HTTP/3                  |
| Browser support   | Native                    | Requires grpc-web proxy              | Via WASM                                  |
| Field whitelist   | None                      | .proto schema                        | Contract hash per route                   |
| Schema migration  | Manual                    | Regenerate .proto                    | Regenerate contract file                  |

---

## Bandwidth Reduction — Measured Results

The following numbers are from a real-world concurrent benchmark (100 concurrent users, 10,000 requests each):

```
Environment:  Apple M-series, macOS, uvicorn + uvloop + httptools
Framework:    FastAPI + Pydantic v2, 1 worker
Tool:         Apache Benchmark (ab -k -c 100 -n 10000)
Payload:      Realistic nested JSON — user profile with devices, location, metadata
```

| Metric         | Standard JSON | HRest Binary | Delta              |
|----------------|---------------|--------------|--------------------|
| Payload size   | 574 bytes     | 268 bytes    | **-53.3%**         |
| Bandwidth used | 2,237 KB/s    | 1,570 KB/s   | **-29.8%**         |
| Throughput     | 9,625 req/s   | 8,690 req/s  | -9.7% (acceptable) |
| Errors         | 0             | 0            | —                  |

**Bandwidth is reduced proportionally to payload size.** Because each HRest response is 53% smaller, clients using HRest consume roughly 30% less download bandwidth for the same number of requests.

The throughput gap (9.7%) is the unavoidable cost of encode/decode middleware — Rust-side processing takes ~7 microseconds per request; the remaining overhead is ASGI Python machinery.

---

## Architecture

```
+------------------------------------------------+
|  Backend Developer (FastAPI / Express / etc.)  |
|  Pydantic Model / Zod Schema / DTO             |
|                   |                            |
|        hrest-py / hrest-express                |  <- Middleware
|        reads schema, auto-assigns field IDs    |
+-------------------+----------------------------+
                    | calls
                    v
+------------------------------------------------+
|             hrest-core  (Rust)                 |  <- This repository
|                                                |
|  encode(route, json, &contract) -> Vec<u8>     |
|  decode(route, bytes, &contract) -> String     |
|  verify_hash(contract, client_hash) -> bool    |
+-------------------+----------------------------+
                    |
         +----------+----------+
         v                     v
   C FFI (.so/.dll)      WASM (.wasm)
   hrest-py (PyO3)       hrest-js (npm)
   hrest-go (cgo)        Browser SDK
```

---

## Wire Format

Every field is packed using a compact **TLV (Type-Length-Value)** structure:

```
+--------------+--------------+-------------------------+
| FieldID (1B) | DataType (1B)| Value (N bytes)         |
+--------------+--------------+-------------------------+
```

### Type Table (Wire Format v2)

| Type Byte | Type    | Encoding                                                 |
|-----------|---------|----------------------------------------------------------|
| `0x00`    | Null    | *(no value bytes)*                                       |
| `0x01`    | String  | `[u8 length*][UTF-8 bytes]`                              |
| `0x02`    | Integer | Zigzag varint — efficient for small and negative numbers |
| `0x03`    | Boolean | `0x00` = false, `0x01` = true                            |
| `0x04`    | Bytes   | `[u8 length*][raw bytes]`                                |
| `0x05`    | Nested  | `[kind][u8 count*][...]` — recursive object or array     |
| `0x06`    | Float64 | IEEE 754 f64, 8 bytes little-endian                      |
| `0x07`    | Float32 | IEEE 754 f32, 4 bytes little-endian                      |

> `*` Compact encoding: values `< 255` use 1 byte; values `>= 255` use sentinel `0xFF` + 2-byte u16-LE.
> In practice, nearly all strings, arrays, and objects are under 255 elements, so the overhead is always 1 byte.

### Why Float32?

For most application data (GPS coordinates, percentages, scores, battery levels), f32 provides approximately 7 significant digits of precision — more than sufficient. Using f32 instead of f64 saves **4 bytes per float field**. If full double precision is required (e.g. financial amounts), use integer encoding in cents or pass raw f64 bytes via the `Bytes` type.

---

## Installation

### As a Rust Crate

```toml
# Cargo.toml
[dependencies]
hrest-core = "0.1"

# With C FFI (shared library for Python/Go/etc.):
hrest-core = { version = "0.1", features = ["ffi"] }

# With WASM (for browser/Node.js):
hrest-core = { version = "0.1", features = ["wasm"] }
```

---

## Quick Start — Rust

```rust
use hrest_core::{encode, decode, JsonContractLoader};

fn main() {
    // 1. Load contract (auto-generated by middleware)
    let contract_json = std::fs::read_to_string("hrest-contract.json").unwrap();
    let loader = JsonContractLoader::from_str(&contract_json).unwrap();

    // 2. Encode: JSON -> binary (send to client)
    let payload = r#"{"event": "start", "task_id": 42, "headless": true}"#;
    let binary = encode("POST /api/v1/test/run", payload, &loader).unwrap();

    println!("JSON  : {} bytes", payload.len());
    println!("Binary: {} bytes ({:.0}% smaller)",
        binary.len(),
        (1.0 - binary.len() as f64 / payload.len() as f64) * 100.0
    );

    // 3. Decode: binary -> JSON (receive from client)
    let restored = decode("POST /api/v1/test/run", &binary, &loader).unwrap();
    println!("Restored: {}", restored);
}
```

**Output:**
```
JSON  : 51 bytes
Binary: 15 bytes (~71% smaller)
Restored: {"event":"start","headless":true,"task_id":42}
```

---

## Contract File

HRest uses `hrest-contract.json` as the **single source of truth** between backend and clients:

```json
{
  "version": "1.0.0",
  "contract_hash": "a3f8b2c9...",
  "routes": {
    "POST /api/v1/test/run": {
      "fields": {
        "event":      1,
        "headless":   2,
        "task_id":    3,
        "target_url": 4,
        "timeout":    5
      }
    }
  }
}
```

> Field IDs are auto-assigned alphabetically by the middleware. Developers never write these numbers manually.

### Hash Validation

```rust
// Backend: verify X-Hrest-Hash header from incoming client request
let client_hash = request.headers["X-Hrest-Hash"];
if !loader.verify_hash(client_hash) {
    return Err(StatusCode::PRECONDITION_FAILED); // HTTP 412
}

// Compute hash from a contract (used by middleware at startup)
let hash = JsonContractLoader::compute_hash(&contract_json).unwrap();
```

---

## Security Model

| Threat                  | Protection                                                              |
|-------------------------|-------------------------------------------------------------------------|
| Field injection         | Per-route field whitelist — unknown field ID returns HTTP 422           |
| Token injection         | Type byte whitelist — unknown byte returns HTTP 422                     |
| Buffer overflow         | `ByteCursor` bounds-checked — every read validated before access        |
| Stack overflow          | Max nesting depth = 32 — deeply nested binary is rejected               |
| Memory exhaustion       | Soft pre-alloc cap of 256 — crafted count field cannot OOM              |
| NaN / Infinity float    | Rejected — not representable in JSON                                    |
| Contract drift          | SHA-256 hash header — mismatch returns HTTP 412                         |
| Non-deterministic hash  | `BTreeMap` sorted keys — stable hash across all runs and platforms      |
| Duplicate field IDs     | Detected at contract load time — invalid contract is rejected           |

---

## FFI — Python Integration (PyO3)

```bash
# Build shared library
cargo build --release --features ffi

# Output:
#   target/release/libhrest_core.dylib  (macOS)
#   target/release/libhrest_core.so     (Linux)
#   target/release/hrest_core.dll       (Windows)
```

```python
# Python — via hrest-py (PyO3 binding)
from hrest._core import HrestLoader

# Parse contract once at startup — not per request
loader = HrestLoader(open("hrest-contract.json").read())

# Decode incoming binary request body
json_str = loader.decode("POST /api/v1/test/run", request.body)

# Encode outgoing JSON response to binary
binary = loader.encode("POST /api/v1/test/run", json_str)
```

---

## WASM — Browser / Node.js Integration

```bash
# Build WASM package
wasm-pack build --features wasm --target web     # browser
wasm-pack build --features wasm --target nodejs   # Node.js
```

```javascript
// JavaScript / TypeScript (preview: hrest-js coming soon)
import init, { encode, decode, verifyHash } from './pkg/hrest_core.js';

await init();

const contract = await fetch('/_hrest/contract').then(r => r.text());

// Encode payload and send as binary
const binary = encode(
  "POST /api/v1/test/run",
  JSON.stringify({ event: "start", task_id: 42 }),
  contract
);

const response = await fetch('/api/v1/test/run', {
  method: 'POST',
  headers: {
    'Content-Type': 'application/hrest',
    'X-Hrest-Hash': contractHash,
  },
  body: binary,
});

// Decode binary response
const resultBinary = new Uint8Array(await response.arrayBuffer());
const result = JSON.parse(decode("POST /api/v1/test/run", resultBinary, contract));
```

---

## HTTP Headers

| Header                        | Direction          | Description                             |
|-------------------------------|--------------------|-----------------------------------------|
| `Content-Type: application/hrest` | Request + Response | Signals binary HRest payload        |
| `X-Hrest-Hash: <sha256>`      | Request            | Client contract version validation      |
| `X-Hrest-Version: 1.0.0`     | Request            | Protocol version check                  |
| `X-Hrest-Error: <CODE>`       | Response           | Machine-readable error code             |

**Error code — HTTP status mapping:**

| `HrestError`        | HTTP Status | `X-Hrest-Error` Header |
|---------------------|-------------|------------------------|
| `UnknownRoute`      | 400         | `UNKNOWN_ROUTE`        |
| `UnknownField`      | 422         | `UNKNOWN_FIELD`        |
| `UnknownToken`      | 422         | `UNKNOWN_TOKEN`        |
| `BufferOverflow`    | 400         | `BUFFER_OVERFLOW`      |
| `MalformedPayload`  | 400         | `MALFORMED_PAYLOAD`    |
| Hash mismatch       | 412         | `HASH_MISMATCH`        |

> Header processing is handled by middleware (hrest-py, hrest-express, etc.), not by `hrest-core` directly.

---

## Test Suite

```bash
# Run all tests (58 integration + 18 unit + 4 doc = 80 total)
cargo test

# Unit tests only
cargo test --lib

# Integration tests only
cargo test --test integration_test

# With verbose output
cargo test -- --nocapture
```

**Results:**
```
running 18 tests (unit)        ... ok
running 58 tests (integration) ... ok
running  4 tests (doc)         ... ok
------------------------------------
total: 80 passed, 0 failed
```

**Test categories:**
- Round-trip: all 8 types (null, string, int, bool, bytes, nested, float64, float32)
- Security [SEC]: depth attack, NaN/Infinity, memory exhaustion, token injection, fuzz
- Reliability [REL]: Unicode, i64 boundaries, float precision, subnormal floats
- Durability [DUR]: hash determinism, duplicate IDs, wire format stability

---

## Roadmap

- [x] `hrest-core` — Rust binary engine (encode / decode / verify)
- [x] C FFI exports (`libhrest_core.so`)
- [x] WASM exports (`hrest_core.wasm`)
- [x] Wire format v2 — compact u8 lengths, f32 float support
- [ ] `hrest-py` — Python middleware (FastAPI + Pydantic auto-schema)
- [ ] `hrest-express` — Node.js middleware (Express + Zod)
- [ ] `hrest-fastify` — Fastify plugin
- [ ] `hrest-laravel` — PHP middleware
- [ ] `hrestc` — CLI tool (validate, export contract)
- [ ] `hrest-client-ts` — TypeScript client SDK

---

## Repository Structure

```
hyperrest/
+-- hrest-core/                <- Rust core engine (this repository)
|   +-- src/
|   |   +-- lib.rs             Public API: encode(), decode()
|   |   +-- domain/            Pure Rust, zero external deps
|   |   |   +-- error.rs       HrestError enum
|   |   |   +-- data_type.rs   DataType enum + TYPE_* constants (v2)
|   |   |   +-- contract.rs    ContractData + FieldMap
|   |   +-- application/       Use cases (Clean Architecture)
|   |   |   +-- ports.rs       Traits: ContractProvider, Encode, Decode
|   |   |   +-- encoder.rs     JSON -> binary TLV (v2)
|   |   |   +-- decoder.rs     binary TLV -> JSON, depth-limited (v2)
|   |   +-- infrastructure/    External adapters
|   |   |   +-- contract_loader.rs  JsonContractLoader (serde_json, sha2)
|   |   |   +-- varint.rs           Zigzag varint encode/decode
|   |   +-- ffi/               C exports (feature = "ffi")
|   |   +-- wasm/              WASM exports (feature = "wasm")
|   +-- tests/
|       +-- integration_test.rs
+-- .gitignore
+-- LICENSE
```

---

## Contributing

Pull requests are welcome. For major changes, please open an issue first to discuss what you would like to change.

```bash
git clone https://github.com/oririfai/hrest-core
cd hrest-core
cargo test        # all tests must pass
cargo clippy      # no warnings allowed
```

---

## License

[MIT](../LICENSE) © 2026 HyperRest Project
