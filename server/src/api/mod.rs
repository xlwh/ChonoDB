use axum::{
    routing::{get, post},
    Router,
};
use std::sync::Arc;
use crate::state::ServerState;

pub mod handlers;
pub mod models;
pub mod response;
pub mod opentsdb;

use handlers::*;
use crate::remote_server;

pub fn create_routes(state: Arc<ServerState>) -> Router {
    Router::new()
        .route("/api/v1/query", axum::routing::MethodRouter::new().get(handle_query_get).post(handle_query_post))
        .route("/api/v1/query_range", axum::routing::MethodRouter::new().get(handle_query_range_get).post(handle_query_range_post))
        .route("/api/v1/series", get(handle_series))
        .route("/api/v1/labels", get(handle_labels))
        .route("/api/v1/label/:name/values", get(handle_label_values))
        .route("/api/v1/metadata", get(handle_metadata))
        .route("/api/v1/targets", get(handle_targets))
        .route("/api/v1/rules", get(handle_rules))
        .route("/api/v1/alerts", get(handle_alerts))
        .route("/api/v1/write", post(remote_server::handle_remote_write))
        .route("/api/v1/read", post(remote_server::handle_remote_read))
        .route("/api/put", post(opentsdb::handle_opentsdb_put))
        .route("/-/healthy", get(handle_healthy))
        .route("/-/ready", get(handle_ready))
        .route("/api/v1/status/runtimeinfo", get(handle_runtime_info))
        .route("/api/v1/status/buildinfo", get(handle_build_info))
        .with_state(state)
}
