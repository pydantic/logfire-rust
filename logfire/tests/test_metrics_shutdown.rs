//! Regression tests for shutdown of metrics export.
//!
//! The async-runtime `PeriodicReader` spawns its worker task lazily during
//! `SdkMeterProvider::builder().build()` (via `MetricReader::register_pipeline`), on
//! whatever tokio runtime is ambient at that point. Before this was fixed, the worker
//! would land on the *caller's* runtime when `configure()` was called inside a tokio
//! context, so `shutdown()` from a single-threaded runtime deadlocked (it blocks the only
//! thread which could drive the worker); calling `configure()` outside any tokio context
//! panicked instead ("there is no reactor running").
#![cfg(feature = "export-http-protobuf")]

use std::time::Duration;

use logfire::config::{AdvancedOptions, MetricsOptions};
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Runs `configure()` + counter + `shutdown()` inside `rt` on a fresh thread, returning
/// the number of requests the mock server received to `/v1/metrics`, or an error if the
/// scenario did not complete within a generous timeout (i.e. shutdown hung).
fn run_scenario_in_runtime(
    rt: tokio::runtime::Runtime,
    shutdown_via_guard_drop: bool,
) -> Result<usize, std::sync::mpsc::RecvTimeoutError> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let metrics_requests = rt.block_on(async {
            let mock_server = MockServer::start().await;
            mock_server
                .register(Mock::given(method("POST")).respond_with(ResponseTemplate::new(200)))
                .await;

            let logfire = logfire::configure()
                .local()
                .send_to_logfire(true)
                .with_token("test-token")
                .with_metrics(Some(MetricsOptions::default()))
                .with_advanced_options(AdvancedOptions::default().with_base_url(mock_server.uri()))
                .finish()
                .expect("failed to configure logfire");

            logfire.metrics().u64_counter("counter").build().add(1, &[]);

            if shutdown_via_guard_drop {
                drop(logfire.shutdown_guard());
            } else {
                logfire.shutdown().expect("shutdown should succeed");
            }

            mock_server
                .received_requests()
                .await
                .unwrap_or_default()
                .iter()
                .filter(|r| r.url.path() == "/v1/metrics")
                .count()
        });
        let _ = tx.send(metrics_requests);
    });
    // this would hang forever on regression; a timeout makes the test fail instead
    rx.recv_timeout(Duration::from_secs(60))
}

/// `shutdown()` called from inside a single-threaded tokio runtime used to deadlock.
#[test]
fn test_shutdown_inside_current_thread_runtime() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("failed to build runtime");
    let metrics_requests =
        run_scenario_in_runtime(rt, false).expect("shutdown hung inside current_thread runtime");
    assert!(metrics_requests > 0, "metrics should have been exported");
}

/// Same as above, but shutting down by dropping a `ShutdownGuard` (as at the end of an
/// `#[tokio::main]` function).
#[test]
fn test_shutdown_guard_drop_inside_current_thread_runtime() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("failed to build runtime");
    let metrics_requests = run_scenario_in_runtime(rt, true)
        .expect("shutdown guard drop hung inside current_thread runtime");
    assert!(metrics_requests > 0, "metrics should have been exported");
}

/// `shutdown()` called from inside a multi-threaded tokio runtime.
#[test]
fn test_shutdown_inside_multi_thread_runtime() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .expect("failed to build runtime");
    let metrics_requests =
        run_scenario_in_runtime(rt, false).expect("shutdown hung inside multi_thread runtime");
    assert!(metrics_requests > 0, "metrics should have been exported");
}

/// `configure()` with metrics enabled from a plain synchronous context (no ambient tokio
/// runtime) used to panic with "there is no reactor running".
#[test]
fn test_configure_and_shutdown_outside_runtime() {
    // host the mock server on a dedicated runtime thread so that the test thread itself
    // has no ambient tokio runtime
    let (uri_tx, uri_rx) = std::sync::mpsc::channel();
    let (stop_tx, stop_rx) = tokio::sync::oneshot::channel::<()>();
    let server_thread = std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("failed to build runtime");
        rt.block_on(async {
            let mock_server = MockServer::start().await;
            mock_server
                .register(Mock::given(method("POST")).respond_with(ResponseTemplate::new(200)))
                .await;
            uri_tx
                .send(mock_server.uri())
                .expect("failed to send server uri");
            let _ = stop_rx.await;
            mock_server
                .received_requests()
                .await
                .unwrap_or_default()
                .iter()
                .filter(|r| r.url.path() == "/v1/metrics")
                .count()
        })
    });
    let uri = uri_rx
        .recv_timeout(Duration::from_secs(10))
        .expect("mock server failed to start");

    let logfire = logfire::configure()
        .local()
        .send_to_logfire(true)
        .with_token("test-token")
        .with_metrics(Some(MetricsOptions::default()))
        .with_advanced_options(AdvancedOptions::default().with_base_url(uri))
        .finish()
        .expect("failed to configure logfire");

    logfire.metrics().u64_counter("counter").build().add(1, &[]);
    logfire.shutdown().expect("shutdown should succeed");

    stop_tx.send(()).expect("failed to stop mock server");
    let metrics_requests = server_thread.join().expect("server thread panicked");
    assert!(metrics_requests > 0, "metrics should have been exported");
}
