use std::sync::LazyLock;

use opentelemetry::{KeyValue, metrics::Counter};

static BASIC_COUNTER: LazyLock<Counter<u64>> = LazyLock::new(|| {
    logfire::u64_counter("basic_counter")
        .with_description("Just an example")
        .with_unit("s")
        .build()
});

fn main() {
    let shutdown_handler = logfire::configure()
        .install_panic_handler()
        .send_to_logfire(true)
        .console_mode(logfire::ConsoleMode::Fallback)
        .finish()
        .expect("Failed to configure logfire");

    logfire::info!("Hello, world!");

    BASIC_COUNTER.add(1, &[KeyValue::new("process", "abc123")]);

    shutdown_handler
        .shutdown()
        .expect("Failed to shutdown logfire");
}
