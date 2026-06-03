use opentelemetry::{KeyValue, SpanId, trace::SpanContext};
use opentelemetry_sdk::trace::{IdGenerator, SpanData, SpanProcessor};

use crate::{config::SharedIdGenerator, internal::span_data_ext::SpanDataExt};

/// Wrapper around a `SpanProcessor` which emits pending logfire spans on `on_start`.
#[derive(Debug)]
pub(crate) struct PendingSpanProcessor<T> {
    inner: T,
    id_generator: SharedIdGenerator,
}

impl<T> PendingSpanProcessor<T> {
    pub fn new(inner: T, id_generator: SharedIdGenerator) -> Self {
        Self {
            inner,
            id_generator,
        }
    }
}

impl<T: SpanProcessor> SpanProcessor for PendingSpanProcessor<T> {
    fn on_start(&self, span: &mut opentelemetry_sdk::trace::Span, cx: &opentelemetry::Context) {
        self.inner.on_start(span, cx);

        let Some(data) = span.exported_data() else {
            return;
        };

        // Only "real" spans should have pending spans emitted. `None` is handled transparently
        // in the backend a "real" span. (This avoids "log" type in particular).
        if data.get_span_type().is_some_and(|s| s != "span") {
            return;
        }

        let mut attributes = data.attributes;
        if let Some(attr) = attributes
            .iter_mut()
            .find(|kv| kv.key.as_str() == "logfire.span_type")
        {
            attr.value = "pending_span".into();
        } else {
            attributes.push(KeyValue::new("logfire.span_type", "pending_span"));
        }
        if data.parent_span_id != SpanId::INVALID {
            attributes.push(KeyValue::new(
                "logfire.pending_parent_id",
                data.parent_span_id.to_string(),
            ));
        }

        let pending = SpanData {
            span_context: SpanContext::new(
                data.span_context.trace_id(),
                self.id_generator.new_span_id(),
                data.span_context.trace_flags(),
                false,
                data.span_context.trace_state().clone(),
            ),
            // The pending span is exported as a child of the real span, so the real span's
            // span_id becomes the pending span's parent.
            parent_span_id: data.span_context.span_id(),
            parent_span_is_remote: false,
            span_kind: data.span_kind,
            name: data.name,
            start_time: data.start_time,
            end_time: data.start_time,
            attributes,
            dropped_attributes_count: data.dropped_attributes_count,
            events: data.events,
            links: data.links,
            status: data.status,
            instrumentation_scope: data.instrumentation_scope,
        };

        self.inner.on_end(pending);
    }

    fn on_end(&self, span: opentelemetry_sdk::trace::SpanData) {
        self.inner.on_end(span);
    }

    fn force_flush(&self) -> opentelemetry_sdk::error::OTelSdkResult {
        self.inner.force_flush()
    }

    fn shutdown_with_timeout(
        &self,
        timeout: std::time::Duration,
    ) -> opentelemetry_sdk::error::OTelSdkResult {
        self.inner.shutdown_with_timeout(timeout)
    }

    fn shutdown(&self) -> opentelemetry_sdk::error::OTelSdkResult {
        self.inner.shutdown()
    }

    fn set_resource(&mut self, resource: &opentelemetry_sdk::Resource) {
        self.inner.set_resource(resource);
    }
}
