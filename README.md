# LumenKV 🦀

> A high-performance, persistent Key-Value store built from scratch in Rust.

![Build Status](https://img.shields.io/badge/build-passing-brightgreen)
![Language](https://img.shields.io/badge/language-Rust-orange)
![License](https://img.shields.io/badge/license-MIT-blue)

## 📖 Overview

LumenKV is a persistent, log-structured database engine designed to demonstrate the core principles of database internals: **Durability**, **Concurrency**, and **RPC Architecture**.

Unlike wrapper libraries, LumenKV implements its own storage engine using a **LSM-Tree** architecture and a **Write-Ahead Log (WAL)** to ensure ACID compliance (Atomicity & Durability).

## 🛠️ Architecture

LumenKV follows a modular systems architecture, separating the gRPC transport layer from the storage engine.

### Runtime Data Flow

```text
gRPC Client
    │
    ▼  TCP :50051
┌─────────────────────────────────────────────────────────┐
│  lumen-server                                           │
│  ┌───────────────────────────────────────────────────┐  │
│  │  KvService  (tonic async_trait impl)              │  │
│  │  • validates request fields                       │  │
│  │  • maps EngineError → tonic::Status               │  │
│  └──────────────────┬────────────────────────────────┘  │
│                     │ Arc<Engine>                       │
│  ┌──────────────────▼────────────────────────────────┐  │
│  │  Engine  (lumen-core)                             │  │
│  │  • Arc<RwLock<BTreeMap>>  ← memtable              │  │
│  │  • Arc<Mutex<WriteAheadLog>>                      │  │
│  │                                                   │  │
│  │  put()  → WAL.append(PUT)  → memtable.insert      │  │
│  │  delete()→ WAL.append(DEL) → memtable.remove      │  │
│  │  get()  → memtable.read (lock-free concurrent)    │  │
│  └──────────────────┬────────────────────────────────┘  │
│                     │                                   │
│  ┌──────────────────▼────────────────────────────────┐  │
│  │  WriteAheadLog                                    │  │
│  │  BufWriter<File>  (O_APPEND, flushed per record)  │  │
│  │  Format: [Op][CRC32][KeyLen][ValLen][Key][Val]    │  │
│  └───────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────┘
             /data/wal.log  (persisted on disk)
```

## 🧱 Core Components

### 1. Storage Engine (`lumen-core`)
* **Memtable:** In-memory `BTreeMap` protected by fine-grained `RwLock`.
* **WAL:** Append-only log using `BufWriter<File>` with `O_APPEND` system calls.
* **Integrity:** Custom binary format `[Op][CRC32][KeyLen][ValLen][Key][Val]` ensures corruption detection on recovery.
* **Durability:** `fsync` guarantees data survives power loss.

### 2. Network Layer (`lumen-server`)
* Built on **gRPC** (Tonic) and **Protocol Buffers** (Prost).
* Asynchronous request handling via the **Tokio** runtime.

### 3. Observability
* Structured logging via `tracing` and `tracing-subscriber`.

## 🚀 Performance

Benchmarked on Fedora Linux (AMD Ryzen 5625U):

| Metric | Value | Notes |
| :--- | :--- | :--- |
| **Throughput** | **16,399 ops/sec** | Concurrent (100 clients) |
| **Throughput** | **2,597 ops/sec** | Synchronous (Sequential) |
| **Latency (P99)** | **520 µs** | Sub-millisecond Write Latency |
| **Payload Size** | 128 bytes | |
| **Consistency** | Strong | WAL-backed durability |

## 📦 Installation & Usage

### Prerequisites
* Rust (Cargo)
* Protobuf Compiler (`protoc`)
* Docker (Optional)

### 1. Start the Server
```bash
cargo run --release --bin lumen-server

```

### 2. Run the Benchmark
```bash
cargo run --release --bin lumen-bench
```
### 3. CLI Usage (via grpcurl)
```bash
# Put a value (Base64 encoded)
grpcurl -plaintext -import-path ./proto -proto kv.proto \
  -d '{"key":"faang", "value":"aGlyZWQ="}' \
  localhost:50051 kv.KeyValueStore/Put

# Get a value
grpcurl -plaintext -import-path ./proto -proto kv.proto \
  -d '{"key":"faang"}' \
  localhost:50051 kv.KeyValueStore/Get
```
### 4. Docker Deployment
```bash
docker build -t lumen-kv:latest .
docker run --rm -p 50051:50051 -v lumen-data:/data lumen-kv:latest
```

## 🧠 Why Rust?
Chosen for its **Zero-Cost Abstractions** and **Memory Safety**.

* **`Arc<RwLock<T>>`** ensures thread-safe access to the Memtable without a global mutex bottleneck.
* **Tokio** allows handling thousands of concurrent connections with minimal OS-thread overhead.
