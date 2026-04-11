use axum::{
    routing::{get, post, put},
    Router,
};
use std::sync::Arc;
use crate::state::ServerState;

pub mod handlers;
pub mod models;
pub mod response;
pub mod opentsdb;
pub mod admin;

use handlers::*;
use crate::remote_server;
use admin::*;
use crate::static_files;

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
        .route("/live", get(handle_live))
        .route("/ready", get(handle_ready_check))
        .route("/api/v1/status/runtimeinfo", get(handle_runtime_info))
        .route("/api/v1/status/buildinfo", get(handle_build_info))
        .route("/api/admin/data/put", post(handle_data_put))
        .route("/api/admin/data/batch", post(handle_batch_data_put))
        .route("/api/admin/stats/storage", get(handle_stats_storage))
        .route("/api/admin/stats/query", get(handle_stats_query))
        .route("/api/admin/stats/memory", get(handle_stats_memory))
        .route("/api/admin/config", get(handle_config_get).put(handle_config_put))
        .route("/api/admin/cluster/nodes", get(handle_cluster_nodes))
        .route("/api/admin/cluster/shards", get(handle_cluster_shards))
        .route("/api/admin/alerts/rules", get(handle_alerts_rules_get).post(handle_alerts_rules_post))
        .route("/api/admin/alerts/firing", get(handle_alerts_firing))
        .route("/api/v1/status/config", get(handle_status_config))
        .route("/api/v1/status/flags", get(handle_status_flags))
        .route("/ui", get(static_files::index_handler))
        .route("/ui/", get(static_files::index_handler))
        .fallback(static_files::static_handler)
        .with_state(state)
}
