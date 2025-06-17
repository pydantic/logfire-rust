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
    trace::{SpanId, TraceId},
};
use opentelemetry_sdk::{
    Resource,
    error::OTelSdkResult,
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
                    sum_metrics: scope_metric
                        .metrics()
                        .filter_map(|metric| {
                            if let AggregatedMetrics::U64(MetricData::Sum(sum)) = metric.data() {
                                Some(sum.data_points().map(|dp| dp.value()).collect())
                            } else {
                                None
                            }
                        })
                        .collect(),
                })
                .collect(),
        })
        .collect()
}

/// A reproduction of the `ScopeMetrics` type from the otel sdk, narrowed to u64 sum metrics
///
/// This exists because the otel SDK (as of version 0.30) exposes no way to create a `ScopeMetrics`
/// outside of the library.
#[derive(Debug)]
pub struct DeterministicScopeMetrics {
    scope: InstrumentationScope,
    sum_metrics: Vec<Vec<u64>>,
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
