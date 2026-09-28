#![cfg(feature = "otel")]
//! The OTLP pipeline starts inside a Tokio runtime, exports (to a collector
//! that is not there) without panicking, and shuts down without hanging.

use std::time::Duration;

use stateset_observability::{TracingConfig, init_tracing_otel, shutdown_otel};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn otel_pipeline_starts_exports_and_shuts_down() {
    // SAFETY: this test binary runs this single test, so nothing reads the
    // environment concurrently.
    unsafe { std::env::set_var("OTEL_EXPORTER_OTLP_ENDPOINT", "http://127.0.0.1:9") };
    let config = TracingConfig::new("stateset-otel-test", "test", "local");
    init_tracing_otel(&config).expect("pipeline starts");

    tracing::info_span!("otel_smoke").in_scope(|| tracing::info!("a span to export"));

    let shutdown = tokio::task::spawn_blocking(shutdown_otel);
    tokio::time::timeout(Duration::from_secs(30), shutdown)
        .await
        .expect("shutdown finishes")
        .expect("shutdown task");
}
