use axum::{response::IntoResponse, http::StatusCode};
use metrics_exporter_prometheus::PrometheusBuilder;
use std::sync::OnceLock;

static PROMETHEUS_HANDLE: OnceLock<metrics_exporter_prometheus::PrometheusHandle> = OnceLock::new();

pub fn init_metrics() {
    PROMETHEUS_HANDLE.get_or_init(|| {
        let builder = PrometheusBuilder::new();
        builder
            .install_recorder()
            .expect("failed to install Prometheus recorder")
    });
}

pub async fn metrics_handler() -> impl IntoResponse {
    if let Some(handle) = PROMETHEUS_HANDLE.get() {
        (StatusCode::OK, handle.render())
    } else {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Metrics not initialized".to_string(),
        )
    }
}
