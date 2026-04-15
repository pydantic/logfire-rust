//! Example of using `logfire` to instrument a toy Rust application

use std::fs;
use std::sync::LazyLock;

use opentelemetry::{KeyValue, metrics::Counter};

static FILES_COUNTER: LazyLock<Counter<u64>> = LazyLock::new(|| {
    logfire::u64_counter("FILES_COUNTER")
        .with_description("Just an example")
        .with_unit("s")
        .build()
});

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn main() -> Result<()> {
    let logfire = logfire::configure().finish()?;
    let _guard = logfire.shutdown_guard();

    let mut total_size = 0u64;

    let cwd = std::env::current_dir()?;

    logfire::span!("counting size of {cwd}", cwd = cwd.display().to_string()).in_scope(|| {
        let entries = fs::read_dir(&cwd)?;
        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            let _span = logfire::span!(
                "reading {path}",
                path = path
                    .strip_prefix(&cwd)
                    .unwrap_or(&path)
                    .display()
                    .to_string()
            )
            .entered();

            FILES_COUNTER.add(1, &[KeyValue::new("process", "abc123")]);

            let metadata = entry.metadata()?;
            if metadata.is_file() {
                total_size += metadata.len();
            }
        }
        Result::Ok(())
    })?;

    logfire::info!(
        "total size of {cwd} is {size} bytes",
        cwd = cwd.display().to_string(),
        size = total_size
    );

    Ok(())
}
