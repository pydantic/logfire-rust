# Rust SDK for Pydantic Logfire

<p align="center">
  <a href="https://github.com/pydantic/logfire-rust/actions?query=event%3Apush+branch%3Amain+workflow%3ACI"><img src="https://github.com/pydantic/logfire-rust/actions/workflows/main.yml/badge.svg?event=push" alt="CI" /></a>
  <a href="https://codecov.io/gh/pydantic/logfire-rust"><img src="https://codecov.io/gh/pydantic/logfire-rust/graph/badge.svg?token=735CNGCGFD" alt="codecov" /></a>
  <a href="https://crates.io/crates/logfire"><img src="https://img.shields.io/crates/v/logfire.svg?logo=rust" alt="crates.io" /></a>
  <a href="https://github.com/pydantic/logfire-rust/blob/main/LICENSE"><img src="https://img.shields.io/github/license/pydantic/logfire-rust.svg" alt="license" /></a>
  <a href="https://github.com/pydantic/logfire"><img src="https://img.shields.io/crates/msrv/logfire.svg?logo=rust" alt="MSRV" /></a>
  <a href="https://logfire.pydantic.dev/docs/join-slack/"><img src="https://img.shields.io/badge/Slack-Join%20Slack-4A154B?logo=slack" alt="Join Slack" /></a>
</p>

> ***Initial release - feedback wanted!***
>
> This is an initial release of the Logfire Rust SDK. We've been using it internally to build Logfire for some time, and it is serving us well. As we're using it ourselves in production, we figured it's ready for everyone else also using Logfire.
>
> We are continually iterating to make this SDK better. We'd love your feedback on all aspects of the SDK and are keen to make the design as idiomatic and performant as possible.
>
> In particular, the current coupling to `tracing` is an open design point. By building on top of tracing we get widest compatibility and a relatively simple SDK, however to make Logfire-specific adjustments we might prefer in future to move `tracing` to be an optional integration.

From the team behind Pydantic, **Logfire** is an observability platform built on the same belief as our
open source library — that the most powerful tools can be easy to use.

This repository contains the Rust SDK for instrumenting with Logfire.

See also:
 - The [SDK documentation on docs.rs](https://docs.rs/logfire) for an API reference for this library.
 - The [Logfire documentation](https://logfire.pydantic.dev/docs/) for more information about Logfire in general.
 - The [Logfire GitHub repository](https://github.com/pydantic/logfire) for the source of the documentation, the Python SDK and an issue tracker for general questions about Logfire.

The Logfire server application for recording and displaying data is closed source.

## Using Logfire's Rust SDK

First [set up a Logfire project](https://logfire.pydantic.dev/docs/#logfire) and [create a write token](https://logfire.pydantic.dev/docs/how-to-guides/create-write-tokens/). You'll need to set this token as an environment variable (`LOGFIRE_TOKEN`) to export to Logfire.

Here's a simple manual tracing (aka logging) example:

```rust

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let shutdown_handler = logfire::configure()
        .install_panic_handler()
        .send_to_logfire(true)
        .console_mode(logfire::ConsoleMode::Fallback)
        .finish()?;

    logfire::info!("Hello, {name}!", name = "world");

    {
        let _span = logfire::span!(
            "Asking the user their {question}",
            question = "age",
        ).entered();

        println!("When were you born [YYYY-mm-dd]?");
        let mut dob = String::new();
        std::io::stdin().read_line(&mut dob)?;

        logfire::debug!("dob={dob}", dob = dob.trim().to_owned());
    }

    shutdown_handler.shutdown()?;
    Ok(())
}
```

### Integration

Logfire's Rust SDK is currently built directly upon [`tracing`](https://docs.rs/tracing/latest/tracing/) and [`opentelemetry`](https://github.com/open-telemetry/opentelemetry-rust/).

This means that anything instrumented using `tracing` will just work with `logfire` (however this SDK's macros contain some additional customizations on top of `tracing` for best support directly on the Logfire platform).

There is also an integration with `log`, so anything using `log` will also be captured by Logfire.

## Contributing

We'd love anyone interested to contribute to the Logfire SDK!

Please send us all your feedback and ideas about the current design and functionality of this SDK. Code contributions are also very welcome!

## Reporting a Security Vulnerability

See our [security policy](https://github.com/pydantic/logfire-rust/security).
