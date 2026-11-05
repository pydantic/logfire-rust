//! Exercise collector failures without replacing the SDK's HTTP client.

#![cfg(feature = "export-http-protobuf")]

use opentelemetry::metrics::MeterProvider;
use opentelemetry_proto::tonic::{
    collector::metrics::v1::ExportMetricsServiceRequest,
    metrics::v1::{metric::Data, number_data_point::Value},
};
use opentelemetry_sdk::{
    metrics::{SdkMeterProvider, periodic_reader_with_async_runtime::PeriodicReader},
    runtime,
};
use prost::Message;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn metric_export_preserves_http_status() {
    #[cfg(not(feature = "tls-aws-lc"))]
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .unwrap();

    for (status, expected_error) in [
        (401, "status code: 401"),
        (429, "status code: 429"),
        (503, "status code: 503"),
    ] {
        let collector = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/metrics"))
            .respond_with(ResponseTemplate::new(status))
            .expect(1)
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
        assert!(error.contains(expected_error), "HTTP {status}: {error}");

        let requests = collector.received_requests().await.unwrap();
        let request = ExportMetricsServiceRequest::decode(requests[0].body.as_slice()).unwrap();
        let Some(Data::Sum(sum)) = &request.resource_metrics[0].scope_metrics[0].metrics[0].data
        else {
            panic!("expected the exported counter");
        };
        assert_eq!(sum.data_points[0].value, Some(Value::AsInt(7)));
    }
}
