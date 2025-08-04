use std::collections::HashMap;

use opentelemetry_sdk::{
    error::OTelSdkResult,
    trace::{SpanData, SpanExporter},
};

use crate::internal::span_data_ext::SpanDataExt;

#[derive(Debug)]
pub struct RemovePendingSpansExporter<Inner>(Inner);

impl<Inner> RemovePendingSpansExporter<Inner> {
    pub const fn new(inner: Inner) -> Self {
        Self(inner)
    }
}

impl<Inner: SpanExporter> SpanExporter for RemovePendingSpansExporter<Inner> {
    async fn export(&self, mut spans: Vec<SpanData>) -> OTelSdkResult {
        let mut spans_by_id = HashMap::new();

        spans = spans
            .into_iter()
            .filter_map(|span| {
                let span_type = span.get_span_type();

                match span_type {
                    Some("pending_span") => {
                        // for pending span, the parent is the "real" span
                        let key = (span.span_context.trace_id(), span.parent_span_id);
                        spans_by_id.entry(key).or_insert(span);
                        None
                    }
                    Some("span") => {
                        let key = (span.span_context.trace_id(), span.span_context.span_id());
                        spans_by_id.insert(key, span);
                        None
                    }
                    _ => Some(span),
                }
            })
            .collect();

        spans.extend(spans_by_id.into_values());
        self.0.export(spans).await
    }

    fn shutdown(&mut self) -> OTelSdkResult {
        self.0.shutdown()
    }

    fn force_flush(&mut self) -> OTelSdkResult {
        self.0.force_flush()
    }

    fn set_resource(&mut self, resource: &opentelemetry_sdk::Resource) {
        self.0.set_resource(resource);
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::config::AdvancedOptions;
    use crate::set_local_logfire;
    use crate::test_utils::{DeterministicExporter, DeterministicIdGenerator};

    use super::*;

    use opentelemetry_sdk::trace::BatchConfigBuilder;
    use opentelemetry_sdk::trace::BatchSpanProcessor;
    use opentelemetry_sdk::trace::InMemorySpanExporterBuilder;
    use tracing::Level;
    use tracing::level_filters::LevelFilter;

    #[test]
    fn test_remove_pending_spans() {
        let exporter = InMemorySpanExporterBuilder::new().build();

        let guard = crate::configure()
            .local()
            .send_to_logfire(false)
            .with_additional_span_processor(
                BatchSpanProcessor::builder(DeterministicExporter::new(
                    RemovePendingSpansExporter(exporter.clone()),
                    file!(),
                    line!(),
                ))
                .with_batch_config(
                    // Set a batch delay large enough that all spans will be in a single batch
                    // when .shutdown() is called.
                    BatchConfigBuilder::default()
                        .with_scheduled_delay(Duration::from_secs(1000))
                        .build(),
                )
                .build(),
            )
            .with_default_level_filter(LevelFilter::TRACE)
            .with_advanced_options(
                AdvancedOptions::default().with_id_generator(DeterministicIdGenerator::new()),
            )
            .finish()
            .unwrap();

        let guard = set_local_logfire(guard);

        tracing::subscriber::with_default(guard.subscriber(), || {
            let _root = crate::span!("root span").entered();
            let _hello = crate::span!("hello world span").entered();
            let _debug = crate::span!(level: Level::DEBUG, "debug span").entered();
        });

        guard.shutdown().unwrap();

        let mut spans = exporter.get_finished_spans().unwrap();
        spans.sort_unstable_by(|a, b| a.name.cmp(&b.name));
        let spans = spans
            .iter()
            .map(|span| (span.name.as_ref(), span.get_span_type()))
            .collect::<Vec<_>>();

        assert_eq!(
            spans,
            vec![
                ("debug span", Some("span")),
                ("hello world span", Some("span")),
                ("root span", Some("span")),
            ]
        );
    }
}
