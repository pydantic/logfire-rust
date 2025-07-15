use std::{any::TypeId, borrow::Cow};

use opentelemetry::{
    Context, KeyValue,
    global::ObjectSafeSpan,
    logs::Severity,
    trace::{SamplingDecision, TraceContextExt},
};
use tracing::{Subscriber, field::Visit};
use tracing_opentelemetry::{OtelData, PreSampledTracer};
use tracing_subscriber::{Layer, registry::LookupSpan};

use crate::{__macros_impl::LogfireValue, LogfireTracer};

/// A `tracing` layer that bridges `tracing` spans to OpenTelemetry spans using the Pydantic Logfire tracer.
/// Pydantic Logfire-specific metadata.
///
/// See [`ShutdownHandler::tracing_layer`][crate::ShutdownHandler::tracing_layer] for how to use
/// this layer.
pub struct LogfireTracingLayer<S> {
    tracer: LogfireTracer,
    /// This odd structure with two inner layers is deliberate; we don't want to send any events
    /// to the `otel_layer` and we only send (some) events to the `metrics_layer`.
    otel_layer: tracing_opentelemetry::OpenTelemetryLayer<S, opentelemetry_sdk::trace::Tracer>,
    metrics_layer: Option<tracing_opentelemetry::MetricsLayer<S>>,
}

impl<S> LogfireTracingLayer<S>
where
    S: Subscriber + for<'span> LookupSpan<'span>,
{
    /// Create a new `LogfireTracingLayer` with the given tracer.
    pub(crate) fn new(tracer: LogfireTracer, enable_tracing_metrics: bool) -> Self {
        let otel_layer = tracing_opentelemetry::layer()
            .with_error_records_to_exceptions(true)
            .with_tracer(tracer.inner.clone());

        let metrics_layer = enable_tracing_metrics
            .then(|| tracing_opentelemetry::MetricsLayer::new(tracer.meter_provider.clone()));

        LogfireTracingLayer {
            tracer,
            otel_layer,
            metrics_layer,
        }
    }
}

impl<S> Layer<S> for LogfireTracingLayer<S>
where
    S: Subscriber + for<'span> LookupSpan<'span>,
{
    fn on_new_span(
        &self,
        attrs: &tracing::span::Attributes<'_>,
        id: &tracing::span::Id,
        ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        // Delegate to OpenTelemetry layer first
        self.otel_layer.on_new_span(attrs, id, ctx.clone());

        // Delegate to MetricsLayer as well
        self.metrics_layer.on_new_span(attrs, id, ctx.clone());

        // Add Logfire-specific attributes
        let span = ctx.span(id).expect("span not found");
        let mut extensions = span.extensions_mut();
        if let Some(otel_data) = extensions.get_mut::<OtelData>() {
            let attributes = otel_data
                .builder
                .attributes
                .get_or_insert_with(Default::default);

            attributes.push(opentelemetry::KeyValue::new(
                "logfire.level_num",
                tracing_level_to_severity(*attrs.metadata().level()) as i64,
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
        // Delegate to OpenTelemetry layer first
        self.otel_layer.on_enter(id, ctx.clone());

        let span = ctx.span(id).expect("span not found");
        let mut extensions = span.extensions_mut();

        if extensions.get_mut::<LogfirePendingSpanSent>().is_some() {
            return;
        }

        extensions.insert(LogfirePendingSpanSent);

        // Guaranteed to be on first entering of the span
        if let Some(otel_data) = extensions.get_mut::<OtelData>() {
            emit_pending_span(&self.tracer, otel_data);
        }
    }

    /// Tracing events currently are recorded as span events, so do not get printed by the span emitter.
    ///
    /// Instead we need to handle them here and write them to the logfire writer.
    fn on_event(&self, event: &tracing::Event<'_>, ctx: tracing_subscriber::layer::Context<'_, S>) {
        let is_metrics_event = self.metrics_layer.is_some()
            && event.fields().any(|field| {
                let name = field.name();

                name.starts_with("counter.")
                    || name.starts_with("monotonic_counter.")
                    || name.starts_with("histogram.")
                    || name.starts_with("monotonic_histogram.")
            });

        // Allow the metrics layer to see all events, so it can record metrics as needed.
        if is_metrics_event {
            self.metrics_layer.on_event(event, ctx.clone());
        }

        // However we don't want to allow the opentelemetry layer to see events, it will record them
        // as span events. Instead we handle them here and emit them as log spans.
        let event_span = ctx.event_span(event).and_then(|span| ctx.span(&span.id()));
        let mut event_span_extensions = event_span.as_ref().map(|s| s.extensions_mut());

        let context = if let Some(otel_data) = event_span_extensions
            .as_mut()
            .and_then(|e| e.get_mut::<OtelData>())
        {
            self.tracer.inner.sampled_context(otel_data)
        } else {
            Context::new()
        };

        emit_event_as_log_span(&self.tracer, event, &context);
    }

    fn on_exit(&self, id: &tracing::span::Id, ctx: tracing_subscriber::layer::Context<'_, S>) {
        self.otel_layer.on_exit(id, ctx);
    }

    fn on_close(&self, id: tracing::span::Id, ctx: tracing_subscriber::layer::Context<'_, S>) {
        let span = ctx.span(&id).expect("span not found");
        let mut extensions = span.extensions_mut();

        // We write pending spans to the console; if the pending span was never created then
        // we have to manually write it now.
        if extensions.get_mut::<LogfirePendingSpanSent>().is_none() {
            if let Some(otel_data) = extensions.get_mut::<OtelData>() {
                // emit pending span now just before it is closed, assume the processor will
                // deduplicate as needed
                emit_pending_span(&self.tracer, otel_data);
            }
        }

        // Delegate to OpenTelemetry layer after handling pending span (it will remove the
        // `OtelData` so cannot do before).
        drop(extensions);
        drop(span);
        self.otel_layer.on_close(id, ctx);
    }

    fn on_follows_from(
        &self,
        span: &tracing::span::Id,
        follows: &tracing::span::Id,
        ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        self.otel_layer.on_follows_from(span, follows, ctx);
    }

    fn on_record(
        &self,
        span: &tracing::span::Id,
        values: &tracing::span::Record<'_>,
        ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        self.otel_layer.on_record(span, values, ctx);
    }

    unsafe fn downcast_raw(&self, id: TypeId) -> Option<*const ()> {
        if id == TypeId::of::<Self>() {
            Some(std::ptr::from_ref(self).cast())
        } else {
            unsafe { self.otel_layer.downcast_raw(id) }
        }
    }

    fn on_register_dispatch(&self, subscriber: &tracing::Dispatch) {
        self.otel_layer.on_register_dispatch(subscriber);
        self.metrics_layer.on_register_dispatch(subscriber);
    }

    fn on_layer(&mut self, subscriber: &mut S) {
        self.otel_layer.on_layer(subscriber);
        self.metrics_layer.on_layer(subscriber);
    }

    fn enabled(
        &self,
        metadata: &tracing::Metadata<'_>,
        ctx: tracing_subscriber::layer::Context<'_, S>,
    ) -> bool {
        self.otel_layer.enabled(metadata, ctx.clone()) || self.metrics_layer.enabled(metadata, ctx)
    }

    fn event_enabled(
        &self,
        event: &tracing::Event<'_>,
        ctx: tracing_subscriber::layer::Context<'_, S>,
    ) -> bool {
        self.otel_layer.event_enabled(event, ctx.clone())
            || self.metrics_layer.event_enabled(event, ctx)
    }

    fn on_id_change(
        &self,
        old: &tracing::span::Id,
        new: &tracing::span::Id,
        ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        self.otel_layer.on_id_change(old, new, ctx.clone());
        self.metrics_layer.on_id_change(old, new, ctx);
    }
}

/// Dummy struct to mark that we've already entered this span.
struct LogfirePendingSpanSent;

/// Samples and emits a pending span if the span is sampled.
fn emit_pending_span(tracer: &LogfireTracer, otel_data: &mut tracing_opentelemetry::OtelData) {
    let context = tracer.inner.sampled_context(otel_data);
    let sampling_result = otel_data
        .builder
        .sampling_result
        .as_ref()
        .expect("we just asked for sampling to happen");

    // Deliberately match on all cases here so that if the enum changes in the future,
    // we can update this code to match the possible decisions.
    let span_is_sampled = match &sampling_result.decision {
        SamplingDecision::Drop | SamplingDecision::RecordOnly => false,
        SamplingDecision::RecordAndSample => true,
    };

    if !span_is_sampled {
        return;
    }

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

    pending_span_builder.span_id = Some(tracer.inner.new_span_id());

    let start_time = pending_span_builder
        .start_time
        .expect("otel SDK sets start time");

    // emit pending span
    let mut pending_span = pending_span_builder.start_with_context(&tracer.inner, &context);
    pending_span.end_with_timestamp(start_time);
}

pub(crate) fn tracing_level_to_severity(level: tracing::Level) -> Severity {
    match level {
        tracing::Level::ERROR => Severity::Error,
        tracing::Level::WARN => Severity::Warn,
        tracing::Level::INFO => Severity::Info,
        tracing::Level::DEBUG => Severity::Debug,
        tracing::Level::TRACE => Severity::Trace,
    }
}

fn emit_event_as_log_span(
    tracer: &LogfireTracer,
    event: &tracing::Event<'_>,
    parent_context: &Context,
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
        .map(|(name, value)| {
            LogfireValue::new(name, Some(opentelemetry::Value::String(value.into())))
        })
        .collect();

    tracer.export_log(
        event.metadata().name(),
        parent_context,
        message,
        tracing_level_to_severity(*event.metadata().level()),
        "{}",
        event.metadata().file().map(Cow::Borrowed),
        event.metadata().line(),
        event.metadata().module_path().map(Cow::Borrowed),
        attributes,
    );
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
    use opentelemetry_sdk::{
        logs::{InMemoryLogExporter, SimpleLogProcessor},
        trace::{InMemorySpanExporterBuilder, SimpleSpanProcessor},
    };
    use tracing::{Level, level_filters::LevelFilter};

    use crate::{
        config::{AdvancedOptions, ConsoleOptions, Target},
        set_local_logfire,
        test_utils::{
            DeterministicExporter, DeterministicIdGenerator, make_deterministic_logs,
            remap_timestamps_in_console_output,
        },
    };

    #[test]
    fn test_tracing_bridge() {
        const TEST_FILE: &str = file!();
        const TEST_LINE: u32 = line!();

        let exporter = InMemorySpanExporterBuilder::new().build();
        let log_exporter = InMemoryLogExporter::default();

        let handler =
            crate::configure()
                .local()
                .send_to_logfire(false)
                .with_additional_span_processor(SimpleSpanProcessor::new(
                    DeterministicExporter::new(exporter.clone(), TEST_FILE, TEST_LINE),
                ))
                .install_panic_handler()
                .with_default_level_filter(LevelFilter::TRACE)
                .with_advanced_options(
                    AdvancedOptions::default()
                        .with_id_generator(DeterministicIdGenerator::new())
                        .with_log_processor(SimpleLogProcessor::new(log_exporter.clone())),
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
                    span_id: 00000000000000f1,
                    trace_flags: TraceFlags(
                        1,
                    ),
                    is_remote: false,
                    trace_state: TraceState(
                        None,
                    ),
                },
                parent_span_id: 00000000000000f0,
                span_kind: Internal,
                name: "root span",
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
                            28,
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
                    trace_id: 000000000000000000000000000000f0,
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
                name: "hello world span",
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
                            29,
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
                                "00000000000000f0",
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
                    trace_id: 000000000000000000000000000000f0,
                    span_id: 00000000000000f2,
                    trace_flags: TraceFlags(
                        1,
                    ),
                    is_remote: false,
                    trace_state: TraceState(
                        None,
                    ),
                },
                parent_span_id: 00000000000000f0,
                span_kind: Internal,
                name: "hello world span",
                start_time: SystemTime {
                    tv_sec: 1,
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
                            29,
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
                    trace_id: 000000000000000000000000000000f0,
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
                name: "debug span",
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
                            30,
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
                                "00000000000000f0",
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
                    trace_id: 000000000000000000000000000000f0,
                    span_id: 00000000000000f4,
                    trace_flags: TraceFlags(
                        1,
                    ),
                    is_remote: false,
                    trace_state: TraceState(
                        None,
                    ),
                },
                parent_span_id: 00000000000000f0,
                span_kind: Internal,
                name: "debug span",
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
                            30,
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
                    trace_id: 000000000000000000000000000000f0,
                    span_id: 00000000000000f7,
                    trace_flags: TraceFlags(
                        1,
                    ),
                    is_remote: false,
                    trace_state: TraceState(
                        None,
                    ),
                },
                parent_span_id: 00000000000000f6,
                span_kind: Internal,
                name: "debug span with explicit parent",
                start_time: SystemTime {
                    tv_sec: 5,
                    tv_nsec: 0,
                },
                end_time: SystemTime {
                    tv_sec: 5,
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
                            31,
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
                                "00000000000000f0",
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
                    trace_id: 000000000000000000000000000000f0,
                    span_id: 00000000000000f6,
                    trace_flags: TraceFlags(
                        1,
                    ),
                    is_remote: false,
                    trace_state: TraceState(
                        None,
                    ),
                },
                parent_span_id: 00000000000000f0,
                span_kind: Internal,
                name: "debug span with explicit parent",
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
                            31,
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
                name: "root span",
                start_time: SystemTime {
                    tv_sec: 0,
                    tv_nsec: 0,
                },
                end_time: SystemTime {
                    tv_sec: 7,
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
                            28,
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

        let logs = log_exporter.get_emitted_logs().unwrap();
        let logs = make_deterministic_logs(logs, TEST_FILE, TEST_LINE);
        assert_debug_snapshot!(logs, @r#"
        [
            LogDataWithResource {
                record: SdkLogRecord {
                    event_name: None,
                    target: None,
                    timestamp: Some(
                        SystemTime {
                            tv_sec: 0,
                            tv_nsec: 0,
                        },
                    ),
                    observed_timestamp: Some(
                        SystemTime {
                            tv_sec: 0,
                            tv_nsec: 0,
                        },
                    ),
                    trace_context: Some(
                        TraceContext {
                            trace_id: 00000000000000000000000000000000,
                            span_id: 0000000000000000,
                            trace_flags: Some(
                                TraceFlags(
                                    0,
                                ),
                            ),
                        },
                    ),
                    severity_text: Some(
                        "INFO",
                    ),
                    severity_number: Some(
                        Info,
                    ),
                    body: Some(
                        String(
                            Owned(
                                "root event",
                            ),
                        ),
                    ),
                    attributes: GrowableArray {
                        inline: [
                            Some(
                                (
                                    Static(
                                        "code.filepath",
                                    ),
                                    String(
                                        Static(
                                            "src/bridges/tracing.rs",
                                        ),
                                    ),
                                ),
                            ),
                            Some(
                                (
                                    Static(
                                        "code.lineno",
                                    ),
                                    Int(
                                        25,
                                    ),
                                ),
                            ),
                            Some(
                                (
                                    Static(
                                        "code.namespace",
                                    ),
                                    String(
                                        Static(
                                            "logfire::bridges::tracing::tests",
                                        ),
                                    ),
                                ),
                            ),
                            Some(
                                (
                                    Static(
                                        "logfire.json_schema",
                                    ),
                                    String(
                                        Static(
                                            "{}",
                                        ),
                                    ),
                                ),
                            ),
                            Some(
                                (
                                    Static(
                                        "logfire.msg",
                                    ),
                                    String(
                                        Owned(
                                            "root event",
                                        ),
                                    ),
                                ),
                            ),
                        ],
                        overflow: Some(
                            [
                                Some(
                                    (
                                        Static(
                                            "thread.id",
                                        ),
                                        Int(
                                            0,
                                        ),
                                    ),
                                ),
                                Some(
                                    (
                                        Static(
                                            "thread.name",
                                        ),
                                        String(
                                            Owned(
                                                "bridges::tracing::tests::test_tracing_bridge",
                                            ),
                                        ),
                                    ),
                                ),
                            ],
                        ),
                        count: 5,
                    },
                },
                instrumentation: InstrumentationScope {
                    name: "logfire",
                    version: None,
                    schema_url: None,
                    attributes: [],
                },
                resource: Resource {
                    inner: ResourceInner {
                        attrs: {},
                        schema_url: None,
                    },
                },
            },
            LogDataWithResource {
                record: SdkLogRecord {
                    event_name: None,
                    target: None,
                    timestamp: Some(
                        SystemTime {
                            tv_sec: 1,
                            tv_nsec: 0,
                        },
                    ),
                    observed_timestamp: Some(
                        SystemTime {
                            tv_sec: 1,
                            tv_nsec: 0,
                        },
                    ),
                    trace_context: Some(
                        TraceContext {
                            trace_id: 00000000000000000000000000000000,
                            span_id: 0000000000000000,
                            trace_flags: Some(
                                TraceFlags(
                                    0,
                                ),
                            ),
                        },
                    ),
                    severity_text: Some(
                        "INFO",
                    ),
                    severity_number: Some(
                        Info,
                    ),
                    body: Some(
                        String(
                            Owned(
                                "root event with value",
                            ),
                        ),
                    ),
                    attributes: GrowableArray {
                        inline: [
                            Some(
                                (
                                    Static(
                                        "code.filepath",
                                    ),
                                    String(
                                        Static(
                                            "src/bridges/tracing.rs",
                                        ),
                                    ),
                                ),
                            ),
                            Some(
                                (
                                    Static(
                                        "code.lineno",
                                    ),
                                    Int(
                                        26,
                                    ),
                                ),
                            ),
                            Some(
                                (
                                    Static(
                                        "code.namespace",
                                    ),
                                    String(
                                        Static(
                                            "logfire::bridges::tracing::tests",
                                        ),
                                    ),
                                ),
                            ),
                            Some(
                                (
                                    Static(
                                        "field_value",
                                    ),
                                    String(
                                        Owned(
                                            "1",
                                        ),
                                    ),
                                ),
                            ),
                            Some(
                                (
                                    Static(
                                        "logfire.json_schema",
                                    ),
                                    String(
                                        Static(
                                            "{}",
                                        ),
                                    ),
                                ),
                            ),
                        ],
                        overflow: Some(
                            [
                                Some(
                                    (
                                        Static(
                                            "logfire.msg",
                                        ),
                                        String(
                                            Owned(
                                                "root event with value",
                                            ),
                                        ),
                                    ),
                                ),
                                Some(
                                    (
                                        Static(
                                            "thread.id",
                                        ),
                                        Int(
                                            0,
                                        ),
                                    ),
                                ),
                                Some(
                                    (
                                        Static(
                                            "thread.name",
                                        ),
                                        String(
                                            Owned(
                                                "bridges::tracing::tests::test_tracing_bridge",
                                            ),
                                        ),
                                    ),
                                ),
                            ],
                        ),
                        count: 5,
                    },
                },
                instrumentation: InstrumentationScope {
                    name: "logfire",
                    version: None,
                    schema_url: None,
                    attributes: [],
                },
                resource: Resource {
                    inner: ResourceInner {
                        attrs: {},
                        schema_url: None,
                    },
                },
            },
            LogDataWithResource {
                record: SdkLogRecord {
                    event_name: None,
                    target: None,
                    timestamp: Some(
                        SystemTime {
                            tv_sec: 2,
                            tv_nsec: 0,
                        },
                    ),
                    observed_timestamp: Some(
                        SystemTime {
                            tv_sec: 2,
                            tv_nsec: 0,
                        },
                    ),
                    trace_context: Some(
                        TraceContext {
                            trace_id: 000000000000000000000000000000f0,
                            span_id: 00000000000000f0,
                            trace_flags: Some(
                                TraceFlags(
                                    1,
                                ),
                            ),
                        },
                    ),
                    severity_text: Some(
                        "INFO",
                    ),
                    severity_number: Some(
                        Info,
                    ),
                    body: Some(
                        String(
                            Owned(
                                "hello world log",
                            ),
                        ),
                    ),
                    attributes: GrowableArray {
                        inline: [
                            Some(
                                (
                                    Static(
                                        "code.filepath",
                                    ),
                                    String(
                                        Static(
                                            "src/bridges/tracing.rs",
                                        ),
                                    ),
                                ),
                            ),
                            Some(
                                (
                                    Static(
                                        "code.lineno",
                                    ),
                                    Int(
                                        33,
                                    ),
                                ),
                            ),
                            Some(
                                (
                                    Static(
                                        "code.namespace",
                                    ),
                                    String(
                                        Static(
                                            "logfire::bridges::tracing::tests",
                                        ),
                                    ),
                                ),
                            ),
                            Some(
                                (
                                    Static(
                                        "logfire.json_schema",
                                    ),
                                    String(
                                        Static(
                                            "{}",
                                        ),
                                    ),
                                ),
                            ),
                            Some(
                                (
                                    Static(
                                        "logfire.msg",
                                    ),
                                    String(
                                        Owned(
                                            "hello world log",
                                        ),
                                    ),
                                ),
                            ),
                        ],
                        overflow: Some(
                            [
                                Some(
                                    (
                                        Static(
                                            "thread.id",
                                        ),
                                        Int(
                                            0,
                                        ),
                                    ),
                                ),
                                Some(
                                    (
                                        Static(
                                            "thread.name",
                                        ),
                                        String(
                                            Owned(
                                                "bridges::tracing::tests::test_tracing_bridge",
                                            ),
                                        ),
                                    ),
                                ),
                            ],
                        ),
                        count: 5,
                    },
                },
                instrumentation: InstrumentationScope {
                    name: "logfire",
                    version: None,
                    schema_url: None,
                    attributes: [],
                },
                resource: Resource {
                    inner: ResourceInner {
                        attrs: {},
                        schema_url: None,
                    },
                },
            },
            LogDataWithResource {
                record: SdkLogRecord {
                    event_name: None,
                    target: None,
                    timestamp: Some(
                        SystemTime {
                            tv_sec: 3,
                            tv_nsec: 0,
                        },
                    ),
                    observed_timestamp: Some(
                        SystemTime {
                            tv_sec: 3,
                            tv_nsec: 0,
                        },
                    ),
                    trace_context: Some(
                        TraceContext {
                            trace_id: 000000000000000000000000000000f0,
                            span_id: 00000000000000f0,
                            trace_flags: Some(
                                TraceFlags(
                                    1,
                                ),
                            ),
                        },
                    ),
                    severity_text: Some(
                        "INFO",
                    ),
                    severity_number: Some(
                        Info,
                    ),
                    body: Some(
                        String(
                            Owned(
                                "hello world log with value",
                            ),
                        ),
                    ),
                    attributes: GrowableArray {
                        inline: [
                            Some(
                                (
                                    Static(
                                        "code.filepath",
                                    ),
                                    String(
                                        Static(
                                            "src/bridges/tracing.rs",
                                        ),
                                    ),
                                ),
                            ),
                            Some(
                                (
                                    Static(
                                        "code.lineno",
                                    ),
                                    Int(
                                        34,
                                    ),
                                ),
                            ),
                            Some(
                                (
                                    Static(
                                        "code.namespace",
                                    ),
                                    String(
                                        Static(
                                            "logfire::bridges::tracing::tests",
                                        ),
                                    ),
                                ),
                            ),
                            Some(
                                (
                                    Static(
                                        "field_value",
                                    ),
                                    String(
                                        Owned(
                                            "1",
                                        ),
                                    ),
                                ),
                            ),
                            Some(
                                (
                                    Static(
                                        "logfire.json_schema",
                                    ),
                                    String(
                                        Static(
                                            "{}",
                                        ),
                                    ),
                                ),
                            ),
                        ],
                        overflow: Some(
                            [
                                Some(
                                    (
                                        Static(
                                            "logfire.msg",
                                        ),
                                        String(
                                            Owned(
                                                "hello world log with value",
                                            ),
                                        ),
                                    ),
                                ),
                                Some(
                                    (
                                        Static(
                                            "thread.id",
                                        ),
                                        Int(
                                            0,
                                        ),
                                    ),
                                ),
                                Some(
                                    (
                                        Static(
                                            "thread.name",
                                        ),
                                        String(
                                            Owned(
                                                "bridges::tracing::tests::test_tracing_bridge",
                                            ),
                                        ),
                                    ),
                                ),
                            ],
                        ),
                        count: 5,
                    },
                },
                instrumentation: InstrumentationScope {
                    name: "logfire",
                    version: None,
                    schema_url: None,
                    attributes: [],
                },
                resource: Resource {
                    inner: ResourceInner {
                        attrs: {},
                        schema_url: None,
                    },
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

        assert_snapshot!(output, @r"
        [2m1970-01-01T00:00:00.000000Z[0m[32m  INFO[0m [2;3mlogfire::bridges::tracing::tests[0m [1mroot event[0m
        [2m1970-01-01T00:00:00.000001Z[0m[32m  INFO[0m [2;3mlogfire::bridges::tracing::tests[0m [1mroot event with value[0m [3mfield_value[0m=1
        [2m1970-01-01T00:00:00.000002Z[0m[32m  INFO[0m [2;3mlogfire::bridges::tracing::tests[0m [1mroot span[0m
        [2m1970-01-01T00:00:00.000003Z[0m[32m  INFO[0m [2;3mlogfire::bridges::tracing::tests[0m [1mhello world span[0m
        [2m1970-01-01T00:00:00.000004Z[0m[34m DEBUG[0m [2;3mlogfire::bridges::tracing::tests[0m [1mdebug span[0m
        [2m1970-01-01T00:00:00.000005Z[0m[34m DEBUG[0m [2;3mlogfire::bridges::tracing::tests[0m [1mdebug span with explicit parent[0m
        [2m1970-01-01T00:00:00.000006Z[0m[32m  INFO[0m [2;3mlogfire::bridges::tracing::tests[0m [1mhello world log[0m
        [2m1970-01-01T00:00:00.000007Z[0m[32m  INFO[0m [2;3mlogfire::bridges::tracing::tests[0m [1mhello world log with value[0m [3mfield_value[0m=1
        [2m1970-01-01T00:00:00.000008Z[0m[34m DEBUG[0m [2;3mopentelemetry_sdk::metrics::meter_provider[0m [1mUser initiated shutdown of MeterProvider.[0m [3mname[0m=MeterProvider.Shutdown
        [2m1970-01-01T00:00:00.000009Z[0m[34m DEBUG[0m [2;3mopentelemetry_sdk::logs::logger_provider[0m [1m[0m [3mname[0m=LoggerProvider.ShutdownInvokedByUser
        ");
    }

    #[tokio::test]
    async fn test_tracing_metrics_layer() {
        use crate::test_utils::make_deterministic_resource_metrics;
        use opentelemetry_sdk::metrics::{
            InMemoryMetricExporterBuilder, ManualReader, data::ResourceMetrics,
            exporter::PushMetricExporter, reader::MetricReader,
        };
        use std::sync::Arc;

        #[derive(Clone, Debug)]
        struct SharedManualReader {
            reader: Arc<ManualReader>,
        }

        impl SharedManualReader {
            fn new(reader: ManualReader) -> Self {
                Self {
                    reader: Arc::new(reader),
                }
            }

            async fn export<E: PushMetricExporter>(&self, exporter: &E) {
                let mut metrics = ResourceMetrics::default();
                self.reader.collect(&mut metrics).unwrap();
                exporter.export(&mut metrics).await.unwrap();
            }
        }

        impl MetricReader for SharedManualReader {
            fn register_pipeline(
                &self,
                pipeline: std::sync::Weak<opentelemetry_sdk::metrics::Pipeline>,
            ) {
                self.reader.register_pipeline(pipeline);
            }

            fn collect(
                &self,
                rm: &mut opentelemetry_sdk::metrics::data::ResourceMetrics,
            ) -> opentelemetry_sdk::error::OTelSdkResult {
                self.reader.collect(rm)
            }

            fn force_flush(&self) -> opentelemetry_sdk::error::OTelSdkResult {
                self.reader.force_flush()
            }

            fn shutdown(&self) -> opentelemetry_sdk::error::OTelSdkResult {
                self.reader.shutdown()
            }

            fn shutdown_with_timeout(
                &self,
                timeout: std::time::Duration,
            ) -> opentelemetry_sdk::error::OTelSdkResult {
                self.reader.shutdown_with_timeout(timeout)
            }

            fn temporality(
                &self,
                kind: opentelemetry_sdk::metrics::InstrumentKind,
            ) -> opentelemetry_sdk::metrics::Temporality {
                self.reader.temporality(kind)
            }
        }

        let mut exporter = InMemoryMetricExporterBuilder::new().build();

        let reader = SharedManualReader::new(
            ManualReader::builder()
                .with_temporality(opentelemetry_sdk::metrics::Temporality::Delta)
                .build(),
        );

        let handler = crate::configure()
            .local()
            .send_to_logfire(false)
            .with_metrics(Some(
                crate::config::MetricsOptions::default().with_additional_reader(reader.clone()),
            ))
            .install_panic_handler()
            .with_default_level_filter(LevelFilter::TRACE)
            .with_advanced_options(
                AdvancedOptions::default()
                    .with_resource(
                        opentelemetry_sdk::Resource::builder_empty()
                            .with_service_name("test")
                            .build(),
                    )
                    .with_id_generator(DeterministicIdGenerator::new())
                    .with_tracing_metrics(true),
            )
            .finish()
            .unwrap();

        let _guard = set_local_logfire(handler.clone());

        tracing::info!(counter.test_counter = 1, "test counter event");
        tracing::info!(histogram.test_histogram = 2.5, "test histogram event");
        tracing::info!(
            monotonic_counter.test_monotonic = 3,
            "test monotonic counter event"
        );

        tracing::info!(counter.test_counter = 2, "test counter event");
        tracing::info!(histogram.test_histogram = 3.5, "test histogram event");
        tracing::info!(
            monotonic_counter.test_monotonic = 4,
            "test monotonic counter event"
        );

        reader.export(&mut exporter).await;

        tracing::info!(counter.test_counter = 3, "test counter event");
        tracing::info!(histogram.test_histogram = 4.5, "test histogram event");
        tracing::info!(
            monotonic_counter.test_monotonic = 5,
            "test monotonic counter event"
        );

        reader.export(&mut exporter).await;

        handler.shutdown().unwrap();

        let metrics = exporter.get_finished_metrics().unwrap();
        let metrics = make_deterministic_resource_metrics(metrics);

        assert_debug_snapshot!(metrics, @r#"
        [
            DeterministicResourceMetrics {
                resource: Resource {
                    inner: ResourceInner {
                        attrs: {
                            Static(
                                "service.name",
                            ): String(
                                Static(
                                    "test",
                                ),
                            ),
                        },
                        schema_url: None,
                    },
                },
                scope_metrics: [
                    DeterministicScopeMetrics {
                        scope: InstrumentationScope {
                            name: "tracing/tracing-opentelemetry",
                            version: Some(
                                "0.31.0",
                            ),
                            schema_url: None,
                            attributes: [],
                        },
                        metrics: [
                            DeterministicMetric {
                                name: "test_counter",
                                values: [
                                    3,
                                ],
                            },
                            DeterministicMetric {
                                name: "test_histogram",
                                values: [
                                    2,
                                ],
                            },
                            DeterministicMetric {
                                name: "test_monotonic",
                                values: [
                                    7,
                                ],
                            },
                        ],
                    },
                ],
            },
            DeterministicResourceMetrics {
                resource: Resource {
                    inner: ResourceInner {
                        attrs: {
                            Static(
                                "service.name",
                            ): String(
                                Static(
                                    "test",
                                ),
                            ),
                        },
                        schema_url: None,
                    },
                },
                scope_metrics: [
                    DeterministicScopeMetrics {
                        scope: InstrumentationScope {
                            name: "tracing/tracing-opentelemetry",
                            version: Some(
                                "0.31.0",
                            ),
                            schema_url: None,
                            attributes: [],
                        },
                        metrics: [
                            DeterministicMetric {
                                name: "test_counter",
                                values: [
                                    6,
                                ],
                            },
                            DeterministicMetric {
                                name: "test_histogram",
                                values: [
                                    1,
                                ],
                            },
                            DeterministicMetric {
                                name: "test_monotonic",
                                values: [
                                    5,
                                ],
                            },
                        ],
                    },
                ],
            },
        ]
        "#);
    }
}
