use axum::{
    middleware,
    routing::{get, post},
    Router,
};
use std::sync::Arc;
use crate::state::ServerState;
use crate::auth::auth_middleware;

pub mod handlers;
pub mod models;
pub mod response;
pub mod opentsdb;

use handlers::*;
use crate::remote_server;

pub fn create_routes(state: Arc<ServerState>) -> Router {
    // 需要认证的路由
    let authenticated_routes = Router::new()
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
        .route("/api/v1/status/runtimeinfo", get(handle_runtime_info))
        .route("/api/v1/status/buildinfo", get(handle_build_info))
        .route("/api/v1/status/config", get(handle_status_config))
        .route("/api/v1/status/flags", get(handle_status_flags))
        .layer(middleware::from_fn_with_state(state.clone(), auth_middleware));

    // 健康检查路由（不需要认证）
    let health_routes = Router::new()
        .route("/-/healthy", get(handle_healthy))
        .route("/-/ready", get(handle_ready))
        .route("/live", get(handle_live))
        .route("/ready", get(handle_ready_check));

    Router::new()
        .merge(authenticated_routes)
        .merge(health_routes)
        .with_state(state)
}
