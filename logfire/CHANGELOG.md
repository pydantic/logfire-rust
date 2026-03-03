## [v0.9.0] (2025-11-06)

* support setting service resource attributes from environment variable by @davidhewitt in [#109](https://github.com/pydantic/logfire-rust/pull/109)
* add more `LogfireConverter` by @gabrielhsc95 in [#110](https://github.com/pydantic/logfire-rust/pull/110)
* properly handle options and type inference in macros by @davidhewitt in [#112](https://github.com/pydantic/logfire-rust/pull/112)
* implement `ConsoleOptions::with_colors` option by @davidhewitt in [#115](https://github.com/pydantic/logfire-rust/pull/115)
* add `force_flush` and documentation to support running on AWS Lambda by @davidhewitt in [#114](https://github.com/pydantic/logfire-rust/pull/114)

## [v0.8.2] (2025-09-10)

* removed unused import from README example by @PoorlyDefinedBehaviour in [#102](https://github.com/pydantic/logfire-rust/pull/102)
* Fix the `test_log_bridge_console_output` by @cetra3 in [#104](https://github.com/pydantic/logfire-rust/pull/104)
* Fix console logging with values by @cetra3 in [#108](https://github.com/pydantic/logfire-rust/pull/108)

## [v0.8.1] (2025-08-21)

* make macros hygienic by @davidhewitt in [#100](https://github.com/pydantic/logfire-rust/pull/100)

## [v0.8.0] (2025-08-21)

* logs: stop sending `logfire.msg`, test `event_name` properly by @davidhewitt in [#84](https://github.com/pydantic/logfire-rust/pull/84)
* chore: explicitly call shutdown on drop of `ShutdownHandler` by @the-wondersmith in [#85](https://github.com/pydantic/logfire-rust/pull/85)
* don't fabricate "panic" and "log message" event names by @davidhewitt in [#86](https://github.com/pydantic/logfire-rust/pull/86)
* create a proper `Logfire` type by @davidhewitt in [#87](https://github.com/pydantic/logfire-rust/pull/87)
* filter out default tracing span name by @davidhewitt in [#88](https://github.com/pydantic/logfire-rust/pull/88)
* feat: allow multiple `opentelemetry_sdk::Resource`s to be set via `AdvancedOptions` by @the-wondersmith in [#89](https://github.com/pydantic/logfire-rust/pull/89)
* allow picking up credentials from a .logfire directory by @davidhewitt in [#90](https://github.com/pydantic/logfire-rust/pull/90)
* make `LogfireTracingLayer` respect configured filters by @davidhewitt in [#93](https://github.com/pydantic/logfire-rust/pull/93)
* implement service_name, service_version, and deployment_environment config builders by @davidhewitt in [#96](https://github.com/pydantic/logfire-rust/pull/96)
* suppress telemetry from otel export by @davidhewitt in [#95](https://github.com/pydantic/logfire-rust/pull/95)
* handle panics by default by @davidhewitt in [#97](https://github.com/pydantic/logfire-rust/pull/97)
* Create functions to create `u64` and `f64` exponential histograms by @PoorlyDefinedBehaviour in [#94](https://github.com/pydantic/logfire-rust/pull/94)

## [v0.7.1] (2025-07-16)

* add `trace!` macro by @davidhewitt in [#80](https://github.com/pydantic/logfire-rust/pull/80)
* fix setting resource on custom log processor by @davidhewitt in [#79](https://github.com/pydantic/logfire-rust/pull/79)
* fix case where tracing enabled caused too-verbose logging by @davidhewitt in [#81](https://github.com/pydantic/logfire-rust/pull/81)

## [v0.7.0] (2025-07-16)

* export logs over logs stream by @davidhewitt in [#73](https://github.com/pydantic/logfire-rust/pull/73)
* send log bridge data over logs stream by @davidhewitt in [#74](https://github.com/pydantic/logfire-rust/pull/74)
* send tracing event logs over logs stream by @davidhewitt in [#75](https://github.com/pydantic/logfire-rust/pull/75)
* support tracing metrics layer by @davidhewitt in [#76](https://github.com/pydantic/logfire-rust/pull/76)

## [v0.6.1] (2025-06-26)

* fix location of panic messages by @davidhewitt in [#67](https://github.com/pydantic/logfire-rust/pull/67)
* test span / log syntax examples, make them consistent by @davidhewitt in [#69](https://github.com/pydantic/logfire-rust/pull/69)
* fix parent span id for tracing events by @davidhewitt in [#71](https://github.com/pydantic/logfire-rust/pull/71)

## [v0.6.0] (2025-06-23)

* Add include_timestamps config by @hramezani in [#42](https://github.com/pydantic/logfire-rust/pull/42)
* Add min_log_level config by @hramezani in [#43](https://github.com/pydantic/logfire-rust/pull/43)
* upgrade to opentelemetry 0.30 by @davidhewitt in [#54](https://github.com/pydantic/logfire-rust/pull/54)
* send tracing events as logs by @davidhewitt in [#56](https://github.com/pydantic/logfire-rust/pull/56)
* Support ident without value shorthand in macros by @davidhewitt in [#57](https://github.com/pydantic/logfire-rust/pull/57)
* add examples for `axum` and `actix-web` by @davidhewitt in [#59](https://github.com/pydantic/logfire-rust/pull/59)
* add `LogfireTracingLayer` as a public type by @davidhewitt in [#61](https://github.com/pydantic/logfire-rust/pull/61)
* fix deadlock in console caused by SimpleSpanProcessor by @davidhewitt in [#60](https://github.com/pydantic/logfire-rust/pull/60)
* remove APIs deprecated on 0.4 by @davidhewitt in [#62](https://github.com/pydantic/logfire-rust/pull/62)

## [v0.5.0] (2025-04-02)

* update to opentelemetry 0.29 by @davidhewitt in https://github.com/pydantic/logfire-rust/pull/40

## [v0.4.0] (2025-03-26)

* remove debug print, and add clippy lint to prevent future regressions by @davidhewitt in https://github.com/pydantic/logfire-rust/pull/32
* support `dotted.attribute` syntax in the macros by @davidhewitt in https://github.com/pydantic/logfire-rust/pull/34
* add `with_console_options` config, deprecate `console_mode` by @davidhewitt in https://github.com/pydantic/logfire-rust/pull/33
* rename `with_metrics_options` to `with_metrics` by @davidhewitt in https://github.com/pydantic/logfire-rust/pull/35
* add support for inferring token from region by @davidhewitt in https://github.com/pydantic/logfire-rust/pull/36

## [v0.3.1] (2025-03-25)

* fix double-print to console of tracing events outside spans by @davidhewitt in https://github.com/pydantic/logfire-rust/pull/30
* revert to only emit pending spans on first enter by @davidhewitt in https://github.com/pydantic/logfire-rust/pull/29

## [v0.3.0] (2025-03-25)

* support printing tracing events (to console) by @davidhewitt in https://github.com/pydantic/logfire-rust/pull/21
* fix double-print to console when `LOGFIRE_SEND_TO_LOGFIRE=no` by @davidhewitt in https://github.com/pydantic/logfire-rust/pull/23
* fix print to console to emit spans in the right order by @davidhewitt in https://github.com/pydantic/logfire-rust/pull/24
* support configuring metrics by @davidhewitt in https://github.com/pydantic/logfire-rust/pull/25
* print tracing event fields by @davidhewitt in https://github.com/pydantic/logfire-rust/pull/26
* emit log entries for top-level tracing events by @davidhewitt in https://github.com/pydantic/logfire-rust/pull/27

## [v0.2.0] (2025-03-19)

* Add `span_exporter()` helper to build a span exporter directly in [#20](https://github.com/pydantic/logfire-rust/pull/20)
* Fix `set_resource` not being called on span processors added with `with_additional_span_processor()` in [#20](https://github.com/pydantic/logfire-rust/pull/20)

## [v0.1.0] (2025-03-13)

Initial release.

[v0.1.0]: https://github.com/pydantic/logfire-rust/commits/v0.1.0
[v0.2.0]: https://github.com/pydantic/logfire-rust/compare/v0.1.0..v0.2.0
[v0.3.0]: https://github.com/pydantic/logfire-rust/compare/v0.2.0..v0.3.0
[v0.3.1]: https://github.com/pydantic/logfire-rust/compare/v0.3.0..v0.3.1
[v0.4.0]: https://github.com/pydantic/logfire-rust/compare/v0.3.1..v0.4.0
[v0.5.0]: https://github.com/pydantic/logfire-rust/compare/v0.4.0..v0.5.0
[v0.6.0]: https://github.com/pydantic/logfire-rust/compare/v0.5.0..v0.6.0
[v0.6.1]: https://github.com/pydantic/logfire-rust/compare/v0.6.0..v0.6.1
[v0.7.0]: https://github.com/pydantic/logfire-rust/compare/v0.6.1..v0.7.0
[v0.7.1]: https://github.com/pydantic/logfire-rust/compare/v0.7.0..v0.7.1
[v0.8.0]: https://github.com/pydantic/logfire-rust/compare/v0.7.1..v0.8.0
[v0.8.1]: https://github.com/pydantic/logfire-rust/compare/v0.8.0..v0.8.1
[v0.8.2]: https://github.com/pydantic/logfire-rust/compare/v0.8.1..v0.8.2
[v0.9.0]: https://github.com/pydantic/logfire-rust/compare/v0.8.2..v0.9.0
