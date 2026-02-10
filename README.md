# LumenKV 🦀

> A high-performance, distributed Key-Value store built from scratch in Rust.

![Build Status](https://img.shields.io/badge/build-passing-brightgreen)
![Language](https://img.shields.io/badge/language-Rust-orange)
![License](https://img.shields.io/badge/license-MIT-blue)

## 📖 Overview
LumenKV is a persistent, log-structured database engine designed to demonstrate the core principles of database internals: **Durability**, **Concurrency**, and **RPC Architecture**. 

Unlike wrapper libraries, LumenKV implements its own storage engine using a **LSM-Tree** architecture and a **Write-Ahead Log (WAL)** to ensure ACID compliance (Atomicity & Durability).

## 🚀 Performance
Benchmarked on Fedora Linux (Ryzen/Intel):
| Metric | Value |
| :--- | :--- |
| **Throughput (Concurrent)** | **[INSERT YOUR NEW OPS/SEC HERE] ops/sec** |
| **Latency (P99)** | **520 µs** |
| **Payload Size** | 64 bytes |
| **Consistency** | Strong (WAL-backed) |

## 🛠️ Architecture
LumenKV follows a modular systems architecture:

1.  **Storage Engine (`lumen-core`)**: 
    * **Memtable:** In-memory `BTreeMap` protected by fine-grained `RwLock`.
    * **WAL:** Append-only log with **CRC32 checksums** for crash recovery.
    * **Durability:** `fsync` guarantees data survives power loss.
    
2.  **Network Layer (`lumen-server`)**:
    * Built on **gRPC** (Tonic) and **Protocol Buffers** (Prost).
    * Asynchronous request handling via the **Tokio** runtime.
    
3.  **Observability**:
    * Structured logging via `tracing` and `tracing-subscriber`.
  
<img width="1024" height="559" alt="image" src="https://github.com/user-attachments/assets/77a43fb3-225a-4243-bbd2-46dea6c3d24f" />


## 📦 Installation & Usage

### Prerequisites
* Rust (Cargo)
* Protobuf Compiler (`protoc`)

### 1. Start the Server
```bash
cargo run --release --bin lumen-server
