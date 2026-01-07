# Rust SDK for Pydantic Logfire

<p align="center">
  <a href="https://github.com/pydantic/logfire-rust/actions?query=event%3Apush+branch%3Amain+workflow%3ACI"><img src="https://github.com/pydantic/logfire-rust/actions/workflows/main.yml/badge.svg?event=push" alt="CI" /></a>
  <a href="https://codecov.io/gh/pydantic/logfire-rust"><img src="https://codecov.io/gh/pydantic/logfire-rust/graph/badge.svg?token=735CNGCGFD" alt="codecov" /></a>
  <a href="https://crates.io/crates/logfire"><img src="https://img.shields.io/crates/v/logfire.svg?logo=rust" alt="crates.io" /></a>
  <a href="https://github.com/pydantic/logfire-rust/blob/main/LICENSE"><img src="https://img.shields.io/github/license/pydantic/logfire-rust.svg" alt="license" /></a>
  <a href="https://github.com/pydantic/logfire"><img src="https://img.shields.io/crates/msrv/logfire.svg?logo=rust" alt="MSRV" /></a>
  <a href="https://logfire.pydantic.dev/docs/join-slack/"><img src="https://img.shields.io/badge/Slack-Join%20Slack-4A154B?logo=slack" alt="Join Slack" /></a>
</p>

From the team behind Pydantic Validation, **Pydantic Logfire** is an observability platform built on the same belief as our open source library — that the most powerful tools can be easy to use.

What sets Logfire apart:

- **Simple and Powerful:** Logfire's dashboard is simple relative to the power it provides, ensuring your entire engineering team will actually use it.
- **SQL:** Query your data using standard SQL — all the control and (for many) nothing new to learn. Using SQL also means you can query your data with existing BI tools and database querying libraries.
- **OpenTelemetry:** Logfire is an opinionated wrapper around OpenTelemetry, allowing you to leverage existing tooling, infrastructure, and instrumentation for many common Python packages, and enabling support for virtually any language. We offer full support for all OpenTelemetry signals (traces, metrics and logs).

This repository contains the Rust SDK for instrumenting with Logfire.

See also:
 - The [SDK documentation on docs.rs](https://docs.rs/logfire) for an API reference for this library.
 - The [Logfire documentation](https://logfire.pydantic.dev/docs/) for more information about Logfire in general.
 - The [Logfire GitHub repository](https://github.com/pydantic/logfire) for the source of the documentation, the Python SDK and an issue tracker for general questions about Logfire.

The Logfire server application for recording and displaying data is closed source.

## Using Logfire's Rust SDK

First [set up a Logfire project](https://logfire.pydantic.dev/docs/#logfire). You will then configure the SDK to send to Logfire by:
- [creating a write token](https://logfire.pydantic.dev/docs/how-to-guides/create-write-tokens/) manually and setting this token as an environment variable (`LOGFIRE_TOKEN`), or
- [using the Logfire CLI](https://logfire.pydantic.dev/docs/#instrument) to select a project.

With a logfire project set up, start by adding the `logfire` crate to your `Cargo.toml`:

```toml
[dependencies]
logfire = "0.6"
```

Then, you can use the SDK to instrument the code. Here's a simple example which counts the size of files in the current directory, creating spans for the full operation and each file read:


```rust
use std::fs;
use std::sync::LazyLock;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn main() -> Result<()> {
    let shutdown_handler = logfire::configure().install_panic_handler().finish()?;

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

    shutdown_handler.shutdown()?;
    Ok(())
}
```

(Read the [Logfire concepts documentation](https://logfire.pydantic.dev/docs/concepts/) for additional detail on spans, events, and further Logfire concepts.)

### Logging Custom Types

Logfire supports multiple ways to log custom types using `Display`, `Debug`, or `Serialize`. You can use sigils (similar to the `tracing` crate) to specify the formatting:

```rust
use serde::Serialize;

#[derive(Debug, Serialize)]
struct User {
    id: u64,
    name: String,
    email: String,
}

impl std::fmt::Display for User {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "User #{}: {}", self.id, self.name)
    }
}

let user = User {
    id: 123,
    name: "Alice".to_string(),
    email: "alice@example.com".to_string(),
};

// Using Display (default, concise, human-readable)
logfire::info!("User logged in: {user}", user = &user);
// Output: "User logged in: User #123: Alice"

// Using sigils for different formatting
logfire::info!("User (display): {user}", %user);  // % = Display
// Output: "User (display): User #123: Alice"

logfire::info!("User (debug): {user}", ?user);    // ? = Debug
// Output: "User (debug): User { id: 123, name: \"Alice\", email: \"alice@example.com\" }"

logfire::info!("User (json): {user}", #user);     // # = Serialize (requires serde)
// Output: "User (json): {\"id\":123,\"name\":\"Alice\",\"email\":\"alice@example.com\"}"

// You can also use explicit field names with sigils
logfire::info!("User: {u}", u = ? user);          // Explicit field name with Debug
logfire::info!("User: {u}", u = % user);          // Explicit field name with Display
logfire::info!("User: {u}", u = # user);          // Explicit field name with Serialize
```

**Sigil Reference:**
- No sigil or `%`: Use `Display` trait (human-readable output)
- `?`: Use `Debug` trait (developer-friendly, detailed output)
- `#`: Use `Serialize` trait (JSON format, requires `serde` feature)

The sigil syntax is inspired by the `tracing` crate and provides a concise way to control how values are formatted without explicit helper macros.

See additional examples in the [examples directory](https://github.com/pydantic/logfire-rust/tree/main/examples):

- [basic](https://github.com/pydantic/logfire-rust/tree/main/examples/basic.rs) - Simple file size counting example
- [custom_types](https://github.com/pydantic/logfire-rust/tree/main/examples/custom_types.rs) - Logging custom types with Display, Debug, and Serialize
- [axum webserver](https://github.com/pydantic/logfire-rust/tree/main/examples/axum.rs)
- [actix webserver](https://github.com/pydantic/logfire-rust/tree/main/examples/actix-web.rs)

### Integration

Logfire's Rust SDK is currently built directly upon [`tracing`](https://docs.rs/tracing/latest/tracing/) and [`opentelemetry`](https://github.com/open-telemetry/opentelemetry-rust/).

This means that anything instrumented using `tracing` will just work with `logfire` (however this SDK's macros contain some additional customizations on top of `tracing` for best support directly on the Logfire platform).

There is also an integration with `log`, so anything using `log` will also be captured by Logfire.

## Contributing

We'd love anyone interested to contribute to the Logfire SDK!

Please send us all your feedback and ideas about the current design and functionality of this SDK. Code contributions are also very welcome!

## Reporting a Security Vulnerability

See our [security policy](https://github.com/pydantic/logfire-rust/security).
