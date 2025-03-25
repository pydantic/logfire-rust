use std::{
    collections::{HashMap, hash_map::Entry},
    future::Future,
    pin::Pin,
    sync::atomic::{AtomicU64, Ordering},
    time::SystemTime,
};

use opentelemetry::{
    Value,
    trace::{SpanId, TraceId},
};
use opentelemetry_sdk::{
    error::OTelSdkResult,
    trace::{IdGenerator, SpanData, SpanExporter},
};

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
    next_timestamp: u64,
    timestamp_remap: HashMap<SystemTime, SystemTime>,
    // Information used to adjust line number to help minimise test churn
    file: &'static str,
    line_offset: u32,
}

impl<Inner: SpanExporter> SpanExporter for DeterministicExporter<Inner> {
    fn export(
        &mut self,
        mut batch: Vec<SpanData>,
    ) -> Pin<Box<dyn Future<Output = OTelSdkResult> + Send>> {
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
            next_timestamp: 0,
            timestamp_remap: HashMap::new(),
            file,
            line_offset,
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
