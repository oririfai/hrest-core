# HRest Core (hrest-core)

[Benchmark](https://github.com/oririfai/hrest-benchmark) | [Core](https://github.com/oririfai/hrest-core) | [CLI](https://github.com/oririfai/hrest-cli) | [Python](https://github.com/oririfai/hrest-py) | [Node](https://github.com/oririfai/hrest-node) | [Go](https://github.com/oririfai/hrest-go) | [TS](https://github.com/oririfai/hrest-ts)

---

HRest Core is the ultra-fast Rust engine powering the entire Hyper-REST ecosystem. It implements the binary serialization, dictionary-based compression, and zero-copy JSON translation logic that allows HRest to drastically reduce network payloads with minimal CPU overhead.

By centralizing the protocol logic in Rust, HRest ensures exactly identical behavior, safety, and deterministic memory performance across all supported platforms and language stacks.

## Compilation Targets

`hrest-core` is explicitly designed to be a polyglot foundation. It exports bindings tailored for multiple runtime environments:

1. **Python (`PyO3`)**: 
   Compiles into a native Python extension module. It bypasses the Global Interpreter Lock (GIL) to perform heavy serialization, resulting in extreme throughput for FastAPI/Starlette applications.

2. **Node.js & Web (`wasm-pack`)**: 
   Compiles to WebAssembly (WASM). It allows the Node.js middleware to run near-native speeds outside the standard V8 garbage collection cycle.

3. **Go (`CGO`)**: 
   Compiles to a static C-compatible library (`libhrest_core.a`). Go integrates with this via CGO, utilizing `unsafe.Pointer` to share memory buffers, achieving true zero-copy JSON parsing at the boundary layer.

## Core Concepts

### Dictionary-Based Compression
Instead of sending repetitive JSON keys (e.g., `{"id": 1, "status": "active"}`), HRest relies on static contracts. The core engine maps these keys to single-byte identifiers in memory, transforming the payload into a highly packed binary format over the wire.

### Memory Safety & Performance
Rust's strict memory ownership model guarantees that HRest middleware will never leak memory or encounter data races under high-concurrency environments, essential for enterprise gateways and reverse proxies.

## Development

To build the targets locally:

```bash
# Build static library for C/Go
cargo build --release

# Build Python bindings
maturin develop --release

# Build WASM bindings
wasm-pack build --target nodejs
```

## License

[MIT](../LICENSE) © 2026 HyperRest Project
