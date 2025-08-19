#![allow(dead_code)] // used by lib and test suites individually

use std::{
    borrow::Cow,
    collections::{HashMap, hash_map::Entry},
    future::Future,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{self, SystemTime},
};

use opentelemetry::{
    InstrumentationScope, Value,
    logs::{LogRecord, Logger, LoggerProvider as _},
    trace::{SpanId, TraceId},
};
use opentelemetry_proto::tonic::collector::trace::v1::ExportTraceServiceRequest;
use opentelemetry_sdk::{
    Resource,
    error::OTelSdkResult,
    logs::{SdkLoggerProvider, in_memory_exporter::LogDataWithResource},
    metrics::data::{AggregatedMetrics, MetricData, ResourceMetrics},
    trace::{IdGenerator, SpanData, SpanExporter},
};
use regex::{Captures, Regex};

#[derive(Debug)]
pub struct DeterministicIdGenerator {
    next_trace_id: AtomicU64,
    next_span_id: AtomicU64,
}

impl IdGenerator for DeterministicIdGenerator {
    fn new_trace_id(&self) -> opentelemetry::trace::TraceId {
        TraceId::from_u128(self.next_trace_id.fetch_add(1, Ordering::Relaxed).into())
    }

    fn new_span_id(&self) -> opentelemetry::trace::SpanId {
        SpanId::from_u64(self.next_span_id.fetch_add(1, Ordering::Relaxed))
    }
}

impl DeterministicIdGenerator {
    pub fn new() -> Self {
        // start at OxF0 because 0 is reserved for invalid IDs,
        // and if we have a couple of bytes used, it's a more interesting check of
        // the hex formatting
        Self {
            next_trace_id: 0xF0.into(),
            next_span_id: 0xF0.into(),
        }
    }
}

#[derive(Debug)]
pub struct DeterministicExporter<Inner> {
    exporter: Inner,
    timestamp_remap: Arc<Mutex<TimestampRemapper>>,
    // Information used to adjust line number to help minimise test churn
    file: &'static str,
    line_offset: u32,
}

impl<Inner> DeterministicExporter<Inner> {
    pub fn inner(&self) -> &Inner {
        &self.exporter
    }
}

impl<Inner: SpanExporter> SpanExporter for DeterministicExporter<Inner> {
    fn export(&self, mut batch: Vec<SpanData>) -> impl Future<Output = OTelSdkResult> + Send {
        for span in &mut batch {
            // By remapping timestamps to deterministic values, we should find that
            // - pending spans have the same start time as their real span
            // - pending spans also have the same end as start
            span.start_time = self.remap_timestamp(span.start_time);
            span.end_time = self.remap_timestamp(span.end_time);

            let mut remap_line = false;

            for attr in &mut span.attributes {
                // thread info is not deterministic
                // nor are timings
                if attr.key.as_str() == "thread.id"
                    || attr.key.as_str() == "busy_ns"
                    || attr.key.as_str() == "idle_ns"
                {
                    attr.value = 0.into();
                }

                // to minimize churn on tests, remap line numbers in the test to be relative to
                // the test function
                if attr.key.as_str() == "code.filepath" && attr.value.as_str() == self.file {
                    remap_line = true;
                }
            }

            if remap_line {
                for attr in &mut span.attributes {
                    if attr.key.as_str() == "code.lineno" {
                        if let Value::I64(line) = &mut attr.value {
                            *line -= i64::from(self.line_offset);
                        }
                    }

                    // panic location
                    if attr.key.as_str() == "location" {
                        let string_value = attr.value.as_str();
                        let mut parts = string_value.splitn(3, ':');
                        let file = parts.next().unwrap();
                        let line = parts.next().unwrap().parse::<i64>().unwrap()
                            - i64::from(self.line_offset);
                        let column = parts.next().unwrap();
                        attr.value = format!("{file}:{line}:{column}").into();
                    }
                }
            }

            for event in &mut span.events.events {
                event.timestamp = self.remap_timestamp(event.timestamp);
            }
        }
        self.exporter.export(batch)
    }
}

impl<Inner> DeterministicExporter<Inner> {
    /// Create deterministic exporter, feeding it current file and line.
    pub fn new(exporter: Inner, file: &'static str, line_offset: u32) -> Self {
        Self {
            exporter,
            timestamp_remap: Arc::new(Mutex::new(TimestampRemapper::new())),
            file,
            line_offset,
        }
    }

    fn remap_timestamp(&self, from: SystemTime) -> SystemTime {
        self.timestamp_remap.lock().unwrap().remap_timestamp(from)
    }
}

#[derive(Debug)]
struct TimestampRemapper {
    next_timestamp: u64,
    timestamp_remap: HashMap<SystemTime, SystemTime>,
}

impl TimestampRemapper {
    fn new() -> Self {
        Self {
            next_timestamp: 0,
            timestamp_remap: HashMap::new(),
        }
    }

    fn remap_timestamp(&mut self, from: SystemTime) -> SystemTime {
        match self.timestamp_remap.entry(from) {
            Entry::Occupied(entry) => *entry.get(),
            Entry::Vacant(entry) => {
                let new_timestamp =
                    SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(self.next_timestamp);
                self.next_timestamp += 1;
                *entry.insert(new_timestamp)
            }
        }
    }

    fn remap_u64_nano_timestamp(&mut self, from: u64) -> u64 {
        self.remap_timestamp(SystemTime::UNIX_EPOCH + std::time::Duration::from_nanos(from))
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            .try_into()
            .unwrap()
    }
}

pub fn remap_timestamps_in_console_output(output: &str) -> Cow<'_, str> {
    // Replace all timestamps in output to make them deterministic
    let mut timestamp = chrono::DateTime::UNIX_EPOCH;
    let re = Regex::new(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{6}Z").unwrap();
    re.replace_all(output, |_: &Captures<'_>| {
        let replaced = timestamp.to_rfc3339_opts(chrono::SecondsFormat::Micros, true);
        timestamp += time::Duration::from_micros(1);
        replaced
    })
}

pub fn make_deterministic_resource_metrics(
    metrics: Vec<ResourceMetrics>,
) -> Vec<DeterministicResourceMetrics> {
    metrics
        .into_iter()
        .map(|metric| DeterministicResourceMetrics {
            resource: metric.resource().clone(),
            scope_metrics: metric
                .scope_metrics()
                .map(|scope_metric| DeterministicScopeMetrics {
                    scope: scope_metric.scope().clone(),
                    metrics: scope_metric
                        .metrics()
                        .filter_map(|metric| {
                            let name = metric.name().to_string();
                            match metric.data() {
                                AggregatedMetrics::U64(MetricData::Sum(sum)) => {
                                    Some(DeterministicMetric {
                                        name,
                                        values: sum
                                            .data_points()
                                            .map(|dp| dp.value() as u64)
                                            .collect(),
                                    })
                                }
                                AggregatedMetrics::I64(MetricData::Sum(sum)) => {
                                    Some(DeterministicMetric {
                                        name,
                                        values: sum
                                            .data_points()
                                            .map(|dp| dp.value() as u64)
                                            .collect(),
                                    })
                                }
                                AggregatedMetrics::F64(MetricData::Histogram(histogram)) => {
                                    Some(DeterministicMetric {
                                        name,
                                        values: histogram
                                            .data_points()
                                            .map(|dp| dp.count() as u64)
                                            .collect(),
                                    })
                                }
                                _ => None,
                            }
                        })
                        .collect(),
                })
                .collect(),
        })
        .collect()
}

/// A reproduction of the `ScopeMetrics` type from the otel sdk, narrowed to support histogram and counter metrics
///
/// This exists because the otel SDK (as of version 0.30) exposes no way to create a `ScopeMetrics`
/// outside of the library.
#[derive(Debug)]
pub struct DeterministicScopeMetrics {
    scope: InstrumentationScope,
    metrics: Vec<DeterministicMetric>,
}

#[derive(Debug)]
pub struct DeterministicMetric {
    name: String,
    values: Vec<u64>,
}

/// Deterministic resource metrics
#[derive(Debug)]
pub struct DeterministicResourceMetrics {
    resource: Resource,
    scope_metrics: Vec<DeterministicScopeMetrics>,
}

/// Find a span by name in a slice of SpanData.
pub fn find_span<'a>(
    spans: &'a [opentelemetry_sdk::trace::SpanData],
    name: &str,
) -> &'a opentelemetry_sdk::trace::SpanData {
    spans.iter().find(|s| s.name == name).expect("span present")
}

/// Find an attribute by key in a span's attributes, panicking if not found.
pub fn find_attr<'a>(
    span: &'a opentelemetry_sdk::trace::SpanData,
    key: &str,
) -> &'a opentelemetry::KeyValue {
    span.attributes
        .iter()
        .find(|kv| kv.key.as_str() == key)
        .unwrap_or_else(|| panic!("attribute '{}' not found in span '{}'", key, span.name))
}

/// Find a log by event name in a slice of LogDataWithResource.
pub fn find_log<'a>(logs: &'a [LogDataWithResource], name: &str) -> &'a LogDataWithResource {
    logs.iter()
        .find(|log| {
            log.record
                .event_name()
                .is_some_and(|event_name| event_name == name)
        })
        .expect("log present")
}

/// Find an attribute by key in a log's attributes, panicking if not found.
pub fn find_log_attr<'a>(
    log: &'a LogDataWithResource,
    key: &str,
) -> &'a opentelemetry::logs::AnyValue {
    log.record
        .attributes_iter()
        .find(|(k, _)| k.as_str() == key)
        .map(|(_, v)| v)
        .unwrap_or_else(|| panic!("attribute '{}' not found in log", key))
}

/// Make log data deterministic by cleaning up non-deterministic fields
pub fn make_deterministic_logs(
    logs: Vec<LogDataWithResource>,
    file: &str,
    line_offset: u32,
) -> Vec<LogDataWithResource> {
    let mut timestamp_remap = TimestampRemapper::new();

    // A new logger is necessary to be able to create log records; `SdkLogRecord` constructor is private.
    let logger_provider = SdkLoggerProvider::builder().build();
    let logger = logger_provider.logger("test_logger");

    logs.into_iter()
        .map(|log_data| {
            let original_record = &log_data.record;

            // Create a new log record
            let mut new_record = logger.create_log_record();

            // Copy basic fields with deterministic timestamp remapping
            if let Some(timestamp) = original_record.timestamp() {
                new_record.set_timestamp(timestamp_remap.remap_timestamp(timestamp));
            }
            if let Some(observed_timestamp) = original_record.observed_timestamp() {
                new_record
                    .set_observed_timestamp(timestamp_remap.remap_timestamp(observed_timestamp));
            }
            if let Some(event_name) = original_record.event_name() {
                new_record.set_event_name(event_name);
            }

            if let Some(trace_context) = original_record.trace_context() {
                new_record.set_trace_context(
                    trace_context.trace_id,
                    trace_context.span_id,
                    trace_context.trace_flags,
                );
            }
            if let Some(severity_text) = original_record.severity_text() {
                new_record.set_severity_text(severity_text);
            }
            if let Some(severity_number) = original_record.severity_number() {
                new_record.set_severity_number(severity_number);
            }
            if let Some(body) = original_record.body() {
                new_record.set_body(body.clone());
            }
            if let Some(target) = original_record.target() {
                new_record.set_target(target.clone());
            }

            // Check if we need to remap line numbers for this file
            let mut remap_line = false;
            for (key, value) in original_record.attributes_iter() {
                if key.as_str() == "code.filepath" {
                    if let opentelemetry::logs::AnyValue::String(s) = value {
                        if s.as_str() == file {
                            remap_line = true;
                            break;
                        }
                    }
                }
            }

            let mut new_attributes = Vec::new();

            // Copy attributes with deterministic modifications
            for (key, value) in original_record.attributes_iter() {
                let new_value = match key.as_str() {
                    "thread.id" => opentelemetry::logs::AnyValue::Int(0),
                    "code.lineno" if remap_line => {
                        if let opentelemetry::logs::AnyValue::Int(line) = value {
                            opentelemetry::logs::AnyValue::Int(line - i64::from(line_offset))
                        } else {
                            value.clone()
                        }
                    }
                    _ => value.clone(),
                };
                new_attributes.push((key.clone(), new_value));
            }

            new_attributes.sort_by(|(a, _), (b, _)| a.cmp(b));

            new_record.add_attributes(new_attributes);

            LogDataWithResource {
                record: new_record,
                // avoid non-deteministic resource
                resource: Cow::Owned(Resource::builder_empty().build()),
                ..log_data
            }
        })
        .collect()
}

pub fn make_trace_request_deterministic(req: &mut ExportTraceServiceRequest) {
    let mut timestamp_remap = TimestampRemapper::new();

    for resource_span in &mut req.resource_spans {
        if let Some(resource) = &mut resource_span.resource {
            // Sort attributes by key
            resource.attributes.sort_by_key(|attr| attr.key.clone());
        }

        for scope_span in &mut resource_span.scope_spans {
            if let Some(scope) = &mut scope_span.scope {
                // Sort attributes by key
                scope.attributes.sort_by_key(|attr| attr.key.clone());
            }

            for span in &mut scope_span.spans {
                // Set start/end timestamps to deterministic values
                span.start_time_unix_nano =
                    timestamp_remap.remap_u64_nano_timestamp(span.start_time_unix_nano);
                span.end_time_unix_nano =
                    timestamp_remap.remap_u64_nano_timestamp(span.end_time_unix_nano);

                // Zero out non-deterministic attributes
                for attr in &mut span.attributes {
                    if attr.key == "thread.id" || attr.key == "busy_ns" || attr.key == "idle_ns" {
                        attr.value = Some(opentelemetry_proto::tonic::common::v1::AnyValue {
                            value: Some(
                                opentelemetry_proto::tonic::common::v1::any_value::Value::IntValue(
                                    0,
                                ),
                            ),
                        });
                    }
                }

                // Also sort attributes by key
                span.attributes.sort_by_key(|attr| attr.key.clone());

                // Set event timestamps to deterministic values
                for event in &mut span.events {
                    event.time_unix_nano =
                        timestamp_remap.remap_u64_nano_timestamp(event.time_unix_nano);
                }
            }
        }
    }
}
