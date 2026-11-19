//! Observe the process-wide callsite cache independently of span export.

use std::sync::{
    Arc, Barrier,
    atomic::{AtomicUsize, Ordering},
};

use opentelemetry::{Context, Value};
use opentelemetry_sdk::trace::{InMemorySpanExporterBuilder, SimpleSpanProcessor};
use tracing::{Dispatch, Metadata, Subscriber, subscriber::Interest};
use tracing_subscriber::{Layer, layer::SubscriberExt};

const PROBE: &str = "scoped registration probe";
const THREADS: i64 = 4;
const PER_THREAD: i64 = 8;

struct CountRegistrations(Arc<AtomicUsize>);

impl<S: Subscriber> Layer<S> for CountRegistrations {
    fn register_callsite(&self, metadata: &'static Metadata<'static>) -> Interest {
        if metadata.name() == PROBE {
            self.0.fetch_add(1, Ordering::Relaxed);
        }
        Interest::always()
    }
}

fn record(sequence: i64) {
    drop(tracing::error_span!(PROBE, sequence));
}

#[test]
fn concurrent_global_scopes_reuse_registration_and_export_once() {
    let spans = InMemorySpanExporterBuilder::new().build();
    let global = logfire::configure()
        .send_to_logfire(false)
        .with_console(None)
        .with_additional_span_processor(SimpleSpanProcessor::new(spans.clone()))
        .finish()
        .expect("configure global Logfire");
    let registrations = Arc::new(AtomicUsize::new(0));
    let observer = Dispatch::new(
        tracing_subscriber::registry().with(CountRegistrations(Arc::clone(&registrations))),
    );
    // Warm both the scope and the callsite. Only a rebuild should register the probe again.
    {
        let _warm = logfire::set_local_logfire(global.clone());
        record(-1);
    }
    spans.reset();
    let warmed = registrations.load(Ordering::Relaxed);
    assert!(warmed > 0, "observer must have seen the probe callsite");

    let start = Barrier::new(usize::try_from(THREADS).expect("positive thread count"));
    std::thread::scope(|threads| {
        for worker in 0..THREADS {
            let (global, observer, start) = (&global, &observer, &start);
            threads.spawn(move || {
                tracing::dispatcher::with_default(observer, || {
                    start.wait();
                    for index in 0..PER_THREAD {
                        {
                            let _scope = logfire::set_local_logfire(global.clone());
                            {
                                let _suppressed = Context::enter_telemetry_suppressed_scope();
                                record(-2);
                            }
                            record(worker * PER_THREAD + index);
                        }
                        record(-1);
                    }
                });
            });
        }
    });

    assert_eq!(registrations.load(Ordering::Relaxed), warmed);
    let mut sequences: Vec<_> = spans
        .get_finished_spans()
        .expect("exported spans")
        .into_iter()
        .filter(|span| span.name == PROBE)
        .map(|span| {
            span.attributes
                .into_iter()
                .find(|kv| kv.key.as_str() == "sequence")
                .expect("sequence field on the probe")
                .value
        })
        .map(|value| match value {
            Value::I64(i) => i,
            other => panic!("expected an integer sequence, got {other:?}"),
        })
        .collect();
    sequences.sort_unstable();
    assert_eq!(sequences, (0..THREADS * PER_THREAD).collect::<Vec<_>>());
}
