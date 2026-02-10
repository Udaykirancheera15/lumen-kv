//! gRPC service implementation for the `KeyValueStore` interface.
//!
//! Each RPC handler:
//!   1. Validates the request.
//!   2. Delegates to the `Engine`.
//!   3. Maps engine errors to an appropriate `tonic::Status` code.

use std::sync::Arc;

use tonic::{Request, Response, Status};
use tracing::{error, info, instrument};

use lumen_core::Engine;

use crate::kv::{
    key_value_store_server::KeyValueStore,
    DeleteRequest, DeleteResponse,
    GetRequest, GetResponse,
    PutRequest, PutResponse,
};

// ---------------------------------------------------------------------------
// KvService
// ---------------------------------------------------------------------------

/// Stateless wrapper that holds a shared reference to the storage engine.
#[derive(Debug)]
pub struct KvService {
    engine: Arc<Engine>,
}

impl KvService {
    pub fn new(engine: Arc<Engine>) -> Self {
        Self { engine }
    }
}

// ---------------------------------------------------------------------------
// RPC implementations
// ---------------------------------------------------------------------------

#[tonic::async_trait]
impl KeyValueStore for KvService {
    /// Write a key/value pair.
    #[instrument(name = "rpc_put", skip(self, request))]
    async fn put(
        &self,
        request: Request<PutRequest>,
    ) -> Result<Response<PutResponse>, Status> {
        let req = request.into_inner();

        if req.key.is_empty() {
            return Err(Status::invalid_argument("key must not be empty"));
        }

        info!(key = %req.key, value_bytes = req.value.len(), "PUT");

        self.engine
            .put(req.key.clone(), req.value.into())
            .map_err(|e| {
                error!(key = %req.key, error = %e, "PUT failed");
                Status::internal(e.to_string())
            })?;

        Ok(Response::new(PutResponse { success: true }))
    }

    /// Read the value for a key.
    ///
    /// Returns `found = false` (and an empty value) when the key is absent —
    /// this is NOT treated as an error at the RPC layer.
    #[instrument(name = "rpc_get", skip(self, request))]
    async fn get(
        &self,
        request: Request<GetRequest>,
    ) -> Result<Response<GetResponse>, Status> {
        let req = request.into_inner();

        if req.key.is_empty() {
            return Err(Status::invalid_argument("key must not be empty"));
        }

        info!(key = %req.key, "GET");

        let maybe_value = self.engine.get(&req.key).map_err(|e| {
            error!(key = %req.key, error = %e, "GET failed");
            Status::internal(e.to_string())
        })?;

        match maybe_value {
            Some(value) => Ok(Response::new(GetResponse {
                value: value.into(),
                found: true,
            })),
            None => Ok(Response::new(GetResponse {
                value: Vec::new(),
                found: false,
            })),
        }
    }

    /// Delete a key from the store.
    ///
    /// `success` is `true` when the key existed, `false` when it was already absent.
    #[instrument(name = "rpc_delete", skip(self, request))]
    async fn delete(
        &self,
        request: Request<DeleteRequest>,
    ) -> Result<Response<DeleteResponse>, Status> {
        let req = request.into_inner();

        if req.key.is_empty() {
            return Err(Status::invalid_argument("key must not be empty"));
        }

        info!(key = %req.key, "DELETE");

        let existed = self.engine.delete(&req.key).map_err(|e| {
            error!(key = %req.key, error = %e, "DELETE failed");
            Status::internal(e.to_string())
        })?;

        Ok(Response::new(DeleteResponse { success: existed }))
    }
}
