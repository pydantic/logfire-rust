use std::time::SystemTime;

use opentelemetry::{
    KeyValue,
    global::ObjectSafeSpan,
    trace::{SamplingDecision, TraceContextExt, Tracer},
};
use tracing::{Subscriber, field::Visit};
use tracing_opentelemetry::{OpenTelemetrySpanExt, OtelData, PreSampledTracer};
use tracing_subscriber::{Layer, registry::LookupSpan};

use crate::{LogfireTracer, try_with_logfire_tracer};

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
        }
    }

    /// Emit a pending span when this span is first entered, if this span will be sampled.
    ///
    /// We do this on first enter, not on creation, because some SDKs set the parent span after
    /// creation.
    ///
    /// e.g. <https://github.com/davidB/tracing-opentelemetry-instrumentation-sdk/blob/5830c9113b0d42b72167567bf8e5f4c6b20933c8/axum-tracing-opentelemetry/src/middleware/trace_extractor.rs#L132>
    fn on_enter(&self, id: &tracing::span::Id, ctx: tracing_subscriber::layer::Context<'_, S>) {
        let span = ctx.span(id).expect("span not found");
        let mut extensions = span.extensions_mut();

        if extensions.get_mut::<LogfirePendingSpanSent>().is_some() {
            return;
        }

        extensions.insert(LogfirePendingSpanSent);

        // Guaranteed to be on first entering of the span
        if let Some(otel_data) = extensions.get_mut::<OtelData>() {
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

    /// Tracing events currently are recorded as span events, so do not get printed by the span emitter.
    ///
    /// Instead we need to handle them here and write them to the logfire writer.
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        try_with_logfire_tracer(|tracer| {
            // All events are emitted as log spans
            emit_event_as_log_span(tracer, event, &tracing::Span::current());
        });
    }
}

/// Helper to print spans when dropped; if it was never entered then the pending span
/// is never sent (the console writer uses pending spans).
///
/// This needs to be a separate layer so that it can access the `OtelData` before the
/// `tracing_opentelemetry` layer removes it.
pub struct LogfireTracingPendingSpanNotSentLayer;

impl<S> Layer<S> for LogfireTracingPendingSpanNotSentLayer
where
    S: Subscriber + for<'span> LookupSpan<'span>,
{
    fn on_close(&self, id: tracing::span::Id, ctx: tracing_subscriber::layer::Context<'_, S>) {
        let span = ctx.span(&id).expect("span not found");
        let mut extensions = span.extensions_mut();

        if extensions.get_mut::<LogfirePendingSpanSent>().is_some() {
            return;
        }

        // Guaranteed to be on first entering of the span
        if let Some(otel_data) = extensions.get_mut::<OtelData>() {
            try_with_logfire_tracer(|tracer| {
                if let Some(writer) = &tracer.console_writer {
                    writer.write_tracing_opentelemetry_data(otel_data);
                }
            });
        }
    }
}

/// Dummy struct to mark that we've already entered this span.
struct LogfirePendingSpanSent;

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

fn emit_event_as_log_span(
    tracer: &LogfireTracer,
    event: &tracing::Event<'_>,
    parent_span: &tracing::Span,
) {
    let mut visitor = FieldsVisitor {
        message: None,
        fields: Vec::new(),
    };

    event.record(&mut visitor);

    // using the message as the span name matches `tracing-opentelemetry` behavior in how
    // it sets the span event name
    let message = visitor
        .message
        .unwrap_or_else(|| event.metadata().name().to_owned());

    let attributes: Vec<_> = visitor
        .fields
        .into_iter()
        .map(|(name, value)| KeyValue::new(name, value))
        .chain([
            KeyValue::new("logfire.msg", message.clone()),
            KeyValue::new(
                "logfire.level_num",
                level_to_level_number(*event.metadata().level()),
            ),
            KeyValue::new("logfire.span_type", "log"),
        ])
        .chain(
            event
                .metadata()
                .file()
                .map(|file| KeyValue::new("code.filepath", file)),
        )
        .chain(
            event
                .metadata()
                .line()
                .map(|line| KeyValue::new("code.lineno", i64::from(line))),
        )
        .chain(
            event
                .metadata()
                .module_path()
                .map(|module_path| KeyValue::new("code.namespace", module_path)),
        )
        .collect();

    // FIXME add thread.id, thread.name

    let ts = SystemTime::now();

    tracer
        .inner
        .span_builder(message)
        .with_attributes(attributes)
        .with_start_time(ts)
        // .with_end_time(ts) seems to not be respected, need to explicitly end as per below
        .start_with_context(&tracer.inner, &parent_span.context())
        .end_with_timestamp(ts);
}

/// Internal helper to `visit` a `tracing::Event` and collect relevant fields.
struct FieldsVisitor {
    message: Option<String>,
    fields: Vec<(&'static str, String)>,
}

impl Visit for FieldsVisitor {
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "message" {
            self.message = Some(value.to_string());
        } else {
            self.fields.push((field.name(), value.to_string()));
        }
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.message = Some(format!("{value:?}"));
        } else {
            self.fields.push((field.name(), format!("{value:?}")));
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use insta::{assert_debug_snapshot, assert_snapshot};
    use opentelemetry_sdk::trace::{InMemorySpanExporterBuilder, SimpleSpanProcessor};
    use tracing::{Level, level_filters::LevelFilter};

    use crate::{
        config::{AdvancedOptions, ConsoleOptions, Target},
        set_local_logfire,
        test_utils::{
            DeterministicExporter, DeterministicIdGenerator, remap_timestamps_in_console_output,
        },
    };

    #[test]
    fn test_tracing_bridge() {
        let exporter = InMemorySpanExporterBuilder::new().build();

        let handler =
            crate::configure()
                .local()
                .send_to_logfire(false)
                .with_additional_span_processor(SimpleSpanProcessor::new(
                    DeterministicExporter::new(exporter.clone(), file!(), line!()),
                ))
                .install_panic_handler()
                .with_default_level_filter(LevelFilter::TRACE)
                .with_advanced_options(
                    AdvancedOptions::default().with_id_generator(DeterministicIdGenerator::new()),
                )
                .finish()
                .unwrap();

        let guard = set_local_logfire(handler);

        tracing::subscriber::with_default(guard.subscriber().clone(), || {
            tracing::info!("root event"); // FIXME: this event is not emitted
            tracing::info!(name: "root event with value", field_value = 1); // FIXME: this event is not emitted

            let root = tracing::span!(Level::INFO, "root span").entered();
            let _ = tracing::span!(Level::INFO, "hello world span").entered();
            let _ = tracing::span!(Level::DEBUG, "debug span");
            let _ = tracing::span!(parent: &root, Level::DEBUG, "debug span with explicit parent");

            tracing::info!("hello world log");
            tracing::info!(name: "hello world log with value", field_value = 1);
        });

        let spans = exporter.get_finished_spans().unwrap();
        assert_debug_snapshot!(spans, @r#"
        [
            SpanData {
                span_context: SpanContext {
                    trace_id: 000000000000000000000000000000f0,
                    span_id: 00000000000000f0,
                    trace_flags: TraceFlags(
                        1,
                    ),
                    is_remote: false,
                    trace_state: TraceState(
                        None,
                    ),
                },
                parent_span_id: 0000000000000000,
                span_kind: Internal,
                name: "root event",
                start_time: SystemTime {
                    tv_sec: 0,
                    tv_nsec: 0,
                },
                end_time: SystemTime {
                    tv_sec: 0,
                    tv_nsec: 0,
                },
                attributes: [
                    KeyValue {
                        key: Static(
                            "logfire.msg",
                        ),
                        value: String(
                            Owned(
                                "root event",
                            ),
                        ),
                    },
                    KeyValue {
                        key: Static(
                            "logfire.level_num",
                        ),
                        value: I64(
                            9,
                        ),
                    },
                    KeyValue {
                        key: Static(
                            "logfire.span_type",
                        ),
                        value: String(
                            Static(
                                "log",
                            ),
                        ),
                    },
                    KeyValue {
                        key: Static(
                            "code.filepath",
                        ),
                        value: String(
                            Static(
                                "src/bridges/tracing.rs",
                            ),
                        ),
                    },
                    KeyValue {
                        key: Static(
                            "code.lineno",
                        ),
                        value: I64(
                            13,
                        ),
                    },
                    KeyValue {
                        key: Static(
                            "code.namespace",
                        ),
                        value: String(
                            Static(
                                "logfire::bridges::tracing::tests",
                            ),
                        ),
                    },
                ],
                dropped_attributes_count: 0,
                events: SpanEvents {
                    events: [],
                    dropped_count: 0,
                },
                links: SpanLinks {
                    links: [],
                    dropped_count: 0,
                },
                status: Unset,
                instrumentation_scope: InstrumentationScope {
                    name: "logfire",
                    version: None,
                    schema_url: None,
                    attributes: [],
                },
            },
            SpanData {
                span_context: SpanContext {
                    trace_id: 000000000000000000000000000000f1,
                    span_id: 00000000000000f1,
                    trace_flags: TraceFlags(
                        1,
                    ),
                    is_remote: false,
                    trace_state: TraceState(
                        None,
                    ),
                },
                parent_span_id: 0000000000000000,
                span_kind: Internal,
                name: "root event with value",
                start_time: SystemTime {
                    tv_sec: 1,
                    tv_nsec: 0,
                },
                end_time: SystemTime {
                    tv_sec: 1,
                    tv_nsec: 0,
                },
                attributes: [
                    KeyValue {
                        key: Static(
                            "field_value",
                        ),
                        value: String(
                            Owned(
                                "1",
                            ),
                        ),
                    },
                    KeyValue {
                        key: Static(
                            "logfire.msg",
                        ),
                        value: String(
                            Owned(
                                "root event with value",
                            ),
                        ),
                    },
                    KeyValue {
                        key: Static(
                            "logfire.level_num",
                        ),
                        value: I64(
                            9,
                        ),
                    },
                    KeyValue {
                        key: Static(
                            "logfire.span_type",
                        ),
                        value: String(
                            Static(
                                "log",
                            ),
                        ),
                    },
                    KeyValue {
                        key: Static(
                            "code.filepath",
                        ),
                        value: String(
                            Static(
                                "src/bridges/tracing.rs",
                            ),
                        ),
                    },
                    KeyValue {
                        key: Static(
                            "code.lineno",
                        ),
                        value: I64(
                            14,
                        ),
                    },
                    KeyValue {
                        key: Static(
                            "code.namespace",
                        ),
                        value: String(
                            Static(
                                "logfire::bridges::tracing::tests",
                            ),
                        ),
                    },
                ],
                dropped_attributes_count: 0,
                events: SpanEvents {
                    events: [],
                    dropped_count: 0,
                },
                links: SpanLinks {
                    links: [],
                    dropped_count: 0,
                },
                status: Unset,
                instrumentation_scope: InstrumentationScope {
                    name: "logfire",
                    version: None,
                    schema_url: None,
                    attributes: [],
                },
            },
            SpanData {
                span_context: SpanContext {
                    trace_id: 000000000000000000000000000000f2,
                    span_id: 00000000000000f3,
                    trace_flags: TraceFlags(
                        1,
                    ),
                    is_remote: false,
                    trace_state: TraceState(
                        None,
                    ),
                },
                parent_span_id: 00000000000000f2,
                span_kind: Internal,
                name: "root span",
                start_time: SystemTime {
                    tv_sec: 2,
                    tv_nsec: 0,
                },
                end_time: SystemTime {
                    tv_sec: 2,
                    tv_nsec: 0,
                },
                attributes: [
                    KeyValue {
                        key: Static(
                            "code.filepath",
                        ),
                        value: String(
                            Static(
                                "src/bridges/tracing.rs",
                            ),
                        ),
                    },
                    KeyValue {
                        key: Static(
                            "code.namespace",
                        ),
                        value: String(
                            Static(
                                "logfire::bridges::tracing::tests",
                            ),
                        ),
                    },
                    KeyValue {
                        key: Static(
                            "code.lineno",
                        ),
                        value: I64(
                            16,
                        ),
                    },
                    KeyValue {
                        key: Static(
                            "thread.id",
                        ),
                        value: I64(
                            0,
                        ),
                    },
                    KeyValue {
                        key: Static(
                            "thread.name",
                        ),
                        value: String(
                            Owned(
                                "bridges::tracing::tests::test_tracing_bridge",
                            ),
                        ),
                    },
                    KeyValue {
                        key: Static(
                            "logfire.level_num",
                        ),
                        value: I64(
                            9,
                        ),
                    },
                    KeyValue {
                        key: Static(
                            "logfire.span_type",
                        ),
                        value: String(
                            Static(
                                "pending_span",
                            ),
                        ),
                    },
                ],
                dropped_attributes_count: 0,
                events: SpanEvents {
                    events: [],
                    dropped_count: 0,
                },
                links: SpanLinks {
                    links: [],
                    dropped_count: 0,
                },
                status: Unset,
                instrumentation_scope: InstrumentationScope {
                    name: "logfire",
                    version: None,
                    schema_url: None,
                    attributes: [],
                },
            },
            SpanData {
                span_context: SpanContext {
                    trace_id: 000000000000000000000000000000f2,
                    span_id: 00000000000000f5,
                    trace_flags: TraceFlags(
                        1,
                    ),
                    is_remote: false,
                    trace_state: TraceState(
                        None,
                    ),
                },
                parent_span_id: 00000000000000f4,
                span_kind: Internal,
                name: "hello world span",
                start_time: SystemTime {
                    tv_sec: 3,
                    tv_nsec: 0,
                },
                end_time: SystemTime {
                    tv_sec: 3,
                    tv_nsec: 0,
                },
                attributes: [
                    KeyValue {
                        key: Static(
                            "code.filepath",
                        ),
                        value: String(
                            Static(
                                "src/bridges/tracing.rs",
                            ),
                        ),
                    },
                    KeyValue {
                        key: Static(
                            "code.namespace",
                        ),
                        value: String(
                            Static(
                                "logfire::bridges::tracing::tests",
                            ),
                        ),
                    },
                    KeyValue {
                        key: Static(
                            "code.lineno",
                        ),
                        value: I64(
                            17,
                        ),
                    },
                    KeyValue {
                        key: Static(
                            "thread.id",
                        ),
                        value: I64(
                            0,
                        ),
                    },
                    KeyValue {
                        key: Static(
                            "thread.name",
                        ),
                        value: String(
                            Owned(
                                "bridges::tracing::tests::test_tracing_bridge",
                            ),
                        ),
                    },
                    KeyValue {
                        key: Static(
                            "logfire.level_num",
                        ),
                        value: I64(
                            9,
                        ),
                    },
                    KeyValue {
                        key: Static(
                            "logfire.span_type",
                        ),
                        value: String(
                            Static(
                                "pending_span",
                            ),
                        ),
                    },
                    KeyValue {
                        key: Static(
                            "logfire.pending_parent_id",
                        ),
                        value: String(
                            Owned(
                                "00000000000000f2",
                            ),
                        ),
                    },
                ],
                dropped_attributes_count: 0,
                events: SpanEvents {
                    events: [],
                    dropped_count: 0,
                },
                links: SpanLinks {
                    links: [],
                    dropped_count: 0,
                },
                status: Unset,
                instrumentation_scope: InstrumentationScope {
                    name: "logfire",
                    version: None,
                    schema_url: None,
                    attributes: [],
                },
            },
            SpanData {
                span_context: SpanContext {
                    trace_id: 000000000000000000000000000000f2,
                    span_id: 00000000000000f4,
                    trace_flags: TraceFlags(
                        1,
                    ),
                    is_remote: false,
                    trace_state: TraceState(
                        None,
                    ),
                },
                parent_span_id: 00000000000000f2,
                span_kind: Internal,
                name: "hello world span",
                start_time: SystemTime {
                    tv_sec: 3,
                    tv_nsec: 0,
                },
                end_time: SystemTime {
                    tv_sec: 4,
                    tv_nsec: 0,
                },
                attributes: [
                    KeyValue {
                        key: Static(
                            "code.filepath",
                        ),
                        value: String(
                            Static(
                                "src/bridges/tracing.rs",
                            ),
                        ),
                    },
                    KeyValue {
                        key: Static(
                            "code.namespace",
                        ),
                        value: String(
                            Static(
                                "logfire::bridges::tracing::tests",
                            ),
                        ),
                    },
                    KeyValue {
                        key: Static(
                            "code.lineno",
                        ),
                        value: I64(
                            17,
                        ),
                    },
                    KeyValue {
                        key: Static(
                            "thread.id",
                        ),
                        value: I64(
                            0,
                        ),
                    },
                    KeyValue {
                        key: Static(
                            "thread.name",
                        ),
                        value: String(
                            Owned(
                                "bridges::tracing::tests::test_tracing_bridge",
                            ),
                        ),
                    },
                    KeyValue {
                        key: Static(
                            "logfire.level_num",
                        ),
                        value: I64(
                            9,
                        ),
                    },
                    KeyValue {
                        key: Static(
                            "logfire.span_type",
                        ),
                        value: String(
                            Static(
                                "span",
                            ),
                        ),
                    },
                    KeyValue {
                        key: Static(
                            "busy_ns",
                        ),
                        value: I64(
                            0,
                        ),
                    },
                    KeyValue {
                        key: Static(
                            "idle_ns",
                        ),
                        value: I64(
                            0,
                        ),
                    },
                ],
                dropped_attributes_count: 0,
                events: SpanEvents {
                    events: [],
                    dropped_count: 0,
                },
                links: SpanLinks {
                    links: [],
                    dropped_count: 0,
                },
                status: Unset,
                instrumentation_scope: InstrumentationScope {
                    name: "logfire",
                    version: None,
                    schema_url: None,
                    attributes: [],
                },
            },
            SpanData {
                span_context: SpanContext {
                    trace_id: 000000000000000000000000000000f2,
                    span_id: 00000000000000f6,
                    trace_flags: TraceFlags(
                        1,
                    ),
                    is_remote: false,
                    trace_state: TraceState(
                        None,
                    ),
                },
                parent_span_id: 00000000000000f2,
                span_kind: Internal,
                name: "debug span",
                start_time: SystemTime {
                    tv_sec: 5,
                    tv_nsec: 0,
                },
                end_time: SystemTime {
                    tv_sec: 6,
                    tv_nsec: 0,
                },
                attributes: [
                    KeyValue {
                        key: Static(
                            "code.filepath",
                        ),
                        value: String(
                            Static(
                                "src/bridges/tracing.rs",
                            ),
                        ),
                    },
                    KeyValue {
                        key: Static(
                            "code.namespace",
                        ),
                        value: String(
                            Static(
                                "logfire::bridges::tracing::tests",
                            ),
                        ),
                    },
                    KeyValue {
                        key: Static(
                            "code.lineno",
                        ),
                        value: I64(
                            18,
                        ),
                    },
                    KeyValue {
                        key: Static(
                            "thread.id",
                        ),
                        value: I64(
                            0,
                        ),
                    },
                    KeyValue {
                        key: Static(
                            "thread.name",
                        ),
                        value: String(
                            Owned(
                                "bridges::tracing::tests::test_tracing_bridge",
                            ),
                        ),
                    },
                    KeyValue {
                        key: Static(
                            "logfire.level_num",
                        ),
                        value: I64(
                            5,
                        ),
                    },
                    KeyValue {
                        key: Static(
                            "logfire.span_type",
                        ),
                        value: String(
                            Static(
                                "span",
                            ),
                        ),
                    },
                    KeyValue {
                        key: Static(
                            "busy_ns",
                        ),
                        value: I64(
                            0,
                        ),
                    },
                    KeyValue {
                        key: Static(
                            "idle_ns",
                        ),
                        value: I64(
                            0,
                        ),
                    },
                ],
                dropped_attributes_count: 0,
                events: SpanEvents {
                    events: [],
                    dropped_count: 0,
                },
                links: SpanLinks {
                    links: [],
                    dropped_count: 0,
                },
                status: Unset,
                instrumentation_scope: InstrumentationScope {
                    name: "logfire",
                    version: None,
                    schema_url: None,
                    attributes: [],
                },
            },
            SpanData {
                span_context: SpanContext {
                    trace_id: 000000000000000000000000000000f2,
                    span_id: 00000000000000f7,
                    trace_flags: TraceFlags(
                        1,
                    ),
                    is_remote: false,
                    trace_state: TraceState(
                        None,
                    ),
                },
                parent_span_id: 00000000000000f2,
                span_kind: Internal,
                name: "debug span with explicit parent",
                start_time: SystemTime {
                    tv_sec: 7,
                    tv_nsec: 0,
                },
                end_time: SystemTime {
                    tv_sec: 8,
                    tv_nsec: 0,
                },
                attributes: [
                    KeyValue {
                        key: Static(
                            "code.filepath",
                        ),
                        value: String(
                            Static(
                                "src/bridges/tracing.rs",
                            ),
                        ),
                    },
                    KeyValue {
                        key: Static(
                            "code.namespace",
                        ),
                        value: String(
                            Static(
                                "logfire::bridges::tracing::tests",
                            ),
                        ),
                    },
                    KeyValue {
                        key: Static(
                            "code.lineno",
                        ),
                        value: I64(
                            19,
                        ),
                    },
                    KeyValue {
                        key: Static(
                            "thread.id",
                        ),
                        value: I64(
                            0,
                        ),
                    },
                    KeyValue {
                        key: Static(
                            "thread.name",
                        ),
                        value: String(
                            Owned(
                                "bridges::tracing::tests::test_tracing_bridge",
                            ),
                        ),
                    },
                    KeyValue {
                        key: Static(
                            "logfire.level_num",
                        ),
                        value: I64(
                            5,
                        ),
                    },
                    KeyValue {
                        key: Static(
                            "logfire.span_type",
                        ),
                        value: String(
                            Static(
                                "span",
                            ),
                        ),
                    },
                    KeyValue {
                        key: Static(
                            "busy_ns",
                        ),
                        value: I64(
                            0,
                        ),
                    },
                    KeyValue {
                        key: Static(
                            "idle_ns",
                        ),
                        value: I64(
                            0,
                        ),
                    },
                ],
                dropped_attributes_count: 0,
                events: SpanEvents {
                    events: [],
                    dropped_count: 0,
                },
                links: SpanLinks {
                    links: [],
                    dropped_count: 0,
                },
                status: Unset,
                instrumentation_scope: InstrumentationScope {
                    name: "logfire",
                    version: None,
                    schema_url: None,
                    attributes: [],
                },
            },
            SpanData {
                span_context: SpanContext {
                    trace_id: 000000000000000000000000000000f3,
                    span_id: 00000000000000f8,
                    trace_flags: TraceFlags(
                        1,
                    ),
                    is_remote: false,
                    trace_state: TraceState(
                        None,
                    ),
                },
                parent_span_id: 0000000000000000,
                span_kind: Internal,
                name: "hello world log",
                start_time: SystemTime {
                    tv_sec: 9,
                    tv_nsec: 0,
                },
                end_time: SystemTime {
                    tv_sec: 9,
                    tv_nsec: 0,
                },
                attributes: [
                    KeyValue {
                        key: Static(
                            "logfire.msg",
                        ),
                        value: String(
                            Owned(
                                "hello world log",
                            ),
                        ),
                    },
                    KeyValue {
                        key: Static(
                            "logfire.level_num",
                        ),
                        value: I64(
                            9,
                        ),
                    },
                    KeyValue {
                        key: Static(
                            "logfire.span_type",
                        ),
                        value: String(
                            Static(
                                "log",
                            ),
                        ),
                    },
                    KeyValue {
                        key: Static(
                            "code.filepath",
                        ),
                        value: String(
                            Static(
                                "src/bridges/tracing.rs",
                            ),
                        ),
                    },
                    KeyValue {
                        key: Static(
                            "code.lineno",
                        ),
                        value: I64(
                            21,
                        ),
                    },
                    KeyValue {
                        key: Static(
                            "code.namespace",
                        ),
                        value: String(
                            Static(
                                "logfire::bridges::tracing::tests",
                            ),
                        ),
                    },
                ],
                dropped_attributes_count: 0,
                events: SpanEvents {
                    events: [],
                    dropped_count: 0,
                },
                links: SpanLinks {
                    links: [],
                    dropped_count: 0,
                },
                status: Unset,
                instrumentation_scope: InstrumentationScope {
                    name: "logfire",
                    version: None,
                    schema_url: None,
                    attributes: [],
                },
            },
            SpanData {
                span_context: SpanContext {
                    trace_id: 000000000000000000000000000000f4,
                    span_id: 00000000000000f9,
                    trace_flags: TraceFlags(
                        1,
                    ),
                    is_remote: false,
                    trace_state: TraceState(
                        None,
                    ),
                },
                parent_span_id: 0000000000000000,
                span_kind: Internal,
                name: "hello world log with value",
                start_time: SystemTime {
                    tv_sec: 10,
                    tv_nsec: 0,
                },
                end_time: SystemTime {
                    tv_sec: 10,
                    tv_nsec: 0,
                },
                attributes: [
                    KeyValue {
                        key: Static(
                            "field_value",
                        ),
                        value: String(
                            Owned(
                                "1",
                            ),
                        ),
                    },
                    KeyValue {
                        key: Static(
                            "logfire.msg",
                        ),
                        value: String(
                            Owned(
                                "hello world log with value",
                            ),
                        ),
                    },
                    KeyValue {
                        key: Static(
                            "logfire.level_num",
                        ),
                        value: I64(
                            9,
                        ),
                    },
                    KeyValue {
                        key: Static(
                            "logfire.span_type",
                        ),
                        value: String(
                            Static(
                                "log",
                            ),
                        ),
                    },
                    KeyValue {
                        key: Static(
                            "code.filepath",
                        ),
                        value: String(
                            Static(
                                "src/bridges/tracing.rs",
                            ),
                        ),
                    },
                    KeyValue {
                        key: Static(
                            "code.lineno",
                        ),
                        value: I64(
                            22,
                        ),
                    },
                    KeyValue {
                        key: Static(
                            "code.namespace",
                        ),
                        value: String(
                            Static(
                                "logfire::bridges::tracing::tests",
                            ),
                        ),
                    },
                ],
                dropped_attributes_count: 0,
                events: SpanEvents {
                    events: [],
                    dropped_count: 0,
                },
                links: SpanLinks {
                    links: [],
                    dropped_count: 0,
                },
                status: Unset,
                instrumentation_scope: InstrumentationScope {
                    name: "logfire",
                    version: None,
                    schema_url: None,
                    attributes: [],
                },
            },
            SpanData {
                span_context: SpanContext {
                    trace_id: 000000000000000000000000000000f2,
                    span_id: 00000000000000f2,
                    trace_flags: TraceFlags(
                        1,
                    ),
                    is_remote: false,
                    trace_state: TraceState(
                        None,
                    ),
                },
                parent_span_id: 0000000000000000,
                span_kind: Internal,
                name: "root span",
                start_time: SystemTime {
                    tv_sec: 2,
                    tv_nsec: 0,
                },
                end_time: SystemTime {
                    tv_sec: 11,
                    tv_nsec: 0,
                },
                attributes: [
                    KeyValue {
                        key: Static(
                            "code.filepath",
                        ),
                        value: String(
                            Static(
                                "src/bridges/tracing.rs",
                            ),
                        ),
                    },
                    KeyValue {
                        key: Static(
                            "code.namespace",
                        ),
                        value: String(
                            Static(
                                "logfire::bridges::tracing::tests",
                            ),
                        ),
                    },
                    KeyValue {
                        key: Static(
                            "code.lineno",
                        ),
                        value: I64(
                            16,
                        ),
                    },
                    KeyValue {
                        key: Static(
                            "thread.id",
                        ),
                        value: I64(
                            0,
                        ),
                    },
                    KeyValue {
                        key: Static(
                            "thread.name",
                        ),
                        value: String(
                            Owned(
                                "bridges::tracing::tests::test_tracing_bridge",
                            ),
                        ),
                    },
                    KeyValue {
                        key: Static(
                            "logfire.level_num",
                        ),
                        value: I64(
                            9,
                        ),
                    },
                    KeyValue {
                        key: Static(
                            "logfire.span_type",
                        ),
                        value: String(
                            Static(
                                "span",
                            ),
                        ),
                    },
                    KeyValue {
                        key: Static(
                            "busy_ns",
                        ),
                        value: I64(
                            0,
                        ),
                    },
                    KeyValue {
                        key: Static(
                            "idle_ns",
                        ),
                        value: I64(
                            0,
                        ),
                    },
                ],
                dropped_attributes_count: 0,
                events: SpanEvents {
                    events: [],
                    dropped_count: 0,
                },
                links: SpanLinks {
                    links: [],
                    dropped_count: 0,
                },
                status: Unset,
                instrumentation_scope: InstrumentationScope {
                    name: "logfire",
                    version: None,
                    schema_url: None,
                    attributes: [],
                },
            },
        ]
        "#);
    }

    #[test]
    fn test_tracing_bridge_console_output() {
        let output = Arc::new(Mutex::new(Vec::new()));

        let console_options = ConsoleOptions {
            target: Target::Pipe(output.clone()),
            ..ConsoleOptions::default().with_min_log_level(Level::TRACE)
        };

        let handler = crate::configure()
            .local()
            .send_to_logfire(false)
            .with_console(Some(console_options.clone()))
            .install_panic_handler()
            .with_default_level_filter(LevelFilter::TRACE)
            .finish()
            .unwrap();

        let guard = crate::set_local_logfire(handler);

        tracing::subscriber::with_default(guard.subscriber().clone(), || {
            tracing::info!("root event");
            tracing::info!(name: "root event with value", field_value = 1);

            let root = tracing::span!(Level::INFO, "root span").entered();
            let _ = tracing::span!(Level::INFO, "hello world span").entered();
            let _ = tracing::span!(Level::DEBUG, "debug span");
            let _ = tracing::span!(parent: &root, Level::DEBUG, "debug span with explicit parent");

            tracing::info!("hello world log");
            tracing::info!(name: "hello world log with value", field_value = 1);
        });

        guard.shutdown_handler.shutdown().unwrap();

        let output = output.lock().unwrap();
        let output = std::str::from_utf8(&output).unwrap();
        let output = remap_timestamps_in_console_output(output);

        assert_snapshot!(output, @r#"
        [2m1970-01-01T00:00:00.000000Z[0m[32m  INFO[0m [2;3mlogfire::bridges::tracing::tests[0m [1mroot event[0m
        [2m1970-01-01T00:00:00.000001Z[0m[32m  INFO[0m [2;3mlogfire::bridges::tracing::tests[0m [1mroot event with value[0m [3mfield_value[0m=1
        [2m1970-01-01T00:00:00.000002Z[0m[32m  INFO[0m [2;3mlogfire::bridges::tracing::tests[0m [1mroot span[0m
        [2m1970-01-01T00:00:00.000003Z[0m[32m  INFO[0m [2;3mlogfire::bridges::tracing::tests[0m [1mhello world span[0m
        [2m1970-01-01T00:00:00.000004Z[0m[34m DEBUG[0m [2;3mlogfire::bridges::tracing::tests[0m [1mdebug span[0m
        [2m1970-01-01T00:00:00.000005Z[0m[34m DEBUG[0m [2;3mlogfire::bridges::tracing::tests[0m [1mdebug span with explicit parent[0m
        [2m1970-01-01T00:00:00.000006Z[0m[32m  INFO[0m [2;3mlogfire::bridges::tracing::tests[0m [1mhello world log[0m
        [2m1970-01-01T00:00:00.000007Z[0m[32m  INFO[0m [2;3mlogfire::bridges::tracing::tests[0m [1mhello world log with value[0m [3mfield_value[0m=1
        "#);
    }
}
