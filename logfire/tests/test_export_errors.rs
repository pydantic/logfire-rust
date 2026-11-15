//! Exercise collector failures without replacing the SDK's HTTP client.

#![cfg(feature = "export-http-protobuf")]

use opentelemetry::metrics::MeterProvider;
use opentelemetry_proto::tonic::{
    collector::metrics::v1::ExportMetricsServiceRequest,
    metrics::v1::{metric::Data, number_data_point::Value},
};
use opentelemetry_sdk::{
    error::OTelSdkResult,
    logs::{LogBatch, LogExporter},
    metrics::{
        SdkMeterProvider, data::ResourceMetrics, exporter::PushMetricExporter,
        periodic_reader_with_async_runtime::PeriodicReader,
    },
    runtime,
    trace::SpanExporter,
};
use prost::Message;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn metric_export_preserves_http_status() {
    #[cfg(not(feature = "tls-aws-lc"))]
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    for (status, attempts) in [(401, 1), (429, 4), (503, 4)] {
        let collector = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/metrics"))
            .respond_with(ResponseTemplate::new(status))
            .expect(attempts)
            .mount(&collector)
            .await;

        let exporter = logfire::exporters::metric_exporter(&collector.uri(), None).unwrap();
        let reader = PeriodicReader::builder(exporter, runtime::Tokio).build();
        let provider = SdkMeterProvider::builder().with_reader(reader).build();
        provider
            .meter("export-status-test")
            .u64_counter("requests")
            .build()
            .add(7, &[]);

        let result = provider.force_flush().map_err(|error| error.to_string());
        provider.shutdown().unwrap();
        let error = result.unwrap_err();
        assert!(
            error.contains(&format!("status code: {status}")),
            "HTTP {status}: {error}"
        );

        let requests = collector.received_requests().await.unwrap();
        assert_eq!(requests.len() as u64, attempts);
        assert!(
            requests
                .iter()
                .all(|request| request.body == requests[0].body)
        );
        let request = ExportMetricsServiceRequest::decode(requests[0].body.as_slice()).unwrap();
        let Some(Data::Sum(sum)) = &request.resource_metrics[0].scope_metrics[0].metrics[0].data
        else {
            panic!("expected the exported counter");
        };
        assert_eq!(sum.data_points[0].value, Some(Value::AsInt(7)));
    }
}

#[derive(Clone, Copy)]
enum Signal {
    Traces,
    Logs,
    Metrics,
}

impl Signal {
    fn path(self) -> &'static str {
        match self {
            Self::Traces => "/v1/traces",
            Self::Logs => "/v1/logs",
            Self::Metrics => "/v1/metrics",
        }
    }

    async fn export(self, endpoint: &str) -> OTelSdkResult {
        match self {
            Self::Traces => {
                logfire::exporters::span_exporter(endpoint, None)
                    .unwrap()
                    .export(Vec::new())
                    .await
            }
            Self::Logs => {
                logfire::exporters::log_exporter(endpoint, None)
                    .unwrap()
                    .export(LogBatch::new(&[]))
                    .await
            }
            Self::Metrics => {
                logfire::exporters::metric_exporter(endpoint, None)
                    .unwrap()
                    .export(&ResourceMetrics::default())
                    .await
            }
        }
    }
}

#[tokio::test]
async fn transient_http_failures_are_retried() {
    #[cfg(not(feature = "tls-aws-lc"))]
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    for signal in [Signal::Traces, Signal::Logs, Signal::Metrics] {
        for status in [429, 502, 503, 504] {
            let collector = MockServer::start().await;
            Mock::given(method("POST"))
                .and(path(signal.path()))
                .respond_with(ResponseTemplate::new(status))
                .up_to_n_times(2)
                .with_priority(1)
                .expect(2)
                .mount(&collector)
                .await;
            Mock::given(method("POST"))
                .and(path(signal.path()))
                .respond_with(ResponseTemplate::new(200))
                .with_priority(2)
                .expect(1)
                .mount(&collector)
                .await;

            signal.export(&collector.uri()).await.unwrap();
            let requests = collector.received_requests().await.unwrap();
            assert_eq!(requests.len(), 3);
            assert!(
                requests
                    .iter()
                    .all(|request| request.body == requests[0].body)
            );
        }
    }
}

#[tokio::test]
async fn retry_after_beyond_export_timeout_is_not_retried() {
    #[cfg(not(feature = "tls-aws-lc"))]
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    for signal in [Signal::Traces, Signal::Logs, Signal::Metrics] {
        let collector = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path(signal.path()))
            .respond_with(ResponseTemplate::new(429).insert_header("retry-after", "120"))
            .expect(1)
            .mount(&collector)
            .await;

        let error = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            signal.export(&collector.uri()),
        )
        .await
        .expect("Retry-After exceeds the default export timeout")
        .unwrap_err();
        assert!(error.to_string().contains("status code: 429"));
        assert_eq!(collector.received_requests().await.unwrap().len(), 1);
    }
}

#[tokio::test]
async fn http_retries_are_bounded() {
    #[cfg(not(feature = "tls-aws-lc"))]
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    for signal in [Signal::Traces, Signal::Logs, Signal::Metrics] {
        for (status, attempts) in [(401, 1), (413, 1), (503, 4)] {
            let collector = MockServer::start().await;
            Mock::given(method("POST"))
                .and(path(signal.path()))
                .respond_with(ResponseTemplate::new(status))
                .expect(attempts)
                .mount(&collector)
                .await;

            let error = signal.export(&collector.uri()).await.unwrap_err();
            assert!(
                error
                    .to_string()
                    .contains(&format!("status code: {status}")),
                "HTTP {status}: {error}"
            );
            assert_eq!(
                collector.received_requests().await.unwrap().len() as u64,
                attempts
            );
        }
    }
}
