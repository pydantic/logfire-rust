//! This binary owns the process-wide tracing default.

use opentelemetry_sdk::trace::{InMemorySpanExporterBuilder, SimpleSpanProcessor};
use tracing::level_filters::LevelFilter;

fn cold_span() -> tracing::Span {
    logfire::span!("first seen under an override")
}

#[test]
fn global_recording_scope_routes_and_restores_traces() {
    let exporter = InMemorySpanExporterBuilder::new().build();
    let global = logfire::configure()
        .send_to_logfire(false)
        .with_console(None)
        .with_default_level_filter(LevelFilter::TRACE)
        .with_additional_span_processor(SimpleSpanProcessor::new(exporter.clone()))
        .finish()
        .expect("configure recording pipeline");

    let parent = logfire::span!("parent");
    parent.in_scope(|| {
        std::thread::spawn(|| {
            tracing::dispatcher::with_default(&tracing::Dispatch::none(), || {
                assert!(cold_span().is_disabled());
            });
        })
        .join()
        .expect("thread with no-op override");
        let _scope = logfire::set_local_logfire(global.clone());
        drop(cold_span());
    });
    drop(parent);

    let other_exporter = InMemorySpanExporterBuilder::new().build();
    let other = logfire::configure()
        .local()
        .send_to_logfire(false)
        .with_console(None)
        .with_default_level_filter(LevelFilter::TRACE)
        .with_additional_span_processor(SimpleSpanProcessor::new(other_exporter.clone()))
        .finish()
        .expect("configure recording pipeline");
    {
        let _other = logfire::set_local_logfire(other.clone());
        drop(logfire::span!("foreign before"));
        {
            let _global = logfire::set_local_logfire(global.clone());
            drop(logfire::span!("worker global"));
        }
        drop(logfire::span!("foreign after"));
    }

    let spans = exporter.get_finished_spans().expect("exported spans");
    let child = spans
        .iter()
        .find(|s| s.name == "first seen under an override")
        .expect("exported span after scope entry");
    let parent = spans
        .iter()
        .find(|s| s.name == "parent")
        .expect("exported parent");
    assert_eq!(child.parent_span_id, parent.span_context.span_id());
    assert_eq!(
        child.span_context.trace_id(),
        parent.span_context.trace_id()
    );
    assert!(spans.iter().any(|s| s.name == "worker global"));
    assert!(!spans.iter().any(|s| s.name.starts_with("foreign ")));
    let foreign = other_exporter.get_finished_spans().expect("exported spans");
    assert!(foreign.iter().any(|s| s.name == "foreign before"));
    assert!(foreign.iter().any(|s| s.name == "foreign after"));
    assert!(!foreign.iter().any(|s| s.name == "worker global"));
}
