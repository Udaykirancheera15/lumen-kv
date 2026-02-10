//! LumenKV — gRPC server entry point.
//!
//! Configuration is read from environment variables:
//!   DATA_DIR  – directory for WAL & future SSTables (default: ./data)
//!   BIND_ADDR – host:port to listen on              (default: 0.0.0.0:50051)
//!   RUST_LOG  – tracing filter (default: info)

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Context;
use tonic::transport::Server;
use tracing::info;
use tracing_subscriber::EnvFilter;

mod service;

/// Generated protobuf / tonic types live inside this module.
pub mod kv {
    tonic::include_proto!("kv");
}

use kv::key_value_store_server::KeyValueStoreServer;
use service::KvService;

const FILE_DESCRIPTOR_SET: &[u8] =
    tonic::include_file_descriptor_set!("kv_descriptor");

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // ── Logging ─────────────────────────────────────────────────────────────
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("lumen_server=info,lumen_core=info"));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .compact()
        .init();

    // ── Configuration ────────────────────────────────────────────────────────
    let data_dir  = std::env::var("DATA_DIR").unwrap_or_else(|_| "./data".to_owned());
    let bind_addr = std::env::var("BIND_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:50051".to_owned())
        .parse::<SocketAddr>()
        .context("BIND_ADDR must be a valid socket address (e.g. 0.0.0.0:50051)")?;

    // ── Storage engine ───────────────────────────────────────────────────────
    let engine = lumen_core::Engine::open(&data_dir)
        .context("Failed to open LumenKV storage engine")?;
    let engine = Arc::new(engine);

    info!(bind_addr = %bind_addr, data_dir = %data_dir, "LumenKV starting");

    // ── gRPC server ──────────────────────────────────────────────────────────
    let reflection = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(FILE_DESCRIPTOR_SET)
        .build()
        .context("Failed to build gRPC reflection service")?;

    Server::builder()
        .add_service(KeyValueStoreServer::new(KvService::new(engine)))
        .add_service(reflection)
        .serve(bind_addr)
        .await
        .context("gRPC server exited with an error")?;

    Ok(())
}
