use opentelemetry::{
    KeyValue,
    global::ObjectSafeSpan,
    trace::{SamplingDecision, TraceContextExt},
};
use tracing::Subscriber;
use tracing_opentelemetry::{OtelData, PreSampledTracer};
use tracing_subscriber::{Layer, registry::LookupSpan};

pub(crate) struct LogfireTracingLayer(pub(crate) opentelemetry_sdk::trace::Tracer);

impl<S> Layer<S> for LogfireTracingLayer
where
    S: Subscriber + for<'span> LookupSpan<'span>,
{
    fn on_new_span(
        &self,
        attrs: &tracing::span::Attributes<'_>,
        id: &tracing::span::Id,
        ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let span = ctx.span(id).expect("span not found");
        let mut extensions = span.extensions_mut();
        if let Some(otel_data) = extensions.get_mut::<OtelData>() {
            let attributes = otel_data
                .builder
                .attributes
                .get_or_insert_with(Default::default);

            attributes.push(opentelemetry::KeyValue::new(
                "logfire.level_num",
                level_to_level_number(*attrs.metadata().level()),
            ));
            attributes.push(KeyValue::new("logfire.span_type", "span"));

            // Emit a pending span, if this span will be sampled.
            let context = self.0.sampled_context(otel_data);
            let sampling_result = otel_data
                .builder
                .sampling_result
                .as_ref()
                .expect("we just asked for sampling to happen");

            // Deliberately match on all cases here so that if the enum changes in the future,
            // we can update this code to match the possible decisions.
            let should_emit_pending = match &sampling_result.decision {
                SamplingDecision::Drop | SamplingDecision::RecordOnly => false,
                SamplingDecision::RecordAndSample => true,
            };

            if should_emit_pending {
                let mut pending_span_builder = otel_data.builder.clone();

                // Pending span is sent as a child of the actual span with kind pending_span.
                // - The parent id is the actual span we want.
                // - The pending_parent_id is the parent of the pending span.

                let span_id = otel_data.builder.span_id.expect("otel SDK sets span ID");
                pending_span_builder.span_id = Some(span_id);

                let attributes = pending_span_builder
                    .attributes
                    .get_or_insert_with(Default::default);

                // update type of the pending span export
                if let Some(attr) = attributes
                    .iter_mut()
                    .find(|kv| kv.key.as_str() == "logfire.span_type")
                {
                    attr.value = "pending_span".into();
                } else {
                    attributes.push(opentelemetry::KeyValue::new(
                        "logfire.span_type",
                        "pending_span",
                    ));
                }

                // record the real parent ID
                let parent_span = otel_data.parent_cx.span();
                let parent_span_context = parent_span.span_context();

                if parent_span_context.is_valid() {
                    attributes.push(opentelemetry::KeyValue::new(
                        "logfire.pending_parent_id",
                        parent_span_context.span_id().to_string(),
                    ));
                }

                pending_span_builder.span_id = Some(self.0.new_span_id());

                let start_time = pending_span_builder
                    .start_time
                    .expect("otel SDK sets start time");

                // emit pending span
                let mut pending_span = pending_span_builder.start_with_context(&self.0, &context);
                pending_span.end_with_timestamp(start_time);
            }
        }
    }
}

pub(crate) fn level_to_level_number(level: tracing::Level) -> i64 {
    // These numbers were chosen to match the values emitted by the Python logfire SDK.
    match level {
        tracing::Level::TRACE => 1,
        tracing::Level::DEBUG => 5,
        tracing::Level::INFO => 9,
        tracing::Level::WARN => 13,
        tracing::Level::ERROR => 17,
    }
}
