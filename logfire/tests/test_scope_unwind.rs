//! Native Logfire logs and tracing spans use separate thread-local state.

use std::panic::{AssertUnwindSafe, catch_unwind};

use logfire::config::{AdvancedOptions, LogfireConfigBuilder};
use opentelemetry_sdk::logs::{InMemoryLogExporter, SimpleLogProcessor};

fn pipeline(builder: LogfireConfigBuilder) -> (logfire::Logfire, InMemoryLogExporter) {
    let logs = InMemoryLogExporter::default();
    let pipeline = builder
        .send_to_logfire(false)
        .with_console(None)
        .with_install_panic_handler(false)
        .with_advanced_options(
            AdvancedOptions::default().with_log_processor(SimpleLogProcessor::new(logs.clone())),
        )
        .finish()
        .expect("configure log exporter");
    (pipeline, logs)
}

fn messages(exporter: &InMemoryLogExporter) -> Vec<String> {
    exporter
        .get_emitted_logs()
        .expect("exported logs")
        .iter()
        .filter_map(|log| log.record.event_name())
        .filter(|name| name.starts_with("scope test:"))
        .map(str::to_owned)
        .collect()
}

#[test]
fn unwind_restores_the_outer_tracer_and_dispatcher() {
    fn probe() -> tracing::Span {
        tracing::error_span!("unwind probe")
    }

    let (global, global_logs) = pipeline(logfire::configure());
    let (local, local_logs) = pipeline(logfire::configure().local());
    {
        let _outer = logfire::set_local_logfire(local.clone());
        logfire::error!("scope test: outer before");
        tracing::dispatcher::with_default(&tracing::Dispatch::none(), || {
            let result = catch_unwind(AssertUnwindSafe(|| {
                let _inner = logfire::set_local_logfire(global.clone());
                assert!(!probe().is_disabled());
                logfire::error!("scope test: inner");
                panic!("leave recording scope");
            }));
            let panic = result.expect_err("inner scope must unwind");
            assert_eq!(panic.downcast_ref::<&str>(), Some(&"leave recording scope"));
            assert!(probe().is_disabled());
        });
        logfire::error!("scope test: outer after");
    }
    logfire::error!("scope test: ambient");

    assert_eq!(
        messages(&global_logs),
        ["scope test: inner", "scope test: ambient"]
    );
    assert_eq!(
        messages(&local_logs),
        ["scope test: outer before", "scope test: outer after"]
    );
}
