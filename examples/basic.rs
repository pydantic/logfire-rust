//! A basic example of using Logfire to instrument Rust code.

use std::sync::LazyLock;

use opentelemetry::{KeyValue, metrics::Counter};

static BASIC_COUNTER: LazyLock<Counter<u64>> = LazyLock::new(|| {
    logfire::u64_counter("basic_counter")
        .with_description("Just an example")
        .with_unit("s")
        .build()
});

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let shutdown_handler = logfire::configure()
        .install_panic_handler()
        .with_console(None)
        .finish()?;

    tracing::info!("Hello, world!");

    {
        let _span = logfire::span!("Asking the user their {question}", question = "age").entered();

        println!("When were you born [YYYY-mm-dd]?");
        let mut dob = String::new();
        std::io::stdin().read_line(&mut dob)?;

        logfire::debug!("dob={dob}", dob = dob.trim().to_owned());
    }

    BASIC_COUNTER.add(1, &[KeyValue::new("process", "abc123")]);

    shutdown_handler.shutdown()?;
    Ok(())
}
