//! The callsite cache and dispatcher registry are process-wide. Keep this test isolated.

use opentelemetry_sdk::trace::{InMemorySpanExporterBuilder, SimpleSpanProcessor};
use tracing::level_filters::LevelFilter;

#[test]
fn scope_records_a_callsite_first_seen_outside_it() {
    fn span() -> tracing::Span {
        tracing::info_span!("late local span")
    }

    let exporter = InMemorySpanExporterBuilder::new().build();
    let logfire = logfire::configure()
        .local()
        .send_to_logfire(false)
        .with_console(None)
        .with_default_level_filter(LevelFilter::TRACE)
        .with_additional_span_processor(SimpleSpanProcessor::new(exporter.clone()))
        .finish()
        .expect("configure local Logfire");

    drop(logfire::set_local_logfire(logfire.clone()));
    assert!(span().is_disabled());
    let guard = logfire::set_local_logfire(logfire);
    assert!(!span().is_disabled());
    let spans = exporter.get_finished_spans().expect("exported spans");
    assert_eq!(
        spans.iter().filter(|s| s.name == "late local span").count(),
        1
    );
    guard.shutdown().expect("shut down local Logfire");
}
